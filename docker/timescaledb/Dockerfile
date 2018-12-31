# Instead of building from scratch, you can also pull from Docker Hub with:
# docker run -d --name timescaledb -p 5432:5432 -e POSTGRES_PASSWORD=password timescale/timescaledb
#
# Command: docker build . --no-cache --tag timescaledb:v11.1 --file Dockerfile
#
# Reference: https://github.com/timescale/timescaledb-docker

FROM timescale/timescaledb:latest-pg11
LABEL maintainer="dandriscoll@bitgo.com"

ENV POSTGRES_PASSWORD bitcoin-indexer
ENV POSTGRES_USER bitcoin-indexer
ENV POSTGRES_DB bitcoin-indexer

COPY postgresql.conf /var/lib/postgresql/
COPY 000_install_timescaledb.sh /docker-entrypoint-initdb.d/

EXPOSE 5432
