"""Cortex Memory — Main entry point.

Initializes all subsystems (Palace, Brain, NATS Bridge) and starts
serving requests from the Rust core via NATS.

Usage:
    python -m cortex_memory
"""

from __future__ import annotations

import asyncio
import logging
import sys

from .brain.engine import BrainEngine
from .brain.model_router import ModelRouter
from .brain.ollama_client import OllamaClient
from .config import CortexConfig
from .nats_bridge import NATSBridge
from .palace.knowledge_graph import KnowledgeGraph
from .palace.search import HybridSearchEngine
from .palace.storage import PalaceStorage
from .palace.vector import VectorStore

logger = logging.getLogger(__name__)


def _setup_logging() -> None:
    """Configure structured logging — JSON for production, readable for dev."""
    log_format = (
        "%(asctime)s | %(levelname)-7s | %(name)s | %(message)s"
    )
    logging.basicConfig(
        level=logging.INFO,
        format=log_format,
        datefmt="%Y-%m-%dT%H:%M:%S",
        stream=sys.stderr,
    )
    # Suppress noisy third-party loggers
    logging.getLogger("httpx").setLevel(logging.WARNING)
    logging.getLogger("chromadb").setLevel(logging.WARNING)
    logging.getLogger("urllib3").setLevel(logging.WARNING)


async def run() -> None:
    """Initialize all subsystems and start the NATS bridge."""
    config = CortexConfig.from_env()

    logger.info("═══════════════════════════════════════════")
    logger.info("  CORTEX MEMORY v0.1.0 — Starting up...")
    logger.info("═══════════════════════════════════════════")
    logger.info("  NATS:     %s", config.nats.url)
    logger.info("  Ollama:   %s (%s)", config.ollama.url, config.ollama.model)
    logger.info("  ChromaDB: %s:%d", config.chroma.host, config.chroma.port)
    logger.info("  Storage:  %s", config.storage.db_path)
    logger.info("═══════════════════════════════════════════")

    # ── Initialize Palace ────────────────────────────
    config.storage.ensure_dir()
    storage = PalaceStorage(config.storage.db_path)
    await storage.initialize()

    # Vector store (ChromaDB)
    vector_store = VectorStore(config.chroma)
    try:
        vector_store.initialize()
        logger.info("ChromaDB connected ✓")
    except Exception as e:
        logger.warning("ChromaDB unavailable (%s) — running without vector search", e)

    # Knowledge graph
    knowledge_graph = KnowledgeGraph(storage.db)
    await knowledge_graph.initialize()

    # Hybrid search engine
    search_engine = HybridSearchEngine(storage, vector_store)
    await search_engine.rebuild_bm25_index()

    # Ingest pipeline
    from .palace.ingest import IngestPipeline
    ingest_pipeline = IngestPipeline(storage, vector_store)

    # ── Initialize Brain ─────────────────────────────
    ollama_client = OllamaClient(config.ollama)
    ollama_client.initialize()

    model_router = ModelRouter(ollama_client, preferred_model=config.ollama.model)
    await model_router.initialize()

    brain_engine = BrainEngine(
        ollama=ollama_client,
        router=model_router,
        search_engine=search_engine,
        knowledge_graph=knowledge_graph,
        tools=["bash", "file_read", "file_write"],
    )

    # Health check
    health = await brain_engine.health_check()
    if health["ollama"]:
        logger.info("Ollama connected ✓ (model: %s)", health["selected_model"])
    else:
        logger.warning("Ollama unavailable — LLM features degraded")

    # ── Start NATS Bridge ────────────────────────────
    bridge = NATSBridge(
        config=config.nats,
        storage=storage,
        search_engine=search_engine,
        ingest_pipeline=ingest_pipeline,
        brain_engine=brain_engine,
        knowledge_graph=knowledge_graph,
    )

    try:
        await bridge.connect()
        logger.info("NATS bridge connected ✓")
        await bridge.serve()
    except KeyboardInterrupt:
        logger.info("Keyboard interrupt — shutting down")
    except Exception as e:
        logger.error("Fatal error: %s", e)
        raise
    finally:
        await bridge.disconnect()
        await storage.close()
        logger.info("Cortex Memory shut down cleanly")


def main() -> None:
    """Synchronous entry point."""
    _setup_logging()
    try:
        asyncio.run(run())
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
