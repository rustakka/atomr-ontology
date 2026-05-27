"""Static type stubs for atomr_ontology.http_driver (feature: http-driver)."""
from __future__ import annotations

from .extract import Backend

class HttpDriver:
    def __init__(self, provider: str, model: str) -> None: ...
    def as_backend(self) -> Backend: ...
    @property
    def label(self) -> str: ...
