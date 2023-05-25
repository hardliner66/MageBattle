#![warn(clippy::pedantic, clippy::perf)]

use clap::Parser;
use shared::{deserialize, serialize, ClientMessage, ServerMessage, Uuid};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::{mpsc, RwLock};
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

#[derive(Default)]
struct GameServerState {
    users: HashMap<Uuid, User>,
}

struct User {
    tx: OutBoundChannel,
    in_game: bool,
    name: String,
}
type GameServer = Arc<RwLock<GameServerState>>;

fn send_welcome(out: &OutBoundChannel, seed: u64) -> Uuid {
    let id = Uuid::new_v4();
    let states = ServerMessage::Welcome { id };
    send_msg(out, &states);
    id
}

fn send_msg(tx: &OutBoundChannel, msg: &ServerMessage) {
    let buffer = serialize(msg).unwrap();
    let msg = Message::binary(buffer);
    tx.send(Ok(msg)).unwrap();
}

struct ClientMessageWrapper {
    id: Uuid,
    msg: ClientMessage,
}

type OutBoundChannel = mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>;
type ClientChannelSender = mpsc::UnboundedSender<ClientMessageWrapper>;
type ClientChannelReceiver = mpsc::UnboundedReceiver<ClientMessageWrapper>;

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

async fn user_connected(
    ws: WebSocket,
    sender: ClientChannelSender,
    game_server: GameServer,
    seed: u64,
) {
    use futures_util::StreamExt;
    let (ws_sender, mut ws_receiver) = ws.split();
    let tx = create_send_channel(ws_sender);
    let my_id = send_welcome(&tx, seed);
    log::debug!("new user connected: {}", my_id);
    {
        game_server.write().await.users.insert(
            my_id,
            User {
                tx,
                name: String::new(),
                in_game: false,
            },
        );
    }
    while let Some(result) = ws_receiver.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                log::warn!("websocket err (id={}): '{}'", my_id, e);
                break;
            }
        };
        log::debug!("user sent message: {:?}", msg);

        if let Some(msg) = parse_message(msg) {
            if sender
                .send(ClientMessageWrapper { id: my_id, msg })
                .is_err()
            {
                break;
            }
        }
    }
    log::debug!("user disconnected: {}", my_id);
    game_server.write().await.users.remove(&my_id);
    broadcast(&game_server, ServerMessage::GoodBye(my_id)).await;
}

fn parse_message(msg: Message) -> Option<ClientMessage> {
    if msg.is_binary() {
        let msg = msg.into_bytes();
        deserialize::<ClientMessage>(msg.as_slice()).ok()
    } else {
        None
    }
}

async fn user_message(msg: ClientMessage, id: Uuid, game_server: &GameServer) {
    match msg {
        ClientMessage::Connect { name } => {
            let msg = ServerMessage::PlayerJoined { id, name };
            for user in game_server.read().await.users.values() {
                send_msg(&user.tx, &msg);
            }
        }
        ClientMessage::ChangeName { name } => {
            if game_server
                .read()
                .await
                .users
                .iter()
                .find(|(_, user)| user.name.to_lowercase() == name.to_lowercase())
                .is_some()
            {
                ServerMessage::NameNotAvailable { name };
            } else {
                broadcast(
                    game_server,
                    ServerMessage::PlayerChangedName { id, new_name: name },
                )
                .await;
            }
        }
        ClientMessage::ChallengePlayer { name } => {
            if let Some((_, player)) = game_server
                .read()
                .await
                .users
                .iter()
                .find(|(_, user)| user.name.to_lowercase() == name.to_lowercase())
            {
                let request_id = Uuid::new_v4();
                send_msg(
                    &player.tx,
                    &ServerMessage::ChallengeReceived {
                        request_id: request_id.clone(),
                        name: player.name.clone(),
                    },
                );
                send_msg(&player.tx, &ServerMessage::RequestReceived { request_id });
            }
        }
        ClientMessage::AcceptChallenge { request_id } => todo!(),
        ClientMessage::DenyChallenge { request_id } => todo!(),
        ClientMessage::State { kills } => todo!(),
    }
}

async fn broadcast(game_server: &GameServer, msg: ServerMessage) {
    let game_server = game_server.read().await;
    for (_, User { tx, .. }) in game_server.users.iter() {
        send_msg(tx, &msg);
    }
}

async fn update_loop(mut rx: ClientChannelReceiver, game_server: GameServer) {
    loop {
        while let Some(ClientMessageWrapper { id, msg }) = rx.recv().await {
            let user_still_here = game_server.read().await.users.contains_key(&id);
            if user_still_here {
                user_message(msg, id, &game_server).await;
                // match answer {
                //     Some(Answer::Broadcast(msg)) => {
                //         for (_, user) in users.write().await.iter_mut() {
                //             send_msg(&user.tx, &msg)
                //         }
                //     }
                //     Some(Answer::Response(msg)) => users
                //         .write()
                //         .await
                //         .get_mut(&id)
                //         .map(|user| send_msg(&user.tx, &msg))
                //         .unwrap_or(()),
                //     Some(Answer::SendTo { other, msg }) => users
                //         .write()
                //         .await
                //         .get_mut(&other)
                //         .map(|user| send_msg(&user.tx, &msg))
                //         .unwrap_or(()),
                //     None => (),
                // }
            }
        }
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

    let game_server = GameServer::default();
    let seed: u64 = rand::random();

    let arc_game_server = game_server.clone();

    let (sender, receiver) = mpsc::unbounded_channel();

    tokio::spawn(async move { update_loop(receiver, arc_game_server).await });

    let game_server = warp::any().map(move || game_server.clone());
    let seed = warp::any().map(move || seed);

    let game = warp::path("game")
        .and(warp::ws())
        .and(game_server)
        .and(seed)
        .map(move |ws: warp::ws::Ws, game_server, seed| {
            let sender = sender.clone();
            ws.on_upgrade(move |socket| user_connected(socket, sender, game_server, seed))
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
