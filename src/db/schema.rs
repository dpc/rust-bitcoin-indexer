table! {
    blocks (hash) {
        id -> Int8,
        height -> Int8,
        hash -> Bytea,
        prev_hash -> Bytea,
    }
}

table! {
    inputs (output_id) {
        output_id -> Int8,
        height -> Int8,
    }
}

table! {
    outputs (tx_id, tx_idx) {
        id -> Int8,
        height -> Int8,
        tx_id -> Int8,
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
