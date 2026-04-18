"""NATS Bridge — Connects the Python brain to the Rust core via NATS message bus.

Subscribes to cortex.memory.* and cortex.brain.* subjects, processes
requests, and publishes replies using the NATS request/reply pattern.
This is the nervous system connecting Python intelligence to Rust execution.
"""

from __future__ import annotations

import asyncio
import json
import logging
import signal
from typing import Any
from uuid import uuid4

import nats
from nats.aio.client import Client as NATSClient

from .brain.engine import BrainEngine
from .config import NATSConfig
from .palace.ingest import IngestPipeline
from .palace.knowledge_graph import KnowledgeGraph
from .palace.search import HybridSearchEngine
from .palace.storage import PalaceStorage

logger = logging.getLogger(__name__)


def _ok_response(output: Any, request_id: str | None = None) -> bytes:
    """Build a success response matching Rust's TaskResult serde format."""
    result = {
        "id": request_id or uuid4().hex,
        "status": "success",
        "output": json.dumps(output) if not isinstance(output, str) else output,
        "error": None,
    }
    return json.dumps(result).encode()


def _err_response(error: str, request_id: str | None = None) -> bytes:
    """Build an error response matching Rust's TaskResult serde format."""
    result = {
        "id": request_id or uuid4().hex,
        "status": "error",
        "output": "",
        "error": error,
    }
    return json.dumps(result).encode()


class NATSBridge:
    """The NATS bridge service — listens for requests from Rust and replies.

    Subjects handled:
    - cortex.memory.store — store a new memory
    - cortex.memory.search — hybrid search
    - cortex.memory.ingest — ingest raw text
    - cortex.brain.think — LLM generation with memory context
    - cortex.brain.health — health check all subsystems

    Usage::

        bridge = NATSBridge(config, storage, search, ingest, brain, kg)
        await bridge.connect()
        await bridge.serve()  # blocks until shutdown
    """

    def __init__(
        self,
        config: NATSConfig,
        storage: PalaceStorage,
        search_engine: HybridSearchEngine,
        ingest_pipeline: IngestPipeline,
        brain_engine: BrainEngine,
        knowledge_graph: KnowledgeGraph,
    ) -> None:
        self._config = config
        self._storage = storage
        self._search = search_engine
        self._ingest = ingest_pipeline
        self._brain = brain_engine
        self._kg = knowledge_graph
        self._nc: NATSClient | None = None
        self._shutdown_event = asyncio.Event()

    async def connect(self) -> None:
        """Connect to the NATS server with optional mTLS."""
        connect_opts: dict[str, Any] = {"servers": [self._config.url]}
        if self._config.token:
            connect_opts["token"] = self._config.token

        if self._config.ca_path or self._config.cert_path:
            import ssl
            logger.info("Configuring SSL/TLS for NATS bridge...")
            ssl_ctx = ssl.create_default_context(purpose=ssl.Purpose.SERVER_AUTH)
            if self._config.ca_path:
                ssl_ctx.load_verify_locations(cafile=self._config.ca_path)
            if self._config.cert_path and self._config.key_path:
                ssl_ctx.load_cert_chain(certfile=self._config.cert_path, keyfile=self._config.key_path)
            connect_opts["tls"] = ssl_ctx

        self._nc = await nats.connect(**connect_opts)
        logger.info("NATS bridge connected to %s (mTLS: %s)", self._config.url, "tls" in connect_opts)

    async def disconnect(self) -> None:
        """Gracefully disconnect from NATS."""
        if self._nc and self._nc.is_connected:
            await self._nc.drain()
            await self._nc.close()
            logger.info("NATS bridge disconnected")

    @property
    def nc(self) -> NATSClient:
        """Get the active NATS connection."""
        if self._nc is None or not self._nc.is_connected:
            msg = "NATS bridge not connected"
            raise RuntimeError(msg)
        return self._nc

    async def serve(self) -> None:
        """Start serving requests — blocks until shutdown signal.

        Sets up subscriptions for all cortex.* subjects and processes
        incoming requests using the NATS request/reply pattern.
        """
        # Register signal handlers
        loop = asyncio.get_running_loop()
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, self._signal_shutdown)

        # Subscribe to all subjects
        await self.nc.subscribe("cortex.memory.store", cb=self._handle_memory_store)
        await self.nc.subscribe("cortex.memory.search", cb=self._handle_memory_search)
        await self.nc.subscribe("cortex.memory.list", cb=self._handle_memory_list)
        await self.nc.subscribe("cortex.memory.delete", cb=self._handle_memory_delete)
        await self.nc.subscribe("cortex.memory.ingest", cb=self._handle_memory_ingest)
        await self.nc.subscribe("cortex.brain.think", cb=self._handle_brain_think)
        await self.nc.subscribe("cortex.brain.health", cb=self._handle_brain_health)
        await self.nc.subscribe("cortex.audit.log", cb=self._handle_audit_log)

        logger.info("NATS bridge serving — listening on cortex.memory.* and cortex.brain.*")

        # Block until shutdown
        await self._shutdown_event.wait()
        logger.info("NATS bridge shutting down...")
        await self.disconnect()

    def _signal_shutdown(self) -> None:
        """Handle shutdown signals."""
        logger.info("Shutdown signal received")
        self._shutdown_event.set()

    # ── Handlers ──────────────────────────────────────────────

    async def _handle_memory_store(self, msg: nats.aio.client.Msg) -> None:
        """Handle cortex.memory.store — store a new memory."""
        try:
            data = json.loads(msg.data)
            content = data.get("content", "")
            wing = data.get("wing", "general")
            room = data.get("room", "default")
            metadata = data.get("metadata", {})

            if not content:
                await msg.respond(_err_response("Missing 'content' field"))
                return

            memory = await self._ingest._storage.store_memory(
                content=content,
                wing_id=(await self._storage.get_or_create_wing(wing)).id,
                room_id=(await self._storage.get_or_create_room(
                    (await self._storage.get_or_create_wing(wing)).id, room
                )).id,
                metadata=metadata,
            )

            response = {
                "memory_id": memory.id,
                "content": memory.content,
                "wing": wing,
                "room": room,
            }
            await msg.respond(_ok_response(response))
            logger.info("Stored memory %s via NATS", memory.id[:8])

        except Exception as e:
            logger.error("memory.store failed: %s", e)
            await msg.respond(_err_response(str(e)))

    async def _handle_memory_search(self, msg: nats.aio.client.Msg) -> None:
        """Handle cortex.memory.search — hybrid search."""
        try:
            data = json.loads(msg.data)
            query = data.get("query", "")
            top_k = data.get("top_k", 5)
            wing = data.get("wing")

            if not query:
                await msg.respond(_err_response("Missing 'query' field"))
                return

            results = await self._search.search(query, wing=wing, top_k=top_k)

            response = {
                "results": [
                    {
                        "memory_id": r.memory.id,
                        "content": r.memory.content,
                        "score": r.score,
                        "source": r.source,
                    }
                    for r in results
                ],
                "total": len(results),
            }
            await msg.respond(_ok_response(response))

        except Exception as e:
            logger.error("memory.search failed: %s", e)
            await msg.respond(_err_response(str(e)))

    async def _handle_memory_list(self, msg: nats.aio.client.Msg) -> None:
        """Handle cortex.memory.list — list memories."""
        try:
            data = json.loads(msg.data)
            wing = data.get("wing")
            room = data.get("room")
            limit = data.get("limit", 50)

            memories = await self._storage.list_memories(
                wing_id=(await self._storage.get_wing(wing)).id if wing else None,
                room_id=(await self._storage.get_room((await self._storage.get_wing(wing)).id if wing else None, room)).id if (wing and room) else None,
                limit=limit,
            )

            response = {
                "memories": [
                    {
                        "id": m.id,
                        "content": m.content,
                        "wing_id": m.wing_id,
                        "room_id": m.room_id,
                        "created_at": m.created_at.isoformat(),
                    }
                    for m in memories
                ],
                "count": len(memories),
            }
            await msg.respond(_ok_response(response))

        except Exception as e:
            logger.error("memory.list failed: %s", e)
            await msg.respond(_err_response(str(e)))

    async def _handle_memory_delete(self, msg: nats.aio.client.Msg) -> None:
        """Handle cortex.memory.delete — delete a memory."""
        try:
            data = json.loads(msg.data)
            memory_id = data.get("memory_id")

            if not memory_id:
                await msg.respond(_err_response("Missing 'memory_id' field"))
                return

            success = await self._storage.delete_memory(memory_id)
            await msg.respond(_ok_response({"success": success}))
            logger.info("Deleted memory %s via NATS", memory_id[:8])

        except Exception as e:
            logger.error("memory.delete failed: %s", e)
            await msg.respond(_err_response(str(e)))

    async def _handle_memory_ingest(self, msg: nats.aio.client.Msg) -> None:
        """Handle cortex.memory.ingest — ingest raw text."""
        try:
            data = json.loads(msg.data)
            text = data.get("text", "")
            wing = data.get("wing", "general")
            room = data.get("room", "default")
            metadata = data.get("metadata", {})

            if not text:
                await msg.respond(_err_response("Missing 'text' field"))
                return

            memories = await self._ingest.ingest(
                text=text,
                wing=wing,
                room=room,
                metadata=metadata,
            )

            response = {
                "ingested": len(memories),
                "memory_ids": [m.id for m in memories],
            }
            await msg.respond(_ok_response(response))
            logger.info("Ingested %d memories via NATS", len(memories))

        except Exception as e:
            logger.error("memory.ingest failed: %s", e)
            await msg.respond(_err_response(str(e)))

    async def _handle_brain_think(self, msg: nats.aio.client.Msg) -> None:
        """Handle cortex.brain.think — LLM generation with memory context."""
        try:
            data = json.loads(msg.data)
            prompt = data.get("prompt", "")
            model = data.get("model")
            include_memory = data.get("include_memory", True)

            if not prompt:
                await msg.respond(_err_response("Missing 'prompt' field"))
                return

            result = await self._brain.think(
                prompt=prompt,
                model=model,
                include_memory=include_memory,
                role=data.get("role"),
            )

            response = {
                "response": result["response"],
                "model": result["model"],
                "memories_used": result["memories_used"],
                "tool_call": result["tool_call"],
                "metadata": data.get("metadata"),
            }
            await msg.respond(_ok_response(response))
            logger.info(
                "Brain.think via NATS: model=%s, memories=%d",
                result["model"],
                result["memories_used"],
            )

        except Exception as e:
            logger.error("brain.think failed: %s", e)
            await msg.respond(_err_response(str(e)))

    async def _handle_brain_health(self, msg: nats.aio.client.Msg) -> None:
        """Handle cortex.brain.health — health check all subsystems."""
        try:
            brain_health = await self._brain.health_check()
            memory_count = await self._storage.count_memories()
            triple_count = await self._kg.count_triples()

            response = {
                "status": "healthy",
                "ollama": brain_health["ollama"],
                "models": brain_health["models"],
                "selected_model": brain_health["selected_model"],
                "memories": memory_count,
                "triples": triple_count,
            }
            await msg.respond(_ok_response(response))

        except Exception as e:
            logger.error("brain.health failed: %s", e)
            await msg.respond(_err_response(str(e)))

    async def _handle_audit_log(self, msg: nats.aio.client.Msg) -> None:
        """Handle cortex.audit.log — persist an audit event."""
        try:
            data = json.loads(msg.data)
            component = data.get("component", "unknown")
            event_type = data.get("event", "unknown")
            metadata = data.get("metadata", {})
            user_id = data.get("user_id")

            await self._storage.store_audit_log(
                component=component,
                event_type=event_type,
                metadata=metadata,
                user_id=user_id,
            )
        except Exception as e:
            logger.error("audit.log failed: %s", e)
