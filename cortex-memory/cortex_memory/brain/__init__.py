"""Brain — LLM integration and task routing.

Public API for the brain subsystem.
"""

from .engine import BrainEngine
from .model_router import ModelRouter
from .ollama_client import OllamaClient
from .prompts import SystemPromptBuilder

__all__ = [
    "BrainEngine",
    "ModelRouter",
    "OllamaClient",
    "SystemPromptBuilder",
]
