# Stage 1: Build eBPF kernel programs (nightly toolchain, BPF target)
FROM rust:latest AS ebpf-builder
RUN rustup install nightly && rustup component add rust-src --toolchain nightly
WORKDIR /build
COPY sigma-agent-ebpf-common/ sigma-agent-ebpf-common/
COPY sigma-agent-ebpf/ sigma-agent-ebpf/
RUN cd sigma-agent-ebpf && \
    cargo +nightly build \
      --release \
      -Z build-std=core \
      --target bpfel-unknown-none

# Stage 2: Build agent with eBPF feature (stable toolchain)
FROM rust:latest AS builder
WORKDIR /build
COPY sigma-agent-ebpf-common/ sigma-agent-ebpf-common/
COPY sigma-agent/ sigma-agent/
# Copy compiled eBPF bytecode into a well-known location
COPY --from=ebpf-builder \
    /build/sigma-agent-ebpf/target/bpfel-unknown-none/release/sigma-agent-ebpf \
    /build/sigma-agent/ebpf-programs/sigma-agent-ebpf
RUN cd sigma-agent && cargo build --release --features ebpf-traffic

# Stage 3: Minimal runtime image
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y ca-certificates iproute2 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/sigma-agent/target/release/sigma-agent /usr/local/bin/
CMD ["sigma-agent"]
