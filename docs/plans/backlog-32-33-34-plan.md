# Implementation plan — issues #34, #33, #32 (rev 2, post-review)

Spec: `backlog-32-33-34-spec.md`. Reviewed by advisor and kimi k3; findings verified
against source before adoption. Baseline: 919 tests via
`cargo test --workspace --exclude patchlang-python`, clippy clean.

**Phase order: 3 → 1 → 2 → 4.** The rename goes first so later phases are written against
the final type name. Sequential, not parallel: `cargo test` regenerates `bindings/*.ts`
and concurrent cargo runs contend for the target-dir lock.

Each phase ends with a **separate reviewing agent** that did not write the code.

---

## Corrections adopted from review

| Finding | Verified | Effect |
|---|---|---|
| `bus_name` is `sanitize_id(&bus.label)` (`routes.rs:186`), not a display name | ✅ confirmed | Phase 1 must not use it raw as a human-visible label |
| The legacy-fallback trigger payload **already exists twice** | ✅ confirmed | Phase 2 rewritten — no new payload needed |
| No file has both `BusOutput` types in scope | ✅ confirmed | Phase 3 rationale corrected; 4 sites, zero test files |
| CI runs **no** Python job (`ci.yml` = test + lint, both Rust) | ✅ confirmed | Phase 4 test is local-only; must be stated, not implied |
| kimi: frontend fallback at `emitterAssembly.ts:164-166` | ❌ **rejected** | Direct grep shows `236-238`, `DEFAULT_BUS_OUTPUT_LABEL` at `:99`. Kimi misread; keeping verified line numbers. |
| kimi: patchlang-python build blocker is real | ❌ **rejected** | Disproven empirically — `maturin build` works, wheel installs and imports |

---

## Phase 3 (first) — #33: rename the DTO struct

**Exactly 4 sites.** Not "every referencing site" — no test file references the DTO
`BusOutput` (tests use `BusOutputEmitInput`, or import `ast::BusOutput` explicitly).

- `builder/canvas_output.rs:45` (field type), `:153` (definition) — rename to
  `BusLoadOutput`
- `builder/canvas_load.rs:296`, `:403` — update references

**Corrected rationale.** The plan previously claimed two `BusOutput` types were live in one
scope and a rename would avert a silent flip. That is false: `canvas_load.rs` is the only
file with the `canvas_output::*` glob, and it does **not** import `ast::BusOutput`. So the
rename produces clean compile errors at exactly those two lines — loud, not silent.

The real reason the collision hid: `bindings/*.ts` generation is a separate process from
`cargo build`, so two same-named `#[ts(export)]` types never produce a Rust error at all.
Renaming the Rust type (rather than only `#[ts(rename)]`) is still preferred — it removes
the source-level ambiguity as well as the generated-file clash — but as tidiness, not as
bug-avoidance. Let the compiler drive it.

**Expected bindings:** `BusOutput.ts` corrected in place to the AST shape; new
`BusLoadOutput.ts` with `input_groups`, `insert_send`, `insert_return`. Nothing orphaned.

**Verify:** after regeneration, confirm no two exported types share an *effective exported
name* (post-rename, not by Rust ident — that is the check that catches a rename-induced
collision). `BusLoadOutput` is free today; confirm mechanically.

---

## Phase 1 — #34 R1: synthesized bus-output label

**File:** `builder/canvas_emit/routes.rs`, legacy fallback (~line 247).

Do **not** use `bus_name` — it is `sanitize_id(&bus.label)` (`routes.rs:186`), so a bus
shown as "Main L/R" would emit `output "Main_LR"` and that mangled string becomes the
permanent user-visible name. A bus-output label is quoted free text, not an identifier, so
it needs no sanitizing:

```rust
label: bus.display_name.clone()
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| bus_name.clone()),
```

This mirrors the frontend, which falls back to `bus.name` then `'Output'`
(`emitterAssembly.ts:236-238`, `DEFAULT_BUS_OUTPUT_LABEL` at `:99`).

**Comment must record** (this is a user-visible, permanent semantic choice):
- becomes `BusNamedOutput.name` on load (`canvas_load.rs:377`), visible in the bus manager;
- sticky — once emitted, the next load makes `named_outputs` non-empty so the modern path
  takes over and the name persists;
- accepted because emitting nothing drops routing and erroring breaks saves;
- unreachable from the current frontend — defense-in-depth, not a live break.

**Also write D024** in `docs/decisions.md`. The repo records user-visible semantic
decisions as D-numbers; the reasoning already exists in the spec and belongs where the next
person looks.

**Check:** the duplicate-bus-output-label DRC (`drc/structural/port_refs.rs:316-336`) must
not newly fire. Expected safe (`seen_labels` resets per bus; legacy branch emits exactly
one output) — confirm by running, do not assume.

**Watch:** if `display_name` can contain a `"`, Phase 1 hands #35 a new way to fire. Note
it; do not fix #35 here.

**Mutation check:** revert to `String::new()` — the Phase 2 assertions must fail.

---

## Phase 2 — #34 R2: emit→parse invariant

**The trigger payload already exists — twice.** The previous plan was wrong to call the
suite vacuous and to mandate building a new payload:

- `builder_tests/canvas_bus_route_tests.rs:45-56` — `named_outputs: vec![]`,
  `output_channels: vec![3, 4]`
- `builder_tests/canvas_insert_tests.rs:171-181` — `named_outputs: vec![]`,
  `output_channels: vec![2]` (written earlier today, for the split-io insert test)

Both emit `output ""` **today** and pass, because they assert only `patch.contains(...)`
and never call `load_from_patch`. The defect is already being produced inside the test
suite and simply never parsed.

**So Phase 2 is: route these through `load_from_patch`,** not invent a redundant payload.

**Helper location — not `canvas_roundtrip_tests.rs`.** That file has zero bus payloads and
does not import `load_from_patch`; putting the helper there would apply it to
label/template payloads and miss the two that actually trigger the bug. Put it in a shared
test-helper module reachable from the bus tests (`canvas_bus_route_tests.rs`,
`canvas_insert_tests.rs`, `canvas_load_tests.rs` — all already import `load_from_patch`).

```rust
fn assert_emit_parses(input: CanvasEmitInput, what: &str) {
    let patch = emit_from_canvas_input(input).expect("emit");
    if let Err(e) = load_from_patch(&patch, "") {
        panic!("emit produced text load_from_patch rejects ({what}): {e}\n---\n{patch}");
    }
}
```

**Mutation-sensitivity must be stated explicitly in the test file.** The two existing
legacy tests assert only port-name containment, so they stay green whether or not Phase 1
is applied — they are *not* sensitive to #34. The new parse assertions are the sole guard.
Without this noted, a reviewer could revert Phase 1, see the suite green, and misread it.

**If other violations surface:** file each as its own issue, exclude explicitly *in the
test file* with the issue number in the comment, report the list. Do not expand scope, do
not skip silently.

**Out of scope, explicitly:**
- **#35** (unescaped quotes) — that output *parses* and corrupts silently, so a
  "does it parse" assertion is structurally blind to it. No `display_name`-with-quote
  payload here; #35 owns it.
- **Proptest.** The repo has `builder_tests/property_tests.rs` and "generate arbitrary
  `CanvasEmitInput`" is a tempting adjacent idea that would blow the scope. This stays a
  fixed-payload invariant.

---

## Phase 4 — #32: pyo3 properties

**File:** `crates/patchlang-python/src/lib.rs:143-151`

```rust
#[pyo3(signature = (instance, port, index, label, props=None))]
fn set_label(&mut self, instance: &str, port: &str, index: u32, label: &str,
             props: Option<HashMap<String, String>>) -> PyResult<()> {
    self.inner
        .set_label(instance, port, index, label, props.unwrap_or_default())
        .map_err(|e| PyValueError::new_err(e.to_string()))
}
```

Native dict, not a JSON string — the WASM binding takes JSON only because JS has no dict
across that boundary. The `Option` carries no meaning (`None` and `Some({})` both forward
an empty map); it exists solely because pyo3 0.22 cannot default a bare `HashMap` in the
signature string. Don't build logic on the distinction.

**The test needs more setup than "set a property and check".** `set_label` is a method on
`ProgramBuilder` (`lib.rs:16`) and calls `require_instance`, so the test must construct the
builder, `add_template`, `add_instance`, *then* `set_label`, then assert on `format()`.
`tests/test_python.py` has never instantiated `ProgramBuilder` — it only calls module-level
`parse`/`validate`. Also assert the no-props call still works (back-compat).

**Environment — resolve this explicitly, do not paper over it.** `tests/test_python.py` is
a bare script (top-level asserts, no pytest discovery), run by `scripts/test-all.sh:17` as
`python tests/test_python.py` against whatever `python` is on PATH. Verification builds a
wheel into a scratch venv. Those are different interpreters.

And **CI runs no Python at all** — `ci.yml` has only `test` and `lint`, both Rust;
`release.yml` builds wheels but runs no tests. So:

- The test will **not** run in CI. State that plainly in the commit body — do not let its
  existence imply coverage.
- Adding a CI Python job is a reasonable follow-up; **file it, don't absorb it.**
- Run it locally against a freshly built wheel and paste the actual output in the report.
  `cargo check` is not verification: a pyo3 `signature` string and the Rust params can
  disagree and fail only at import time.

---

## Per-phase review

Fresh agent per phase, given the diff and the spec:

1. Does the code do what the spec requires — not merely something reasonable?
2. Is each new test **mutation-sensitive**? Break the implementation, confirm the intended
   test fails. For Phase 2 this is the primary question.
3. Any silent behaviour change not called out in a comment?
4. Rules: file sizes, DRY, naming, no magic numbers, explicit errors, `trash` not `rm`.

**Verify reviewer findings against source before acting.** Both reviewers made confident
factual errors this round — kimi on the Python blocker and on a frontend line citation,
and the first draft of this plan repeated an unverified claim about `bus_name`.

## Definition of done

- 919 + new tests green; clippy clean.
- Each phase mutation-checked, with the check shown.
- D024 written; issues updated; anything found-not-absorbed filed.
- Nothing pushed without explicit authorization.
