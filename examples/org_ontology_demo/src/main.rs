//! org_ontology_demo
//!
//! Build a small W3C Org Ontology graph from a hand-written seed,
//! drive a few extractor stages against a [`MockBackend`], and
//! print a final report. Acts as the canonical smoke-test for the
//! workspace.

use std::sync::Arc;

use anyhow::Result;

use atomr_ontology::prelude::*;
use atomr_ontology::store::OntologyDelta;
use atomr_ontology_org::reference_ontology;
use atomr_ontology_testkit::{assert_subclass_of, MockBackend};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Seed the store with the W3C Org reference vocabulary.
    let store = MemStore::from_ontology(reference_ontology());
    let snap = store.snapshot().await?;
    println!(
        "seeded ontology: {} node types, {} edge types, {} axioms",
        snap.schema.node_types.len(),
        snap.schema.edge_types.len(),
        snap.axioms.len(),
    );

    // 2. Set up a mock backend with scripted responses for the three
    //    extractor calls we exercise below.
    let backend = Arc::new({
        let m = MockBackend::with_label("demo-mock");
        m.enqueue(
            r#"[
            {"surface":"Acme Inc.","score":0.99,"category":"ORG"},
            {"surface":"Globex Inc.","score":0.97,"category":"ORG"},
            {"surface":"Bob Smith","score":0.95,"category":"PERSON"}
        ]"#,
        );
        m.enqueue(
            r#"[
            {"surface":"Acme Inc.","iri":"https://example.org/Acme","type_name":"Organization","score":0.99,"is_new":true},
            {"surface":"Globex Inc.","iri":"https://example.org/Globex","type_name":"Organization","score":0.97,"is_new":true},
            {"surface":"Bob Smith","iri":"https://example.org/Bob","type_name":"Person","score":0.95,"is_new":true}
        ]"#,
        );
        m.enqueue(
            r#"[
            {"source":"Bob Smith","label":"memberOf","target":"Acme Inc.","score":0.95},
            {"source":"Globex Inc.","label":"subOrganizationOf","target":"Acme Inc.","score":0.9}
        ]"#,
        );
        m
    });

    // 3. Extract terms.
    let term_extractor = TermExtractor::new(backend.clone());
    let corpus_doc = atomr_ontology_testkit::toy_corpus().join("\n");
    let (terms, terms_activity) = term_extractor.extract(&corpus_doc).await?;
    println!("extracted {} terms", terms.len());

    // 4. Resolve entities.
    let resolver = EntityResolver::new(backend.clone());
    let (entities, resolve_activity) = resolver.resolve(&terms).await?;
    println!("resolved {} entities", entities.len());

    // 5. Extract relations.
    let rel_extractor = RelationExtractor::new(backend.clone());
    let (relations, relations_activity) = rel_extractor.extract(&corpus_doc, &entities).await?;
    println!("proposed {} relations", relations.len());

    // 6. Commit: write entities + relations + provenance.
    use std::collections::HashMap;
    let mut surface_to_id: HashMap<String, NodeId> = HashMap::new();
    let nodes = EntityResolver::into_nodes(&entities, true);
    for (cand, node) in entities.iter().zip(nodes.iter()) {
        surface_to_id.insert(cand.surface.clone(), node.id);
    }
    let edges = RelationExtractor::into_edges(&relations, &surface_to_id);

    let commit_activity = Activity::started("auto-extract.commit")
        .by(AgentRef::software("agent://demo", "org_ontology_demo"))
        .with_attribute("source", serde_json::json!("toy-corpus"));
    let delta = OntologyDelta { nodes, edges, axioms: Vec::new() };
    let prov_id = store.commit_with_provenance(delta, commit_activity).await?;

    let report = atomr_ontology::validate::validate(&store.snapshot().await?);
    println!(
        "post-commit: clean={} findings={} provenance_id={}",
        report.is_clean(),
        report.findings.len(),
        prov_id,
    );

    // 7. Smoke-test the schema: FormalOrganization is a subclass of Organization.
    assert_subclass_of(&store.snapshot().await?, "FormalOrganization", "Organization");

    // 8. Provenance trace check.
    let log = store.provenance().await?;
    println!(
        "provenance log: {} activities (commit + {} extractor stages)",
        log.activities.len(),
        [&terms_activity, &resolve_activity, &relations_activity].len()
    );

    Ok(())
}
