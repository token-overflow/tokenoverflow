FROM rust:1.93-slim

RUN apt-get update && apt-get install -y --no-install-recommends libpq-dev && rm -rf /var/lib/apt/lists/*
RUN rustup component add rustfmt
RUN cargo install diesel_cli --version 2.3.8 --no-default-features --features postgres

RUN useradd -r -u 1001 diesel
USER diesel

WORKDIR /volume

ENTRYPOINT ["diesel"]
