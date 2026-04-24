# ─── Build stage ────────────────────────────────────────────────────────────
FROM rust:1.87-slim AS builder

WORKDIR /app

RUN apt-get update -y && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY frontend ./frontend

RUN cargo build --release

# ─── Runtime stage ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update -y && \
    apt-get install -y ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/roundchat /usr/local/bin/roundchat
COPY frontend /app/frontend

WORKDIR /app

EXPOSE 3000

CMD ["roundchat", "serve"]
