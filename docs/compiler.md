---
layout: default
title: Compiler
permalink: /compiler/
---

# Compiler

## Design Decisions

### `bridge` vs `route`: Semantic Contract

These two keywords are semantically distinct and must remain so. The distinction drives Signal Trace annotation, DRC reliability, and Probe v2 push correctness.

| Keyword | Scope | Meaning | Probe v2 behavior |
|---|---|---|---|
| `bridge` (in template) | Template body | Path guaranteed by device design. Exists in every unit regardless of software config. DRC treats as invariant. | Do NOT push (hardware-fixed) |
| `bridge` (top-level) | File root | System designer's DRC assertion for signal tracing across instances. | Read-only, not pushed |
| `route` (in instance) | Instance body | Operator-configured routing state for this specific device. May change between shows. | Push via Probe v2 (SCP, Ember+, AES70, Q-SYS) |

**The rule for template authors:**
- Use `bridge` only for paths the manufacturer hardwired into every unit (mic preamps → Dante output on a stagebox, ADC → DSP input on a fixed-path converter).
- Do NOT use `bridge` for operator-configurable routing. A CL5 console template has no `bridge` declarations; all its internal routing is software-defined and belongs in the instance as `route`.
- A fully flexible device (SDI router, DSP matrix, mixing console) may have zero `bridge` declarations in its template. That is correct.

**Why this matters for Probe v2:** When pushing configuration to live hardware, Probe must know what to touch. `route` = push. `bridge` = do not touch. If `bridge` were used for operator-configurable paths (as the rejected "physical/logical axis" would allow), Probe would have no way to distinguish fixed hardware behavior from operator intent, making correct push implementation impossible.

**Why this matters for Signal Trace:** Both `bridge` and `route` are traversed by the signal tracer. Under this model, the tracer can annotate each hop: `bridge` hops are "guaranteed by hardware design"; `route` hops are "depends on current operator configuration." This is meaningful information when an engineer is tracing a fault.

---

### IO Direction Model

See [Port Direction Model](language-reference.md#port-direction-model) in the Language Reference for the canonical table and rules.

Summary: channel-based protocols (`in` + `out`), ring/bus protocols and management ports (`io`). WordClock always uses split `in`/`out`, never `io`.

**Backward compatibility:** The parser accepts `io` for any protocol (legacy files). The emitter must produce split `in`/`out` for channel protocols.

### Cards Are Templates

No `card` keyword. Cards are templates with `meta { kind: "card" }`.

```
template MY16_AUD {
  meta {
    manufacturer: "Yamaha"
    model: "MY16-AUD"
    kind: "card"
    fits: "MY_Format"
  }
  ports {
    Dante_In[1..16]: in [Dante]
    Dante_Out[1..16]: out [Dante]
  }
}
```

### Inverted Slot Compatibility

Cards declare what they fit. Slots declare the bay shape only.

```
# Slot on a device — declares bay format
slot MY_Slot[1..3]: MY_Format

# Card template — declares what bays it fits
template MY16_AUD {
  meta { fits: "MY_Format" }
  ...
}
```

Adding a new card type never requires editing existing templates. Multiple format compatibility: `fits: "MY_Format, HDX_Format"`.

### Deterministic Port IDs

IDs use `::` as separator, derived from template and port names:

```
pl::CL5::Dante_In              # Scalar port (no index)
pl::CL5::Dante_In_1            # Ranged port, channel 1
pl::CL5::Dante_In_72           # Ranged port, channel 72
rule::CL5::Mic_In::Dante_Out   # Route ID (4 segments)
slot::CL5::MY_Slot             # Slot group ID (3 segments)
```

The index suffix uses an underscore (`_1`), not a double-colon. The `instance_name` parameter is accepted by the API for symmetry but is not included in the generated ID: IDs are template-scoped.

The `::` separator cannot appear in PatchLang identifiers, making parsing unambiguous. The old `pl_` underscore format is deprecated; the loader should accept both formats during migration.

### Meta Schema

| Key | Type | Used by | Validated |
|-----|------|---------|-----------|
| `kind` | string | UI filtering, DRC, hierarchy | Known values list (see below) |
| `fits` | string (comma-sep) | Slot compatibility | Matches slot formats in scope |
| `rf_subtype` | string | RF system config | Known values list |
| `rf_min_channels` | number | RF channel range | Must be positive |
| `rf_max_channels` | number | RF channel range | Must be >= min |
| `rf_band` | string | RF band info | Only on rf-system devices |
| `manufacturer` | string | Library browsing | No validation |
| `model` | string | Library browsing | No validation |
| `category` | string | Library browsing | No validation |

**Deprecated:** `device_type` is accepted as an alias for `kind` during the transition period. The compiler emits an info-level deprecation warning when `device_type` is encountered and maps it to `kind` internally. New files must use `kind`.

### Template Kinds

The `kind` meta key classifies what a template represents in the project hierarchy. It replaces the former `device_type` field (see D011). Unknown values trigger an info-level warning (not an error), so custom kinds are allowed.

**Device kinds** (templates representing physical hardware):

| Value | Meaning | DRC / UI behavior |
|-------|---------|-------------------|
| *(absent)* | Generic device | Default. No special handling. |
| `device` | Generic (console, amp, camera, router) | Default when absent. Requires `manufacturer` and `model` in stock libraries. |
| `card` | Expansion card (MY16-AUD, HDX) | Uses `fits` for slot compatibility |
| `fixed-converter` | Deterministic routing (stagebox, protocol bridge) | DRC: deterministic routing assumed |
| `stage-core` | Passive XLR loom/snake | — |
| `mic-di` | Single microphone or DI box | — |
| `mic-splitter` | Multi-way analogue signal splitter | See "Splitter Modeling" below |
| `rf-system` | Wireless mic receiver, IEM transmitter | Enables RF meta keys. See "RF Systems" below |

**Composition kinds** (templates representing organizational groupings of devices):

| Value | Meaning | DRC / UI behavior |
|-------|---------|-------------------|
| `system` | Logical grouping of devices (FOH rack, stage system, monitor world) | Must contain at least one `instance`. |
| `venue` | Top-level facility or building | Must not declare physical ports. Must contain at least one `instance`. |

### RF Systems

RF devices use `kind: "rf-system"` and additional meta keys for frequency management:

```
template AD4Q {
  meta {
    manufacturer: "Shure"
    model: "AD4Q"
    kind: "rf-system"
    rf_subtype: "radio-mic"
    rf_min_channels: 4
    rf_max_channels: 4
    rf_band: "G50 (470-558 MHz)"
  }
  ports {
    Antenna_A: in(BNC_50) [RF]
    Antenna_B: in(BNC_50) [RF]
    Antenna_C: in(BNC_50) [RF]
    Antenna_D: in(BNC_50) [RF]
    Analog_Out[1..4]: out(XLR) [analog, line_level]
    AES_Out[1..2]: out(XLR) [AES3]
    Dante_Pri_In[1..4]: in(RJ45) [Dante, AES67, primary]
    Dante_Pri_Out[1..4]: out(RJ45) [Dante, AES67, primary]
    Dante_Sec_In[1..4]: in(RJ45) [Dante, AES67, secondary]
    Dante_Sec_Out[1..4]: out(RJ45) [Dante, AES67, secondary]
    Network_Control_A: io(RJ45)
    Network_Control_B: io(RJ45)
  }
  bridge Antenna_A -> Analog_Out
  bridge Antenna_A -> Dante_Pri_Out
}
```

**Known `rf_subtype` values:** `radio-mic`, `iem`, `bidirectional`.

### Ring Networks (Detailed)

The `ring` keyword declares shared transport bus topologies. Here is the full fixture showing both primary and redundant rings:

```
ring OptoCore_Primary {
  protocol: "OptoCore"
  member Console
  member StageRack_1
  member StageRack_2
  member MonitorRack
}

ring OptoCore_Redundant {
  protocol: "OptoCore"
  label: "Redundant ring via B ports"
  member Console.OptoCore_B
  member StageRack_1.OptoCore_B
  member StageRack_2.OptoCore_B
  member MonitorRack.OptoCore_B
}
```

**Redundant rings:** Standard broadcast practice (Hillsong uses this). Two ring declarations with the same protocol but different port references (A ports vs B ports). Each ring is independent: if one fails, the other carries traffic.

**Protocol groups for compatibility checking:**
- Dante / AES67 (interoperable)
- SDI / HD_SDI / 3G_SDI / 12G_SDI (all SDI variants)
- WordClock / BlackBurst / TriLevel (sync signals)

### Splitter Modeling

Splitters use `kind: "mic-splitter"` and model multiple outputs as separate port arrays:

```
template Splitter_80ch {
  meta {
    model: "80-ch 3-way Splitter"
    kind: "mic-splitter"
  }
  ports {
    Inputs[1..80]: in(XLR)
    Output_A[1..80]: out(XLR)
    Output_B[1..80]: out(XLR)
    Output_C[1..80]: out(XLR)
  }
}
```

**Known gap:** PatchLang does not currently distinguish between passive, active, and transformer-isolated splitter outputs. All outputs are modeled as identical `out(XLR)` port arrays. This is deferred until a real use case demands it.

---

## Compiler API

### Public API Surface

The `patchlang` crate exports these public functions and types:

| Function | Purpose |
|----------|---------|
| `parse(source) -> ParseResult` | Parse only. Returns `{ program, errors }`. No DRC. |
| `check(source) -> CheckResult` | Parse + auto-resolve + DRC. Returns `{ program, errors, diagnostics }`. |
| `compile_project(files, entry) -> ProjectResult` | Multi-file compilation with namespace resolution and merged DRC. |
| `resolve_uses(source) -> Vec<String>` | Quick-parse to extract `use` statement namespaces. |
| `format_source(source) -> Result<String, String>` | Format source into canonical style. Returns Err on parse errors. |
| `parse_manifest(json) -> ManifestResult` | Parse and validate a `project.json` manifest. |
| `validate_layout(json) -> String` | Validate a `.layout.json` against the schema. |
| `validate_project_consistency(patch, layout) -> String` | Cross-validate `.patch` and `.layout.json` instance names. |
| `generate_port_id(instance, template, port, index) -> String` | Deterministic port ID. |
| `generate_route_id(template, source_port, target_port) -> String` | Deterministic route ID. |
| `generate_slot_id(template, slot_name) -> String` | Deterministic slot ID. |
| `format_program(program) -> String` | Format a `PatchProgram` AST directly (no parse step). |
| `PatchProgramBuilder::new()` | Create an empty builder for programmatic AST construction. |
| `PatchProgramBuilder::from_program(program)` | Wrap an existing parsed program for editing. |

**Re-exported types:** `PatchProgram`, `CheckResult`, `Diagnostic`, `ParseError`, `Span`, `ProjectResult`, `ProjectManifest`, `ManifestResult`, `PatchProgramBuilder`, `BuilderError`, `CascadeResult`.

### Single-File Pipeline

For single-file projects or live editing:

- **`parse(source)`**: Parse only. Returns `{ program, errors }`. No DRC.
- **`check(source)`**: Parse + auto-resolution + DRC. Returns `{ program, errors, diagnostics }`. DRC is skipped when parse errors exist.

`check()` is the primary API for the editor. It provides real-time error feedback including auto-index resolution and DRC diagnostics. The pipeline is:

1. Parse source into AST
2. If parse errors exist, return immediately (no DRC)
3. Run auto-resolution pass (`resolve_auto_indices`) to resolve `[auto]` specs
4. Convert AST to TypeScript-compatible output with resolved indices
5. Convert auto-resolution errors to diagnostics
6. Run all DRC checks (`drc::run_all`)
7. Return combined result

### Source Formatter

`format_source(source)` parses the source, walks the AST, and emits a consistently formatted version. Returns `Err` if the source has parse errors.

Behavior:
- Parses the source and rejects files with errors
- Emits each statement type with canonical indentation and spacing
- Blank line between top-level statements
- Trailing newline guaranteed
- **Comments are NOT preserved** (the lexer discards them)

Individual statement emitters are in `formatter_emit.rs`.

### Project Manifest

`parse_manifest(json)` parses and validates a `project.json` string. Returns a `ManifestResult` with:

- `manifest`: parsed `ProjectManifest` (or `None` on invalid JSON)
- `errors`: validation errors

**`ProjectManifest` fields:**

| Field | Type | Required |
|-------|------|----------|
| `name` | string | Yes (must not be empty) |
| `root` | string | Yes (must end with `.patch`) |
| `author` | string | No |
| `created` | string | No |
| `description` | string | No |
| `libraries` | string[] | No (each must end with `.patch`) |
| `dependencies` | map<string, string> | No |

---

## Multi-File Compilation

### Overview

The compiler supports two modes:

- **Single-file:** `check(source)`, see above.
- **Multi-file:** `compile_project(files, entry)`, receives all files as a map, resolves `use` statements internally.

The compiler does **no filesystem I/O**. All files are provided as strings by the caller.

### `resolve_uses`

```rust
pub fn resolve_uses(source: &str) -> Vec<String>
```

Quick-parses a source string and returns the namespace strings referenced by `use` statements. Callers use this to discover dependencies and build the file map before calling `compile_project`.

Example: for source containing `use buildings.foh { FOH_System }`, returns `["buildings.foh"]`.

### `compile_project`

```rust
pub fn compile_project(
    files: HashMap<String, String>,
    entry: &str,
) -> ProjectResult
```

- `files`: map of relative path to source string (e.g., `"campus.patch" -> "..."`)
- `entry`: the root file path (key in the map)

The compiler returns a `ProjectResult` containing:

- `program`: the merged program (all files combined, `use` statements removed)
- `errors`: parse errors, prefixed with `[filename]` for multi-file
- `diagnostics`: DRC diagnostics on the merged program (empty if parse errors exist)
- `files`: BFS-ordered list of file paths visited during compilation (index matches `span.file` on diagnostics)
- `templateFiles`: map of template name to source file path (for hierarchy drill-down)
- `useGraph`: map of file path to list of namespace dependencies (for sidebar tree)

Every statement in the merged program carries a `span` with a `file` field (a `u16` index into the `files` array). This lets the frontend trace any statement or diagnostic back to its source file. For single-file `check()`, `span.file` is absent (null in JSON).

### Multi-File Pipeline

1. Check that the entry file exists in the map
2. BFS from entry, parsing each file independently
3. Resolve `use` statements by mapping namespaces to paths (`buildings.foh` -> `buildings/foh.patch`)
4. Report errors for missing files or duplicate template names
5. Set file provenance (`span.file`) on every statement
6. Merge all non-`use` statements into a combined AST
7. Run DRC on the merged result (skipped if any parse errors)
8. Return `ProjectResult` with provenance metadata

### Namespace-to-Path Resolution

```
resolve_namespace("buildings.foh") -> "buildings/foh.patch"
resolve_namespace("yamaha")        -> "yamaha.patch"
resolve_namespace("lib.custom")    -> "lib/custom.patch"
```

Dots become path separators. `.patch` extension is appended.

---

## Auto-Index Resolution

`check()` and `compile_project()` run an auto-resolution pass after parsing and before DRC. This resolves `[auto]` index specs to concrete channel numbers using sequential packing in declaration order.

### How It Works

1. **Phase 1, pre-scan:** Collect all explicit indices from connects and bridges to build a consumed-channels set per port
2. **Phase 2, resolve:** Walk connections in declaration order; for each `[auto]`, allocate the next N contiguous channels not in the consumed set
3. Results are stored in a side table; the AST retains `Auto` for roundtrip fidelity
4. The JSON output contains resolved concrete indices, not `auto`

Channel count is inferred from the other side of the connection. If the other side specifies `[1..4]`, auto allocates 4 channels. If the other side is scalar (no index), auto allocates 1 channel.

### Auto-Resolution Error Codes

These are non-suppressible errors emitted as diagnostics with `layer: structural`:

| Code | Condition |
|------|-----------|
| A02 | Both sides of a connection use `[auto]` |
| A03 | `[auto]` on a scalar port (no declared range), or cannot infer count from other side |
| A04 | Auto-assignment exceeds the port's declared range |
| A05 | Explicit indices fragment the range: cannot find N contiguous channels |

### S14: Vector Port Without Index

| Code | Severity | Condition |
|------|----------|-----------|
| S14 | Warning | Vector port referenced in a connection without any channel index |

Suppressible via `@suppress(structural)` on the connection.

---

## DRC Engine

### Architecture

The DRC engine runs after parsing and auto-resolution. It operates on the full AST (merged for multi-file). The entry point is `drc::run_all(program)` which calls each layer checker in order:

1. **Structural**: undefined references, duplicate names, port resolution, slot checks, meta hints
2. **Direction**: invalid connection directions (out-to-out, in-to-in)
3. **Mechanical**: physical connector type mismatches
4. **Electrical**: signal level mismatches
5. **Logical**: protocol mismatches
6. **Temporal**: clock domain mismatches
7. **Ring**: ring topology member validation
8. **Flow**: AES67 interoperability (flow slots, channel limits, multicast prefixes)
9. **Convention**: style and usage advisories

### Diagnostic Structure

Each diagnostic serializes as:

```json
{
  "severity": "error" | "warning" | "info",
  "layer": "structural" | "direction" | "mechanical" | "electrical" | "logical" | "temporal" | "ring" | "flow" | "convention",
  "message": "human-readable description",
  "span": { "start": 142, "end": 168, "file": 0 },
  "source": "optional port ref label",
  "target": "optional port ref label",
  "fix": "optional suggestion"
}
```

The `span.file` field is an index into the `ProjectResult.files` array. For single-file `check()`, `span.file` is absent.

### Suppression

Connection-level suppression via `@suppress(layer_name)`. Supported layers: `structural`, `direction`, `mechanical`, `electrical`, `logical`, `temporal`.

### Complete Rule Reference

#### Structural Layer (S01-S16)

| Code | Severity | Rule |
|------|----------|------|
| S01 | Error | Instance references unknown template |
| S02 | Error | Slot assignment references unknown card template |
| S03 | Error | Connect references unknown port on instance |
| S04 | Error | Route references unknown port on instance (checks effective ports: template + card) |
| S05 | Error | Bus input/output references unknown port on instance (checks effective ports: template + card) |
| S06 | Error | Channel index out of range for port |
| S07 | Error | Config block references unknown instance |
| S08 | Error | Signal origin references unknown instance |
| S09 | Error | Signal origin references unknown port on instance |
| S10 | Error | Duplicate instance name |
| S11 | Error | Duplicate signal name |
| S12 | Warning | Slot card does not declare `fits` matching slot format, or `fits` does not match |
| S13 | Warning | Card `fits` value does not match any slot format in scope |
| S14 | Warning | Vector port referenced without channel index (suppressible) |
| S15 | Error | Range size mismatch: left and right sides of `connect` have different channel counts |
| S16 | Error | Card port name collision: card port conflicts with template port or another card's port |

#### Card Port Expansion

When a card template is installed in a slot via a slot assignment on an instance, the card's ports are merged into the instance's effective port namespace using a flat merge. This means card ports are referenced directly (e.g., `FOH.MicIn[1]`) without slot-qualified syntax.

- **Template ports win:** If a card port name duplicates a template port name, the template port takes precedence and an S16 error is emitted.
- **Multi-card collision:** If two different cards installed on the same instance declare the same port name, an S16 error is emitted.
- **Route/bus checks use effective ports:** Internal routing (`route`) and bus declarations (`bus`) check the instance's effective port namespace: both template-declared ports and card-provided ports are valid targets. This means a route like `route MADI[41] -> LINE[1]` works when `MADI` comes from an installed card.

#### Direction Layer (D01-D03)

| Code | Severity | Rule |
|------|----------|------|
| D01 | Error | Cannot connect output to output |
| D02 | Error | Cannot connect input to input |
| D03 | — | (Ports with direction `io` are always valid: skipped) |

#### Mechanical Layer (M01)

| Code | Severity | Rule |
|------|----------|------|
| M01 | Error | Physical connector type mismatch (connectors cannot mate) |

#### Electrical Layer (E01-E02)

| Code | Severity | Rule |
|------|----------|------|
| E01 | Warning | Level mismatch: pad or level adjustment may be needed |
| E02 | Error | Level mismatch: could damage target equipment |

#### Logical Layer (L01)

| Code | Severity | Rule |
|------|----------|------|
| L01 | Error | Protocol mismatch: protocols are not interoperable |

#### Temporal Layer (T01)

| Code | Severity | Rule |
|------|----------|------|
| T01 | Warning | Clock domain mismatch: sample rate conversion may introduce artifacts |

#### Ring Layer (R01-R04)

| Code | Severity | Rule |
|------|----------|------|
| R01 | Error | Ring member references unknown instance |
| R02 | Error | Ring member explicit port does not exist on template |
| R03 | Warning | Ring member port does not have the ring's protocol in its attributes |
| R04 | Error | Implicit ring member: zero or multiple ports match the protocol (ambiguous) |

#### Flow Layer (F01-F03)

| Code | Severity | Rule |
|------|----------|------|
| F01 | Warning | Flow slot exhaustion: stream count exceeds Dante chipset limit |
| F02 | Info | AES67 stream exceeds 8 channels: hardware will auto-split into multiple flows |
| F03 | Error | Multicast prefix mismatch between AES67 devices: audio will silently fail |

#### Convention Layer (C01-C05)

| Code | Severity | Rule |
|------|----------|------|
| C01 | Info | Orphaned instance: has no connections, bridges, ring membership, or config |
| C02 | Warning | Duplicate connection: same source/target port pair connected more than once |
| C03 | Info | Template declared with zero ports |
| C04 | Info | Bus declared with zero outputs |
| C05 | Info | Redundancy terminates at AES67 boundary: AES67 flows use Primary port only |

#### Meta Info Hints (M-I01 through M-I08)

These run as part of the structural layer but use distinct codes:

| Code | Severity | Rule |
|------|----------|------|
| M-I01 | Info | Unknown `kind` value |
| M-I02 | Info | Deprecated `device_type` used: migrate to `kind` |
| M-I03 | Info | Unknown `rf_subtype` value |
| M-I04 | Info | `rf_band` present but `kind` is not `rf-system` |
| M-I05 | Warning | `rf_min_channels` is zero (must be positive) |
| M-I06 | Warning | `rf_max_channels` is less than `rf_min_channels` |
| M-I07 | Info | Unknown `dante_chipset` value: expected Ultimo, Broadway, Brooklyn_II, Brooklyn_3, or HC |
| M-I08 | Warning | Ultimo chipset does not support AES67: instance has `aes67_mode: true` but template uses Ultimo |

---

## Layout Validation

Two functions validate `.layout.json` files:

### `validate_layout(json)`

Validates a `.layout.json` string against the schema. Returns JSON: `{ valid: bool, errors: [...] }`.

**Schema (version 1):**

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `version` | integer | Yes | Must equal `1` |
| `positions` | object | Yes | Keys are instance names |
| `positions.*.x` | number | Yes | |
| `positions.*.y` | number | Yes | |
| `positions.*.collapsed` | boolean | No | |
| `groupBoxes` | array | No | |
| `groupBoxes[].id` | string | Yes | Must be unique |
| `groupBoxes[].label` | string | Yes | |
| `groupBoxes[].x` | number | Yes | |
| `groupBoxes[].y` | number | Yes | |
| `groupBoxes[].width` | number | Yes | |
| `groupBoxes[].height` | number | Yes | |
| `groupBoxes[].color` | string | No | |
| `viewport` | object | No | |
| `viewport.x` | number | No | |
| `viewport.y` | number | No | |
| `viewport.zoom` | number | No | |

Unknown fields at any level produce errors.

### `validate_project_consistency(patch, layout)`

Cross-validates instance names between a `.patch` source and its `.layout.json`. Returns JSON: `{ valid: bool, errors: [...], warnings: [...] }`.

Checks performed:
- Runs `validate_layout` first: returns errors if the layout is invalid
- **Orphaned layout keys**: position keys in the layout with no matching instance in the patch
- **Missing positions**: instances in the patch with no position in the layout

Both are exported via WASM and Python.

---

## Deterministic ID Generation

### Port IDs

```rust
pub fn generate_port_id(
    _instance_name: &str,  // accepted for API symmetry, not used
    template_name: &str,
    port_name: &str,
    index: Option<u32>,    // None for scalar ports
) -> String
```

Format: `pl::{template}::{port}` or `pl::{template}::{port}_{index}`. Always 3 segments when split on `::`.

### Route IDs

```rust
pub fn generate_route_id(template_name: &str, source_port: &str, target_port: &str) -> String
```

Format: `rule::{template}::{source}::{target}`. Always 4 segments.

### Slot IDs

```rust
pub fn generate_slot_id(template_name: &str, slot_name: &str) -> String
```

Format: `slot::{template}::{slot}`. Always 3 segments.

### Sanitization

All segments are sanitized before inclusion:
1. Replace non-ASCII-alphanumeric characters with `_`
2. Collapse consecutive underscores
3. Trim leading/trailing underscores
4. Empty result becomes `"unnamed"`

---

## PatchProgram Builder API

The builder API (`crates/patchlang/src/builder/`) provides programmatic AST construction as an alternative to parsing text. The frontend calls builder methods via WASM instead of concatenating PatchLang strings in a TypeScript emitter. This eliminates the emitter bug class: port naming, direction model, and slot resolution are enforced in Rust.

### Architecture

```
Frontend (TypeScript)              SignalCanvasLang (Rust/WASM)
───────────────────                ──────────────────────────────
Call WASM: add_instance()  ──────► PatchProgramBuilder
                                           │
                                   format() → valid .patch text
                                   check()  → DRC diagnostics
                                   to_json() → AST JSON
```

### Builder Methods

| Method | Returns | Purpose |
|--------|---------|---------|
| `new()` | `PatchProgramBuilder` | Empty builder |
| `from_program(program)` | `PatchProgramBuilder` | Wrap existing parsed program |
| `program()` | `&PatchProgram` | Read-only access to AST |
| `format()` | `String` | Canonical PatchLang text (guaranteed parseable) |
| `check()` | `Vec<Diagnostic>` | Full DRC without serializing |
| `to_json()` | `String` | TypeScript-compatible AST JSON |
| `add_template(decl)` | `Result<(), BuilderError>` | Add template; rejects duplicates |
| `remove_template(name)` | `Result<(), BuilderError>` | Remove; rejects if instances reference it |
| `update_template(name, decl)` | `Result<(), BuilderError>` | Full replacement |
| `add_instance(decl)` | `Result<(), BuilderError>` | Add; validates template exists |
| `remove_instance(name)` | `Result<CascadeResult, BuilderError>` | Cascade: removes connects, bridges, configs, ring members |
| `add_connect(src, tgt, props)` | `Result<String, BuilderError>` | Returns deterministic ID; validates ports + direction |
| `remove_connect(id)` | `Result<(), BuilderError>` | Remove by ID |
| `set_slot(inst, slot, idx, card)` | `Result<(), BuilderError>` | Install card; validates slot + card template |
| `add_route(inst, from, ch, to, ch)` | `Result<(), BuilderError>` | Internal routing |
| `set_routes(inst, routes)` | `Result<(), BuilderError>` | Replace all routes |
| `add_bus(inst, bus)` / `remove_bus(inst, name)` | `Result<(), BuilderError>` | Bus CRUD |
| `set_label(inst, port, idx, label, props)` | `Result<(), BuilderError>` | Channel labels; auto-creates config block |
| `add_signal(decl)` / `add_stream(decl)` / `add_flag(decl)` | `Result<(), BuilderError>` | Signal flow declarations |
| `add_ring(decl)` / `add_ring_member(ring, inst, port)` | `Result<(), BuilderError>` | Ring topology |
| `add_bridge(src, tgt)` | `Result<(), BuilderError>` | Top-level bridges |

### Eager Validation

`add_connect()` validates at build time:
1. Source and target instances exist
2. Source and target ports exist (including card-expanded ports from slot assignments)
3. Direction compatibility: out→out and in→in rejected

Uses the same `build_effective_port_map` as the DRC: no rule duplication.

### Connection IDs

Format: `connect_{srcInst}_{srcPort}_{tgtInst}_{tgtPort}`. Duplicate endpoints get `_2`, `_3` suffix. Deterministic and stable across edits.

### Statement Ordering

`format()` outputs canonical order: uses → card templates → device templates → instances → connects → bridges → signals → streams → flags → configs → rings. Internal storage uses insertion order.

---

## WASM Exports

The `patchlang-wasm` crate exports all functions via `wasm_bindgen`:

```javascript
// Single-file
const parseResult = JSON.parse(parse(source));           // { program, errors }
const checkResult = JSON.parse(check(source));           // { program, errors, diagnostics }
const isValid = validate(source);                         // boolean
const formatted = format_source(source);                  // string or JSON error

// Multi-file
const deps = JSON.parse(resolve_uses(source));           // ["buildings.foh", "yamaha"]
const result = JSON.parse(compile_project(
    JSON.stringify(filesMap), "campus.patch"
));  // { program, errors, diagnostics, files, templateFiles, useGraph }

// Project manifest
const manifest = JSON.parse(parse_manifest(jsonString));  // { manifest, errors }

// Layout validation
const layoutResult = JSON.parse(validate_layout(layoutJson));
const consistency = JSON.parse(validate_project_consistency(patchSource, layoutJson));

// ID generation (NO_INDEX = -1 for scalar ports)
const portId = generate_port_id("Console", "CL5", "Dante_In", 1);   // "pl::CL5::Dante_In_1"
const portIdScalar = generate_port_id("Console", "CL5", "Dante_In", -1);  // "pl::CL5::Dante_In"
const routeId = generate_route_id("CL5", "Mic_In", "Dante_Out");     // "rule::CL5::Mic_In::Dante_Out"
const slotId = generate_slot_id("CL5", "MY_Slot");                    // "slot::CL5::MY_Slot"

// Builder API (handle-based)
const handle = create_program();                          // new empty builder
const handle2 = JSON.parse(create_program_from_source(src)); // from existing .patch
const source = format_program(handle);                    // → .patch text
const json = get_program_json(handle);                    // → AST JSON
const diags = check_program(handle);                      // → diagnostics JSON
free_program(handle);                                     // release memory

// Builder mutations (all return JSON: {"ok":true} or {"error":"..."})
add_template(handle, templateJson);
remove_template(handle, name);
add_instance(handle, instanceJson);
remove_instance(handle, name);                            // → CascadeResult JSON
add_connect(handle, sourceJson, targetJson, propsJson);   // → {"ok":true,"id":"..."}
remove_connect(handle, id);
set_slot(handle, instance, slotName, slotIndex, cardTemplate); // slotIndex: -1 = None
add_route(handle, instance, fromPort, fromCh, toPort, toCh);
set_routes(handle, instance, routesJson);
add_bus(handle, instance, busJson);
set_label(handle, instance, port, index, label, propsJson);
add_signal(handle, signalJson);
add_stream(handle, streamJson);
add_ring(handle, ringJson);
add_ring_member(handle, ringName, instance, port);        // empty port = None
add_bridge(handle, sourceJson, targetJson);
```

**Note:** `generate_port_id` and `set_slot` use `i32` for optional indices because `wasm_bindgen` does not support `Option<u32>`. Pass `-1` for "no index".

**Handle lifecycle:** `create_program` / `create_program_from_source` allocate a handle (u32). `free_program` releases it. Handles are indices into `Vec<Option<PatchProgramBuilder>>`: freed slots are reused.

---

## Python Exports

The `patchlang_python` module exports all functions via PyO3:

```python
import patchlang_python as pl
import json

# Single-file
result = json.loads(pl.check(source))           # { program, errors, diagnostics }
parse_result = json.loads(pl.parse(source))     # { program, errors }
is_valid = pl.validate(source)                   # bool
formatted = pl.format_source(source)             # str (raises ValueError on parse errors)

# Multi-file
deps = pl.resolve_uses(source)                   # list of namespace strings (native Python list)
result = json.loads(pl.compile_project(
    {"campus.patch": source, "buildings/foh.patch": foh_source},
    "campus.patch"
))  # { program, errors, diagnostics, files, templateFiles, useGraph }

# Project manifest
manifest = json.loads(pl.parse_manifest(json_string))  # { manifest, errors }

# Layout validation
layout_result = json.loads(pl.validate_layout(layout_json_str))
consistency = json.loads(pl.validate_project_consistency(patch_source, layout_json_str))

# ID generation (index defaults to None for scalar ports)
port_id = pl.generate_port_id("Console", "CL5", "Dante_In", 1)    # "pl::CL5::Dante_In_1"
port_id_scalar = pl.generate_port_id("Console", "CL5", "Dante_In") # "pl::CL5::Dante_In"
route_id = pl.generate_route_id("CL5", "Mic_In", "Dante_Out")
slot_id = pl.generate_slot_id("CL5", "MY_Slot")
```

**Note:** `compile_project` and `check` return JSON strings, not Python dicts. Call `json.loads()` on the result. `resolve_uses` returns a native Python list (not JSON). `format_source` raises `ValueError` on parse errors (does not return an error string).

### Python Builder API

```python
from patchlang_python import ProgramBuilder
import json

# Create builder
prog = ProgramBuilder()                          # empty
prog = ProgramBuilder.from_source(patch_source)  # from existing .patch

# Add statements (JSON strings for complex types)
prog.add_template(template_json)
prog.add_instance(instance_json)
conn_id = prog.add_connect(source_json, target_json, props_json)
prog.add_route("FOH", "MADI_In", 41, "LINE_Out", 1)
prog.set_label("FOH", "Dante_In", 1, "Lead Vocal")
prog.remove_connect(conn_id)
cascade_json = prog.remove_instance("Stage_Left")

# Output
source = prog.format()          # → .patch text
diags_json = prog.check()       # → diagnostics JSON
ast_json = prog.to_json()       # → AST JSON
```

All errors raise `ValueError`. `remove_instance` returns cascade result as JSON string.

---

## What We Are NOT Building

- No module system or scoping: all templates share a flat namespace after merge
- No incremental or cached compilation: total project size is well under 1 MB, compilation is milliseconds
- No filesystem access in the compiler: callers provide strings
- No dependency ordering by the compiler: `use`-walking from entry is sufficient
