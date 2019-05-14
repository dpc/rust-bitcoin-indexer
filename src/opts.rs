use structopt::StructOpt;

#[derive(Debug, StructOpt, Clone)]
#[structopt(name = "indexer", about = "Bitcoin Indexer")]
pub struct Opts {
    #[structopt(long = "wipe-whole-db")]
    pub wipe_db: bool,

    #[structopt(long = "wipe-to-height")]
    pub wipe_to_height: Option<u64>,
}
