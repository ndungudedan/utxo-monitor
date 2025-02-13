use diesel::debug_query;
use diesel::pg::Pg;
use diesel::{
    query_dsl::methods::{FilterDsl, SelectDsl},
    ExpressionMethods, PgConnection, RunQueryDsl,
};

use crate::{
    db,
    models::{MatchedAddress, User, UserAddress},
    schema::{matched_addresses, user_addresses, users},
};

pub fn create_new_user(nostr_pubkey: String) -> Result<User, diesel::result::Error> {
    let new_user = User {
        nostr_pubkey: nostr_pubkey,
    };
    let mut conn = db::get_connection();
    diesel::insert_into(users::table)
        .values(&new_user)
        .execute(&mut *conn)?;
    Ok(new_user)
}

pub fn store_user_address(nostr_pubkey: String, address: String) {
    let new_addr = UserAddress {
        nostr_pubkey,
        address,
    };
    let mut conn = db::get_connection();

    diesel::insert_into(user_addresses::table)
        .values(new_addr)
        .execute(&mut *conn);
}

pub fn store_matched_address(
    nostr_pubkey: String,
    address: Vec<String>,
    txid: String,
    prev_txid: Option<String>,
) {
    let new_match = MatchedAddress {
        nostr_pubkey,
        txid,
        prev_txid,
        address,
    };
    let mut conn = db::get_connection();
    diesel::insert_into(matched_addresses::table)
        .values(new_match)
        .execute(&mut *conn);
}

pub fn get_tagged_addresses(user: String) -> Result<Vec<UserAddress>, diesel::result::Error> {
    use self::user_addresses::dsl::*;

    let mut conn = db::get_connection();
    let query = user_addresses.filter(nostr_pubkey.eq(user.to_lowercase()));
    let addrs = query.load::<UserAddress>(&mut conn)?;
    Ok(addrs)
}

pub fn get_all_tagged_addresses() -> Result<Vec<UserAddress>, diesel::result::Error> {
    use self::user_addresses::dsl::*;

    let mut conn = db::get_connection();
    let addrs = user_addresses.load::<UserAddress>(&mut conn)?;

    println!("get_all_tagged_addresses::: {:?}", addrs.len());
    Ok(addrs)
}
