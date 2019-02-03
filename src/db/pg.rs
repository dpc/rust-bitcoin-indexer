use log::{debug, error, info, trace};

use super::*;
use crate::prelude::*;
use dotenv::dotenv;
use postgres::{Connection, TlsMode};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::{env, fmt::Write};

pub fn establish_connection() -> Result<Connection> {
    dotenv()?;

    let database_url = env::var("DATABASE_URL")?;
    Ok(Connection::connect(database_url, TlsMode::None)?)
}

fn create_bulk_insert_blocks_query(blocks: &[Block]) -> Vec<String> {
    if blocks.is_empty() {
        return vec![];
    }
    if blocks.len() > 9000 {
        let mid = blocks.len() / 2;
        let mut p1 = create_bulk_insert_blocks_query(&blocks[0..mid]);
        let mut p2 = create_bulk_insert_blocks_query(&blocks[mid..blocks.len()]);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String =
        "INSERT INTO blocks (height, hash, prev_hash, merkle_root, time) VALUES".into();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "({},'\\x{}','\\x{}','\\x{}', {})",
            block.height, block.hash, block.prev_hash, block.merkle_root, block.time
        ))
        .unwrap();
    }
    q.write_str(";").expect("Write to string can't fail");
    return vec![q];
}

fn create_bulk_insert_txs_query(txs: &[Tx], block_ids: &HashMap<BlockHash, i64>) -> Vec<String> {
    if txs.is_empty() {
        return vec![];
    }
    if txs.len() > 9000 {
        let mid = txs.len() / 2;
        let mut p1 = create_bulk_insert_txs_query(&txs[0..mid], block_ids);
        let mut p2 = create_bulk_insert_txs_query(&txs[mid..txs.len()], block_ids);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String = "INSERT INTO txs (block_id, hash, coinbase) VALUES".into();
    for (i, tx) in txs.iter().enumerate() {
        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "({},'\\x{}',{})",
            block_ids[&tx.block_hash], tx.hash, tx.coinbase,
        ))
        .unwrap();
    }
    q.write_str(";").expect("Write to string can't fail");
    return vec![q];
}

fn create_bulk_insert_outputs_query(
    outputs: &[Output],
    tx_ids: &HashMap<TxHash, i64>,
) -> Vec<String> {
    if outputs.is_empty() {
        return vec![];
    }
    if outputs.len() > 9000 {
        let mid = outputs.len() / 2;
        let mut p1 = create_bulk_insert_outputs_query(&outputs[0..mid], tx_ids);
        let mut p2 = create_bulk_insert_outputs_query(&outputs[mid..outputs.len()], tx_ids);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String =
        "INSERT INTO outputs (tx_id, tx_idx, value, address, coinbase) VALUES ".into();
    for (i, output) in outputs.iter().enumerate() {
        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "({},{},{},{},{})",
            tx_ids[&output.out_point.txid],
            output.out_point.vout,
            output.value,
            output
                .address
                .as_ref()
                .map_or("null".into(), |s| format!("'{}'", s)),
            output.coinbase,
        ))
        .unwrap();
    }
    q.write_str(";").expect("Write to string can't fail");
    return vec![q];
}

fn create_bulk_insert_inputs_query(
    inputs: &[Input],
    outputs: &HashMap<OutPoint, UtxoSetEntry>,
    tx_ids: &HashMap<TxHash, i64>,
) -> Vec<String> {
    if inputs.is_empty() {
        return vec![];
    }
    if inputs.len() > 9000 {
        let mid = inputs.len() / 2;
        let mut p1 = create_bulk_insert_inputs_query(&inputs[0..mid], outputs, tx_ids);
        let mut p2 = create_bulk_insert_inputs_query(&inputs[mid..inputs.len()], outputs, tx_ids);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String = "INSERT INTO inputs (output_id, tx_id) VALUES ".into();
    for (i, input) in inputs.iter().enumerate() {
        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "({},{})",
            outputs[&input.out_point].id, tx_ids[&input.tx_id]
        ))
        .unwrap();
    }
    q.write_str(";").expect("Write to string can't fail");
    vec![q]
}

fn crate_fetch_outputs_query(outputs: &[OutPoint]) -> Vec<String> {
    if outputs.len() > 1500 {
        let mid = outputs.len() / 2;
        let mut p1 = crate_fetch_outputs_query(&outputs[0..mid]);
        let mut p2 = crate_fetch_outputs_query(&outputs[mid..outputs.len()]);
        p1.append(&mut p2);
        return p1;
    }
    let mut q: String = "SELECT outputs.id, outputs.value, txs.hash, outputs.tx_idx FROM outputs JOIN txs ON (txs.id = outputs.tx_id) JOIN blocks ON txs.block_id = blocks.id WHERE blocks.orphaned = false AND (txs.hash, outputs.tx_idx) IN ( VALUES ".into();
    for (i, output) in outputs.iter().enumerate() {
        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "('\\x{}'::bytea,{})",
            output.txid, output.vout
        ))
        .unwrap();
    }
    q.write_str(" );").expect("Write to string can't fail");
    vec![q]
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct UtxoSetEntry {
    id: i64,
    value: u64,
}

#[derive(Default)]
/// Cache of utxo set
struct UtxoSetCache {
    entries: HashMap<OutPoint, UtxoSetEntry>,
}

impl UtxoSetCache {
    fn insert(&mut self, point: OutPoint, id: i64, value: u64) {
        self.entries.insert(point, UtxoSetEntry { id, value });
    }

    /// Consume `outputs`
    ///
    /// Returns:
    /// * Mappings for Outputs that were found
    /// * Vector of outputs that were missing from the set
    fn consume(
        &mut self,
        outputs: impl Iterator<Item = OutPoint>,
    ) -> (HashMap<OutPoint, UtxoSetEntry>, Vec<OutPoint>) {
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
        missing: Vec<OutPoint>,
    ) -> Result<HashMap<OutPoint, UtxoSetEntry>> {
        let missing_len = missing.len();
        debug!("Fetching {} missing outputs", missing_len);
        let mut out = HashMap::default();

        if missing.is_empty() {
            return Ok(HashMap::default());
        }

        let start = Instant::now();
        let missing: Vec<_> = missing.into_iter().collect();
        for q in crate_fetch_outputs_query(&missing) {
            for row in &conn.query(&q, &[])? {
                let tx_hash = {
                    let mut human_bytes = row.get::<_, Vec<u8>>(2);
                    human_bytes.reverse();
                    BlockHash::from(human_bytes.as_slice())
                };
                out.insert(
                    OutPoint {
                        txid: tx_hash,
                        vout: row.get::<_, i32>(3) as u32,
                    },
                    UtxoSetEntry {
                        id: row.get(0),
                        value: row.get::<_, i64>(1) as u64,
                    },
                );
            }
        }

        trace!(
            "Fetched {} missing outputs in {}s",
            missing_len,
            Instant::now().duration_since(start).as_secs()
        );
        Ok(out)
    }
}

fn read_next_id(conn: &Connection, table_name: &str, id_col_name: &str) -> Result<i64> {
    // explanation: https://dba.stackexchange.com/a/78228
    let q = format!(
        "select setval(pg_get_serial_sequence('{table}', '{id_col}'), GREATEST(nextval(pg_get_serial_sequence('{table}', '{id_col}')) - 1, 1)) as id",
        table = table_name,
        id_col = id_col_name
    );
    const PG_STARTING_ID: i64 = 1;
    Ok(conn
        .query(&q, &[])?
        .iter()
        .next()
        .expect("at least one row")
        .get::<_, Option<i64>>(0)
        .map(|v| v + 1)
        .unwrap_or(PG_STARTING_ID))
}

fn execute_bulk_insert_transcation(
    conn: &Connection,
    name: &str,
    len: usize,
    batch_id: u64,
    queries: impl Iterator<Item = String>,
) -> Result<()> {
    trace!("Inserting {} {} from batch {}...", len, name, batch_id);
    let start = Instant::now();
    let transaction = conn.transaction()?;
    for s in queries {
        transaction.batch_execute(&s)?;
    }
    transaction.commit()?;
    trace!(
        "Inserted {} {} from batch {} in {}s",
        len,
        name,
        batch_id,
        Instant::now().duration_since(start).as_secs()
    );
    Ok(())
}

fn read_next_tx_id(conn: &Connection) -> Result<i64> {
    read_next_id(conn, "txs", "id")
}

fn read_next_output_id(conn: &Connection) -> Result<i64> {
    read_next_id(conn, "outputs", "id")
}

fn read_next_block_id(conn: &Connection) -> Result<i64> {
    read_next_id(conn, "blocks", "id")
}

type BlocksInFlight = HashSet<BlockHash>;

/// Worker Pipepline
///
/// `Pipeline` is reponsible for actually inserting data into the db.
///  It is split between multiple threads - each handling one table.
///  The idea here is to have some level of paralelism to help saturate
///  network IO, and then disk IO. As each thread touches only one
///  table - there is no contention between them.
///
///  `Pipeline` name comes from the fact that each thread does its job
///  and passes rest of the data to the next one.
///
///  A lot of stuff here about performance is actually speculative,
///  but there is only so many hours in a day, and it seems to work well
///  in practice.
///
///  In as `atomic` mode, last thread inserts entire data in one transaction
///  to prevent temporary inconsistency (eg. txs inserted, but blocks not yet).
///  It is to be used in non-bulk mode, when blocks are indexed one at the time,
///  so performance is not important. Passing formatted queries around is a compromise
///  between having two different versions of this logic, and good performance
///  in bulk mode.
struct Pipeline {
    tx: Option<crossbeam_channel::Sender<(u64, Vec<Parsed>)>>,
    txs_thread: Option<std::thread::JoinHandle<Result<()>>>,
    outputs_thread: Option<std::thread::JoinHandle<Result<()>>>,
    inputs_thread: Option<std::thread::JoinHandle<Result<()>>>,
    blocks_thread: Option<std::thread::JoinHandle<Result<()>>>,
}

// TODO: fail the whole Pipeline somehow
fn fn_log_err<F>(name: &'static str, mut f: F) -> impl FnMut() -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    move || {
        let res = f();
        if let Err(ref e) = res {
            error!("{} finished with an error: {}", name, e);
        }

        res
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum PipelineMode {
    Atomic,
    Bulk,
}

impl PipelineMode {
    fn is_atomic(self) -> bool {
        self == PipelineMode::Atomic
    }
}

impl Pipeline {
    fn new(in_flight: Arc<Mutex<BlocksInFlight>>, mode: PipelineMode) -> Result<Self> {
        // We use only rendezvous (0-size) channels, to allow passing
        // work and parallelism, but without doing any buffering of
        // work in the channels. Buffered work does not
        // improve performance, and  more things in flight means
        // incrased memory usage.
        let (tx, blocks_rx) = crossbeam_channel::bounded::<(u64, Vec<Parsed>)>(0);

        let (blocks_tx, txs_rx) = crossbeam_channel::bounded::<(
            u64,
            HashMap<BlockHash, i64>,
            Vec<Tx>,
            Vec<Output>,
            Vec<Input>,
            u64,
            Vec<Vec<String>>,
        )>(0);

        let (txs_tx, outputs_rx) = crossbeam_channel::bounded::<(
            u64,
            HashMap<BlockHash, i64>,
            HashMap<TxHash, i64>,
            Vec<Output>,
            Vec<Input>,
            u64,
            Vec<Vec<String>>,
        )>(0);

        let (outputs_tx, inputs_rx) = crossbeam_channel::bounded::<(
            u64,
            HashMap<BlockHash, i64>,
            HashMap<TxHash, i64>,
            Vec<Input>,
            u64,
            Vec<Vec<String>>,
        )>(0);

        let utxo_set_cache = Arc::new(Mutex::new(UtxoSetCache::default()));

        let blocks_thread = std::thread::spawn({
            let conn = establish_connection()?;
            fn_log_err("db_worker_blocks", move || {
                let mut next_id = read_next_block_id(&conn)?;
                while let Ok((batch_id, parsed)) = blocks_rx.recv() {
                    let mut blocks: Vec<super::Block> = vec![];
                    let mut txs: Vec<super::Tx> = vec![];
                    let mut outputs: Vec<super::Output> = vec![];
                    let mut inputs: Vec<super::Input> = vec![];
                    let mut pending_queries = vec![];

                    for mut parsed in parsed {
                        blocks.push(parsed.block);
                        txs.append(&mut parsed.txs);
                        outputs.append(&mut parsed.outputs);
                        inputs.append(&mut parsed.inputs);
                    }

                    let max_block_height = blocks
                        .iter()
                        .rev()
                        .next()
                        .map(|b| b.height)
                        .expect("at least one block");

                    let min_block_height = blocks
                        .iter()
                        .next()
                        .map(|b| b.height)
                        .expect("at least one block");

                    let blocks_len = blocks.len();

                    let insert_queries = create_bulk_insert_blocks_query(&blocks);
                    let reorg_queries = vec![format!(
                        "UPDATE blocks SET orphaned = true WHERE height >= {};",
                        min_block_height
                    )];

                    if mode.is_atomic() {
                        pending_queries.push(reorg_queries);
                        pending_queries.push(insert_queries);
                    } else {
                        assert_eq!(next_id, read_next_block_id(&conn)?);
                        execute_bulk_insert_transcation(
                            &conn,
                            "blocks",
                            blocks.len(),
                            batch_id,
                            vec![reorg_queries, insert_queries].into_iter().flatten(),
                        )?;
                    }

                    let block_ids: HashMap<_, _> = blocks
                        .into_iter()
                        .enumerate()
                        .map(|(i, tx)| (tx.hash, next_id + i as i64))
                        .collect();

                    next_id += blocks_len as i64;

                    blocks_tx.send((
                        batch_id,
                        block_ids,
                        txs,
                        outputs,
                        inputs,
                        max_block_height,
                        pending_queries,
                    ))?;
                }
                Ok(())
            })
        });
        let txs_thread = std::thread::spawn({
            let conn = establish_connection()?;
            fn_log_err("db_worker_txs", move || {
                let mut next_id = read_next_tx_id(&conn)?;
                while let Ok((
                    batch_id,
                    block_ids,
                    txs,
                    outputs,
                    inputs,
                    max_block_height,
                    mut pending_queries,
                )) = txs_rx.recv()
                {
                    let queries = create_bulk_insert_txs_query(&txs, &block_ids);

                    if mode.is_atomic() {
                        pending_queries.push(queries);
                    } else {
                        assert_eq!(next_id, read_next_tx_id(&conn)?);
                        execute_bulk_insert_transcation(
                            &conn,
                            "txs",
                            txs.len(),
                            batch_id,
                            queries.into_iter(),
                        )?
                    };

                    let batch_len = txs.len();
                    let tx_ids: HashMap<_, _> = txs
                        .into_iter()
                        .enumerate()
                        .map(|(i, tx)| (tx.hash, next_id + i as i64))
                        .collect();

                    next_id += batch_len as i64;

                    txs_tx.send((
                        batch_id,
                        block_ids,
                        tx_ids,
                        outputs,
                        inputs,
                        max_block_height,
                        pending_queries,
                    ))?;
                }
                Ok(())
            })
        });

        let outputs_thread = std::thread::spawn({
            let conn = establish_connection()?;
            let utxo_set_cache = utxo_set_cache.clone();
            fn_log_err("db_worker_outputs", move || {
                let mut next_id = read_next_output_id(&conn)?;
                while let Ok((
                    batch_id,
                    block_ids,
                    tx_ids,
                    outputs,
                    inputs,
                    max_block_height,
                    mut pending_queries,
                )) = outputs_rx.recv()
                {
                    let queries = create_bulk_insert_outputs_query(&outputs, &tx_ids);

                    if mode.is_atomic() {
                        pending_queries.push(queries);
                    } else {
                        assert_eq!(next_id, read_next_output_id(&conn)?);
                        execute_bulk_insert_transcation(
                            &conn,
                            "outputs",
                            outputs.len(),
                            batch_id,
                            queries.into_iter(),
                        )?;
                    }

                    let mut utxo_lock = utxo_set_cache.lock().unwrap();
                    outputs.iter().enumerate().for_each(|(i, output)| {
                        let id = next_id + (i as i64);
                        utxo_lock.insert(output.out_point, id, output.value);
                    });
                    drop(utxo_lock);

                    next_id += outputs.len() as i64;

                    outputs_tx.send((
                        batch_id,
                        block_ids,
                        tx_ids,
                        inputs,
                        max_block_height,
                        pending_queries,
                    ))?;
                }
                Ok(())
            })
        });

        let inputs_thread = std::thread::spawn({
            let conn = establish_connection()?;
            let utxo_set_cache = utxo_set_cache.clone();
            fn_log_err("db_worker_inputs", move || {
                while let Ok((
                    batch_id,
                    block_ids,
                    tx_ids,
                    inputs,
                    max_block_height,
                    mut pending_queries,
                )) = inputs_rx.recv()
                {
                    let mut utxo_lock = utxo_set_cache.lock().unwrap();
                    let (mut output_ids, missing) =
                        utxo_lock.consume(inputs.iter().map(|i| i.out_point));
                    drop(utxo_lock);
                    let missing = UtxoSetCache::fetch_missing(&conn, missing)?;
                    for (k, v) in missing.into_iter() {
                        output_ids.insert(k, v);
                    }

                    let mut queries =
                        create_bulk_insert_inputs_query(&inputs, &output_ids, &tx_ids);

                    queries.push(format!(
                        "UPDATE indexer_state SET height = {};",
                        max_block_height
                    ));

                    if mode.is_atomic() {
                        pending_queries.push(queries);

                        execute_bulk_insert_transcation(
                            &conn,
                            "all block data",
                            block_ids.len(),
                            batch_id,
                            pending_queries.into_iter().flatten(),
                        )?;
                    } else {
                        execute_bulk_insert_transcation(
                            &conn,
                            "inputs",
                            inputs.len(),
                            batch_id,
                            queries.into_iter(),
                        )?;
                    }

                    info!("Block {}H fully indexed and commited", max_block_height);

                    let mut any_missing = false;
                    let mut lock = in_flight.lock().unwrap();
                    for hash in block_ids.keys() {
                        let missing = !lock.remove(&hash);
                        any_missing = any_missing || missing;
                    }
                    drop(lock);
                    assert!(!any_missing);
                }
                Ok(())
            })
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
            join.join()
                .expect("Couldn't join on thread")
                .expect("Worker thread panicked");
        }
    }
}

pub struct Postresql {
    connection: Connection,
    cached_max_height: Option<u64>,
    pipeline: Option<Pipeline>,
    batch: Vec<BlockInfo>,
    batch_txs_total: u64,
    batch_id: u64,
    bulk_mode: bool,

    in_flight: Arc<Mutex<BlocksInFlight>>,
}

impl Drop for Postresql {
    fn drop(&mut self) {
        self.stop_workers();
    }
}

impl Postresql {
    pub fn new() -> Result<Self> {
        let connection = establish_connection()?;
        Self::init(&connection)?;
        let (height, bulk_mode) = Self::read_indexer_state(&connection)?;
        let mut s = Postresql {
            connection,
            pipeline: None,
            cached_max_height: None,
            batch: vec![],
            batch_txs_total: 0,
            batch_id: 0,
            bulk_mode,
            in_flight: Arc::new(Mutex::new(BlocksInFlight::new())),
        };
        s.wipe_inconsistent_data(height)?;
        s.start_workers();
        Ok(s)
    }

    fn read_indexer_state(conn: &Connection) -> Result<(Option<u64>, bool)> {
        let state = conn.query("SELECT bulk_mode, height FROM indexer_state", &[])?;
        if let Some(state) = state.iter().next() {
            Ok((
                state.get::<_, Option<i64>>(1).map(|h| h as u64),
                state.get(0),
            ))
        } else {
            conn.execute(
                "INSERT INTO indexer_state (bulk_mode, height) VALUES (true, NULL)",
                &[],
            )?;
            Ok((None, true))
        }
    }

    fn init(conn: &Connection) -> Result<()> {
        info!("Creating db schema");
        conn.batch_execute(include_str!("pg_init_base.sql"))?;
        Ok(())
    }

    /// Wipe all the data that might have been added, before a `block` entry
    /// was commited to the DB.
    ///
    /// `Blocks` is  the last table to have data inserted, and is
    /// used as a commitment that everything else was inserted already..
    fn wipe_inconsistent_data(&mut self, height: Option<BlockHeight>) -> Result<()> {
        // there could be no inconsistent date outside of bulk mode
        if self.bulk_mode {
            if let Some(height) = height {
                info!("Deleting potentially inconsistent data from previous bulk run");
                self.wipe_to_height(height)?;
            }
        }

        Ok(())
    }

    fn stop_workers(&mut self) {
        debug!("Stopping DB pipeline workers");
        self.pipeline.take();
        debug!("Stopped DB pipeline workers");
        assert!(self.in_flight.lock().unwrap().is_empty());
    }

    fn start_workers(&mut self) {
        debug!("Starting DB pipeline workers");
        // TODO: This `unwrap` is not OK. Connecting to db can fail.
        self.pipeline = Some(
            Pipeline::new(
                self.in_flight.clone(),
                if self.bulk_mode {
                    PipelineMode::Bulk
                } else {
                    PipelineMode::Atomic
                },
            )
            .unwrap(),
        )
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
        if self.batch.is_empty() {
            return Ok(());
        }
        trace!(
            "Flushing batch {}, with {} txes",
            self.batch_id,
            self.batch_txs_total
        );
        let parsed: Result<Vec<_>> = std::mem::replace(&mut self.batch, vec![])
            .par_iter()
            .map(|block_info| super::parse_node_block(&block_info))
            .collect();
        let parsed = parsed?;

        let mut in_flight = self.in_flight.lock().expect("locking works");
        for parsed in &parsed {
            in_flight.insert(parsed.block.hash);
        }
        drop(in_flight);

        self.pipeline
            .as_ref()
            .expect("workers running")
            .tx
            .as_ref()
            .expect("tx not null")
            .send((self.batch_id, parsed))
            .expect("Send should not fail");
        trace!("Batch flushed");
        self.batch_txs_total = 0;
        self.batch_id += 1;
        Ok(())
    }
}

impl DataStore for Postresql {
    fn wipe(&mut self) -> Result<()> {
        info!("Wiping db schema");
        self.connection.batch_execute(include_str!("pg_wipe.sql"))?;
        Ok(())
    }

    fn mode_bulk(&mut self) -> Result<()> {
        info!("Entering bulk mode: minimum indices");
        if !self.bulk_mode {
            self.bulk_mode = true;
            self.flush_batch()?;
            self.flush_workers();
        }
        self.connection
            .batch_execute(include_str!("pg_mode_bulk.sql"))?;
        Ok(())
    }

    fn mode_fresh(&mut self) -> Result<()> {
        info!("Entering fresh mode: dropping all indices");
        if !self.bulk_mode {
            self.bulk_mode = true;
            self.flush_batch()?;
            self.flush_workers();
        }
        self.connection
            .batch_execute(include_str!("pg_mode_fresh.sql"))?;
        Ok(())
    }

    fn mode_normal(&mut self) -> Result<()> {
        if self.bulk_mode {
            self.bulk_mode = false;
            self.flush_batch()?;
            self.flush_workers();
        }
        info!("Entering normal mode: creating all indices");
        self.connection
            .batch_execute(include_str!("pg_mode_normal.sql"))?;
        Ok(())
    }

    fn get_max_height(&mut self) -> Result<Option<BlockHeight>> {
        if let Some(height) = self.cached_max_height {
            return Ok(Some(height));
        }
        self.cached_max_height = self
            .connection
            .query("SELECT height FROM blocks ORDER BY id DESC LIMIT 1", &[])?
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
        // things in flight better
        self.flush_batch()?;
        if !self.in_flight.lock().unwrap().is_empty() {
            eprintln!("TODO: Unnecessary flush");
            self.flush_workers();
        }

        Ok(self
            .connection
            .query(
                "SELECT hash FROM blocks WHERE height = $1",
                &[&(height as i64)],
            )?
            .iter()
            .next()
            .map(|row| row.get::<_, Vec<u8>>(0))
            .map(|mut human_bytes| {
                human_bytes.reverse();
                BlockHash::from(human_bytes.as_slice())
            }))
    }

    fn insert(&mut self, info: BlockInfo) -> Result<()> {
        self.update_max_height(&info);

        self.batch_txs_total += info.block.txdata.len() as u64;
        let height = info.height;
        self.batch.push(info);
        if self.bulk_mode {
            if self.batch_txs_total > 100_000 {
                self.flush_batch()?;
            }
        } else if self.cached_max_height.expect("Already set") <= height {
            self.flush_batch()?;
        }

        Ok(())
    }

    fn wipe_to_height(&mut self, height: u64) -> Result<()> {
        info!("Deleting data above {}H", height);
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM blocks WHERE height > $1", &[&(height as i64)])?;
        transaction.execute("DELETE FROM txs WHERE id IN (SELECT txs.id FROM txs LEFT JOIN blocks ON txs.block_id = blocks.id WHERE blocks.id IS NULL)", &[])?;
        transaction.execute("DELETE FROM outputs WHERE id IN (SELECT outputs.id FROM outputs LEFT JOIN txs ON outputs.tx_id = txs.id WHERE txs.id IS NULL)", &[])?;
        transaction.execute("DELETE FROM inputs WHERE output_id IN (SELECT inputs.output_id FROM inputs LEFT JOIN txs ON inputs.tx_id = txs.id WHERE txs.id IS NULL)", &[])?;
        transaction.commit()?;
        trace!("Deleted data above {}H", height);
        Ok(())
    }
}
