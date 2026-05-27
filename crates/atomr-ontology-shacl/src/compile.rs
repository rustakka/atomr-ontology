//! Compile an [`atomr_ontology_core::Schema`] to SHACL shapes in Turtle.
//!
//! The output is a self-contained Turtle document that declares
//! `sh:NodeShape`s for every `NodeType` in the schema, with one
//! `sh:property` block per declared `PropertyType` and additional
//! `sh:property` blocks for every `EdgeType` whose `domain` includes
//! the node type. Cardinality bounds map to `sh:minCount` /
//! `sh:maxCount`, and `Datatype`s map to their `xsd:*` counterparts.
//!
//! The emitter uses explicit `_:` blank-node labels (rather than
//! `[ ... ]` shorthand) so that the bundled Turtle parser in
//! [`atomr_ontology_rdf::turtle`] can round-trip the output.

use atomr_ontology_core::{Datatype, EdgeType, Iri, NodeType, PropertyType, Schema};
use thiserror::Error;

use crate::ns::{shacl_iri, xsd_iri, SH_PREFIX, SH_URI, XSD_PREFIX, XSD_URI};

/// Errors raised while compiling a [`Schema`] to SHACL Turtle.
#[derive(Debug, Error)]
pub enum ShaclCompileError {
    /// The schema referenced a type whose IRI is required for SHACL
    /// projection but is missing.
    #[error("missing IRI for {0}")]
    MissingIri(String),
    /// A catch-all for unexpected compilation failures.
    #[error("shacl compile error: {0}")]
    Other(String),
}

/// Compile a [`Schema`] to a SHACL Turtle document.
pub fn to_shacl_turtle(schema: &Schema) -> Result<String, ShaclCompileError> {
    let mut out = String::new();
    push_prefix(&mut out, SH_PREFIX, SH_URI);
    push_prefix(&mut out, XSD_PREFIX, XSD_URI);
    out.push('\n');

    let mut blank_counter: u64 = 0;
    for ty in schema.node_types.values() {
        let mut bodies: Vec<String> = Vec::new();
        let refs = collect_property_refs(schema, ty, &mut blank_counter, &mut bodies);
        emit_node_shape(&mut out, ty, &refs);
        for body in &bodies {
            out.push_str(body);
        }
        out.push('\n');
    }
    Ok(out)
}

fn push_prefix(out: &mut String, prefix: &str, base: &str) {
    out.push_str("@prefix ");
    out.push_str(prefix);
    out.push_str(": <");
    out.push_str(base);
    out.push_str("> .\n");
}

/// Build the standalone `_:bN sh:path ... .` statements for every
/// property and edge block belonging to `ty` and return the list of
/// blank-node references the NodeShape should point at via
/// `sh:property`.
fn collect_property_refs(
    schema: &Schema,
    ty: &NodeType,
    blank_counter: &mut u64,
    bodies: &mut Vec<String>,
) -> Vec<String> {
    let mut refs = Vec::new();
    for prop in &ty.properties {
        let label = next_blank(blank_counter);
        bodies.push(render_property_block(&label, prop));
        refs.push(label);
    }
    for edge in schema.edge_types.values() {
        if edge.domain.iter().any(|d| d == &ty.name) {
            let label = next_blank(blank_counter);
            bodies.push(render_edge_block(&label, schema, edge));
            refs.push(label);
        }
    }
    refs
}

fn emit_node_shape(out: &mut String, ty: &NodeType, property_refs: &[String]) {
    let target_iri = type_iri(ty);
    let shape_iri = shape_iri_for(ty);

    push_iri_term(out, &shape_iri);
    out.push_str(" a ");
    push_iri_term(out, &shacl_iri("NodeShape"));
    out.push_str(" ;\n  ");
    push_iri_term(out, &shacl_iri("targetClass"));
    out.push(' ');
    push_iri_term(out, &target_iri);

    for label in property_refs {
        out.push_str(" ;\n  ");
        push_iri_term(out, &shacl_iri("property"));
        out.push_str(" _:");
        out.push_str(label);
    }
    out.push_str(" .\n");
}

/// Render a standalone Turtle statement for an `sh:property` block on
/// a datatype-valued property.
fn render_property_block(label: &str, prop: &PropertyType) -> String {
    let path_iri = property_path_iri(prop);
    let datatype_iri = xsd_for_datatype(prop.datatype);

    let mut out = String::new();
    out.push_str("_:");
    out.push_str(label);
    out.push(' ');
    push_iri_term(&mut out, &shacl_iri("path"));
    out.push(' ');
    push_iri_term(&mut out, &path_iri);

    if prop.cardinality.min > 0 {
        out.push_str(" ;\n  ");
        push_iri_term(&mut out, &shacl_iri("minCount"));
        out.push(' ');
        push_xsd_integer(&mut out, prop.cardinality.min as i64);
    }
    if let Some(max) = prop.cardinality.max {
        out.push_str(" ;\n  ");
        push_iri_term(&mut out, &shacl_iri("maxCount"));
        out.push(' ');
        push_xsd_integer(&mut out, max as i64);
    }
    out.push_str(" ;\n  ");
    push_iri_term(&mut out, &shacl_iri("datatype"));
    out.push(' ');
    push_iri_term(&mut out, &datatype_iri);
    out.push_str(" .\n");
    out
}

/// Render a standalone Turtle statement for an `sh:property` block
/// describing an outgoing edge: it constrains `sh:nodeKind sh:IRI` and
/// emits one `sh:class` constraint per target node type.
fn render_edge_block(label: &str, schema: &Schema, edge: &EdgeType) -> String {
    let path_iri = edge_path_iri(edge);

    let mut out = String::new();
    out.push_str("_:");
    out.push_str(label);
    out.push(' ');
    push_iri_term(&mut out, &shacl_iri("path"));
    out.push(' ');
    push_iri_term(&mut out, &path_iri);

    if edge.cardinality.min > 0 {
        out.push_str(" ;\n  ");
        push_iri_term(&mut out, &shacl_iri("minCount"));
        out.push(' ');
        push_xsd_integer(&mut out, edge.cardinality.min as i64);
    }
    if let Some(max) = edge.cardinality.max {
        out.push_str(" ;\n  ");
        push_iri_term(&mut out, &shacl_iri("maxCount"));
        out.push(' ');
        push_xsd_integer(&mut out, max as i64);
    }

    out.push_str(" ;\n  ");
    push_iri_term(&mut out, &shacl_iri("nodeKind"));
    out.push(' ');
    push_iri_term(&mut out, &shacl_iri("IRI"));

    for target_name in &edge.range {
        let target_iri = schema
            .node_types
            .get(target_name)
            .and_then(|nt| nt.iri.clone())
            .unwrap_or_else(|| fallback_iri(target_name));
        out.push_str(" ;\n  ");
        push_iri_term(&mut out, &shacl_iri("class"));
        out.push(' ');
        push_iri_term(&mut out, &target_iri);
    }
    out.push_str(" .\n");
    out
}

fn next_blank(counter: &mut u64) -> String {
    let n = *counter;
    *counter += 1;
    format!("b{n}")
}

fn type_iri(ty: &NodeType) -> Iri {
    ty.iri.clone().unwrap_or_else(|| fallback_iri(&ty.name))
}

fn shape_iri_for(ty: &NodeType) -> Iri {
    let base = ty
        .iri
        .as_ref()
        .map(|i| i.as_str().to_string())
        .unwrap_or_else(|| fallback_iri(&ty.name).into_string());
    Iri::from_unchecked(format!("{base}Shape"))
}

fn property_path_iri(prop: &PropertyType) -> Iri {
    prop.iri.clone().unwrap_or_else(|| fallback_iri(&prop.name))
}

fn edge_path_iri(edge: &EdgeType) -> Iri {
    edge.iri.clone().unwrap_or_else(|| fallback_iri(&edge.name))
}

fn fallback_iri(name: &str) -> Iri {
    Iri::from_unchecked(format!("urn:atomr:shacl:{name}"))
}

fn xsd_for_datatype(d: Datatype) -> Iri {
    let local = match d {
        Datatype::String => "string",
        Datatype::Integer => "integer",
        Datatype::Float => "double",
        Datatype::Bool => "boolean",
        Datatype::DateTime => "dateTime",
        Datatype::Iri => "anyURI",
        Datatype::Bytes => "base64Binary",
        Datatype::Json => "string",
    };
    xsd_iri(local)
}

fn push_iri_term(out: &mut String, iri: &Iri) {
    let s = iri.as_str();
    if let Some(local) = s.strip_prefix(SH_URI) {
        if is_simple_local(local) {
            out.push_str(SH_PREFIX);
            out.push(':');
            out.push_str(local);
            return;
        }
    }
    if let Some(local) = s.strip_prefix(XSD_URI) {
        if is_simple_local(local) {
            out.push_str(XSD_PREFIX);
            out.push(':');
            out.push_str(local);
            return;
        }
    }
    out.push('<');
    out.push_str(s);
    out.push('>');
}

fn is_simple_local(s: &str) -> bool {
    !s.is_empty()
        && !s.contains(' ')
        && !s.contains('/')
        && !s.contains('#')
        && !s.contains('<')
        && !s.contains('>')
}

fn push_xsd_integer(out: &mut String, value: i64) {
    out.push('"');
    out.push_str(&value.to_string());
    out.push('"');
    out.push_str("^^");
    push_iri_term(out, &xsd_iri("integer"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomr_ontology_core::schema::{Cardinality, NodeType, PropertyType};

    #[test]
    fn emits_node_shape_with_required_property() {
        let mut schema = Schema::new();
        schema.declare_node_type(
            NodeType::new("Organization")
                .with_iri(Iri::from_unchecked("http://example.org/Organization"))
                .with_property(PropertyType {
                    name: "name".into(),
                    datatype: Datatype::String,
                    cardinality: Cardinality::ONE,
                    iri: Some(Iri::from_unchecked("http://example.org/name")),
                    description: None,
                }),
        );
        let ttl = to_shacl_turtle(&schema).unwrap();
        assert!(ttl.contains("sh:NodeShape"), "missing sh:NodeShape\n{ttl}");
        assert!(ttl.contains("sh:targetClass"), "missing sh:targetClass\n{ttl}");
        assert!(ttl.contains("sh:minCount"), "missing sh:minCount\n{ttl}");
        // minCount 1 lexical form should appear via an xsd:integer literal.
        assert!(ttl.contains("\"1\""), "minCount value 1 missing\n{ttl}");
        assert!(ttl.contains("sh:datatype"), "missing sh:datatype\n{ttl}");
        assert!(ttl.contains("xsd:string"), "missing xsd:string\n{ttl}");
    }
}
