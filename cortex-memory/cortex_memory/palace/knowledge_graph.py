"""Knowledge Graph — Temporal triple store for structured reasoning.

Stores entity relationships as (subject, predicate, object) triples with
timestamps and confidence scores. Enables the LLM to reason about
relationships between concepts across the entire memory palace.
"""

from __future__ import annotations

import json
import logging
from datetime import datetime, timezone

import aiosqlite

from .models import Triple

logger = logging.getLogger(__name__)

_KG_SCHEMA = """
CREATE TABLE IF NOT EXISTS knowledge_triples (
    id          TEXT PRIMARY KEY,
    subject     TEXT NOT NULL,
    predicate   TEXT NOT NULL,
    object      TEXT NOT NULL,
    confidence  REAL DEFAULT 1.0,
    metadata    TEXT DEFAULT '{}',
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_triples_subject ON knowledge_triples(subject);
CREATE INDEX IF NOT EXISTS idx_triples_predicate ON knowledge_triples(predicate);
CREATE INDEX IF NOT EXISTS idx_triples_object ON knowledge_triples(object);
CREATE INDEX IF NOT EXISTS idx_triples_created ON knowledge_triples(created_at DESC);
"""


class KnowledgeGraph:
    """SQLite-backed temporal knowledge graph.

    Stores relationships between entities as triples, enabling
    structured queries like "what do we know about project X?"
    or "what tools are related to security?".

    Usage::

        kg = KnowledgeGraph(db)
        await kg.initialize()
        await kg.add_triple("Cortex OS", "uses", "Rust")
        await kg.add_triple("Cortex OS", "uses", "Python")
        triples = await kg.query_triples(subject="Cortex OS")
    """

    def __init__(self, db: aiosqlite.Connection) -> None:
        self._db = db

    async def initialize(self) -> None:
        """Create the knowledge graph tables."""
        await self._db.executescript(_KG_SCHEMA)
        await self._db.commit()
        logger.info("Knowledge graph initialized")

    async def add_triple(
        self,
        subject: str,
        predicate: str,
        obj: str,
        confidence: float = 1.0,
        metadata: dict | None = None,
    ) -> Triple:
        """Add a knowledge triple.

        Args:
            subject: The entity the triple is about.
            predicate: The relationship type (e.g. "uses", "depends_on", "created_by").
            obj: The target entity.
            confidence: How confident we are in this fact (0.0 to 1.0).
            metadata: Additional context (source, conversation_id, etc.).
        """
        triple = Triple(
            subject=subject,
            predicate=predicate,
            object=obj,
            confidence=confidence,
            metadata=metadata or {},
        )
        await self._db.execute(
            "INSERT INTO knowledge_triples "
            "(id, subject, predicate, object, confidence, metadata, created_at) "
            "VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                triple.id,
                triple.subject,
                triple.predicate,
                triple.object,
                triple.confidence,
                json.dumps(triple.metadata),
                triple.created_at.isoformat(),
            ),
        )
        await self._db.commit()
        logger.info("Added triple: %s → %s → %s", subject, predicate, obj)
        return triple

    async def query_triples(
        self,
        subject: str | None = None,
        predicate: str | None = None,
        obj: str | None = None,
        min_confidence: float = 0.0,
        limit: int = 100,
    ) -> list[Triple]:
        """Query triples with optional filters on any field."""
        conditions = ["confidence >= ?"]
        params: list = [min_confidence]

        if subject:
            conditions.append("subject = ?")
            params.append(subject)
        if predicate:
            conditions.append("predicate = ?")
            params.append(predicate)
        if obj:
            conditions.append("object = ?")
            params.append(obj)

        where_clause = " AND ".join(conditions)
        query = (
            f"SELECT * FROM knowledge_triples WHERE {where_clause} "
            "ORDER BY created_at DESC LIMIT ?"
        )
        params.append(limit)

        async with self._db.execute(query, params) as cursor:
            rows = await cursor.fetchall()
            return [self._row_to_triple(r) for r in rows]

    async def get_entity_context(self, entity: str, limit: int = 50) -> list[Triple]:
        """Get all triples involving an entity (as subject OR object).

        This gives the LLM a complete picture of everything known about an entity.
        """
        query = (
            "SELECT * FROM knowledge_triples "
            "WHERE subject = ? OR object = ? "
            "ORDER BY confidence DESC, created_at DESC "
            "LIMIT ?"
        )
        async with self._db.execute(query, (entity, entity, limit)) as cursor:
            rows = await cursor.fetchall()
            return [self._row_to_triple(r) for r in rows]

    async def get_triples_since(self, since: datetime, limit: int = 100) -> list[Triple]:
        """Get triples created after a specific time — temporal queries."""
        query = (
            "SELECT * FROM knowledge_triples "
            "WHERE created_at >= ? "
            "ORDER BY created_at DESC LIMIT ?"
        )
        async with self._db.execute(query, (since.isoformat(), limit)) as cursor:
            rows = await cursor.fetchall()
            return [self._row_to_triple(r) for r in rows]

    async def delete_triple(self, triple_id: str) -> bool:
        """Delete a triple by ID."""
        cursor = await self._db.execute(
            "DELETE FROM knowledge_triples WHERE id = ?", (triple_id,)
        )
        await self._db.commit()
        return cursor.rowcount > 0

    async def count_triples(self) -> int:
        """Count total triples in the knowledge graph."""
        async with self._db.execute("SELECT COUNT(*) FROM knowledge_triples") as cursor:
            row = await cursor.fetchone()
            return row[0] if row else 0

    async def export_for_llm(self, entity: str | None = None, limit: int = 50) -> str:
        """Export triples as a compact text format for LLM context injection.

        AAAK-style compressed index:
        - subject → predicate → object [confidence] (timestamp)
        """
        if entity:
            triples = await self.get_entity_context(entity, limit)
        else:
            triples = await self.query_triples(limit=limit)

        if not triples:
            return ""

        lines = []
        for t in triples:
            conf = f" [{t.confidence:.1f}]" if t.confidence < 1.0 else ""
            ts = t.created_at.strftime("%Y-%m-%d")
            lines.append(f"• {t.subject} → {t.predicate} → {t.object}{conf} ({ts})")

        return "\n".join(lines)

    @staticmethod
    def _row_to_triple(row: aiosqlite.Row) -> Triple:
        """Convert a database row to a Triple model."""
        return Triple(
            id=row["id"],
            subject=row["subject"],
            predicate=row["predicate"],
            object=row["object"],
            confidence=row["confidence"],
            metadata=json.loads(row["metadata"]) if row["metadata"] else {},
            created_at=datetime.fromisoformat(row["created_at"]),
        )
