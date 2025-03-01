use actix_web::{App, HttpServer, Result};
use bitcoin::address::Address;
use bitcoin::consensus::encode::deserialize;
use bitcoin::network::Network;
use bitcoin::Transaction;
use hex;
use models::{GenTransaction, InputTrans};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{Error, ErrorKind};
use std::process::{exit, Command};
use std::str::FromStr;
use tokio::task;
use zmq;

pub mod db;
pub mod db_operations;
pub mod models;
pub mod nostr_notify;
pub mod routes;
pub mod schema;
pub mod tests;

// Maps nostr_pubkey -> [Bitcoin RecordType]
type Pikachus = HashMap<String, Vec<RecordType>>;

#[actix_web::main]
async fn main() -> Result<()> {
    task::spawn(async move {
        println!("🚀 HTTP server running at 127.0.0.1:9090");
        if let Err(e) = HttpServer::new(move || {
            App::new()
                .service(routes::index)
                .service(routes::store_user)
                .service(routes::store_monitored_addresses)
                .service(routes::get_monitored_addresses)
        })
        .bind("127.0.0.1:9090")
        .expect("Failed to bind to port 9090")
        .run()
        .await
        {
            eprintln!("❌ Server error: {:?}", e);
        }
    });

    let is_pruned = is_bitcoin_node_pruned()?;
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

    loop {
        let topic = subscriber.recv_string(0);
        match topic {
            Ok(Ok(_)) => {
                let tx_data = subscriber.recv_bytes(0).expect("Failed to receive tx data");
                let tx_hex = hex::encode(&tx_data);

                // println!("New transaction received:: {:?}", tx_hex);

                if let Ok(tx) = deserialize::<Transaction>(&tx_data) {
                    find_address_match(tx, is_pruned).await;
                } else {
                    println!("Failed to decode transaction.");
                }
            }
            Ok(Err(_)) => println!("Received non-UTF8 topic:"),
            Err(e) => eprintln!("Error receiving message: {}", e),
        }
    }
}

fn process_outputs(tx: &Transaction) -> Vec<Address> {
    // println!("\n🔹 **Detecting DELIVERY (Receiver) Addresses**:");
    let mut outs: Vec<Address> = [].to_vec();
    for (i, output) in tx.output.iter().enumerate() {
        if let Ok(addr) = Address::from_script(&output.script_pubkey, Network::Bitcoin) {
            // println!(
            //     "Output {}: Address = {} (Amount: {} sats)",
            //     i, addr, output.value
            // );
            outs.push(addr);
        } else {
            println!("Output {}: Could not decode address", i);
        }
    }
    outs
}

async fn process_inputs(tx: &Transaction, is_pruned: bool) -> Vec<InputTrans> {
    // println!("\n🔹 **Detecting SOURCING (Sender) Addresses**:");
    let mut inputs = Vec::new();
    for (i, input) in tx.input.iter().enumerate() {
        let prev_txid = input.previous_output.txid.to_string();

        // println!("Input {}: Spends from previous TXID {}", i, prev_txid);

        if let Some(prev_tx) = fetch_previous_tx(&prev_txid, &is_pruned).await {
            // println!("Fetched previous transaction: {}", prev_txid);
            let temp = process_outputs(&prev_tx);
            inputs.push(InputTrans {
                txid: prev_txid,
                output_address: temp,
            });
        } else {
            // println!("❌ Failed to fetch previous transaction for {}", prev_txid);
        }
    }
    inputs
}

async fn find_address_match(tx: Transaction, is_pruned: bool) {
    let inputs = process_inputs(&tx, is_pruned).await;
    let outputs = process_outputs(&tx);
    let genesis = GenTransaction {
        txid: tx.compute_txid().to_string(),
        output_address: outputs,
        input_address: inputs,
    };
    // println!("{:?}", genesis);
    let pikachus = process_tagged_addresses_from_db();
    let genesis_outs: HashSet<_> = genesis.output_address.iter().collect();
    println!(
        "Users to Monitor on behalf::{}, TX input addrs::{}, TX output addrs::{} ",
        pikachus.len(),
        genesis_outs.len(),
        genesis.input_address.len()
    );

    pikachus.iter().for_each(|(user, user_addrs)| {
        let matching_outs: Vec<_> = user_addrs
            .iter()
            .filter(|addr| genesis_outs.contains(addr))
            .map(|f| f.to_string())
            .collect();

        if !matching_outs.is_empty() {
           let message = format!(
                "Your watch list Address {:?} has been spent in this tx {}",
                matching_outs, genesis.txid
            );
            db_operations::store_matched_address(user.clone(), (*matching_outs).to_vec(), genesis.txid.clone(),None);
            nostr_notify::send_message(message,user.to_string());
        }

        genesis.input_address.iter().for_each(|f|{
            let genesis_ins:Vec<_> =f.output_address.iter().collect();
            let matching_ins:Vec<_>=user_addrs.iter().filter(|addr| genesis_ins.contains(addr)).map(|f| f.to_string()).collect();
            if !matching_ins.is_empty(){
                let message = format!(
                    "Your watch list Address {:?} has been spent as an input for this tx {}. The input was previously funnded by this tx: {}",
                    matching_ins, genesis.txid,f.txid
                );
                db_operations::store_matched_address(user.clone(), (*matching_ins).to_vec(), genesis.txid.clone(),Some(f.txid.clone()));
            nostr_notify::send_message(message,user.to_string());
            }

        });
    });
}

async fn fetch_previous_tx(prev_txid: &str, is_pruned: &bool) -> Option<Transaction> {
    if *is_pruned {
        let url = format!("https://mempool.space/api/tx/{}/hex", prev_txid);
        // println!("{}", url);
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

/// Checks if the local bitcoind node is pruned or not
fn is_bitcoin_node_pruned() -> Result<bool,Error> {
    let output = Command::new("bitcoin-cli")
        .arg("getblockchaininfo")
        .output()?;

            if output.status.success() {
                let raw_json = String::from_utf8_lossy(&output.stdout);
                let json: Value = serde_json::from_str(&raw_json).expect("Failed to parse JSON");
                println!("✅ Bitcoin node network:: {}", json["chain"]);
                if let Some(pruned) = json["pruned"].as_bool() {
                    if pruned {
                        println!(
                            "✅ Bitcoin node is pruned. Prune height: {}",
                            json["pruneheight"]
                        );
                        return Ok(true);
                    }
                }
                Ok(false)
            } else {
                println!("{:?}", String::from_utf8_lossy(&output.stderr));
                Err(Error::new(ErrorKind::Other, String::from_utf8_lossy(&output.stderr)))
            }
    }

fn process_tagged_addresses_from_db() -> Pikachus {
    let all_addr = db_operations::get_all_tagged_addresses();
    let mut pikachus = Pikachus::default();

    if let Ok(t) = all_addr {
        for e in t {
            let entry = pikachus
                .entry(e.nostr_pubkey.clone())
                .or_insert_with(Vec::new);
            if let Ok(addr) = Address::from_str(&e.address) {
                entry.push(Address::assume_checked(addr));
            } else {
                eprintln!("❌ Failed to parse address::{}", e.address);
            }
        }
    } else {
        eprintln!("❌ Failed to fetch addresses from DB.");
    }

    pikachus
}

