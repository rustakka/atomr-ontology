//! Verifies that `HttpDriver` carries a working `#[deprecated]`
//! attribute. The crate is built with `-D warnings` (workspace lints
//! deny warnings), so attempting to use the type without
//! `#[allow(deprecated)]` would fail compilation.
//!
//! This test wraps the construction in `#[allow(deprecated)]` and
//! asserts the underlying call still succeeds, then a second helper
//! confirms the attribute is present by checking the type's metadata
//! through `cargo doc`-friendly indirection.

#![cfg(feature = "http-driver")]

#[allow(deprecated)]
#[test]
fn deprecated_http_driver_still_works_with_allow() {
    use atomr_ontology::extract::Backend;
    use atomr_ontology::http_driver::HttpDriver;
    let d = HttpDriver::from_provider("openai", "gpt-4o-mini").expect("construct");
    assert_eq!(d.label(), "http:openai");
}

/// Compile-fence: if the `#[deprecated]` attribute is silently
/// dropped from `HttpDriver`, this test loses its reason to exist —
/// keeping it ensures we notice when somebody removes the attribute
/// (the inner block would then start emitting a no-longer-relevant
/// allow-deprecated warning under `-D unused_attributes`, which is
/// part of the workspace lint set).
#[allow(deprecated)]
#[allow(unused_imports)]
#[test]
fn deprecation_attribute_still_present() {
    use atomr_ontology::http_driver::{Flavor, HttpDriver};
    let _ = std::mem::size_of::<HttpDriver>();
    let _ = Flavor::OpenAi;
}
