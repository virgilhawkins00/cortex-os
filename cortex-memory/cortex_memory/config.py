"""Cortex Memory — Configuration loaded from environment variables."""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from pathlib import Path

from dotenv import load_dotenv

load_dotenv()


@dataclass(frozen=True)
class NATSConfig:
    """NATS message bus connection settings."""

    url: str = field(default_factory=lambda: os.getenv("NATS_URL", "nats://127.0.0.1:4222"))
    token: str | None = field(default_factory=lambda: os.getenv("NATS_AUTH_TOKEN"))


@dataclass(frozen=True)
class OllamaConfig:
    """Ollama LLM server settings."""

    url: str = field(default_factory=lambda: os.getenv("OLLAMA_URL", "http://127.0.0.1:11434"))
    model: str = field(
        default_factory=lambda: os.getenv("OLLAMA_MODEL", "dolphin-mistral:latest")
    )
    timeout: int = field(default_factory=lambda: int(os.getenv("OLLAMA_TIMEOUT", "120")))


@dataclass(frozen=True)
class ChromaConfig:
    """ChromaDB vector database settings."""

    host: str = field(default_factory=lambda: os.getenv("CHROMA_HOST", "127.0.0.1"))
    port: int = field(default_factory=lambda: int(os.getenv("CHROMA_PORT", "8000")))
    collection_prefix: str = field(
        default_factory=lambda: os.getenv("CHROMA_COLLECTION_PREFIX", "cortex_")
    )


@dataclass(frozen=True)
class StorageConfig:
    """Palace storage settings."""

    db_path: Path = field(
        default_factory=lambda: Path(os.getenv("CORTEX_MEMORY_DB_PATH", "./data/cortex.db"))
    )

    def ensure_dir(self) -> None:
        """Create the database directory if it doesn't exist."""
        self.db_path.parent.mkdir(parents=True, exist_ok=True)


@dataclass(frozen=True)
class CortexConfig:
    """Root configuration aggregating all subsystem configs."""

    nats: NATSConfig = field(default_factory=NATSConfig)
    ollama: OllamaConfig = field(default_factory=OllamaConfig)
    chroma: ChromaConfig = field(default_factory=ChromaConfig)
    storage: StorageConfig = field(default_factory=StorageConfig)

    @classmethod
    def from_env(cls) -> CortexConfig:
        """Load all configuration from environment variables."""
        return cls()
