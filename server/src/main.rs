#![warn(clippy::pedantic, clippy::perf)]

use shared::{ClientMessage, Direction, RemoteState, ServerMessage, SPEED, TICKRATE};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::{mpsc, RwLock};
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

struct User {
    tx: OutBoundChannel,
    state: RemoteState,
}
type Users = Arc<RwLock<HashMap<usize, User>>>;

static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);
fn send_welcome(out: &OutBoundChannel) -> usize {
    let id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);
    let states = ServerMessage::Welcome(id);
    send_msg(out, &states);
    id
}

fn send_msg(tx: &OutBoundChannel, msg: &ServerMessage) {
    let buffer = serde_json::to_vec(msg).unwrap();
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

async fn user_connected(ws: WebSocket, users: Users) {
    use futures_util::StreamExt;
    let (ws_sender, mut ws_receiver) = ws.split();
    let tx = create_send_channel(ws_sender);
    let my_id = send_welcome(&tx);
    log::debug!("new user connected: {}", my_id);
    {
        users.write().await.insert(
            my_id,
            User {
                tx,
                state: RemoteState {
                    id: my_id,
                    ..Default::default()
                },
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
            let mut users = users.write().await;
            if let Some(mut user) = users.get_mut(&my_id) {
                user_message(msg, &mut user).await;
            }
        }
    }
    log::debug!("user disconnected: {}", my_id);
    users.write().await.remove(&my_id);
    broadcast(ServerMessage::GoodBye(my_id), &users).await;
}

fn parse_message(msg: Message) -> Option<ClientMessage> {
    if msg.is_binary() {
        let msg = msg.into_bytes();
        serde_json::from_slice::<ClientMessage>(msg.as_slice()).ok()
    } else {
        None
    }
}

async fn user_message(msg: ClientMessage, user: &mut User) {
    match msg {
        ClientMessage::State(state) => {
            user.state.direction = state.direction;
        }
    }
}

async fn broadcast(msg: ServerMessage, users: &Users) {
    let users = users.read().await;
    for (_, User { tx, .. }) in users.iter() {
        send_msg(tx, &msg);
    }
}

fn update_state(state: &mut RemoteState) {
    match state.direction {
        Some(Direction::Up) => state.position.y -= SPEED,
        Some(Direction::UpRight) => {
            state.position.x += SPEED;
            state.position.y -= SPEED;
        }
        Some(Direction::Right) => state.position.x += SPEED,
        Some(Direction::DownRight) => {
            state.position.x += SPEED;
            state.position.y += SPEED;
        }
        Some(Direction::Down) => state.position.y += SPEED,
        Some(Direction::DownLeft) => {
            state.position.x -= SPEED;
            state.position.y += SPEED;
        }
        Some(Direction::Left) => state.position.x -= SPEED,
        Some(Direction::UpLeft) => {
            state.position.x -= SPEED;
            state.position.y -= SPEED;
        }
        None => (),
    }
}

async fn update_loop(users: Users) {
    loop {
        for (&_uid, user) in users.write().await.iter_mut() {
            update_state(&mut user.state);
            let state = ServerMessage::Update(user.state.clone());
            send_msg(&user.tx, &state);
        }
        tokio::time::sleep(std::time::Duration::from_millis(1000 / TICKRATE)).await;
    }
}

#[tokio::main]
#[allow(clippy::similar_names)]
async fn main() {
    pretty_env_logger::init();
    let status = warp::path!("status").map(move || warp::reply::html("hello"));

    let users = Users::default();

    let arc_users = users.clone();

    tokio::spawn(async move { update_loop(arc_users).await });

    let users = warp::any().map(move || users.clone());

    let game = warp::path("game")
        .and(warp::ws())
        .and(users)
        .map(|ws: warp::ws::Ws, users| ws.on_upgrade(move |socket| user_connected(socket, users)));
    let routes = status.or(game);
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}
