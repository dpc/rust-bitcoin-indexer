DROP INDEX IF EXISTS blocks_orphaned;

DROP INDEX IF EXISTS txs_hash;
DROP INDEX IF EXISTS txs_block_id;
DROP INDEX IF EXISTS txs_coinbase;

DROP INDEX IF EXISTS outputs_address_value;
DROP INDEX IF EXISTS outputs_tx_id_tx_idx;

DROP INDEX IF EXISTS inputs_tx_id;
