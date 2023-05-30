use crate::{
    broadcast,
    gameserver::{GameServerState, User},
    send_msg,
};
use async_trait::async_trait;
use coerce::actor::{
    context::ActorContext,
    message::{Handler, Message as ActorMessage},
    Actor,
};
use shared::{ClientMessage, ServerMessage, Uuid};

pub struct Lobby {
    pub game_server: GameServerState,
}

#[async_trait]
impl Actor for Lobby {}

pub struct NewUser(pub User);

impl ActorMessage for NewUser {
    type Result = Result<Uuid, User>;
}

#[async_trait]
impl Handler<NewUser> for Lobby {
    async fn handle(
        &mut self,
        NewUser(new_user): NewUser,
        _ctx: &mut ActorContext,
    ) -> Result<Uuid, User> {
        if let Some(_) = self
            .game_server
            .users
            .iter()
            .find(|(key, user)| user.name == new_user.name)
        {
            Err(new_user)
        } else {
            let id = Uuid::new_v4();
            self.game_server.users.insert(id, new_user);

            Ok(id)
        }
    }
}

pub struct ClientMessageWrapper {
    pub id: Uuid,
    pub msg: ClientMessage,
}

impl ActorMessage for ClientMessageWrapper {
    type Result = ();
}

async fn user_message(msg: ClientMessage, id: Uuid, game_server: &mut GameServerState) {
    match msg {
        ClientMessage::Connect { name } => {
            let msg = ServerMessage::PlayerJoined { id, name };
            for user in game_server.users.values() {
                send_msg(&user.tx, dbg!(&msg));
            }
        }
        ClientMessage::ChangeName { name } => {
            if game_server
                .users
                .iter()
                .find(|(_, user)| user.name.to_lowercase() == name.to_lowercase())
                .is_some()
            {
                if let Some((_, player)) = game_server.users.iter().find(|(uid, _)| **uid == id) {
                    send_msg(&player.tx, &ServerMessage::NameNotAvailable);
                }
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

#[async_trait]
impl Handler<ClientMessageWrapper> for Lobby {
    async fn handle(
        &mut self,
        ClientMessageWrapper { id, msg }: ClientMessageWrapper,
        _ctx: &mut ActorContext,
    ) {
        user_message(msg, id, &mut self.game_server).await
    }
}
