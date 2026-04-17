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

## 🟡 Fase 2 — Cérebro (Memória + LLM)

**Objetivo:** O sistema lembra de tudo e pensa usando LLM local.

### cortex-memory (Python)
- [ ] Palace storage engine (Wing → Room → Drawer) em SQLite
- [ ] ChromaDB backend para embeddings vetoriais
- [ ] Hybrid search (BM25 full-text + cosine similarity vetorial)
- [ ] Knowledge Graph temporal (Entity → Predicate → Entity com timestamps)
- [ ] AAAK-style index compression para scanning rápido por LLM
- [ ] MCP Server (ou gRPC) para o Rust consumir via NATS
- [ ] Ingest pipeline: texto → chunking → embedding → storage
- [ ] Verbatim recall (nunca resumir, nunca parafrasear)

### Integração Ollama
- [ ] Engine client em Python (ollama SDK)
- [ ] Model router (fallback chain: local → API)
- [ ] Prompt templates (system prompt + memory injection + tool calling)
- [ ] Streaming de respostas via NATS para CLI/TUI

### Testes
- [ ] pytest com fixtures para palace operations
- [ ] Benchmark: search < 200ms em 10K documentos
- [ ] Benchmark: ingest < 1s por documento

---

## 🟡 Fase 3 — Autonomia (Agent Loop)

**Objetivo:** O sistema planeja e executa tarefas autônomas multi-step.

### Agent Core (Rust)
- [ ] Agent loop: receive task → think → plan → execute tools → evaluate → iterate
- [ ] Step planner com DAG de dependências
- [ ] Observation buffer (armazena output de cada tool para contexto)
- [ ] Auto-correction: se uma tool falha, retry com ajuste
- [ ] Max iterations safety (prevent runaway loops)
- [ ] Token budget tracking

### Tool Expansion (Rust)
- [ ] `web_search` — scraping de busca via serper/searxng
- [ ] `web_read` — fetch de URL com HTML→Markdown
- [ ] `code_edit` — edição cirúrgica de arquivos (find & replace, AST-aware)
- [ ] `git` — commit, push, branch, diff nativo
- [ ] `project_search` — grep/ripgrep com contexto semântico
- [ ] `image_generate` — via API (stable diffusion / dall-e fallback)

### Workflow Engine
- [ ] Pipeline definido em YAML ou JSON
- [ ] Parallel execution ($team mode inspirado no oh-my-codex)
- [ ] Sequential execution com gates (resultado anterior condiciona próximo)
- [ ] Deep-interview mode: sistema faz perguntas de clarificação antes de agir

### Testes
- [ ] Test harness: mock LLM + deterministic tool outputs
- [ ] Integration test: "create a hello world project" end-to-end
- [ ] Chaos test: random tool failures, verify graceful recovery

---

## 🟡 Fase 4 — Interfaces Completas

**Objetivo:** UX premium no terminal e na web.

### TUI Dashboard (Rust — cortex-tui)
- [ ] Multi-tab layout (Agents, Memory, Tools, Config)
- [ ] Real-time log streaming via NATS
- [ ] Agent status com spinners animados
- [ ] Memory browser (navegar Wings/Rooms/Drawers visualmente)
- [ ] Tool execution panel com syntax highlighting
- [ ] Themes (dark/light/cyberpunk/solarized)
- [ ] Keybindings configuráveis

### Web Dashboard (React + Vite)
- [ ] Design system completo (tokens, components, layouts)
- [ ] Chat interface com streaming de respostas
- [ ] Agent execution timeline (visualização do DAG)
- [ ] Memory explorer (busca semântica visual)
- [ ] Tool catalog com docs inline
- [ ] Settings panel (LLM model, permissions, theme)
- [ ] WebSocket real-time via NATS bridge
- [ ] Responsive (mobile-friendly)

### Testes
- [ ] TUI: snapshot tests com insta crate
- [ ] Web: Playwright E2E para fluxos críticos

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
