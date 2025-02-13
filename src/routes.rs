use actix_files::NamedFile;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder, Result};
use bitcoin::Address;
use serde_json::Value;
use std::str::FromStr;

use crate::{db_operations, nostr_notify};

#[get("/")]
pub async fn index(_req: HttpRequest) -> Result<NamedFile> {
    Ok(NamedFile::open("www/index.html")?)
}

#[post("/store-user")]
pub async fn store_user(payload: web::Json<Value>) -> impl Responder {
    if let Some(pubkey) = payload.get("pubkey").and_then(|v| v.as_str()) {
        let _ = db_operations::create_new_user(pubkey.to_string());
        nostr_notify::send_message("Welcome to utxo monitoring. You will receive notifications here if an address makes a move.".to_string(), pubkey.to_string());
        HttpResponse::Ok().body("PubKey stored successfully")
    } else {
        return HttpResponse::BadRequest().body("Invalid pubkey");
    }
}

#[post("/monitor-address")]
pub async fn store_monitored_addresses(
    req: HttpRequest,
    payload: web::Json<Value>,
) -> impl Responder {
    let pubkey: Option<String> = req.cookie("nostr_pubkey").map(|c| c.value().to_string());

    if let Some(pubkey) = pubkey {
        if let Some(address) = payload.get("address").and_then(|v| v.as_str()) {
            if let Ok(addr) = Address::from_str(&address) {
                let addr = Address::assume_checked(addr);
                db_operations::store_user_address(pubkey.clone(), addr.clone().to_string());
                nostr_notify::send_message(format!("Address added: {}", addr), pubkey);
                HttpResponse::Ok().body("Address stored successfully")
            } else {
                HttpResponse::BadRequest().body("Address not set")
            }
        } else {
            HttpResponse::BadRequest().body("Invalid payload: missing 'address'")
        }
    } else {
        HttpResponse::BadRequest().body("Pubkey not set")
    }
}

#[get("/monitor-address")]
pub async fn get_monitored_addresses(req: HttpRequest) -> impl Responder {
    let pubkey = req.cookie("nostr_pubkey").map(|c| c.value().to_string());

    if let Some(pubkey) = pubkey {
        let addrs = db_operations::get_tagged_addresses(pubkey);
        match addrs {
            Ok(user_addresses) => {
                return HttpResponse::Ok().json(user_addresses);
            }
            Err(e) => {
                println!("Error loading tagged addresses:: {}", e);
            }
        };
    }
    HttpResponse::Ok().json(Vec::<String>::new())
}
