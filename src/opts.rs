use structopt::StructOpt;

#[derive(Debug, StructOpt, Clone)]
#[structopt(name = "indexer", about = "Bitcoin Indexer")]
pub struct Opts {
    #[structopt(long = "rpc-url")]
    pub node_rpc_url: String,
    #[structopt(long = "rpc-user")]
    pub node_rpc_user: Option<String>,
    #[structopt(long = "rpc-pass")]
    pub node_rpc_pass: Option<String>,
}
