"""Python bindings for atomr-ontology.

Build, manage, and reason over labeled property graphs and their
RDF/OWL projections. The full Rust workspace is exposed:

* :mod:`atomr_ontology.core` — LPG primitives (Iri, Node, Edge, …)
* :mod:`atomr_ontology.store` — async OntologyStore + MemStore
* :mod:`atomr_ontology.extract` — agent-driven extractors (terms,
  entities, relations, records) and the Backend abstraction
* :mod:`atomr_ontology.induce` — taxonomy induction, concept
  formation, axiom mining
* :mod:`atomr_ontology.validate` — SHACL-style shape + axiom checks
* :mod:`atomr_ontology.provenance` — PROV-O activity/entity log
* :mod:`atomr_ontology.rdf` — RDF/OWL adapter + Turtle/N-Triples/
  JSON-LD writers
* :mod:`atomr_ontology.org` — W3C Org Ontology reference vocabulary
* :mod:`atomr_ontology.testkit` — MockBackend and golden fixtures

All async APIs return Python ``Awaitable`` objects (asyncio Futures)
and can be ``await``-ed inside ``asyncio.run`` or any compatible
event loop. The asyncio runtime is initialized lazily on first use.
"""

from ._atomr_ontology import (
    core,
    embed,
    extract,
    import_,
    induce,
    org,
    persist,
    provenance,
    query,
    rdf,
    reason,
    remote,
    shacl,
    store,
    testkit,
    validate,
    version,
    viz,
)
from ._atomr_ontology import (
    AtomrOntologyError,
    AdapterError,
    BackendError,
    IriError,
    OntologyError,
    StoreError,
    ValidationError,
)

# Re-export the "prelude" — the most common types — at the top level.
from ._atomr_ontology.core import (
    Axiom,
    AxiomKind,
    Cardinality,
    Datatype,
    Edge,
    EdgeId,
    EdgeType,
    Iri,
    Namespace,
    Node,
    NodeId,
    NodeType,
    Ontology,
    Property,
    PropertyType,
    PropertyValue,
    Record,
    RecordId,
    Schema,
    Vocabulary,
)
from ._atomr_ontology.provenance import (
    Activity,
    AgentKind,
    AgentRef,
    ProvAgent,
    ProvEntity,
    ProvenanceId,
    ProvenanceLog,
)
from ._atomr_ontology.store import (
    EdgePattern,
    MatchRow,
    MemStore,
    NodePattern,
    OntologyDelta,
    StoreDiff,
    TraversalPlan,
    TraversalStep,
)
from ._atomr_ontology.extract import (
    Backend,
    EntityCandidate,
    EntityResolver,
    ExtractStage,
    Prompt,
    RecordExtractor,
    RelationCandidate,
    RelationExtractor,
    TermCandidate,
    TermExtractor,
)
from ._atomr_ontology.induce import (
    AxiomMiner,
    AxiomProposal,
    ConceptCluster,
    ConceptFormer,
    SubclassProposal,
    TaxonomyInducer,
)
from ._atomr_ontology.validate import (
    Severity,
    ValidationFinding,
    ValidationReport,
    check_consistency,
    check_shapes,
    validate as run_validate,
)
from ._atomr_ontology.rdf import (
    Class,
    DataProperty,
    Individual,
    Object,
    ObjectProperty,
    Quad,
    Subject,
    Triple,
    from_rdf,
    to_rdf,
)
from ._atomr_ontology.org import (
    FOAF_NS,
    ORG_NS,
    SCHEMA_NS,
    build_reference_vocabulary,
    reference_ontology,
)
from ._atomr_ontology.testkit import (
    MockBackend,
    assert_axiom_present,
    assert_subclass_of,
    toy_corpus,
    toy_org_ontology,
)

__all__ = [
    # submodules
    "core",
    "embed",
    "extract",
    "import_",
    "induce",
    "org",
    "persist",
    "provenance",
    "query",
    "rdf",
    "reason",
    "remote",
    "shacl",
    "store",
    "testkit",
    "validate",
    "version",
    "viz",
    # exceptions
    "AtomrOntologyError",
    "AdapterError",
    "BackendError",
    "IriError",
    "OntologyError",
    "StoreError",
    "ValidationError",
    # core
    "Axiom",
    "AxiomKind",
    "Cardinality",
    "Datatype",
    "Edge",
    "EdgeId",
    "EdgeType",
    "Iri",
    "Namespace",
    "Node",
    "NodeId",
    "NodeType",
    "Ontology",
    "Property",
    "PropertyType",
    "PropertyValue",
    "Record",
    "RecordId",
    "Schema",
    "Vocabulary",
    # provenance
    "Activity",
    "AgentKind",
    "AgentRef",
    "ProvAgent",
    "ProvEntity",
    "ProvenanceId",
    "ProvenanceLog",
    # store
    "EdgePattern",
    "MatchRow",
    "MemStore",
    "NodePattern",
    "OntologyDelta",
    "StoreDiff",
    "TraversalPlan",
    "TraversalStep",
    # extract
    "Backend",
    "EntityCandidate",
    "EntityResolver",
    "ExtractStage",
    "Prompt",
    "RecordExtractor",
    "RelationCandidate",
    "RelationExtractor",
    "TermCandidate",
    "TermExtractor",
    # induce
    "AxiomMiner",
    "AxiomProposal",
    "ConceptCluster",
    "ConceptFormer",
    "SubclassProposal",
    "TaxonomyInducer",
    # validate
    "Severity",
    "ValidationFinding",
    "ValidationReport",
    "check_consistency",
    "check_shapes",
    "run_validate",
    # rdf
    "Class",
    "DataProperty",
    "Individual",
    "Object",
    "ObjectProperty",
    "Quad",
    "Subject",
    "Triple",
    "from_rdf",
    "to_rdf",
    # org
    "FOAF_NS",
    "ORG_NS",
    "SCHEMA_NS",
    "build_reference_vocabulary",
    "reference_ontology",
    # testkit
    "MockBackend",
    "assert_axiom_present",
    "assert_subclass_of",
    "toy_corpus",
    "toy_org_ontology",
]
