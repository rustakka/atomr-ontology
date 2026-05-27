//! Built-in [`ToolSpec`] adapters over a live [`OntologyStore`].
//!
//! These tools let an agentic driver introspect the current store
//! during a session — checking whether a class already exists, listing
//! known supertypes, counting instances — instead of guessing in
//! isolation. They are the foundation that the agentic inducers in
//! `atomr-ontology-induce` build on, but they're reusable from any
//! caller that holds an `Arc<dyn OntologyStore>`.
//!
//! [`OntologyStore`]: atomr_ontology_store::OntologyStore
//! [`ToolSpec`]: crate::agentic::ToolSpec

use std::sync::Arc;

use atomr_ontology_store::OntologyStore;

use crate::agentic::ToolSpec;
use crate::backend::BackendError;

/// Build the default tool bundle (`class_exists`, `list_classes`,
/// `list_edge_types`, `count_instances`, `subclasses_of`,
/// `supertypes_of`, `properties_of`) over the given store.
pub fn default_store_tools(store: Arc<dyn OntologyStore>) -> Vec<ToolSpec> {
    vec![
        class_exists_tool(store.clone()),
        list_classes_tool(store.clone()),
        list_edge_types_tool(store.clone()),
        count_instances_tool(store.clone()),
        subclasses_of_tool(store.clone()),
        supertypes_of_tool(store.clone()),
        properties_of_tool(store),
    ]
}

/// `class_exists({ "name": "Organization" }) -> { "exists": bool }`
pub fn class_exists_tool(store: Arc<dyn OntologyStore>) -> ToolSpec {
    ToolSpec::new(
        "class_exists",
        "Check whether a class (node type) with the given name is already declared in the ontology.",
        serde_json::json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"],
        }),
        move |args| {
            let store = store.clone();
            Box::pin(async move {
                let name = string_arg(&args, "name")?;
                let snap = snapshot(&store).await?;
                Ok(serde_json::json!({ "exists": snap.schema.node_types.contains_key(&name) }))
            })
        },
    )
}

/// `list_classes({}) -> { "classes": [string] }`
pub fn list_classes_tool(store: Arc<dyn OntologyStore>) -> ToolSpec {
    ToolSpec::new(
        "list_classes",
        "List every class (node type) currently declared in the ontology.",
        serde_json::json!({ "type": "object", "properties": {} }),
        move |_args| {
            let store = store.clone();
            Box::pin(async move {
                let snap = snapshot(&store).await?;
                let names: Vec<&String> = snap.schema.node_types.keys().collect();
                Ok(serde_json::json!({ "classes": names }))
            })
        },
    )
}

/// `list_edge_types({}) -> { "edge_types": [string] }`
pub fn list_edge_types_tool(store: Arc<dyn OntologyStore>) -> ToolSpec {
    ToolSpec::new(
        "list_edge_types",
        "List every edge type (property / relation label) currently declared in the ontology.",
        serde_json::json!({ "type": "object", "properties": {} }),
        move |_args| {
            let store = store.clone();
            Box::pin(async move {
                let snap = snapshot(&store).await?;
                let names: Vec<&String> = snap.schema.edge_types.keys().collect();
                Ok(serde_json::json!({ "edge_types": names }))
            })
        },
    )
}

/// `count_instances({ "type": "Organization" }) -> { "count": n }`
pub fn count_instances_tool(store: Arc<dyn OntologyStore>) -> ToolSpec {
    ToolSpec::new(
        "count_instances",
        "Count nodes whose declared type matches the given name.",
        serde_json::json!({
            "type": "object",
            "properties": { "type": { "type": "string" } },
            "required": ["type"],
        }),
        move |args| {
            let store = store.clone();
            Box::pin(async move {
                let ty = string_arg(&args, "type")?;
                let snap = snapshot(&store).await?;
                let count = snap.nodes.values().filter(|n| n.has_type(&ty)).count();
                Ok(serde_json::json!({ "count": count }))
            })
        },
    )
}

/// `subclasses_of({ "class": "Organization" }) -> { "subclasses": [string] }`
///
/// Returns names of declared classes whose direct supertype list
/// contains the given name (one hop only — recurse on the agent side
/// if you need transitive closure).
pub fn subclasses_of_tool(store: Arc<dyn OntologyStore>) -> ToolSpec {
    ToolSpec::new(
        "subclasses_of",
        "List declared classes whose direct supertype list contains the given class.",
        serde_json::json!({
            "type": "object",
            "properties": { "class": { "type": "string" } },
            "required": ["class"],
        }),
        move |args| {
            let store = store.clone();
            Box::pin(async move {
                let class = string_arg(&args, "class")?;
                let snap = snapshot(&store).await?;
                let subs: Vec<&String> = snap
                    .schema
                    .node_types
                    .iter()
                    .filter(|(_, ty)| ty.supertypes.iter().any(|s| s == &class))
                    .map(|(name, _)| name)
                    .collect();
                Ok(serde_json::json!({ "subclasses": subs }))
            })
        },
    )
}

/// `supertypes_of({ "class": "FormalOrganization" }) -> { "supertypes": [string] }`
///
/// Returns the transitive supertype chain starting from the given
/// class (the class itself is included as the first entry).
pub fn supertypes_of_tool(store: Arc<dyn OntologyStore>) -> ToolSpec {
    ToolSpec::new(
        "supertypes_of",
        "Return the transitive supertype chain for the given class (depth-first).",
        serde_json::json!({
            "type": "object",
            "properties": { "class": { "type": "string" } },
            "required": ["class"],
        }),
        move |args| {
            let store = store.clone();
            Box::pin(async move {
                let class = string_arg(&args, "class")?;
                let snap = snapshot(&store).await?;
                let chain: Vec<String> = snap
                    .schema
                    .supertypes_of(&class)
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect();
                Ok(serde_json::json!({ "supertypes": chain }))
            })
        },
    )
}

/// `properties_of({ "class": "Organization" }) -> { "properties": [{ "name", "datatype" }] }`
pub fn properties_of_tool(store: Arc<dyn OntologyStore>) -> ToolSpec {
    ToolSpec::new(
        "properties_of",
        "List declared properties (name + datatype) for the given class.",
        serde_json::json!({
            "type": "object",
            "properties": { "class": { "type": "string" } },
            "required": ["class"],
        }),
        move |args| {
            let store = store.clone();
            Box::pin(async move {
                let class = string_arg(&args, "class")?;
                let snap = snapshot(&store).await?;
                let props = snap
                    .schema
                    .node_type(&class)
                    .map(|ty| {
                        ty.properties
                            .iter()
                            .map(|p| {
                                serde_json::json!({
                                    "name": p.name,
                                    "datatype": format!("{:?}", p.datatype).to_lowercase(),
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                Ok(serde_json::json!({ "properties": props }))
            })
        },
    )
}

async fn snapshot(
    store: &Arc<dyn OntologyStore>,
) -> Result<atomr_ontology_core::Ontology, BackendError> {
    store.snapshot().await.map_err(|e| BackendError::Other(format!("store snapshot: {e}")))
}

fn string_arg(args: &serde_json::Value, key: &str) -> Result<String, BackendError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| BackendError::Parse(format!("missing string arg `{key}`")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::{Iri, Node, NodeType};
    use atomr_ontology_store::MemStore;

    fn store_with_org_schema() -> Arc<dyn OntologyStore> {
        let mut o = atomr_ontology_core::Ontology::new();
        o.schema.declare_node_type(
            NodeType::new("Organization").with_description("A formal organization"),
        );
        o.schema
            .declare_node_type(NodeType::new("FormalOrganization").with_supertype("Organization"));
        o.upsert_node(
            Node::from_iri(Iri::from_unchecked("https://example.org/Acme"), "Organization")
                .with_property("name", "Acme"),
        );
        Arc::new(MemStore::from_ontology(o))
    }

    #[tokio::test]
    async fn class_exists_works() {
        let store = store_with_org_schema();
        let tool = class_exists_tool(store);
        let r = (tool.handler)(serde_json::json!({"name": "Organization"})).await.unwrap();
        assert_eq!(r, serde_json::json!({"exists": true}));
        let r = (tool.handler)(serde_json::json!({"name": "Nope"})).await.unwrap();
        assert_eq!(r, serde_json::json!({"exists": false}));
    }

    #[tokio::test]
    async fn subclasses_of_finds_one_hop() {
        let store = store_with_org_schema();
        let tool = subclasses_of_tool(store);
        let r = (tool.handler)(serde_json::json!({"class": "Organization"})).await.unwrap();
        assert_eq!(r, serde_json::json!({"subclasses": ["FormalOrganization"]}));
    }

    #[tokio::test]
    async fn supertypes_of_returns_chain() {
        let store = store_with_org_schema();
        let tool = supertypes_of_tool(store);
        let r = (tool.handler)(serde_json::json!({"class": "FormalOrganization"})).await.unwrap();
        assert_eq!(
            r,
            serde_json::json!({"supertypes": ["FormalOrganization", "Organization"]})
        );
    }

    #[tokio::test]
    async fn count_instances_counts() {
        let store = store_with_org_schema();
        let tool = count_instances_tool(store);
        let r = (tool.handler)(serde_json::json!({"type": "Organization"})).await.unwrap();
        assert_eq!(r, serde_json::json!({"count": 1}));
    }
}
