# Issue #31 — Insert send/return PatchLang round-trip

**Repo:** SignalCanvasLang · **Issue:** ByteBard97/SignalCanvasLang#31 · **Author:** @reidwwall
**Scope:** Rust/Lang side only. Reid does the frontend cutover once these round-trip.

## Problem

Channel and bus **inserts** (breaking a channel/bus out to an external send/return path)
are sidecar-only in the frontend because the Lang↔canvas DTO boundary is lossy:

- `ChannelLabelOutput` is a fixed struct (`channel_index, label, phantom, propagated,
  source_type, capsule, rf_band`). `PatchBuilder::set_label` accepts an arbitrary
  `HashMap<String,String>` property bag, and those properties **do** survive `format()`
  as `.patch` text — but `canvas_load.rs:134` rebuilds the fixed struct from the parsed
  props and drops every other key. So insert data survives emit and dies on reload.
- `BusOutput` has no extension point at all, and `BusEntry` (the AST) has no property
  bag — buses need an actual grammar change.

Same family as FrontendV1#202 (commit `8b0a8d5`, connect properties): the canvas DTO is
lossy where the AST is not.

## Data shape (from frontend `feature/august-1-features`)

```ts
// src/types/internalRouting.ts
export interface InsertEndpoint { interfaceId: string; channel: number }
```

`insertSend` / `insertReturn` are each an **ordered list** of endpoints — `[L]` mono,
`[L, R]` stereo. Endpoints are independent: no adjacency and no width constraint, so
`send: MADI 3 & 10, return: MADI 4 & 8` must be representable. Present on both
`ChannelLabel` (`canvasScene.ts:111`) and `InternalBus` (`internalRouting.ts:67`).
`interfaceId` is a canvas-session id; the emitter already resolves interface ids to
**port names** (`directional_port_name`), so the Lang-side representation is a port ref.

## Approach

Two different mechanisms, because the two carriers differ:

### A. Channel labels — no grammar change needed

Label properties are already arbitrary `key: "value"` pairs that parse and re-emit fine.
The only fix needed is at the DTO. Do **both**, exactly as `8b0a8d5` did for connects:

1. **Typed fields** `insert_send: Vec<InsertEndpointOutput>` / `insert_return` on
   `ChannelLabelOutput` — gives the frontend a parsed shape, satisfies the ticket.
2. **Verbatim `properties: BTreeMap<String,String>`** passthrough on `ChannelLabelOutput`
   — so no *future* label key is silently dropped. `BTreeMap`, not `HashMap`: iteration
   order must be stable or emit→load→emit reorders keys and breaks idempotency (that is
   the documented reason in `8b0a8d5`). Side benefit, in-spirit but beyond the ticket:
   this is the same mechanism `stand`/`gain` need. We ship the mechanism only; the
   frontend cutover for those is Reid's call.

Wire encoding — a string-valued property holding a port-ref list:

```patchlang
label Mic_In[1]: "Kick" {
  insert_send: "Ext_Out[3], Ext_Out[10]"
  insert_return: "Ext_In[4], Ext_In[8]"
}
```

**Why a string and not a native port ref.** A `KvValue::PortRef` variant *does* exist
(`ast.rs:251`), and `parse_key_value_full` (`body_parser.rs:357`) already accepts
`Some(Token::Identifier(_)) => KvValue::PortRef(...)`. But it parses exactly **one**
port ref with no comma continuation, and an insert leg list is inherently multi-valued.
Extending kv parsing to comma-lists would change property syntax globally. So: string.
`mapping_text` is the existing precedent for structured-text-in-a-string here.

**Hazard this creates (kimi, verified).** Because `KvValue::PortRef` is real, a user or
LLM writing the *unquoted* form `insert_send: Ext_Out[3]` gets a successful parse into
a `PortRef` — and then `kv_map` (`canvas_load.rs:515-525`) hits its `else { None }` arm
and **silently drops it**. Right syntax, plausible-looking, data gone. Two consequences:

- **Fix `kv_map` to stringify `KvValue::PortRef`** instead of dropping it, exactly as
  `graph/mod.rs:491-504`'s `kv_to_string_map` already does. This is a pre-existing
  silent-drop bug affecting *every* label property, squarely in this ticket's theme, and
  it costs three lines. In scope.
- Document the quoted form as canonical in SPEC.md and the skill, so generated patches
  use it.

**Explicitly rejected:** a grammar-native `insert Mic_In[1] send: ... return: ...`
statement. The ticket proposes threading through `set_label`'s property parser; that's
the face-value reading and it's far cheaper. Not open for relitigation in review.

### B. Buses — grammar change required

`BusEntry` has no properties. Add native port-ref-list entries to the `bus { }` block:

```patchlang
bus Main_LR {
  input: Fader[1]
  insert_send: Ext_Out[3], Ext_Out[10]
  insert_return: Ext_In[4], Ext_In[8]
  output "Mix": Matrix_Out[1]
}
```

Parsed as plain `Identifier` in the existing bus-body match — no new lexer token.

**Compat note for Reid:** `parse_bus_entry`'s body loop ends in
`_ => { self.advance(); continue; }`, so on Lang ≤0.3.1 an `insert_send:` line inside
`bus { }` is *silently token-skipped* — no error, data gone. The frontend cutover
therefore needs a minimum-Lang-version guard. Call this out in the issue comment.

## Endpoint resolution — settled by socratic debate, see D023

The frontend sends `{ interfaceId, channel }`; PatchLang needs a template port name.
Two conventions already existed side by side in `canvas_emit/`, and the first draft of
this plan silently picked the wrong one.

**Decision: resolve in Rust** via the existing `resolve_route_endpoint`
(`routes.rs:87-89`), with `PortSide::Output` for send legs and `PortSide::Input` for
return legs — the same path channel labels and instance routes already use.

The deciding fact is `should_split_io` (`ports.rs:34-43`): one io/asymmetric interface
expands into **two** template ports (`{base}_In` / `{base}_Out`) for every non-ring/bus
protocol, and MADI is not on the ring/bus list. The ticket's own canonical example sends
and returns on the *same* MADI interface, so the legs must emit as `MADI_Out[3]` and
`MADI_In[4]`. Only the side tells them apart, and the side is knowable solely from which
list a leg sits in — the emitter has that, TypeScript does not. Sanitizing the interface
id alone (the first draft) emits a bare `MADI[3]` the template never declares, which the
loader then rejects and silently drops. `insert_legs_resolve_to_the_directional_port_for_a_split_io_interface`
pins this; it fails against the sanitize-only version.

The `unwrap_or_else(sanitize_id)` fallback inside `resolve_route_endpoint` is retained
and load-bearing: already-resolved port names and slot-qualified `__` compounds match no
interface and must pass through unchanged.

Consequence for the touch list: `build_instance_buses` gains `installed_cards`,
`manufacturer_cards` and `all_instances` — all already in `build_instance_decl`'s scope,
so it is a call-site widening, and it retires the `let _ = ifaces; // used for future
label resolution` TODO at `routes.rs:220`.

## Shared type

```rust
/// One leg of an insert send/return. Same-device by convention (the frontend's
/// InsertEndpoint points at an interface on the SAME device); `instance` exists only
/// so a qualified ref round-trips if one ever appears. Nothing is built on it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct InsertEndpointOutput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub instance: Option<String>,
    pub port: String,
    pub channel: u32,
}
```

`#[ts(export)]` is required or the generated TS type dangles.

## Traps (each has a test below)

1. **Insert legs are an ordered flat list — never group or union them.** The bus code
   this sits next to deliberately groups by `(instance, port)` and unions channels,
   because bus inputs are set-like. Insert legs are not: `[L, R]` is order-significant
   and the ticket's own example repeats a port. Copying the grouping pattern makes
   `[A1, B1, A2]` round-trip as `[A1, A2, B1]` and silently swaps stereo. One `Vec`
   entry per endpoint, first-seen order, no dedup, no union.
2. **Constrain each endpoint index to `IndexElement::Single`.** Running it through
   `expand_index` turns `[1..2]` into two endpoints. Defined behaviour: a range or
   multi-element index makes the **whole leg malformed** → skipped by the typed parser,
   text preserved per trap 8. Never expanded, never clamped, never a hard parse error.
3. **`#[serde(default)]` on every new `*EmitInput` field.** Without it serde rejects the
   *whole payload* with "missing field" — `canvas_input.rs` documents this exact failure
   for `from_start`/`from_end`. A frontend built before this change sends payloads with
   no insert fields, and new Rust must accept them.
4. **Plain `Vec` (empty = absent), not `Option<Vec>`,** for the list fields — avoids the
   `skip_serializing_if` + `#[ts(optional)]` trap that the existing
   `skip_serialized_option_fields_are_ts_optional` test exists to police (ts-rs ignores
   `skip_serializing_if`).
5. **Bag holds the leftovers only — no precedence rule.** On load, `properties` gets
   every label key that has *no* dedicated field: `insert_send`/`insert_return` (and
   `phantom`, `propagated`, `source_type`, `capsule`, `rf_band`) are **excluded**.
   Rationale, to be stated in a code comment: this deliberately diverges from `8b0a8d5`,
   which kept connect's dedicated fields duplicated in the bag. Connect's dedicated
   fields are a *lossless* re-read of the same string. `insert_send`'s typed field is a
   *lossy parse* — a malformed leg is skipped by the parser. If both existed and typed
   won on re-emit, a malformed-but-intact source string would be blanked by the fix
   itself. With the keys excluded there is no collision and `properties` honestly means
   "the leftovers." The one exception — a malformed insert string, which stays in the
   bag precisely so it is not lost — is trap 8.
8. **Malformed legs must not be silently blanked — all-or-nothing parse.**
   `parse_insert_list` returns `Option<Vec<InsertEndpointOutput>>`: `Some` only when
   **every** comma-separated leg parsed cleanly, `None` if any leg is malformed (bad
   identifier, range index per trap 2, missing channel). Combined with trap 5 this gives
   one unambiguous rule and needs no extra `_raw` field on the DTO:

   - **Parsed cleanly** → typed fields populated, key **excluded** from the bag.
   - **Any leg malformed** → typed fields empty, key **stays in the bag** verbatim, and
     is re-emitted byte-for-byte.

   Data is carried by exactly one of the two mechanisms, never both, never neither.
   Emit therefore needs no precedence logic at all: write the typed list if non-empty,
   then merge the bag; by construction the keys cannot collide.
6. **Pass endpoints through verbatim — do not run `is_valid_port` on them.** That filter
   drops "garbage sentinel" ports; this ticket is about *not* losing data.
7. **Construction-site churn.** New `ChannelLabelOutput` fields break ~10 struct
   literals: `builder_tests/canvas_test_helpers.rs:43`, `canvas_load_helpers.rs:30`,
   `builder_tests/canvas_roundtrip_tests.rs:175/184/270`,
   `tests/bridge_emit_invariant.rs:45`, `tests/bridge_span_roundtrip.rs:57/142`,
   `tests/canvas_roundtrip_tests.rs:48/128/217` — plus the easy-to-miss
   `canvas_load.rs:147` `resize_with(...)` sparse-channel filler. Add
   `#[derive(Default)]` and use `..Default::default()` to cut future churn.

## Out of scope — file as follow-ups, do not absorb

- **`patchlang-python/src/lib.rs:143-151`** — `set_label` hard-codes `HashMap::new()`,
  so the Python binding forwards **no label properties at all**, ever. A genuine second
  lossy boundary, but it's a public Python API signature change needing its own tests,
  and inserts don't flow through Python today. File it.
- Frontend cutover (`emitterSidecar.ts`, `patchLangSidecarLabels.ts`, `busToRust` in
  `wasmAstConverters.ts`) — Reid's, per the ticket.

Verified **non**-issues (kimi swept these; no action needed): no DRC pass whitelists
label property keys, so `insert_send` produces no spurious diagnostics; the LSP has no
private grammar copy and delegates to `patchlang::parse`; there are no golden/snapshot
files to update, all format tests are behavioural; the graph compiler never consumes
`BusEntry`; serde has no `deny_unknown_fields` anywhere, so *extra* keys are already
tolerated — only *missing* ones break (trap 3).

## No DRC

The ticket states endpoints are independent with no adjacency/width constraint. A
"send count ≠ return count" warning would contradict the stated spec. Skipped
deliberately.

## Touch list

**Shared**
- `crates/patchlang/src/builder/canvas_output.rs` — `InsertEndpointOutput`; two fields +
  `properties` bag on `ChannelLabelOutput`; two fields on `BusOutput`; `#[derive(Default)]`.
- `crates/patchlang/src/builder/canvas_input.rs` — insert fields on
  `ChannelLabelEmitInput` and `BusEmitInput`, all `#[serde(default)]`.
- `crates/patchlang/bindings/*.ts` — regenerate and commit, incl. the new endpoint type.

**Labels**
- `canvas_load.rs` (~134) — parse `insert_send`/`insert_return` props into typed
  endpoints; populate the verbatim `properties` bag; update the `resize_with` filler.
- `canvas_emit/mod.rs` (~250) — encode typed endpoints back into props, then merge the
  bag per the precedence rule.
- New `insert_ref.rs` (or a small module next to the parser) — `parse_insert_list(&str)
  -> Vec<InsertEndpointOutput>` and `format_insert_list(&[InsertEndpointOutput]) -> String`,
  one place, used by both directions. Tolerant on parse (skip malformed legs), canonical
  on format.

**Buses**
- `ast.rs` `BusEntry` — `insert_send: Vec<PortRef>`, `insert_return: Vec<PortRef>`,
  **both `#[serde(default)]`**. This is not optional: `patchlang-wasm/src/lib.rs:315`
  deserializes `ast::BusEntry` *directly* from frontend JSON (`add_bus(handle, instance,
  bus_json)`), not via `BusEmitInput`. Without the default, serde's "missing field"
  breaks every existing frontend `add_bus` call the moment this ships. Also update that
  function's doc-comment, which documents the JSON shape verbatim — **and
  `update_bus` at `patchlang-wasm/src/lib.rs:639-649`**, which deserializes `BusEntry`
  the same direct way and carries the same doc-comment.
- `canvas_load.rs:515` `kv_map` — stringify `KvValue::PortRef` rather than dropping it
  (see the hazard note above).
- `body_parser.rs` `parse_bus_entry` — two new identifier arms, comma-separated port refs.
- `formatter_emit.rs` `emit_bus_entry` — emit after `input:`, before `output`.
- `compat.rs:354` `convert_bus_entry` → `TsBusDecl` — second lossy boundary, not in the
  ticket's list.
- `builder/routing.rs` `add_bus` — carry the new fields.
- `canvas_emit/routes.rs` `build_instance_buses` (~204) — build them.
- `canvas_load.rs` `internal_buses` (~295) — read them, **outside** the grouping logic.

**Docs** (load-bearing — the ticket's "LLM-generatable" goal depends on them)
- `SPEC.md` — bus `insert_send`/`insert_return` grammar; label property encoding.
- `SCHEMA.md` — new DTO fields.
- `signalcanvas-skills` `SKILL.md` / patchlang skill — same, so generated patches use it.

**Release**
- Version bump 0.3.1 → 0.3.2 + WASM release (confirm with Geoff before triggering).

## Test matrix

Every row for **both** labels and buses:

| # | Case | Asserts |
|---|------|---------|
| 1 | Mono `[L]` | single endpoint survives load |
| 2 | Stereo `[L, R]` | **order** preserved |
| 3 | Scattered — send `MADI[3], MADI[10]`, return `MADI[4], MADI[8]` | no collapse to a range |
| 4 | Interleaved ports `[A1, B1, A2]` | no grouping/union — catches trap 1 |
| 5 | Range index `[1..2]` in an endpoint | leg skipped from typed `Vec`, source text re-emitted verbatim — never 2 endpoints, never clamped |
| 5b | Malformed leg `insert_send: "Ext_Out[bogus]"` | round-trips **unchanged**, not blanked — catches trap 8 |
| 6 | emit→load→emit on a fixture with both | byte-identical (idempotency) |
| 7 | EmitInput payload with insert fields **absent** | deserializes — catches trap 3 |
| 7b | Legacy `add_bus` JSON (`ast::BusEntry`, no insert keys) | deserializes — catches the `patchlang-wasm:315` direct-deserialize break |
| 8 | Unknown label property (`stand`, `gain`) | survives load via the verbatim bag |
| 9 | **Unquoted** `insert_send: Ext_Out[3]` (parses as `KvValue::PortRef`) | survives `kv_map` instead of being silently dropped |

Plus a `.patch` fixture under `tests/` exercising channel + bus inserts together, and the
existing suite (757+ tests) staying green.

## Build order

1. `InsertEndpointOutput` + shared parse/format helper + its unit tests (TDD).
2. Label path: output struct → load → emit → tests 1–5, 7, 8.
3. Bus path: AST → parser → formatter → compat → builder → emit → load → tests 1–6.
4. Fixture + idempotency test 6 end-to-end.
5. Bindings regen, docs, version bump.
6. `cargo test` + `cargo clippy` green (Lint CI has been green since `a433675` — keep it).
