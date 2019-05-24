-- normal mode schema: after reaching chainhead/first reorg
-- we build all indices, etc. to enable all the queries etc.
CREATE INDEX IF NOT EXISTS block_extinct ON block (extinct);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'tx' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE tx ADD PRIMARY KEY (hash_id);
  END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS tx_hash_id ON tx (hash_id);
CREATE INDEX IF NOT EXISTS tx_coinbase ON tx (coinbase);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'block_tx' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE block_tx ADD PRIMARY KEY (block_hash_id, tx_hash_id);
  END IF;
END $$;
CREATE UNIQUE INDEX IF NOT EXISTS block_tx_tx_hash_id_block_hash_id ON block_tx (tx_hash_id, block_hash_id);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'output' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE output ADD PRIMARY KEY (tx_hash_id, tx_idx);
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS output_address_value ON output (address, value);
CREATE INDEX IF NOT EXISTS output_value_address ON output (value, address);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT constraint_name FROM information_schema.table_constraints WHERE table_name = 'input' AND constraint_type = 'PRIMARY KEY'
  ) THEN
    ALTER TABLE input ADD PRIMARY KEY (output_tx_hash_id, output_tx_idx);
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS input_tx_hash_id ON input (tx_hash_id);

ANALYZE block;
ANALYZE tx;
ANALYZE block_tx;
ANALYZE output;
ANALYZE input;

-- enableautovacum: it might be useful anyway
ALTER TABLE event SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE block SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE tx SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE block_tx SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE output SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE input SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);

ALTER TABLE input SET (
  autovacuum_enabled = true, toast.autovacuum_enabled = true
);
