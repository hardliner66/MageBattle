use std::collections::HashMap;

use shared::Uuid;

use crate::OutBoundChannel;

#[derive(Clone)]
pub struct User {
    pub tx: OutBoundChannel,
    pub in_game: bool,
    pub name: String,
}

#[derive(Default)]
pub struct GameServerState {
    pub users: HashMap<Uuid, User>,
}
