FROM rust:1.63.0

WORKDIR /usr/src/indexer
COPY . .

RUN cargo install --path .

CMD ["bitcoin-indexer"]