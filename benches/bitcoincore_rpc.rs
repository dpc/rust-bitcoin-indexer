use criterion::{criterion_group, criterion_main, Criterion};

use bitcoin_hashes::{hex::FromHex, sha256d::Hash as Sha256dHash};
use bitcoincore_rpc::RpcApi;

fn get_rpc() -> bitcoincore_rpc::Client {
    bitcoincore_rpc::Client::new(
        "http://localhost:8332".into(),
        bitcoincore_rpc::Auth::UserPass("user".into(), "magicpassword".into()),
    )
    .expect("rpc client creation")
}

const BLOCK_HASH: &str = "00000000000000000013e7c4a60f1aa0e07ed1efb08accbe684602f7bdc7385f";

fn get_block(c: &mut Criterion) {
    c.bench_function("getblock", |b| {
        let rpc = get_rpc();
        let hash = Sha256dHash::from_hex(BLOCK_HASH).unwrap();

        b.iter(|| rpc.get_block(&hash).unwrap())
    });
    c.bench_function("getblock_verbose", |b| {
        let rpc = get_rpc();
        let hash = Sha256dHash::from_hex(BLOCK_HASH).unwrap();

        b.iter(|| rpc.get_block(&hash).unwrap())
    });
    c.bench_function("getrawtransaction", |b| {
        let rpc = get_rpc();
        let hash = Sha256dHash::from_hex(
            "45c105fadfac138711b3312044abd32cddedf7ef2cf466f10d93b5e83dba3ada",
        )
        .unwrap();

        b.iter(|| rpc.get_raw_transaction(&hash, None).unwrap())
    });
}

criterion_group!(benches, get_block);
criterion_main!(benches);
