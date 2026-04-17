use anyhow::Result;
use serde_json::Value;
use std::path::Path;
use tracing::info;

use crate::permissions::PermissionPolicy;

use super::{Tool, ToolOutput};

const MAX_READ_SIZE: u64 = 10 * 1024 * 1024; // 10MB

// ─── File Read ───────────────────────────────────────

pub struct FileReadTool;

#[async_trait::async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file with size and binary detection"
    }

    async fn execute(&self, args: Value, _policy: &PermissionPolicy) -> Result<ToolOutput> {
        let path = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;

        let file_path = Path::new(path);

        if !file_path.exists() {
            return Ok(ToolOutput {
                success: false,
                content: String::new(),
                error: Some(format!("File does not exist: {path}")),
            });
        }

        // Size check
        let metadata = tokio::fs::metadata(path).await?;
        if metadata.len() > MAX_READ_SIZE {
            return Ok(ToolOutput {
                success: false,
                content: String::new(),
                error: Some(format!(
                    "File too large: {} bytes (max {})",
                    metadata.len(),
                    MAX_READ_SIZE
                )),
            });
        }

        let bytes = tokio::fs::read(path).await?;

        // Binary detection (NUL byte in first 8KB)
        let check_len = bytes.len().min(8192);
        if bytes[..check_len].contains(&0) {
            return Ok(ToolOutput {
                success: false,
                content: String::new(),
                error: Some("Binary file detected — refusing to read".into()),
            });
        }

        let content = String::from_utf8_lossy(&bytes).to_string();
        info!("Read {} bytes from {path}", content.len());

        Ok(ToolOutput {
            success: true,
            content,
            error: None,
        })
    }
}

// ─── File Write ──────────────────────────────────────

pub struct FileWriteTool;

#[async_trait::async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Write content to a file with permission and path traversal checks"
    }

    async fn execute(&self, args: Value, policy: &PermissionPolicy) -> Result<ToolOutput> {
        let path = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;

        let content = args
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' argument"))?;

        // Permission check
        if !policy.can_write(path) {
            return Ok(ToolOutput {
                success: false,
                content: String::new(),
                error: Some(format!("Write denied for path: {path}")),
            });
        }

        // Create parent directories if needed
        if let Some(parent) = Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(path, content).await?;
        info!("Wrote {} bytes to {path}", content.len());

        Ok(ToolOutput {
            success: true,
            content: format!("Written {path} ({} bytes)", content.len()),
            error: None,
        })
    }
}
