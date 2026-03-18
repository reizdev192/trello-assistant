# ============================================================
# Stage 1: Build Frontend (Node.js)
# ============================================================
FROM node:20-alpine AS frontend-builder

WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json* ./
RUN npm ci --silent
COPY frontend/ ./
RUN npm run build

# ============================================================
# Stage 2: Build Backend (Rust)
# ============================================================
FROM rust:1.82-bookworm AS backend-builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

# Copy built frontend into dist/ for embedding
COPY --from=frontend-builder /app/dist ./dist/

RUN cargo build --release

# ============================================================
# Stage 3: Runtime (minimal)
# ============================================================
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=backend-builder /app/target/release/trello-assistant ./

# Copy frontend dist
COPY --from=backend-builder /app/dist ./dist/

EXPOSE 3000

CMD ["./trello-assistant"]
