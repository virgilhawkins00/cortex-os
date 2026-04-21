# 🧠 Cortex OS

> **The Autonomous AI Operating System.**
> Uncensored. Sovereign. Plug & Play.

---

## What is Cortex OS?

Cortex OS is a **unified, from-scratch** autonomous AI runtime, designed as a superior, extensible alternative to tools like Claude Code and OpenDevin. It operates natively in your terminal with advanced multi-agent capabilities.

- **Sandboxed execution** of code and shell commands (Rust)
- **Native Mempalace Integration**: Persistent semantic memory with verbatim recall, dramatically reducing token context window bloat compared to standard chat platforms.
- **Multi-Agent Swarms (Squads)**: Define specialized squads (e.g., Financial Analysts or Full-Stack Engineering teams) that work in parallel on complex domains using modern stacks (JS/TS, Rust, Go, Python, C++).
- **Model Flexibility**: Built Open-Source first (Ollama, Qwen, Llama), but fully compatible with **Anthropic, OpenAI, Gemini, and Groq** APIs via native Token Reduction systems to keep costs low.
- **Secrets Vault**: AES-256-GCM encrypted environment — API keys never touch disk in plain text.
- **Circuit Breaker**: Self-healing health monitor that halts agents if LLM infrastructure goes down.
- **Beautiful interfaces** — both TUI (terminal) and Web dashboard

One binary. One `docker compose up`. Done.

## Architecture

```
cortex-os/
├── cortex-core/        # Rust lib — execution runtime, tools, sandbox, vault, health
├── cortex-cli/         # Rust bin — interactive REPL + NATS daemon + vault commands
├── cortex-tui/         # Rust bin — Ratatui terminal dashboard
├── cortex-gateway/     # Rust bin — multi-channel gateway (Discord, Telegram)
├── cortex-memory/      # Python — semantic memory, knowledge graph, LLM brain
├── agents/             # Agent configs, tools, and squad definitions
│   ├── architect/          # Software Architect agent
│   ├── software-engineer/  # Full-Stack Engineer agent (linter tools)
│   ├── devops/             # DevOps agent (Dockerfile validation)
│   ├── cyber-security/     # Security Auditor agent
│   ├── market-analyst/     # Technical Analyst (Binance ticker tools)
│   ├── fundamental-researcher/ # Macro Economist agent
│   ├── risk-manager/       # Capital allocation & risk management
│   └── squads/             # Squad orchestration definitions
├── web/                # React+Vite — web dashboard
├── docker-compose.yml  # NATS + Ollama + ChromaDB
├── setup.sh            # One-liner installer
├── Dockerfile          # Multi-stage production build
└── Cargo.toml          # Rust workspace root
```

## Quick Start

```bash
# Option 1: One-liner install
curl -sSL https://raw.githubusercontent.com/virgilhawkins00/cortex-os/main/setup.sh | bash

# Option 2: Manual
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

### Vault Setup (Recommended)

```bash
# Initialize encrypted vault for API keys
./target/release/cortex vault init

# Store secrets securely (AES-256-GCM encrypted)
./target/release/cortex vault set ANTHROPIC_API_KEY sk-ant-...
./target/release/cortex vault set OPENAI_API_KEY sk-...
./target/release/cortex vault set GEMINI_API_KEY AIza...

# On next boot, Cortex auto-detects .env.vault and prompts for master password
./target/release/cortex
# > Vault found. Enter Master Password to boot Cortex OS: ****
```

## Tech Stack

| Layer | Tech | Purpose |
|---|---|---|
| Execution | Rust (tokio, async) | Tool registry, sandbox, permissions, squad orchestration |
| Memory | Python (ChromaDB, SQLite) | Mempalace, semantic search, Caveman token compression |
| Bus | NATS | Component communication & real-time telemetry |
| LLM | Ollama / Anthropic / OpenAI / Gemini / Groq | Local uncensored inference + optimized paid API routing |
| Security | AES-256-GCM + PBKDF2 | Secrets Vault, mTLS, audit logging |
| Reliability | CancellationToken + Circuit Breaker | Graceful shutdown, health monitoring |
| TUI | Ratatui | Terminal dashboard with Swarm visualization |
| Web | React + Vite | Web dashboard |

## License

MIT — [@virgilhawkins00](https://github.com/virgilhawkins00)
