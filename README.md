# 🧠 Cortex OS

> **The Autonomous AI Operating System.**
> Uncensored. Sovereign. Plug & Play.

---

## What is Cortex OS?

Cortex OS is a **unified, from-scratch** autonomous AI runtime that combines:

- **Sandboxed execution** of code and shell commands (Rust)
- **Persistent semantic memory** with verbatim recall (Python + ChromaDB)
- **Multi-agent workflow orchestration** (plan → execute → verify)
- **Local-first LLM inference** with uncensored models (Ollama)
- **Beautiful interfaces** — both TUI (terminal) and Web dashboard

One binary. One `docker compose up`. Done.

## Architecture

```
cortex-os/
├── cortex-core/        # Rust lib — execution runtime, tools, sandbox, permissions
├── cortex-cli/         # Rust bin — interactive REPL + NATS daemon
├── cortex-tui/         # Rust bin — Ratatui terminal dashboard
├── cortex-memory/      # Python — semantic memory, knowledge graph, LLM brain
├── web/                # React+Vite — web dashboard
├── docker-compose.yml  # NATS + Ollama + ChromaDB
└── Cargo.toml          # Rust workspace root
```

## Quick Start

```bash
git clone git@github.com:virgilhawkins00/cortex-os.git
cd cortex-os

# Start infrastructure
cp .env.example .env
docker compose up -d
docker exec cortex_ollama ollama pull dolphin-mistral:latest

# Build and run
cargo build --workspace --release
./target/release/cortex
```

## Tech Stack

| Layer | Tech | Purpose |
|---|---|---|
| Execution | Rust (tokio, async) | Tool registry, sandbox, permissions |
| Memory | Python (ChromaDB, SQLite) | Semantic search, knowledge graph |
| Bus | NATS | Component communication |
| LLM | Ollama | Local uncensored inference |
| TUI | Ratatui | Terminal dashboard |
| Web | React + Vite | Web dashboard |

## License

MIT — [@virgilhawkins00](https://github.com/virgilhawkins00)
