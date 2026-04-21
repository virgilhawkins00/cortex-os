# 🧠 Cortex OS

> **The Autonomous AI Operating System.**
> Uncensored. Sovereign. Plug & Play.

---

## What is Cortex OS?

Cortex OS is a **unified, from-scratch** autonomous AI runtime, designed as a superior, extensible alternative to tools like Claude Code and OpenDevin. It operates natively in your terminal with advanced multi-agent capabilities.

- **Sandboxed execution** of code and shell commands (Rust)
- **Native Mempalace Integration**: Persistent semantic memory with verbatim recall, dramatically reducing token context window bloat compared to standard chat platforms.
- **Multi-Agent Swarms (Squads)**: Define specialized squads (e.g., Financial Analysts or Full-Stack Engineering teams) that work in parallel on complex domains using modern stacks (JS/TS, Rust, Go, Python, C++).
- **Model Flexibility**: Built Open-Source first (Ollama, Qwen, Llama), but fully compatible with Anthropic and OpenAI APIs via native Token Reduction systems to keep costs low.
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
| Execution | Rust (tokio, async) | Tool registry, sandbox, permissions, squad orchestration |
| Memory | Python (ChromaDB, SQLite) | Mempalace, semantic search, context compression |
| Bus | NATS | Component communication & real-time telemetry |
| LLM | Ollama (Local) / Anthropic & OpenAI | Local uncensored inference (Qwen/Mistral) + Optimized API integrations |
| TUI | Ratatui | Terminal dashboard with Swarm visualization |
| Web | React + Vite | Web dashboard |

## License

MIT — [@virgilhawkins00](https://github.com/virgilhawkins00)
