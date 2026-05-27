"""Static type stubs for atomr_ontology.remote."""
from __future__ import annotations

class RemoteClient:
    def __init__(self, base_url: str) -> None: ...
    @property
    def base_url(self) -> str: ...
