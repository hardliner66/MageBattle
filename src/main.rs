#![warn(clippy::pedantic, clippy::perf)]

mod tcpstream;
mod ws;

use glam::Vec2;
use macroquad::prelude::{
    clear_background, color_u8, coroutines::start_coroutine, draw_rectangle, draw_texture_ex,
    is_key_down, next_frame, screen_height, screen_width, Color, DrawTextureParams, KeyCode, Rect,
    Texture2D, BLACK, WHITE,
};
use shared::{ClientMessage, RemoteState, ServerMessage, State};
use std::{io, sync::Arc};
use ws::Connection;

const PLANE_WIDTH: f32 = 32.;
const PLANE_HEIGHT: f32 = 32.;

pub struct Game {
    pub player_state: RemoteState,
    pub remote_states: Vec<RemoteState>,
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
            Texture2D::from_file_with_format(include_bytes!("../assets/planes.png"), None);
        let game = Self {
            player_state: RemoteState {
                id: 0,
                position: Vec2::new(100f32, 100f32),
                rotation: 0f32,
            },
            remote_states: vec![],
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
            ServerMessage::GoodBye(id) => {
                self.remote_states.retain(|s| s.id != id);
            }
            ServerMessage::Update(remote_states) => {
                self.remote_states = remote_states;
            }
        }
    }

    fn update(&mut self) {
        const ROT_SPEED: f32 = 0.015;
        const SPEED: f32 = 0.6;

        if is_key_down(KeyCode::Escape) {
            self.quit = true;
        }
        if is_key_down(KeyCode::A) {
            self.player_state.rotation += ROT_SPEED;
        }
        if is_key_down(KeyCode::D) {
            self.player_state.rotation -= ROT_SPEED;
        }

        self.player_state.position += vec2_from_angle(self.player_state.rotation) * SPEED;

        for state in &mut self.remote_states {
            state.position += vec2_from_angle(state.rotation) * SPEED;
        }

        if self.player_state.position.x > screen_width() {
            self.player_state.position.x = -PLANE_WIDTH;
        } else if self.player_state.position.x < -PLANE_WIDTH {
            self.player_state.position.x = screen_width();
        }
        if self.player_state.position.y > screen_height() {
            self.player_state.position.y = -PLANE_HEIGHT;
        } else if self.player_state.position.y < -PLANE_HEIGHT {
            self.player_state.position.y = screen_height();
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation
    )]
    pub fn draw_plane(&self, state: &RemoteState) {
        let cols = (self.texture.width() / PLANE_WIDTH).floor() as usize;
        let index = state.id % 10;
        let tx_x = index % cols;
        let tx_y = index / cols;
        draw_texture_ex(
            self.texture,
            state.position.x,
            state.position.y,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(
                    tx_x as f32 * PLANE_WIDTH,
                    tx_y as f32 * PLANE_HEIGHT,
                    PLANE_WIDTH,
                    PLANE_HEIGHT,
                )),
                rotation: state.rotation,
                ..Default::default()
            },
        );
    }

    pub fn draw(&self) {
        clear_background(color_u8!(0, 211, 205, 205));
        draw_box(Vec2::new(200f32, 200f32), Vec2::new(10f32, 10f32));
        self.draw_plane(&self.player_state);
        for state in &self.remote_states {
            self.draw_plane(state);
        }
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
                position: game.player_state.position,
                rotation: game.player_state.rotation,
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
