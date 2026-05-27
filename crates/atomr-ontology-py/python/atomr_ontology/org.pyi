"""Static type stubs for atomr_ontology.org."""
from __future__ import annotations

from .core import Ontology, Vocabulary

ORG_NS: str
FOAF_NS: str
SCHEMA_NS: str

def reference_ontology() -> Ontology: ...
def build_reference_vocabulary() -> Vocabulary: ...
