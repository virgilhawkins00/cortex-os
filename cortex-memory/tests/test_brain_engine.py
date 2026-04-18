"""Tests for Brain Engine — mocked Ollama, tool call parsing, memory context."""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from cortex_memory.brain.engine import BrainEngine
from cortex_memory.brain.model_router import ModelRouter
from cortex_memory.brain.ollama_client import OllamaClient
from cortex_memory.brain.prompts import SystemPromptBuilder


# ── Prompt Builder ─────────────────────────────────────────


def test_prompt_builder_minimal() -> None:
    builder = SystemPromptBuilder()
    prompt = builder.build_minimal()
    assert "Cortex" in prompt
    assert len(prompt) > 100


def test_prompt_builder_with_memories() -> None:
    builder = SystemPromptBuilder()
    prompt = builder.build(memories=["Memory 1 content", "Memory 2 content"])
    assert "Memory 1 content" in prompt
    assert "Memory 2 content" in prompt
    assert "Relevant Memories" in prompt


def test_prompt_builder_with_tools() -> None:
    builder = SystemPromptBuilder()
    prompt = builder.build(tools=["bash", "file_read", "file_write"])
    assert "bash" in prompt
    assert "file_read" in prompt
    assert "Tool Calling" in prompt


def test_prompt_builder_truncates_long_memories() -> None:
    """Memories over 500 chars should be truncated."""
    builder = SystemPromptBuilder()
    long_memory = "x" * 1000
    prompt = builder.build(memories=[long_memory])
    # The truncated version should be in the prompt (500 chars + "...")
    assert "..." in prompt


def test_prompt_builder_with_knowledge_graph() -> None:
    builder = SystemPromptBuilder()
    kg_text = "• Cortex OS → uses → Rust (2026-04-17)"
    prompt = builder.build(knowledge_graph=kg_text)
    assert "Cortex OS" in prompt
    assert "Knowledge Graph" in prompt


# ── Tool Call Parsing ──────────────────────────────────────


def test_parse_tool_call_json_block() -> None:
    text = '''I'll run bash to check the directory.

```json
{
  "tool": "bash",
  "args": {"command": "ls -la"},
  "reasoning": "Need to see directory contents"
}
```
'''
    result = BrainEngine._parse_tool_call(text)
    assert result is not None
    assert result["tool"] == "bash"
    assert result["args"]["command"] == "ls -la"
    assert result["reasoning"] == "Need to see directory contents"


def test_parse_tool_call_no_tool() -> None:
    text = "The answer is 42. No tools needed."
    result = BrainEngine._parse_tool_call(text)
    assert result is None


def test_parse_tool_call_malformed_json() -> None:
    text = '```json\n{"tool": "bash", malformed\n```'
    result = BrainEngine._parse_tool_call(text)
    assert result is None


def test_parse_tool_call_no_json_block() -> None:
    text = '{"some_other_key": "value"}'
    result = BrainEngine._parse_tool_call(text)
    assert result is None


# ── Model Router ───────────────────────────────────────────


async def test_model_router_preferred_first() -> None:
    client = MagicMock(spec=OllamaClient)
    client.list_models = AsyncMock(return_value=["dolphin-mistral:latest", "llama3:latest"])
    router = ModelRouter(client, preferred_model="dolphin-mistral:latest")
    await router.initialize()

    model = router.route("default")
    assert model == "dolphin-mistral:latest"


async def test_model_router_fallback() -> None:
    client = MagicMock(spec=OllamaClient)
    # Only llama3 available
    client.list_models = AsyncMock(return_value=["llama3:latest"])
    router = ModelRouter(client, preferred_model="dolphin-mistral:latest")
    await router.initialize()

    model = router.route("default")
    assert model == "llama3:latest"  # Fallback chain finds llama3


async def test_model_router_no_models() -> None:
    client = MagicMock(spec=OllamaClient)
    client.list_models = AsyncMock(return_value=[])
    router = ModelRouter(client)
    await router.initialize()

    model = router.route()
    assert model is None


# ── Brain Engine ───────────────────────────────────────────


def _make_engine(response_text: str = "I am Cortex, ready to help.") -> BrainEngine:
    """Create a BrainEngine with fully mocked dependencies."""
    ollama = MagicMock(spec=OllamaClient)
    ollama.generate = AsyncMock(return_value=response_text)
    ollama.is_available = AsyncMock(return_value=True)
    ollama.list_models = AsyncMock(return_value=["dolphin-mistral:latest"])

    router = MagicMock(spec=ModelRouter)
    router.route = MagicMock(return_value="dolphin-mistral:latest")

    return BrainEngine(ollama=ollama, router=router, tools=["bash", "file_read"])


async def test_brain_think_returns_response() -> None:
    engine = _make_engine("The Cortex OS is an autonomous AI runtime.")
    result = await engine.think("What is Cortex OS?")

    assert result["response"] == "The Cortex OS is an autonomous AI runtime."
    assert result["model"] == "dolphin-mistral:latest"
    assert result["memories_used"] == 0


async def test_brain_think_no_model_available() -> None:
    ollama = MagicMock(spec=OllamaClient)
    router = MagicMock(spec=ModelRouter)
    router.route = MagicMock(return_value=None)

    engine = BrainEngine(ollama=ollama, router=router)
    result = await engine.think("Hello")

    assert "No LLM model available" in result["response"]
    assert result["tool_call"] is None


async def test_brain_think_with_memory_search() -> None:
    """Engine should call search and inject memories into context."""
    engine = _make_engine("Response with memory context.")

    # Mock search engine
    search = MagicMock()
    search_result = MagicMock()
    search_result.memory = MagicMock()
    search_result.memory.content = "Relevant memory content"
    search.search = AsyncMock(return_value=[search_result])
    engine._search = search

    result = await engine.think("Tell me about the project", include_memory=True)
    assert result["memories_used"] == 1


async def test_brain_think_memory_search_failure_graceful() -> None:
    """If memory search fails, engine should still respond."""
    engine = _make_engine("Fallback response without memory.")

    search = MagicMock()
    search.search = AsyncMock(side_effect=RuntimeError("DB connection failed"))
    engine._search = search

    # Should not raise — just skip memory injection
    result = await engine.think("What is Rust?", include_memory=True)
    assert result["response"] == "Fallback response without memory."
    assert result["memories_used"] == 0


async def test_brain_think_detects_tool_call() -> None:
    tool_response = '''I'll list the files.

```json
{
  "tool": "bash",
  "args": {"command": "ls -la"},
  "reasoning": "User wants to see files"
}
```'''
    engine = _make_engine(tool_response)
    result = await engine.think("List files in the current directory")

    assert result["tool_call"] is not None
    assert result["tool_call"]["tool"] == "bash"


async def test_brain_health_check() -> None:
    engine = _make_engine()
    health = await engine.health_check()

    assert "ollama" in health
    assert "models" in health
    assert "selected_model" in health
