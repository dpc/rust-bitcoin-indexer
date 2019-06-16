-- bulk mode schema: this switches schema to bulk mode
-- it must remove almost all thing that `mode_normal.sql`
-- created: they would hurt insert performance


--- event
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_event_block_hash_id') THEN
    ALTER TABLE event
    DROP CONSTRAINT fk_event_block_hash_id;
  END IF;
END;
$$;

--- block
/* nothing atm */

--- block_tx
DO $$
BEGIN
  IF EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'block_tx' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE block_tx DROP CONSTRAINT block_tx_pkey CASCADE;
  END IF;
END $$;
DROP INDEX IF EXISTS block_tx_tx_hash_id_block_hash_id;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_block_tx_block_hash_id') THEN
    ALTER TABLE block_tx
    DROP CONSTRAINT fk_block_tx_block_hash_id;
  END IF;
END;
$$;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_block_tx_tx_hash_id') THEN
    ALTER TABLE block_tx
    DROP CONSTRAINT fk_block_tx_tx_hash_id;
  END IF;
END;
$$;

--- tx
DO $$
BEGIN
  IF EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'tx' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE tx DROP CONSTRAINT tx_pkey CASCADE;
  END IF;
END $$;

DROP INDEX IF EXISTS tx_coinbase_eq_true;
DROP INDEX IF EXISTS tx_current_height;

--- output

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
DROP INDEX IF EXISTS output_address;
DROP INDEX IF EXISTS output_value;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_output_tx_hash_id') THEN
    ALTER TABLE output
    DROP CONSTRAINT fk_output_tx_hash_id;
  END IF;
END;
$$;

--- input
DO $$
BEGIN
  IF EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'input' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE input DROP CONSTRAINT input_pkey CASCADE;
  END IF;
END $$;
DROP INDEX IF EXISTS input_tx_hash_id;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_input_tx_hash_id') THEN
    ALTER TABLE input
    DROP CONSTRAINT fk_input_tx_hash_id;
  END IF;
END;
$$;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_input_output') THEN
    ALTER TABLE input
    DROP CONSTRAINT fk_input_output;
  END IF;
END;
$$;

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
