# Build for Rust Bitcoin Indexer
# Command: docker build . --no-cache --tag rust-bitcoin-indexer --file <root>/docker/rust-bitcoin-indexer/Dockerfile

FROM rust:latest

ENV DATABASE_URL=postgres://bitcoin-indexer:bitcoin-indexer@localhost/bitcoin-indexer

RUN mkdir /usr/src/rust-bitcoin-indexer

COPY . /usr/src/rust-bitcoin-indexer/.

WORKDIR /usr/src/rust-bitcoin-indexer

RUN cargo install diesel_cli --no-default-features --features postgres

RUN cargo build --release

EXPOSE 8082

COPY ./docker/rust-bitcoin-indexer/start.sh /usr/src/rust-bitcoin-indexer/.