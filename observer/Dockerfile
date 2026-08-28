FROM rust:1.91.1-bookworm AS builder
WORKDIR /workspace
COPY observer/Cargo.toml observer/Cargo.lock ./
COPY observer/src ./src
COPY observer/migrations ./migrations
RUN cargo build --locked --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /workspace/target/release/iron-vault-observer /usr/local/bin/iron-vault-observer
COPY --from=builder /workspace/migrations /app/migrations
WORKDIR /app
USER 65532:65532
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=3s --retries=3 CMD curl --fail --silent http://127.0.0.1:8080/healthz || exit 1
ENTRYPOINT ["iron-vault-observer"]
