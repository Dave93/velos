# Stage 1: Zig build
FROM ghcr.io/ziglang/zig:0.15.2 AS zig-builder
WORKDIR /src
COPY zig/ zig/
COPY include/ include/
RUN cd zig && zig build -Doptimize=ReleaseFast

# Stage 2: Rust build
FROM rust:1.83 AS rust-builder
WORKDIR /src
COPY --from=zig-builder /src/zig/zig-out/lib/libvelos_core.a /src/zig/zig-out/lib/
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY include/ include/
COPY zig/build.zig zig/build.zig
RUN cargo build --release

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=rust-builder /src/target/release/velos /usr/local/bin/velos

ENV HOME=/root
RUN mkdir -p /root/.velos

EXPOSE 3100 9615

ENTRYPOINT ["velos"]
CMD ["daemon"]
