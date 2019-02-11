DROP INDEX IF EXISTS block_orphaned;

CREATE UNIQUE INDEX IF NOT EXISTS tx_hash ON tx (hash);
DROP INDEX IF EXISTS tx_block_id;
DROP INDEX IF EXISTS tx_coinbase;

CREATE INDEX IF NOT EXISTS output_tx_id_tx_idx ON output (tx_id, tx_idx);
DROP INDEX IF EXISTS output_address_value;

DROP INDEX IF EXISTS input_output_id;

-- disable autovacum: we don't delete data anyway
ALTER TABLE block SET (
  autovacuum_enabled = false, toast.autovacuum_enabled = false
);

ALTER TABLE tx SET (
  autovacuum_enabled = false, toast.autovacuum_enabled = false
);

ALTER TABLE output SET (
  autovacuum_enabled = false, toast.autovacuum_enabled = false
);

ALTER TABLE input SET (
  autovacuum_enabled = false, toast.autovacuum_enabled = false
);

ALTER TABLE block SET UNLOGGED;
ALTER TABLE tx SET UNLOGGED;
ALTER TABLE output SET UNLOGGED;
ALTER TABLE input SET UNLOGGED;
