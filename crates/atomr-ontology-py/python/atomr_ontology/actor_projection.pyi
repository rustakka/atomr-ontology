"""Static type stubs for atomr_ontology.actor_projection.

Project actor-system persistence data (supervision paths, journal
events, serialized state) into an OntologyStore. Mirrors the Rust
crate ``atomr-ontology-actor-projection``.
"""
from __future__ import annotations

from typing import Any, Awaitable, Literal, Optional, TypedDict

class _ProjectorReport(TypedDict):
    batches: int
    nodes_written: int
    edges_written: int
    activities_recorded: int

class InMemoryActorSource:
    """Hand-built in-memory source for tests and demos."""

    def __init__(self, label: str) -> None: ...
    def push_path(self, path: str) -> None: ...
    def push_event(
        self,
        actor: str,
        kind: Literal["created", "state_changed", "completed", "terminated"] | str,
        payload: Optional[Any] = None,
        path: Optional[str] = None,
    ) -> None: ...
    def put_state(self, actor: str, payload: Any) -> None: ...
    def event_count(self) -> int: ...

class OntologyStoreHandle:
    """Opaque handle to an OntologyStore."""

    def node_count(self) -> Awaitable[int]: ...
    def edge_count(self) -> Awaitable[int]: ...
    def activity_count(self) -> Awaitable[int]: ...

class Projector:
    """Built projector. Call :meth:`run` (returns a coroutine)."""

    def run(self) -> Awaitable[_ProjectorReport]: ...

class ProjectorBuilder:
    """Fluent builder for :class:`Projector`."""

    def __init__(self) -> None: ...
    def source(self, src: InMemoryActorSource) -> None: ...
    def with_replay(self) -> None: ...
    def with_polling(self, interval_ms: int) -> None: ...
    def projection(
        self,
        name: Literal["hierarchical", "event_stream", "snapshot_diff", "flat"],
    ) -> None: ...
    def iri(
        self,
        kind: Literal["path_based", "content_addressed", "uuid"],
        base: Optional[str] = None,
    ) -> None: ...
    def conflict(
        self,
        kind: Literal["last_write_wins", "merge", "skip_existing"],
    ) -> None: ...
    def schema(self, kind: Literal["induced", "hybrid", "fixed"]) -> None: ...
    def store_from_memory(self) -> Awaitable[OntologyStoreHandle]: ...
    def set_store(self, handle: OntologyStoreHandle) -> None: ...
    def build(self) -> Projector: ...
