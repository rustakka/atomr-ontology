"""Static type stubs for atomr_ontology.validate."""
from __future__ import annotations

from .core import Ontology

class Severity:
    @classmethod
    def info(cls) -> "Severity": ...
    @classmethod
    def warning(cls) -> "Severity": ...
    @classmethod
    def violation(cls) -> "Severity": ...
    @property
    def name(self) -> str: ...

class ValidationFinding:
    severity: Severity
    code: str
    message: str
    subject: str

class ValidationReport:
    findings: list[ValidationFinding]
    def is_clean(self) -> bool: ...

def validate(ontology: Ontology) -> ValidationReport: ...
def check_shapes(ontology: Ontology) -> ValidationReport: ...
def check_consistency(ontology: Ontology) -> ValidationReport: ...
