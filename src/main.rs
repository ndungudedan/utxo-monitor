use bitcoin::address::Address;
use bitcoin::consensus::encode::deserialize;
use bitcoin::network::Network;
use bitcoin::p2p::message;
use bitcoin::Transaction;
use futures_util::{SinkExt, StreamExt};
use hex;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::{
    any::Any,
    ops::Add,
    process::{exit, Command},
};
use tokio::io::empty;
use tokio::sync::mpsc;
use tokio::task;
use uuid::Uuid;
use warp::ws::{Message, WebSocket};
use warp::Filter;
use zmq;

type Users = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Message>>>>;
type Pikachus = Arc<Mutex<HashMap<String, Vec<Address>>>>;

async fn fetch_previous_tx(prev_txid: &str, is_pruned: &bool) -> Option<Transaction> {
    if *is_pruned {
        let url = format!("https://mempool.space/api/tx/{}/hex", prev_txid);
        println!("{}", url);
        let response = reqwest::get(&url).await;
        match response {
            Ok(res) => {
                if res.status() == 200 {
                    let txt = res.text().await.ok().unwrap();
                    let tx_bytes = hex::decode(&txt.trim()).ok().unwrap();
                    return deserialize(&tx_bytes).ok();
                }
            }
            Err(e) => eprintln!("Error receiving message: {}", e),
        }
        None
    } else {
        let output = Command::new("bitcoin-cli")
            .arg("getrawtransaction")
            .arg(prev_txid)
            .arg("1")
            .output()
            .expect("Failed to execute bitcoin-cli");

        if output.status.success() {
            return deserialize(&output.stdout).ok();
        } else {
            None
        }
    }
}

fn is_bitcoin_node_pruned() -> bool {
    let output = Command::new("bitcoin-cli")
        .arg("getblockchaininfo")
        .output();
    match output {
        Ok(res) => {
            println!("{:?}", String::from_utf8_lossy(&res.stderr));
            if res.status.success() {
                let raw_json = String::from_utf8_lossy(&res.stdout);
                let json: Value = serde_json::from_str(&raw_json).expect("Failed to parse JSON");
                println!("✅ Bitcoin node network:: {}", json["chain"]);
                if let Some(pruned) = json["pruned"].as_bool() {
                    if pruned {
                        println!(
                            "✅ Bitcoin node is pruned. Prune height: {}",
                            json["pruneheight"]
                        );
                        return true;
                    }
                }
                false
            } else {
                false
            }
        }
        Err(er) => {
            println!("{}", er);
            false
        }
    }
}

#[tokio::main]
async fn main() {
    let users = Users::default();
    let pikachus: Pikachus = Pikachus::default();
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::filters::query::query::<HashMap<String, String>>())
        .and(with_users(users.clone()))
        .and(with_pikachus(pikachus.clone()))
        .map(
            |ws: warp::ws::Ws,
             params: HashMap<String, String>,
             users: Users,
             pikachus: Pikachus| {
                let session_id = params
                    .get("session_id")
                    .cloned()
                    .unwrap_or_else(|| Uuid::new_v4().to_string()); // Generate new UUID if missing

                ws.on_upgrade(move |socket| {
                    handle_connection(socket, session_id, users.clone(), pikachus.clone())
                })
            },
        );

    let routes = ws_route.with(warp::cors().allow_any_origin());

    task::spawn(async move {
        println!("🚀 WebSocket server running at ws://localhost:9090/ws");
        warp::serve(routes).run(([127, 0, 0, 1], 9090)).await;
    });

    let is_pruned = is_bitcoin_node_pruned();
    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB).expect("Failed to create socket");

    let zmq_url = "tcp://127.0.0.1:28333";
    subscriber
        .connect(zmq_url)
        .expect("Failed to connect to ZMQ");

    subscriber
        .set_subscribe(b"rawtx")
        .expect("Failed to subscribe to rawtx");

    println!("Listening for Bitcoin transactions on {}", zmq_url);
    let mut data: Vec<GenTransaction> = Vec::new();
    loop {
        let topic = subscriber.recv_string(0);
        match topic {
            Ok(Ok(topic_str)) => {
                let tx_data = subscriber.recv_bytes(0).expect("Failed to receive tx data");
                let tx_hex = hex::encode(&tx_data);

                println!("New transaction received:");
                println!("Topic: {:?}", topic_str);

                if let Ok(tx) = deserialize::<Transaction>(&tx_data) {
                    println!("\n🔹 **Detecting OUTGOING (Sender) Addresses**:");
                    let mut inputs = Vec::new();
                    for (i, input) in tx.input.iter().enumerate() {
                        let prev_txid = input.previous_output.txid.to_string();

                        println!("Input {}: Spends from previous TXID {}", i, prev_txid);

                        if let Some(prev_tx) = fetch_previous_tx(&prev_txid, &is_pruned).await {
                            println!("Fetched previous transaction: {}", prev_txid);
                            let temp = process_outputs(&prev_tx);
                            inputs.push(InputTrans {
                                txid: prev_txid,
                                output_address: temp,
                            });
                        } else {
                            println!("❌ Failed to fetch previous transaction for {}", prev_txid);
                        }
                    }

                    println!("\n🔹 **Detecting INCOMING (Receiver) Addresses**:");
                    let temp = process_outputs(&tx);
                    let genesis = GenTransaction {
                        txid: tx.compute_txid().to_string(),
                        output_address: temp,
                        input_address: inputs,
                    };
                    let urs = pikachus.lock().unwrap();
                    let genesis_outs: HashSet<_> = genesis.output_address.iter().collect();

                    urs.iter().for_each(|(user, user_addrs)| {
                        let matching_outs: Vec<_> = user_addrs
                            .iter()
                            .filter(|addr| genesis_outs.contains(addr))
                            .collect();

                        if !matching_outs.is_empty() {
                           let message = format!(
                                "Your watch list Address {:?} has been spent in this tx {}",
                                matching_outs, genesis.txid
                            );
                            println!("----- Attempting to send:: {}", message);
                            send_message_to_user(&users, user.to_string(), Message::text(message));
                        }

                        genesis.input_address.iter().for_each(|f|{
                            let genesis_ins:Vec<_> =f.output_address.iter().collect();
                            let matching_ins:Vec<_>=user_addrs.iter().filter(|addr| genesis_ins.contains(addr)).collect();
                            if !matching_ins.is_empty(){
                                let message = format!(
                                    "Your watch list Address {:?} has been spent as an input for this tx {}. The input was previously funnded by this tx: {}",
                                    matching_outs, genesis.txid,f.txid
                                );
                            println!("----- Attempting to send:: {}", message);
                                send_message_to_user(&users, user.to_string(), Message::text(message));
                            }

                        });


                    });
                    data.push(genesis);
                } else {
                    println!("Failed to decode transaction.");
                }
            }
            Ok(Err(e)) => println!("Received non-UTF8 topic: ",),
            Err(e) => eprintln!("Error receiving message: {}", e),
        }
        println!(
            "-----------Genesis Tree-------------------{}--------------------------",
            data.len()
        );
    }
}

fn process_outputs(tx: &Transaction) -> Vec<Address> {
    let mut outs: Vec<Address> = [].to_vec();
    for (i, output) in tx.output.iter().enumerate() {
        if let Ok(address) = Address::from_script(&output.script_pubkey, Network::Bitcoin) {
            println!(
                "Output {}: Address = {} (Amount: {} sats)",
                i, address, output.value
            );
            outs.push(address);
        } else {
            println!("Output {}: Could not decode address", i);
        }
    }
    outs
}

#[derive(Debug, Serialize)]
struct GenTransaction {
    txid: String,
    output_address: Vec<Address>,
    input_address: Vec<InputTrans>,
}

#[derive(Debug, Serialize)]
struct InputTrans {
    txid: String,
    output_address: Vec<Address>,
}

fn with_users(
    users: Users,
) -> impl Filter<Extract = (Users,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || users.clone())
}

fn with_pikachus(
    pikachus: Pikachus,
) -> impl Filter<Extract = (Pikachus,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || pikachus.clone())
}

async fn handle_connection(ws: WebSocket, session_id: String, users: Users, pikachus: Pikachus) {
    let (mut tx, mut rx) = ws.split();
    let (sender, mut receiver) = mpsc::unbounded_channel();

    println!("✅ User {} connected", session_id);
    users.lock().unwrap().insert(session_id.clone(), sender);

    tokio::spawn(async move {
        while let Some(Ok(msg)) = rx.next().await {
            if let Ok(text) = msg.to_str() {
                if let Ok(addr) = Address::from_str(text) {
                    let mut info = pikachus.lock().unwrap();
                    info.entry(session_id.clone())
                        .or_insert_with(Vec::new)
                        .push(Address::assume_checked(addr));
                    send_message_to_user(
                        &users,
                        session_id.clone(),
                        Message::text(format!("📩 Message Received {}: {}", session_id, text)),
                    );
                    println!("📩 Message from {}: {}", session_id, text);
                } else {
                }
            }
        }
    });

    while let Some(msg) = receiver.recv().await {
        let _ = tx.send(msg).await;
    }
}

fn send_message_to_user(users: &Users, session_id: String, message: Message) {
    let users = Arc::clone(users);
    task::spawn(async move {
        let users_locked = users.lock().unwrap();

        if let Some(tx) = users_locked.get(&session_id) {
            if tx.send(message).is_err() {
                println!("❌ User {} is disconnected", session_id);
            } else {
                println!("✅ Message sent to {}", session_id);
            }
        } else {
            println!("⚠️ User {} not found", session_id);
        }
    });
}

async fn broadcast_message(users: &Users, message: Message) {
    let mut disconnected_users = Vec::new(); // Collect disconnected users

    {
        let users_locked = users.lock().unwrap(); // Lock only once
        for (user_id, tx) in users_locked.iter() {
            if tx.send(message.clone()).is_err() {
                disconnected_users.push(user_id.clone()); // Collect disconnected user IDs
            }
        }
    } // 🔥 Unlock here before modifying the HashMap

    if !disconnected_users.is_empty() {
        let mut users_locked = users.lock().unwrap();
        for user_id in disconnected_users {
            users_locked.remove(&user_id); // Remove disconnected users
        }
    }
}
