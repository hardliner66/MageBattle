#![warn(clippy::pedantic, clippy::perf)]

mod tcpstream;
mod ws;

use clap::Parser;
use glam::Vec2;
use lazy_static::lazy_static;
use macroquad::prelude::{
    clear_background, color_u8,
    coroutines::{start_coroutine, wait_seconds},
    draw_rectangle, draw_texture_ex, is_key_down, next_frame, screen_height, screen_width, Color,
    DrawTextureParams, KeyCode, Rect, Texture2D, BLACK, WHITE,
};
use shared::{deserialize, serialize, ClientMessage, ServerMessage, Uuid, SPEED};
use std::{collections::HashMap, io, sync::Arc};
use ws::Connection;

const CHAR_WIDTH: f32 = 16.;
const CHAR_HEIGHT: f32 = 16.;

#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Up,
    UpRight,
    Right,
    DownRight,
    Down,
    DownLeft,
    Left,
    UpLeft,
}

#[derive(Default, Clone)]
pub struct OnlineState {
    pub id: Uuid,
}

#[derive(Default, Clone)]
pub struct InGame {
    seed: u64,
    anim_id: usize,
    position: Vec2,
    kills: usize,
}

#[derive(Default, Clone)]
pub struct Base {
    name: String,
}

#[derive(Clone)]
pub enum PlayerState {
    Offline {
        base: Base,
    },
    Online {
        base: Base,
        online_state: OnlineState,
    },
    InGame {
        base: Base,
        online_state: OnlineState,
        ingame: InGame,
    },
}

pub struct RemotePlayerState {
    name: String,
}

pub enum Command {
    Connect,
}

pub struct Game {
    pub command: Option<Command>,
    pub player_state: PlayerState,
    pub players: HashMap<Uuid, RemotePlayerState>,
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
            command: None,
            player_state: PlayerState::Offline {
                base: Base {
                    name: String::new(),
                },
            },
            players: HashMap::new(),
            texture,
            quit: false,
        };
        Ok(game)
    }

    pub fn handle_message(&mut self, msg: ServerMessage) {
        let next_state = match &mut self.player_state {
            PlayerState::Offline { base } => match msg {
                ServerMessage::Welcome { id } => Some(PlayerState::Online {
                    base: base.clone(),
                    online_state: OnlineState { id },
                }),
                // ServerMessage::InvalidMessage => todo!(),
                // ServerMessage::NameNotAvailable => todo!(),
                _ => None,
            },
            PlayerState::Online { base, online_state } => match msg {
                ServerMessage::GoodBye(id) => {
                    if id != online_state.id {
                        self.players.remove(&id);
                    } else {
                        self.quit = true;
                    }
                    Some(PlayerState::Offline { base: base.clone() })
                }
                ServerMessage::PlayerChangedName { id, new_name } => {
                    if online_state.id == id {
                        base.name = new_name;
                    } else {
                        if let Some(player) = self.players.get_mut(&id) {
                            player.name = new_name;
                        }
                    }
                    None
                }
                ServerMessage::PlayerJoined { id, name } => {
                    self.players.insert(id, RemotePlayerState { name });
                    None
                }
                ServerMessage::ChallengeReceived { request_id, name } => todo!(),
                ServerMessage::ChallengeDenied { request_id } => todo!(),
                ServerMessage::RequestReceived { request_id } => todo!(),
                ServerMessage::InvalidMessage => todo!(),
                _ => None,
            },
            PlayerState::InGame {
                base,
                online_state,
                ingame,
            } => todo!(),
        };
    }

    fn update(&mut self) {
        if is_key_down(KeyCode::Escape) {
            self.quit = true;
        }

        if is_key_down(KeyCode::Space) {
            match &mut self.player_state {
                PlayerState::Offline { base } => todo!(),
                PlayerState::Online { base, online_state } => todo!(),
                PlayerState::InGame {
                    base,
                    online_state,
                    ingame,
                } => ingame.kills += 1,
            }
        }

        let direction = match (
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

        if let PlayerState::InGame {
            base,
            online_state,
            ingame,
        } = &mut self.player_state
        {
            ingame.anim_id = 0;

            match direction {
                Some(Direction::Up) => ingame.position.y -= SPEED,
                Some(Direction::UpRight) => {
                    ingame.position.x += SPEED;
                    ingame.position.y -= SPEED;
                }
                Some(Direction::Right) => ingame.position.x += SPEED,
                Some(Direction::DownRight) => {
                    ingame.position.x += SPEED;
                    ingame.position.y += SPEED;
                }
                Some(Direction::Down) => ingame.position.y += SPEED,
                Some(Direction::DownLeft) => {
                    ingame.position.x -= SPEED;
                    ingame.position.y += SPEED;
                }
                Some(Direction::Left) => ingame.position.x -= SPEED,
                Some(Direction::UpLeft) => {
                    ingame.position.x -= SPEED;
                    ingame.position.y -= SPEED;
                }
                None => (),
            }

            if ingame.position.x > screen_width() {
                ingame.position.x = -CHAR_WIDTH;
            } else if ingame.position.x < -CHAR_WIDTH {
                ingame.position.x = screen_width();
            }
            if ingame.position.y > screen_height() {
                ingame.position.y = -CHAR_HEIGHT;
            } else if ingame.position.y < -CHAR_HEIGHT {
                ingame.position.y = screen_height();
            }
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation
    )]
    pub fn draw_character(&self, state: &PlayerState) {
        let cols = (self.texture.width() / CHAR_WIDTH).floor() as usize;
        match &self.player_state {
            PlayerState::InGame {
                base,
                online_state,
                ingame,
            } => {
                let index = ingame.anim_id % cols;
                let tx_x = index % cols;
                let tx_y = index / cols;
                draw_texture_ex(
                    self.texture,
                    ingame.position.x,
                    ingame.position.y,
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

                egui_macroquad::ui(|egui_ctx| {
                    egui::Window::new("debug").show(egui_ctx, |ui| {
                        ui.label(&format!("Kills: {}", ingame.kills));
                    });
                });
            }
            _ => todo!(),
        }

        // Draw things before egui

        egui_macroquad::draw();
    }

    pub fn draw(&self) {
        clear_background(color_u8!(0, 211, 205, 205));
        draw_box(Vec2::new(200f32, 200f32), Vec2::new(10f32, 10f32));
        self.draw_character(&self.player_state);
    }
}

pub async fn client_connect(connection: Arc<Connection>, url: String) {
    while let Err(err) = connection.connect(&url).await {
        log::error!("{}, attempting again in 1 second", err);
        wait_seconds(1.0).await;
    }
    log::info!("Connection established successfully");
}

pub fn client_send(msg: &ClientMessage, connection: &Arc<Connection>) {
    let bytes = serialize(&msg).expect("serialization failed");
    if let Err(err) = connection.send(bytes) {
        log::error!("Failed to send: {}", err);
        if let tungstenite::Error::Io(err) = err {
            if let io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted = err.kind() {
                log::error!("Connection lost, attempting to reconnect");
                connection.restart();
                let address = format!(
                    "ws://{}/game",
                    ARGS.address
                        .clone()
                        .unwrap_or_else(|| "localhost:3030".to_string())
                );

                start_coroutine(client_connect(connection.clone(), address));
            }
        }
    }
}

pub fn client_receive(game: &mut Game, connection: &Arc<Connection>) {
    if let Some(msg) = connection.poll() {
        let msg: ServerMessage = deserialize(msg.as_slice()).expect("deserialization failed");
        game.handle_message(msg);
    }
}

#[derive(Parser)]
struct Arguments {
    #[arg(short, long)]
    address: Option<String>,
}

lazy_static! {
    /// This is an example for using doc comment attributes
    static ref ARGS: Arguments = Arguments::parse();
}

#[macroquad::main("game")]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let args = Arguments::parse();

    let address = format!(
        "ws://{}/game",
        args.address.unwrap_or_else(|| "localhost:3030".to_string())
    );

    let connection = Arc::new(Connection::new());
    let connection_coroutine = start_coroutine(client_connect(connection.clone(), address));

    let mut game = Game::new().await?;
    loop {
        if connection_coroutine.is_done() {
            match &game.command {
                Some(cmd) => match cmd {
                    Command::Connect => match &game.player_state {
                        PlayerState::Offline { base } => {
                            let state = ClientMessage::Connect {
                                name: base.name.clone(),
                            };
                            client_send(&state, &connection);
                        }
                        PlayerState::Online { base, online_state } => todo!(),
                        PlayerState::InGame {
                            base,
                            online_state,
                            ingame,
                        } => todo!(),
                    },
                },
                None => todo!(),
            };

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
