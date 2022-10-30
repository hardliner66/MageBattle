#![warn(clippy::pedantic, clippy::perf)]

use glam::Vec2;
use serde::{Deserialize, Serialize};

pub const SPEED: f32 = 3.;
pub const TICKRATE: u64 = 64;

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
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

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RemoteState {
    pub id: usize,
    pub seed: u64,
    pub anim_id: usize,
    pub position: Vec2,
    pub direction: Option<Direction>,
}

impl Default for RemoteState {
    fn default() -> Self {
        Self {
            id: 0,
            seed: 0,
            anim_id: 0,
            position: Vec2::new(100f32, 100f32),
            direction: Default::default(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct WelcomeMessage {
    pub id: usize,
    pub seed: u64,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ServerMessage {
    Welcome(WelcomeMessage),
    GoodBye(usize),
    Update(RemoteState),
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct State {
    pub direction: Option<Direction>,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ClientMessage {
    State(State),
}
