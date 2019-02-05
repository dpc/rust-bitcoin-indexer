ALTER TABLE blocks SET LOGGED;
ALTER TABLE txs SET LOGGED;
ALTER TABLE outputs SET LOGGED;
ALTER TABLE inputs SET LOGGED;

ANALYZE blocks;
ANALYZE txs;
ANALYZE outputs;
ANALYZE inputs;

CREATE INDEX IF NOT EXISTS blocks_orphaned ON blocks (orphaned);

CREATE UNIQUE INDEX IF NOT EXISTS txs_hash ON txs (hash, block_id);
CREATE INDEX IF NOT EXISTS txs_block_id ON txs (block_id);
CREATE INDEX IF NOT EXISTS txs_coinbase ON txs (coinbase);

CREATE INDEX IF NOT EXISTS outputs_tx_id_tx_idx ON outputs (tx_id, tx_idx);
CREATE INDEX IF NOT EXISTS outputs_address_value ON outputs (address, value);

CREATE INDEX IF NOT EXISTS inputs_tx_id ON inputs (tx_id);


-- enableautovacum: we don't delete data, but it might be useful anyway
ALTER TABLE blocks SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE txs SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE outputs SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE inputs SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

