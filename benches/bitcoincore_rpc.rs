use criterion::Criterion;
use criterion::{criterion_group, criterion_main};

use bitcoin::util::hash::Sha256dHash;

fn get_rpc() -> bitcoin_rpc::BitcoinRpc {
    bitcoin_rpc::BitcoinRpc::new(
        "http://localhost:8332".into(),
        Some("user".into()),
        Some("magicpassword".into()),
    )
}

const BLOCK_HASH = "00000000000000000013e7c4a60f1aa0e07ed1efb08accbe684602f7bdc7385f";

fn get_block(c: &mut Criterion) {
    c.bench_function("getblock", |b| {
        let rpc = get_rpc();
        let hash = Sha256dHash::from_hex(
            BLOCK_HASH,
        )
        .unwrap();

        b.iter(|| rpc.get_block(&hash).unwrap())
    });
    c.bench_function("getblock_verbose", |b| {
        let rpc = get_rpc();
        let hash = Sha256dHash::from_hex(
            BLOCK_HASH,
        )
        .unwrap();

        b.iter(|| rpc.get_block_verbose(&hash).unwrap())
    });
    c.bench_function("getrawtransaction", |b| {
        let rpc = get_rpc();
        let hash = Sha256dHash::from_hex(
            "45c105fadfac138711b3312044abd32cddedf7ef2cf466f10d93b5e83dba3ada",
        )
        .unwrap();

        b.iter(|| rpc.get_raw_transaction(&hash).unwrap())
    });
}

criterion_group!(benches, get_block);
criterion_main!(benches);
