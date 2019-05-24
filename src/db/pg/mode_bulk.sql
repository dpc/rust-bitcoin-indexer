-- bulk mode schema: this switches schema to bulk mode
-- mostly drops all the indices that we don't really need
-- to optimize the performance


DROP INDEX IF EXISTS block_extinct;

DO $$
BEGIN
  IF EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'tx' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE tx DROP CONSTRAINT tx_pkey;
  END IF;
END $$;

DROP INDEX IF EXISTS tx_hash_id;
DROP INDEX IF EXISTS tx_coinbase;

DO $$
BEGIN
  IF EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'block_tx' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE block_tx DROP CONSTRAINT block_tx_pkey;
  END IF;
END $$;
DROP INDEX IF EXISTS block_tx_tx_hash_id_block_hash_id;

-- in bulk mode, the utxo cache starts empty, so we need to be
-- able to fetch outputs by key
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'output' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE output ADD PRIMARY KEY (tx_hash_id, tx_idx);
  END IF;
END $$;
DROP INDEX IF EXISTS output_address_value;
DROP INDEX IF EXISTS output_value_address;

DO $$
BEGIN
  IF EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'input' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE input DROP CONSTRAINT input_pkey;
  END IF;
END $$;
DROP INDEX IF EXISTS input_tx_hash_id;

-- disable autovacum: we don't delete data anyway
ALTER TABLE event SET (
  autovacuum_enabled = false, toast.autovacuum_enabled = false
);

ALTER TABLE block SET (
  autovacuum_enabled = false, toast.autovacuum_enabled = false
);

ALTER TABLE tx SET (
  autovacuum_enabled = false, toast.autovacuum_enabled = false
);

ALTER TABLE block_tx SET (
  autovacuum_enabled = false, toast.autovacuum_enabled = false
);

ALTER TABLE output SET (
  autovacuum_enabled = false, toast.autovacuum_enabled = false
);

ALTER TABLE input SET (
  autovacuum_enabled = false, toast.autovacuum_enabled = false
);
