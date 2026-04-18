"""Ingest Pipeline — Transform raw text into stored, searchable, embeddable memories.

Flow: text → validate → chunk (if long) → store in SQLite → embed in ChromaDB
"""

from __future__ import annotations

import logging
import re
from typing import Any

from .models import Memory
from .storage import PalaceStorage
from .vector import VectorStore

logger = logging.getLogger(__name__)

# Maximum content length before chunking kicks in
_CHUNK_THRESHOLD = 2000
# Overlap between chunks to preserve context
_CHUNK_OVERLAP = 200
# Minimum chunk size to avoid tiny fragments
_MIN_CHUNK_SIZE = 100


def _chunk_text(text: str) -> list[str]:
    """Split text into chunks if it exceeds the threshold.

    Strategy:
    1. If text is short enough, return as-is
    2. Split by double newlines (paragraphs)
    3. Merge small chunks with overlap for context continuity
    """
    if len(text) <= _CHUNK_THRESHOLD:
        return [text]

    paragraphs = re.split(r"\n\s*\n", text)
    paragraphs = [p.strip() for p in paragraphs if p.strip()]

    if not paragraphs:
        return [text]

    chunks: list[str] = []
    current_chunk: list[str] = []
    current_length = 0

    for para in paragraphs:
        if current_length + len(para) > _CHUNK_THRESHOLD and current_chunk:
            chunk_text_val = "\n\n".join(current_chunk)
            chunks.append(chunk_text_val)

            overlap_text = chunk_text_val[-_CHUNK_OVERLAP:] if len(chunk_text_val) > _CHUNK_OVERLAP else ""
            if overlap_text:
                current_chunk = [overlap_text, para]
                current_length = len(overlap_text) + len(para)
            else:
                current_chunk = [para]
                current_length = len(para)
        else:
            current_chunk.append(para)
            current_length += len(para)

    if current_chunk:
        chunk_text_val = "\n\n".join(current_chunk)
        if len(chunk_text_val) >= _MIN_CHUNK_SIZE:
            chunks.append(chunk_text_val)
        elif chunks:
            chunks[-1] += "\n\n" + chunk_text_val
        else:
            chunks.append(chunk_text_val)

    return chunks if chunks else [text]


class IngestPipeline:
    """Transforms raw text into stored, searchable memories.

    Usage::

        pipeline = IngestPipeline(storage, vector_store)
        memories = await pipeline.ingest(
            text="Long document content...",
            wing="projects",
            room="cortex-os",
        )
    """

    def __init__(
        self,
        storage: PalaceStorage,
        vector_store: VectorStore,
    ) -> None:
        self._storage = storage
        self._vector_store = vector_store

    async def ingest(
        self,
        text: str,
        wing: str,
        room: str,
        metadata: dict[str, Any] | None = None,
    ) -> list[Memory]:
        """Ingest text into the memory system.

        1. Get or create wing and room
        2. Chunk if text is too long
        3. Store each chunk as a Memory in SQLite
        4. Embed each chunk in ChromaDB
        5. Link memory to embedding

        Args:
            text: Raw text to ingest (stored verbatim).
            wing: Wing name (category).
            room: Room name (topic).
            metadata: Additional metadata for the memory.

        Returns:
            List of created Memory objects.
        """
        text = text.strip()
        if not text:
            logger.warning("Ingest called with empty text — skipping")
            return []

        # ── Ensure wing and room exist ────────────
        wing_obj = await self._storage.get_or_create_wing(wing)
        room_obj = await self._storage.get_or_create_room(wing_obj.id, room)

        # ── Chunk if needed ───────────────────────
        chunks = self._chunk_text(text)
        logger.info(
            "Ingesting %d chunk(s) into %s/%s (%d chars total)",
            len(chunks),
            wing,
            room,
            len(text),
        )

        # ── Store each chunk ──────────────────────
        memories: list[Memory] = []
        for i, chunk in enumerate(chunks):
            chunk_meta = dict(metadata or {})
            if len(chunks) > 1:
                chunk_meta["chunk_index"] = i
                chunk_meta["total_chunks"] = len(chunks)

            # Store in SQLite
            memory = await self._storage.store_memory(
                content=chunk,
                wing_id=wing_obj.id,
                room_id=room_obj.id,
                metadata=chunk_meta,
            )

            # Embed in ChromaDB
            try:
                embedding_id = self._vector_store.add_embedding(
                    memory_id=memory.id,
                    text=chunk,
                    wing=wing,
                    metadata=chunk_meta,
                )
                await self._storage.update_embedding_id(memory.id, embedding_id)
                memory.embedding_id = embedding_id
            except Exception:
                logger.warning(
                    "Failed to embed memory %s — stored without vector",
                    memory.id[:8],
                )

            memories.append(memory)

        logger.info(
            "Ingested %d memories into %s/%s",
            len(memories),
            wing,
            room,
        )
        return memories

    async def ingest_batch(
        self,
        items: list[tuple[str, str, str, dict[str, Any] | None]],
    ) -> list[Memory]:
        """Batch ingest multiple texts.

        Args:
            items: List of (text, wing, room, metadata) tuples.

        Returns:
            All created memories.
        """
        all_memories: list[Memory] = []
        for text, wing, room, metadata in items:
            memories = await self.ingest(text, wing, room, metadata)
            all_memories.extend(memories)
        return all_memories

    @staticmethod
    def _chunk_text(text: str) -> list[str]:
        """Delegate to module-level _chunk_text."""
        return _chunk_text(text)
