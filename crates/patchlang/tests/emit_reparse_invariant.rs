//! The emitter must produce text its own parser reads back identically.
//!
//! This is a *generic* guard, deliberately not tied to any one construct. Two bugs of
//! this exact shape shipped in short succession:
//!
//! - **#34** — the legacy bus-output fallback emitted `output ""`, which the parser
//!   rejects. The emitter produced a file it could not read.
//! - **#35** — a bus name containing `"` emitted `label: "The "Big" Mix"`, which *parses*
//!   but yields a different value. Silent corruption, no diagnostic.
//!
//! Both were found by hand, after the fact, on the one construct someone happened to
//! look at. The invariant below catches the whole family across every fixture we have:
//!
//! ```text
//! parse -> emit -> parse -> emit
//! ```
//!
//! and asserts the second parse is clean and the two emissions are byte-identical.
//! #34 breaks the second parse; #35 makes the second emission differ from the first,
//! because the corrupted value re-emits shorter. A construct only needs to appear in
//! some fixture to be covered, so this keeps working as the language grows.
//!
//! Comparing emitted TEXT rather than ASTs is deliberate: spans necessarily differ
//! between the two parses, and a span-blind AST comparison is precisely the kind of
//! projection that hides a formatting defect.

use std::path::{Path, PathBuf};

/// Every `.patch` file in the workspace fixtures tree, sorted for deterministic output.
fn all_fixtures() -> Vec<PathBuf> {
    let root = Path::new("../../tests/fixtures");
    let mut out = Vec::new();
    collect(root, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read fixture dir {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "patch") {
            out.push(path);
        }
    }
}

#[test]
fn every_fixture_survives_emit_reparse_unchanged() {
    let fixtures = all_fixtures();
    assert!(
        fixtures.len() > 50,
        "expected the fixture corpus to be found; got {} files — has the path moved?",
        fixtures.len()
    );

    let mut checked = 0usize;
    let mut skipped: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    for path in &fixtures {
        let name = path.display().to_string();
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {name}: {e}"));

        // Some fixtures are deliberately malformed (DRC/error cases). They are not
        // this test's subject: an emitter can only be held to round-tripping input
        // the parser accepted in the first place.
        let first = patchlang::parse(&source);
        if !first.errors.is_empty() {
            skipped.push(name);
            continue;
        }

        let emitted_once = patchlang::format_program(&first.program);

        let second = patchlang::parse(&emitted_once);
        if !second.errors.is_empty() {
            failures.push(format!(
                "{name}: emitted text does not parse ({} error(s)); first is {:?}",
                second.errors.len(),
                second.errors.first().map(|e| &e.message)
            ));
            continue;
        }

        let emitted_twice = patchlang::format_program(&second.program);
        if emitted_once != emitted_twice {
            failures.push(format!(
                "{name}: emit is not stable across a reparse — a value changed meaning.\n{}",
                first_difference(&emitted_once, &emitted_twice)
            ));
            continue;
        }

        checked += 1;
    }

    // A silent zero would make this test pass while covering nothing.
    assert!(
        checked >= 50,
        "only {checked} fixtures were actually checked ({} skipped as unparseable) — \
         the corpus or the parser regressed",
        skipped.len()
    );

    assert!(
        failures.is_empty(),
        "{} of {} fixtures broke the emit/reparse invariant:\n\n{}",
        failures.len(),
        checked + failures.len(),
        failures.join("\n\n")
    );
}

/// Report the first differing line, with context — a raw diff of two large files is
/// unreadable in test output.
fn first_difference(a: &str, b: &str) -> String {
    for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
        if la != lb {
            return format!("  line {}:\n    first  emit: {la:?}\n    second emit: {lb:?}", i + 1);
        }
    }
    format!(
        "  identical for {} shared lines, but lengths differ: {} vs {} lines",
        a.lines().count().min(b.lines().count()),
        a.lines().count(),
        b.lines().count()
    )
}
