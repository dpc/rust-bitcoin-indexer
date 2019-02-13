DROP INDEX IF EXISTS block_orphaned;

DROP INDEX IF EXISTS tx_hash;
DROP INDEX IF EXISTS tx_block_id;
DROP INDEX IF EXISTS tx_coinbase;

DROP INDEX IF EXISTS output_address_value;
DROP INDEX IF EXISTS output_tx_id_tx_idx;

DO $$
BEGIN
  IF EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'input' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE input DROP CONSTRAINT input_pkey;
  END IF;
END $$;
DROP INDEX IF EXISTS input_tx_id;
