# 🧠 Cortex OS — Roadmap Completo

> Do esqueleto ao produto final. Cada fase tem entregáveis concretos e critérios de "done".

---

## Visão Final

Um **sistema operacional autônomo para IA** que:
- Recebe um objetivo em linguagem natural
- Planeja a execução (multi-step)
- Executa ferramentas em sandbox segura
- Persiste memória semântica entre sessões (nunca esquece)
- Se comunica via qualquer canal (Terminal, Web, WhatsApp, Discord, Telegram...)
- Roda 100% local (sem dependência de APIs externas) OU na nuvem (VM/GCP)
- É plug & play — `git clone && cargo build && docker compose up`

---

## 🟢 Fase 1 — Fundação (COMPLETA ✅)

**Objetivo:** Esqueleto compilando com ferramentas básicas funcionais.

- [x] Rust workspace com 3 crates (`cortex-core`, `cortex-cli`, `cortex-tui`)
- [x] Tool trait + registry (bash, file_read, file_write)
- [x] Sandbox com timeout e limites de output
- [x] Permission enforcement (ReadOnly / WriteWorkspace / Full)
- [x] NATS bus client com auth
- [x] CLI interativo (REPL) + modo daemon
- [x] TUI básica com Ratatui (layout, input, status bar)
- [x] Docker Compose (NATS + Ollama + ChromaDB)
- [x] README, .gitignore, .env.example

---

## 🟢 Fase 2 — Cérebro (Memória + LLM) ✅

**Objetivo:** O sistema lembra de tudo e pensa usando LLM local.

### cortex-memory (Python)
- [x] Palace storage engine (Wing → Room → Drawer) em SQLite + FTS5
- [x] ChromaDB backend para embeddings vetoriais
- [x] Hybrid search (BM25 full-text + cosine similarity vetorial) com RRF
- [x] Knowledge Graph temporal (Entity → Predicate → Entity com timestamps)
- [x] AAAK-style index compression para scanning rápido por LLM
- [x] NATS bridge (request/reply) para o Rust consumir
- [x] Ingest pipeline: texto → chunking → embedding → storage
- [x] Verbatim recall (nunca resumir, nunca parafrasear)

### Integração Ollama
- [x] Engine client em Python (ollama SDK)
- [x] Model router (fallback chain: local → qualquer modelo disponível)
- [x] Prompt templates (system prompt + memory injection + tool calling)
- [x] Streaming de respostas via NATS para CLI/TUI

### Testes
- [x] pytest com fixtures para palace operations (80 testes, 0 falhas)
- [ ] Benchmark: search < 200ms em 10K documentos
- [ ] Benchmark: ingest < 1s por documento

---

## 🟢 Fase 3 — Autonomia (Agent Loop) ✅

**Objetivo:** O sistema planeja e executa tarefas autônomas multi-step.

### Agent Core (Rust)
- [x] Agent loop: receive task → think → plan → execute tools → evaluate → iterate
- [x] Step tracking: histórico de pensamentos, ações e observações
- [x] Observation buffer: alimenta o LLM com o resultado das ferramentas
- [x] Context management: short-term memory persistente durante a tarefa
- [x] Max iterations safety: limite de passos para evitar loops infinitos

### Tool Expansion (Rust)
- [x] `web_search` — interface de busca (placeholder extensível)
- [x] `web_read` — fetch de URL com conversão HTML→Markdown/Texto
- [x] `file_tree` — mapeamento recursivo de workspace (ignore list nativa)

### Workflow Engine
- [x] Workflow runner: execução de sequências de ferramentas via YAML/Structs
- [x] Sequential execution: suporte a pipeline de automação

### CLI Integration
- [x] Comando `agent <goal>` para início de loop autônomo
- [x] Comando `tree` no REPL interativo
- [x] Status indicators: Thinking / Acting em tempo real

### Testes
- [x] Build limpo sem warnings (Cargo workspace)
- [x] Agent logic verification (Think-Act-Observe cycle)
- [x] Tool-to-Brain observation loop funcional

---

## 🟢 Fase 4 — Interfaces Completas ✅

**Objetivo:** UX premium no terminal e na web.

### TUI Dashboard (Rust — cortex-tui)
- [x] Multi-tab layout (Agents, Memory, Tools, Config)
- [x] Real-time log streaming via NATS (CortexBus)
- [x] Memory Explorer funcional com navegação de wings/rooms
- [x] Tool catalog com documentação e detalhes de execução
- [x] Status do sistema em tempo real (NATS, Brain, Memory Stats)

### Web Dashboard (React + Vite)
- [x] Design system completo (Cyberpunk, Glassmorphism, HSL colors)
- [x] Integração NATS-over-WebSocket (Porta 4223)
- [x] Dashboard Grid: Activity / System Insight / Real-time Memory Stats
- [x] Memory Explorer visual com grid de memórias
- [x] Tool Catalog interativo com visualização de ferramentas core

### Infra & Segurança
- [x] NATS WebSocket server enabled e exposed em Docker
- [x] Allowed origins configuradas para dev local

---

## 🟡 Fase 5 — Gateway Multi-Canal

**Objetivo:** O Cortex OS responde em qualquer canal de comunicação.

### Gateway Core
- [ ] Channel abstraction trait (send/receive messages)
- [ ] Inbound message → Agent Task pipeline
- [ ] Agent Result → Outbound message formatting
- [ ] Session management (um contexto por conversa)
- [ ] DM pairing/authentication (inspirado OpenClaw)

### Channels (prioridade)
- [ ] Discord bot (via serenity-rs)
- [ ] Telegram bot (via teloxide)
- [ ] WhatsApp (via Baileys/WWebJS bridge)
- [ ] Slack (via Slack API)
- [ ] WebChat embeddable

### Testes
- [ ] Mock channel adapter
- [ ] Integration test: message in → tool exec → response out

---

## 🟡 Fase 6 — Segurança & Hardening

**Objetivo:** Pronto para produção.

### Security
- [ ] mTLS entre componentes (NATS + internal services)
- [ ] Secrets vault (encrypted .env at rest)
- [ ] Audit log de todas as execuções de tools
- [ ] Rate limiting no gateway
- [ ] Input sanitization (prompt injection defense)
- [ ] Container sandbox (Docker) para execução untrusted
- [ ] RBAC: definir quem pode usar quais tools

### Reliability
- [ ] Health checks para todos os serviços
- [ ] Graceful shutdown em todos os binários
- [ ] Reconnect automático no NATS
- [ ] Circuit breaker no LLM client
- [ ] Metrics export (Prometheus)
- [ ] Structured logging (JSON para produção)

### Testes
- [ ] Penetration test: tentar path traversal, command injection
- [ ] Stress test: 100 tasks simultâneas
- [ ] Failover test: matar NATS/Ollama mid-task

---

## 🟡 Fase 7 — Deployment & Plug-and-Play

**Objetivo:** Qualquer pessoa instala em 2 minutos.

### Packaging
- [ ] `setup.sh` — one-liner que instala tudo (detect OS, install deps, build)
- [ ] Dockerfile multi-stage para binário Rust otimizado
- [ ] Docker Compose all-in-one (um `up` e tudo roda)
- [ ] GitHub Actions CI/CD (test → build → release)
- [ ] Release binaries (macOS arm64, macOS x86, Linux x86)
- [ ] Homebrew formula (macOS)
- [ ] AUR package (Arch Linux)

### Cloud Deploy
- [ ] Terraform/Pulumi para GCP Cloud Run
- [ ] VM startup script (one-click on any VPS)
- [ ] Tailscale VPN auto-setup para acesso remoto seguro

### Documentation
- [ ] User guide completo (instalação, configuração, uso)
- [ ] Developer guide (como criar tools, channels, plugins)
- [ ] Architecture Decision Records (ADRs)
- [ ] Video walkthrough

---

## 🔮 Fase 8 — Ecossistema (Futuro)

**Objetivo:** Cortex OS como plataforma.

- [ ] Plugin system (tools como crates/packages externos)
- [ ] Skill marketplace (compartilhar workflows entre usuários)
- [ ] Multi-agent coordination (múltiplos Cortex conversando entre si)
- [ ] Voice interface (STT + TTS nativo)
- [ ] Mobile companion app (React Native)
- [ ] Integração ViaOptima (monitoramento logístico via Cortex)
- [ ] Integração Polymarket (trading bot como plugin)
- [ ] Self-improvement: o sistema sugere otimizações no próprio código

---

## Timeline Estimada

| Fase | Duração | Dependências |
|---|---|---|
| 1. Fundação | ✅ Completa | — |
| 2. Cérebro | 1-2 semanas | Fase 1 |
| 3. Autonomia | 2-3 semanas | Fase 2 |
| 4. Interfaces | 1-2 semanas | Fase 3 |
| 5. Gateway | 1-2 semanas | Fase 3 |
| 6. Segurança | 1 semana | Fase 3-5 |
| 7. Deploy | 1 semana | Fase 6 |
| 8. Ecossistema | Ongoing | Fase 7 |

**Total estimado até produto funcional (Fase 5): ~6-9 semanas**
**Total até production-ready (Fase 7): ~8-12 semanas**

---

*Última atualização: 2026-04-17*
*Autor: @virgilhawkins00*
