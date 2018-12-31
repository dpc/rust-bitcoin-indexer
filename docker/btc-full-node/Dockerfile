# Build for BTC Full Node
# Build Command: docker build . --no-cache --tag btc-full-node:v17 --file docker/btc-full-node/Dockerfile

# Originally Sourced & Modified From Docker Hub Here: https://hub.docker.com/r/ruimarinho/bitcoin-core/
FROM debian:stable-slim

LABEL maintainer="dandriscoll@bitgo.com"

ENV BITCOIN_VERSION=0.17.0
ENV PATH=/opt/bitcoin-${BITCOIN_VERSION}/bin:$PATH

# Sourced bitcoin release from: https://bitcoin.org/bin/bitcoin-core-${BITCOIN_VERSION}/bitcoin-${BITCOIN_VERSION}-x86_64-linux-gnu.tar.gz
COPY docker/btc-full-node/bitcoin-${BITCOIN_VERSION}-x86_64-linux-gnu.tar.gz /opt/.

RUN useradd -r bitcoin \
  && apt-get update -y \
  && apt-get install -y curl gnupg \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* \
  && set -ex \
  && cd /opt \
  && tar -xzf *.tar.gz -C . \
  && mkdir bitcoin_data

# VOLUME [" /opt/bitcoin_data"]

COPY docker/btc-full-node/bitcoin.conf /opt/bitcoin_data/.

EXPOSE 8331 8332 8333 18332 18333 18443 18444

CMD ["bitcoind", "-datadir=/opt/bitcoin_data"]
# Run with command: docker run --rm -it --name btc-full-node -p 8332:8332 btc-full-node:v17