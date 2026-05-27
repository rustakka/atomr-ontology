"""Top-level re-exports for atomr_ontology."""
from . import core as core
from . import extract as extract
from . import induce as induce
from . import org as org
from . import provenance as provenance
from . import rdf as rdf
from . import store as store
from . import testkit as testkit
from . import validate as validate

# Exceptions (registered by errors::register on the package root).
class AtomrOntologyError(Exception): ...
class AdapterError(AtomrOntologyError): ...
class BackendError(AtomrOntologyError): ...
class IriError(AtomrOntologyError): ...
class OntologyError(AtomrOntologyError): ...
class StoreError(AtomrOntologyError): ...
class ValidationError(AtomrOntologyError): ...

# Re-exports.
from .core import (
    Axiom as Axiom,
    AxiomKind as AxiomKind,
    Cardinality as Cardinality,
    Datatype as Datatype,
    Edge as Edge,
    EdgeId as EdgeId,
    EdgeType as EdgeType,
    Iri as Iri,
    Namespace as Namespace,
    Node as Node,
    NodeId as NodeId,
    NodeType as NodeType,
    Ontology as Ontology,
    Property as Property,
    PropertyType as PropertyType,
    PropertyValue as PropertyValue,
    Record as Record,
    RecordId as RecordId,
    Schema as Schema,
    Vocabulary as Vocabulary,
)
from .provenance import (
    Activity as Activity,
    AgentKind as AgentKind,
    AgentRef as AgentRef,
    ProvAgent as ProvAgent,
    ProvEntity as ProvEntity,
    ProvenanceId as ProvenanceId,
    ProvenanceLog as ProvenanceLog,
)
from .store import (
    EdgePattern as EdgePattern,
    MatchRow as MatchRow,
    MemStore as MemStore,
    NodePattern as NodePattern,
    OntologyDelta as OntologyDelta,
    StoreDiff as StoreDiff,
    TraversalPlan as TraversalPlan,
    TraversalStep as TraversalStep,
)
from .extract import (
    Backend as Backend,
    EntityCandidate as EntityCandidate,
    EntityResolver as EntityResolver,
    ExtractStage as ExtractStage,
    Prompt as Prompt,
    RecordExtractor as RecordExtractor,
    RelationCandidate as RelationCandidate,
    RelationExtractor as RelationExtractor,
    TermCandidate as TermCandidate,
    TermExtractor as TermExtractor,
)
from .induce import (
    AxiomMiner as AxiomMiner,
    AxiomProposal as AxiomProposal,
    ConceptCluster as ConceptCluster,
    ConceptFormer as ConceptFormer,
    SubclassProposal as SubclassProposal,
    TaxonomyInducer as TaxonomyInducer,
)
from .validate import (
    Severity as Severity,
    ValidationFinding as ValidationFinding,
    ValidationReport as ValidationReport,
    check_consistency as check_consistency,
    check_shapes as check_shapes,
)
from .validate import validate as run_validate
from .rdf import (
    Class as Class,
    DataProperty as DataProperty,
    Individual as Individual,
    Object as Object,
    ObjectProperty as ObjectProperty,
    Quad as Quad,
    Subject as Subject,
    Triple as Triple,
    from_rdf as from_rdf,
    to_rdf as to_rdf,
)
from .org import (
    FOAF_NS as FOAF_NS,
    ORG_NS as ORG_NS,
    SCHEMA_NS as SCHEMA_NS,
    build_reference_vocabulary as build_reference_vocabulary,
    reference_ontology as reference_ontology,
)
from .testkit import (
    MockBackend as MockBackend,
    assert_axiom_present as assert_axiom_present,
    assert_subclass_of as assert_subclass_of,
    toy_corpus as toy_corpus,
    toy_org_ontology as toy_org_ontology,
)
