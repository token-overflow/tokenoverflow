FROM rust:1.93-slim

RUN apt-get update && apt-get install -y libpq-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install diesel_cli --no-default-features --features postgres

WORKDIR /volume

ENTRYPOINT ["diesel"]
