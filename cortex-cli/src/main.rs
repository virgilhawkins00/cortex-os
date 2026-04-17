use anyhow::Result;
use clap::Parser;
use cortex_core::nats_bus::{CortexBus, TaskRequest, TaskResult};
use cortex_core::permissions::{Permission, PermissionPolicy};
use cortex_core::sandbox::Sandbox;
use cortex_core::tools::ToolRegistry;
use futures::StreamExt;
use serde_json::json;
use std::io::{self, Write};
use tracing::{error, info};

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
        run_interactive(&registry, &policy).await?;
    }

    Ok(())
}

/// Interactive REPL mode — direct tool execution.
async fn run_interactive(registry: &ToolRegistry, policy: &PermissionPolicy) -> Result<()> {
    println!("  Type a tool command: bash <cmd> | file_read <path> | file_write <path> <content>");
    println!("  Type 'exit' to quit.\n");

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
        let tool_name = parts[0];
        let arg_str = parts.get(1).unwrap_or(&"");

        let args = match tool_name {
            "bash" => json!({ "command": arg_str }),
            "file_read" => json!({ "path": arg_str }),
            "file_write" => {
                let file_parts: Vec<&str> = arg_str.splitn(2, ' ').collect();
                if file_parts.len() < 2 {
                    println!("  Usage: file_write <path> <content>");
                    continue;
                }
                json!({ "path": file_parts[0], "content": file_parts[1] })
            }
            "tools" => {
                println!("  Available tools: {:?}", registry.list());
                continue;
            }
            _ => {
                println!("  Unknown tool: {tool_name}. Type 'tools' to list available.");
                continue;
            }
        };

        match registry.execute(tool_name, args, policy).await {
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

    Ok(())
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
