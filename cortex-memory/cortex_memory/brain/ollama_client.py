"""Ollama Client — Async wrapper for the Ollama Python SDK.

Provides generation (streaming and non-streaming), model listing,
and health checking. All LLM calls go through this client.
"""

from __future__ import annotations

import logging
from collections.abc import AsyncGenerator

import ollama as ollama_sdk

from ..config import OllamaConfig

logger = logging.getLogger(__name__)


class OllamaClient:
    """Async Ollama LLM client.

    Usage::

        client = OllamaClient(OllamaConfig())
        if await client.is_available():
            response = await client.generate("Explain Rust ownership")
            async for chunk in client.generate_stream("Tell me about NATS"):
                print(chunk, end="")
    """

    def __init__(self, config: OllamaConfig) -> None:
        self._config = config
        self._client: ollama_sdk.AsyncClient | None = None

    def initialize(self) -> None:
        """Create the Ollama async client."""
        self._client = ollama_sdk.AsyncClient(host=self._config.url)
        logger.info("Ollama client initialized: %s (model: %s)", self._config.url, self._config.model)

    @property
    def client(self) -> ollama_sdk.AsyncClient:
        """Get the active Ollama client."""
        if self._client is None:
            msg = "OllamaClient not initialized — call initialize() first"
            raise RuntimeError(msg)
        return self._client

    async def generate(
        self,
        prompt: str,
        model: str | None = None,
        system: str | None = None,
    ) -> str:
        """Generate a complete response (non-streaming).

        Args:
            prompt: The user prompt.
            model: Model name override (uses config default if None).
            system: Optional system prompt.

        Returns:
            The complete generated text.
        """
        model_name = model or self._config.model

        options: dict = {}
        if system:
            options["system"] = system

        try:
            response = await self.client.generate(
                model=model_name,
                prompt=prompt,
                system=system or "",
                stream=False,
            )
            text = response.get("response", "")
            logger.info(
                "Generated %d chars with %s",
                len(text),
                model_name,
            )
            return text
        except Exception as e:
            logger.error("Ollama generation failed: %s", e)
            raise

    async def generate_stream(
        self,
        prompt: str,
        model: str | None = None,
        system: str | None = None,
    ) -> AsyncGenerator[str, None]:
        """Generate a streaming response — yields text chunks as they arrive.

        Args:
            prompt: The user prompt.
            model: Model name override.
            system: Optional system prompt.

        Yields:
            Text chunks as they are generated.
        """
        model_name = model or self._config.model

        try:
            stream = await self.client.generate(
                model=model_name,
                prompt=prompt,
                system=system or "",
                stream=True,
            )
            total_chars = 0
            async for chunk in stream:
                text = chunk.get("response", "")
                if text:
                    total_chars += len(text)
                    yield text

            logger.info("Streamed %d chars with %s", total_chars, model_name)
        except Exception as e:
            logger.error("Ollama stream failed: %s", e)
            raise

    async def chat(
        self,
        messages: list[dict[str, str]],
        model: str | None = None,
    ) -> str:
        """Chat completion with message history.

        Args:
            messages: List of {"role": "user"|"assistant"|"system", "content": "..."}.
            model: Model name override.

        Returns:
            The assistant's response text.
        """
        model_name = model or self._config.model
        try:
            response = await self.client.chat(
                model=model_name,
                messages=messages,
                stream=False,
            )
            return response.get("message", {}).get("content", "")
        except Exception as e:
            logger.error("Ollama chat failed: %s", e)
            raise

    async def list_models(self) -> list[str]:
        """List all locally available models."""
        try:
            response = await self.client.list()
            models = response.get("models", [])
            return [m.get("name", "") for m in models if m.get("name")]
        except Exception as e:
            logger.warning("Failed to list models: %s", e)
            return []

    async def pull_model(self, name: str) -> bool:
        """Pull a model if not locally available.

        Returns True if the model is now available.
        """
        try:
            logger.info("Pulling model: %s", name)
            await self.client.pull(name)
            logger.info("Model pulled successfully: %s", name)
            return True
        except Exception as e:
            logger.error("Failed to pull model %s: %s", name, e)
            return False

    async def is_available(self) -> bool:
        """Check if Ollama server is reachable and has at least one model."""
        try:
            models = await self.list_models()
            return len(models) > 0
        except Exception:
            return False

    async def ensure_model(self, model: str | None = None) -> str | None:
        """Ensure the requested model is available, pulling if needed.

        Returns the model name if available, None if not.
        """
        model_name = model or self._config.model
        models = await self.list_models()

        if model_name in models:
            return model_name

        # Try pulling
        if await self.pull_model(model_name):
            return model_name

        # Return first available model as fallback
        if models:
            logger.warning("Requested model %s unavailable, falling back to %s", model_name, models[0])
            return models[0]

        return None
