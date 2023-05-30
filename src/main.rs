#![warn(clippy::pedantic, clippy::perf)]

mod tcpstream;
mod ws;

use clap::Parser;
use glam::Vec2;
use lazy_static::lazy_static;
use macroquad::prelude::{
    clear_background, color_u8,
    coroutines::{start_coroutine, wait_seconds, Coroutine},
    draw_rectangle, draw_texture_ex, is_key_down, next_frame, screen_height, screen_width, Color,
    DrawTextureParams, KeyCode, Rect, Texture2D, BLACK, WHITE,
};
use serde::{Deserialize, Serialize};
use shared::{deserialize, serialize, ClientMessage, ServerMessage, Uuid, SPEED};
use std::{cell::RefCell, collections::HashMap, io, sync::Arc};
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

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Base {
    name: String,
    server: String,
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

impl PlayerState {
    fn base(&self) -> &Base {
        match self {
            PlayerState::Online { base, .. }
            | PlayerState::InGame { base, .. }
            | PlayerState::Offline { base } => base,
        }
    }
}

pub struct RemotePlayerState {
    name: String,
}

pub enum Command {
    Connect(String),
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

fn address_from_server(server: &str) -> String {
    format!("ws://{}/game", server)
}

impl Game {
    async fn new(address: Option<String>) -> anyhow::Result<Self> {
        let config_path = std::path::PathBuf::from("settings.json");
        let mut base = if config_path.exists() {
            serde_json::from_str(&std::fs::read_to_string(config_path)?)?
        } else {
            Base::default()
        };
        if let Some(address) = address {
            base.server = address;
        }
        if base.server.trim() == "" {
            base.server = "localhost:3030".to_string()
        }
        let texture =
            Texture2D::from_file_with_format(include_bytes!("../assets/8Bit Wizard.png"), None);
        let game = Self {
            command: None,
            player_state: PlayerState::Offline { base },
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
                    online_state: OnlineState { id: dbg!(id) },
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

        if let Some(state) = next_state {
            self.player_state = state;
        }
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
    pub fn draw_state(&mut self) {
        let cols = (self.texture.width() / CHAR_WIDTH).floor() as usize;
        match &mut self.player_state {
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
            PlayerState::Offline { base } => {
                egui_macroquad::ui(|egui_ctx| {
                    egui::Window::new("UI").show(egui_ctx, |ui| {
                        ui.label("Username");
                        ui.text_edit_singleline(&mut base.name);
                        ui.label("Server");
                        ui.text_edit_singleline(&mut base.server);
                        if ui.button("Connect").clicked() {
                            self.command = Some(Command::Connect(base.server.clone()));
                        }
                    });
                });
            }
            PlayerState::Online { base, online_state } => {
                egui_macroquad::ui(|egui_ctx| {
                    egui::Window::new("UI").show(egui_ctx, |ui| {
                        ui.text_edit_singleline(&mut base.name);
                        if ui.button("Connect").clicked() {}
                    });
                });
            }
        }

        // Draw things before egui

        egui_macroquad::draw();
    }

    pub fn draw(&mut self) {
        clear_background(color_u8!(0, 211, 205, 205));
        self.draw_state();
    }
}

pub async fn client_connect(connection: Arc<Connection>, url: String) {
    while let Err(err) = connection.connect(&url).await {
        log::error!("{}, attempting again in 1 second", err);
        wait_seconds(1.0).await;
    }
    log::info!("Connection established successfully");
}

pub fn client_send(game: &Game, msg: &ClientMessage, connection: &Arc<Connection>) {
    let bytes = serialize(&msg).expect("serialization failed");
    if let Err(err) = connection.send(bytes) {
        log::error!("Failed to send: {}", err);
        if let tungstenite::Error::Io(err) = err {
            if let io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted = err.kind() {
                log::error!("Connection lost, attempting to reconnect");
                connection.restart();

                start_coroutine(client_connect(
                    connection.clone(),
                    address_from_server(&game.player_state.base().server),
                ));
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

#[macroquad::main("game")]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let args = Arguments::parse();

    let connection = Arc::new(Connection::new());

    let mut game = Game::new(args.address).await?;
    let mut is_online = false;
    loop {
        let next_state = match &game.command {
            Some(cmd) => match cmd {
                Command::Connect(address) => match &game.player_state {
                    PlayerState::Offline { base } => {
                        let connection_coroutine =
                            start_coroutine(client_connect(connection.clone(), address.to_owned()));

                        while !connection_coroutine.is_done() {}
                        is_online = true;
                        let state = ClientMessage::Connect {
                            name: base.name.clone(),
                        };
                        client_send(&game, &state, &connection);
                        None
                    }
                    PlayerState::Online { base, online_state } => todo!(),
                    PlayerState::InGame {
                        base,
                        online_state,
                        ingame,
                    } => todo!(),
                },
            },
            None => None,
        };
        game.command = None;

        if let Some(state) = next_state {
            game.player_state = state;
        }

        if is_online {
            client_receive(&mut game, &connection);
        }

        game.update();
        game.draw();

        if game.quit {
            std::fs::write(
                "settings.json",
                serde_json::to_string_pretty(game.player_state.base())?,
            )?;
            return Ok(());
        }
        next_frame().await;
    }
}
