CREATE INDEX IF NOT EXISTS outputs_tx_id_tx_idx ON outputs (tx_id, tx_idx);
CREATE UNIQUE INDEX IF NOT EXISTS txs_hash ON txs (hash);


DROP INDEX IF EXISTS txs_height;
DROP INDEX IF EXISTS outputs_height;
DROP INDEX IF EXISTS outputs_address_value;
DROP INDEX IF EXISTS inputs_height;

ALTER TABLE blocks SET UNLOGGED;
ALTER TABLE txs SET UNLOGGED;
ALTER TABLE outputs SET UNLOGGED;
ALTER TABLE inputs SET UNLOGGED;
