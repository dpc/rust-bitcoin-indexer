use log::debug;

use super::*;
use dotenv::dotenv;
use failure::format_err;
use postgres::{transaction::Transaction, Connection, TlsMode};
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::{env, fmt::Write, str::FromStr};

pub fn establish_connection() -> Result<Connection> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;
    Ok(Connection::connect(database_url, TlsMode::None)?)
}

fn insert_blocks_query(blocks: &[Block]) -> Vec<String> {
    if blocks.is_empty() {
        return vec![];
    }
    if blocks.len() > 9000 {
        let mid = blocks.len() / 2;
        let mut p1 = insert_blocks_query(&blocks[0..mid]);
        let mut p2 = insert_blocks_query(&blocks[mid..blocks.len()]);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String = "INSERT INTO blocks (height, hash, prev_hash) VALUES".into();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "({}, '\\x{}', '\\x{}')",
            block.height, block.hash, block.prev_hash,
        ))
        .unwrap();
    }
    q.write_str(";");
    return vec![q];
}

fn insert_txs_query(txs: &[Tx]) -> Vec<String> {
    if txs.is_empty() {
        return vec![];
    }
    if txs.len() > 9000 {
        let mid = txs.len() / 2;
        let mut p1 = insert_txs_query(&txs[0..mid]);
        let mut p2 = insert_txs_query(&txs[mid..txs.len()]);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String = "INSERT INTO txs (height, hash, coinbase) VALUES".into();
    for (i, tx) in txs.iter().enumerate() {
        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "({}, '\\x{}', {})",
            tx.height, tx.hash, tx.coinbase,
        ))
        .unwrap();
    }
    q.write_str(";");
    return vec![q];
}

fn insert_outputs_query(outputs: &[Output], tx_ids: &HashMap<TxHash, i64>) -> Vec<String> {
    if outputs.is_empty() {
        return vec![];
    }
    if outputs.len() > 9000 {
        let mid = outputs.len() / 2;
        let mut p1 = insert_outputs_query(&outputs[0..mid], tx_ids);
        let mut p2 = insert_outputs_query(&outputs[mid..outputs.len()], tx_ids);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String =
        "INSERT INTO outputs (height, tx_id, tx_idx, value, address, coinbase) VALUES ".into();
    for (i, output) in outputs.iter().enumerate() {
        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "({}, {}, {}, {}, {}, {})",
            output.height,
            tx_ids[&output.tx_hash],
            output.tx_idx,
            output.value,
            output
                .address
                .as_ref()
                .map_or("null".into(), |s| format!("'{}'", s)),
            output.coinbase,
        ))
        .unwrap();
    }
    q.write_str(";");
    return vec![q];
}

fn insert_inputs_query(
    inputs: &[Input],
    outputs: &HashMap<(TxHash, u32), UtxoSetEntry>,
) -> Vec<String> {
    if inputs.is_empty() {
        return vec![];
    }
    if inputs.len() > 9000 {
        let mid = inputs.len() / 2;
        let mut p1 = insert_inputs_query(&inputs[0..mid], outputs);
        let mut p2 = insert_inputs_query(&inputs[mid..inputs.len()], outputs);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String = "INSERT INTO inputs (height, output_id) VALUES ".into();
    for (i, input) in inputs.iter().enumerate() {
        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "({}, {})",
            input.height,
            outputs[&(input.utxo_tx_hash, input.utxo_tx_idx)].id,
        ))
        .unwrap();
    }
    q.write_str(";");
    return vec![q];
}

struct UtxoSetEntry {
    id: i64,
    value: u64,
}

#[derive(Default)]
/// Cache of utxo set
/// TODO: Rename to `UtxoSetCache`
struct UtxoSet {
    entries: HashMap<(TxHash, u32), UtxoSetEntry>,
}

impl UtxoSet {
    fn insert(&mut self, tx_hash: TxHash, tx_idx: u32, id: i64, value: u64) {
        self.entries
            .insert((tx_hash, tx_idx), UtxoSetEntry { id, value });
    }

    /// Consume `outputs`
    ///
    /// Returns:
    /// * Mappings for Outputs that were found
    /// * Vector of outputs that were missing from the set
    fn consume(
        &mut self,
        outputs: impl Iterator<Item = (TxHash, u32)>,
    ) -> (HashMap<(TxHash, u32), UtxoSetEntry>, Vec<(TxHash, u32)>) {
        let mut found = HashMap::default();
        let mut missing = vec![];

        for output in outputs {
            match self.entries.remove(&output) {
                Some(details) => {
                    found.insert(output, details);
                }
                None => missing.push(output),
            }
        }

        (found, missing)
    }

    fn fetch_missing(
        conn: &Connection,
        missing: Vec<(TxHash, u32)>,
    ) -> Result<HashMap<(TxHash, u32), UtxoSetEntry>> {
        debug!("Fetching {} missing outputs", missing.len());
        let mut out = HashMap::default();

        for missing in missing {
            let tx_bytes = (missing.0.as_bytes());
            let tx_bytes = &tx_bytes[..];
            let args = &[&tx_bytes, &missing.1 as &dyn postgres::types::ToSql];
            let rows = conn.query("SELECT id, value FROM outputs INNER JOIN txs (txs.id = outputs.tx_id) WHERE txs.hash = $1 AND tx_idx = $2",
                      args)?;
            let row = rows
                .iter()
                .next()
                .ok_or_else(|| format_err!("output {}:{} not found", missing.0, missing.1))?;

            out.insert(
                (missing.0, missing.1),
                UtxoSetEntry {
                    id: row.get(0),
                    value: row.get::<_, i64>(1) as u64,
                },
            );
        }

        Ok(out)
    }
}

/// TODO: Use `select currval(pg_get_serial_sequence('txs', 'id')) as id;` instead
fn read_next_id(conn: &Connection, q: &str) -> Result<i64> {
    const PG_STARTING_ID: i64 = 1;
    Ok(conn
        .query(q, &[])?
        .iter()
        .next()
        .expect("at least one row")
        .get::<_, Option<i64>>(0)
        .map(|v| v + 1)
        .unwrap_or(PG_STARTING_ID))
}

fn read_next_tx_id(conn: &Connection) -> Result<i64> {
    read_next_id(conn, "SELECT MAX(id) FROM txs")
}

fn read_next_output_id(conn: &Connection) -> Result<i64> {
    read_next_id(conn, "SELECT MAX(id) FROM outputs")
}

fn read_next_block_id(conn: &Connection) -> Result<i64> {
    read_next_id(conn, "SELECT MAX(id) FROM blocks")
}

/// Worker Pipepline
pub struct Pipeline {
    tx: Option<crossbeam_channel::Sender<Vec<Parsed>>>,
    txs_thread: Option<std::thread::JoinHandle<Result<()>>>,
    outputs_thread: Option<std::thread::JoinHandle<Result<()>>>,
    inputs_thread: Option<std::thread::JoinHandle<Result<()>>>,
    blocks_thread: Option<std::thread::JoinHandle<Result<()>>>,
}

impl Pipeline {
    fn new() -> Result<Self> {
        let (tx, txs_rx) = crossbeam_channel::bounded::<Vec<Parsed>>(8);
        let (txs_tx, outputs_rx) =
            crossbeam_channel::bounded::<(Vec<Parsed>, HashMap<TxHash, i64>)>(8);
        let (outputs_tx, inputs_rx) = crossbeam_channel::bounded::<Vec<Parsed>>(8);
        let (inputs_tx, blocks_rx) = crossbeam_channel::bounded::<Vec<Parsed>>(8);
        let utxo_set_cache = Arc::new(Mutex::new(UtxoSet::default()));

        let txs_thread = std::thread::spawn({
            let conn = establish_connection()?;
            move || {
                let mut next_id = read_next_tx_id(&conn)?;
                while let Ok(mut parsed) = txs_rx.recv() {
                    debug_assert_eq!(next_id, read_next_tx_id(&conn)?);

                    let mut batch: Vec<super::Tx> = vec![];

                    for parsed in &mut parsed {
                        batch.append(&mut parsed.txs);
                    }

                    for s in insert_txs_query(&batch) {
                        conn.batch_execute(&s)?;
                    }

                    let batch_len = batch.len();
                    let tx_ids: HashMap<_, _> = batch
                        .into_iter()
                        .enumerate()
                        .map(|(i, tx)| (tx.hash, next_id + i as i64))
                        .collect();

                    next_id += batch_len as i64;

                    txs_tx.send((parsed, tx_ids))?;
                }
                Ok(())
            }
        });

        let outputs_thread = std::thread::spawn({
            let conn = establish_connection()?;
            let utxo_set_cache = utxo_set_cache.clone();
            move || {
                let mut next_id = read_next_output_id(&conn)?;
                while let Ok((mut parsed, tx_ids)) = outputs_rx.recv() {
                    let mut batch: Vec<super::Output> = vec![];

                    debug_assert_eq!(next_id, read_next_output_id(&conn)?);
                    for parsed in &mut parsed {
                        batch.append(&mut parsed.outputs);
                    }

                    for s in insert_outputs_query(&batch, &tx_ids) {
                        conn.batch_execute(&s)?;
                    }

                    let mut utxo_lock = utxo_set_cache.lock().unwrap();
                    batch.iter().enumerate().for_each(|(i, output)| {
                        let id = next_id + (i as i64);
                        let tx_id = tx_ids[&output.tx_hash];
                        utxo_lock.insert(output.tx_hash, output.tx_idx, tx_id, output.value);
                    });
                    drop(utxo_lock);

                    next_id += batch.len() as i64;

                    outputs_tx.send(parsed)?;
                }
                Ok(())
            }
        });

        let inputs_thread = std::thread::spawn({
            let conn = establish_connection()?;
            let utxo_set_cache = utxo_set_cache.clone();
            move || {
                while let Ok((mut parsed)) = inputs_rx.recv() {
                    let mut batch: Vec<super::Input> = vec![];

                    for parsed in &mut parsed {
                        batch.append(&mut parsed.inputs);
                    }

                    let mut utxo_lock = utxo_set_cache.lock().unwrap();
                    let (mut output_ids, missing) =
                        utxo_lock.consume(batch.iter().map(|i| (i.utxo_tx_hash, i.utxo_tx_idx)));
                    drop(utxo_lock);
                    let missing = UtxoSet::fetch_missing(&conn, missing)?;
                    for (k, v) in missing.into_iter() {
                        output_ids.insert(k, v);
                    }

                    for s in insert_inputs_query(&batch, &output_ids) {
                        conn.batch_execute(&s)?;
                    }

                    inputs_tx.send(parsed)?;
                }
                Ok(())
            }
        });

        let blocks_thread = std::thread::spawn({
            let conn = establish_connection()?;
            move || {
                while let Ok((mut parsed)) = blocks_rx.recv() {
                    let mut batch: Vec<super::Block> = vec![];

                    for parsed in parsed.into_iter() {
                        batch.push(parsed.block);
                    }

                    for s in insert_blocks_query(&batch) {
                        conn.batch_execute(&s)?;
                    }
                }
                Ok(())
            }
        });
        Ok(Self {
            tx: Some(tx),
            txs_thread: Some(txs_thread),
            outputs_thread: Some(outputs_thread),
            inputs_thread: Some(inputs_thread),
            blocks_thread: Some(blocks_thread),
        })
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        drop(self.tx.take());

        let joins = vec![
            self.txs_thread.take().unwrap(),
            self.outputs_thread.take().unwrap(),
            self.inputs_thread.take().unwrap(),
            self.blocks_thread.take().unwrap(),
        ];

        for join in joins {
            join.join().expect("Worker thread panicked");
        }
    }
}

pub struct Postresql {
    // TODO: pool
    connection: Connection,
    cached_max_height: Option<u64>,
    pipeline: Option<Pipeline>,
    batch: Vec<BlockInfo>,
    batch_txs_total: u64,
}

impl Drop for Postresql {
    fn drop(&mut self) {
        self.stop_workers();
    }
}

impl Postresql {
    pub fn new() -> Result<Self> {
        let connection = establish_connection()?;
        let mut s = Postresql {
            connection,
            pipeline: None,
            cached_max_height: None,
            batch: vec![],
            batch_txs_total: 0,
        };
        s.wipe_inconsistent_data();
        s.start_workers();
        Ok(s)
    }

    /// Wipe all the data that might have been added, before a `block` entry
    /// was commited to the DB, which is the last one to be added.
    fn wipe_inconsistent_data(&self) -> Result<()> {
        // TODO: get highest block, wipe all entries from other tables
        // with height higher than that
        //
        Ok(())
    }

    fn stop_workers(&mut self) {
        self.pipeline.take();
    }

    fn start_workers(&mut self) {
        // TODO: This `unwrap` is not OK. Connecting to db can fail.
        self.pipeline = Some(Pipeline::new().unwrap())
    }

    fn flush_workers(&mut self) {
        self.stop_workers();
        self.start_workers();
    }

    fn update_max_height(&mut self, info: &BlockInfo) {
        self.cached_max_height = Some(
            self.cached_max_height
                .map_or(info.height, |h| std::cmp::max(h, info.height)),
        );
    }

    fn flush_batch(&mut self) -> Result<()> {
        let parsed: Result<Vec<_>> = std::mem::replace(&mut self.batch, vec![])
            .par_iter()
            .map(|block_info| super::parse_node_block(&block_info))
            .collect();
        let parsed = parsed?;
        self.pipeline
            .as_ref()
            .expect("workers running")
            .tx
            .as_ref()
            .expect("tx not null")
            .send(parsed);
        self.batch_txs_total = 0;
        Ok(())
    }
}

impl DataStore for Postresql {
    // TODO: semantics against things in flight are unclear
    // Document.
    fn get_max_height(&mut self) -> Result<Option<BlockHeight>> {
        self.cached_max_height = self
            .connection
            .query("SELECT MAX(height) FROM blocks", &[])?
            .iter()
            .next()
            .and_then(|row| row.get::<_, Option<i64>>(0))
            .map(|u| u as u64);

        Ok(self.cached_max_height)
    }

    fn get_hash_by_height(&mut self, height: BlockHeight) -> Result<Option<BlockHash>> {
        if let Some(max_height) = self.cached_max_height {
            if max_height < height {
                return Ok(None);
            }
        }

        // TODO: This could be done better, if we were just tracking
        // things in flight
        eprintln!("TODO: Unnecessary flush");
        self.flush_batch();
        self.flush_workers();

        Ok(self
            .connection
            .query(
                "SELECT hash FROM blocks WHERE height = $1",
                &[&(height as i64)],
            )?
            .iter()
            .next()
            .map(|row| BlockHash::from(row.get::<_, Vec<u8>>(0).as_slice())))
    }

    fn reorg_at_height(&mut self, height: BlockHeight) -> Result<()> {
        self.flush_batch();
        self.flush_workers();

        // Always start with removing `blocks` since that invalidates
        // all other data in case of crash
        self.connection
            .execute("REMOVE FROM blocks WHERE height >= $1", &[&(height as i64)])?;
        self.connection
            .execute("REMOVE FROM txs WHERE height >= $1", &[&(height as i64)])?;
        self.connection
            .execute("REMOVE FROM inputs WHERE height >= $1", &[&(height as i64)])?;
        self.connection.execute(
            "REMOVE FROM inputs WHERE outputs >= $1",
            &[&(height as i64)],
        )?;

        self.cached_max_height = None;
        Ok(())
    }

    fn insert(&mut self, info: BlockInfo) -> Result<()> {
        self.update_max_height(&info);

        self.batch_txs_total += info.block.txdata.len() as u64;
        self.batch.push(info);
        if self.batch_txs_total > 100_000 {
            self.flush_batch();
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.flush_batch();
        Ok(())
    }
}
