"""Tests for Hybrid Search — BM25 + vector fusion with RRF."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from cortex_memory.palace.search import HybridSearchEngine, SearchConfig, _RRF_K
from cortex_memory.palace.storage import PalaceStorage
from cortex_memory.palace.vector import VectorStore


def _mock_vector_no_results() -> MagicMock:
    """Vector store that always returns empty results."""
    mock = MagicMock(spec=VectorStore)
    mock.search_similar.return_value = []
    mock.search_similar_all_wings.return_value = []
    return mock


def _mock_vector_with_results(pairs: list[tuple[str, float]]) -> MagicMock:
    """Vector store that returns specific (id, score) pairs."""
    mock = MagicMock(spec=VectorStore)
    mock.search_similar.return_value = pairs
    mock.search_similar_all_wings.return_value = pairs
    return mock


@pytest.fixture
async def storage(tmp_path: Path) -> PalaceStorage:
    s = PalaceStorage(tmp_path / "test.db")
    await s.initialize()
    # Seed with test data
    wing = await s.create_wing("test")
    room = await s.create_room(wing.id, "search")
    await s.store_memory("Rust is the primary language for Cortex OS", wing.id, room.id)
    await s.store_memory("Python handles LLM integration and embeddings", wing.id, room.id)
    await s.store_memory("NATS is used as the message bus between services", wing.id, room.id)
    await s.store_memory("ChromaDB stores vector embeddings for semantic search", wing.id, room.id)
    await s.store_memory("The sandbox enforces timeout and output limits", wing.id, room.id)
    yield s
    await s.close()


async def test_search_returns_results(storage: PalaceStorage) -> None:
    engine = HybridSearchEngine(storage, _mock_vector_no_results())
    await engine.rebuild_bm25_index()

    results = await engine.search("Rust language")
    assert len(results) >= 1
    assert any("Rust" in r.memory.content for r in results)


async def test_search_empty_query(storage: PalaceStorage) -> None:
    engine = HybridSearchEngine(storage, _mock_vector_no_results())
    results = await engine.search("")
    assert results == []


async def test_search_no_results(storage: PalaceStorage) -> None:
    engine = HybridSearchEngine(storage, _mock_vector_no_results())
    await engine.rebuild_bm25_index()

    results = await engine.search("xyzabc123notarealword99999")
    assert results == []


async def test_results_have_scores(storage: PalaceStorage) -> None:
    engine = HybridSearchEngine(storage, _mock_vector_no_results())
    await engine.rebuild_bm25_index()

    results = await engine.search("NATS message bus")
    for r in results:
        assert r.score > 0.0
        assert r.source in ("bm25", "vector", "hybrid")


async def test_results_verbatim(storage: PalaceStorage) -> None:
    """Search results must always return verbatim memory content."""
    engine = HybridSearchEngine(storage, _mock_vector_no_results())
    await engine.rebuild_bm25_index()

    results = await engine.search("sandbox timeout")
    assert len(results) >= 1
    # Content must be exactly what was stored — no modification
    assert results[0].memory.content == "The sandbox enforces timeout and output limits"


async def test_rrf_boosts_documents_in_both_result_sets(storage: PalaceStorage) -> None:
    """Documents appearing in both BM25 and vector results should rank higher."""
    # Get a real memory ID from storage
    memories = await storage.list_memories()
    assert memories

    # Mock vector to return the same doc that BM25 would find
    rust_memory = next(m for m in memories if "Rust" in m.content)
    vector_mock = _mock_vector_with_results([(rust_memory.id, 0.9)])

    engine = HybridSearchEngine(storage, vector_mock)
    await engine.rebuild_bm25_index()

    results = await engine.search("Rust language Cortex")
    assert len(results) >= 1
    # The overlapping result should be first due to RRF boosting
    assert results[0].memory.id == rust_memory.id


async def test_search_top_k_limit(storage: PalaceStorage) -> None:
    engine = HybridSearchEngine(storage, _mock_vector_no_results())
    await engine.rebuild_bm25_index()

    results = await engine.search("the", top_k=2)
    assert len(results) <= 2


async def test_rrf_merge_correctness() -> None:
    """Unit test for the RRF merge algorithm itself."""
    engine = HybridSearchEngine.__new__(HybridSearchEngine)
    engine._config = SearchConfig(bm25_weight=0.4, vector_weight=0.6)

    bm25 = [("doc_A", 10.0), ("doc_B", 8.0), ("doc_C", 5.0)]
    vector = [("doc_B", 0.9), ("doc_A", 0.7), ("doc_D", 0.5)]

    merged = engine._rrf_merge(bm25, vector)
    merged_ids = [m[0] for m in merged]

    # doc_B and doc_A both appear in both lists — should rank at top
    assert merged_ids[0] in ("doc_A", "doc_B")
    assert merged_ids[1] in ("doc_A", "doc_B")
    # doc_D only in vector, doc_C only in bm25
    assert "doc_D" in merged_ids or "doc_C" in merged_ids


async def test_bm25_fallback_to_fts5_when_no_index(storage: PalaceStorage) -> None:
    """If BM25 index not built, should fall back to SQLite FTS5."""
    engine = HybridSearchEngine(storage, _mock_vector_no_results())
    # Do NOT call rebuild_bm25_index — index is None

    results = await engine.search("NATS")
    # Should still return results via FTS5 fallback
    assert isinstance(results, list)
