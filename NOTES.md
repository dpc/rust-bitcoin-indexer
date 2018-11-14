# Some notes and observations

Queering blocks in raw hex and parsing from that seems fast and easy. Node
(storage access) seems like a bottleneck. A pool of nodes could be used to
spread the load, as long as reorg detection is solid. But in practice,
it's going to be the indexer DB that is a bottleneck anyway.

I like the pattern of block fetching as an iterator (`src/prefetcher.rs`).

Indexer DB is going to be a overall bottleneck. No way around it. Most other
projects seem to go with simpler/faster, often local DBs for speed. I want
relational DB most, for all the features.

In postgres, using `INSERT INTO x VALUES (...), ..., (...)` seems like the
fastest "normal" way to pump a lot of records into the DB.

I would need to test a more real-life setup to tune the performance.

Right now I've used `diesel` for schema, but it's unnecessary and harmful.
It's better for the `indexer` to take care of the DB by itself, and eg.
create indexes only after it reached the chainhead (it's faster to initially
INSERT without indexes).

Running multiple indexer instances doesn't seem to help much (at least with
speed) of initial indexing, as it's the DB that is going to be a limiting
factor. And it creates problems of synchronization.

Calculating how much fee was paid is a PITA, because one needs to fetch all
the inputs to answer that. But the UTXO can be cached and kept in memory,
even in some form of size-bound LRU.
