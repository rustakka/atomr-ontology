# Contributing to atomr-ontology

Thanks for your interest in contributing. This crate is part of the
`rustakka/atomr` ecosystem and follows the same conventions as
`atomr-agents` and `atomr-infer`.

## Development setup

```bash
git clone https://github.com/rustakka/atomr-ontology
cd atomr-ontology

cargo build --workspace
cargo test --workspace
```

All dependencies — including the optional `atomr-agents` / `atomr-infer`
sibling crates — are pinned to crates.io versions. There are no
path dependencies; the crate resolves cleanly from a standalone
checkout without any sibling repos present.

The pinned toolchain is described in [`rust-toolchain.toml`](rust-toolchain.toml).
A stable Rust toolchain with `rustfmt` and `clippy` is required.

## Conventions

- **Format** — `cargo fmt --all`. CI fails on diffs (`-- --check`).
- **Lint** — `cargo clippy --workspace --all-targets -- -D warnings`.
- **Test** — `cargo test --workspace` must pass on every PR.
- **Docs** — `cargo doc --workspace --no-deps` must build with no
  broken intra-doc links.
- **Feature gating** — provider features (`provider-openai`,
  `provider-vllm`, …) are re-exported through `atomr-ontology` so
  consumers pull only what they need.
- **Naming** — follow the table in [`docs/naming.md`](docs/naming.md);
  do not introduce a new term when an industry-standard one already
  exists.

## xtask

```bash
cargo xtask parity   # crate-presence report
cargo xtask verify   # build + test + clippy + audit (1.0-rc gate)
cargo xtask audit    # anti-pattern sentinel count
```

## License

By contributing you agree that your contribution is licensed under the
Apache-2.0 license that governs this project.
