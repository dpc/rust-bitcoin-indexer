use super::*;

use dotenv::dotenv;
use postgres::{transaction::Transaction, Connection, TlsMode};
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

fn insert_outputs_query(outputs: &[Output]) -> Vec<String> {
    if outputs.is_empty() {
        return vec![];
    }
    if outputs.len() > 9000 {
        let mid = outputs.len() / 2;
        let mut p1 = insert_outputs_query(&outputs[0..mid]);
        let mut p2 = insert_outputs_query(&outputs[mid..outputs.len()]);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String =
        "INSERT INTO outputs (height, tx_hash, tx_idx, value, address, coinbase) VALUES ".into();
    for (i, output) in outputs.iter().enumerate() {
        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "({}, '\\x{}', {}, {}, {}, {})",
            output.height,
            output.tx_hash,
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

fn insert_inputs_query(inputs: &[Input]) -> Vec<String> {
    if inputs.is_empty() {
        return vec![];
    }
    if inputs.len() > 9000 {
        let mid = inputs.len() / 2;
        let mut p1 = insert_inputs_query(&inputs[0..mid]);
        let mut p2 = insert_inputs_query(&inputs[mid..inputs.len()]);
        p1.append(&mut p2);
        return p1;
    }

    let mut q: String = "INSERT INTO inputs (height, utxo_tx_hash, utxo_tx_idx) VALUES ".into();
    for (i, input) in inputs.iter().enumerate() {
        if i > 0 {
            q.push_str(",")
        }
        q.write_fmt(format_args!(
            "({}, '\\x{}', {})",
            input.height, input.utxo_tx_hash, input.utxo_tx_idx,
        ))
        .unwrap();
    }
    q.write_str(";");
    return vec![q];
}

fn insert_parsed(conn: &Connection, parsed: Vec<Parsed>) -> Result<()> {
    let mut bs = vec![];
    let mut ts = vec![];
    let mut is = vec![];
    let mut os = vec![];

    for parsed in parsed.into_iter() {
        let Parsed {
            mut block,
            mut txs,
            mut outputs,
            mut inputs,
        } = parsed;
        bs.push(block);
        ts.append(&mut txs);
        is.append(&mut inputs);
        os.append(&mut outputs);
    }
    for s in insert_txs_query(&ts) {
        conn.batch_execute(&s)?;
    }
    for s in insert_inputs_query(&is) {
        conn.batch_execute(&s)?;
    }
    for s in insert_outputs_query(&os) {
        conn.batch_execute(&s)?;
    }

    for s in insert_blocks_query(&bs) {
        conn.batch_execute(&s)?;
    }

    Ok(())
}
pub struct Postresql {
    // TODO: pool
    connection: Connection,
    tx: Option<crossbeam_channel::Sender<Vec<Parsed>>>,
    thread_joins: Vec<std::thread::JoinHandle<Result<()>>>,
    thread_num: usize,
    cached_max_height: Option<u64>,
    batch: Vec<super::Parsed>,
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
            // It looks like 1 thread can already saturate
            // PG; convenient, because otherwise there's a problem
            // of potential holes in the DB, if indexer was shut down
            // forcefully and one thread have written higher-height blocks
            // while the other one did not complete yet --dpc
            thread_num: 1,
            tx: default(),
            thread_joins: vec![],
            cached_max_height: None,
            batch: vec![],
            batch_txs_total: 0,
        };
        s.start_workers();
        Ok(s)
    }

    fn stop_workers(&mut self) {
        drop(self.tx.take());

        let results: Vec<_> = self.thread_joins.drain(..).map(|j| j.join()).collect();
        for res in results.into_iter() {
            res.expect("Worker thread panicked");
        }
    }

    fn start_workers(&mut self) {
        let (tx, rx) = crossbeam_channel::bounded(self.thread_num * 2);
        self.tx = Some(tx);
        assert!(self.thread_joins.is_empty());
        for _ in 0..self.thread_num {
            self.thread_joins.push({
                std::thread::spawn({
                    let rx = rx.clone();
                    move || {
                        let connection = establish_connection().unwrap();

                        while let Ok(parsed) = rx.recv() {
                            insert_parsed(&connection, parsed);
                        }
                        Ok(())
                    }
                })
            });
        }
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

    fn flush_batch(&mut self) {
        self.tx
            .as_ref()
            .unwrap()
            .send(std::mem::replace(&mut self.batch, vec![]));
        self.batch_txs_total = 0;
    }
}

impl DataStore for Postresql {
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
        self.batch.push(super::parse_node_block(&info)?);
        if self.batch_txs_total > 10000 {
            self.flush_batch();
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.flush_batch();
        Ok(())
    }
}
