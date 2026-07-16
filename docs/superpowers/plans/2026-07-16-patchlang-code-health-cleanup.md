# PatchLang Code-Health Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring every oversized file in `crates/patchlang` under the 500-line target (700 hard max) by splitting along responsibility boundaries, and fix the ts-rs binding-fidelity gap where `#[serde(skip_serializing_if)]` Option fields are typed as always-present in generated TypeScript.

**Architecture:** Pure refactor + a binding-annotation fix. No behavior changes. Every split preserves the existing public surface (`pub`/`pub(crate)` items keep their paths; only internal items move and gain `pub(super)`). The full test suite (812 tests) is the safety net — it must stay green after every task. Splits follow the two patterns already established in this codebase: (1) **directory module** — convert `foo.rs` → `foo/mod.rs` + focused submodules (as done for `builder/canvas_emit/` and `drc/structural/`); (2) **sibling test modules + shared helpers** — extract a `*_helpers.rs` with `pub(super)` builders and split tests by concern into sibling files declared in the parent `mod.rs` (as done for `builder_tests/canvas_*`).

**Tech Stack:** Rust 2021, `cargo test`, `cargo clippy`, `cargo fix` (for mechanical unused-import cleanup), ts-rs (TypeScript binding generation via `#[ts(export)]`).

## Global Constraints

- **File size:** 700 lines is a hard ceiling; target under 500. Never treat 700 as a goal. (SignalCanvas code rule 2)
- **No RUST behavior change:** Tasks 1–6 are pure refactors; `cargo test -p patchlang` must report the **same** pass counts as before: `758` (lib) + `12` + `39` + `3` = 812 tests, 0 failed. (Task 7 is the exception — it *intentionally* changes the generated TypeScript bindings; that is its whole point and is not a Rust behavior change.)
- **No new warnings:** after each task, `cargo build -p patchlang --tests` must produce no new warnings beyond the pre-existing `ts-rs failed to parse serde attribute` notes (Task 7 reduces those from 47 to 2). To clear mechanical unused-import warnings introduced by moves, use `cargo fix -p patchlang --all-targets --allow-dirty --broken-code` — **`--all-targets`, not `--lib`**, because moved `#[cfg(test)]` modules are only compiled under the test cfg and `--lib` alone will not fix their imports.
- **Never use `rm`:** to delete a file after converting it to a directory module, use `trash <path>` (never `rm`, never Python `unlink`).
- **Preserve public paths:** do not change the module path of any `pub`/`pub(crate)` item that is referenced from outside its module. Only internal (`fn`, `const`) items move; grant them the minimum visibility (`pub(super)`) needed for the parent to call them.
- **Commit per task:** one commit per task, message prefix `refactor:` for splits, `fix:` for the ts-rs task.

## Out of Scope (separate efforts — do NOT attempt here)

- **Dual-emitter consolidation.** `FrontendV1/src/lang/emitterBuilder.ts` reimplements `builder/canvas_emit` in TypeScript; the two drift and issue #28 existed in both. Making the frontend call the Rust WASM emitter is a large architectural epic that needs its own brainstorming + spec. Note only.
- **Frontend half of issue #28.** The `crossDevicePortResolver` path in `emitterBuilder.ts` lives in the SignalCanvasFrontend repo and needs its own plan there.

---

## File Structure

Files created/modified by this plan (all under `crates/patchlang/`):

| File | Responsibility after cleanup |
|------|------------------------------|
| `src/drc/drc_tests_rules/mod.rs` | declares per-layer test submodules |
| `src/drc/drc_tests_rules/{structural,direction_electrical,mechanical_logical,temporal_flow,trace}.rs` | DRC rule tests grouped by layer |
| `src/builder_tests/unit_tests.rs` → `unit_tests/mod.rs` + submodules | builder unit tests split by section |
| `src/compat_tests.rs` → `compat_tests/mod.rs` + `{helpers,record,ports}.rs` | compat conversion tests + shared `span()` helper |
| `src/builder_tests/canvas_load_tests.rs` → sibling files + shared helpers | canvas-load tests split by concern |
| `src/parser.rs` → `parser/mod.rs` + submodules | parser split by grammar area (SOURCE — higher risk) |
| `src/import/easyschematic.rs` → `easyschematic/mod.rs` + submodules | EasySchematic importer split by phase (SOURCE — higher risk) |
| `src/builder/canvas_output.rs` and other `#[ts(export)]` structs | `#[ts(optional)]` added to skip-serialized Option fields |

---

## Task 1: Split `drc_tests_rules.rs` (1058 → per-layer files)

**Files:**
- Create dir: `crates/patchlang/src/drc/drc_tests_rules/`
- Create: `drc_tests_rules/mod.rs`, and one submodule file per layer group (below)
- Delete (via `trash`): `crates/patchlang/src/drc/drc_tests_rules.rs`
- Modify: none outside — `drc/mod.rs` already declares `mod drc_tests_rules;`, which resolves to the new directory automatically.

**Context:** The file is already partitioned into **9** inline `#[cfg(test)] mod <layer> { … }` blocks at these line boundaries: `structural`@2, `direction`@314, `mechanical`@403, `electrical`@488, `logical`@570, `temporal`@631, `flow`@684, `convention_c05`@828, `trace`@874. (Do not miss `convention_c05` — it sits *between* `flow` and `trace`.) Each block is self-contained (its own `use` imports inside the block). Extraction is moving each block into its own file with the `#[cfg(test)]` and `mod <layer> { … }` wrapper preserved verbatim. Confirm the block list before splitting with: `grep -nE "^mod [a-z_0-9]+ \{" crates/patchlang/src/drc/drc_tests_rules.rs` (note the `_0-9` in the character class — a bare `[a-z]+` misses `convention_c05`).

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: nothing other tasks depend on (test-only module).

- [ ] **Step 1: Record the baseline**

Run: `cargo test -p patchlang 2>&1 | grep "test result:"`
Expected: `test result: ok. 758 passed; 0 failed; 1 ignored …` (+ 12 + 39 + 3). Note these numbers.

- [ ] **Step 2: Create the directory module scaffold**

Create `crates/patchlang/src/drc/drc_tests_rules/mod.rs`:

```rust
//! DRC rule tests, split by design-rule layer. See docs/decisions.md D0xx.
mod structural;
mod direction_electrical;
mod mechanical_logical;
mod temporal_flow;
mod trace;
```

- [ ] **Step 3: Move each inline `mod` block into its grouped file (verbatim)**

Group to keep each file well under 500 lines (all 9 blocks accounted for):
- `structural.rs` ← `mod structural { … }` (lines 2–312).
- `direction_electrical.rs` ← `mod direction { … }` (314–401) + `mod electrical { … }` (488–568).
- `mechanical_logical.rs` ← `mod mechanical { … }` (403–486) + `mod logical { … }` (570–629).
- `temporal_flow.rs` ← `mod temporal { … }` (631–682) + `mod flow { … }` (684–826).
- `trace.rs` ← `mod convention_c05 { … }` (828–872) + `mod trace { … }` (874–end).

**Avoid double-nesting.** A file that holds a **single** layer module (`structural.rs`) must NOT wrap the tests in an inner `mod structural { … }` — that would make the path `drc_tests_rules::structural::structural`. Instead strip the inner wrapper: move that block's `use`s and `#[test] fn`s directly into `structural.rs`, and declare it `#[cfg(test)] mod structural;` in `mod.rs`. A file that **groups two** layer modules (`direction_electrical.rs`, etc.) keeps both inner `#[cfg(test)] mod direction { … }` / `mod electrical { … }` wrappers verbatim, and is declared as a plain `mod direction_electrical;` (the inner mods carry their own `#[cfg(test)]`). Do not alter any test body or `use` statement inside the blocks.

`drc_tests_rules/mod.rs`:

```rust
//! DRC rule tests, split by design-rule layer. See docs/decisions.md D0xx.
#[cfg(test)]
mod structural;          // single module — inner wrapper stripped
mod direction_electrical; // two modules — inner wrappers kept
mod mechanical_logical;
mod temporal_flow;
mod trace;
```

After the move, verify all 9 original layer-modules still exist: `grep -rhE "^mod [a-z_0-9]+ \{" crates/patchlang/src/drc/drc_tests_rules/ | wc -l` → `9`.

- [ ] **Step 4: Delete the original with `trash`**

Run: `trash crates/patchlang/src/drc/drc_tests_rules.rs`

- [ ] **Step 5: Compile and clear mechanical warnings**

Run: `cargo build -p patchlang --tests 2>&1 | grep -E "error|warning: unused"`
If unused-import warnings appear: `cargo fix -p patchlang --all-targets --allow-dirty --broken-code && cargo build -p patchlang --tests 2>&1 | grep -E "error|warning: unused"`
Expected: no output (clean).

- [ ] **Step 6: Verify tests unchanged and sizes compliant**

Run: `cargo test -p patchlang 2>&1 | grep "test result:"` — must match Step 1 counts exactly.
Run: `wc -l crates/patchlang/src/drc/drc_tests_rules/*.rs` — every file < 500.

- [ ] **Step 7: Commit**

```bash
git add crates/patchlang/src/drc/drc_tests_rules crates/patchlang/src/drc/drc_tests_rules.rs
git commit -m "refactor: split drc_tests_rules into per-layer test modules"
```

---

## Task 2: Split `unit_tests.rs` (963 → sectioned submodules)

**Files:**
- Create dir: `crates/patchlang/src/builder_tests/unit_tests/`
- Create: `unit_tests/mod.rs` + one submodule per `// ---` section
- Delete (via `trash`): `crates/patchlang/src/builder_tests/unit_tests.rs`
- Modify: none — `builder_tests/mod.rs` already declares `mod unit_tests;`, resolving to the directory.

**Context:** The file is organized by `// ---------` banner-comment sections (first boundaries at lines 13, 76, 166, 195, …). Each section is a group of related `#[test] fn`s. Any small helper `fn`s defined at file top (before the first `#[test]`) that are used by multiple sections must move to a shared `unit_tests/helpers.rs` with `pub(super)` visibility, imported via `use super::helpers::*;` in each submodule.

**Interfaces:**
- Consumes: nothing.
- Produces: nothing (test-only).

- [ ] **Step 1: Baseline** — `cargo test -p patchlang 2>&1 | grep "test result:"` (record counts).

- [ ] **Step 2: Read the file and list section banners with line ranges**

Run: `grep -nE "^// ---|^fn |^#\[test\]" crates/patchlang/src/builder_tests/unit_tests.rs`
Identify: (a) top-of-file helper `fn`s (shared builders), (b) 3–4 contiguous section groups that each land under ~450 lines.

- [ ] **Step 3: Extract shared helpers (only if ≥2 sections use them)**

Create `unit_tests/helpers.rs`:

```rust
//! Shared builders for builder unit tests.
use crate::builder::canvas_input::*;
// (move each shared helper fn here, prefixing with `pub(super) `)
```

- [ ] **Step 4: Create `unit_tests/mod.rs` and the grouped submodule files**

`unit_tests/mod.rs`:

```rust
//! Builder unit tests, split by concern.
mod helpers;          // omit this line if Step 3 produced no shared helpers
mod <group_a>;
mod <group_b>;
mod <group_c>;
```

Each `unit_tests/<group>.rs` begins with the imports the moved tests need (copy the original file's top-level `use` lines) plus `use super::helpers::*;` when helpers were extracted, then the verbatim `#[test] fn` bodies for that section.

- [ ] **Step 5: Delete original** — `trash crates/patchlang/src/builder_tests/unit_tests.rs`

- [ ] **Step 6: Compile + clear warnings** — same as Task 1 Step 5 (`cargo build --tests`, then `cargo fix` if needed). Expected clean.

- [ ] **Step 7: Verify** — tests match Step 1 counts; `wc -l crates/patchlang/src/builder_tests/unit_tests/*.rs` all < 500.

- [ ] **Step 8: Commit**

```bash
git add crates/patchlang/src/builder_tests/unit_tests crates/patchlang/src/builder_tests/unit_tests.rs
git commit -m "refactor: split builder unit_tests into concern-grouped modules"
```

---

## Task 3: Split `compat_tests.rs` (835 → submodules + shared `span()`)

**Files:**
- Create dir: `crates/patchlang/src/compat_tests/`
- Create: `compat_tests/mod.rs`, `compat_tests/helpers.rs`, and 2 grouped test files
- Delete (via `trash`): `crates/patchlang/src/compat_tests.rs`
- Modify: none — `lib.rs` already declares `mod compat_tests;`, resolving to the directory.

**Context:** The file has a `fn span() -> Span` helper at line 11 used across the tests, then a flat list of `#[test] fn`s starting at line 17 (`kv_to_string_record_basic`, `kv_to_string_record_port_ref_value`, …). Group the tests into two contiguous halves that each land under ~450 lines.

**Interfaces:**
- Consumes: nothing.
- Produces: nothing (test-only).

- [ ] **Step 1: Baseline** — record `test result:` counts.

- [ ] **Step 2: List the test functions to pick a split point**

Run: `grep -nE "^fn |^#\[test\]" crates/patchlang/src/compat_tests.rs`
Choose a boundary near the middle that keeps related tests together.

- [ ] **Step 3: Create `compat_tests/helpers.rs`**

```rust
//! Shared helpers for compat conversion tests.
use crate::error::Span;

pub(super) fn span() -> Span {
    // (paste the exact body of the original `span()` here)
}
```

- [ ] **Step 4: Create `compat_tests/mod.rs` + two grouped test files**

`compat_tests/mod.rs`:

```rust
//! PatchLang compat-layer conversion tests.
mod helpers;
mod record_tests;
mod port_tests;
```

Each grouped file: copy the original top-level `use` lines, add `use super::helpers::span;`, then the verbatim `#[test] fn` bodies for its half.

- [ ] **Step 5: Delete original** — `trash crates/patchlang/src/compat_tests.rs`

- [ ] **Step 6: Compile + clear warnings** (as Task 1 Step 5). Expected clean.

- [ ] **Step 7: Verify** — counts match; `wc -l crates/patchlang/src/compat_tests/*.rs` all < 500.

- [ ] **Step 8: Commit**

```bash
git add crates/patchlang/src/compat_tests crates/patchlang/src/compat_tests.rs
git commit -m "refactor: split compat_tests into helper + grouped test modules"
```

---

## Task 4: Split `canvas_load_tests.rs` (773 → sibling test files)

**Files:**
- Create: `crates/patchlang/src/builder_tests/canvas_load_helpers.rs` (only if shared builders exist), and one additional sibling test file
- Modify: `crates/patchlang/src/builder_tests/canvas_load_tests.rs` (keep ~half the tests), `crates/patchlang/src/builder_tests/mod.rs` (add the new `mod` line)

**Context:** Organized by `// ---------` banners (first at lines 6, 53, 105, 141, 152, …). This mirrors the already-completed `canvas_roundtrip_tests` split: keep `canvas_load_tests.rs` as one concern-half and move the other half into a new sibling module. If shared builder `fn`s exist at file top, move them to `canvas_load_helpers.rs` with `pub(super)` and import via `use super::canvas_load_helpers::*;` in both files. Reuse `canvas_test_helpers` (already present in `builder_tests/`) if the load tests use the same `make_interface`/`make_simple_instance` builders — check before duplicating.

**Interfaces:**
- Consumes: possibly `super::canvas_test_helpers::{make_interface, make_simple_instance}` (already `pub(super)`).
- Produces: nothing (test-only).

- [ ] **Step 1: Baseline** — record `test result:` counts.

- [ ] **Step 2: Inventory the file**

Run: `grep -nE "^// ---|^fn |^#\[test\]|^use " crates/patchlang/src/builder_tests/canvas_load_tests.rs`
Decide the split boundary (a banner near the middle) and whether any top-of-file helpers are shared. Check whether load tests already reference `canvas_test_helpers`.

- [ ] **Step 3: (If needed) extract shared load-test helpers**

Create `crates/patchlang/src/builder_tests/canvas_load_helpers.rs` with the shared `fn`s, each `pub(super)`, and the imports they need. Skip this file if the tests only use `canvas_test_helpers` builders.

- [ ] **Step 4: Move the second half into a new sibling file**

Create `crates/patchlang/src/builder_tests/canvas_load_extra_tests.rs`:

```rust
//! Additional canvas-load tests (split from canvas_load_tests.rs for size).
// (copy the original file's top-level `use` lines here)
// (paste the second-half #[test] fns verbatim)
```

Remove those same `#[test] fn`s from `canvas_load_tests.rs`.

- [ ] **Step 5: Register the new module(s)**

Edit `crates/patchlang/src/builder_tests/mod.rs`: after `mod canvas_load_tests;` add `mod canvas_load_extra_tests;` (and `mod canvas_load_helpers;` if Step 3 created it).

- [ ] **Step 6: Compile + clear warnings** (as Task 1 Step 5). Remove any now-unused `use` (e.g., `HashMap`) reported.

- [ ] **Step 7: Verify** — counts match; `wc -l crates/patchlang/src/builder_tests/canvas_load*.rs` all < 500.

- [ ] **Step 8: Commit**

```bash
git add crates/patchlang/src/builder_tests
git commit -m "refactor: split canvas_load_tests to satisfy file-size rule"
```

---

## Task 5: Split `parser.rs` (732 → grammar-area submodules) — SOURCE, HIGHER RISK

> **CRITICAL — do NOT convert `parser.rs` to `parser/mod.rs`.** A `crates/patchlang/src/parser/` directory **already exists** and holds this module's test submodules (`test_helpers.rs`, `tests_basic.rs`, `tests_declarations.rs`, `tests_instance_body.rs`), which `parser.rs` declares at lines 726–732 via `mod test_helpers;` etc. In Rust a `foo.rs` file and a `foo/` sibling directory coexist (the `.rs` is the module root; the directory holds its submodules). `parser.rs` therefore STAYS as the module root — we shrink it by moving method groups into NEW sibling files under the existing `parser/` directory, declared with additional `mod` lines in `parser.rs`.

**Files:**
- Modify: `crates/patchlang/src/parser.rs` (remove the moved methods; add `mod statements;` / `mod refs;` declarations)
- Create: `crates/patchlang/src/parser/statements.rs`, `crates/patchlang/src/parser/refs.rs` — each an additional `impl Parser` block for one grammar area
- Do NOT create `parser/mod.rs`. Do NOT delete `parser.rs`. Do NOT touch the existing `parser/tests_*.rs` files.

**Context:** `parser.rs` contains `struct Parser<'a>` and one main `impl<'a> Parser<'a>` block (starts ~line 39) plus a `impl<'a> TemplateParserExt for Parser<'a>` block (~line 671), then the test-module `mod` declarations at the bottom. Rust allows a type's inherent `impl` to be split across multiple files in the same module, so the clean split is: keep the struct, constructor, token-cursor helpers (`peek`/`advance`/`expect`/`current_span`/`at_end`), top-level statement dispatch, and the `TemplateParserExt` impl in `parser.rs`; move two cohesive method groups into additional `impl Parser` blocks in new sibling files. Method **visibility must be preserved exactly**: methods currently `pub(crate)` (e.g. `parse_port_ref`) stay `pub(crate)`; private methods that the moved blocks call, or that the retained code calls on moved methods, must be raised to `pub(super)` (visible within the `parser` module). Because this is production parsing code, verify with the full suite AND the parser's own tests.

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: `Parser` keeps its existing path `crate::parser::Parser` and every `pub(crate)` method signature unchanged. The module path of the moved methods is unchanged (still `Parser::<method>`), since they remain `impl Parser` in the same module.

- [ ] **Step 1: Baseline** — `cargo test -p patchlang 2>&1 | grep "test result:"` (record) and confirm parser tests: `cargo test -p patchlang parser:: 2>&1 | grep "test result:"`.

- [ ] **Step 2: Map the impl blocks and method visibilities**

Run: `grep -nE "^impl |^    (pub\(crate\) )?fn |^    fn |^    pub fn |^struct Parser|^mod " crates/patchlang/src/parser.rs`
Group the main `impl Parser` methods into: **retained** (struct, `new`, cursor helpers, top-level dispatch — stay in `parser.rs`), **statements** (instance/template/connect/bridge/ring/config/signal parsing → `statements.rs`), **refs** (`parse_port_ref`, `parse_optional_index`, `parse_route_entry`, index/range parsing → `refs.rs`). Record each method's current visibility keyword. Leave the `TemplateParserExt` impl and the bottom `mod tests_*;` declarations untouched in `parser.rs`.

- [ ] **Step 3: Create `parser/statements.rs` and `parser/refs.rs`**

Each new file:

```rust
//! <grammar area> parsing — additional `impl Parser` block split from parser.rs.
use super::*;               // brings Parser, its use-imports, and shared types into scope

impl<'a> Parser<'a> {
    // (paste the verbatim method bodies for this grammar area from parser.rs,
    //  preserving each method's original visibility keyword exactly)
}
```

If `use super::*;` does not resolve an AST type the moved block constructs (because `parser.rs`'s top-level `use` imports are private and not re-exported), add the specific `use crate::ast::{…};` / `use crate::token::…;` lines that block needs — copy them from `parser.rs`'s import list.

- [ ] **Step 4: Shrink `parser.rs`**

Remove the two moved method groups from the main `impl Parser` block. Add the submodule declarations near the existing `mod tests_*;` block (or just after the imports):

```rust
mod statements;
mod refs;
```

Any private method still called across the split (retained code → moved method, or moved code → retained method) must be `pub(super) fn` in its defining file.

- [ ] **Step 5: (removed — `parser.rs` is retained, not deleted)**

- [ ] **Step 6: Compile + resolve visibility errors**

Run: `cargo build -p patchlang 2>&1 | grep -E "error"`
For each `private … is not accessible` error, raise that method from `fn` to `pub(super) fn` in **its defining file** (`parser.rs` for retained methods, `statements.rs`/`refs.rs` for moved ones). Re-run until clean. Then `cargo fix -p patchlang --all-targets --allow-dirty --broken-code` for unused imports.

- [ ] **Step 7: Verify — full suite AND parser tests unchanged**

Run: `cargo test -p patchlang 2>&1 | grep "test result:"` (match Step 1).
Run: `cargo clippy -p patchlang 2>&1 | grep -E "parser" ` (no new warnings).
Run: `wc -l crates/patchlang/src/parser.rs crates/patchlang/src/parser/statements.rs crates/patchlang/src/parser/refs.rs` — all < 500 (the existing `parser/tests_*.rs` are unchanged and out of scope).

- [ ] **Step 8: Commit**

```bash
git add crates/patchlang/src/parser.rs crates/patchlang/src/parser/statements.rs crates/patchlang/src/parser/refs.rs
git commit -m "refactor: split parser into grammar-area modules"
```

---

## Task 6: Split `easyschematic.rs` (852 → phase submodules) — SOURCE, HIGHER RISK

**Files:**
- Create dir: `crates/patchlang/src/import/easyschematic/`
- Create: `easyschematic/mod.rs` (public entry point + re-exports), `easyschematic/types.rs` (`SchematicFile`, `EsPort`, `EsDeviceData`, `ImportError` + trait impls), `easyschematic/convert.rs` (the import/conversion logic), and `easyschematic/tests.rs` (the `#[cfg(test)] mod tests` if present).
- Delete (via `trash`): `crates/patchlang/src/import/easyschematic.rs`
- Modify: none — `import/mod.rs` declaration of `mod easyschematic;` resolves to the directory.

**Context:** Unlike `parser.rs` (Task 5), `easyschematic.rs` has **no** coexisting `import/easyschematic/` directory (verified — its only submodule is the inline `#[cfg(test)] mod tests { … }` at line 468), so converting it to `easyschematic/mod.rs` + submodules is safe. The file mixes serde input types (`struct SchematicFile`@20, `impl EsPort`@84, `impl EsDeviceData`@115), the `ImportError` error type with its `Display`/`Error`/`From` impls (@134–168), and the conversion logic, plus that inline test module. Split by responsibility: data types, error type, conversion, tests. The public entry function (the `pub fn` that `import/mod.rs` calls) stays in `mod.rs` or is re-exported from it so its path `crate::import::easyschematic::<fn>` is unchanged.

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: the public import entry point keeps its existing path and signature (`import/mod.rs` and any callers must not change).

- [ ] **Step 1: Baseline** — record `test result:` counts; note the public entry fn: `grep -n "pub fn" crates/patchlang/src/import/easyschematic.rs`.

- [ ] **Step 2: Map sections**

Run: `grep -nE "^// ---|^pub fn |^fn |^struct |^impl |^enum |^#\[cfg\(test\)\]|^mod tests" crates/patchlang/src/import/easyschematic.rs`
Assign each item to: `types.rs` (structs + their inherent impls), `error.rs`-or-inline (`ImportError` + Display/Error/From), `convert.rs` (conversion fns + private helpers), `tests.rs` (the test module).

- [ ] **Step 3: Create `easyschematic/mod.rs`**

```rust
//! EasySchematic → PatchLang importer.
mod types;
mod convert;
#[cfg(test)]
mod tests;

pub use convert::<public_entry_fn>;   // keep crate::import::easyschematic::<fn> stable
// (move the ImportError type here or into types.rs; re-export if external code names it)
```

- [ ] **Step 4: Create `types.rs`, `convert.rs`, `tests.rs`**

Move each item verbatim, adding per-file `use` imports. Grant `pub(super)` to any type/fn/field crossing a file boundary. In `tests.rs`, the module body is `use super::*;` plus the moved `#[test] fn`s.

- [ ] **Step 5: Delete original** — `trash crates/patchlang/src/import/easyschematic.rs`

- [ ] **Step 6: Compile + resolve visibility/import errors** (as Task 5 Step 6). Expected clean after raising cross-file items to `pub(super)` and `cargo fix`.

- [ ] **Step 7: Verify** — counts match; no new clippy warnings on `easyschematic`; `wc -l crates/patchlang/src/import/easyschematic/*.rs` all < 500.

- [ ] **Step 8: Commit**

```bash
git add crates/patchlang/src/import/easyschematic crates/patchlang/src/import/easyschematic.rs
git commit -m "refactor: split easyschematic importer into phase modules"
```

---

## Task 7: Fix ts-rs binding fidelity — `#[ts(optional)]` on skip-serialized Option fields

**Files:**
- Modify: every `#[ts(export)]` struct that has an **`Option`** field annotated `#[serde(skip_serializing_if = "Option::is_none")]` — **45 fields**. Confirmed: `src/builder/canvas_output.rs` (`BusNamedOutput.output_instance`, `RouteRuleOutput.from_instance`/`to_instance`, `BusOutput.display_name`). Others: `src/layout_validator.rs`, `src/error.rs`, `src/compat_types.rs`, `src/drc/types.rs`, `src/graph/types.rs` — audit each with the Step 6 grep.
- Verify: regenerated `.ts` files under `crates/patchlang/bindings/`.

**Context:** ts-rs (v10 — confirmed in `crates/patchlang/Cargo.toml`) cannot parse `#[serde(skip_serializing_if = ...)]` and ignores it, producing **47** "failed to parse serde attribute" warnings. Consequently a field serde **omits** when `None` is typed in TypeScript as `field: T | null` (always present); the frontend can read `undefined` where the type promised `null`. Fix verified against the ts-rs 10 source (`~/.cargo/.../ts-rs-10.*/src/lib.rs`): `#[ts(optional)]` generates `field?: T` (docs ~line 316) and `fn decl() -> String` exists (~line 415) for the Step 2 assertion. `#[ts(optional)]` sits alongside the existing `#[serde(...)]` on the same field. Bindings regenerate when tests run (ts-rs exports during `cargo test`).

**Scope caveat — 2 collection fields:** of the 47 warnings, **2** are on non-`Option` fields using `Vec::is_empty` / `BTreeMap::is_empty`. `#[ts(optional)]` is only valid on `Option` fields (the macro rejects it otherwise), so those 2 are out of scope and remain as **2 residual warnings** — acceptable, since an omitted-when-empty array/map reads the same as `[]`/`{}` for TS consumers. Do **not** annotate them. Success = 47 → 2, and the 2 survivors are exactly those collection fields (verify in Step 7).

**Interfaces:**
- Consumes: nothing.
- Produces: corrected `.ts` bindings; no Rust API change.

- [ ] **Step 1: Baseline — capture the warning count**

Run: `cargo build -p patchlang 2>&1 | grep -c "skip_serializing_if"`
Expected: `47`. (Also note `cargo test -p patchlang 2>&1 | grep "test result:"` counts.) Record which 2 are non-Option: `grep -rn "skip_serializing_if" crates/patchlang/src/ | grep -viE "Option::is_none"` — these are the collection fields that stay.

- [ ] **Step 2: Write a failing binding assertion**

Add to `crates/patchlang/src/builder/canvas_output.rs` (or a nearby `#[cfg(test)] mod` in that file) a test that regenerates and inspects the binding:

```rust
#[cfg(test)]
mod ts_binding_tests {
    use super::*;
    use ts_rs::TS;

    #[test]
    fn route_rule_output_instance_fields_are_optional() {
        let decl = RouteRuleOutput::decl();
        // skip-serialized Option fields must be TS-optional (`field?:`),
        // not always-present (`field: … | null`).
        assert!(
            decl.contains("from_instance?:") && decl.contains("to_instance?:"),
            "expected optional instance fields, got:\n{decl}"
        );
    }
}
```

- [ ] **Step 3: Run it to confirm it fails**

Run: `cargo test -p patchlang route_rule_output_instance_fields_are_optional -- --nocapture`
Expected: FAIL — the decl shows `from_instance: string | null` (no `?`).

- [ ] **Step 4: Add `#[ts(optional)]` to the three `canvas_output.rs` fields**

For each of `BusNamedOutput.output_instance`, `RouteRuleOutput.from_instance`, `RouteRuleOutput.to_instance` (and `BusOutput.display_name` if it is `Option` + skip-serialized), add the attribute:

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub from_instance: Option<String>,
```

- [ ] **Step 5: Run the test to confirm it passes**

Run: `cargo test -p patchlang route_rule_output_instance_fields_are_optional`
Expected: PASS.

- [ ] **Step 6: Apply the same annotation to the remaining structs**

For each file from the Files list, find `skip_serializing_if` **Option** fields and add `#[ts(optional)]`:
Run: `grep -rn -B2 "skip_serializing_if = \"Option::is_none\"" crates/patchlang/src/`
Add `#[ts(optional)]` to each such field **only if the field type is `Option<...>`**. Do NOT annotate the 2 `Vec::is_empty`/`BTreeMap::is_empty` fields (the macro rejects `#[ts(optional)]` on non-Option; they stay as residual warnings). Re-run `cargo build -p patchlang 2>&1 | grep -c "skip_serializing_if"` after each file; the count must strictly decrease toward **2**.

- [ ] **Step 7: Verify — Option warnings gone, bindings correct, tests green**

Run: `cargo build -p patchlang 2>&1 | grep -c "skip_serializing_if"` → `2` (the collection fields only).
Run: `cargo build -p patchlang 2>&1 | grep -A3 "skip_serializing_if" | grep -E "Vec::is_empty|BTreeMap::is_empty" | wc -l` → confirms both survivors are collection fields (not Option).
Run: `grep -h "instance" crates/patchlang/bindings/RouteRuleOutput.ts` → shows `from_instance?:` / `to_instance?:`.
Run: `cargo test -p patchlang 2>&1 | grep "test result:"` → matches Step 1 counts + the 1 new test.

- [ ] **Step 8: Commit**

```bash
git add crates/patchlang/src crates/patchlang/bindings
git commit -m "fix: mark skip-serialized Option fields ts(optional) for TS binding fidelity"
```

---

## Task 8: Final full-crate verification

**Files:** none (verification only).

- [ ] **Step 1: No file over the hard limit**

Run: `find crates/patchlang/src -name "*.rs" | xargs wc -l | awk '$1>700 && $2!="total"'`
Expected: no output.

- [ ] **Step 2: Full suite green, no new warnings**

Run: `cargo test -p patchlang 2>&1 | grep "test result:"` (812 baseline + Task 7's new test, 0 failed).
Run: `cargo build -p patchlang --tests 2>&1 | grep "warning:" | grep -viE "ts-rs|serde attribute"` → empty (the only remaining warnings are the **2** residual `skip_serializing_if` notes on the collection fields from Task 7's scope caveat).
Run: `cargo clippy -p patchlang --all-targets 2>&1 | grep -E "warning:|error:" | grep -viE "ts-rs|serde attribute"` → empty.

- [ ] **Step 3: Commit any stray binding regenerations**

```bash
git add -A && git commit -m "chore: regenerate ts bindings after code-health cleanup" --allow-empty
```

---

## Notes for the executor

- **Order:** do the four test-file splits (Tasks 1–4) first — they are the lowest-risk and build confidence in the module-split mechanics before the two source-file splits (Tasks 5–6).
- **If a source split (Task 5/6) gets stuck** on tangled visibility after 3 attempts, STOP and reconsider the grouping boundary rather than sprinkling `pub(super)` — a bad boundary is the usual cause (see systematic-debugging Phase 4.5).
- **`cargo test` regenerates ts-rs bindings** as a side effect; expect `crates/patchlang/bindings/*.ts` to show as modified after Task 7 — that is intended and committed in that task.
