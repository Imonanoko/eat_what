use crate::lobby::Lobby;
use crate::messages::{ClientActorMessage, Connect, Disconnect, WsMessage,IncomingMessage, Type};
use actix::ActorFutureExt;
use actix::{fut, ActorContext, ContextFutureSpawner, WrapFuture};
use actix::{Actor, Addr, Running, StreamHandler};
use actix::{AsyncContext, Handler};
use actix_web_actors::ws;
use std::time::{Duration, Instant};
use uuid::Uuid;
use serde_json::from_str;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(3);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);
//send：返回一个 Future，可以等待消息完成並獲取結果。
//do_send：立即發送消息，不返回结果，不等待消息處理完成。
pub struct WsConn {
    room: Uuid,
    lobby_addr: Addr<Lobby>, //用來實現WsConn跟Lobby的通訊
    hb: Instant,
    id: Uuid,
    name:String,
}

impl WsConn {
    pub fn new(room: Uuid, lobby: Addr<Lobby>) -> WsConn {
        WsConn {
            id: Uuid::new_v4(),
            room,
            hb: Instant::now(),
            lobby_addr: lobby,
            name:"".to_string(),
        }
    }
}

impl Actor for WsConn {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        //WsConn的ADDR讓Lobby知道要傳給誰
        let addr = ctx.address();
        self.lobby_addr
            .send(Connect {
                addr: addr.recipient(),
                lobby_id: self.room,
                self_id: self.id,
            })
            //轉換成actor
            .into_actor(self)
            //處理轉換後的結果
            .then(|res, _, ctx| {
                match res {
                    Ok(_res) => (),
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            //等待處理完才會下一步
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        self.lobby_addr.do_send(Disconnect {
            id: self.id,
            room_id: self.room,
            name: (*self.name).to_string(),
        });
        Running::Stop
    }
}

impl WsConn {
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        //每隔一段時間會自動觸發一次
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!("Disconnecting failed heartbeat");
                ctx.stop();
                return;
            }
            //更新hb的時間
            ctx.ping(b"hi");
        });
    }
}
//處理從客戶端收到的訊息
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConn {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            Ok(ws::Message::Continuation(_)) => {
                ctx.stop();
            }
            Ok(ws::Message::Nop) => (),
            Ok(ws::Message::Text(s)) => {
                // 解析json
                let result: serde_json::Result<IncomingMessage> = from_str(&s);
                match result {
                    Ok(parsed_msg) => {
                        match parsed_msg {
                            IncomingMessage::Join { name } => {
                                self.name = name;
                                self.lobby_addr.do_send(ClientActorMessage {
                                    r#type:Type::Join,
                                    id: self.id,
                                    msg: self.name.clone(),
                                    room_id: self.room,
                                    name: self.name.clone(),
                                });
                            }
                            IncomingMessage::Message { message } => {
                                self.lobby_addr.do_send(ClientActorMessage {
                                    r#type:Type::Message,
                                    id: self.id,
                                    msg: message,
                                    room_id: self.room,
                                    name: self.name.clone(),
                                });
                            }
                            IncomingMessage::SetVoteMode { vote_mode } => {
                                let _= self.lobby_addr.send(ClientActorMessage {
                                    r#type: Type::SetVoteMode(vote_mode),
                                    id: self.id,
                                    room_id: self.room,
                                    name: self.name.clone(),
                                    msg: "".to_string(),
                                });
                            }
                            IncomingMessage::AddRestarant { restaurant_name, remark } => {
                                if restaurant_name.trim().is_empty() {
                                    return;
                                }
                                let restaurant_info = serde_json::json!({
                                    "restaurant_name": restaurant_name,
                                    "remark": remark
                                });
            
                                let json_string = serde_json::to_string(&restaurant_info).expect("Failed to serialize to JSON");
            
                                self.lobby_addr.do_send(ClientActorMessage {
                                    r#type: Type::AddRestaurant,
                                    id: self.id,
                                    msg: json_string,
                                    room_id: self.room,
                                    name: self.name.clone(),
                                });
                            }
                            IncomingMessage::Vote { is_agree } =>{
                                self.lobby_addr.do_send(ClientActorMessage {
                                    r#type:Type::Vote,
                                    id: self.id,
                                    msg: is_agree.to_string(),//會變成true跟false
                                    room_id: self.room,
                                    name: self.name.clone(),
                                });
                            }
                        }
                    }
                    Err(e) => {
                        println!("Failed to parse JSON: {}", e);
                    }
                }
            }
            Err(e) => std::panic::panic_any(e),
        }
    }
}

impl Handler<WsMessage> for WsConn {
    type Result = ();

    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}
