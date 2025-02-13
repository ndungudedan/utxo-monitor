// @generated automatically by Diesel CLI.

diesel::table! {
    gen_transactions (txid) {
        txid -> Text,
        output_address -> Array<Nullable<Text>>,
        input_address -> Array<Nullable<Text>>,
    }
}

diesel::table! {
    input_transactions (txid) {
        txid -> Text,
        output_address -> Array<Nullable<Text>>,
    }
}

diesel::table! {
    matched_addresses (id) {
        id -> Int4,
        nostr_pubkey -> Text,
        txid -> Text,
        prev_txid -> Nullable<Text>,
        address -> Array<Nullable<Text>>,
    }
}

diesel::table! {
    user_addresses (nostr_pubkey, address) {
        nostr_pubkey -> Text,
        address -> Text,
    }
}

diesel::table! {
    users (nostr_pubkey) {
        nostr_pubkey -> Text,
    }
}

diesel::joinable!(user_addresses -> users (nostr_pubkey));

diesel::allow_tables_to_appear_in_same_query!(
    gen_transactions,
    input_transactions,
    matched_addresses,
    user_addresses,
    users,
);
