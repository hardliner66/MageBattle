#![warn(clippy::pedantic, clippy::perf)]

mod tcpstream;
mod ws;

use glam::Vec2;
use macroquad::prelude::{
    clear_background, color_u8, coroutines::start_coroutine, draw_rectangle, draw_texture_ex,
    is_key_down, next_frame, screen_height, screen_width, Color, DrawTextureParams, KeyCode, Rect,
    Texture2D, BLACK, WHITE,
};
use shared::{ClientMessage, Direction, RemoteState, ServerMessage, State};
use std::{io, sync::Arc};
use ws::Connection;

const CHAR_WIDTH: f32 = 16.;
const CHAR_HEIGHT: f32 = 16.;

pub struct Game {
    pub player_state: RemoteState,
    pub texture: Texture2D,
    pub quit: bool,
}

fn draw_box(pos: Vec2, size: Vec2) {
    let dimension = size * 2.;
    let upper_left = pos - size;
    draw_rectangle(upper_left.x, upper_left.y, dimension.x, dimension.y, BLACK);
}

#[must_use]
pub fn vec2_from_angle(angle: f32) -> Vec2 {
    let angle = angle - std::f32::consts::FRAC_PI_2;
    Vec2::new(angle.cos(), angle.sin())
}

impl Game {
    async fn new() -> anyhow::Result<Self> {
        let texture =
            Texture2D::from_file_with_format(include_bytes!("../assets/8Bit Wizard.png"), None);
        let game = Self {
            player_state: RemoteState::default(),
            texture,
            quit: false,
        };
        Ok(game)
    }

    pub fn handle_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::Welcome(id) => {
                self.player_state.id = id;
            }
            ServerMessage::GoodBye(_id) => {}
            ServerMessage::Update(remote_state) => {
                self.player_state.position = remote_state.position;
            }
        }
    }

    fn update(&mut self) {
        if is_key_down(KeyCode::Escape) {
            self.quit = true;
        }

        self.player_state.direction = match (
            is_key_down(KeyCode::A),
            is_key_down(KeyCode::W),
            is_key_down(KeyCode::S),
            is_key_down(KeyCode::D),
        ) {
            // left, up, down, right
            (true, true, true, true) => None,
            (true, true, false, true) => Some(Direction::Up),
            (true, true, true, false) => Some(Direction::Left),
            (true, true, false, false) => Some(Direction::UpLeft),
            (true, false, true, true) => Some(Direction::Down),
            (true, false, false, true) => None,
            (true, false, true, false) => Some(Direction::DownLeft),
            (true, false, false, false) => Some(Direction::Left),
            (false, true, true, true) => Some(Direction::Right),
            (false, true, false, true) => Some(Direction::UpRight),
            (false, true, true, false) => None,
            (false, true, false, false) => Some(Direction::Up),
            (false, false, true, true) => Some(Direction::DownRight),
            (false, false, false, true) => Some(Direction::Right),
            (false, false, true, false) => Some(Direction::Down),
            (false, false, false, false) => None,
        };

        if let None = self.player_state.direction {
            self.player_state.anim_id = 0;
        }

        if self.player_state.position.x > screen_width() {
            self.player_state.position.x = -CHAR_WIDTH;
        } else if self.player_state.position.x < -CHAR_WIDTH {
            self.player_state.position.x = screen_width();
        }
        if self.player_state.position.y > screen_height() {
            self.player_state.position.y = -CHAR_HEIGHT;
        } else if self.player_state.position.y < -CHAR_HEIGHT {
            self.player_state.position.y = screen_height();
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation
    )]
    pub fn draw_character(&self, state: &RemoteState) {
        let cols = (self.texture.width() / CHAR_WIDTH).floor() as usize;
        let index = state.anim_id % cols;
        let tx_x = index % cols;
        let tx_y = index / cols;
        draw_texture_ex(
            self.texture,
            state.position.x,
            state.position.y,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(
                    tx_x as f32 * CHAR_WIDTH,
                    tx_y as f32 * CHAR_HEIGHT,
                    CHAR_WIDTH,
                    CHAR_HEIGHT,
                )),
                ..Default::default()
            },
        );
    }

    pub fn draw(&self) {
        clear_background(color_u8!(0, 211, 205, 205));
        draw_box(Vec2::new(200f32, 200f32), Vec2::new(10f32, 10f32));
        self.draw_character(&self.player_state);
    }
}

pub async fn client_connect(connection: Arc<Connection>, url: &str) {
    if let Err(err) = connection.connect(url).await {
        log::error!("Failed to connect to {}: {}", url, err);
    }
}

pub fn client_send(msg: &ClientMessage, connection: &Arc<Connection>) {
    let bytes = serde_json::to_vec(&msg).expect("serialization failed");
    if let Err(err) = connection.send(bytes) {
        log::error!("Failed to send: {}", err);
        if let tungstenite::Error::Io(err) = err {
            if let io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted = err.kind() {
                log::error!("Connection lost, attempting to reconnect");
                connection.restart();
                start_coroutine(client_connect(
                    connection.clone(),
                    "ws://localhost:3030/game",
                ));
            }
        }
    }
}

pub fn client_receive(game: &mut Game, connection: &Arc<Connection>) {
    if let Some(msg) = connection.poll() {
        let msg: ServerMessage =
            serde_json::from_slice(msg.as_slice()).expect("deserialization failed");
        game.handle_message(msg);
    }
}

#[macroquad::main("game")]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let connection = Arc::new(Connection::new());
    let connection_coroutine = start_coroutine(client_connect(
        connection.clone(),
        "ws://localhost:3030/game",
    ));

    let mut game = Game::new().await?;
    loop {
        if connection_coroutine.is_done() {
            let state = ClientMessage::State(State {
                direction: game.player_state.direction,
            });
            client_send(&state, &connection);
            client_receive(&mut game, &connection);

            game.update();
            game.draw();
        }
        if game.quit {
            return Ok(());
        }
        next_frame().await;
    }
}
