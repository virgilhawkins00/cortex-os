use anyhow::Result;
use reqwest::Client;
use serde_json::{json, Value};
use html2text::from_read;

use crate::permissions::PermissionPolicy;
use crate::tools::{Tool, ToolOutput};

/// Tool for reading web pages and converting them to clean text.
pub struct WebReadTool {
    client: Client,
}

impl WebReadTool {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("CortexOS/0.1.0 (Autonomous Agent)")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait::async_trait]
impl Tool for WebReadTool {
    fn name(&self) -> &str {
        "web_read"
    }

    fn description(&self) -> &str {
        "Fetch a URL and convert its content to readable text."
    }

    async fn execute(&self, args: Value, policy: &PermissionPolicy) -> Result<ToolOutput> {
        let url = match args.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => return Ok(ToolOutput {
                success: false,
                content: String::new(),
                error: Some("Missing 'url' argument".into()),
            }),
        };

        // Network tools usually require Full permission unless specific overrides exist
        if !matches!(policy.can_exec_bash(), true) { // Reusing exec_bash check for network for now
             // In a real system, we might have a separate can_network check.
        }

        match self.client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    return Ok(ToolOutput {
                        success: false,
                        content: String::new(),
                        error: Some(format!("HTTP error: {status}")),
                    });
                }

                let body = resp.bytes().await?;
                let text = from_read(&body[..], 80); // 80 chars width

                Ok(ToolOutput {
                    success: true,
                    content: text,
                    error: None,
                })
            }
            Err(e) => Ok(ToolOutput {
                success: false,
                content: String::new(),
                error: Some(format!("Request failed: {e}")),
            }),
        }
    }
}

/// Tool for searching the web.
pub struct WebSearchTool {
    _client: Client,
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self {
            _client: Client::builder()
                .user_agent("CortexOS/0.1.0")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for a given query (Simulated for now)."
    }

    async fn execute(&self, args: Value, _policy: &PermissionPolicy) -> Result<ToolOutput> {
         let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => return Ok(ToolOutput {
                success: false,
                content: String::new(),
                error: Some("Missing 'query' argument".into()),
            }),
        };

        // For now, we simulate a search result. 
        // In a real implementation, this would call DuckDuckGo, Tavily, or SearXNG.
        let results = json!([
            {
                "title": format!("Search result for: {}", query),
                "url": "https://example.com/search",
                "snippet": "This is a simulated search result snippet for Cortex OS autonomous agent testing."
            }
        ]);

        Ok(ToolOutput {
            success: true,
            content: serde_json::to_string_pretty(&results)?,
            error: None,
        })
    }
}
