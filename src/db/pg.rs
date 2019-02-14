use log::{debug, error, info, trace};

use super::*;
use crate::db;
use crate::prelude::*;
use dotenv::dotenv;
use postgres::{Connection, TlsMode};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::{env, fmt, fmt::Write};

pub fn establish_connection() -> Result<Connection> {
    dotenv()?;

    let database_url = env::var("DATABASE_URL")?;
    Ok(Connection::connect(database_url, TlsMode::None)?)
}

fn get_hash(row: &postgres::rows::Row, index: usize) -> BlockHash {
    let mut human_bytes = row.get::<_, Vec<u8>>(index);
    human_bytes.reverse();
    BlockHash::from(human_bytes.as_slice())
}

fn create_bulk_insert_blocks_query(blocks: &[db::Block]) -> Vec<String> {
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
        "INSERT INTO block (height, hash, prev_hash, merkle_root, time) VALUES".into();
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

fn create_bulk_insert_txs_query(
    txs: &[Tx],
    block_ids: &HashMap<BlockHash, i64>,
    outputs: &HashMap<OutPoint, UtxoSetEntry>,
) -> Vec<String> {
    if txs.is_empty() {
        return vec![];
    }
    if txs.len() > 9000 {
        let mid = txs.len() / 2;
        let mut p1 = create_bulk_insert_txs_query(&txs[0..mid], block_ids, outputs);
        let mut p2 = create_bulk_insert_txs_query(&txs[mid..txs.len()], block_ids, outputs);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String = "INSERT INTO tx (block_id, hash, fee, weight, coinbase) VALUES".into();
    for (i, tx) in txs.iter().enumerate() {
        let fee = if tx.coinbase {
            0
        } else {
            let input_value_sum = tx
                .inputs
                .iter()
                .fold(0, |acc, out_point| acc + outputs[out_point].value);
            assert!(tx.output_value_sum <= input_value_sum);
            input_value_sum - tx.output_value_sum
        };

        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "({},'\\x{}',{},{},{})",
            block_ids[&tx.block_hash], tx.hash, fee, tx.weight, tx.coinbase,
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
        "INSERT INTO output (tx_id, tx_idx, value, address, coinbase) VALUES ".into();
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
    if inputs.len() > 9_000 {
        let mid = inputs.len() / 2;
        let mut p1 = create_bulk_insert_inputs_query(&inputs[0..mid], outputs, tx_ids);
        let mut p2 = create_bulk_insert_inputs_query(&inputs[mid..inputs.len()], outputs, tx_ids);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String = "INSERT INTO input (output_id, tx_id) VALUES ".into();
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

fn create_fetch_outputs_query(outputs: &[OutPoint]) -> Vec<String> {
    if outputs.len() > 1500 {
        let mid = outputs.len() / 2;
        let mut p1 = create_fetch_outputs_query(&outputs[0..mid]);
        let mut p2 = create_fetch_outputs_query(&outputs[mid..outputs.len()]);
        p1.append(&mut p2);
        return p1;
    }
    let mut q: String = r#"
    SELECT output.id, output.value, tx.hash, output.tx_idx
    FROM output JOIN tx ON (tx.id = output.tx_id)
    JOIN block ON tx.block_id = block.id
    WHERE block.orphaned = false AND (tx.hash, output.tx_idx) IN ( VALUES "#
        .into();
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
        for q in create_fetch_outputs_query(&missing) {
            for row in &conn.query(&q, &[])? {
                let tx_hash = get_hash(&row, 2);
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
    debug!("Inserting {} {} from batch {}...", len, name, batch_id);
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
    read_next_id(conn, "tx", "id")
}

fn read_next_output_id(conn: &Connection) -> Result<i64> {
    read_next_id(conn, "output", "id")
}

fn read_next_block_id(conn: &Connection) -> Result<i64> {
    read_next_id(conn, "block", "id")
}

type BlocksInFlight = HashSet<BlockHash>;

/// Insertion Worker Thread
///
/// Reponsible for actually inserting data into the db.
struct InsertThread {
    tx: Option<crossbeam_channel::Sender<(u64, Vec<Parsed>)>>,
    processing_thread: Option<std::thread::JoinHandle<Result<()>>>,
    writer_thread: Option<std::thread::JoinHandle<Result<()>>>,
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
enum Mode {
    FreshBulk,
    Bulk,
    Normal,
}

impl Mode {
    fn is_bulk(self) -> bool {
        match self {
            Mode::FreshBulk => true,
            Mode::Bulk => true,
            Mode::Normal => false,
        }
    }

    fn to_sql_query_str(self) -> &'static str {
        match self {
            Mode::FreshBulk => include_str!("pg/mode_fresh.sql"),
            Mode::Bulk => include_str!("pg/mode_bulk.sql"),
            Mode::Normal => include_str!("pg/mode_normal.sql"),
        }
    }

    fn to_entering_str(self) -> &'static str {
        match self {
            Mode::FreshBulk => "fresh mode: no indices",
            Mode::Bulk => "fresh mode: minimum indices",
            Mode::Normal => "normal mode: all indices",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Mode::FreshBulk => "fresh-bulk",
            Mode::Bulk => "bulk",
            Mode::Normal => "normal",
        })
    }
}

impl InsertThread {
    fn new(in_flight: Arc<Mutex<BlocksInFlight>>) -> Result<Self> {
        // We use only rendezvous (0-size) channels, to allow passing
        // work and parallelism, but without doing any buffering of
        // work in the channels. Buffered work does not
        // improve performance, and  more things in flight means
        // incrased memory usage.
        let (tx, rx) = crossbeam_channel::bounded::<(u64, Vec<Parsed>)>(0);
        let (writer_tx, writer_rx) =
            crossbeam_channel::bounded::<(u64, Vec<Vec<String>>, HashMap<BlockHash, i64>, u64, usize)>(0);

        let utxo_set_cache = Arc::new(Mutex::new(UtxoSetCache::default()));

        let processing_thread = std::thread::spawn({
            let conn = establish_connection()?;
            fn_log_err("pg_processing", move || {
                let mut next_block_id = read_next_block_id(&conn)?;
                let mut next_tx_id = read_next_tx_id(&conn)?;
                let mut next_output_id = read_next_output_id(&conn)?;
                while let Ok((batch_id, parsed)) = rx.recv() {
                    let mut blocks: Vec<super::Block> = vec![];
                    let mut txs: Vec<super::Tx> = vec![];
                    let mut outputs: Vec<super::Output> = vec![];
                    let mut inputs: Vec<super::Input> = vec![];
                    let mut pending_queries = vec![];
                    let mut tx_len = 0;

                    for mut parsed in parsed {
                        blocks.push(parsed.block);
                        tx_len += parsed.txs.len();
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

                    let block_ids: HashMap<_, _> = blocks
                        .iter()
                        .enumerate()
                        .map(|(i, tx)| (tx.hash, next_block_id + i as i64))
                        .collect();

                    next_block_id += blocks.len() as i64;

                    let tx_ids: HashMap<_, _> = txs
                        .iter()
                        .enumerate()
                        .map(|(i, tx)| (tx.hash, next_tx_id + i as i64))
                        .collect();

                    next_tx_id += txs.len() as i64;

                    let mut utxo_lock = utxo_set_cache.lock().unwrap();
                    outputs.iter().enumerate().for_each(|(i, output)| {
                        let id = next_output_id + (i as i64);
                        utxo_lock.insert(output.out_point, id, output.value);
                    });
                    drop(utxo_lock);

                    next_output_id += outputs.len() as i64;

                    let mut utxo_lock = utxo_set_cache.lock().unwrap();
                    let (mut output_ids, missing) =
                        utxo_lock.consume(inputs.iter().map(|i| i.out_point));
                    drop(utxo_lock);

                    // We're fetching utxos, before marking any blocks potentially
                    // orphaned by currently inserted ones. You might wonder if that means,
                    // we could potentially pick ids of `output`s that will be orphaned.
                    // Fortunately it's not a problem. If `output` like that was to be orphaned, and re-added,
                    // it would have to be in the blocks we're processing and therfore can not
                    // be "missing", as we already must have added it to a cache. This is howerver quite
                    // subtle, so worth remembering about, and thinking about again.
                    //
                    // To rephrase: correctness of this fetching getting fresh IDs,
                    // depends on all possible valid outputs being either in the UTXO cache,
                    // or having any previous instances from orphaned blocks already marked
                    // as such.
                    let missing = UtxoSetCache::fetch_missing(&conn, missing)?;
                    for (k, v) in missing.into_iter() {
                        output_ids.insert(k, v);
                    }

                    pending_queries.push(vec![format!(
                        "UPDATE block SET orphaned = true WHERE height >= {};",
                        min_block_height
                    )]);
                    pending_queries.push(create_bulk_insert_blocks_query(&blocks));
                    pending_queries.push(create_bulk_insert_txs_query(
                        &txs,
                        &block_ids,
                        &output_ids,
                    ));
                    pending_queries.push(create_bulk_insert_outputs_query(&outputs, &tx_ids));
                    pending_queries.push(create_bulk_insert_inputs_query(
                        &inputs,
                        &output_ids,
                        &tx_ids,
                    ));

                    writer_tx
                        .send((batch_id, pending_queries, block_ids, max_block_height, tx_len))
                        .expect("Send not fail");
                }
                Ok(())
            })
        });

        let writer_thread = std::thread::spawn({
            let conn = establish_connection()?;
            fn_log_err("pg_writer", move || {
                let mut prev_time = std::time::Instant::now();
                while let Ok((batch_id, queries, block_ids, max_block_height, tx_len)) = writer_rx.recv() {
                    execute_bulk_insert_transcation(
                        &conn,
                        "all block data",
                        block_ids.len(),
                        batch_id,
                        queries.into_iter().flatten(),
                    )?;

                    let current_time = std::time::Instant::now();
                    let duration = current_time.duration_since(prev_time);
                    prev_time = current_time;

                    info!(
                        "Block {}H fully indexed and commited; {}block/s; {}tx/s",
                        max_block_height,
                        block_ids.len() as u64 / duration.as_secs(),
                        tx_len as u64 / duration.as_secs(),
                    );

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

        Ok(InsertThread {
            tx: Some(tx),
            processing_thread: Some(processing_thread),
            writer_thread: Some(writer_thread),
        })
    }
}

impl Drop for InsertThread {
    fn drop(&mut self) {
        drop(self.tx.take());

        let joins = vec![
            self.processing_thread.take().unwrap(),
            self.writer_thread.take().unwrap(),
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
    pipeline: Option<InsertThread>,
    batch: Vec<crate::BlockCore>,
    batch_txs_total: u64,
    batch_id: u64,
    mode: Mode,
    node_chain_head_height: BlockHeight,

    in_flight: Arc<Mutex<BlocksInFlight>>,

    // for double checking logic here
    last_inserted_block_height: Option<BlockHeight>,
    num_inserted: u64,
}

impl Drop for Postresql {
    fn drop(&mut self) {
        self.stop_workers();
    }
}

impl Postresql {
    pub fn new(node_chain_head_height: BlockHeight) -> Result<Self> {
        let connection = establish_connection()?;
        Self::init(&connection)?;
        let mode = Self::read_indexer_state(&connection)?;
        let mut s = Postresql {
            connection,
            pipeline: None,
            cached_max_height: None,
            batch: vec![],
            batch_txs_total: 0,
            batch_id: 0,
            mode,
            node_chain_head_height,
            in_flight: Arc::new(Mutex::new(BlocksInFlight::new())),
            last_inserted_block_height: None,
            num_inserted: 0,
        };
        if s.mode == Mode::FreshBulk {
            s.self_test()?;
        }
        s.set_schema_to_mode(s.mode)?;
        s.start_workers();
        Ok(s)
    }

    fn read_indexer_state(conn: &Connection) -> Result<Mode> {
        let state = conn.query("SELECT bulk_mode FROM indexer_state", &[])?;
        if let Some(state) = state.iter().next() {
            let is_bulk_mode = state.get(0);
            let mode = if is_bulk_mode {
                let count = conn
                    .query("SELECT COUNT(*) FROM block", &[])?
                    .into_iter()
                    .next()
                    .expect("A row from the db")
                    .get::<_, i64>(0);
                if count == 0 {
                    Mode::FreshBulk
                } else {
                    Mode::Bulk
                }
            } else {
                Mode::Normal
            };

            Ok(mode)
        } else {
            conn.execute(
                "INSERT INTO indexer_state (bulk_mode) VALUES ($1)",
                &[&true],
            )?;
            Ok(Mode::FreshBulk)
        }
    }

    fn init(conn: &Connection) -> Result<()> {
        info!("Creating initial db schema");
        conn.batch_execute(include_str!("pg/init_base.sql"))?;
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
        self.pipeline = Some(InsertThread::new(self.in_flight.clone()).unwrap())
    }

    fn flush_workers(&mut self) {
        self.stop_workers();
        self.start_workers();
    }

    fn update_max_height(&mut self, block: &crate::BlockCore) {
        self.cached_max_height = Some(
            self.cached_max_height
                .map_or(block.height, |h| std::cmp::max(h, block.height)),
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

    pub fn wipe() -> Result<()> {
        info!("Wiping db schema");
        let connection = establish_connection()?;
        connection.batch_execute(include_str!("pg/wipe.sql"))?;
        Ok(())
    }

    fn set_mode(&mut self, mode: Mode) -> Result<()> {
        if self.mode == mode {
            return Ok(());
        }

        self.set_mode_uncodintionally(mode)?;
        Ok(())
    }

    fn set_schema_to_mode(&mut self, mode: Mode) -> Result<()> {
        info!("Adjusting schema to mode: {}", mode);
        self.connection.batch_execute(mode.to_sql_query_str())?;
        Ok(())
    }

    fn set_mode_uncodintionally(&mut self, mode: Mode) -> Result<()> {
        self.mode = mode;

        info!("Entering {}", mode.to_entering_str());
        self.flush_batch()?;
        self.flush_workers();

        self.set_schema_to_mode(mode)?;
        // commit to the new mode in the db last
        self.connection.execute(
            "UPDATE indexer_state SET bulk_mode = $1",
            &[&(mode.is_bulk())],
        )?;
        Ok(())
    }

    /// Switch between all modes to double-check all queries
    fn self_test(&mut self) -> Result<()> {
        assert_eq!(self.mode, Mode::FreshBulk);

        self.set_mode_uncodintionally(Mode::FreshBulk)?;
        self.set_mode_uncodintionally(Mode::Bulk)?;
        self.set_mode_uncodintionally(Mode::Normal)?;
        self.set_mode_uncodintionally(Mode::Bulk)?;
        self.set_mode_uncodintionally(Mode::FreshBulk)?;
        Ok(())
    }
}

impl DataStore for Postresql {
    fn get_head_height(&mut self) -> Result<Option<BlockHeight>> {
        if let Some(height) = self.cached_max_height {
            return Ok(Some(height));
        }
        self.cached_max_height = self
            .connection
            .query("SELECT height FROM block ORDER BY id DESC LIMIT 1", &[])?
            .iter()
            .next()
            .and_then(|row| row.get::<_, Option<i64>>(0))
            .map(|u| u as u64);

        Ok(self.cached_max_height)
    }

    fn get_hash_by_height(&mut self, height: BlockHeight) -> Result<Option<BlockHash>> {
        trace!("PG: get_hash_by_height {}H", height);
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
                "SELECT hash FROM block WHERE height = $1 AND orphaned = false",
                &[&(height as i64)],
            )?
            .iter()
            .next()
            .map(|row| get_hash(&row, 0)))
    }

    fn insert(&mut self, block: crate::BlockCore) -> Result<()> {
        if let Some(db_hash) = self.get_hash_by_height(block.height)? {
            if db_hash != block.id {
                // we move forward and there is a query in a inseting
                // pipeline (`reorg_queries`)
                // that will mark anything above and eq this hight as orphaned
                info!(
                    "Node block != db block at {}H; {} != {} - reorg",
                    block.height, block.id, db_hash
                );
            } else {
                // we already have exact same block, non-orphaned, and we don't want
                // to add it twice
                trace!(
                    "Skip indexing alredy included block {}H {}",
                    block.height,
                    block.id
                );
                // if we're here, we must have not inserted anything yet,
                // and these are prefetcher starting from some past blocks
                assert_eq!(self.num_inserted, 0);
                return Ok(());
            }
        } else {
            // we can only be inserting non-reorg blocks one height at a time
            if let Some(last_inserted_block_height) = self.last_inserted_block_height {
                assert_eq!(block.height, last_inserted_block_height + 1);
            }
        }

        self.num_inserted += 1;
        self.last_inserted_block_height = Some(block.height);

        self.update_max_height(&block);

        self.batch_txs_total += block.data.txdata.len() as u64;
        let height = block.height;
        self.batch.push(block);

        if self.mode.is_bulk() {
            if self.batch_txs_total > 100_000 {
                self.flush_batch()?;
            }
        } else if self.cached_max_height.expect("Already set") <= height {
            self.flush_batch()?;
        }

        if self.node_chain_head_height == height {
            self.set_mode(Mode::Normal)?;
        }

        Ok(())
    }

    fn wipe_to_height(&mut self, height: u64) -> Result<()> {
        info!("Deleting data above {}H", height);
        let transaction = self.connection.transaction()?;
        info!("Deleting blocks above {}H", height);
        transaction.execute("DELETE FROM block WHERE height > $1", &[&(height as i64)])?;
        info!("Deleting txs above {}H", height);
        transaction.execute("DELETE FROM tx WHERE id IN (SELECT tx.id FROM tx LEFT JOIN block ON tx.block_id = block.id WHERE block.id IS NULL)", &[])?;
        info!("Deleting outputs above {}H", height);
        transaction.execute("DELETE FROM output WHERE id IN (SELECT output.id FROM output LEFT JOIN tx ON output.tx_id = tx.id WHERE tx.id IS NULL)", &[])?;
        info!("Deleting inputs above {}H", height);
        transaction.execute("DELETE FROM input WHERE output_id IN (SELECT input.output_id FROM input LEFT JOIN tx ON input.tx_id = tx.id WHERE tx.id IS NULL)", &[])?;
        transaction.commit()?;
        trace!("Deleted data above {}H", height);
        Ok(())
    }
}

impl crate::event_source::EventSource for postgres::Connection {
    type Cursor = i64;
    type Id = BlockHash;
    type Data = ();

    fn next(
        &mut self,
        cursor: Option<Self::Cursor>,
        limit: u64,
    ) -> Result<(Vec<Block<Self::Id, Self::Data>>, Self::Cursor)> {
        let cursor = cursor.unwrap_or(-1);
        let rows = self.query(
            "SELECT id, hash, height FROM block WHERE block.id > $1 ORDER BY id ASC LIMIT $2;",
            &[&cursor, &(limit as i64)],
        )?;

        let mut res = vec![];
        let mut prev_height = None;
        let mut last = None;

        for row in &rows {
            let id: i64 = row.get(0);
            let hash = get_hash(&row, 1);
            let height: i64 = row.get(2);
            if prev_height
                .map(|prev_height| height <= prev_height)
                .unwrap_or(true)
            {
                let orphaned = self.query(
                    r#"SELECT id, hash, height
                    FROM block
                    WHERE block.id < $1 AND
                      block.id > (SELECT MAX(id) FROM block WHERE height < $2 AND id < $1)
                    ORDER BY id DESC;"#,
                    &[&id, &height],
                )?;
                for row in &orphaned {
                    let _id: i64 = row.get(0);
                    let hash = get_hash(&row, 1);
                    let height: i64 = row.get(2);
                    res.push(Block {
                        height: height as u64,
                        id: hash,
                        data: (),
                    });
                }
            }
            res.push(Block {
                height: height as u64,
                id: hash,
                data: (),
            });

            prev_height = Some(height);
            last = Some(id);
        }

        Ok((res, last.unwrap_or(cursor)))
    }
}
