//! atomr-ontology xtask
//!
//! Subcommands:
//! - `parity`  — emit a JSON report of workspace crates and their presence.
//! - `audit`   — count anti-pattern sentinels (`todo!`, `unimplemented!`, `unwrap()`).
//! - `verify`  — chain `cargo fmt --check`, `cargo clippy`, `cargo test`, `cargo audit`.

use std::path::PathBuf;
use std::process::{Command, ExitStatus};

use anyhow::{anyhow, Result};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    match cmd {
        "parity" => parity(),
        "audit" => audit(),
        "verify" => verify(),
        _ => {
            help();
            Ok(())
        }
    }
}

fn help() {
    println!(
        "atomr-ontology xtask\n\nUsage:\n  cargo xtask parity\n  cargo xtask audit\n  cargo xtask verify\n"
    );
}

fn workspace_root() -> Result<PathBuf> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = PathBuf::from(manifest).join("..").canonicalize()?;
    Ok(path)
}

fn parity() -> Result<()> {
    let root = workspace_root()?;
    let crates_dir = root.join("crates");
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&crates_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            let cargo = entry.path().join("Cargo.toml");
            let lib = entry.path().join("src/lib.rs");
            entries.push(serde_json::json!({
                "crate": name,
                "has_manifest": cargo.exists(),
                "has_lib": lib.exists(),
            }));
        }
    }
    let report = serde_json::json!({
        "root": root.display().to_string(),
        "crates": entries,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn audit() -> Result<()> {
    let root = workspace_root()?;
    let mut total_todo = 0usize;
    let mut total_unimplemented = 0usize;
    let mut total_unwrap = 0usize;
    walk(&root.join("crates"), &mut |path: &PathBuf| -> Result<()> {
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let body = std::fs::read_to_string(path)?;
            for line in body.lines() {
                let l = line.trim();
                if l.starts_with("//") || l.starts_with("///") {
                    continue;
                }
                if l.contains("todo!") {
                    total_todo += 1;
                }
                if l.contains("unimplemented!") {
                    total_unimplemented += 1;
                }
                if l.contains(".unwrap()") {
                    total_unwrap += 1;
                }
            }
        }
        Ok(())
    })?;
    let report = serde_json::json!({
        "todo": total_todo,
        "unimplemented": total_unimplemented,
        "unwrap": total_unwrap,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    if total_todo > 0 || total_unimplemented > 0 {
        return Err(anyhow!("audit failed: todo/unimplemented sentinels present"));
    }
    Ok(())
}

fn verify() -> Result<()> {
    let root = workspace_root()?;
    // `cargo fmt --all` recurses into path-dependency source trees;
    // iterate over workspace members directly so the gate stays
    // scoped to this repo.
    for p in workspace_members(&root)? {
        run(Command::new("cargo").current_dir(&root).args(["fmt", "--check", "-p", &p]))?;
    }
    run(Command::new("cargo").current_dir(&root).args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]))?;
    run(Command::new("cargo").current_dir(&root).args(["test", "--workspace"]))?;
    audit()?;
    Ok(())
}

fn workspace_members(root: &PathBuf) -> Result<Vec<String>> {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!("cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let v: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    Ok(v["packages"]
        .as_array()
        .ok_or_else(|| anyhow!("metadata missing packages array"))?
        .iter()
        .filter_map(|p| p["name"].as_str().map(String::from))
        .collect())
}

fn run(cmd: &mut Command) -> Result<()> {
    let status: ExitStatus = cmd.status()?;
    if !status.success() {
        return Err(anyhow!("subprocess failed: {status}"));
    }
    Ok(())
}

fn walk(root: &PathBuf, f: &mut dyn FnMut(&PathBuf) -> Result<()>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk(&path, f)?;
        } else {
            f(&path)?;
        }
    }
    Ok(())
}
