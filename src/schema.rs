table! {
    blocks (id) {
        id -> Int8,
        height -> Int8,
        hash -> Text,
        prev_hash -> Text,
    }
}

table! {
    inputs (id) {
        id -> Int8,
        height -> Int8,
        utxo_tx_hash -> Int8,
        utxo_tx_idx -> Int2,
    }
}

table! {
    outputs (id) {
        id -> Int8,
        height -> Int8,
        tx_hash -> Text,
        tx_idx -> Int2,
        value -> Int8,
        address -> Nullable<Text>,
    }
}

table! {
    txs (id) {
        id -> Int8,
        height -> Int8,
        hash -> Text,
    }
}

allow_tables_to_appear_in_same_query!(
    blocks,
    inputs,
    outputs,
    txs,
);
