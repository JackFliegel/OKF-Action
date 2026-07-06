# ── Build stage ────────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

WORKDIR /app

# Copy manifests first so Docker can cache the dependency-compilation layer.
COPY Cargo.toml Cargo.lock ./

# Build a stub binary to pre-compile all dependencies.
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy the real source and rebuild only the changed crate.
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ── Runtime stage ──────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

# git is required for cloning external repositories.
RUN apt-get update && \
    apt-get install -y --no-install-recommends git ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/okf-action /usr/local/bin/okf-action

ENTRYPOINT ["/usr/local/bin/okf-action"]
