DROP INDEX IF EXISTS block_extinct;

-- we need it in bulk for wipe_inconsistent_data
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'tx' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE tx ADD PRIMARY KEY (id);
  END IF;
END $$;
CREATE UNIQUE INDEX IF NOT EXISTS tx_hash ON tx (hash);
DROP INDEX IF EXISTS tx_block_id;
DROP INDEX IF EXISTS tx_coinbase;

-- we need it in bulk for wipe_inconsistent_data
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'output' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE output ADD PRIMARY KEY (id);
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS output_tx_id_tx_idx ON output (tx_id, tx_idx);
DROP INDEX IF EXISTS output_address_value;

DO $$
BEGIN
  IF EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'input' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE input DROP CONSTRAINT input_pkey;
  END IF;
END $$;
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
