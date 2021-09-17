DROP TABLE IF EXISTS input CASCADE;
DROP TABLE IF EXISTS output CASCADE;
DROP TABLE IF EXISTS tx CASCADE;
DROP TABLE IF EXISTS block_tx CASCADE;
DROP TABLE IF EXISTS block CASCADE;
DROP TABLE IF EXISTS event CASCADE;
-- but not this one!
-- DROP TABLE IF EXISTS indexer_state CASCADE;

DROP FUNCTION IF EXISTS reverse_bytes_iter(bytes bytea, length int, midpoint int, index int) CASCADE;
DROP FUNCTION IF EXISTS hash_from_parts(hash_id bytea, hash_rest bytea) CASCADE;
DROP FUNCTION IF EXISTS hash_to_hash_id(hash bytea) CASCADE;
