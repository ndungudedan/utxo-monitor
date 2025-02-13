-- Your SQL goes here
-- Enable PostgreSQL extensions for array and JSON support
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create `gen_transactions` table
CREATE TABLE gen_transactions (
    txid TEXT PRIMARY KEY,
    output_address TEXT[] NOT NULL,  -- Store addresses as an array
    input_address TEXT[] NOT NULL     -- Store input transactions as JSON
);

-- Create `input_transactions` table
CREATE TABLE input_transactions (
    txid TEXT PRIMARY KEY,
    output_address TEXT[] NOT NULL  -- Store addresses as an array
);

CREATE TABLE users (
    nostr_pubkey TEXT UNIQUE PRIMARY KEY NOT NULL
);

CREATE TABLE user_addresses (
    nostr_pubkey TEXT NOT NULL,
    address TEXT NOT NULL,
    PRIMARY KEY (nostr_pubkey, address),
    FOREIGN KEY (nostr_pubkey) REFERENCES users(nostr_pubkey) ON DELETE CASCADE
);


CREATE TABLE matched_addresses (
    id SERIAL PRIMARY KEY,
    nostr_pubkey TEXT NOT NULL,
    txid TEXT NOT NULL,
    prev_txid TEXT,
    address TEXT[] NOT NULL
);
