#![warn(clippy::pedantic, clippy::perf)]

use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

pub const SPEED: f32 = 1.;
pub const TICKRATE: u64 = 64;

#[cfg(feature = "json")]
pub fn serialize<T>(value: &T) -> anyhow::Result<Vec<u8>>
where
    T: ?Sized + Serialize,
{
    Ok(serde_json::to_vec(value)?)
}

#[cfg(feature = "binary")]
pub fn serialize<T>(value: &T) -> anyhow::Result<Vec<u8>>
where
    T: ?Sized + Serialize,
{
    Ok(bincode::serialize(value)?)
}

#[cfg(feature = "json")]
pub fn deserialize<'a, T>(v: &'a [u8]) -> anyhow::Result<T>
where
    T: serde::de::Deserialize<'a>,
{
    Ok(serde_json::from_slice::<T>(v)?)
}

#[cfg(feature = "binary")]
pub fn deserialize<'a, T>(v: &'a [u8]) -> anyhow::Result<T>
where
    T: serde::de::Deserialize<'a>,
{
    Ok(bincode::deserialize(v)?)
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ServerMessage {
    #[serde(rename = "w")]
    Welcome {
        #[serde(rename = "i")]
        id: Uuid,
    },
    #[serde(rename = "im")]
    InvalidMessage,
    #[serde(rename = "j")]
    PlayerJoined {
        #[serde(rename = "i")]
        id: Uuid,
        #[serde(rename = "n")]
        name: String,
    },
    #[serde(rename = "gb")]
    GoodBye(Uuid),
    #[serde(rename = "ucn")]
    PlayerChangedName {
        #[serde(rename = "i")]
        id: Uuid,
        #[serde(rename = "n")]
        new_name: String,
    },
    #[serde(rename = "n")]
    NameNotAvailable,
    #[serde(rename = "u")]
    Update {
        #[serde(rename = "s")]
        spawns: usize,
    },
    #[serde(rename = "f")]
    Finish {
        #[serde(rename = "k")]
        enemy_kills: usize,
    },
    #[serde(rename = "cr")]
    ChallengeReceived {
        #[serde(rename = "rid")]
        request_id: Uuid,
        #[serde(rename = "n")]
        name: String,
    },
    #[serde(rename = "cd")]
    ChallengeDenied {
        #[serde(rename = "rid")]
        request_id: Uuid,
    },
    #[serde(rename = "rr")]
    RequestReceived {
        #[serde(rename = "rid")]
        request_id: Uuid,
    },
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ClientMessage {
    #[serde(rename = "c")]
    Connect {
        #[serde(rename = "n")]
        name: String,
    },
    #[serde(rename = "cn")]
    ChangeName {
        #[serde(rename = "n")]
        name: String,
    },
    #[serde(rename = "cp")]
    ChallengePlayer {
        #[serde(rename = "n")]
        name: String,
    },
    #[serde(rename = "ac")]
    AcceptChallenge {
        #[serde(rename = "rid")]
        request_id: Uuid,
    },
    #[serde(rename = "dc")]
    DenyChallenge {
        #[serde(rename = "rid")]
        request_id: Uuid,
    },
    #[serde(rename = "s")]
    State {
        #[serde(rename = "k")]
        kills: usize,
    },
}
