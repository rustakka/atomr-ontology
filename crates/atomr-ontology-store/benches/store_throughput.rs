//! Throughput benchmarks for `MemStore`.

use atomr_ontology_core::{Edge, Node};
use atomr_ontology_store::{EdgePattern, MemStore, NodePattern, OntologyStore, TraversalPlan};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use tokio::runtime::Runtime;

fn upsert_nodes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("upsert_nodes");
    for &n in &[100usize, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| {
                rt.block_on(async {
                    let store = MemStore::new();
                    store.with_mut(|o| o.declare_node_type("Organization"));
                    for i in 0..n {
                        let node = Node::new("Organization").with_property("idx", i as i64);
                        store.upsert_node(node).await.unwrap();
                    }
                    black_box(store)
                })
            });
        });
    }
}

fn match_by_property(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let store = rt.block_on(async {
        let store = MemStore::new();
        store.with_mut(|o| o.declare_node_type("Organization"));
        for i in 0..1000 {
            let node = Node::new("Organization").with_property("idx", i as i64);
            store.upsert_node(node).await.unwrap();
        }
        store
    });

    c.bench_function("match_by_property_1000", |b| {
        b.iter(|| {
            rt.block_on(async {
                let pat = NodePattern::any().typed("Organization").with_property("idx", 500i64);
                let rows = store.match_pattern(&pat).await.unwrap();
                black_box(rows)
            })
        });
    });
}

fn traverse_chain(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (store, root) = rt.block_on(async {
        let store = MemStore::new();
        store.with_mut(|o| {
            o.declare_node_type("Org");
            o.declare_edge_type("memberOf");
        });
        let mut prev = store.upsert_node(Node::new("Org")).await.unwrap();
        let root = prev;
        for _ in 0..50 {
            let next = store.upsert_node(Node::new("Org")).await.unwrap();
            store.upsert_edge(Edge::between(prev, "memberOf", next)).await.unwrap();
            prev = next;
        }
        (store, root)
    });

    c.bench_function("traverse_path_1_to_5", |b| {
        b.iter(|| {
            rt.block_on(async {
                let plan = TraversalPlan::from(NodePattern::any().bind("a").typed("Org").with_id(root))
                    .outbound(
                        EdgePattern::any().labeled("memberOf").repeat(1..=5),
                        NodePattern::any().bind("b"),
                    );
                let rows = store.traverse(&plan).await.unwrap();
                black_box(rows)
            })
        });
    });
}

criterion_group!(benches, upsert_nodes, match_by_property, traverse_chain);
criterion_main!(benches);
