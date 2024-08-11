use actix::prelude::{Message, Recipient};
use uuid::Uuid;
use serde::{Deserialize,Serialize};
#[derive(Message)]
#[rtype(result = "()")]
pub struct WsMessage(pub String);

#[derive(Message)]
#[rtype(result = "()")]
pub struct Connect {
    pub addr: Recipient<WsMessage>,
    pub lobby_id: Uuid,
    pub self_id: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: Uuid,
    pub room_id: Uuid,
    pub name: String,
}
pub enum Type {
    Join,
    Message,
    Vote,
    SetVoteMode(VoteMode),
    AddRestaurant
}
#[derive(Message)]
#[rtype(result = "()")]
pub struct ClientActorMessage {
    pub r#type:Type,
    pub id: Uuid,
    pub msg: String,
    pub room_id: Uuid,
    pub name: String,
}
#[derive(Deserialize)]
pub enum VoteMode {
    //多數決
    #[serde(rename = "majority decision")]
    MajorityDecision,
    //合意投票
    #[serde(rename = "consensus decision")]
    ConsensusDecision,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum IncomingMessage {
    #[serde(rename = "join")]
    Join { name: String },
    #[serde(rename = "message")]
    Message { message: String },
    #[serde(rename = "set vote mode")]
    SetVoteMode {vote_mode: VoteMode},
    #[serde(rename = "add restaurant")]
    AddRestarant{
        restaurant_name:String,
        remark:String,
    },
    #[serde(rename = "vote")]
    Vote{is_agree:bool},
}

#[derive(Serialize)]
pub struct MessagePayload {
    pub r#type: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct JoinPayload {
    pub r#type: String,
    pub message: String,
    pub length: usize,
}