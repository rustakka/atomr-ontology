"""Static type stubs for atomr_ontology.shacl."""
from __future__ import annotations

from .core import Schema

def to_shacl_turtle(schema: Schema) -> str: ...
def from_shacl_turtle(input: str) -> Schema: ...
