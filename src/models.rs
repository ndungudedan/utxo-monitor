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
    pub record: RecordType,
}

#[derive(Debug, Insertable, Queryable, Serialize)]
#[diesel(table_name = crate::schema::matched_addresses)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct MatchedEvent {
    pub nostr_pubkey:String,
    pub txid: String,
    pub prev_txid: Option<String>,
    pub record: Vec<RecordType>,

}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum RecordType {
    Address(Address),
    Xpub(String),
    Utxo(String),
    Descriptor(String),
}

impl TryFrom<String> for RecordType {
    type Error=&'static str;
    fn try_from(s: String)-> Result<Self,Self::Error>{
        if s.starts_with("1") || s.starts_with("3") || s.starts_with("bc1") {
            Ok(RecordType::Address(Address::assume_checked(Address::from_str(&s)?)?))
        } else if s.starts_with("xpub") || s.starts_with("ypub") || s.starts_with("zpub") {
            Ok(RecordType::Xpub(s))
        } else if s.contains(':') && s.len() > 65 {
            Ok(RecordType::Utxo(s))
        } else if s.starts_with("wpkh(") || s.starts_with("sh(") || s.starts_with("multi(") {
            Ok(RecordType::Descriptor(s))
        } else {
            Err("Invalid RecordType string")
        }
    }
}

impl From<RecordType> for String{
    fn from(record: RecordType) -> Self {
        match record {
            RecordType::Address(s) => s,
            RecordType::Xpub(s) => s,
            RecordType::Utxo(s) => s,
            RecordType::Descriptor(s) => s,
        }
    }
}