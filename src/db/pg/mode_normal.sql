ALTER TABLE block SET LOGGED;
ALTER TABLE tx SET LOGGED;
ALTER TABLE output SET LOGGED;
ALTER TABLE input SET LOGGED;

ANALYZE block;
ANALYZE tx;
ANALYZE output;
ANALYZE input;

CREATE INDEX IF NOT EXISTS block_orphaned ON block (orphaned);

CREATE UNIQUE INDEX IF NOT EXISTS tx_hash ON tx (hash, block_id);
CREATE INDEX IF NOT EXISTS tx_block_id ON tx (block_id);
CREATE INDEX IF NOT EXISTS tx_coinbase ON tx (coinbase);

CREATE INDEX IF NOT EXISTS output_tx_id_tx_idx ON output (tx_id, tx_idx);
CREATE INDEX IF NOT EXISTS output_address_value ON output (address, value);

CREATE INDEX IF NOT EXISTS input_tx_id ON input (tx_id);


-- enableautovacum: we don't delete data, but it might be useful anyway
ALTER TABLE block SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE tx SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE output SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE input SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

