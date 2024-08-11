use crate::messages::{
    ClientActorMessage, Connect, Disconnect, JoinPayload, MessagePayload, Type, VoteMode, WsMessage,
};
use actix::prelude::{Actor, Context, Handler, Recipient};
use actix_web::web::Data;
use serde_json::to_string;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use uuid::Uuid;

type Socket = Recipient<WsMessage>;
const SAVE_MESSAGE_MAX_LEN: usize = 50;
pub struct Lobby {
    sessions: HashMap<Uuid, Socket>, //使用者的uuid對應他的WsConn的ADDR
    rooms: HashMap<Uuid, RoomData>,  //房間的uuid 對應 每個房間使用者的uuid集合
    room_id_map: Data<Mutex<HashMap<String, Uuid>>>,
}
enum GamePhase {
    Waiting,
    Selection,
    Voting,
    Ending,
}
struct VoteData {
    agree:usize,
    disagree:usize,
    agree_list:HashSet<Uuid>,
    disagree_list:HashSet<Uuid>,
}
impl VoteData {
    fn new() -> Self {
        VoteData {
            agree: 0,
            disagree: 0,
            agree_list: HashSet::new(),
            disagree_list: HashSet::new(),
        }
    }
    
    fn vote(&mut self, user_id: Uuid, is_agree: &str) -> bool {
        if self.agree_list.contains(&user_id) || self.disagree_list.contains(&user_id) {
            return false;
        }
    
        match is_agree.to_lowercase().as_str() {
            "true" => {
                self.agree += 1;
                self.agree_list.insert(user_id);
            }
            "false" => {
                self.disagree += 1;
                self.disagree_list.insert(user_id);
            }
            _ => {
                println!("is agree: {}",is_agree);
                return false;
            }
        }
        true
    }
    fn total_votes(&self) -> usize {
        self.agree_list.len() + self.disagree_list.len()
    }
}

pub struct RoomData {
    users: HashSet<Uuid>,
    user_name_list:HashMap<Uuid,String>,
    data: VecDeque<(String, String)>,
    max_size: usize,
    vote_mode: Option<VoteMode>,
    game_phase: GamePhase,
    current_restaurant_vote:VoteData,
}

impl RoomData {
    pub fn new() -> Self {
        Self {
            users: HashSet::new(),
            user_name_list:HashMap::new(),
            data: VecDeque::with_capacity(SAVE_MESSAGE_MAX_LEN),
            max_size: SAVE_MESSAGE_MAX_LEN,
            vote_mode: None,
            game_phase: GamePhase::Waiting,
            current_restaurant_vote: VoteData::new(),
        }
    }

    pub fn insert_data(&mut self, new_data: (String, String)) {
        if self.data.len() >= self.max_size {
            self.data.pop_front();
        }
        self.data.push_back(new_data);
    }
}

impl Lobby {
    pub fn new(room_id_map: Data<Mutex<HashMap<String, Uuid>>>) -> Self {
        Self {
            sessions: HashMap::new(),
            rooms: HashMap::new(),
            room_id_map,
        }
    }
    fn remove_room_id(&self, room_id: &Uuid) {
        let mut key_to_remove = None;
        {
            let room_id_map = self.room_id_map.lock().unwrap();
            for (key, value) in room_id_map.iter() {
                if value == room_id {
                    key_to_remove = Some(key.clone());
                    break;
                }
            }
        }

        if let Some(key) = key_to_remove {
            let mut room_id_map = self.room_id_map.lock().unwrap();
            room_id_map.remove(&key);
        }
    }
    fn send_message(&self, message: &str, id_to: &Uuid) {
        if let Some(socket_recipient) = self.sessions.get(id_to) {
            let payload = MessagePayload {
                r#type: "message".to_string(),
                message: message.to_string(),
            };

            match to_string(&payload) {
                Ok(json_message) => {
                    let _ = socket_recipient.do_send(WsMessage(json_message));
                }
                Err(e) => {
                    println!("Failed to serialize message payload: {}", e);
                }
            }
        } else {
            println!("Attempting to send message but couldn't find user id.");
        }
    }
    fn send_join_message(&self, message: &str, room_id: &Uuid) {
        if let Some(room_data) = self.rooms.get_key_value(room_id) {
            let users = &room_data.1.users;
            let payload = JoinPayload {
                r#type: "join".to_string(),
                message: message.to_string(),
                length: users.len(),
            };
            for user in users {
                if let Some(socket_recipient) = self.sessions.get(user) {
                    match to_string(&payload) {
                        Ok(json_message) => {
                            let _ = socket_recipient.do_send(WsMessage(json_message));
                        }
                        Err(e) => {
                            println!("Failed to serialize message payload: {}", e);
                        }
                    }
                }
            }
        } else {
            println!("Attempting to send message but couldn't find user id.");
        }
    }
    fn send_selection_restaurant(&self, room_id: &Uuid, restaurant_name: &str, remark: &str) {
        if let Some(room_data) = self.rooms.get_key_value(room_id) {
            let users = &room_data.1.users;

            let custom_payload = serde_json::json!({
                "type": "add restaurant",
                "restaurant_name": restaurant_name,
                "remark": remark,
            });

            match to_string(&custom_payload) {
                Ok(json_message) => {
                    for user in users {
                        if let Some(socket_recipient) = self.sessions.get(user) {
                            let _ = socket_recipient.do_send(WsMessage(json_message.clone()));
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to serialize custom payload to JSON: {}", e);
                }
            }
        } else {
            println!("Attempting to send message but couldn't find room id.");
        }
    }
    fn send_vote_result(&self, room_id: &Uuid, result: &str, reject_list: HashSet<Uuid>) {
        if let Some(room_data) = self.rooms.get(room_id) {
            let users = &room_data.users;
            let reject_names: Vec<String> = reject_list
                .iter()
                .filter_map(|user_id| room_data.user_name_list.get(user_id).cloned())
                .collect();
            let payload = serde_json::json!({
                "type": "vote result",
                "result": result,
                "reject_list": reject_names,
            });

            match to_string(&payload) {
                Ok(json_message) => {
                    for user_id in users {
                        if let Some(socket_recipient) = self.sessions.get(user_id) {
                            let _ = socket_recipient.do_send(WsMessage(json_message.clone()));
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to serialize vote result payload to JSON: {}", e);
                }
            }
        } else {
            println!("Attempting to send vote result but couldn't find room id.");
        }
    }
    fn send_current_vote_count(&self, room_id: &Uuid) {
        if let Some(room_data) = self.rooms.get(room_id) {
            let vote_data = &room_data.current_restaurant_vote;
            let payload = serde_json::json!({
                "type": "current vote count",
                "agree": vote_data.agree,
                "disagree": vote_data.disagree,
            });

            match to_string(&payload) {
                Ok(json_message) => {
                    for user_id in &room_data.users {
                        if let Some(socket_recipient) = self.sessions.get(user_id) {
                            let _ = socket_recipient.do_send(WsMessage(json_message.clone()));
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to serialize current vote count payload to JSON: {}", e);
                }
            }
        } else {
            println!("Attempting to send current vote count but couldn't find room id.");
        }
    }
}

impl Actor for Lobby {
    type Context = Context<Self>;
}
//<>裡面是被發送了甚麼消息要做出相應的handle()
impl Handler<Disconnect> for Lobby {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        if self.sessions.remove(&msg.id).is_some() {
            self.rooms
                .get(&msg.room_id)
                .unwrap()
                .users
                .iter()
                .filter(|conn_id| *conn_id.to_owned() != msg.id)
                .for_each(|user_id| {
                    self.send_message(&format!("{} disconnected.", &msg.name), user_id)
                });
            self.rooms.get_mut(&msg.room_id).unwrap().users.remove(&msg.id);
            self.rooms.get_mut(&msg.room_id).unwrap().user_name_list.remove(&msg.id);
            if let Some(lobby) = self.rooms.get_mut(&msg.room_id) {
                if lobby.users.len() > 1 {
                    lobby.users.remove(&msg.id);
                } else {
                    //如果剩下一個人直接移除房間就好(能夠順便將他移除)
                    self.remove_room_id(&msg.room_id);
                    self.rooms.remove(&msg.room_id);
                    println!("刪除房間，房間對應關係 {:?}", self.room_id_map);
                }
            }
        }
    }
}
//<>裡面是被發送了甚麼消息要做出相應的handle()
impl Handler<Connect> for Lobby {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        //entry是進入房間or_insert_with搭配entry如果進不去(沒有對應的key)，則創建新的房間並加入value
        self.rooms
            .entry(msg.lobby_id)
            .or_insert_with(|| RoomData::new())
            .users
            .insert(msg.self_id);
        //讓lobby知道使用者的id對應哪個ws地址，讓lobby廣播的時候可以知道要給誰
        self.sessions.insert(msg.self_id, msg.addr);

        // self.send_message(&format!("your id is {}", msg.self_id), &msg.self_id);
        //傳輸歷史紀錄給新進來的人知道
        for (keys, value) in self.rooms.get(&msg.lobby_id).unwrap().data.iter() {
            let restaurant_info: serde_json::Value =
                serde_json::from_str(&value).expect("Failed to deserialize JSON");
            let restaurant_name = restaurant_info["restaurant_name"]
                .as_str()
                .unwrap_or("Unknown");
            let remark = restaurant_info["remark"].as_str().unwrap_or("");
            self.send_message(&format!("{} suggest[restaurant: {}, remark: {}]", keys, restaurant_name, remark), &msg.self_id);
        }
        // self.send_message("--------------history~--------------", &msg.self_id);
    }
}
//<>裡面是被發送了甚麼消息要做出相應的handle()
impl Handler<ClientActorMessage> for Lobby {
    type Result = ();

    fn handle(&mut self, msg: ClientActorMessage, _ctx: &mut Context<Self>) -> Self::Result {
        match msg.r#type {
            Type::Join => {
                println!("{} join the room.", msg.name);
                self.rooms.get_mut(&msg.room_id).unwrap().user_name_list.insert(msg.id.clone(), msg.name.clone());
                self.send_join_message(&format!("{} join the room.", msg.name), &msg.room_id);
            }
            Type::Message => {
                self.rooms
                    .get(&msg.room_id)
                    .unwrap()
                    .users
                    .iter()
                    .for_each(|client| {
                        self.send_message(&format!("{} say: {}", msg.name, msg.msg), client)
                    });
                self.rooms
                    .get_mut(&msg.room_id)
                    .unwrap()
                    .insert_data((msg.name, msg.msg))
            }
            Type::Vote => {
                let room_data = self.rooms.get(&msg.room_id).unwrap();
                match room_data.game_phase {
                    GamePhase::Voting=>{
                        self.rooms.get_mut(&msg.room_id).unwrap().current_restaurant_vote.vote(msg.id, &msg.msg);
                        let room_data = self.rooms.get(&msg.room_id).unwrap();
                        self.send_current_vote_count(&msg.room_id);
                        //總投票數要等於人數才行，進行結果判斷
                        if room_data.current_restaurant_vote.total_votes() == room_data.users.len() {
                            if let Some(mode) = &room_data.vote_mode {
                                match mode {
                                    VoteMode::MajorityDecision=>{
                                        if room_data.current_restaurant_vote.agree >= room_data.current_restaurant_vote.disagree{
                                            self.send_vote_result(&msg.room_id, "pass", room_data.current_restaurant_vote.disagree_list.clone());
                                            self.rooms.get_mut(&msg.room_id).unwrap().game_phase = GamePhase::Ending;
                                        }
                                        else{
                                            self.send_vote_result(&msg.room_id, "failed", room_data.current_restaurant_vote.disagree_list.clone());
                                            self.rooms.get_mut(&msg.room_id).unwrap().game_phase = GamePhase::Selection;
                                        }
                                    },
                                    VoteMode::ConsensusDecision=>{
                                        if room_data.current_restaurant_vote.disagree == 0{
                                            self.send_vote_result(&msg.room_id, "pass", room_data.current_restaurant_vote.disagree_list.clone());
                                            self.rooms.get_mut(&msg.room_id).unwrap().game_phase = GamePhase::Ending;
                                        }
                                        else{
                                            self.send_vote_result(&msg.room_id, "failed", room_data.current_restaurant_vote.disagree_list.clone());
                                            self.rooms.get_mut(&msg.room_id).unwrap().game_phase = GamePhase::Selection;
                                        }
                                    },
                                }
                            }else {
                                println!("未設定投票模式")
                            }
                        }
                    },
                    _=>(),
                }
            },
            Type::SetVoteMode(vote_mode) => match self.rooms.get(&msg.room_id).unwrap().vote_mode {
                None => {
                    self.rooms.get_mut(&msg.room_id).unwrap().vote_mode = Some(vote_mode);
                }
                _ => (),
            },
            Type::AddRestaurant => {
                let restaurant_info: serde_json::Value =
                    serde_json::from_str(&msg.msg).expect("Failed to deserialize JSON");
                let room_data = self.rooms.get(&msg.room_id).unwrap();
                let restaurant_name = restaurant_info["restaurant_name"]
                    .as_str()
                    .unwrap_or("Unknown");
                let remark = restaurant_info["remark"].as_str().unwrap_or("");
                println!("新的餐廳: {}, 備註: {}", restaurant_name, remark);
                match room_data.game_phase {
                    GamePhase::Waiting => {
                        self.rooms.get_mut(&msg.room_id).unwrap().game_phase = GamePhase::Voting;
                        self.send_selection_restaurant(&msg.room_id, restaurant_name, remark);
                        self.rooms
                            .get_mut(&msg.room_id)
                            .unwrap()
                            .insert_data((msg.name, msg.msg))
                    }
                    GamePhase::Selection => {
                        self.rooms.get_mut(&msg.room_id).unwrap().game_phase = GamePhase::Voting;
                        self.send_selection_restaurant(&msg.room_id, restaurant_name, remark);
                        self.rooms
                            .get_mut(&msg.room_id)
                            .unwrap()
                            .insert_data((msg.name, msg.msg))
                    }
                    _ => (),
                }
            }
        }
    }
}
