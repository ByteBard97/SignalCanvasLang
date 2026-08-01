//! Every `#[ts(export)]` type must have a unique *effective* exported name (#33).
//!
//! ts-rs derives the output filename from the exported name, so two types sharing one
//! name write to the same `.ts` file and the last writer silently wins. That is exactly
//! what happened to `BusOutput`: the AST node and the canvas DTO both exported under it,
//! the AST shape won, and the DTO had no generated TypeScript at all — including fields
//! added across two releases.
//!
//! It stayed invisible because nothing *asserts* on binding output. The bindings do get
//! regenerated on every `cargo test` (ts-rs emits an `export_bindings_*` test per type,
//! and CI runs `cargo test`), so the clash was reproduced constantly — it simply never
//! failed anything. This test is that missing assertion.
//!
//! Checks the **effective** name — `#[ts(rename = "…")]` if present, else the Rust ident.
//! Comparing Rust idents would miss the nastier case: a rename colliding with an existing
//! type's name, where the source shows two different identifiers and the output silently
//! collapses to one file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Collect `.rs` files under a directory.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Pull `rename = "Foo"` out of a `#[ts(...)]` attribute body.
fn rename_value(attr: &str) -> Option<String> {
    let rest = attr.split_once("rename")?.1.trim_start();
    // Skip `=` and whitespace, then take the quoted value. Guards against matching
    // `rename_all`, which is a different attribute with different semantics.
    let rest = rest.strip_prefix('=')?.trim_start();
    let inner = rest.strip_prefix('"')?;
    inner.split('"').next().map(str::to_string)
}

/// Effective exported name → the sites declaring it.
fn exported_names(root: &Path) -> HashMap<String, Vec<String>> {
    let mut files = Vec::new();
    rust_files(root, &mut files);
    files.sort();

    let mut names: HashMap<String, Vec<String>> = HashMap::new();
    for file in &files {
        let Ok(src) = std::fs::read_to_string(file) else { continue };
        let lines: Vec<&str> = src.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with("#[ts(") || !trimmed.contains("export") {
                continue;
            }
            // The declaration follows, possibly after further attributes.
            let decl = lines[i + 1..]
                .iter()
                .map(|l| l.trim())
                .find(|l| l.starts_with("pub struct ") || l.starts_with("pub enum "));
            let Some(decl) = decl else { continue };
            let ident = decl
                .trim_start_matches("pub struct ")
                .trim_start_matches("pub enum ")
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                .unwrap_or_default();
            if ident.is_empty() {
                continue;
            }
            let effective = rename_value(trimmed).unwrap_or_else(|| ident.to_string());
            names
                .entry(effective)
                .or_default()
                .push(format!("{}::{ident}", file.display()));
        }
    }
    names
}

#[test]
fn exported_ts_names_are_unique() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let names = exported_names(&root);

    assert!(
        names.len() > 50,
        "scanner found only {} exported types — it has probably stopped matching the \
         source layout, which would make this test silently vacuous",
        names.len()
    );

    let collisions: Vec<_> = names.iter().filter(|(_, sites)| sites.len() > 1).collect();
    assert!(
        collisions.is_empty(),
        "two or more #[ts(export)] types share an exported name, so they overwrite each \
         other's generated .ts file and one shape is silently lost:\n{collisions:#?}"
    );
}

#[test]
fn rename_value_ignores_rename_all() {
    // `rename_all` is a casing rule, not an exported name. Treating it as one would make
    // every serde-cased type report a bogus exported name.
    assert_eq!(rename_value(r#"#[ts(export, rename = "Foo")]"#), Some("Foo".into()));
    assert_eq!(rename_value(r#"#[ts(export, rename_all = "camelCase")]"#), None);
    assert_eq!(rename_value(r#"#[ts(export)]"#), None);
}
