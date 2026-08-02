# Issue #37 — stream channel selection (AES67 TX)

## Summary

Reid asked for a grammar addition so a `stream` can record *which* source channels
go on an AES67 flow. **No grammar work is required.** `PortRef` has always carried
`index: Option<IndexSpec>`, and `parse_body_with_port_ref_key` accepts any property
key, so both syntaxes proposed in the issue already parse, emit, and round-trip.

Verified against the current tree with `patchlang fmt` / `patchlang check`:

```patchlang
stream Drums {
  source: DM7.Dante_Out[7, 1, 5, 3]   # 0 errors; non-monotonic order preserved
  channels: "4"
  source_channels: "1,3,5,7"          # also survives — generic KV bag
}
stream Wide {
  source: DM7.Dante_Out[9, 1..4, 7]   # ranges parse; syntax is `..`, not `-`
}
```

The real gap is the canvas DTO bridge, which drops the selection in both directions.

## Decisions

**D-1: the file form is the index spec on the source port ref.**
`source: DM7.Dante_Out[1,3,5,7]`, not `source_channels: "1,3,5,7"`.

Rationale: structured rather than stringly-typed; the AST already models it; it is the
same mechanism connections use for channel selection (DRY); ranges come free. The DTO
still exposes a flat ordered `source_channels: Vec<u32>`, so the frontend's existing
`TxStream.sourceChannels` needs no change. We do **not** also accept the string form —
Reid's emitter has not shipped, so no files in the wild use it, and two ways to say one
thing is a permanent maintenance cost (YAGNI).

**D-2: `channels` stays, and a mismatch is diagnosed.**
Not derived. Keeping the property is non-breaking for every existing file and consumer,
and the issue's "stays == the list length" is enforced loudly rather than silently
recomputed.

**D-3: DRC is limited to checks local to the stream declaration.**
Bounds-checking indices against the source interface's real channel count needs
interface resolution inside the check pass; that is a larger change, filed as a
follow-up rather than bundled.

## Findings that shape the work

- `canvas_emit/structures.rs` hardcodes `index: None` and emits only
  `channels` / `direction` / `protocol`.
- `canvas_load.rs` reads only those three keys; everything else on the decl is dropped.
- `StreamEmitInput` (`canvas_input.rs`) and `StreamOutput` (`canvas_output.rs`) have no
  field able to carry a selection.
- **F02 is broken today.** It matches `channels` only as `KvValue::Num`, but
  `canvas_emit` writes it via `kv_str`. The AES67 8-channel limit therefore never fires
  on any canvas-emitted file. In scope: #37 is about channel counts.
- `IndexElement::Auto` has no meaning in a stream source.
- **`resolve_port_id` corrupts single-channel selections.** `graph/mod.rs:480` renders a
  one-element index as `Inst:Port_3` rather than `Inst:Port`. A legal 1-channel AES67
  flow would therefore compile to a `StreamIdentity.source_port` naming a port node that
  does not exist. Multi-channel selections fall through unharmed. This is the one real
  cost of D-1, and it is a fix (below), not a reason to prefer the string form.
- `edges::flatten_index_spec` already exists — reuse it on the load path rather than
  writing a second flattener.
- Out of scope, flagged to Reid, not fixed here: `canvas_emit` silently `continue`s on
  duplicate stream names; `sanitize_id(&stream.label)` is lossy on the display label
  (same class as #34/D024); `check`'s JSON emits a duplicated `"type": "Stream"` key.

## Verified before starting

- **F02's blast radius is nil.** Severity is `Info`, so it cannot fail CI, and every
  AES67 stream in `SignalCanvasFrontend/MTG.patch`, `tests/fixtures/examples/hillsong-mtg.patch`
  and `tests/fixtures/mtg-features/10-aes67-interop.patch` declares exactly 8 channels.
  Making F02 fire adds zero diagnostics to any file we have. It ships here.
  These same files confirm the diagnosis: canvas-emitted streams write `channels: "8"`
  (string, invisible to F02); the hand-authored fixture writes `channels: 8` (number),
  which is why F02's own tests pass.
- **`patchlang-python` needs no change** — it has no stream surface beyond
  `removed_stream_sources`.
- **`patchlang-wasm` `add_stream` needs no change** — it deserializes `StreamDecl` via
  serde, so a `PortRef` index rides along for free. The canvas emit/load wasm entry
  points are separate and are covered by Phases 2–3.

## Stated limits

- **The selection does not reach `GraphLevel`.** After the `resolve_port_id` fix the
  index is discarded and `StreamIdentity` has no field for it, so the compiled graph
  loses the channel selection. Acceptable because the issue states the router fix
  already landed and emit/load is the only piece waiting — but this is the one real
  argument for the string form, so it goes to Reid explicitly rather than staying buried.
- **Range expansion is not idempotent.** Load expands `[9, 1..4, 7]` and emit writes only
  Singles, so a hand-authored `1..8` returns as `1, 2, 3, 4, 5, 6, 7, 8` after the first
  canvas save. Deliberate — the flattened form is what the frontend edits — but it is a
  file-mangling complaint waiting to happen and Reid should hear it now.

## Phases

### Phase 1 — DTO fields
- Add `source_channels: Vec<u32>` to `StreamEmitInput`, defaulting to empty via serde so
  existing frontend payloads keep deserializing.
- Add `source_channels: Vec<u32>` to `StreamOutput`.
- Regenerate the `#[ts(export)]` bindings; keep the export-name uniqueness guard green.

### Phase 2 — emit
- In `emit_streams_for`, build `IndexSpec` from `source_channels` as ordered
  `IndexElement::Single`s, preserving the caller's order. Empty selection ⇒ `index: None`
  (byte-identical output to today for every existing file).
- No range coalescing on emit: the selection is user intent and order is significant.
- **When a selection is present, derive `channels` from its length** rather than writing
  the frontend's `channel_count` verbatim. Canvas-emitted files are then self-consistent
  by construction and can never be born tripping F04; the rule exists for hand-authored
  files and for payloads where the two genuinely disagree. With no selection, emit
  `channel_count` exactly as today.
- Fix the stream source's port id so it resolves to `Inst:Port` regardless of its index.
  **`resolve_port_id` is shared by exactly three callers** — signal origins
  (`mod.rs:214`), stream sources (`:235`), and config labels (`:256`). Connections do
  **not** use it; their `_N` suffix comes from `edges.rs`. So the fix must be applied at
  the stream call site (or via a stream-specific variant), never inside `resolve_port_id`
  itself — a label on `Port[3]` depends on getting `Inst:Port_3` back to find its channel
  node.

### Phase 2b — single-channel regression test (write first, must fail first)
The 1-channel selection is the failure mode most likely to ship broken, because it is
legal, plausible in the field (a single talkback or timecode channel on its own flow),
and silently wrong rather than loud. Required cases:
- `stream S { source: DM7.Dante_Out[3] ... }` compiles to `source_port == "DM7:Dante_Out"`,
  and that port id resolves to a port that exists on the node.
- The same fixture round-trips: emit → parse → load yields `source_channels == [3]`.
- **A config label on `Port[3]` still resolves to `Inst:Port_3`** — this is the caller
  actually at risk of collateral damage. An earlier draft of this plan guarded
  connections instead, which would have tested nothing at all.
- A signal origin with a single-index port ref still resolves to `Inst:Port_3` — the
  third caller.
Each must be confirmed failing against the unfixed tree before the fix lands.

### Phase 3 — load
- Flatten `source.index` into the ordered `Vec<u32>` via the existing
  `edges::flatten_index_spec`: `Single` ⇒ one value, `Range { start, end }` ⇒ expanded
  inclusive in declaration order, `Auto` ⇒ skipped.
- Absent index ⇒ empty vec.
- **`[auto]` and an absent index both flatten to `[]`**, so `source: P[auto]` would
  otherwise round-trip to "no selection" with nothing said. That is why Phase 4 emits an
  `Info` on `Auto` in a stream source — the drop stays, but it stops being silent.

### Phase 4 — DRC
- Fix F02 to accept `channels` as either `Num` or a parseable `Str`. This is what makes
  the existing 8-channel rule actually fire. Blast radius verified nil above.
- F02 counts the selection length when a selection is present, else `channels`.
- New rule F04, Flow layer, `Warning`: `channels` disagrees with the selection length
  (D-2). Reads `channels` as `Num` **or** `Str` from the first line — that is exactly the
  trap that left F02 dead for its whole life.
- New rule F05, Flow layer, **`Info`, not `Warning`**: the same source channel appears at
  more than one position in a flow. Downgraded deliberately — because position is
  significant, `[3, 1, 3]` is a legitimate replication (one mono source landing on two
  receiver positions), not an error. Flagging it as a Warning would contradict the
  order-preservation guarantee in D-1. The message asks whether the repeat is intended;
  it does not assert a fault.
- `IndexElement::Auto` in a stream source gets an `Info` diagnostic rather than being
  dropped in silence — consistent with D-2's preference for loud over silent.
- `Warning`, not `Error`, for F04/F05: both describe an inconsistent-but-loadable file,
  and neither should block a save.

### Phase 5 — verification and docs
- Unit tests per phase; every behavioural test mutation-checked (revert the change,
  confirm the test fails) per the standing convention from the #32/#33/#34 sweep.
- Round-trip test: emit → format → parse → load returns the identical ordered selection,
  including a non-monotonic case and a range-in-source case. **Assert `port_name` as well
  as `source_channels`** — emit resolves `interface_id` through `directional_port_name`
  while load returns the raw `source.port`, and those can differ on io splits (`_In`/
  `_Out`). A selection-only assertion would let a port-name regression ride through green.
- **Rebuild WASM before the e2e check** — stale builds silently ignore new DTO fields and
  the round-trip appears to work when it does not. Rebuild whichever of `pkg-node` /
  `pkg-web` / `pkg-bundler` the e2e harness actually loads; naming only one risks
  rebuilding the wrong artifact and testing the stale one.
- Update `SPEC.md`, the language reference, and the PatchLang skill with the selection
  form. Record D025.

## Commit shape

The F02 fix lands as its **own commit**, separate from the selection work. It is a
latent type-mismatch bug that predates #37 and merely shares subject matter; keeping it
separate means it can be reverted independently if it ever proves noisier in the field
than our fixtures suggest.

## Follow-ups to file

1. Bounds-check selection indices against the source interface's channel count.
2. `check` JSON emits a duplicated `"type": "Stream"` key (serialization defect).
3. `canvas_emit` drops streams on duplicate sanitized names.
