FROM rust:latest AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:trixie-slim
RUN apt-get update && apt-get install -y ca-certificates iproute2 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/sigma-agent /usr/local/bin/
CMD ["sigma-agent"]
