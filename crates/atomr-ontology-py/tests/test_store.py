"""Async tests for the OntologyStore / MemStore surface."""

import pytest

import atomr_ontology as ao


async def test_upsert_and_fetch_node():
    store = ao.MemStore()
    node = ao.Node("Organization", iri=ao.Iri("https://example.org/Acme"))
    nid = await store.upsert_node(node)
    got = await store.node(nid)
    assert got is not None
    assert got.id == nid


async def test_match_pattern_filters_by_type_and_property():
    store = ao.MemStore()
    await store.upsert_node(
        ao.Node("Organization", properties={"name": "Acme"}),
    )
    await store.upsert_node(ao.Node("Person", properties={"name": "Bob"}))

    rows = await store.match_pattern(
        ao.NodePattern.any().bind("org").typed("Organization").with_property("name", "Acme")
    )
    assert len(rows) == 1
    assert "org" in rows[0].nodes


async def test_traverse_outbound():
    store = ao.MemStore()
    acme = await store.upsert_node(ao.Node("Organization"))
    bob = await store.upsert_node(ao.Node("Organization"))
    await store.upsert_edge(ao.Edge(bob, "memberOf", acme))

    plan = ao.TraversalPlan(ao.NodePattern.any().bind("a").typed("Organization")).outbound(
        ao.EdgePattern.any().labeled("memberOf"),
        ao.NodePattern.any().bind("b"),
    )
    rows = await store.traverse(plan)
    assert len(rows) >= 1
    assert "a" in rows[0].nodes
    assert "b" in rows[0].nodes


async def test_snapshot_and_diff():
    store = ao.MemStore()
    await store.upsert_node(ao.Node("Organization"))
    snap = await store.snapshot()
    diff = await store.diff(ao.Ontology())
    assert len(diff.added_nodes) == 1
    assert isinstance(snap, ao.Ontology)


async def test_commit_with_provenance_records_activity():
    store = ao.MemStore()
    delta = ao.OntologyDelta()
    delta.with_node(ao.Node("Organization"))
    activity = ao.Activity.started("test-commit")
    prov_id = await store.commit_with_provenance(delta, activity)
    log = await store.provenance()
    assert log.activity(prov_id) is not None


def test_snapshot_blocking():
    store = ao.MemStore.from_ontology(ao.reference_ontology())
    snap = store.snapshot_blocking()
    assert snap.node_count() == 0
    assert len(snap.schema.node_types) > 0
