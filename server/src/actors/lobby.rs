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
            .find(|(_, user)| user.name == new_user.name)
        {
            Err(new_user)
        } else {
            let id = Uuid::new_v4();
            self.game_server.users.insert(id, new_user.clone());

            let msg = ServerMessage::PlayerJoined {
                id,
                name: new_user.name.clone(),
            };
            for user in self.game_server.users.values() {
                send_msg(&user.tx, &msg);
            }
            if let Some((uid, user)) = self
                .game_server
                .users
                .iter()
                .find(|(_, user)| user.name.to_lowercase() == new_user.name.to_lowercase())
            {
                for (other_uid, other_user) in self.game_server.users.iter() {
                    if uid != other_uid {
                        let msg = ServerMessage::PlayerJoined {
                            id: other_uid.clone(),
                            name: other_user.name.clone(),
                        };
                        send_msg(&user.tx, &msg);
                    }
                }
            }

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
        ClientMessage::Connect { .. } => {}
        ClientMessage::GetPlayers => {
            if let Some((uid, user)) = game_server.users.iter().find(|(uid2, _)| id == **uid2) {
                for (other_uid, other_user) in game_server.users.iter() {
                    if uid != other_uid {
                        let msg = ServerMessage::PlayerJoined {
                            id: other_uid.clone(),
                            name: other_user.name.clone(),
                        };
                        send_msg(&user.tx, &msg);
                    }
                }
            }
        }
        ClientMessage::ChangeName { uid: _, name } => {
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
        ClientMessage::ChallengePlayer { uid: _, name } => {
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
        ClientMessage::AcceptChallenge {
            uid: _,
            request_id: _,
        } => todo!(),
        ClientMessage::DenyChallenge {
            uid: _,
            request_id: _,
        } => todo!(),
        ClientMessage::State { uid: _, kills: _ } => todo!(),
        ClientMessage::Disconnect { uid } => {
            game_server.users.remove(&uid);
        }
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
