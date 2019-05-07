
CREATE INDEX IF NOT EXISTS block_extinct ON block (extinct);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'tx' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE tx ADD PRIMARY KEY (id);
  END IF;
END $$;
CREATE UNIQUE INDEX IF NOT EXISTS tx_hash ON tx (hash, block_id);
CREATE INDEX IF NOT EXISTS tx_block_id ON tx (block_id);
CREATE INDEX IF NOT EXISTS tx_coinbase ON tx (coinbase);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'output' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE output ADD PRIMARY KEY (id);
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS output_tx_id_tx_idx ON output (tx_id, tx_idx);
CREATE INDEX IF NOT EXISTS output_address_value ON output (address, value);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'input' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE input ADD PRIMARY KEY (output_id, tx_id);
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS input_tx_id ON input (tx_id);

ANALYZE block;
ANALYZE tx;
ANALYZE output;
ANALYZE input;

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

