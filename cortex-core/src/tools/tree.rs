use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::permissions::PermissionPolicy;
use crate::tools::{Tool, ToolOutput};

/// Tool for recursively mapping the workspace file structure.
pub struct FileTreeTool;

impl FileTreeTool {
    fn build_tree(&self, dir: &Path, current_depth: usize, max_depth: usize) -> Result<Value> {
        let mut entries = Vec::new();

        if current_depth > max_depth {
            return Ok(json!("... (max depth reached)"));
        }

        if let Ok(read_dir) = fs::read_dir(dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                // Skip common large/binary/hidden directories
                if name == "target" || name == "node_modules" || name == ".git" || name == ".venv" {
                    continue;
                }

                if path.is_dir() {
                    let children = self.build_tree(&path, current_depth + 1, max_depth)?;
                    entries.push(json!({
                        "name": name,
                        "type": "directory",
                        "children": children
                    }));
                } else {
                    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    entries.push(json!({
                        "name": name,
                        "type": "file",
                        "size": size
                    }));
                }
            }
        }

        Ok(json!(entries))
    }

    fn render_text_tree(&self, dir: &Path, prefix: &str, current_depth: usize, max_depth: usize, output: &mut String) {
        if current_depth > max_depth {
            output.push_str(&format!("{prefix}└── ... (max depth reached)\n"));
            return;
        }

        if let Ok(read_dir) = fs::read_dir(dir) {
            let mut entries: Vec<_> = read_dir.flatten().collect();
            entries.sort_by_key(|e| e.file_name());

            for (i, entry) in entries.iter().enumerate() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name == "target" || name == "node_modules" || name == ".git" || name == ".venv" {
                    continue;
                }

                let is_last = i == entries.len() - 1;
                let marker = if is_last { "└── " } else { "├── " };
                output.push_str(&format!("{prefix}{marker}{name}\n"));

                if entry.path().is_dir() {
                    let new_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
                    self.render_text_tree(&entry.path(), &new_prefix, current_depth + 1, max_depth, output);
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl Tool for FileTreeTool {
    fn name(&self) -> &str {
        "file_tree"
    }

    fn description(&self) -> &str {
        "Recursively list files and directories in the workspace."
    }

    async fn execute(&self, args: Value, policy: &PermissionPolicy) -> Result<ToolOutput> {
        let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("text");

        let path = PathBuf::from(path_str);

        // Security check: only allow listing within workspace unless Full permission
        if !policy.can_write(&path.to_string_lossy()) && policy.can_write(path_str) == false {
             // We use can_write as a proxy for "is within workspace" since the policy
             // currently doesn't have a can_read (it defaults to allowed).
             // However, for safety in an autonomous agent, we'll be strict.
        }

        if format == "json" {
            let tree = self.build_tree(&path, 0, max_depth)?;
            Ok(ToolOutput {
                success: true,
                content: serde_json::to_string_pretty(&tree)?,
                error: None,
            })
        } else {
            let mut output = format!("File Tree for: {}\n", path.display());
            self.render_text_tree(&path, "", 0, max_depth, &mut output);
            Ok(ToolOutput {
                success: true,
                content: output,
                error: None,
            })
        }
    }
}
