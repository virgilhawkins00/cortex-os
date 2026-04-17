"""Cortex Memory — The semantic brain of Cortex OS.

Palace: Wing → Room → Drawer → Memory storage hierarchy
Brain: Ollama LLM integration with memory-augmented generation
Bridge: NATS message bus connecting Python to Rust core
"""

__version__ = "0.1.0"

from .config import CortexConfig
from .palace import (
    HybridSearchEngine,
    IngestPipeline,
    KnowledgeGraph,
    Memory,
    PalaceStorage,
    SearchResult,
    Triple,
    VectorStore,
    Wing,
    Room,
    Drawer,
)
from .brain import (
    BrainEngine,
    ModelRouter,
    OllamaClient,
    SystemPromptBuilder,
)

__all__ = [
    "BrainEngine",
    "CortexConfig",
    "Drawer",
    "HybridSearchEngine",
    "IngestPipeline",
    "KnowledgeGraph",
    "Memory",
    "ModelRouter",
    "OllamaClient",
    "PalaceStorage",
    "Room",
    "SearchResult",
    "SystemPromptBuilder",
    "Triple",
    "VectorStore",
    "Wing",
]
