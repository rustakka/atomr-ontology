"""Tests for validate, provenance, rdf, org submodules (all sync)."""

import atomr_ontology as ao


def test_validate_returns_report():
    # The toy ontology intentionally has Globex -[memberOf]-> Acme where
    # the edge type's domain is Person — it produces 1 finding by design.
    o = ao.toy_org_ontology()
    report = ao.run_validate(o)
    assert isinstance(report, ao.ValidationReport)
    # An empty ontology, in contrast, must be clean.
    empty_report = ao.run_validate(ao.Ontology())
    assert empty_report.is_clean()


def test_validate_detects_missing_required_property():
    o = ao.Ontology()
    nt = ao.NodeType("Organization").with_property(ao.PropertyType.required_string("name"))
    sch = o.schema
    sch.declare_node_type(nt)
    o.set_schema(sch)
    o.upsert_node(ao.Node("Organization"))
    report = ao.run_validate(o)
    assert not report.is_clean()
    assert any(f.severity == ao.Severity.Error for f in report.findings)


def test_check_shapes_and_consistency_run_independently():
    o = ao.toy_org_ontology()
    shapes = ao.check_shapes(o)
    consistency = ao.check_consistency(o)
    # Both return reports, regardless of cleanliness.
    assert isinstance(shapes, ao.ValidationReport)
    assert isinstance(consistency, ao.ValidationReport)
    # Empty ontologies pass both.
    empty = ao.Ontology()
    assert ao.check_shapes(empty).is_clean()
    assert ao.check_consistency(empty).is_clean()


def test_provenance_log_round_trip():
    log = ao.ProvenanceLog()
    agent = ao.AgentRef.software("agent://x", "x")
    act = ao.Activity.started("extract").by(agent)
    pid = log.record_activity(act)
    ent_id = log.record_entity(ao.ProvEntity("doc.txt"))
    log.used(pid, ent_id)
    log.generated(ent_id, pid)
    log.attributed(ent_id, agent)
    log.derived(ent_id, ent_id)
    assert log.activity(pid) is not None
    assert log.entity(ent_id) is not None
    assert len(log.uses) == 1
    assert len(log.generations) == 1
    assert len(log.attributions) == 1
    assert len(log.derivations) == 1


def test_agent_ref_factories():
    s = ao.AgentRef.software("agent://x", "x")
    assert s.kind == ao.AgentKind.Software
    p = ao.AgentRef.person("agent://p", "Pat")
    assert p.kind == ao.AgentKind.Person


def test_to_rdf_emits_triples():
    o = ao.toy_org_ontology()
    triples = ao.to_rdf(o)
    assert len(triples) > 0
    # Reverse projection — partial, but should not raise.
    back = ao.from_rdf(triples)
    assert isinstance(back, ao.Ontology)


def test_turtle_ntriples_jsonld_writers():
    o = ao.toy_org_ontology()
    ttl = ao.rdf.turtle_write(o)
    nt = ao.rdf.ntriples_write(o)
    jl = ao.rdf.jsonld_write(o)
    assert isinstance(ttl, str) and len(ttl) > 0
    assert isinstance(nt, str) and len(nt) > 0
    assert isinstance(jl, str) and len(jl) > 0


def test_subject_object_constructors():
    s = ao.Subject.iri(ao.Iri("https://example.org/X"))
    assert s.kind == "iri"
    b = ao.Subject.blank("n0")
    assert b.kind == "blank"
    lit = ao.Object.xsd_integer(42)
    assert lit.kind == "literal"
    assert lit.lexical == "42"


def test_reference_ontology_has_org_types():
    o = ao.reference_ontology()
    assert o.schema.node_type("Organization") is not None
    assert o.schema.node_type("FormalOrganization") is not None
    assert o.schema.edge_type("memberOf") is not None


def test_namespace_constants():
    assert ao.ORG_NS.startswith("http://www.w3.org/ns/org")
    assert ao.FOAF_NS.startswith("http://xmlns.com/foaf")
    assert ao.SCHEMA_NS.startswith("http://schema.org")


def test_assert_subclass_of_passes():
    o = ao.toy_org_ontology()
    ao.assert_subclass_of(o, "FormalOrganization", "Organization")


def test_assert_subclass_of_raises():
    import pytest

    o = ao.toy_org_ontology()
    with pytest.raises(AssertionError):
        ao.assert_subclass_of(o, "Person", "Site")


def test_assert_axiom_present_passes():
    o = ao.toy_org_ontology()
    ao.assert_axiom_present(o, "sub_class_of")


def test_vocabulary_standard_bindings():
    v = ao.Vocabulary.with_standard_bindings()
    iri = v.expand_curie("org:Organization")
    assert iri is not None
    assert iri.value == "http://www.w3.org/ns/org#Organization"
