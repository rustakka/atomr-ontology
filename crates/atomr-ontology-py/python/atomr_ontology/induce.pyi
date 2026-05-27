"""Static type stubs for atomr_ontology.induce."""
from __future__ import annotations

from typing import Awaitable, Optional

from .core import Axiom
from .extract import Backend, TermCandidate
from .provenance import Activity, ProvenanceId

class SubclassProposal:
    sub: str
    sup: str
    score: float

class ConceptCluster:
    name: str
    members: list[str]
    description: Optional[str]
    score: float

class AxiomProposal:
    kind: str
    target: str
    score: float

class TaxonomyInducer:
    def __init__(self, backend: Backend) -> None: ...
    def with_system_prompt(self, prompt: str) -> "TaxonomyInducer": ...
    def induce(self, candidate_classes: list[str]) -> Awaitable[tuple[list[SubclassProposal], Activity]]: ...
    @staticmethod
    def into_axioms(proposals: list[SubclassProposal], provenance: Optional[ProvenanceId] = ...) -> list[Axiom]: ...

class ConceptFormer:
    def __init__(self, backend: Backend) -> None: ...
    def with_system_prompt(self, prompt: str) -> "ConceptFormer": ...
    def cluster(self, terms: list[TermCandidate]) -> Awaitable[tuple[list[ConceptCluster], Activity]]: ...

class AxiomMiner:
    def __init__(self, backend: Backend) -> None: ...
    def with_system_prompt(self, prompt: str) -> "AxiomMiner": ...
    def mine(self, candidate_properties: list[str]) -> Awaitable[tuple[list[AxiomProposal], Activity]]: ...
