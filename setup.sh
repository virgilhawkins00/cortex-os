#!/usr/bin/env bash
set -euo pipefail

# ============================================================
# Cortex OS — One-Liner Installer
# Usage: curl -sSL https://raw.githubusercontent.com/.../setup.sh | bash
# ============================================================

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log()   { echo -e "${GREEN}[cortex]${NC} $*"; }
warn()  { echo -e "${YELLOW}[cortex]${NC} $*"; }
error() { echo -e "${RED}[cortex]${NC} $*"; exit 1; }

# ── Detect OS ───────────────────────────────────────────────
detect_os() {
    case "$(uname -s)" in
        Darwin*) OS="macos" ;;
        Linux*)  OS="linux" ;;
        *)       error "Unsupported OS: $(uname -s). Only macOS and Linux are supported." ;;
    esac
    log "Detected OS: ${BOLD}${OS}${NC}"
}

# ── Check Dependencies ─────────────────────────────────────
check_dep() {
    if ! command -v "$1" &> /dev/null; then
        warn "$1 not found. Attempting to install..."
        return 1
    fi
    return 0
}

install_deps() {
    # Rust
    if ! check_dep "cargo"; then
        log "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi
    log "Rust: $(rustc --version)"

    # Docker
    if ! check_dep "docker"; then
        error "Docker is required but not installed. Please install Docker Desktop: https://docker.com/get-started"
    fi
    log "Docker: $(docker --version)"

    # Docker Compose
    if ! docker compose version &> /dev/null; then
        error "Docker Compose V2 is required. Please update Docker Desktop."
    fi
    log "Docker Compose: $(docker compose version --short)"

    # Python 3
    if ! check_dep "python3"; then
        if [ "$OS" = "macos" ]; then
            brew install python3
        else
            sudo apt-get update && sudo apt-get install -y python3 python3-pip python3-venv
        fi
    fi
    log "Python: $(python3 --version)"
}

# ── Build Cortex OS ─────────────────────────────────────────
build_cortex() {
    log "Building Cortex OS workspace (release mode)..."
    cargo build --workspace --release

    log "Installing Python dependencies for cortex-memory..."
    cd cortex-memory
    python3 -m venv .venv 2>/dev/null || true
    source .venv/bin/activate 2>/dev/null || true
    pip install -q -e . 2>/dev/null || pip install -q -r requirements.txt 2>/dev/null || true
    cd ..
}

# ── Start Infrastructure ───────────────────────────────────
start_infra() {
    log "Starting NATS + Ollama + ChromaDB..."
    if [ ! -f .env ]; then
        cp .env.example .env 2>/dev/null || true
    fi
    docker compose up -d

    log "Pulling default LLM model (dolphin-mistral)..."
    docker exec cortex_ollama ollama pull dolphin-mistral:latest 2>/dev/null || \
        warn "Could not pull model automatically. Run: docker exec cortex_ollama ollama pull dolphin-mistral:latest"
}

# ── Print Success ──────────────────────────────────────────
print_success() {
    echo ""
    echo -e "${GREEN}${BOLD}"
    echo "  ██████╗ ██████╗ ██████╗ ████████╗███████╗██╗  ██╗"
    echo "  ██╔════╝██╔═══██╗██╔══██╗╚══██╔══╝██╔════╝╚██╗██╔╝"
    echo "  ██║     ██║   ██║██████╔╝   ██║   █████╗   ╚███╔╝ "
    echo "  ██║     ██║   ██║██╔══██╗   ██║   ██╔══╝   ██╔██╗ "
    echo "  ╚██████╗╚██████╔╝██║  ██║   ██║   ███████╗██╔╝ ██╗"
    echo "   ╚═════╝ ╚═════╝ ╚═╝  ╚═╝   ╚═╝   ╚══════╝╚═╝  ╚═╝"
    echo -e "${NC}"
    echo -e "  ${BOLD}Installation complete!${NC}"
    echo ""
    echo "  Run Cortex OS:"
    echo "    ./target/release/cortex"
    echo ""
    echo "  Setup Vault (recommended):"
    echo "    ./target/release/cortex vault init"
    echo "    ./target/release/cortex vault set ANTHROPIC_API_KEY sk-ant-..."
    echo ""
    echo "  Dashboard:"
    echo "    ./target/release/cortex-tui"
    echo ""
}

# ── Main ───────────────────────────────────────────────────
main() {
    echo ""
    log "${BOLD}Cortex OS Installer${NC}"
    log "─────────────────────────────────────"
    
    detect_os
    install_deps
    build_cortex
    start_infra
    print_success
}

main "$@"
