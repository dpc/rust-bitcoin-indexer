table! {
    blocks (hash) {
        id -> Int8,
        height -> Int8,
        hash -> Bytea,
        prev_hash -> Bytea,
    }
}

table! {
    inputs (utxo_tx_hash, utxo_tx_idx) {
        id -> Int8,
        height -> Int8,
        utxo_tx_hash -> Bytea,
        utxo_tx_idx -> Int4,
    }
}

table! {
    outputs (tx_hash, tx_idx) {
        id -> Int8,
        height -> Int8,
        tx_hash -> Bytea,
        tx_idx -> Int4,
        value -> Int8,
        address -> Nullable<Text>,
        coinbase -> Bool,
    }
}

table! {
    txs (hash) {
        id -> Int8,
        height -> Int8,
        hash -> Bytea,
        coinbase -> Bool,
    }
}

allow_tables_to_appear_in_same_query!(
    blocks,
    inputs,
    outputs,
    txs,
);
