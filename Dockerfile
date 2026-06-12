# Build stage
FROM rust:1.86-slim as builder

WORKDIR /app

# 1C1G 优化: 限制并发构建

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy all source files
COPY . .

# Build directly (skip dependency cache)
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=builder /app/target/release/ai-hub-rust /app/ai-hub-rust
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/public /app/public

# Create data directory
RUN mkdir -p /app/data

# Environment
ENV RUST_LOG=warn
ENV DATABASE_URL=sqlite:/app/data/ai-hub.db

EXPOSE 8080

CMD ["/app/ai-hub-rust"]

