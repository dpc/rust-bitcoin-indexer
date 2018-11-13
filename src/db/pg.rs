use super::*;

use dotenv::dotenv;
use postgres::{transaction::Transaction, Connection, TlsMode};
use std::{env, str::FromStr};

pub fn establish_connection() -> Result<Connection> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;
    Ok(Connection::connect(database_url, TlsMode::None)?)
}

fn insert_parsed(transaction: &Transaction, parsed: &[Parsed]) -> Result<()> {
    let insert_block = transaction
        .prepare_cached("INSERT INTO blocks (height, hash, prev_hash) VALUES ($1, $2, $3)")?;
    let insert_tx = transaction
        .prepare_cached("INSERT INTO txs (height, hash, coinbase) VALUES ($1, $2, $3)")?;

    let insert_input = transaction.prepare_cached(
        "INSERT INTO inputs (height, utxo_tx_hash, utxo_tx_idx) VALUES ($1, $2, $3)",
    )?;
    let insert_output = transaction.prepare_cached(
        "INSERT INTO outputs (height, tx_hash, tx_idx, value, address, coinbase) VALUES ($1, $2, $3, $4, $5, $6)",
        )?;

    for parsed in parsed {
        let &Parsed {
            ref block,
            ref txs,
            ref outputs,
            ref inputs,
        } = parsed;

        insert_block.execute(&[
            &(block.height as i64),
            &block.hash.as_bytes().as_ref(),
            &block.prev_hash.as_bytes().as_ref(),
        ])?;

        for tx in txs {
            insert_tx.execute(&[
                &(tx.height as i64),
                &tx.hash.as_bytes().as_ref(),
                &tx.coinbase,
            ])?;
        }

        for input in inputs {
            insert_input.execute(&[
                &(input.height as i64),
                &input.utxo_tx_hash.as_bytes().as_ref(),
                &(input.utxo_tx_idx as i32),
            ])?;
        }

        for output in outputs {
            insert_output.execute(&[
                &(output.height as i64),
                &output.tx_hash.as_bytes().as_ref(),
                &(output.tx_idx as i32),
                &(output.value as i64),
                &output.address,
                &output.coinbase,
            ])?;
        }
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
            thread_num: num_cpus::get() * 2,
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
                            let transaction = connection.transaction()?;
                            insert_parsed(&transaction, &parsed);
                            transaction.commit()?;
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
