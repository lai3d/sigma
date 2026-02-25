FROM rust:latest AS builder
WORKDIR /app
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/sigma-api /usr/local/bin/
COPY --from=builder /app/migrations /migrations
WORKDIR /
ENTRYPOINT ["sigma-api"]
