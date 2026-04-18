use anyhow::Result;
use clap::Parser;
use cortex_core::nats_bus::{
    BrainThinkRequest, CortexBus, MemoryIngestRequest, MemorySearchRequest, TaskRequest,
    TaskResult, TaskStatus,
};
use cortex_core::permissions::{Permission, PermissionPolicy};
use cortex_core::sandbox::Sandbox;
use cortex_core::tools::ToolRegistry;
use futures::StreamExt;
use serde_json::json;
use std::io::{self, Write};
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "cortex", about = "Cortex OS — Autonomous AI Runtime", version)]
struct Cli {
    /// NATS server URL
    #[arg(long, default_value = "nats://127.0.0.1:4222")]
    nats_url: String,

    /// NATS auth token
    #[arg(long)]
    nats_token: Option<String>,

    /// Permission level (readonly, workspace, full)
    #[arg(long, default_value = "full")]
    permission: String,

    /// Run in daemon mode (listen for NATS tasks instead of interactive prompt)
    #[arg(long)]
    daemon: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    let perm = match cli.permission.as_str() {
        "readonly" => Permission::ReadOnly,
        "workspace" => Permission::WriteWorkspace,
        _ => Permission::Full,
    };

    let cwd = std::env::current_dir()?.to_string_lossy().to_string();
    let policy = PermissionPolicy::new(perm, &cwd);
    let sandbox = Sandbox::default();
    let registry = ToolRegistry::with_defaults(sandbox);

    println!();
    println!("  ██████╗ ██████╗ ██████╗ ████████╗███████╗██╗  ██╗");
    println!("  ██╔════╝██╔═══██╗██╔══██╗╚══██╔══╝██╔════╝╚██╗██╔╝");
    println!("  ██║     ██║   ██║██████╔╝   ██║   █████╗   ╚███╔╝ ");
    println!("  ██║     ██║   ██║██╔══██╗   ██║   ██╔══╝   ██╔██╗ ");
    println!("  ╚██████╗╚██████╔╝██║  ██║   ██║   ███████╗██╔╝ ██╗");
    println!("   ╚═════╝ ╚═════╝ ╚═╝  ╚═╝   ╚═╝   ╚══════╝╚═╝  ╚═╝");
    println!("                          OS v0.1.0");
    println!("  ─────────────────────────────────────────────────");
    println!("  Tools: {:?}", registry.list());
    println!("  Permission: {} | Workspace: {cwd}", cli.permission);
    println!();

    if cli.daemon {
        run_daemon(&cli.nats_url, cli.nats_token.as_deref(), &registry, &policy).await?;
    } else {
        run_interactive(&cli.nats_url, cli.nats_token.as_deref(), &registry, &policy).await?;
    }

    Ok(())
}

/// Interactive REPL mode — direct tool execution + memory/brain commands.
async fn run_interactive(
    nats_url: &str,
    nats_token: Option<&str>,
    registry: &ToolRegistry,
    policy: &PermissionPolicy,
) -> Result<()> {
    // Try connecting to NATS for memory/brain features
    let bus = match CortexBus::connect(nats_url, nats_token).await {
        Ok(bus) => {
            println!("  NATS: connected ✓");
            // Check brain health
            match bus.brain_health().await {
                Ok(result) if result.status == TaskStatus::Success => {
                    println!("  Brain: online ✓");
                }
                _ => {
                    println!("  Brain: offline (start cortex-memory for LLM features)");
                }
            }
            Some(bus)
        }
        Err(_) => {
            println!("  NATS: offline (memory/brain commands unavailable)");
            None
        }
    };

    println!();
    println!("  Commands:");
    println!("    bash <cmd>             — Execute a shell command");
    println!("    file_read <path>       — Read a file");
    println!("    file_write <path> <txt>— Write to a file");
    println!("    think <prompt>         — Ask the LLM (with memory context)");
    println!("    remember <text>        — Store a memory");
    println!("    recall <query>         — Search memories");
    println!("    agent <goal>           — Start an autonomous task loop");
    println!("    tree [path]            — Show file structure");
    println!("    tools                  — List available tools");
    println!("    exit                   — Quit");
    println!();

    loop {
        print!("  cortex > ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }
        if input == "exit" || input == "quit" {
            println!("  Shutting down Cortex OS.");
            break;
        }

        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let cmd = parts[0];
        let arg_str = parts.get(1).unwrap_or(&"");

        match cmd {
            // ── Tool commands ────────────────────────
            "bash" => {
                let args = json!({ "command": arg_str });
                execute_tool(registry, "bash", args, policy).await;
            }
            "file_read" => {
                let args = json!({ "path": arg_str });
                execute_tool(registry, "file_read", args, policy).await;
            }
            "file_write" => {
                let file_parts: Vec<&str> = arg_str.splitn(2, ' ').collect();
                if file_parts.len() < 2 {
                    println!("  Usage: file_write <path> <content>");
                    continue;
                }
                let args = json!({ "path": file_parts[0], "content": file_parts[1] });
                execute_tool(registry, "file_write", args, policy).await;
            }
            "tools" => {
                println!("  Available tools: {:?}", registry.list());
            }
            "tree" => {
                let args = json!({ "path": if arg_str.is_empty() { "." } else { arg_str } });
                execute_tool(registry, "file_tree", args, policy).await;
            }

            // ── Brain commands (require NATS + cortex-memory) ────
            "think" => {
                if arg_str.is_empty() {
                    println!("  Usage: think <prompt>");
                    continue;
                }
                if let Some(ref bus) = bus {
                    handle_think(bus, arg_str).await;
                } else {
                    println!("  [OFFLINE] NATS not connected. Start NATS + cortex-memory first.");
                }
            }
            "remember" => {
                if arg_str.is_empty() {
                    println!("  Usage: remember <text to store>");
                    continue;
                }
                if let Some(ref bus) = bus {
                    handle_remember(bus, arg_str).await;
                } else {
                    println!("  [OFFLINE] NATS not connected.");
                }
            }
            "recall" => {
                if arg_str.is_empty() {
                    println!("  Usage: recall <search query>");
                    continue;
                }
                if let Some(ref bus) = bus {
                    handle_recall(bus, arg_str).await;
                } else {
                    println!("  [OFFLINE] NATS not connected.");
                }
            }
            "agent" => {
                if arg_str.is_empty() {
                    println!("  Usage: agent <goal>");
                    continue;
                }
                if let Some(ref bus) = bus {
                    handle_agent(bus, registry, policy, arg_str).await;
                } else {
                    println!("  [OFFLINE] NATS not connected.");
                }
            }

            _ => {
                println!("  Unknown command: {cmd}. Type 'tools' to list available.");
            }
        }
    }

    Ok(())
}

/// Execute a tool and print the result.
async fn execute_tool(
    registry: &ToolRegistry,
    name: &str,
    args: serde_json::Value,
    policy: &PermissionPolicy,
) {
    match registry.execute(name, args, policy).await {
        Ok(output) => {
            if output.success {
                println!("{}", output.content);
            } else if let Some(err) = &output.error {
                println!("  [ERROR] {err}");
            }
        }
        Err(e) => error!("Tool execution failed: {e}"),
    }
}

/// Handle the `think` command — send prompt to LLM via NATS.
async fn handle_think(bus: &CortexBus, prompt: &str) {
    println!("  🧠 Thinking...\n");

    let req = BrainThinkRequest {
        prompt: prompt.to_string(),
        model: None,
        include_memory: true,
        stream: false,
    };

    match bus.brain_think(&req).await {
        Ok(result) => {
            if result.status == TaskStatus::Success {
                // Parse the JSON output to get the response text
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result.output) {
                    if let Some(response) = parsed.get("response").and_then(|v| v.as_str()) {
                        println!("  {response}");
                    } else {
                        println!("  {}", result.output);
                    }
                    if let Some(model) = parsed.get("model").and_then(|v| v.as_str()) {
                        println!("\n  ─── model: {model} ───");
                    }
                } else {
                    println!("  {}", result.output);
                }
            } else if let Some(err) = &result.error {
                println!("  [ERROR] {err}");
            }
        }
        Err(e) => {
            warn!("Brain request failed: {e}");
            println!("  [ERROR] Brain unavailable: {e}");
        }
    }
}

/// Handle the `remember` command — store a memory via NATS.
async fn handle_remember(bus: &CortexBus, text: &str) {
    let req = MemoryIngestRequest {
        text: text.to_string(),
        wing: "conversations".to_string(),
        room: "repl".to_string(),
        metadata: None,
    };

    match bus.memory_ingest(&req).await {
        Ok(result) => {
            if result.status == TaskStatus::Success {
                println!("  ✓ Memory stored.");
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result.output) {
                    if let Some(count) = parsed.get("ingested") {
                        println!("  ({count} chunk(s) ingested)");
                    }
                }
            } else if let Some(err) = &result.error {
                println!("  [ERROR] {err}");
            }
        }
        Err(e) => {
            println!("  [ERROR] Memory service unavailable: {e}");
        }
    }
}

/// Handle the `recall` command — search memories via NATS.
async fn handle_recall(bus: &CortexBus, query: &str) {
    let req = MemorySearchRequest {
        query: query.to_string(),
        top_k: 5,
        wing: None,
    };

    match bus.memory_search(&req).await {
        Ok(result) => {
            if result.status == TaskStatus::Success {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result.output) {
                    if let Some(results) = parsed.get("results").and_then(|v| v.as_array()) {
                        if results.is_empty() {
                            println!("  No memories found for: \"{query}\"");
                        } else {
                            println!("  Found {} memories:\n", results.len());
                            for (i, r) in results.iter().enumerate() {
                                let content = r
                                    .get("content")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("???");
                                let score = r
                                    .get("score")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                // Truncate for display
                                let display = if content.len() > 120 {
                                    format!("{}...", &content[..120])
                                } else {
                                    content.to_string()
                                };
                                println!("  [{:}] (score: {:.3}) {display}", i + 1, score);
                            }
                        }
                    } else {
                        println!("  {}", result.output);
                    }
                } else {
                    println!("  {}", result.output);
                }
            } else if let Some(err) = &result.error {
                println!("  [ERROR] {err}");
            }
        }
        Err(e) => {
            println!("  [ERROR] Memory service unavailable: {e}");
        }
    }
}

/// Daemon mode — listen for tasks on NATS and execute them.
async fn run_daemon(
    nats_url: &str,
    token: Option<&str>,
    registry: &ToolRegistry,
    policy: &PermissionPolicy,
) -> Result<()> {
    let bus = CortexBus::connect(nats_url, token).await?;
    let mut sub = bus.subscribe("cortex.task").await?;

    info!("Daemon mode: listening on 'cortex.task'...");

    while let Some(msg) = sub.next().await {
        let req: TaskRequest = match serde_json::from_slice(&msg.payload) {
            Ok(r) => r,
            Err(e) => {
                error!("Invalid task payload: {e}");
                continue;
            }
        };

        info!("Received task {}: tool={:?}", req.id, req.tool);

        let tool_name = req.tool.as_deref().unwrap_or("bash");
        let args = req
            .args
            .unwrap_or_else(|| json!({ "command": req.prompt }));

        let result = match registry.execute(tool_name, args, policy).await {
            Ok(output) => TaskResult {
                id: req.id,
                status: if output.success {
                    cortex_core::nats_bus::TaskStatus::Success
                } else {
                    cortex_core::nats_bus::TaskStatus::Error
                },
                output: output.content,
                error: output.error,
            },
            Err(e) => TaskResult {
                id: req.id,
                status: cortex_core::nats_bus::TaskStatus::Error,
                output: String::new(),
                error: Some(e.to_string()),
            },
        };

        if let Err(e) = bus.publish_result("cortex.result", &result).await {
            error!("Failed to publish result: {e}");
        }
    }

    Ok(())
}

/// Handle the `agent` command — run the autonomous Think-Act-Observe loop.
async fn handle_agent(
    bus: &CortexBus,
    registry: &ToolRegistry,
    policy: &PermissionPolicy,
    goal: &str,
) {
    println!("  🤖 Agent starting goal: \"{goal}\"");
    println!("  ─────────────────────────────────────────────────");

    let agent = cortex_core::agent::Agent::new(bus, registry, policy);

    match agent.run(goal).await {
        Ok(result) => {
            println!("\n  ✨ Goal achieved!");
            println!("  ─────────────────────────────────────────────────");
            println!("  Final Answer:\n");
            println!("  {}", result.final_answer);
            println!("\n  Steps taken: {}", result.steps.len());
        }
        Err(e) => {
            println!("\n  [ERROR] Agent failed: {e}");
        }
    }
}
