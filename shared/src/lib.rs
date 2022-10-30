#![warn(clippy::pedantic, clippy::perf)]

use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct RemoteState {
    pub id: usize,
    pub position: Vec2,
    pub rotation: f32,
}
#[derive(Deserialize, Serialize)]
pub enum ServerMessage {
    Welcome(usize),
    GoodBye(usize),
    Update(Vec<RemoteState>),
}
#[derive(Deserialize, Serialize, Clone)]
pub struct State {
    pub position: Vec2,
    pub rotation: f32,
}
#[derive(Deserialize, Serialize)]
pub enum ClientMessage {
    State(State),
}
