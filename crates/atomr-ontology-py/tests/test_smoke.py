"""End-to-end smoke test mirroring `examples/org_ontology_demo`.

Runs the full extract → resolve → relate → commit-with-provenance pipeline
against a MockBackend, then validates and serializes the result.
"""

import atomr_ontology as ao


TERM_RESPONSE = """[
  {"surface":"Acme Inc.","score":0.99,"category":"ORG"},
  {"surface":"Globex Inc.","score":0.97,"category":"ORG"},
  {"surface":"Bob Smith","score":0.95,"category":"PERSON"}
]"""

ENTITY_RESPONSE = """[
  {"surface":"Acme Inc.","iri":"https://example.org/Acme","type_name":"Organization","score":0.99,"is_new":true},
  {"surface":"Globex Inc.","iri":"https://example.org/Globex","type_name":"Organization","score":0.97,"is_new":true},
  {"surface":"Bob Smith","iri":"https://example.org/Bob","type_name":"Person","score":0.95,"is_new":true}
]"""

RELATION_RESPONSE = """[
  {"source":"Bob Smith","label":"memberOf","target":"Acme Inc.","score":0.95},
  {"source":"Globex Inc.","label":"subOrganizationOf","target":"Acme Inc.","score":0.9}
]"""


async def test_full_pipeline_end_to_end():
    # 1. Seed the store with the W3C Org reference vocabulary.
    store = ao.MemStore.from_ontology(ao.reference_ontology())
    seed_snap = await store.snapshot()
    assert seed_snap.schema.node_type("Organization") is not None

    # 2. Programmable backend with three scripted responses.
    backend = ao.MockBackend.with_label("smoke")
    backend.enqueue(TERM_RESPONSE)
    backend.enqueue(ENTITY_RESPONSE)
    backend.enqueue(RELATION_RESPONSE)

    # 3. Extract → resolve → relate.
    corpus = "\n".join(ao.toy_corpus())
    terms, term_act = await ao.TermExtractor(backend).extract(corpus)
    assert len(terms) == 3
    assert term_act.label == "term-extraction"

    entities, ent_act = await ao.EntityResolver(backend).resolve(terms)
    assert len(entities) == 3
    assert all(e.iri is not None for e in entities)

    relations, rel_act = await ao.RelationExtractor(backend).extract(corpus, entities)
    assert len(relations) == 2

    # 4. Convert candidates to typed nodes/edges.
    nodes = ao.EntityResolver.into_nodes(entities, iri_required=True)
    surface_to_id = {c.surface: n.id for c, n in zip(entities, nodes)}
    edges = ao.RelationExtractor.into_edges(relations, surface_to_id)
    assert len(nodes) == 3
    assert len(edges) == 2

    # 5. Commit with provenance.
    activity = (
        ao.Activity.started("smoke-test.commit")
        .by(ao.AgentRef.software("agent://smoke", "smoke"))
        .with_attribute("source", "toy-corpus")
    )
    delta = ao.OntologyDelta(nodes=nodes, edges=edges)
    prov_id = await store.commit_with_provenance(delta, activity)
    assert prov_id is not None

    # 6. Validate the post-commit ontology. EntityResolver-built nodes
    #    only carry `surface` (not `name`), so the schema's `name`
    #    requirement on Organization yields findings. We just check
    #    that validation runs and returns a report.
    snap = await store.snapshot()
    report = ao.run_validate(snap)
    assert isinstance(report, ao.ValidationReport)
    assert snap.node_count() >= 3
    assert snap.edge_count() >= 2

    # 7. Schema-side smoke: FormalOrganization < Organization.
    ao.assert_subclass_of(snap, "FormalOrganization", "Organization")

    # 8. Provenance log contains the commit activity.
    log = await store.provenance()
    assert log.activity(prov_id) is not None

    # 9. RDF export works.
    triples = ao.to_rdf(snap)
    assert len(triples) > 0
    ttl = ao.rdf.turtle_write(snap)
    assert "Organization" in ttl
