"""Tests for Palace Storage — SQLite + FTS5 operations."""

from __future__ import annotations

import pytest
from pathlib import Path

from cortex_memory.palace.storage import PalaceStorage


@pytest.fixture
async def storage(tmp_path: Path) -> PalaceStorage:
    """Create a fresh in-memory storage for each test."""
    db_path = tmp_path / "test.db"
    s = PalaceStorage(db_path)
    await s.initialize()
    yield s
    await s.close()


# ── Wings ──────────────────────────────────────────────────


async def test_create_wing(storage: PalaceStorage) -> None:
    wing = await storage.create_wing("projects", "All project knowledge")
    assert wing.id
    assert wing.name == "projects"
    assert wing.description == "All project knowledge"


async def test_get_wing(storage: PalaceStorage) -> None:
    created = await storage.create_wing("tools")
    fetched = await storage.get_wing("tools")
    assert fetched is not None
    assert fetched.id == created.id


async def test_get_wing_not_found(storage: PalaceStorage) -> None:
    result = await storage.get_wing("nonexistent")
    assert result is None


async def test_get_or_create_wing_idempotent(storage: PalaceStorage) -> None:
    w1 = await storage.get_or_create_wing("projects")
    w2 = await storage.get_or_create_wing("projects")
    assert w1.id == w2.id


async def test_list_wings(storage: PalaceStorage) -> None:
    await storage.create_wing("alpha")
    await storage.create_wing("beta")
    await storage.create_wing("gamma")
    wings = await storage.list_wings()
    names = [w.name for w in wings]
    assert "alpha" in names
    assert "beta" in names
    assert "gamma" in names


# ── Rooms ──────────────────────────────────────────────────


async def test_create_room(storage: PalaceStorage) -> None:
    wing = await storage.create_wing("projects")
    room = await storage.create_room(wing.id, "cortex-os")
    assert room.id
    assert room.wing_id == wing.id
    assert room.name == "cortex-os"


async def test_get_or_create_room_idempotent(storage: PalaceStorage) -> None:
    wing = await storage.create_wing("projects")
    r1 = await storage.get_or_create_room(wing.id, "cortex-os")
    r2 = await storage.get_or_create_room(wing.id, "cortex-os")
    assert r1.id == r2.id


async def test_list_rooms(storage: PalaceStorage) -> None:
    wing = await storage.create_wing("projects")
    await storage.create_room(wing.id, "alpha")
    await storage.create_room(wing.id, "beta")
    rooms = await storage.list_rooms(wing.id)
    assert len(rooms) == 2


# ── Memories ───────────────────────────────────────────────


async def test_store_memory_verbatim(storage: PalaceStorage) -> None:
    """Verify that content is stored EXACTLY as given — never modified."""
    wing = await storage.create_wing("test")
    room = await storage.create_room(wing.id, "verbatim")

    original = "The sandbox timeout is 30 seconds and max output is 512KB."
    memory = await storage.store_memory(
        content=original,
        wing_id=wing.id,
        room_id=room.id,
    )

    retrieved = await storage.get_memory(memory.id)
    assert retrieved is not None
    assert retrieved.content == original  # Exact verbatim match


async def test_get_memory_not_found(storage: PalaceStorage) -> None:
    result = await storage.get_memory("nonexistent-id")
    assert result is None


async def test_store_memory_with_metadata(storage: PalaceStorage) -> None:
    wing = await storage.create_wing("meta")
    room = await storage.create_room(wing.id, "test")
    memory = await storage.store_memory(
        content="test content",
        wing_id=wing.id,
        room_id=room.id,
        metadata={"source": "test", "confidence": 0.95},
    )
    fetched = await storage.get_memory(memory.id)
    assert fetched is not None
    assert fetched.metadata["source"] == "test"
    assert fetched.metadata["confidence"] == 0.95


async def test_list_memories(storage: PalaceStorage) -> None:
    wing = await storage.create_wing("list_test")
    room = await storage.create_room(wing.id, "room1")
    for i in range(5):
        await storage.store_memory(f"memory content {i}", wing.id, room.id)

    memories = await storage.list_memories(wing_id=wing.id)
    assert len(memories) == 5


async def test_delete_memory(storage: PalaceStorage) -> None:
    wing = await storage.create_wing("del_test")
    room = await storage.create_room(wing.id, "room")
    memory = await storage.store_memory("delete me", wing.id, room.id)

    deleted = await storage.delete_memory(memory.id)
    assert deleted is True

    retrieved = await storage.get_memory(memory.id)
    assert retrieved is None


async def test_delete_nonexistent_memory(storage: PalaceStorage) -> None:
    deleted = await storage.delete_memory("fake-id")
    assert deleted is False


async def test_count_memories(storage: PalaceStorage) -> None:
    wing = await storage.create_wing("count")
    room = await storage.create_room(wing.id, "r")
    assert await storage.count_memories() == 0

    await storage.store_memory("one", wing.id, room.id)
    await storage.store_memory("two", wing.id, room.id)
    assert await storage.count_memories() == 2


# ── Full-Text Search ───────────────────────────────────────


async def test_search_fts_finds_keyword(storage: PalaceStorage) -> None:
    wing = await storage.create_wing("fts")
    room = await storage.create_room(wing.id, "search")

    await storage.store_memory("The sandbox timeout is 30 seconds", wing.id, room.id)
    await storage.store_memory("Permissions can be full or readonly", wing.id, room.id)
    await storage.store_memory("NATS is the message bus", wing.id, room.id)

    results = await storage.search_fts("sandbox timeout")
    assert len(results) >= 1
    assert any("sandbox" in r.content.lower() for r in results)


async def test_search_fts_verbatim_recall(storage: PalaceStorage) -> None:
    """FTS results must always return the verbatim original content."""
    wing = await storage.create_wing("fts2")
    room = await storage.create_room(wing.id, "search")

    original = "ChromaDB stores vectors for semantic search operations."
    await storage.store_memory(original, wing.id, room.id)

    results = await storage.search_fts("ChromaDB vectors")
    assert len(results) >= 1
    assert results[0].content == original  # Verbatim — no summarization


async def test_search_fts_empty_query(storage: PalaceStorage) -> None:
    results = await storage.search_fts("")
    assert results == []


async def test_search_fts_no_match(storage: PalaceStorage) -> None:
    wing = await storage.create_wing("fts3")
    room = await storage.create_room(wing.id, "room")
    await storage.store_memory("completely unrelated content", wing.id, room.id)

    results = await storage.search_fts("xyzabc123notaword")
    assert results == []
