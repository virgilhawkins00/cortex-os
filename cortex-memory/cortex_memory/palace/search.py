"""Hybrid Search — BM25 full-text + vector cosine similarity with Reciprocal Rank Fusion.

Combines lexical matching (BM25) with semantic understanding (vector embeddings)
to achieve the best of both worlds. Exact keyword matches score well AND
semantically similar but differently-worded content also surfaces.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass

from rank_bm25 import BM25Okapi

from .models import Memory, SearchResult
from .storage import PalaceStorage
from .vector import VectorStore

logger = logging.getLogger(__name__)

# Reciprocal Rank Fusion constant (standard value from literature)
_RRF_K = 60


@dataclass
class SearchConfig:
    """Configuration for hybrid search behavior."""

    bm25_weight: float = 0.4
    vector_weight: float = 0.6
    top_k: int = 10


class HybridSearchEngine:
    """Hybrid search combining BM25 lexical search with ChromaDB vector search.

    The two result sets are merged using Reciprocal Rank Fusion (RRF), which
    doesn't require score normalization and handles different score scales well.

    Usage::

        engine = HybridSearchEngine(storage, vector_store)
        results = await engine.search("how does the sandbox work?", wing="projects")
    """

    def __init__(
        self,
        storage: PalaceStorage,
        vector_store: VectorStore,
        config: SearchConfig | None = None,
    ) -> None:
        self._storage = storage
        self._vector_store = vector_store
        self._config = config or SearchConfig()
        self._bm25_index: BM25Okapi | None = None
        self._bm25_doc_ids: list[str] = []

    async def rebuild_bm25_index(self) -> None:
        """Rebuild the in-memory BM25 index from all stored memories.

        Call this after bulk inserts or periodically to keep the index fresh.
        For incremental updates, the FTS5 search in SQLite is used as fallback.
        """
        documents = await self._storage.get_all_contents()
        if not documents:
            self._bm25_index = None
            self._bm25_doc_ids = []
            logger.info("BM25 index: empty (no documents)")
            return

        self._bm25_doc_ids = [doc_id for doc_id, _ in documents]
        tokenized = [content.lower().split() for _, content in documents]
        self._bm25_index = BM25Okapi(tokenized)
        logger.info("BM25 index rebuilt with %d documents", len(documents))

    async def search(
        self,
        query: str,
        wing: str | None = None,
        top_k: int | None = None,
    ) -> list[SearchResult]:
        """Execute hybrid search and return ranked results.

        1. Run BM25 over the full corpus (or FTS5 fallback)
        2. Run vector similarity search via ChromaDB
        3. Merge with Reciprocal Rank Fusion
        4. Fetch full Memory objects for top-k results
        """
        k = top_k or self._config.top_k
        if not query.strip():
            return []

        # ── BM25 search ──────────────────────────────
        bm25_ranking = await self._bm25_search(query, k * 2)

        # ── Vector search ────────────────────────────
        vector_ranking = self._vector_search(query, wing, k * 2)

        # ── Reciprocal Rank Fusion ───────────────────
        fused = self._rrf_merge(bm25_ranking, vector_ranking)

        # ── Fetch full Memory objects ────────────────
        results: list[SearchResult] = []
        for memory_id, score in fused[:k]:
            memory = await self._storage.get_memory(memory_id)
            if memory:
                results.append(SearchResult(
                    memory=memory,
                    score=score,
                    source="hybrid",
                ))

        logger.info(
            "Hybrid search for '%s': %d results (bm25=%d, vector=%d)",
            query[:50],
            len(results),
            len(bm25_ranking),
            len(vector_ranking),
        )
        return results

    async def _bm25_search(self, query: str, top_k: int) -> list[tuple[str, float]]:
        """Run BM25 search. Falls back to FTS5 if index not built."""
        if self._bm25_index is not None and self._bm25_doc_ids:
            tokenized_query = query.lower().split()
            scores = self._bm25_index.get_scores(tokenized_query)

            # Pair document IDs with scores, filter out zeros
            scored_docs = [
                (self._bm25_doc_ids[i], float(scores[i]))
                for i in range(len(scores))
                if scores[i] > 0
            ]
            scored_docs.sort(key=lambda x: x[1], reverse=True)
            return scored_docs[:top_k]

        # Fallback: FTS5 search in SQLite
        fts_results = await self._storage.search_fts(query, limit=top_k)
        return [
            (m.id, 1.0 / (i + 1))  # Assign rank-based scores
            for i, m in enumerate(fts_results)
        ]

    def _vector_search(
        self, query: str, wing: str | None, top_k: int
    ) -> list[tuple[str, float]]:
        """Run vector similarity search via ChromaDB."""
        try:
            if wing:
                return self._vector_store.search_similar(query, wing=wing, top_k=top_k)
            return self._vector_store.search_similar_all_wings(query, top_k=top_k)
        except Exception:
            logger.warning("Vector search failed, returning empty results")
            return []

    def _rrf_merge(
        self,
        bm25_results: list[tuple[str, float]],
        vector_results: list[tuple[str, float]],
    ) -> list[tuple[str, float]]:
        """Merge two ranked lists using Reciprocal Rank Fusion.

        RRF score = Σ (weight / (k + rank)) for each ranker.
        This handles different score scales gracefully.
        """
        scores: dict[str, float] = {}

        for rank, (doc_id, _) in enumerate(bm25_results):
            rrf_score = self._config.bm25_weight / (_RRF_K + rank + 1)
            scores[doc_id] = scores.get(doc_id, 0.0) + rrf_score

        for rank, (doc_id, _) in enumerate(vector_results):
            rrf_score = self._config.vector_weight / (_RRF_K + rank + 1)
            scores[doc_id] = scores.get(doc_id, 0.0) + rrf_score

        merged = sorted(scores.items(), key=lambda x: x[1], reverse=True)
        return merged
