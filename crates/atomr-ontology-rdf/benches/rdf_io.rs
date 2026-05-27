//! Read/write throughput benchmarks for the RDF serializers.

use atomr_ontology_core::{
    schema::{Cardinality, NodeType, PropertyType},
    Datatype, Edge, Iri, Node, Ontology,
};
use atomr_ontology_rdf::{jsonld, ntriples, to_rdf, turtle};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn fixture(n: usize) -> Ontology {
    let mut o = Ontology::new();
    o.schema.declare_node_type(
        NodeType::new("Organization")
            .with_iri(Iri::from_unchecked("http://www.w3.org/ns/org#Organization"))
            .with_property(PropertyType {
                name: "name".into(),
                datatype: Datatype::String,
                cardinality: Cardinality::ONE,
                iri: None,
                description: None,
            }),
    );
    o.schema
        .declare_edge_type(atomr_ontology_core::schema::EdgeType::new("memberOf"));
    let mut prev: Option<atomr_ontology_core::NodeId> = None;
    for i in 0..n {
        let n = Node::from_iri(
            Iri::from_unchecked(format!("https://example.org/Org{i}")),
            "Organization",
        )
        .with_property("name", format!("Org{i}"));
        let id = o.upsert_node(n);
        if let Some(p) = prev {
            o.upsert_edge(Edge::between(id, "memberOf", p));
        }
        prev = Some(id);
    }
    o
}

fn turtle_io(c: &mut Criterion) {
    let o = fixture(200);
    c.bench_function("turtle_write_200", |b| {
        b.iter(|| black_box(turtle::write(&o)))
    });
    let serialized = turtle::write(&o);
    c.bench_function("turtle_parse_200", |b| {
        b.iter(|| black_box(turtle::parse(&serialized).unwrap()))
    });
}

fn ntriples_io(c: &mut Criterion) {
    let o = fixture(200);
    c.bench_function("ntriples_write_200", |b| {
        b.iter(|| black_box(ntriples::write(&o)))
    });
    let serialized = ntriples::write(&o);
    c.bench_function("ntriples_parse_200", |b| {
        b.iter(|| black_box(ntriples::parse(&serialized).unwrap()))
    });
}

fn jsonld_io(c: &mut Criterion) {
    let o = fixture(200);
    c.bench_function("jsonld_write_200", |b| {
        b.iter(|| black_box(jsonld::write(&o)))
    });
    let serialized = jsonld::write(&o);
    c.bench_function("jsonld_parse_200", |b| {
        b.iter(|| black_box(jsonld::parse(&serialized).unwrap()))
    });
}

fn to_rdf_projection(c: &mut Criterion) {
    let o = fixture(200);
    c.bench_function("to_rdf_200", |b| b.iter(|| black_box(to_rdf(&o))));
}

criterion_group!(benches, turtle_io, ntriples_io, jsonld_io, to_rdf_projection);
criterion_main!(benches);
