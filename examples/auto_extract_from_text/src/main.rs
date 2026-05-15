//! auto_extract_from_text
//!
//! Run the full 7-stage auto-extract pipeline against a corpus
//! directory. By default the example uses a [`MockBackend`] so it
//! can run hermetically in CI; with `--provider <name>` it expects
//! a sibling adapter (gated behind cargo features) that satisfies
//! the [`Backend`] contract.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::Parser;

use atomr_ontology::prelude::*;
use atomr_ontology::store::OntologyDelta;
use atomr_ontology_org::reference_ontology;
use atomr_ontology_testkit::MockBackend;

/// CLI options.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Path to a corpus directory (one document per line per file).
    #[arg(long, default_value = "examples/auto_extract_from_text/sample-corpus")]
    corpus: PathBuf,
    /// Provider name. `mock` is the default and works without network access.
    #[arg(long, default_value = "mock")]
    provider: String,
    /// Model name (passed through to the provider; ignored by `mock`).
    #[arg(long, default_value = "")]
    model: String,
    /// Output directory for `ontology.{ttl,jsonld}` and `trace.json`.
    #[arg(long, default_value = "out")]
    out_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let args = Args::parse();

    // 1. Resolve provider.
    let backend = pick_backend(&args.provider, &args.model).await?;
    tracing::info!(provider = %args.provider, "backend ready");

    // 2. Seed store with the org reference vocabulary.
    let store = MemStore::from_ontology(reference_ontology());

    // 3. Load corpus.
    let documents = load_corpus(&args.corpus)?;
    if documents.is_empty() {
        return Err(anyhow!("no corpus documents found under {}", args.corpus.display()));
    }
    tracing::info!(count = documents.len(), "loaded corpus");

    let mut all_terms: Vec<TermCandidate> = Vec::new();
    let mut all_entities: Vec<EntityCandidate> = Vec::new();
    let mut all_relations: Vec<RelationCandidate> = Vec::new();
    let mut trace = serde_json::json!({ "stages": [] });

    // 4–6. Per-document: terms, entities, relations.
    let term_extractor = TermExtractor::new(backend.clone());
    let resolver = EntityResolver::new(backend.clone());
    let rel_extractor = RelationExtractor::new(backend.clone());

    for (i, doc) in documents.iter().enumerate() {
        let (terms, t_act) = term_extractor.extract(doc).await?;
        let (entities, e_act) = resolver.resolve(&terms).await?;
        let (relations, r_act) = rel_extractor.extract(doc, &entities).await?;
        trace["stages"].as_array_mut().unwrap().push(serde_json::json!({
            "doc_index": i,
            "terms": t_act,
            "entities": e_act,
            "relations": r_act,
        }));
        all_terms.extend(terms);
        all_entities.extend(entities);
        all_relations.extend(relations);
    }

    // 7. Validate + commit.
    use std::collections::HashMap;
    let mut surface_to_id: HashMap<String, NodeId> = HashMap::new();
    let nodes = EntityResolver::into_nodes(&all_entities, false);
    for (cand, node) in all_entities.iter().zip(nodes.iter()) {
        surface_to_id.insert(cand.surface.clone(), node.id);
    }
    let edges = RelationExtractor::into_edges(&all_relations, &surface_to_id);
    let commit_activity = Activity::started("auto-extract.commit")
        .by(AgentRef::software("agent://auto-extract", "auto_extract_from_text"))
        .with_attribute("provider", serde_json::json!(args.provider));
    let delta = OntologyDelta { nodes, edges, axioms: Vec::new() };
    let prov_id = store.commit_with_provenance(delta, commit_activity).await?;

    let snapshot = store.snapshot().await?;
    let report = atomr_ontology::validate::validate(&snapshot);
    tracing::info!(
        clean = report.is_clean(),
        findings = report.findings.len(),
        provenance_id = %prov_id,
        "pipeline complete"
    );

    // 8. Persist outputs.
    std::fs::create_dir_all(&args.out_dir)?;
    let ttl = atomr_ontology::rdf::turtle::write(&snapshot);
    let jsonld = atomr_ontology::rdf::jsonld::write(&snapshot);
    std::fs::write(args.out_dir.join("ontology.ttl"), ttl)?;
    std::fs::write(args.out_dir.join("ontology.jsonld"), jsonld)?;
    std::fs::write(args.out_dir.join("trace.json"), serde_json::to_string_pretty(&trace)?)?;
    println!("wrote {} (ttl + jsonld + trace)", args.out_dir.display());

    Ok(())
}

async fn pick_backend(provider: &str, _model: &str) -> Result<Arc<dyn Backend>> {
    match provider {
        "mock" => {
            // Deterministic scripted output suitable for CI.
            let m = MockBackend::with_label("scripted-mock");
            for _ in 0..16 {
                m.enqueue(r#"[{"surface":"Acme","score":0.9,"category":"ORG"}]"#);
                m.enqueue(r#"[{"surface":"Acme","iri":"https://example.org/Acme","type_name":"Organization","score":0.9,"is_new":true}]"#);
                m.enqueue(r#"[]"#);
            }
            Ok(Arc::new(m))
        }
        other => Err(anyhow!(
            "provider `{other}` requires building this example with the matching cargo feature (`--features provider-{other}`) and a real atomr-infer driver; not wired up in v0.1"
        )),
    }
}

fn load_corpus(path: &PathBuf) -> Result<Vec<String>> {
    if !path.exists() {
        // Fall back to the toy corpus baked into the testkit.
        return Ok(atomr_ontology_testkit::toy_corpus().into_iter().map(String::from).collect());
    }
    let mut docs = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            docs.push(std::fs::read_to_string(entry.path())?);
        }
    }
    Ok(docs)
}
