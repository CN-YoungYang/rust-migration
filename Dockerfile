# Build stage
FROM rust:1.86-slim AS builder

WORKDIR /app

# 1C1G 优化: 限制并发构建
ENV CARGO_BUILD_JOBS=1

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Layer 1: Cache cargo dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Layer 2: Build with real source
COPY src/ src/
COPY migrations/ migrations/
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m appuser

# Copy binary and assets
COPY --from=builder /app/target/release/ai-hub-rust /app/ai-hub-rust
COPY --from=builder /app/migrations /app/migrations
COPY public/ /app/public/

# Create data directory and set ownership
RUN mkdir -p /app/data && chown -R appuser:appuser /app

USER appuser

# Environment defaults
ENV RUST_LOG=warn
ENV DATABASE_URL=sqlite:/app/data/ai-hub.db

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=10s --retries=3 --start-period=40s \
    CMD wget --spider -q http://localhost:8080/api/health || exit 1

CMD ["/app/ai-hub-rust"]
