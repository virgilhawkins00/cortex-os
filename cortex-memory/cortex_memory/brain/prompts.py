"""Prompt Templates — System prompt builder with memory injection.

Constructs rich system prompts that give the LLM its identity,
inject relevant memories as context, and specify output format
for tool calling.
"""

from __future__ import annotations

from .._version import CORTEX_VERSION

_SYSTEM_IDENTITY = f"""You are Cortex, an autonomous AI operating system (v{CORTEX_VERSION}).

You are a highly capable AI assistant that can execute tools, manage files,
run commands, search the web, and maintain a persistent memory across sessions.
You think step-by-step, plan before acting, and verify your results.

Core principles:
- You NEVER fabricate information. If you don't know, you say so.
- You recall memories from your palace storage to provide contextual answers.
- You execute tools when actions are needed, always checking permissions first.
- You maintain verbatim records — you never paraphrase what you've been told.
- You are honest about your limitations and uncertainties.
"""

_MEMORY_CONTEXT_HEADER = """
## Relevant Memories
The following memories were retrieved from your palace storage based on the current query.
Use them to inform your response, but cite them naturally:

"""

_TOOL_CALLING_FORMAT = """
## Tool Calling
When you need to execute a tool, respond with a JSON block:

```json
{
  "tool": "tool_name",
  "args": {"key": "value"},
  "reasoning": "Why I'm using this tool"
}
```

Available tools:
"""

_NO_TOOL_INSTRUCTION = """
If no tool is needed, respond directly with your answer in plain text.
Do NOT wrap regular responses in JSON.
"""


class SystemPromptBuilder:
    """Builds system prompts with identity, memory context, and tool instructions.

    Usage::

        builder = SystemPromptBuilder()
        prompt = builder.build(
            memories=["memory1 content", "memory2 content"],
            tools=["bash", "file_read", "file_write"],
            knowledge_graph="Cortex OS → uses → Rust\\nCortex OS → uses → Python",
        )
    """

    def build(
        self,
        memories: list[str] | None = None,
        tools: list[str] | None = None,
        knowledge_graph: str | None = None,
        extra_instructions: str | None = None,
    ) -> str:
        """Build the complete system prompt.

        Args:
            memories: Relevant memory contents to inject as context.
            tools: Available tool names.
            knowledge_graph: Pre-formatted knowledge graph context.
            extra_instructions: Additional task-specific instructions.
        """
        parts = [_SYSTEM_IDENTITY.strip()]

        # Inject memory context
        if memories:
            context = _MEMORY_CONTEXT_HEADER.strip() + "\n"
            for i, mem in enumerate(memories, 1):
                # Truncate very long memories for context window efficiency
                display = mem[:500] + "..." if len(mem) > 500 else mem
                context += f"\n[Memory {i}]\n{display}\n"
            parts.append(context)

        # Inject knowledge graph
        if knowledge_graph:
            parts.append(f"\n## Knowledge Graph\n{knowledge_graph}\n")

        # Tool calling instructions
        if tools:
            tool_section = _TOOL_CALLING_FORMAT.strip() + "\n"
            for tool in tools:
                tool_section += f"- `{tool}`\n"
            tool_section += _NO_TOOL_INSTRUCTION.strip()
            parts.append(tool_section)

        # Extra instructions
        if extra_instructions:
            parts.append(f"\n## Additional Instructions\n{extra_instructions}\n")

        return "\n\n".join(parts)

    def build_minimal(self) -> str:
        """Build a minimal system prompt without memory or tools."""
        return _SYSTEM_IDENTITY.strip()
