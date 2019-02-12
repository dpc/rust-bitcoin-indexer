use bitcoin_indexer::{db, event_source::EventSource, opts};
use common_failures::{prelude::*, quick_main};

fn run() -> Result<()> {
    env_logger::init();
    let mut db = db::pg::establish_connection()?;

    let mut cursor = None;

    loop {
        let (blocks, next_cursor) = db.next(cursor, 10)?;
        for block in blocks {
            println!("{} {}H", block.id, block.height);
        }
        if Some(next_cursor) == cursor {
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
        cursor = Some(next_cursor);
    }
}

quick_main!(run);
