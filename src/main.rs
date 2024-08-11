mod lobby;
mod ws;
use lobby::Lobby;
mod messages;
mod api;
use actix::Actor;
use actix_cors::Cors;
use actix_web::{App, HttpServer,web::Data};
use api::get_room_id;
use api::start_connection as start_connection_route;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;
//log
// use std::env;
// use log::{info, warn, error};

// fn some_function() {
//     info!("This is an informational message");
//     warn!("This is a warning message");
//     error!("This is an error message");
// }
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    //log
    // env::set_var("RUST_LOG", "trace");  // 可以設定為 error, warn, info, debug, trace
    // env_logger::init();
    // some_function();
    let room_id_map = Data::new(Mutex::new(HashMap::<String, Uuid>::new()));
    let chat_server = Lobby::new(room_id_map.clone()).start(); //create and spin up a lobby

    HttpServer::new(move || {
        App::new()
            // .wrap(
            //     Cors::default()
            //         .allowed_origin("http://example.com") 
            //         .allowed_methods(vec!["GET", "POST"]) 
            //         .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
            //         .allowed_header(http::header::CONTENT_TYPE)
            //         .supports_credentials(),
            // )

            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header(),
            )
            .service(start_connection_route) //register our route. rename with "as" import or naming conflict
            .app_data(Data::new(chat_server.clone())) //register the lobby
            .service(get_room_id)
            .app_data(room_id_map.clone())
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
