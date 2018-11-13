use super::*;

use dotenv::dotenv;
use postgres::{transaction::Transaction, Connection, TlsMode};
use std::{env, str::FromStr};

pub fn establish_connection() -> Result<Connection> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;
    Ok(Connection::connect(database_url, TlsMode::None)?)
}

fn insert_parsed(transaction: &Transaction, parsed: Parsed) -> Result<()> {
    let Parsed {
        block,
        txs,
        outputs,
        inputs,
    } = parsed;
    transaction.execute(
        "INSERT INTO blocks (height, hash, prev_hash) VALUES ($1, $2, $3)",
        &[
            &(block.height as i64),
            &block.hash.as_bytes().as_ref(),
            &block.prev_hash.as_bytes().as_ref(),
        ],
    )?;

    for tx in &txs {
        transaction.execute(
            "INSERT INTO txs (height, hash, coinbase) VALUES ($1, $2, $3)",
            &[
                &(tx.height as i64),
                &tx.hash.as_bytes().as_ref(),
                &tx.coinbase,
            ],
        )?;
    }

    for input in &inputs {
        transaction.execute(
            "INSERT INTO inputs (height, utxo_tx_hash, utxo_tx_idx) VALUES ($1, $2, $3)",
            &[
                &(input.height as i64),
                &input.utxo_tx_hash.as_bytes().as_ref(),
                &(input.utxo_tx_idx as i32),
            ],
        )?;
    }

    for output in &outputs {
        transaction.execute(
                "INSERT INTO outputs (height, tx_hash, tx_idx, value, address, coinbase) VALUES ($1, $2, $3, $4, $5, $6)",
                &[
                    &(output.height as i64),
                    &output.tx_hash.as_bytes().as_ref(),
                    &(output.tx_idx as i32),
                    &(output.value as i64),
                    &output.address,
                    &output.coinbase,
                ],
            )?;
    }
    Ok(())
}
pub struct Postresql {
    // TODO: pool
    connection: Connection,
    tx: Option<crossbeam_channel::Sender<BlockInfo>>,
    thread_joins: Vec<std::thread::JoinHandle<Result<()>>>,
    thread_num: usize,
    max_height: Option<u64>,
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
            thread_num: 64,
            tx: default(),
            thread_joins: vec![],
            max_height: None,
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
        let (tx, rx) = crossbeam_channel::bounded(1024);
        self.tx = Some(tx);
        assert!(self.thread_joins.is_empty());
        for _ in 0..self.thread_num {
            self.thread_joins.push({
                std::thread::spawn({
                    let rx = rx.clone();
                    move || {
                        let connection = establish_connection().unwrap();

                        while let Ok(binfo) = rx.recv() {
                            let parsed = super::parse_node_block(&binfo)?;
                            let transaction = connection.transaction()?;
                            insert_parsed(&transaction, parsed);
                            transaction.commit()?;
                        }
                        Ok(())
                    }
                })
            });
        }
    }
}

impl DataStore for Postresql {
    fn get_max_height(&mut self) -> Result<Option<BlockHeight>> {
        self.max_height = self
            .connection
            .query("SELECT MAX(height) FROM blocks", &[])?
            .iter()
            .next()
            .and_then(|row| row.get::<_, Option<i64>>(0))
            .map(|u| u as u64);

        Ok(self.max_height)
    }

    fn get_hash_by_height(&mut self, height: BlockHeight) -> Result<Option<BlockHash>> {
        if let Some(max_height) = self.max_height {
            if max_height < height {
                return Ok(None);
            }
        }

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
        Ok(())
    }

    fn insert(&mut self, info: BlockInfo) -> Result<()> {
        if let Some(max_height) = self.max_height {
            if max_height < info.height {
                self.max_height = Some(info.height);
            }
        } else {
            self.max_height = Some(info.height);
        }

        self.tx.as_ref().unwrap().send(info);
        Ok(())
    }
}
