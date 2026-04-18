"""Palace Storage — Async SQLite backend with FTS5 full-text search.

This is the persistence layer for the Memory Palace. All memories are stored
verbatim in SQLite with a FTS5 virtual table for fast full-text search.
"""

from __future__ import annotations

import json
import logging
from datetime import datetime, timezone
from pathlib import Path

import aiosqlite

from .models import Drawer, Memory, Room, Wing

logger = logging.getLogger(__name__)

# ── Schema ────────────────────────────────────────────────────────────

_SCHEMA = """
CREATE TABLE IF NOT EXISTS wings (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT DEFAULT '',
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS rooms (
    id          TEXT PRIMARY KEY,
    wing_id     TEXT NOT NULL REFERENCES wings(id),
    name        TEXT NOT NULL,
    description TEXT DEFAULT '',
    created_at  TEXT NOT NULL,
    UNIQUE(wing_id, name)
);

CREATE TABLE IF NOT EXISTS drawers (
    id          TEXT PRIMARY KEY,
    room_id     TEXT NOT NULL REFERENCES rooms(id),
    name        TEXT NOT NULL,
    description TEXT DEFAULT '',
    created_at  TEXT NOT NULL,
    UNIQUE(room_id, name)
);

CREATE TABLE IF NOT EXISTS memories (
    id           TEXT PRIMARY KEY,
    content      TEXT NOT NULL,
    wing_id      TEXT NOT NULL REFERENCES wings(id),
    room_id      TEXT NOT NULL REFERENCES rooms(id),
    drawer_id    TEXT REFERENCES drawers(id),
    embedding_id TEXT,
    metadata     TEXT DEFAULT '{}',
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

-- FTS5 table without content_rowid: stores content directly.
-- Kept in sync via triggers. Simpler and more portable across SQLite versions.
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    id UNINDEXED,
    content,
    tokenize='porter unicode61'
);

CREATE INDEX IF NOT EXISTS idx_memories_wing ON memories(wing_id);
CREATE INDEX IF NOT EXISTS idx_memories_room ON memories(room_id);
CREATE INDEX IF NOT EXISTS idx_memories_created ON memories(created_at DESC);

CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(id, content) VALUES (NEW.id, NEW.content);
END;

CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
    DELETE FROM memories_fts WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE OF content ON memories BEGIN
    DELETE FROM memories_fts WHERE id = OLD.id;
    INSERT INTO memories_fts(id, content) VALUES (NEW.id, NEW.content);
END;
"""


class PalaceStorage:
    """Async SQLite storage for the Memory Palace.

    Usage::

        storage = PalaceStorage(Path("./data/cortex.db"))
        await storage.initialize()
        wing = await storage.create_wing("projects")
        room = await storage.create_room(wing.id, "cortex-os")
        memory = await storage.store_memory(
            content="The sandbox timeout is 30 seconds",
            wing_id=wing.id,
            room_id=room.id,
        )
        results = await storage.search_fts("sandbox timeout")
        await storage.close()
    """

    def __init__(self, db_path: Path) -> None:
        self._db_path = db_path
        self._db: aiosqlite.Connection | None = None

    async def initialize(self) -> None:
        """Open the database and create tables if needed."""
        self._db_path.parent.mkdir(parents=True, exist_ok=True)
        self._db = await aiosqlite.connect(str(self._db_path))
        self._db.row_factory = aiosqlite.Row

        # Enable WAL mode for concurrent reads
        await self._db.execute("PRAGMA journal_mode=WAL")
        await self._db.execute("PRAGMA foreign_keys=ON")

        await self._db.executescript(_SCHEMA)
        await self._db.commit()
        logger.info("Palace storage initialized at %s", self._db_path)

    async def close(self) -> None:
        """Close the database connection."""
        if self._db:
            await self._db.close()
            self._db = None

    @property
    def db(self) -> aiosqlite.Connection:
        """Get the active database connection."""
        if self._db is None:
            msg = "Storage not initialized — call initialize() first"
            raise RuntimeError(msg)
        return self._db

    # ── Wings ─────────────────────────────────────────────────

    async def create_wing(self, name: str, description: str = "") -> Wing:
        """Create a new wing (top-level category)."""
        wing = Wing(name=name, description=description)
        await self.db.execute(
            "INSERT INTO wings (id, name, description, created_at) VALUES (?, ?, ?, ?)",
            (wing.id, wing.name, wing.description, wing.created_at.isoformat()),
        )
        await self.db.commit()
        logger.info("Created wing: %s (%s)", wing.name, wing.id)
        return wing

    async def get_wing(self, name: str) -> Wing | None:
        """Get a wing by name."""
        async with self.db.execute(
            "SELECT * FROM wings WHERE name = ?", (name,)
        ) as cursor:
            row = await cursor.fetchone()
            if row is None:
                return None
            return Wing(
                id=row["id"],
                name=row["name"],
                description=row["description"],
                created_at=datetime.fromisoformat(row["created_at"]),
            )

    async def get_or_create_wing(self, name: str, description: str = "") -> Wing:
        """Get existing wing or create a new one."""
        wing = await self.get_wing(name)
        if wing is not None:
            return wing
        return await self.create_wing(name, description)

    async def list_wings(self) -> list[Wing]:
        """List all wings."""
        async with self.db.execute("SELECT * FROM wings ORDER BY name") as cursor:
            rows = await cursor.fetchall()
            return [
                Wing(
                    id=r["id"],
                    name=r["name"],
                    description=r["description"],
                    created_at=datetime.fromisoformat(r["created_at"]),
                )
                for r in rows
            ]

    # ── Rooms ─────────────────────────────────────────────────

    async def create_room(self, wing_id: str, name: str, description: str = "") -> Room:
        """Create a new room within a wing."""
        room = Room(wing_id=wing_id, name=name, description=description)
        await self.db.execute(
            "INSERT INTO rooms (id, wing_id, name, description, created_at) VALUES (?, ?, ?, ?, ?)",
            (room.id, room.wing_id, room.name, room.description, room.created_at.isoformat()),
        )
        await self.db.commit()
        logger.info("Created room: %s in wing %s", room.name, wing_id)
        return room

    async def get_room(self, wing_id: str, name: str) -> Room | None:
        """Get a room by wing and name."""
        async with self.db.execute(
            "SELECT * FROM rooms WHERE wing_id = ? AND name = ?", (wing_id, name)
        ) as cursor:
            row = await cursor.fetchone()
            if row is None:
                return None
            return Room(
                id=row["id"],
                wing_id=row["wing_id"],
                name=row["name"],
                description=row["description"],
                created_at=datetime.fromisoformat(row["created_at"]),
            )

    async def get_or_create_room(
        self, wing_id: str, name: str, description: str = ""
    ) -> Room:
        """Get existing room or create a new one."""
        room = await self.get_room(wing_id, name)
        if room is not None:
            return room
        return await self.create_room(wing_id, name, description)

    async def list_rooms(self, wing_id: str) -> list[Room]:
        """List all rooms in a wing."""
        async with self.db.execute(
            "SELECT * FROM rooms WHERE wing_id = ? ORDER BY name", (wing_id,)
        ) as cursor:
            rows = await cursor.fetchall()
            return [
                Room(
                    id=r["id"],
                    wing_id=r["wing_id"],
                    name=r["name"],
                    description=r["description"],
                    created_at=datetime.fromisoformat(r["created_at"]),
                )
                for r in rows
            ]

    # ── Drawers ───────────────────────────────────────────────

    async def create_drawer(self, room_id: str, name: str, description: str = "") -> Drawer:
        """Create a new drawer within a room."""
        drawer = Drawer(room_id=room_id, name=name, description=description)
        await self.db.execute(
            "INSERT INTO drawers (id, room_id, name, description, created_at) "
            "VALUES (?, ?, ?, ?, ?)",
            (
                drawer.id,
                drawer.room_id,
                drawer.name,
                drawer.description,
                drawer.created_at.isoformat(),
            ),
        )
        await self.db.commit()
        return drawer

    # ── Memories ──────────────────────────────────────────────

    async def store_memory(
        self,
        content: str,
        wing_id: str,
        room_id: str,
        drawer_id: str | None = None,
        embedding_id: str | None = None,
        metadata: dict | None = None,
    ) -> Memory:
        """Store a new memory — verbatim, never summarized."""
        memory = Memory(
            content=content,
            wing_id=wing_id,
            room_id=room_id,
            drawer_id=drawer_id,
            embedding_id=embedding_id,
            metadata=metadata or {},
        )
        await self.db.execute(
            "INSERT INTO memories "
            "(id, content, wing_id, room_id, drawer_id, embedding_id, metadata, "
            "created_at, updated_at) "
            "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                memory.id,
                memory.content,
                memory.wing_id,
                memory.room_id,
                memory.drawer_id,
                memory.embedding_id,
                json.dumps(memory.metadata),
                memory.created_at.isoformat(),
                memory.updated_at.isoformat(),
            ),
        )
        await self.db.commit()
        logger.info("Stored memory %s (%d chars)", memory.id[:8], len(content))
        return memory

    async def get_memory(self, memory_id: str) -> Memory | None:
        """Retrieve a memory by ID — always returns verbatim content."""
        async with self.db.execute(
            "SELECT * FROM memories WHERE id = ?", (memory_id,)
        ) as cursor:
            row = await cursor.fetchone()
            if row is None:
                return None
            return self._row_to_memory(row)

    async def list_memories(
        self,
        wing_id: str | None = None,
        room_id: str | None = None,
        limit: int = 100,
    ) -> list[Memory]:
        """List memories with optional filtering."""
        query = "SELECT * FROM memories WHERE 1=1"
        params: list = []

        if wing_id:
            query += " AND wing_id = ?"
            params.append(wing_id)
        if room_id:
            query += " AND room_id = ?"
            params.append(room_id)

        query += " ORDER BY created_at DESC LIMIT ?"
        params.append(limit)

        async with self.db.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            return [self._row_to_memory(r) for r in rows]

    async def delete_memory(self, memory_id: str) -> bool:
        """Delete a memory by ID."""
        cursor = await self.db.execute(
            "DELETE FROM memories WHERE id = ?", (memory_id,)
        )
        await self.db.commit()
        return cursor.rowcount > 0

    async def update_embedding_id(self, memory_id: str, embedding_id: str) -> None:
        """Link a memory to its ChromaDB embedding."""
        now = datetime.now(timezone.utc).isoformat()
        await self.db.execute(
            "UPDATE memories SET embedding_id = ?, updated_at = ? WHERE id = ?",
            (embedding_id, now, memory_id),
        )
        await self.db.commit()

    async def count_memories(self) -> int:
        """Count total memories in storage."""
        async with self.db.execute("SELECT COUNT(*) FROM memories") as cursor:
            row = await cursor.fetchone()
            return row[0] if row else 0

    # ── Full-Text Search ──────────────────────────────────────

    async def search_fts(self, query: str, limit: int = 20) -> list[Memory]:
        """Search memories using FTS5 full-text index.

        Returns memories ranked by BM25 relevance. The content is always
        returned verbatim — no snippets, no highlights, no summarization.
        """
        if not query.strip():
            return []

        sql = """
            SELECT m.*
            FROM memories m
            JOIN memories_fts fts ON m.id = fts.id
            WHERE fts.content MATCH ?
            ORDER BY rank
            LIMIT ?
        """
        async with self.db.execute(sql, (query, limit)) as cursor:
            rows = await cursor.fetchall()
            return [self._row_to_memory(r) for r in rows]

    async def get_all_contents(self) -> list[tuple[str, str]]:
        """Get all (memory_id, content) pairs — used by BM25 index builder."""
        async with self.db.execute("SELECT id, content FROM memories") as cursor:
            rows = await cursor.fetchall()
            return [(r["id"], r["content"]) for r in rows]

    # ── Helpers ───────────────────────────────────────────────

    @staticmethod
    def _row_to_memory(row: aiosqlite.Row) -> Memory:
        """Convert a database row to a Memory model."""
        return Memory(
            id=row["id"],
            content=row["content"],
            wing_id=row["wing_id"],
            room_id=row["room_id"],
            drawer_id=row["drawer_id"],
            embedding_id=row["embedding_id"],
            metadata=json.loads(row["metadata"]) if row["metadata"] else {},
            created_at=datetime.fromisoformat(row["created_at"]),
            updated_at=datetime.fromisoformat(row["updated_at"]),
        )
