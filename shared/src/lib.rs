#![warn(clippy::pedantic, clippy::perf)]

use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

pub const SPEED: f32 = 1.;
pub const TICKRATE: u64 = 64;

#[derive(Default)]
pub struct ModuleLogLevels<'a> {
    pub off: &'a [&'a str],
    pub error: &'a [&'a str],
    pub warn: &'a [&'a str],
    pub info: &'a [&'a str],
    pub debug: &'a [&'a str],
    pub trace: &'a [&'a str],
}

pub fn enable_logging(log_file_name: &str, levels: ModuleLogLevels) -> anyhow::Result<()> {
    let mut dispatch = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339(std::time::SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Debug);

    for o in levels.off.into_iter() {
        dispatch = dispatch.level_for(o.to_string(), log::LevelFilter::Off);
    }

    for e in levels.error.into_iter() {
        dispatch = dispatch.level_for(e.to_string(), log::LevelFilter::Error);
    }

    for w in levels.warn.into_iter() {
        dispatch = dispatch.level_for(w.to_string(), log::LevelFilter::Error);
    }

    for i in levels.info.into_iter() {
        dispatch = dispatch.level_for(i.to_string(), log::LevelFilter::Info);
    }

    for d in levels.debug.into_iter() {
        dispatch = dispatch.level_for(d.to_string(), log::LevelFilter::Info);
    }

    for t in levels.trace.into_iter() {
        dispatch = dispatch.level_for(t.to_string(), log::LevelFilter::Info);
    }

    dispatch
        .level_for("tokio", log::LevelFilter::Info)
        // Output to stdout, files, and other Dispatch configurations
        .chain(std::io::stdout())
        .chain(fern::log_file(format!(
            "log/{}.{}-{}.log",
            log_file_name,
            std::process::id(),
            chrono::Local::now().format("%Y-%m-%d"),
        ))?)
        // Apply globally
        .apply()?;
    Ok(())
}

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
    Welcome { id: Uuid },
    InvalidMessage,
    PlayerJoined { id: Uuid, name: String },
    GoodBye(Uuid),
    PlayerChangedName { id: Uuid, new_name: String },
    NameNotAvailable,
    Update { spawns: usize },
    Finish { enemy_kills: usize },
    ChallengeReceived { request_id: Uuid, name: String },
    ChallengeDenied { request_id: Uuid },
    RequestReceived { request_id: Uuid },
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ClientMessage {
    Connect { name: String },
    GetPlayers,
    Disconnect { uid: Uuid },
    ChangeName { uid: Uuid, name: String },
    ChallengePlayer { uid: Uuid, name: String },
    AcceptChallenge { uid: Uuid, request_id: Uuid },
    DenyChallenge { uid: Uuid, request_id: Uuid },
    State { uid: Uuid, kills: usize },
}
