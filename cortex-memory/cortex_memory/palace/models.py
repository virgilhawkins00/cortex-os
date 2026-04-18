"""Palace data models — the Memory Palace hierarchy.

Wing → Room → Drawer → Memory

Every piece of information the system learns is stored as a Memory
inside a Drawer, inside a Room, inside a Wing. This mirrors the
method of loci mnemonic technique, giving the LLM a spatial metaphor
for organizing and retrieving knowledge.
"""

from __future__ import annotations

from datetime import datetime, timezone
from typing import Any
from uuid import uuid4

from pydantic import BaseModel, ConfigDict, Field, field_serializer


def _utcnow() -> datetime:
    return datetime.now(timezone.utc)


def _new_id() -> str:
    return uuid4().hex


class Wing(BaseModel):
    """Top-level category in the Memory Palace (e.g. 'projects', 'preferences')."""

    id: str = Field(default_factory=_new_id)
    name: str
    description: str = ""
    created_at: datetime = Field(default_factory=_utcnow)


class Room(BaseModel):
    """A topic area within a Wing (e.g. 'cortex-os', 'rust-patterns')."""

    id: str = Field(default_factory=_new_id)
    wing_id: str
    name: str
    description: str = ""
    created_at: datetime = Field(default_factory=_utcnow)


class Drawer(BaseModel):
    """A specific container within a Room (e.g. 'architecture-decisions')."""

    id: str = Field(default_factory=_new_id)
    room_id: str
    name: str
    description: str = ""
    created_at: datetime = Field(default_factory=_utcnow)


class Memory(BaseModel):
    """The atomic unit of knowledge — verbatim content, never summarized.

    Each Memory is stored exactly as received. The system NEVER paraphrases
    or summarizes stored content. Retrieval always returns the original text.
    """

    id: str = Field(default_factory=_new_id)
    content: str
    wing_id: str
    room_id: str
    drawer_id: str | None = None
    embedding_id: str | None = None
    metadata: dict[str, Any] = Field(default_factory=dict)
    created_at: datetime = Field(default_factory=_utcnow)
    updated_at: datetime = Field(default_factory=_utcnow)

    model_config = ConfigDict()

    @field_serializer("created_at", "updated_at")
    def serialize_dt(self, dt: datetime) -> str:
        return dt.isoformat()


class SearchResult(BaseModel):
    """A single result from hybrid search."""

    memory: Memory
    score: float
    source: str = "hybrid"  # "bm25", "vector", or "hybrid"


class Triple(BaseModel):
    """A knowledge graph triple: subject → predicate → object."""

    id: str = Field(default_factory=_new_id)
    subject: str
    predicate: str
    object: str
    confidence: float = 1.0
    created_at: datetime = Field(default_factory=_utcnow)
    metadata: dict[str, Any] = Field(default_factory=dict)
