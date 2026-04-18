"""Integration tests for the NATS bridge.

These tests require a running NATS server. They are skipped automatically
if NATS is not available, to keep CI/CD clean on machines without Docker.

To run locally:
    docker compose up -d nats
    pytest tests/test_nats_bridge.py -v
"""

from __future__ import annotations

import json
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

import pytest

from cortex_memory.nats_bridge import NATSBridge, _ok_response, _err_response


# ── Response helpers ───────────────────────────────────────


def test_ok_response_format() -> None:
    payload = _ok_response({"memory_id": "abc", "content": "test"}, request_id="r1")
    parsed = json.loads(payload)
    assert parsed["id"] == "r1"
    assert parsed["status"] == "success"
    assert parsed["error"] is None
    assert "memory_id" in parsed["output"]


def test_err_response_format() -> None:
    payload = _err_response("Something went wrong", request_id="r2")
    parsed = json.loads(payload)
    assert parsed["id"] == "r2"
    assert parsed["status"] == "error"
    assert parsed["error"] == "Something went wrong"
    assert parsed["output"] == ""


def test_ok_response_auto_id() -> None:
    payload = _ok_response("result string")
    parsed = json.loads(payload)
    assert parsed["id"]  # Auto-generated — should be non-empty
    assert parsed["status"] == "success"


def test_ok_response_string_passthrough() -> None:
    """String outputs should be stored verbatim, not double-JSON-encoded."""
    payload = _ok_response("plain text response")
    parsed = json.loads(payload)
    assert parsed["output"] == "plain text response"


def test_ok_response_dict_json_encoded() -> None:
    """Dict outputs should be JSON-encoded in the output field."""
    payload = _ok_response({"key": "value"})
    parsed = json.loads(payload)
    inner = json.loads(parsed["output"])
    assert inner["key"] == "value"


# ── NATS Bridge handler unit tests (no actual NATS required) ──


def _make_bridge(tmp_path: Path) -> tuple[NATSBridge, MagicMock, MagicMock]:
    """Build a NATSBridge with all dependencies mocked."""
    from cortex_memory.config import NATSConfig
    from cortex_memory.palace.storage import PalaceStorage
    from cortex_memory.palace.search import HybridSearchEngine
    from cortex_memory.palace.ingest import IngestPipeline
    from cortex_memory.brain.engine import BrainEngine
    from cortex_memory.palace.knowledge_graph import KnowledgeGraph

    storage_mock = MagicMock(spec=PalaceStorage)
    search_mock = MagicMock(spec=HybridSearchEngine)
    ingest_mock = MagicMock(spec=IngestPipeline)
    brain_mock = MagicMock(spec=BrainEngine)
    kg_mock = MagicMock(spec=KnowledgeGraph)

    bridge = NATSBridge(
        config=NATSConfig(url="nats://127.0.0.1:4222", token=None),
        storage=storage_mock,
        search_engine=search_mock,
        ingest_pipeline=ingest_mock,
        brain_engine=brain_mock,
        knowledge_graph=kg_mock,
    )
    return bridge, storage_mock, brain_mock


async def test_handle_memory_search_missing_query(tmp_path: Path) -> None:
    bridge, _, _ = _make_bridge(tmp_path)

    msg = MagicMock()
    msg.data = json.dumps({}).encode()
    msg.respond = AsyncMock()

    await bridge._handle_memory_search(msg)

    msg.respond.assert_called_once()
    response = json.loads(msg.respond.call_args[0][0])
    assert response["status"] == "error"
    assert "query" in response["error"].lower()


async def test_handle_memory_ingest_missing_text(tmp_path: Path) -> None:
    bridge, _, _ = _make_bridge(tmp_path)

    msg = MagicMock()
    msg.data = json.dumps({"wing": "x", "room": "y"}).encode()
    msg.respond = AsyncMock()

    await bridge._handle_memory_ingest(msg)

    msg.respond.assert_called_once()
    response = json.loads(msg.respond.call_args[0][0])
    assert response["status"] == "error"


async def test_handle_brain_think_missing_prompt(tmp_path: Path) -> None:
    bridge, _, _ = _make_bridge(tmp_path)

    msg = MagicMock()
    msg.data = json.dumps({"include_memory": False}).encode()
    msg.respond = AsyncMock()

    await bridge._handle_brain_think(msg)

    msg.respond.assert_called_once()
    response = json.loads(msg.respond.call_args[0][0])
    assert response["status"] == "error"


async def test_handle_memory_search_calls_engine(tmp_path: Path) -> None:
    bridge, _, _ = _make_bridge(tmp_path)

    # Mock search to return empty results
    bridge._search.search = AsyncMock(return_value=[])

    msg = MagicMock()
    msg.data = json.dumps({"query": "test query", "top_k": 3}).encode()
    msg.respond = AsyncMock()

    await bridge._handle_memory_search(msg)

    bridge._search.search.assert_called_once_with("test query", wing=None, top_k=3)
    msg.respond.assert_called_once()
    response = json.loads(msg.respond.call_args[0][0])
    assert response["status"] == "success"


async def test_handle_brain_think_calls_brain(tmp_path: Path) -> None:
    bridge, _, brain_mock = _make_bridge(tmp_path)

    brain_mock.think = AsyncMock(return_value={
        "response": "Hello from the brain!",
        "model": "dolphin-mistral",
        "memories_used": 0,
        "tool_call": None,
    })

    msg = MagicMock()
    msg.data = json.dumps({"prompt": "Say hello"}).encode()
    msg.respond = AsyncMock()

    await bridge._handle_brain_think(msg)

    brain_mock.think.assert_called_once()
    call_kwargs = brain_mock.think.call_args
    assert call_kwargs.kwargs["prompt"] == "Say hello"

    response = json.loads(msg.respond.call_args[0][0])
    assert response["status"] == "success"
    inner = json.loads(response["output"])
    assert inner["response"] == "Hello from the brain!"


async def test_handle_brain_health(tmp_path: Path) -> None:
    bridge, storage_mock, brain_mock = _make_bridge(tmp_path)

    brain_mock.health_check = AsyncMock(return_value={
        "ollama": True,
        "models": ["dolphin-mistral:latest"],
        "selected_model": "dolphin-mistral:latest",
        "memory_search": True,
        "knowledge_graph": True,
    })
    storage_mock.count_memories = AsyncMock(return_value=42)
    bridge._kg.count_triples = AsyncMock(return_value=15)

    msg = MagicMock()
    msg.data = b"{}"
    msg.respond = AsyncMock()

    await bridge._handle_brain_health(msg)

    response = json.loads(msg.respond.call_args[0][0])
    assert response["status"] == "success"
    inner = json.loads(response["output"])
    assert inner["memories"] == 42
    assert inner["triples"] == 15
    assert inner["ollama"] is True
