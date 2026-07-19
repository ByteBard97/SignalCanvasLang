# Plan — Canvas DTO plurality: ranged bridges + multi-input buses

**Closes:** SignalCanvasLang #29, SignalCanvasLang #30. Unblocks FrontendV1 #185.
**Repos:** `SignalCanvasLang` (Rust + WASM), `SignalCanvasFrontend` (TS).

---

## 1. One root cause, three symptoms

The canvas DTO layer between the PatchLang AST and the frontend is **scalar where both
ends are plural.** The AST has `Vec<PortRef>` and `IndexSpec`; the frontend has
`InternalBusInput[]` and `RouteRule{fromStart,fromEnd,…}`. Only the DTO in the middle
flattens — data is destroyed crossing a boundary that exists purely for transport.

| # | Symptom | Site |
|---|---|---|
| 29 | Bus fed by 2 ports collapses to the first; all channels re-stamped onto it | `canvas_load.rs:283-287`, `canvas_emit/routes.rs:114-124` |
| 30a | `bridge X[1..32] -> Y[65..96]` loads as a single `X[1] -> Y[65]` rule | `canvas_load.rs:235-236` |
| 30b | Frontend guesses the lost width back from port sizes — sometimes **inventing** channels | `buildRouteRulesFromWasm.ts:133-136` |

Common primitive: `extract_single_index` (`canvas_load.rs:482`) maps
`Range { start, .. } => Some(start)`, discarding `end`.

### Verified evidence
Round-trip of `stdlib/audio/allen-heath.patch` + `instance SB1 is GX4816 {}`:
```
bridge Mic_In[1..48]  -> GX_Out[1..48]   →   bridge Mic_In    -> GX_Out
bridge DX_1_In[1..32] -> GX_Out[65..96]  →   bridge DX_1_In   -> GX_Out[65]
bridge GX_In[65..96]  -> DX_1_Out[1..32] →   bridge GX_In[65] -> DX_1_Out
```
Applying `buildRouteRulesFromWasm`'s inference to all 25 ranged stdlib template bridges:
**24 reconstruct correctly, 1 does not** — `DX168: DX_A_In[17..24] -> DX_Cascade_Out[1..8]`
becomes `[17..32] -> [1..16]`, fabricating 8 mappings never in the source.

---

## 2. Why together

**#29 depends on #30's primitive.** Real files contain ranged bus inputs
(`input: Mix_Bus[1..24]`, `Mix_Bus[1..16]`, `Aux_Out[1..4]`). Building `input_groups` on
`extract_single_index` would bake the range bug into #29's brand-new code path.

Also: same function (`load_from_patch`), adjacent regions — sequential work conflicts. And
the expensive part of either plan is the deprecation cycle (dual-populate → ship → wait for
updater → delete). Together that's **one cycle, not two.**

### The two shapes are different — do not unify
| | #29 buses | #30 bridges |
|---|---|---|
| Needs | *grouping* — `Vec<{port, channels}>` | *widening* — `from_start/from_end/to_start/to_end` |
| Because | a bus sums an arbitrary **set** of channels | a bridge maps a contiguous **span** |
| Helper | `expand_index` (returns `Vec`) | `index_span` (returns `(start,end)`) |

**This distinction is load-bearing.** Buses may be discontiguous (`Fader[1,3,5]` is legal
and meaningful); bridges are contiguous by definition. Using the wrong helper on either
side reintroduces the bug.

---

## 3. THE INVARIANT

Everything below depends on one rule. **Absent index ⟺ full port width.**

- **Load:** absent index → **the port's full declared span**. `[a..b]` → `(a, b)`. `[n]` → `(n, n)`.
- **Emit:** omit the index **iff the span equals the port's full declared span**; otherwise
  always write `[start..end]`.

> ⚠️ **"Full declared span" is NOT `(1, width)`.** Ports may be declared with a non-1 start —
> there are 14 in the shipped stdlib, e.g. `Port_B_1_In[17..24]` in `optocore.patch`. For that
> port an absent index means `(17, 24)`, not `(1, 8)`. Read the declared `start`/`end` off the
> `PortDef` directly; never reconstruct it from a width. (Not reachable today — `optocore.patch`
> declares no bridges — but it is a live trap the moment anyone adds one.)

Worked against the verified data — `DX_1_In[1..32] -> GX_Out[65..96]`:
load → `(1,32)`,`(65,96)`; emit → source span 32 == `width(DX_1_In)`=32, so omit; target
span 32 ≠ `width(GX_Out)`=128, so write. Result: **`bridge DX_1_In -> GX_Out[65..96]`** —
semantically identical to source, and the malformed `GX_Out[65]` is gone.

This also makes `emit → load → emit` byte-idempotent (canonical form in, canonical form
out), which is what `full_idempotency_deterministic` and `prop_emit_is_idempotent` assert.

`build_bridges` **already has the widths it needs** — its signature takes
`ifaces: &[InterfaceEmitInput]` (currently `_ifaces`, unused) and `InterfaceEmitInput`
carries `channel_count`. Wire it up.

If a port is genuinely unresolvable, fall back to `(1, 1)` and **log it** — never swallow.

> ⚠️ **Expect to regenerate checked-in golden `.patch` fixtures.** Bridges that start at
> channel 1 but cover only part of a port will now emit an explicit span where they
> previously emitted none. That is the fix working, not a regression.

---

## 4. Scope decisions

| Question | Decision | Why |
|---|---|---|
| Fan out bridges to one rule per channel? | **No — add range fields** | `RouteRule` is natively ranged on the frontend (`userDevice.ts:15`), `IndexSpec` in the AST. Fan-out would explode 1 rule into N and force the frontend to re-collapse. GX4816: 6 → 192. |
| Widen `instance_routes` too? | **No — out of scope** | **Zero ranged `route` statements exist** across every `.patch` in the monorepo (all 98 ranged refs are `bridge`). And the frontend's `InternalRoute` is scalar (`internalRouting.ts:31`), so widening the Rust DTO would be inert without a matching frontend change. Leave `instance_routes` exactly as-is. |
| Multi-element index (`[1,3,5]`) in a bridge? | **Skip + log** | Not contiguous, so no honest span exists. Never take `min..max` — that invents channels. **Does not occur anywhere in the monorepo today**, so this is a guard, not a migration. Buses never hit it (they expand to a set). |
| Keep `buildRouteRulesFromWasm`'s inference? | **Delete it** | Width must come from the source, not be guessed. It is the cause of 30b. |
| Fix bus output destination flatten (`canvas_load.rs:305-310`)? | **Yes**, no new struct | Emit one `BusNamedOutput` per `(instance, port)` sharing `out.label`. TS emitter already produces that shape; TS loader already merges by name. **No frontend change for this half.** |
| Cross-device bus *inputs*? | **Yes**, but see below | Mirrors `BusOutputEmitInput`; removes the `instance: None` hardcode at `routes.rs:118`. **This is the only slice with no current consumer** (#185 isn't built) — the deferrable one if the PR needs shrinking. |
| Touch `add_bus` / `Builder`? | **No** | Already takes a full `ast::BusEntry` — multi-input capable today. |
| Touch `compat_types.rs`? | **No** | Mirrors the AST directly, already correct. |
| Touch connections / `format_port_ref`? | **No** | `build_channel_mappings_from_indices` already reads full `IndexSpec` (319 of ~420 ranged refs). `format_port_ref` is a display string. |

---

## 5. Target shapes

```rust
// canvas_output.rs — RouteRuleOutput gains a span. Keep from_channel/to_channel in Phase 1.
pub struct RouteRuleOutput {
    pub from_port: String,
    pub from_start: u32,
    pub from_end: u32,
    #[serde(skip_serializing_if = "Option::is_none")] #[ts(optional)]
    pub from_instance: Option<String>,
    pub to_port: String,
    pub to_start: u32,
    pub to_end: u32,
    #[serde(skip_serializing_if = "Option::is_none")] #[ts(optional)]
    pub to_instance: Option<String>,
}

// canvas_input.rs — emit side needs the same widening.
pub struct RouteRuleEmitInput {
    pub from_interface: String,
    pub from_start: u32,
    pub from_end: u32,
    #[serde(default)] #[ts(optional)] pub from_instance: Option<String>,
    pub to_interface: String,
    pub to_start: u32,
    pub to_end: u32,
    #[serde(default)] #[ts(optional)] pub to_instance: Option<String>,
}

// canvas_input.rs — bus input groups. Mirrors BusOutputEmitInput exactly.
pub struct BusInputGroupEmitInput {
    #[serde(default)] #[ts(optional)] pub instance: Option<String>,
    pub interface: String,
    pub channels: Vec<u32>,          // a SET, not a span
}

// canvas_output.rs — mirrors BusNamedOutput's naming exactly.
pub struct BusInputGroup {
    #[serde(skip_serializing_if = "Option::is_none")] #[ts(optional)]
    pub input_instance: Option<String>,
    pub input_port: String,
    pub input_channels: Vec<u32>,    // a SET, not a span
}
```

`#[ts(optional)]` bare is correct — 23 existing uses on ts-rs v10; the exact
`skip_serializing_if` + `ts(optional)` pairing is shipped on `BusNamedOutput.output_instance`;
the crate compiles and tests pass with it. (Raised twice in review as suspect; verified
false both times.)

**Frontend null-handling:** `input_instance` is omitted when `None`. Mirror the existing
loader idiom for the output side — `if (out.output_instance)` truthy check — rather than
distinguishing `undefined` from `null`.

---

## 6. Execution — checkpoints

Six checkpoints. **Rust suite + the acceptance test must be green at each one** before
moving on. These are the review boundaries.

### CP0 — acceptance test first (Lang)
Write the regression test #30 needs, before touching any source. It is the check-signal
for every later checkpoint.

`crates/patchlang/tests/bridge_span_roundtrip.rs`:
- Load `stdlib/audio/allen-heath.patch` + `instance SB1 is GX4816 {}` and
  `instance DX1 is DX168 {}`.
- Assert **semantic identity** of every bridge span: parse source bridges, compare
  `(from_port, from_start, from_end, to_port, to_start, to_end)` against loaded rules.
- Assert specifically that DX168's `DX_A_In[17..24] -> DX_Cascade_Out[1..8]` is
  `(17,24)→(1,8)` — **not** `(17,32)→(1,16)`. This is 30b's fabrication case.
- Assert `emit → load → emit` is **byte-identical** (idempotency).

It fails at CP0. That's the point.

### CP1 — the range primitive (Lang)
Add to `canvas_load.rs`:
```rust
/// Contiguous span of an index spec, per THE INVARIANT.
/// [a..b] → Some((a,b)).  [n] → Some((n,n)).  None/Auto → None (caller applies port width).
/// Multi-element ([1,3,5]) → None — no honest span exists; caller skips and logs.
fn index_span(index: &Option<IndexSpec>) -> Option<(u32, u32)>
```
Leave `extract_single_index` in place — `format_port_ref` still uses it for connection
display strings. Unit-test `index_span` directly: single, range, absent, `Auto`,
multi-element.

**Check:** helper tested, no behaviour change yet, suite green.

### CP2 — DTO structs (Lang)
Add all four shapes from §5. On `BusEmitInput` add
`#[serde(default)] input_groups: Vec<BusInputGroupEmitInput>`; on `BusOutput` add
`input_groups: Vec<BusInputGroup>`. Keep every legacy field, doc-comment them deprecated.

**Check:** compiles, bindings regenerate, suite green (nothing reads the new fields yet).

### CP3 — load path (Lang)
1. **Bridges** (`canvas_load.rs:233-246`): use `index_span` + port width per THE INVARIANT.
   Populate `from_start`/`from_end`/`to_start`/`to_end`; set legacy
   `from_channel`/`to_channel` to the span start (bit-identical to today).
   Multi-element → skip + log.
2. **Bus inputs** (`:278-325`): **filter with `is_valid_port` FIRST, then group** survivors
   by `(instance, port)`, first-seen order. Grouping before filtering would promote
   `Unknown`/`Device` sentinels from old saves into real groups. Channels per group come
   from **`expand_index`** (set semantics — handles ranges *and* multi-element), **not**
   `index_span`. Populate legacy `input_port`/`input_channels` exactly as today.
3. **Bus named outputs** (`:305-310`): stop collapsing to `real_dests.first()`. Emit one
   `BusNamedOutput` per distinct `(instance, port)`, all sharing `out.label`.
4. Leave `instance_routes` untouched (§4).

**Check:** CP0's load-side span assertions pass. Suite green. Run `fixture_tests.rs` —
it exercises real project fixtures and is where latent garbage surfaces.

### CP4 — emit path (Lang)
1. `canvas_emit/structures.rs::build_bridges` (`:46-85`): wire up `ifaces` (drop the
   `_` prefix), emit `IndexElement::Range { start, end }` when `end > start`, and apply
   THE INVARIANT's omit rule — omit the index iff the span covers the full port width.
   This replaces the current "omit when channel == 1" rule at `:59-66`.
2. `canvas_emit/routes.rs::build_instance_buses` (`:105-185`): when `input_groups` is
   non-empty, use it — one `PortRef` per channel per group, with
   `instance: group.instance.clone()` (removes the `instance: None` hardcode at `:118`).
   Otherwise fall through to the legacy path verbatim.

**Check:** CP0 fully green including byte-idempotency. **Regenerate golden fixtures** and
review the diff by eye — every change should be a partial-width bridge gaining an explicit
span. Anything else is a bug.

### CP5 — frontend (two independent commits, after `./scripts/update-wasm.sh`)

**5a — bridge spans (#30)**
- `emitterBuilder.ts:352` — currently `from_channel: r.fromStart`, discarding `fromEnd`.
  Send `from_start`/`from_end`/`to_start`/`to_end`.
- `buildRouteRulesFromWasm.ts:123-146` — **delete the `channelCount` inference**; read the
  span straight through. This is the DX168 fix.
- Regression test: load DX168, assert the cascade rule is `[17..24] -> [1..8]`.

**5b — bus input groups (#29)**
- `types/internalRouting.ts` — add `fromDeviceInstanceId?: string` to `InternalBusInput`,
  matching the convention documented on `InternalRoute:32-36`.
- `emitterAssembly.ts:123-126` — delete the `bus.inputs?.[0]` collapse. Build a `byPort`
  map keyed on `${fromDeviceInstanceId ?? ''}.${resolvedPortName}`, mirroring the existing
  output-side grouping at `:155-175` (same Set-dedup, same sort). Emit `input_groups`.
  Reuse `resolvePortName` / `crossDevicePortResolver` unchanged.
- `loadFromPatchLang.ts:402-465` — read `bus.input_groups`; per group resolve the iface via
  the existing `ifaceByPortName.get(p) ?? get(p + '_In')` fallback; set
  `fromDeviceInstanceId` when `input_instance` is truthy. Keep a fallback to the legacy flat
  fields when `input_groups` is empty.
- **Audit consumers before editing**: `BusManagerModal.vue`, `BusManagerTable.vue`,
  `busDestMatchesOutput` / `resolveBusDestTarget` may assume one input port. These are on
  the god-file list — **extract before adding, do not grow them.**
- **Interleaved-input test:** author a bus as `Fader[5]` / `Mix_L[1]` / `Fader[6]`, round-trip,
  and compare the `(port, channel)` **multiset order-independently**. Grouping reorders these
  and that is correct — buses sum, so order is not semantic. Assert the invariance rather
  than asserting byte identity (which would fail) or skipping the case.

**Check:** frontend suite green. Manual: bus fed from two different ports survives
save/reload; GX4816 and DX168 route-rule spans match source.

### CP6 — delete legacy fields (Lang) — **OPTIONAL / DEFERRABLE**
CP0-CP5 fully fix all three symptoms with no rollout risk. All risk is here. Keeping
deprecated fields is ugly but free. **Recommend deferring.**

If done: only after the updater has rolled CP5 out — old clients on new WASM would silently
lose bus inputs and bridge spans. Find the sites with (**not line numbers, they drift**):
```
grep -rnE 'pub (input_port|input_channels|input_interface|from_channel|to_channel)' crates --include "*.rs"
```

---

## 7. Risks

| Risk | Mitigation |
|---|---|
| Old app + new WASM = silent loss | CP6 gated on updater rollout; CP0-CP5 additive |
| Golden fixtures churn at CP4 | Expected — review the diff by eye; every change should be a partial-width bridge gaining a span |
| `fixture_tests.rs` surfaces latent garbage in real fixtures | Run it at CP3, early | 
| Bus Manager UI assumes one input port | Audit step in 5b; extract from god files first |
| `loadFromPatchLang.ts` is 1069L and grows | New grouping goes in `emitterAssembly.ts` or a new `busInputGroups.ts`, never inline in the loader |
| `sanitize_id` collides two ports into one group | **Known limitation, not fixed here.** Pre-existing; not made worse. |
| Unresolvable port width at emit | Fall back to `(1,1)` and **log** — never swallow |

## 8. Out of scope
- **Top-level `bridge` statements are dropped entirely on canvas load.** `load_from_patch`
  has no `Statement::Bridge` arm; the match falls to `_ => {}` at `canvas_load.rs:107`, and
  `CanvasLoadOutput` has no bridges field. So `bridge A.X[1..32] -> B.Y[1..32]` never reaches
  the canvas and is never re-emitted. Possibly intentional (cross-device flow may be modelled
  purely via `connect`) but silent either way. **Documented in the "separate finding" section
  of #30 — needs its own ticket.**
- `instance_routes` widening — see §4.
- `add_bus` / `Builder` — already capable.
- FrontendV1 #185 — unblocked by this, not part of it.
