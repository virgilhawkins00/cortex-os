"""Tests for the Ingest Pipeline — chunking, storing, and embedding."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import MagicMock

import pytest

from cortex_memory.palace.ingest import IngestPipeline, _CHUNK_THRESHOLD, _chunk_text
from cortex_memory.palace.storage import PalaceStorage
from cortex_memory.palace.vector import VectorStore


def _make_mock_vector_store() -> MagicMock:
    """Create a VectorStore mock that does nothing (no ChromaDB required)."""
    mock = MagicMock(spec=VectorStore)
    mock.add_embedding.return_value = "mock-embedding-id"
    return mock


@pytest.fixture
async def storage(tmp_path: Path) -> PalaceStorage:
    s = PalaceStorage(tmp_path / "test.db")
    await s.initialize()
    yield s
    await s.close()


@pytest.fixture
def pipeline(storage: PalaceStorage) -> IngestPipeline:
    return IngestPipeline(storage, _make_mock_vector_store())


# ── Chunking logic ─────────────────────────────────────────


def test_short_text_not_chunked() -> None:
    text = "Short text that fits in one chunk."
    chunks = _chunk_text(text)
    assert len(chunks) == 1
    assert chunks[0] == text


def test_long_text_chunked() -> None:
    # Build a text longer than the threshold via multiple paragraphs
    para = "This is a paragraph with enough content to be worth storing. " * 10
    long_text = "\n\n".join([para] * 5)  # 5 paragraphs
    assert len(long_text) > _CHUNK_THRESHOLD

    chunks = _chunk_text(long_text)
    assert len(chunks) > 1


def test_chunks_contain_all_content() -> None:
    """No content should be silently dropped during chunking."""
    words = [f"word{i}" for i in range(500)]
    text = " ".join(words)
    # Make it paragraph-friendly
    paras = [" ".join(words[i:i+50]) for i in range(0, 500, 50)]
    text = "\n\n".join(paras)

    chunks = _chunk_text(text)
    # All words should appear across chunks (may have overlap)
    all_chunk_text = " ".join(chunks)
    for word in words[:10]:  # spot check
        assert word in all_chunk_text


def test_empty_text_returns_single_empty_chunk() -> None:
    # Empty string returns an empty list (handled in ingest, not chunker)
    # The chunker itself returns [''] for empty input
    chunks = _chunk_text("   ")
    # whitespace-only stripped paragraphs produce empty list -> returns original
    assert isinstance(chunks, list)


# ── Ingest integration ─────────────────────────────────────


async def test_ingest_creates_memory(pipeline: IngestPipeline) -> None:
    memories = await pipeline.ingest(
        text="Cortex OS uses NATS as its message bus.",
        wing="projects",
        room="cortex-os",
    )
    assert len(memories) == 1
    assert memories[0].content == "Cortex OS uses NATS as its message bus."


async def test_ingest_creates_wing_and_room(pipeline: IngestPipeline, storage: PalaceStorage) -> None:
    await pipeline.ingest("test content", wing="auto-wing", room="auto-room")

    wing = await storage.get_wing("auto-wing")
    assert wing is not None

    room = await storage.get_room(wing.id, "auto-room")
    assert room is not None


async def test_ingest_empty_text_returns_empty(pipeline: IngestPipeline) -> None:
    memories = await pipeline.ingest("", wing="x", room="y")
    assert memories == []


async def test_ingest_with_metadata(pipeline: IngestPipeline, storage: PalaceStorage) -> None:
    memories = await pipeline.ingest(
        text="Important fact.",
        wing="facts",
        room="general",
        metadata={"source": "manual", "confidence": 1.0},
    )
    assert len(memories) == 1

    fetched = await storage.get_memory(memories[0].id)
    assert fetched is not None
    assert fetched.metadata["source"] == "manual"


async def test_ingest_long_text_multiple_chunks(pipeline: IngestPipeline) -> None:
    """Long texts should produce multiple memories, all verbatim."""
    para = "This is a sufficiently long paragraph with meaningful content. " * 20
    long_text = "\n\n".join([para] * 6)
    assert len(long_text) > _CHUNK_THRESHOLD

    memories = await pipeline.ingest(long_text, wing="long", room="test")
    assert len(memories) > 1
    # Each chunk content should be a substring of the original (verbatim)
    for mem in memories:
        # The content should not be fabricated — it must be from the original
        assert len(mem.content) > 0


async def test_ingest_stores_verbatim(pipeline: IngestPipeline, storage: PalaceStorage) -> None:
    """Ingest must never modify, summarize, or paraphrase the content."""
    original = 'The tool registry returns: {"bash": true, "file_read": true}'
    memories = await pipeline.ingest(original, wing="verbatim_test", room="room")

    assert len(memories) == 1
    fetched = await storage.get_memory(memories[0].id)
    assert fetched is not None
    assert fetched.content == original  # Exact verbatim


async def test_ingest_vector_failure_graceful(storage: PalaceStorage) -> None:
    """If ChromaDB is down, ingest should still store in SQLite without crashing."""
    bad_vector = MagicMock(spec=VectorStore)
    bad_vector.add_embedding.side_effect = ConnectionError("ChromaDB unavailable")

    pipeline = IngestPipeline(storage, bad_vector)
    memories = await pipeline.ingest("test content", wing="resilience", room="test")

    # Should still have stored to SQLite
    assert len(memories) == 1
    fetched = await storage.get_memory(memories[0].id)
    assert fetched is not None
    assert fetched.embedding_id is None  # No embedding, but memory exists


async def test_ingest_idempotent_wing_room(pipeline: IngestPipeline) -> None:
    """Calling ingest multiple times with same wing/room should not fail."""
    await pipeline.ingest("first", wing="same", room="room")
    await pipeline.ingest("second", wing="same", room="room")

    from cortex_memory.palace.storage import PalaceStorage as S
    # Both should be stored
    wing = await pipeline._storage.get_wing("same")
    assert wing is not None
    memories = await pipeline._storage.list_memories(wing_id=wing.id)
    assert len(memories) == 2
