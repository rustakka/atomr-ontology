"""Unit tests for the core data types (Tier 1)."""

import pytest

import atomr_ontology as ao
from atomr_ontology import IriError


def test_iri_validation():
    assert ao.Iri("https://example.org/Acme").value == "https://example.org/Acme"
    assert ao.Iri.from_unchecked("not validated").value == "not validated"

    with pytest.raises(IriError):
        ao.Iri("")
    with pytest.raises(IriError):
        ao.Iri("has whitespace")


def test_node_id_round_trip():
    a = ao.NodeId.new_random()
    parsed = ao.NodeId.from_hex(a.hex())
    assert a == parsed
    assert len(a.as_bytes()) == 32


def test_content_address_is_deterministic():
    a = ao.NodeId.content_address(b"https://example.org/Acme")
    b = ao.NodeId.content_address(b"https://example.org/Acme")
    assert a == b


def test_property_value_variants():
    assert ao.PropertyValue.string("x").kind == "string"
    assert ao.PropertyValue.integer(7).to_python() == 7
    assert ao.PropertyValue.float(1.5).to_python() == 1.5
    assert ao.PropertyValue.boolean(True).to_python() is True
    assert ao.PropertyValue.null().to_python() is None
    assert ao.PropertyValue.from_python("hello").kind == "string"
    assert ao.PropertyValue.from_python(7).kind == "integer"
    assert ao.PropertyValue.from_python(None).kind == "null"


def test_node_construction_with_iri_is_content_addressed():
    iri = ao.Iri("https://example.org/Acme")
    a = ao.Node("Organization", iri=iri, properties={"name": "Acme"})
    b = ao.Node("Organization", iri=ao.Iri("https://example.org/Acme"))
    assert a.id == b.id
    assert a.has_type("Organization")
    # property accessor
    assert a.property("name").to_python() == "Acme"


def test_edge_between_is_content_addressed():
    s = ao.NodeId.new_random()
    t = ao.NodeId.new_random()
    e1 = ao.Edge(s, "memberOf", t)
    e2 = ao.Edge(s, "memberOf", t)
    assert e1.id == e2.id


def test_schema_supertypes_walk():
    s = ao.Schema()
    s.declare_node_type(ao.NodeType("Agent"))
    s.declare_node_type(ao.NodeType("Organization").with_supertype("Agent"))
    s.declare_node_type(
        ao.NodeType("FormalOrganization").with_supertype("Organization"),
    )
    chain = s.supertypes_of("FormalOrganization")
    assert chain == ["FormalOrganization", "Organization", "Agent"]


def test_ontology_serde_json_round_trip_empty():
    # Empty ontologies serialize cleanly. Full ontologies with NodeId
    # keys do not survive JSON because NodeId is a 32-byte binary key.
    o = ao.Ontology()
    o.declare_node_type("Organization")
    s = o.to_json()
    back = ao.Ontology.from_json(s)
    assert back.schema.node_type("Organization") is not None


def test_axiom_id_is_deterministic():
    k = ao.AxiomKind.sub_class_of("FormalOrganization", "Organization")
    a1 = ao.Axiom(k)
    a2 = ao.Axiom(ao.AxiomKind.sub_class_of("FormalOrganization", "Organization"))
    assert a1.id == a2.id
    assert a1.kind.tag == "sub_class_of"
    assert a1.kind.operands() == {"sub": "FormalOrganization", "sup": "Organization"}


def test_cardinality_class_attrs():
    assert ao.Cardinality.ONE.min == 1
    assert ao.Cardinality.ONE.max == 1
    assert ao.Cardinality.AT_LEAST_ONE.min == 1
    assert ao.Cardinality.AT_LEAST_ONE.max is None
    assert ao.Cardinality.ONE.contains(1) is True


def test_record_builder():
    r = ao.Record("Organization")
    r.with_iri(ao.Iri("https://example.org/Acme")).with_property("name", "Acme")
    r.with_outbound("hasMember", ao.Iri("https://example.org/Bob"))
    r.with_source("row#1")
    assert r.type_name == "Organization"
    assert r.iri is not None
    assert r.iri.value == "https://example.org/Acme"
    assert r.outbound[0][0] == "hasMember"
