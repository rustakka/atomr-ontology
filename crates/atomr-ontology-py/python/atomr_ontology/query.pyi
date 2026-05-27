"""Static type stubs for atomr_ontology.query."""
from __future__ import annotations

from .store import TraversalPlan

def parse_cypher(query: str) -> TraversalPlan: ...
def parse_sparql(query: str) -> TraversalPlan: ...
