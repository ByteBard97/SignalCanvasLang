# Spec — issues #32, #33, #34

Three defects found while shipping #31, filed rather than absorbed. All are in
SignalCanvasLang. None are blocked on Reid.

---

## #34 — Canvas emit produces `.patch` our own parser rejects

### Observed

`emit_from_canvas_input` with a bus that has an empty `named_outputs` and the deprecated
flat `output_interface`/`output_channels` emits successfully, then `load_from_patch` on
that exact text fails:

```
validation error: parse error(s): error at 289..291:
Bus output label must not be empty (hint: Provide a name: output "Link 1-L": Port[1])
```

Reproduced through the real WASM boundary with inserts stripped out, so it predates #31.

**Reachability — this is a latent defect, not a live one.** The current frontend already
guards it: `emitterAssembly.ts:236-238` computes
`safeName = rawOutName || (bus.name || 'Output')`, so it never sends an empty output name,
and it only leaves `named_outputs` empty when `output_channels` is empty too — which takes
the other branch. So the buggy path is unreachable from the only supported caller; the
repro used a hand-built payload.

That lowers the urgency but not the priority: the emitter and parser must agree regardless
of who calls them, and R2 (the invariant) is where the real value is. The "Observed"
framing above should not be read as a live user-facing break.

**Useful corroboration for R1:** the frontend's own fallback is the **bus name**, falling
back to `'Output'`. So R1 choosing the bus name is not an arbitrary preference — it makes
Rust agree with the behaviour the frontend already shipped.

### Cause

`canvas_emit/routes.rs:247-251` — the legacy fallback hardcodes an empty label:

```rust
vec![BusOutput {
    label: String::new(),
    destinations: dests,
    span: builder_span(),
}]
```

`formatter_emit::emit_bus_entry` writes that as `output ""`, and
`body_parser::parse_bus_entry` rejects an empty bus-output label — a rule introduced with
the named-output syntax (D017). The emitter and the parser disagree.

### Requirements

**R1.** The legacy fallback must emit a non-empty label. Use the **bus name**: it is
already in scope as `bus_name`, is meaningful, and is unique within the instance.
`"Out"` would collide across buses on the same device.

**R1a — this is a data decision, not cosmetics, and must be justified as one.** The label
is not internal plumbing: on load `BusOutput.label` becomes `BusNamedOutput.name`, which
surfaces in the frontend's bus manager as a named output **the user never created**. Worse,
it is self-perpetuating — once emitted, the next load produces a non-empty `named_outputs`,
so the modern path takes over and the synthesized name is permanent in the file.

Accepted anyway, because the alternatives are worse: emitting nothing silently drops the
routing, and erroring out breaks saves for anyone still on the legacy path. A visible,
predictable, editable name beats data loss. But state this in the commit body — a reviewer
who notices "the legacy path invents a persistent user-visible output name" should find
the reasoning already there rather than have to raise it.

**R1b — check the duplicate-label DRC.** `drc/structural/port_refs.rs` warns on duplicate
bus output labels. Confirm that synthesizing the bus name cannot produce a duplicate and
fire a new diagnostic on files that were previously clean. If it can, the label needs
disambiguation.

**R2.** A general invariant: **anything `emit_from_canvas_input` produces must
`load_from_patch` cleanly.** This is the valuable half. R1 fixes one instance; R2 catches
the class. Implement as a test helper asserting emit → parse → no errors, applied across
the existing canvas-emit test payloads plus the legacy-bus payload from R1.

**R2a — policy for when R2 fires elsewhere.** The invariant is not known to hold today
outside the paths already tested. If applying it surfaces *other* pre-existing violations,
do **not** silently expand this ticket or quietly skip the failing payloads. Instead: fix
#34's instance, file each additional violation as its own issue, and land the invariant
scoped to what passes — with the exclusions listed **explicitly in the test file**, each
naming its issue number. A test that quietly skips broken cases reads as "covered" and is
worse than no test.

**R2b — guard against vacuousness. This is the requirement most likely to be satisfied
in form and not in substance.** Nearly every existing canvas-emit payload populates
`named_outputs`, so the invariant already passes for them and proves nothing about this
defect. Only two sites use `named_outputs: vec![]`
(`builder_tests/canvas_bus_route_tests.rs:54`, `builder_tests/canvas_insert_tests.rs:177`)
and neither is confirmed to pair that with **non-empty `output_channels`** — which is the
exact trigger.

So R2 must include a payload of precisely that shape: **empty `named_outputs` AND
non-empty `output_channels`**. Not "a legacy-bus payload" loosely; that shape. The
acceptance test is that R2 **fails when R1 is reverted**. If it still passes, R2 is
decorative and must be reworked.

**R2c — hook location.** Alongside the existing round-trip helpers in
`builder_tests/canvas_roundtrip_tests.rs`, as a helper asserting
`load_from_patch(&emit_from_canvas_input(input)?, "")` is `Ok`.

**R2d — R2 will not catch #35, by design.** Issue #35 (unescaped quotes in
`emit_bus_entry`, filed during this review) is the same family — emitter output the parser
mishandles — but it *parses successfully* and silently truncates the value. An
"does it parse" invariant is blind to it. #35 needs a different assertion:
*does the value survive*. Do not treat R2 as covering it; do not expand this ticket to fix
it. Cross-reference only.

**R3.** No migration concern, and the reason should be stated in the commit: no `.patch`
on disk can contain `output ""`, because such a file has never parsed. The bad value only
exists in memory between emit and load. Nothing to be backward-compatible with.

### Non-requirements

- Not deprecating or removing the flat `output_interface`/`output_channels` path. It
  exists for backward compatibility and is out of scope.
- Not changing the parser to accept empty labels. D017 decided labels are required; the
  emitter is the side that is wrong.

### Acceptance

- The exact repro payload emits and then loads with zero errors.
- The R2 invariant test exists and fails if R1 is reverted.
- Existing 919 tests still pass; clippy clean.

---

## #33 — ts-rs binding collision drops the canvas DTO's TypeScript

### Observed

Two distinct types both derive `#[ts(export)]` and are both named `BusOutput`:

- `ast.rs:281` — the AST node (`label`, `destinations`, `span`)
- `builder/canvas_output.rs:129` — the canvas load DTO (`name`, `display_name`,
  `input_port`, `input_channels`, `input_groups`, `named_outputs`, `insert_send`,
  `insert_return`)

Both write `bindings/BusOutput.ts`; last writer wins. Currently the AST shape lands there,
so **the canvas DTO has no generated TypeScript at all** — including `input_groups`
(v0.3.1) and the insert fields (v0.3.2).

Not currently breaking anything: the frontend hand-maintains
`src/types/CanvasLoadOutput.ts`. But the generated bindings silently misrepresent the DTO,
which is a trap for anyone who starts consuming them.

### Requirements

**R1.** The two types must generate to distinct TypeScript files. Prefer
`#[ts(rename = "BusLoadOutput")]` on the **DTO** — its siblings in the same module already
use the `*LoadOutput` convention (`InstanceLoadOutput`, `PortLoadOutput`,
`ConnectionLoadOutput`, `RingLoadOutput`, `NetworkLoadOutput`), so the DTO is the one
that is off-convention. Renaming the Rust type itself is the tidier end state but touches
more call sites; decide during planning, note the tradeoff.

**R1 (revised) — rename the Rust type, not just its export.** `#[ts(rename)]` *does*
change the output filename — confirmed in the ts-rs 10.1.0 source (`types/mod.rs:27-30`
sets `ts_name` from `attr.rename`; `lib.rs:42-49` derives the path as
`format!("{}.ts", ts_name)`), so no fallback is needed and `#[ts(export_to)]` is
unnecessary.

But rename-only leaves **two Rust types named `BusOutput`** in different modules, which is
precisely the ambiguity that let this collision go unnoticed — and `canvas_load.rs`
glob-imports `canvas_output::*` while also referencing `ast::BusOutput` via `bus.outputs`,
so both are live in one file. Rename the DTO struct itself to `BusLoadOutput`, matching its
siblings (`InstanceLoadOutput`, `PortLoadOutput`, `ConnectionLoadOutput`, …). More call
sites, but it fixes the source hazard rather than just the generated artifact.

**R1b — no orphaned file on this path.** With the DTO renamed, `BusOutput.ts` is
*corrected in place* to the AST shape and a new `BusLoadOutput.ts` appears. Nothing is
orphaned. (An orphan would only occur if the *AST* type were renamed instead.) Do not go
hunting for a file to delete.

**R2 — audit already done during review; do not redo it, verify it.** All 77
`#[ts(export)]` types were enumerated against the 77 generated files: **`BusOutput` is the
only collision.** No other duplicate type names across modules, and there are currently
zero `#[ts(rename)]`/`#[ts(export_to)]` attributes in the crate.

**R2a — but audit by *effective exported name*, not Rust ident.** The audit above compares
Rust identifiers, which would miss a collision *introduced* by a rename (renaming A to an
existing B's name silently makes both write `B.ts`). `BusLoadOutput` is free today, but
confirm that mechanically after the rename rather than by eye — and note the technique so
the next person renaming a type checks the right thing.

**R3.** Any newly generated or renamed binding file must be committed. Use `trash`, never
`rm`, if anything does need removing.

### Non-requirements

- Not changing `src/types/CanvasLoadOutput.ts` in the frontend. That is Reid's file and a
  separate repo. Flag on the issue that generated bindings are now trustworthy.

### Acceptance

- `bindings/BusOutput.ts` and the DTO's binding both exist with correct, distinct shapes.
- The DTO binding contains `insert_send`, `insert_return`, `input_groups`.
- Collision audit reported.
- Tests pass; clippy clean.

---

## #32 — `patchlang-python`'s `set_label` forwards no properties

### Observed

`patchlang-python/src/lib.rs:143-151` hard-codes an empty map:

```rust
fn set_label(&mut self, instance: &str, port: &str, index: u32, label: &str) -> PyResult<()> {
    self.inner.set_label(instance, port, index, label, HashMap::new())
}
```

So the Python binding can never set `phantom`, `source_type`, `capsule`, `rf_band`,
`insert_send`/`insert_return`, or any custom key. The WASM binding
(`patchlang-wasm/src/lib.rs:248`) takes a `props_json` argument and forwards it correctly;
Python is the outlier.

### Requirements

**R1.** `set_label` accepts an optional properties mapping and forwards it. Optional with a
default so existing Python callers are unaffected — this is a published wheel consumed by
the Django backend.

Signature: **`props: Option<HashMap<String, String>>`** with
`#[pyo3(signature = (instance, port, index, label, props=None))]`. This matches house
style — `lib.rs:177` already uses a `#[pyo3(signature = ...)]` with an `Option` default —
and pyo3 0.22 cannot default a bare `HashMap` to `None`. Note that `None` and
`Some(empty)` both forward as an empty map, so the distinction collapses; that is fine, but
do not build logic on it.

**Take a native Python dict, not a JSON string.** The WASM binding takes `props_json: &str`
and deserializes (`patchlang-wasm/src/lib.rs:248-259`) because it crosses a JS boundary
that has no dict. Python has one. Mirroring the WASM signature would import a workaround
into a language that does not need it.

**R2.** Python-side test coverage proving a property set from Python appears in
`format()` output.

**R3.** **There is no build blocker — this was my error, now disproven empirically.**
`cargo build -p patchlang-python` fails to link with undefined Python symbols, and I
wrongly recorded that as a pre-existing environment problem. It is the standard pyo3
`extension-module` gotcha: plain `cargo build` tries to link libpython. Verified working:

```
cargo check -p patchlang-python --features pyo3/extension-module   # clean
maturin build --manifest-path crates/patchlang-python/Cargo.toml   # builds the wheel
```

The wheel installs into a venv and imports; `ProgramBuilder` exposes `set_label`,
`format`, `check`, `to_json` and friends. So #32 **must be properly verified** with a real
Python test that builds the wheel, installs it, sets a property, and asserts it appears in
`format()` output. `cargo check` alone does not count as verification. Do not report this
change as done on the strength of Rust compilation.

### Non-requirements

- Not bumping the backend's pinned wheel URL (`requirements.txt:22`, currently v0.2.13).
  That is a Backend-repo change and a separate conversation.

### Acceptance

- `set_label` forwards properties; existing call signature still works.
- Test exists. If it cannot be executed locally, that is stated explicitly in the commit
  body and the final report, with the blocker named.

---

## Cross-cutting

- Rules in `ClaudeCodeRules.md` apply: files under 500 lines, DRY, explicit error
  handling, no magic numbers, meaningful tests, `trash` not `rm`.
- Every behavioural test must be **mutation-checked** — break the implementation, confirm
  the intended test (and only it) fails, restore. A test that passes either way is
  evidence of nothing.
- Each implementation phase gets a **separate reviewing agent** that did not write the
  code.
- Do not push without explicit in-conversation authorization.

## Sequencing and phases

#34, #33 and #32 are logically independent but **cannot run concurrently in one working
tree**: `cargo test` regenerates `bindings/*.ts` as a side effect, and parallel cargo runs
contend for the target-dir lock. Either sequence them, or give each agent
`isolation: "worktree"`.

Phases, each ending with a **separate reviewing agent** that did not write the code and
gets a bounded diff:

| Phase | Scope | Reviewer checks |
|-------|-------|-----------------|
| 1 | #34 R1 — synthesized label | Round-trip name mutation documented; DRC duplicate-label unaffected |
| 2 | #34 R2 — emit→parse invariant | **Not vacuous**: fails when R1 reverted; exclusions listed with issue numbers |
| 3 | #33 — DTO rename + binding regen | No other collision by effective exported name; DTO binding has the insert fields |
| 4 | #32 — pyo3 props + Python test | Test actually executed against a built wheel, not just `cargo check` |

## Verification bar

- `cargo test --workspace --exclude patchlang-python` is the command behind any test-count
  claim (919 at the start of this work). `patchlang-python` is excluded because plain
  `cargo build` cannot link it; use `maturin` for that crate (see #32 R3).
- `cargo clippy --workspace --exclude patchlang-python --all-targets` must be clean. CI
  Lint has been green since `a433675`; do not regress it.
- Every behavioural test mutation-checked.
- Do not push without explicit in-conversation authorization.

## Found during review, filed not absorbed

- **#35** — `formatter_emit` escapes nothing, so a bus `display_name` containing a quote is
  **silently truncated** (`The "Big" Mix` → `The `). Verified through WASM. Same family as
  #34 and same function, but it parses cleanly and corrupts quietly, so R2 cannot catch it.
  Likely wider than buses — channel labels are user-typed free text too.
