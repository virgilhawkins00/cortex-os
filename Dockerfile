# ============================================================
# Cortex OS — Multi-Stage Production Build
# ============================================================
# Stage 1: Build the Rust workspace
# Stage 2: Minimal runtime image with only the binaries
# ============================================================

# ── Stage 1: Builder ───────────────────────────────────────
FROM rust:1.82-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependency compilation
COPY Cargo.toml Cargo.lock ./
COPY cortex-core/Cargo.toml cortex-core/Cargo.toml
COPY cortex-cli/Cargo.toml cortex-cli/Cargo.toml
COPY cortex-tui/Cargo.toml cortex-tui/Cargo.toml
COPY cortex-gateway/Cargo.toml cortex-gateway/Cargo.toml

# Create stub src files for dependency caching
RUN mkdir -p cortex-core/src cortex-cli/src cortex-tui/src cortex-gateway/src && \
    echo "pub fn stub() {}" > cortex-core/src/lib.rs && \
    echo "fn main() {}" > cortex-cli/src/main.rs && \
    echo "fn main() {}" > cortex-tui/src/main.rs && \
    echo "fn main() {}" > cortex-gateway/src/main.rs && \
    cargo build --workspace --release 2>/dev/null || true

# Now copy the real source and build
COPY . .
RUN cargo build --workspace --release

# ── Stage 2: Runtime ──────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    python3 \
    python3-pip \
    python3-venv \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd --create-home --shell /bin/bash cortex
USER cortex
WORKDIR /home/cortex

# Copy Rust binaries
COPY --from=builder --chown=cortex /build/target/release/cortex ./bin/cortex
COPY --from=builder --chown=cortex /build/target/release/cortex-tui ./bin/cortex-tui
COPY --from=builder --chown=cortex /build/target/release/cortex-gateway ./bin/cortex-gateway

# Copy Python memory service
COPY --from=builder --chown=cortex /build/cortex-memory ./cortex-memory

# Copy agent definitions
COPY --from=builder --chown=cortex /build/agents ./agents

# Install Python dependencies
RUN cd cortex-memory && \
    python3 -m venv .venv && \
    .venv/bin/pip install --no-cache-dir -e . 2>/dev/null || \
    .venv/bin/pip install --no-cache-dir -r requirements.txt 2>/dev/null || true

ENV PATH="/home/cortex/bin:${PATH}"

EXPOSE 4222 4223 8080

ENTRYPOINT ["cortex"]
CMD ["--daemon"]
