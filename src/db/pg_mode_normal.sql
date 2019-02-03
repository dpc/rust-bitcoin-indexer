CREATE INDEX IF NOT EXISTS blocks_orphaned ON blocks (orphaned);

CREATE UNIQUE INDEX IF NOT EXISTS txs_hash ON txs (hash, block_id);
CREATE INDEX IF NOT EXISTS txs_block_id ON txs (block_id);
CREATE INDEX IF NOT EXISTS txs_coinbase ON txs (coinbase);

CREATE INDEX IF NOT EXISTS outputs_address_value ON outputs (address, value);
CREATE INDEX IF NOT EXISTS outputs_tx_id_tx_idx ON outputs (tx_id, tx_idx);

CREATE INDEX IF NOT EXISTS inputs_tx_id ON inputs (tx_id);

ALTER TABLE blocks SET LOGGED;
ALTER TABLE txs SET LOGGED;
ALTER TABLE outputs SET LOGGED;
ALTER TABLE inputs SET LOGGED;
