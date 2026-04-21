"""Model Router — Intelligent fallback chain for model selection.

Automatically selects the best available model based on priority,
task type, and what's actually pulled on the Ollama server.
"""

from __future__ import annotations

import logging

import os

from .ollama_client import OllamaClient
from .api_clients import OpenAiClient, AnthropicClient, GeminiClient, GroqClient

logger = logging.getLogger(__name__)

# Default fallback chain — tried in order until one is found
_DEFAULT_FALLBACK_CHAIN = [
    "dolphin-mistral:latest",
    "llama3:latest",
    "mistral:latest",
    "gemma2:latest",
    "phi3:latest",
]

# Task-specific model preferences (can route heavy tasks to bigger models)
_TASK_PREFERENCES: dict[str, list[str]] = {
    "code": ["deepseek-coder:latest", "codellama:latest"],
    "reasoning": ["dolphin-mixtral:latest", "dolphin-mistral:latest"],
    "creative": ["llama3:latest", "mistral:latest"],
    "default": _DEFAULT_FALLBACK_CHAIN,
}


class ModelRouter:
    """Selects the best available model with intelligent fallback.

    Usage::

        router = ModelRouter(ollama_client, preferred_model="dolphin-mistral")
        await router.initialize()  # scans available models
        model = router.route("code")  # returns best model for code tasks
    """

    def __init__(
        self,
        client: OllamaClient,
        preferred_model: str = "dolphin-mistral:latest",
    ) -> None:
        self._client = client
        self._preferred = preferred_model
        self._available_models: list[str] = []

    async def initialize(self) -> None:
        """Scan available models from the Ollama server."""
        self._available_models = await self._client.list_models()
        logger.info("Available models: %s", self._available_models or "none")

        if not self._available_models:
            logger.warning("No models available — LLM features will be degraded")

    @property
    def available_models(self) -> list[str]:
        """Currently available models."""
        return list(self._available_models)

    def route(self, task_type: str = "default") -> str | None:
        """Select the best model for a given task type.

        Priority:
        1. User-configured preferred model
        2. Task-specific preferences
        3. Default fallback chain
        4. Any available model

        Returns None if no model is available natively AND no external API is configured.
        """
        # First, check if the user specifically forced an external model or if
        # an external API key is set and matches the preferred model.
        external_client = self._check_external_api(self._preferred)
        if external_client:
            logger.info("Routed to external API client for model: %s", self._preferred)
            return external_client

        if not self._available_models:
            return None

        # 1. Preferred model
        if self._is_available(self._preferred):
            return self._preferred

        # 2. Task-specific preferences
        task_prefs = _TASK_PREFERENCES.get(task_type, [])
        for model in task_prefs:
            if self._is_available(model):
                logger.info("Routed task '%s' to model: %s", task_type, model)
                return model

        # 3. Default fallback chain
        for model in _DEFAULT_FALLBACK_CHAIN:
            if self._is_available(model):
                logger.info("Falling back to model: %s", model)
                return model

        # 4. Any available model
        fallback = self._available_models[0]
        logger.warning("No preferred model found, using: %s", fallback)
        return fallback

    def _is_available(self, model: str) -> bool:
        """Check if a model is in the available list.

        Handles version tag matching: 'mistral' matches 'mistral:latest'.
        """
        if model in self._available_models:
            return True

        # Try without version tag
        base_name = model.split(":")[0]
        return any(m.startswith(base_name) for m in self._available_models)

    def _check_external_api(self, model: str):
        """Check if the model matches an external provider and return its client wrapper."""
        if "claude" in model.lower():
            api_key = os.getenv("ANTHROPIC_API_KEY")
            if api_key: return AnthropicClient(api_key, model)
        
        elif "gpt" in model.lower() or "o1" in model.lower():
            api_key = os.getenv("OPENAI_API_KEY")
            if api_key: return OpenAiClient(api_key, model)
            
        elif "gemini" in model.lower():
            api_key = os.getenv("GEMINI_API_KEY")
            if api_key: return GeminiClient(api_key, model)
            
        elif "mixtral" in model.lower() or "llama" in model.lower():
            # If standard open source model, check if user provided GROQ key to run it lightning fast
            api_key = os.getenv("GROQ_API_KEY")
            # Only use Groq if explicitly prefixed with 'groq:' to avoid overriding local ollama unintentionally
            if api_key and model.startswith("groq:"):
                return GroqClient(api_key, model.replace("groq:", ""))
                
        return None

    async def ensure_preferred(self) -> str | None:
        """Ensure the preferred model is available, pulling if needed."""
        if self._is_available(self._preferred):
            return self._preferred
        return await self._client.ensure_model(self._preferred)
