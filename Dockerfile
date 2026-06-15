# Stage 1: Build frontend (Vue 3 -> static assets)
# 源码即真理: 镜像内的前端永远由 frontend/ 重新编译, 不依赖宿主机 public/
FROM node:22-slim AS frontend-builder
WORKDIR /app/frontend

# 先拷依赖清单, 利用 layer 缓存
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --include=dev

# 再拷源码并构建 (vite.config.ts: outDir '../public' -> /app/public)
COPY frontend/ ./
RUN npm run build

# Stage 2: Build Rust backend
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

# Install runtime dependencies + gosu for privilege dropping
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    wget \
    gosu \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 appuser

# Copy binary and assets
COPY --from=builder /app/target/release/ai-hub-rust /app/ai-hub-rust
COPY --from=builder /app/migrations /app/migrations
# 前端来自 Stage 1 的构建产物, 不再依赖宿主机 public/
COPY --from=frontend-builder /app/public/ /app/public/

# Create data directory
RUN mkdir -p /app/data && chown -R appuser:appuser /app

# Entrypoint: fix volume permissions then drop to appuser
COPY <<'EOF' /app/entrypoint.sh
#!/bin/sh
chown -R appuser:appuser /app/data 2>/dev/null
exec gosu appuser /app/ai-hub-rust
EOF
RUN chmod +x /app/entrypoint.sh

# Environment defaults
ENV RUST_LOG=warn
ENV DATABASE_URL=sqlite:/app/data/ai-hub.db

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=10s --retries=3 --start-period=40s \
    CMD wget --spider -q http://localhost:8080/api/health || exit 1

ENTRYPOINT ["/app/entrypoint.sh"]
