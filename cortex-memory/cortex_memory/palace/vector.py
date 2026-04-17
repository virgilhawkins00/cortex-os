"""Vector Store — ChromaDB backend for semantic embeddings.

Each Wing gets its own ChromaDB collection for isolation. Memories are
embedded using ChromaDB's built-in sentence-transformers model.
"""

from __future__ import annotations

import logging
from typing import Any

import chromadb

from ..config import ChromaConfig

logger = logging.getLogger(__name__)


class VectorStore:
    """ChromaDB wrapper for vector similarity search.

    Usage::

        store = VectorStore(ChromaConfig())
        store.initialize()
        store.add_embedding("mem_123", "The sandbox timeout is 30 seconds", wing="projects")
        results = store.search_similar("how long before timeout?", wing="projects")
    """

    def __init__(self, config: ChromaConfig) -> None:
        self._config = config
        self._client: chromadb.ClientAPI | None = None

    def initialize(self) -> None:
        """Connect to ChromaDB server."""
        self._client = chromadb.HttpClient(
            host=self._config.host,
            port=self._config.port,
        )
        logger.info("Connected to ChromaDB at %s:%d", self._config.host, self._config.port)

    @property
    def client(self) -> chromadb.ClientAPI:
        """Get the active ChromaDB client."""
        if self._client is None:
            msg = "VectorStore not initialized — call initialize() first"
            raise RuntimeError(msg)
        return self._client

    def _collection_name(self, wing: str) -> str:
        """Generate a collection name for a wing."""
        # Sanitize: ChromaDB collection names must be 3-63 chars, alphanumeric + underscores
        safe_name = wing.lower().replace("-", "_").replace(" ", "_")
        return f"{self._config.collection_prefix}{safe_name}"

    def _get_collection(self, wing: str) -> chromadb.Collection:
        """Get or create a collection for a wing."""
        name = self._collection_name(wing)
        return self.client.get_or_create_collection(
            name=name,
            metadata={"hnsw:space": "cosine"},
        )

    def add_embedding(
        self,
        memory_id: str,
        text: str,
        wing: str,
        metadata: dict[str, Any] | None = None,
    ) -> str:
        """Embed text and store in ChromaDB.

        ChromaDB handles the embedding via its default model (all-MiniLM-L6-v2).
        Returns the embedding ID (same as memory_id for 1:1 mapping).
        """
        collection = self._get_collection(wing)

        doc_metadata = {"wing": wing}
        if metadata:
            # ChromaDB metadata values must be str, int, float, or bool
            for k, v in metadata.items():
                if isinstance(v, (str, int, float, bool)):
                    doc_metadata[k] = v

        collection.upsert(
            ids=[memory_id],
            documents=[text],
            metadatas=[doc_metadata],
        )
        logger.info("Embedded memory %s in wing '%s'", memory_id[:8], wing)
        return memory_id

    def add_embeddings_batch(
        self,
        items: list[tuple[str, str, dict[str, Any] | None]],
        wing: str,
    ) -> None:
        """Batch embed multiple texts.

        Args:
            items: List of (memory_id, text, metadata) tuples.
            wing: The wing to store in.
        """
        if not items:
            return

        collection = self._get_collection(wing)
        ids = [item[0] for item in items]
        documents = [item[1] for item in items]
        metadatas = []
        for item in items:
            meta = {"wing": wing}
            if item[2]:
                for k, v in item[2].items():
                    if isinstance(v, (str, int, float, bool)):
                        meta[k] = v
            metadatas.append(meta)

        collection.upsert(ids=ids, documents=documents, metadatas=metadatas)
        logger.info("Batch embedded %d memories in wing '%s'", len(items), wing)

    def search_similar(
        self,
        query: str,
        wing: str,
        top_k: int = 10,
    ) -> list[tuple[str, float]]:
        """Search for similar memories by semantic similarity.

        Returns list of (memory_id, distance_score) tuples.
        Lower distance = more similar (cosine distance).
        """
        collection = self._get_collection(wing)

        try:
            count = collection.count()
        except Exception:
            count = 0

        if count == 0:
            return []

        # Don't request more results than exist
        actual_k = min(top_k, count)

        results = collection.query(
            query_texts=[query],
            n_results=actual_k,
        )

        pairs: list[tuple[str, float]] = []
        if results["ids"] and results["distances"]:
            for mid, dist in zip(results["ids"][0], results["distances"][0]):
                # Convert cosine distance to similarity score (0-1)
                similarity = 1.0 - dist
                pairs.append((mid, similarity))

        return pairs

    def search_similar_all_wings(
        self,
        query: str,
        top_k: int = 10,
    ) -> list[tuple[str, float]]:
        """Search across all wings. Returns merged results sorted by similarity."""
        all_results: list[tuple[str, float]] = []

        for collection in self.client.list_collections():
            if not collection.name.startswith(self._config.collection_prefix):
                continue
            try:
                count = collection.count()
                if count == 0:
                    continue
                actual_k = min(top_k, count)
                results = collection.query(query_texts=[query], n_results=actual_k)
                if results["ids"] and results["distances"]:
                    for mid, dist in zip(results["ids"][0], results["distances"][0]):
                        all_results.append((mid, 1.0 - dist))
            except Exception:
                logger.warning("Failed to search collection %s", collection.name)

        all_results.sort(key=lambda x: x[1], reverse=True)
        return all_results[:top_k]

    def delete_embedding(self, memory_id: str, wing: str) -> None:
        """Remove an embedding from ChromaDB."""
        collection = self._get_collection(wing)
        collection.delete(ids=[memory_id])

    def health_check(self) -> bool:
        """Check if ChromaDB is reachable."""
        try:
            self.client.heartbeat()
            return True
        except Exception:
            return False
