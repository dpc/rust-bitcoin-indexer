CREATE UNIQUE INDEX IF NOT EXISTS txs_hash ON txs (hash);
CREATE INDEX IF NOT EXISTS txs_height ON txs (height);
CREATE INDEX IF NOT EXISTS outputs_height ON outputs (height);
CREATE INDEX IF NOT EXISTS outputs_address_value ON outputs (address, value);
CREATE INDEX IF NOT EXISTS outputs_tx_id_tx_idx ON outputs (tx_id, tx_idx);
CREATE INDEX IF NOT EXISTS inputs_height ON inputs (height);
