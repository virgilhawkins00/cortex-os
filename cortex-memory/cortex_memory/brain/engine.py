"""Brain Engine — The orchestrator that thinks using LLM + memory context.

This is the core intelligence loop:
1. Receive a prompt
2. Search memory for relevant context
3. Build a rich system prompt with memories + knowledge graph
4. Call the LLM (Ollama)
5. Parse the response for tool calls or direct answers
6. Return structured output
"""

from __future__ import annotations

import json
import logging
import re

from ..palace.knowledge_graph import KnowledgeGraph
from ..palace.search import HybridSearchEngine
from .model_router import ModelRouter
from .api_clients import ExternalApiClient
from .ollama_client import OllamaClient
from .prompts import SystemPromptBuilder
from .compressor import TokenBudget

logger = logging.getLogger(__name__)

# How many memories to inject into LLM context
_MAX_MEMORY_CONTEXT = 5
# Max characters per memory in context
_MAX_MEMORY_LENGTH = 500


class BrainEngine:
    """The central intelligence — combines LLM with memory retrieval.

    Usage::

        engine = BrainEngine(ollama, router, search_engine, knowledge_graph)
        result = await engine.think("What tools does Cortex OS have?")
        print(result["response"])

        async for chunk in engine.think_stream("Tell me about the sandbox"):
            print(chunk, end="")
    """

    def __init__(
        self,
        ollama: OllamaClient,
        router: ModelRouter,
        search_engine: HybridSearchEngine | None = None,
        knowledge_graph: KnowledgeGraph | None = None,
        tools: list[str] | None = None,
    ) -> None:
        self._ollama = ollama
        self._router = router
        self._search = search_engine
        self._kg = knowledge_graph
        self._tools = tools or []
        self._prompt_builder = SystemPromptBuilder()

    async def think(
        self,
        prompt: str,
        model: str | None = None,
        include_memory: bool = True,
        task_type: str = "default",
        **kwargs,
    ) -> dict:
        """Process a prompt with full memory context and LLM reasoning.

        Returns a dict with:
        - "response": The LLM's text response
        - "tool_call": Parsed tool call (if the LLM wants to use a tool)
        - "model": Which model was used
        - "memories_used": How many memories were injected
        """
        # Select model or client instance
        selected_model_or_client = model or self._router.route(task_type)
        if not selected_model_or_client:
            return {
                "response": "No LLM model accessible. Please configure Ollama or an external API key.",
                "tool_call": None,
                "model": None,
                "memories_used": 0,
            }

        # Apply Caveman compression to the main prompt to catch large tool 
        # observations sent by the Rust worker, keeping it within a strict budget
        # to save tokens when hitting external APIs.
        prompt = TokenBudget.fit_to_budget(prompt, max_tokens=1500, compress_first=True)


        # Build context
        memory_contents: list[str] = []
        kg_context: str | None = None

        if include_memory and self._search:
            try:
                results = await self._search.search(prompt, top_k=_MAX_MEMORY_CONTEXT)
                memory_contents = [
                    r.memory.content[:_MAX_MEMORY_LENGTH]
                    for r in results
                ]
            except Exception:
                logger.warning("Memory search failed during think — proceeding without context")

        if include_memory and self._kg:
            try:
                # Extract potential entity names from the prompt (simple heuristic)
                kg_context = await self._kg.export_for_llm(limit=10)
            except Exception:
                logger.warning("Knowledge graph export failed — proceeding without")

        # Build system prompt
        system_prompt = self._prompt_builder.build(
            memories=memory_contents if memory_contents else None,
            tools=self._tools if self._tools else None,
            knowledge_graph=kg_context,
            role=kwargs.get("role"),
        )

        # Call LLM based on client type
        try:
            if isinstance(selected_model_or_client, ExternalApiClient):
                response_text = await selected_model_or_client.generate(
                    prompt=prompt,
                    system_prompt=system_prompt,
                )
                model_name = selected_model_or_client.model_id
            else:
                response_text = await self._ollama.generate(
                    prompt=prompt,
                    model=selected_model_or_client,
                    system=system_prompt,
                )
                model_name = selected_model_or_client
        except Exception as e:
            logger.error("LLM generation failed: %s", e)
            return {
                "response": f"LLM error: {e}",
                "tool_call": None,
                "model": getattr(selected_model_or_client, "model_id", selected_model_or_client),
                "memories_used": len(memory_contents),
            }

        # Parse for tool calls
        tool_call = self._parse_tool_call(response_text)

        logger.info(
            "Brain.think complete: model=%s, memories=%d, tool_call=%s",
            model_name,
            len(memory_contents),
            tool_call.get("tool") if tool_call else None,
        )

        return {
            "response": response_text,
            "tool_call": tool_call,
            "model": model_name,
            "memories_used": len(memory_contents),
        }

    async def think_stream(
        self,
        prompt: str,
        model: str | None = None,
        include_memory: bool = True,
    ):
        """Stream a response with memory context.

        Yields text chunks as the LLM generates them.
        """
        selected_model = model or self._router.route()
        if not selected_model:
            yield "No LLM model available."
            return

        # Build context (same as think)
        memory_contents: list[str] = []
        if include_memory and self._search:
            try:
                results = await self._search.search(prompt, top_k=_MAX_MEMORY_CONTEXT)
                memory_contents = [r.memory.content[:_MAX_MEMORY_LENGTH] for r in results]
            except Exception:
                pass

        system_prompt = self._prompt_builder.build(
            memories=memory_contents if memory_contents else None,
            tools=self._tools if self._tools else None,
        )

        async for chunk in self._ollama.generate_stream(
            prompt=prompt,
            model=selected_model,
            system=system_prompt,
        ):
            yield chunk

    async def health_check(self) -> dict:
        """Check health of all brain subsystems."""
        ollama_ok = await self._ollama.is_available()
        models = await self._ollama.list_models() if ollama_ok else []

        return {
            "ollama": ollama_ok,
            "models": models,
            "selected_model": self._router.route(),
            "memory_search": self._search is not None,
            "knowledge_graph": self._kg is not None,
        }

    @staticmethod
    def _parse_tool_call(text: str) -> dict | None:
        """Extract a tool call JSON from LLM response.

        Looks for ```json ... ``` blocks containing a "tool" key.
        Returns the parsed dict or None if no tool call found.
        """
        # Try to find JSON code blocks
        json_pattern = r"```(?:json)?\s*\n?(\{[^`]*\})\s*\n?```"
        matches = re.findall(json_pattern, text, re.DOTALL)

        for match in matches:
            try:
                parsed = json.loads(match)
                if "tool" in parsed:
                    return {
                        "tool": parsed["tool"],
                        "args": parsed.get("args", {}),
                        "reasoning": parsed.get("reasoning", ""),
                    }
            except json.JSONDecodeError:
                continue

        # Try to find inline JSON with "tool" key
        inline_pattern = r'\{\s*"tool"\s*:\s*"[^"]+"\s*[,}]'
        match = re.search(inline_pattern, text)
        if match:
            # Find the complete JSON object
            start = match.start()
            brace_count = 0
            end = start
            for i in range(start, len(text)):
                if text[i] == "{":
                    brace_count += 1
                elif text[i] == "}":
                    brace_count -= 1
                    if brace_count == 0:
                        end = i + 1
                        break

            try:
                parsed = json.loads(text[start:end])
                if "tool" in parsed:
                    return {
                        "tool": parsed["tool"],
                        "args": parsed.get("args", {}),
                        "reasoning": parsed.get("reasoning", ""),
                    }
            except json.JSONDecodeError:
                pass

        return None
