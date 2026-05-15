# Naming

`atomr-ontology` ships two parallel vocabularies: a canonical
labeled-property-graph vocabulary used everywhere in user code,
and an RDF/OWL/PROV-O vocabulary used on the projection side.
This document is the authoritative mapping.

## LPG ↔ RDF/OWL

| LPG term | RDF / OWL term | Notes |
| --- | --- | --- |
| `Ontology` | (no direct analogue) | The graph + schema + axioms taken as a snapshot. |
| `Iri` | `IRI` | RFC 3987; not normalized at construction time. |
| `Namespace` | `@prefix` binding | Held as `{prefix, base}` pairs in `Vocabulary`. |
| `Vocabulary` | prefix map | Plain `BTreeMap<String, Iri>`. |
| `Schema` | T-Box | Container of `NodeType`/`EdgeType`. |
| `NodeType` | `owl:Class` | Plus optional supertypes, properties, IRI. |
| `EdgeType` | `owl:ObjectProperty` | Plus domain, range, cardinality, inverse, functional. |
| `PropertyType` | `owl:DatatypeProperty` (or `owl:AnnotationProperty` when untyped) | Plus `xsd:*` range. |
| `Datatype::*` | `xsd:*` | See `datatype-table` below. |
| `Cardinality` | `owl:minCardinality` / `owl:maxCardinality` / `sh:minCount` / `sh:maxCount` | We store both bounds in one struct. |
| `Node` | individual (IRI-named subject) or blank node | Carries `iri` (optional) and `types: Vec<String>`. |
| `Edge` | `<subject> <predicate> <object>` triple | `label` is the predicate name. |
| `Property` | datatype-property assertion | One row in `Node::properties`. |
| `PropertyValue` | typed literal | See `datatype-table`. |
| `Record` | (no direct analogue) | Flat snapshot used during ingestion; resolves into nodes + edges at commit. |
| `Axiom::SubClassOf` | `rdfs:subClassOf` | |
| `Axiom::EquivalentClass` | `owl:equivalentClass` | |
| `Axiom::DisjointWith` | `owl:disjointWith` | |
| `Axiom::Domain` | `rdfs:domain` | |
| `Axiom::Range` | `rdfs:range` | |
| `Axiom::Functional` | `owl:FunctionalProperty` | |
| `Axiom::InverseFunctional` | `owl:InverseFunctionalProperty` | |
| `Axiom::InverseOf` | `owl:inverseOf` | |
| `Axiom::Symmetric` | `owl:SymmetricProperty` | |
| `Axiom::Transitive` | `owl:TransitiveProperty` | |

### Datatype table

| `Datatype` | XSD IRI |
| --- | --- |
| `String` | `xsd:string` |
| `Integer` | `xsd:integer` |
| `Float` | `xsd:double` |
| `Bool` | `xsd:boolean` |
| `DateTime` | `xsd:dateTime` |
| `Iri` | `xsd:anyURI` |
| `Bytes` | `xsd:base64Binary` |
| `Json` | `rdf:JSON` |

## LPG ↔ PROV-O

| LPG term | PROV-O term |
| --- | --- |
| `Activity` | `prov:Activity` |
| `AgentRef` (alias `ProvAgent`) | `prov:Agent` |
| `ProvEntity` | `prov:Entity` |
| `ProvenanceId` | the activity / entity IRI |
| `WasGeneratedBy` | `prov:wasGeneratedBy` |
| `WasDerivedFrom` | `prov:wasDerivedFrom` |
| `WasAttributedTo` | `prov:wasAttributedTo` |
| `Used` | `prov:used` |
| `AgentKind::Person` | `prov:Person` |
| `AgentKind::Software` | `prov:SoftwareAgent` |
| `AgentKind::Organization` | `prov:Organization` |

## LPG ↔ schema.org / Org Ontology

The reference vocabulary in `atomr-ontology-org` projects to:

| LPG `NodeType` | Org / schema.org term |
| --- | --- |
| `Organization` | `org:Organization` (equivalent to `schema:Organization`) |
| `FormalOrganization` | `org:FormalOrganization` |
| `OrganizationalUnit` | `org:OrganizationalUnit` |
| `Person` | `foaf:Person` |
| `Role` | `org:Role` |
| `Post` | `org:Post` |
| `Site` | `org:Site` |
| `Membership` | `org:Membership` |

Edges:

| LPG `EdgeType` | Org Ontology term |
| --- | --- |
| `memberOf` | `org:memberOf` |
| `hasMember` | `org:hasMember` |
| `subOrganizationOf` | `org:subOrganizationOf` |
| `hasSubOrganization` | `org:hasSubOrganization` |
| `hasMembership` | `org:hasMembership` |
| `organization` | `org:organization` |
| `role` | `org:role` |
| `hasSite` | `org:hasSite` |

## What gets dropped on projection

- `PropertyValue::Null` triples are skipped (RDF has no
  null-valued predicate).
- `PropertyValue::Json` triples use `rdf:JSON` as the datatype
  with the JSON document stringified into the lexical form.
- Content-addressed `NodeId`s become blank nodes when the
  corresponding `Node` has no IRI set.
- Inverse edges are not duplicated on projection; consumers that
  need both directions should materialize the inverse explicitly.
