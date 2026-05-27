"""Smoke test for atomr_ontology.actor_projection.

Builds a synthetic in-memory actor source, runs the projector with each
of the four built-in projection shapes, and asserts the resulting node /
edge / activity counts via the OntologyStoreHandle helpers.
"""
from __future__ import annotations

import pytest

import atomr_ontology as ao


@pytest.mark.parametrize(
    "shape, expect_nonempty_edges",
    [
        ("hierarchical", True),
        ("event_stream", True),
        ("snapshot_diff", True),
        ("flat", False),
    ],
)
async def test_projection_shape_end_to_end(shape: str, expect_nonempty_edges: bool):
    src = ao.actor_projection.InMemoryActorSource("py-smoke")
    src.push_path("/workflow/foo/run/1/step/a")
    src.push_path("/workflow/foo/run/1/step/b")
    src.push_event(
        actor="a",
        kind="created",
        payload={"detail": "ok"},
        path="/workflow/foo/run/1/step/a",
    )
    src.push_event(
        actor="b",
        kind="completed",
        payload={"detail": "done"},
        path="/workflow/foo/run/1/step/b",
    )
    src.put_state("a", {"phase": "ready"})

    builder = ao.actor_projection.ProjectorBuilder()
    builder.source(src)
    builder.with_replay()
    builder.projection(shape)
    builder.iri("path_based", base="https://atomr.dev/actor/")
    builder.conflict("merge")
    builder.schema("hybrid")
    handle = await builder.store_from_memory()
    builder.set_store(handle)

    projector = builder.build()
    report = await projector.run()
    assert report["batches"] >= 1, report
    assert report["activities_recorded"] >= 1, report

    node_count = await handle.node_count()
    edge_count = await handle.edge_count()
    activities = await handle.activity_count()
    assert node_count > 0, f"{shape}: expected nodes"
    if expect_nonempty_edges:
        assert edge_count > 0, f"{shape}: expected edges"
    assert activities == report["activities_recorded"]


async def test_skip_existing_is_idempotent_via_python():
    src = ao.actor_projection.InMemoryActorSource("py-skip")
    src.push_path("/workflow/foo/run/1")

    def make_builder() -> tuple[
        "ao.actor_projection.Projector",
        "ao.actor_projection.OntologyStoreHandle",
    ]:
        b = ao.actor_projection.ProjectorBuilder()
        b.source(src)
        b.with_replay()
        b.projection("hierarchical")
        b.iri("path_based", base="https://atomr.dev/actor/")
        b.conflict("skip_existing")
        b.schema("induced")
        return b

    builder = make_builder()
    handle = await builder.store_from_memory()
    builder.set_store(handle)
    projector = builder.build()
    await projector.run()
    first = await handle.node_count()

    builder2 = make_builder()
    builder2.set_store(handle)
    projector2 = builder2.build()
    await projector2.run()
    second = await handle.node_count()

    assert first == second, "skip_existing should keep node count stable"
