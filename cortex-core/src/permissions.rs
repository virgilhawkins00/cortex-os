use std::path::Path;

/// Permission levels for tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Can read files, run non-destructive commands.
    ReadOnly,
    /// Can write files within workspace boundaries.
    WriteWorkspace,
    /// Full access — bash, network, filesystem.
    Full,
}

/// Policy engine that decides whether a tool invocation is allowed.
pub struct PermissionPolicy {
    level: Permission,
    workspace_root: String,
}

impl PermissionPolicy {
    #[must_use]
    pub fn new(level: Permission, workspace_root: &str) -> Self {
        Self {
            level,
            workspace_root: workspace_root.to_string(),
        }
    }

    pub fn full() -> Self {
        Self::new(Permission::Full, ".")
    }

    /// Check if a file write is allowed under current policy.
    #[must_use]
    pub fn can_write(&self, path: &str) -> bool {
        match self.level {
            Permission::ReadOnly => false,
            Permission::WriteWorkspace => {
                // Resolve and check that path is within workspace
                let canonical = Path::new(path);
                let workspace = Path::new(&self.workspace_root);

                // Block obvious traversal attacks
                if path.contains("..") {
                    return false;
                }

                canonical.starts_with(workspace)
            }
            Permission::Full => true,
        }
    }

    /// Check if bash execution is allowed.
    #[must_use]
    pub fn can_exec_bash(&self) -> bool {
        matches!(self.level, Permission::Full | Permission::WriteWorkspace)
    }

    /// Check if a command is destructive (rm, chmod, sudo, etc.)
    #[must_use]
    pub fn is_destructive_command(cmd: &str) -> bool {
        let destructive = [
            "rm ", "rm -", "rmdir", "chmod", "chown", "sudo",
            "mkfs", "dd ", "shutdown", "reboot", "kill -9",
            "> /dev/", "format ",
        ];
        let lower = cmd.to_lowercase();
        destructive.iter().any(|d| lower.contains(d))
    }

    /// Full permission check for a bash command.
    #[must_use]
    pub fn check_bash(&self, cmd: &str) -> PermissionVerdict {
        if !self.can_exec_bash() {
            return PermissionVerdict::Denied("Bash execution not allowed in ReadOnly mode".into());
        }

        if self.level != Permission::Full && Self::is_destructive_command(cmd) {
            return PermissionVerdict::Denied(format!(
                "Destructive command blocked: '{cmd}'. Requires Full permission."
            ));
        }

        PermissionVerdict::Allowed
    }
}

#[derive(Debug, Clone)]
pub enum PermissionVerdict {
    Allowed,
    Denied(String),
}
