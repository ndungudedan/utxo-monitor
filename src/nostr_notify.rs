use dotenvy::dotenv;
use nostr_sdk::prelude::*;
use nostr_sdk::{Client, Keys};
use once_cell::sync::Lazy;
use std::thread;
use std::{env, str::FromStr};
use tokio::runtime::Runtime;
use tokio::sync::OnceCell;
use tokio::task;

static GLOBAL_NOSTR_CLIENT: Lazy<OnceCell<Client>> = Lazy::new(OnceCell::new);

async fn setup_relays(client: &Client) {
    let relays = vec![
        "wss://relay.damus.io",
        "wss://nos.lol",
        "wss://relay.primal.net",
        "wss://relay.0xchat.com",
        "wss://nostr.oxtr.dev",
        "wss://nostr.mom",
        "wss://relay.nostr.info",
        "wss://auth.nostr1.com",
    ];

    for relay in relays {
        match client.add_relay(relay).await {
            Ok(_) => println!("✅ Successfully added relay: {}", relay),
            Err(e) => println!("❌ Failed to add relay: {} | Error: {:?}", relay, e),
        }
    }
}

pub async fn get_nostr_client() -> &'static Client {
    GLOBAL_NOSTR_CLIENT
        .get_or_init(|| async {
            dotenv().ok();
            let nostr_secret = env::var("NOSTR_SECRET_KEY").expect("NOSTR_SECRET_KEY must be set");
            let keys = Keys::parse(&nostr_secret).expect("Invalid Nostr secret key");
            let opts = Options::new().gossip(true);
            let client = Client::builder().signer(keys.clone()).opts(opts).build();

            setup_relays(&client).await;
            client.connect().await;
            setup_dm_clients(&client).await;
            client
        })
        .await
}

async fn setup_dm_clients(client: &Client) {
    let signer = client.signer().await.unwrap();
    let public_key = signer.get_public_key().await.unwrap();

    let relays = vec![
        Url::parse("wss://auth.nostr1.com").unwrap(),
        Url::parse("wss://relay.0xchat.com").unwrap(),
    ];

    let tags: Vec<Tag> = relays
        .into_iter()
        .map(|relay| Tag::custom(TagKind::Relay, vec![relay.to_string()]))
        .collect();

    let event = EventBuilder::new(Kind::InboxRelays, "")
        .tags(tags)
        .build(public_key)
        .sign(&signer)
        .await
        .unwrap();

    let output = client.send_event(event).await;
    match output {
        Ok(t) => println!("setup_dm_clients:: {:?}", t),
        Err(e) => eprintln!("setup_dm_clients:: {:?}", e),
    }
}

pub fn send_message(message: String, pubkey: String) {
    thread::spawn(move || {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            println!("----- Attempting to send:: {}", message);
            let client = get_nostr_client().await;
            let receiver = PublicKey::from_bech32(&pubkey).expect("Unable to parse pubkey");

            let output = client.send_private_msg(receiver, message, []).await;
            match output {
                Ok(output) => {
                    println!("Event ID: {}", output.id().to_bech32().unwrap());
                    println!("Sent to: {:?}", output.success);
                    println!("Not sent to: {:?}", output.failed);
                }
                Err(e) => {
                    eprintln!("send_message Error: {}", e);
                }
            }
        });
    });
}
