use std::collections::HashMap;

use crate::lobby::Lobby;
use crate::ws::WsConn;
use actix::Addr;
use actix_web::{
    get, post,
    web::{Data, Path, Payload},
    Error, HttpRequest, HttpResponse,
};
use actix_web_actors::ws;
use rand::Rng;
use std::sync::Mutex;
use uuid::Uuid;

#[get("/{group_id}")]
pub async fn start_connection(
    req: HttpRequest,
    stream: Payload,
    path: Path<String>,
    srv: Data<Addr<Lobby>>,
    room_id_map: Data<Mutex<HashMap<String, Uuid>>>,
) -> Result<HttpResponse, Error> {
    let group_id_str = path.into_inner();
    let room_id_map = room_id_map.lock().unwrap();
    if let Some(group_id) = room_id_map.get(&group_id_str) {
        println!("group_id:{}",group_id);
        let ws = WsConn::new(group_id.clone(), srv.get_ref().clone());

        let resp = ws::start(ws, &req, stream)?;
        Ok(resp)
    }else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[post("/get_room_id")]
pub async fn get_room_id(room_id_map: Data<Mutex<HashMap<String, Uuid>>>) -> HttpResponse {
    let mut rng = rand::thread_rng();
    let mut new_id;

    loop {
        let random_number: u32 = rng.gen_range(10000..99999);
        new_id = random_number.to_string();

        let mut room_id_map = room_id_map.lock().unwrap();
        if !room_id_map.contains_key(&new_id) {
            let namespace = Uuid::NAMESPACE_DNS;
            let new_uuid = Uuid::new_v5(&namespace, new_id.as_bytes());
            room_id_map.insert(new_id.clone(), new_uuid);
            break;
        }
    }
    println!("加入房間，房間對應關係 {:?}",room_id_map);
    HttpResponse::Ok().body(new_id)
}
