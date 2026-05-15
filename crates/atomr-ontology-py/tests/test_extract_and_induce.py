"""Tests for the extract + induce async surfaces, driven by MockBackend."""

import json

import atomr_ontology as ao


async def test_term_extractor_with_mock_backend():
    backend = ao.MockBackend.with_label("test")
    backend.enqueue_json(
        [{"surface": "Acme Inc.", "score": 0.99, "category": "ORG"}],
    )
    extractor = ao.TermExtractor(backend)
    terms, activity = await extractor.extract("Acme Inc. is a corporation.")
    assert len(terms) == 1
    assert terms[0].surface == "Acme Inc."
    assert isinstance(activity, ao.Activity)
    assert activity.label == "term-extraction"


async def test_entity_resolver_round_trip():
    backend = ao.MockBackend()
    backend.enqueue_json([
        {
            "surface": "Acme",
            "iri": "https://example.org/Acme",
            "type_name": "Organization",
            "score": 0.99,
            "is_new": True,
        },
    ])
    resolver = ao.EntityResolver(backend)
    terms = [ao.TermCandidate("Acme", 0.9, category="ORG")]
    entities, _ = await resolver.resolve(terms)
    assert len(entities) == 1
    assert entities[0].iri.value == "https://example.org/Acme"

    nodes = ao.EntityResolver.into_nodes(entities, iri_required=True)
    assert len(nodes) == 1
    assert nodes[0].has_type("Organization")


async def test_relation_extractor_into_edges():
    backend = ao.MockBackend()
    backend.enqueue_json([
        {"source": "Acme", "label": "memberOf", "target": "Globex", "score": 0.9},
    ])
    extractor = ao.RelationExtractor(backend)
    entities = [
        ao.EntityCandidate("Acme", "Organization", 0.99, iri=ao.Iri("https://example.org/Acme")),
        ao.EntityCandidate("Globex", "Organization", 0.99, iri=ao.Iri("https://example.org/Globex")),
    ]
    rels, _ = await extractor.extract("Acme and Globex.", entities)
    assert len(rels) == 1
    assert rels[0].label == "memberOf"

    a = ao.NodeId.new_random()
    b = ao.NodeId.new_random()
    edges = ao.RelationExtractor.into_edges(rels, {"Acme": a, "Globex": b})
    assert len(edges) == 1


async def test_record_extractor():
    backend = ao.MockBackend()
    backend.enqueue_json({
        "iri": "https://example.org/Acme",
        "type_name": "Organization",
        "properties": {"name": "Acme", "founded": 1995},
        "outbound": [["hasMember", "https://example.org/Bob"]],
        "source": "row#1",
    })
    rec_ex = ao.RecordExtractor(backend)
    record, _ = await rec_ex.extract("Acme Inc.")
    assert record.type_name == "Organization"
    assert record.iri is not None


async def test_taxonomy_inducer_into_axioms():
    backend = ao.MockBackend()
    backend.enqueue_json([
        {"sub": "FormalOrganization", "sup": "Organization", "score": 0.95},
    ])
    inducer = ao.TaxonomyInducer(backend)
    props, _ = await inducer.induce(["FormalOrganization", "Organization"])
    assert len(props) == 1
    axioms = ao.TaxonomyInducer.into_axioms(props)
    assert len(axioms) == 1
    assert axioms[0].kind.tag == "sub_class_of"


async def test_concept_former():
    backend = ao.MockBackend()
    backend.enqueue_json([
        {
            "name": "Organization",
            "members": ["Org", "Company", "Firm"],
            "description": "A formal organization",
            "score": 0.92,
        },
    ])
    former = ao.ConceptFormer(backend)
    clusters, _ = await former.cluster([ao.TermCandidate("Org", 0.9)])
    assert clusters[0].name == "Organization"
    nt = clusters[0].into_node_type()
    assert nt.name == "Organization"


async def test_axiom_miner():
    backend = ao.MockBackend()
    backend.enqueue(
        json.dumps([
            {"kind": "functional", "property": "homepage", "score": 0.9},
            {"kind": "sub_class_of", "sub": "FormalOrganization", "sup": "Organization", "score": 0.95},
        ])
    )
    miner = ao.AxiomMiner(backend)
    proposals, _ = await miner.mine("schema context")
    assert len(proposals) == 2
    tags = {p.kind for p in proposals}
    assert tags == {"functional", "sub_class_of"}
    axioms = [p.into_axiom() for p in proposals]
    assert axioms[0].kind.tag == "functional"


def test_parse_helpers_are_synchronous():
    terms = ao.extract.parse_terms('[{"surface":"x","score":0.5}]')
    assert terms[0].surface == "x"
