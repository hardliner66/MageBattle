#![warn(clippy::pedantic, clippy::perf)]

use std::net::SocketAddr;

use actors::Lobby;
use clap::Parser;
use coerce::actor::{new_actor, LocalActorRef};
use shared::{deserialize, serialize, ClientMessage, ServerMessage, Uuid};
use tokio::sync::mpsc;
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

mod actors;
mod gameserver;

use gameserver::{GameServerState, User};

use crate::actors::{ClientMessageWrapper, NewUser};

fn send_welcome(out: &OutBoundChannel, id: Uuid) -> Uuid {
    let states = ServerMessage::Welcome { id };
    send_msg(out, &states);
    id
}

fn send_msg(tx: &OutBoundChannel, msg: &ServerMessage) {
    let buffer = serialize(msg).unwrap();
    let msg = Message::binary(buffer);
    tx.send(Ok(msg)).unwrap();
}

type OutBoundChannel = mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>;

fn create_send_channel(
    ws_sender: futures_util::stream::SplitSink<WebSocket, Message>,
) -> OutBoundChannel {
    use futures_util::FutureExt;
    use futures_util::StreamExt;
    use tokio_stream::wrappers::UnboundedReceiverStream;
    let (sender, receiver) = mpsc::unbounded_channel();
    let rx = UnboundedReceiverStream::new(receiver);
    tokio::task::spawn(rx.forward(ws_sender).map(|result| {
        if let Err(e) = result {
            log::error!("websocket send error: {}", e);
        }
    }));
    sender
}

async fn user_connected(ws: WebSocket, lobby: LocalActorRef<Lobby>) {
    use futures_util::StreamExt;
    let (ws_sender, mut ws_receiver) = ws.split();
    let tx = create_send_channel(ws_sender);

    let mut player_name = String::new();
    while let Some(result) = ws_receiver.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                log::warn!("websocket err: '{}'", e);
                send_msg(&tx, &ServerMessage::InvalidMessage);
                return;
            }
        };
        log::debug!("user sent message: {:?}", msg);
        if let Some(msg) = parse_message(msg) {
            match msg {
                ClientMessage::Connect { name } => player_name = name,
                _ => {}
            }
        }
    }

    let user = lobby
        .send(NewUser(User {
            tx: tx.clone(),
            name: player_name,
            in_game: false,
        }))
        .await;

    if let Ok(id) = user.unwrap() {
        let id = send_welcome(&tx, id);
        log::debug!("new user connected: {}", id);

        while let Some(result) = ws_receiver.next().await {
            let msg = match result {
                Ok(msg) => msg,
                Err(e) => {
                    log::warn!("websocket err (id={}): '{}'", id, e);
                    break;
                }
            };
            log::debug!("user sent message: {:?}", msg);

            if let Some(msg) = parse_message(msg) {
                if lobby.send(ClientMessageWrapper { id, msg }).await.is_err() {
                    break;
                }
            }
        }
        log::debug!("user disconnected: {}", id);
    } else {
        send_msg(&tx, &ServerMessage::NameNotAvailable);
    }
}

fn parse_message(msg: Message) -> Option<ClientMessage> {
    if msg.is_binary() {
        let msg = msg.into_bytes();
        deserialize::<ClientMessage>(msg.as_slice()).ok()
    } else {
        None
    }
}

async fn broadcast(game_server: &GameServerState, msg: ServerMessage) {
    for (_, User { tx, .. }) in game_server.users.iter() {
        send_msg(tx, &msg);
    }
}

#[derive(Parser)]
struct Arguments {
    #[arg(short, long)]
    listen: Option<String>,
    #[arg(short, long)]
    seed: Option<usize>,
}

#[tokio::main]
#[allow(clippy::similar_names)]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let args = Arguments::parse();

    let status = warp::path!("status").map(move || warp::reply::html("hello"));

    let lobby = new_actor(actors::Lobby {
        game_server: GameServerState::default(),
    })
    .await
    .unwrap();

    let lobby = warp::any().map(move || lobby.clone());

    let game = warp::path("game")
        .and(warp::ws())
        .and(lobby)
        .map(move |ws: warp::ws::Ws, lobby| {
            ws.on_upgrade(move |socket| user_connected(socket, lobby))
        });

    let routes = status.or(game);

    warp::serve(routes)
        .run(
            args.listen
                .unwrap_or_else(|| "127.0.0.1:3030".to_owned())
                .parse::<SocketAddr>()?,
        )
        .await;

    Ok(())
}
