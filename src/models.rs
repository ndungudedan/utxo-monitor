use bitcoin::address::Address;
use diesel::deserialize::{self, FromSql};
use diesel::prelude::*;
use diesel::serialize::{self, Output, ToSql};
use serde::Serialize;

#[derive(Debug, Insertable, Queryable, Serialize)]
#[diesel(table_name = crate::schema::gen_transactions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GenTransaction {
    pub txid: String,
    pub output_address: Vec<Address>,
    pub input_address: Vec<InputTrans>,
}

#[derive(Debug, Insertable, Queryable, Serialize)]
#[diesel(table_name = crate::schema::input_transactions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct InputTrans {
    pub txid: String,
    pub output_address: Vec<Address>,
}

#[derive(Debug, Insertable, Queryable, Serialize)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(primary_key(nostr_pubkey))]
pub struct User {
    pub nostr_pubkey: String,
}

#[derive(Debug, Insertable, Queryable, Serialize)]
#[diesel(table_name = crate::schema::user_addresses)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserAddress {
    pub nostr_pubkey: String,
    pub address: String,
}

#[derive(Debug, Insertable, Queryable, Serialize)]
#[diesel(table_name = crate::schema::matched_addresses)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct MatchedAddress {
    pub nostr_pubkey:String,
    pub txid: String,
    pub prev_txid: Option<String>,
    pub address: Vec<String>,
}
