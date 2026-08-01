---
layout: default
title: Changelog
permalink: /changelog/
---

## Revision History

### v0.3.2 — 2026-08-01 (inserts round-trip)

- **Channel and bus inserts are now first-class PatchLang.** An insert breaks a channel or bus out to an external send/return path; because the signal physically leaves the console, the endpoints are real signal flow. They previously lived in the frontend's `.layout.json` sidecar because the canvas DTO could not round-trip them. Resolves issue #31.
- **New bus grammar:** `insert_send:` / `insert_return:` inside `bus { }`, taking a comma-separated list of port references. Buses have no property block, so this is native syntax.
- **New channel-label property:** `insert_send` / `insert_return` on a `config` label, as a *quoted* comma-separated port-reference list. Label property values are key/value only, so a multi-leg list has to be a string.
- **Leg lists are ordered and independent.** `[L]` mono, `[L, R]` stereo; order is significant. No adjacency or equal-width constraint, so scattered patches (send on 3 and 10, return on 4 and 8) are representable. Unlike bus `input:` entries, legs are never grouped by port or channel-unioned. Each leg must carry a single-channel index — ranges are rejected rather than silently expanded.
- **`ChannelLabelOutput` gained a verbatim `properties` bag.** Any label property with no dedicated field used to be silently dropped on load — the reason `insert`, `stand` and `gain` needed the sidecar. Now every unknown key survives the round-trip. Same class of fix as the connect-properties work in v0.3.1.
- **Bug fix — port-ref-valued properties were silently dropped.** `kv_map` returned `None` for `KvValue::PortRef`, so an unquoted property such as `insert_send: Ext_Out[3]` parsed cleanly and then vanished at the DTO boundary. It is now stringified, matching `graph::kv_to_string_map`. This affected *all* label properties, not just inserts.
- **`BusOutput`, `BusEntry`, `BusEmitInput`, `ChannelLabelEmitInput` and `TsBusDecl`** all carry the new fields. All new deserialized fields are `#[serde(default)]`, so payloads from older frontends still load — `patchlang-wasm`'s `add_bus`/`update_bus` deserialize `ast::BusEntry` directly from frontend JSON, where a missing field would otherwise reject the whole payload.
- **Compatibility note:** on v0.3.1 and earlier, an `insert_send:` line inside `bus { }` is silently skipped by the parser's catch-all — no error, data lost. Tools writing this syntax should require v0.3.2+.
- **Test coverage:** 34 new tests (11 encoder/decoder unit, 18 load round-trip, 5 full canvas emit→load) plus a new `11-inserts.patch` fixture with 3 fixture tests. Total: 923 tests.

### v0.2.12 — 2026-05-22 (network construct)

- **`network` top-level construct added.** Declares L2 switched-fabric domain membership for Dante, SoundGrid, AVB, Milan, and AES67 protocols. Parallel to `ring` but unordered — any member can reach any other member.
- **Three member forms:** device-level (`member Device`), port-group (`member Device.Dante_Pri`), slot reference (`member Device.MY_Slot[1]`).
- **DRC rule N01:** validates that referenced instances exist. No port-group validation in v1 — declarative only.
- **Formatter roundtrip:** all three member forms serialize canonically.
- **TypeScript bindings:** `NetworkDecl.ts` + `NetworkMember.ts` exported via ts-rs.
- **Resolves:** issue #163 (network construct for Cluster Provider B).

### v0.2.8 — 2026-04-06 (builder API)

- **PatchProgram Builder API implemented.** Rust-native AST builder replaces the frontend's TypeScript emitter for programmatic program construction. Mutations are eagerly validated (structural + direction checks at build time). See Builder API section in `docs/compiler.md`.
- **Builder module:** `crates/patchlang/src/builder/` (9 files, ~1600 lines). Core struct `PatchProgramBuilder` with `format()`, `check()`, `to_json()`, canonical statement ordering, and cascade deletes.
- **Operations:** Template CRUD, instance CRUD with cascade delete, connections with direction validation, slot assignments (card-expanded port resolution), routes/buses, config labels, signals/streams/flags/rings.
- **Eager validation:** `add_connect()` validates port existence (including card-expanded ports from slot assignments) and direction compatibility using the same `build_effective_port_map` as the DRC. No rule duplication.
- **WASM exports:** 22 new handle-based `#[wasm_bindgen]` functions. Handle lifecycle via `Vec<Option<PatchProgramBuilder>>`. TypeScript definitions in `pkg-web/patchlang_wasm.d.ts`.
- **Python exports:** `ProgramBuilder` pyclass with 13 methods via PyO3.
- **`format_program()` made public.** Formats a `PatchProgram` AST directly without parsing text first.
- **`Deserialize` added to all AST types.** Enables JSON deserialization for WASM/Python builder inputs.
- **Formatter fix:** Slot assignment template names containing non-identifier characters (hyphens, UUIDs) are now quoted in formatter output.
- **Test coverage:** 50 new builder tests across 5 levels (unit, roundtrip, integration, proptest, fixture regression). Total: 571 tests.

### v0.2.6 — 2026-04-01 (template kinds)

- **D011 decided: Template classification via `kind` meta field.** Replaces `device_type` with a broader `kind` field that classifies templates as devices, systems, or venues. No new keywords — `template` remains the sole declaration keyword. Decided via Socratic debate: typed keywords (`device`, `system`, `venue`) rejected in favor of validated metadata, consistent with D005 card precedent.
- **New `kind` values:** `system` (logical grouping of devices — rooms, racks, subsystems) and `venue` (top-level facility or building) join existing device kinds (`device`, `card`, `fixed-converter`, `stage-core`, `mic-di`, `mic-splitter`, `rf-system`).
- **DRC rules keyed on `kind`:** `device` in stock libraries requires `manufacturer`/`model`. `venue` must not declare physical ports. `system` and `venue` must contain at least one `instance`.
- **Backward compatibility:** `device_type` accepted as deprecated alias for `kind`. Compiler emits info-level M-I02 deprecation warning. No breaking changes.
- **Naming rationale:** `kind` chosen over `role` (circular at `role: "device"`), `type` (reserved word in Rust/TS/Python), and `category` (already used for freeform grouping).
- **Migration script:** `scripts/migrate-device-type-to-kind.py` renames `device_type` → `kind` across `.patch`, `.rs`, `.ts`, `.vue`, `.md`, and `.py` files.
- **Compiler change:** `KNOWN_DEVICE_TYPES` renamed to `KNOWN_KINDS` in `catalog.rs`. `meta.rs` checks both `kind` and `device_type` keys with deprecation warning for the latter.

### v0.2.5 — 2026-03-31 (bus label)

- **D010 decided: Bus display names via `label:` in bus body.** Broadcast console naming conventions use `>` and `-` (e.g. `SPOTIFY>FOH`, `PQ>MM`) that are invalid PatchLang identifiers. The bus identifier remains the stable cross-reference key; `label:` carries the human-readable display name. Pattern is consistent with `config` port labels. Decided via Socratic debate — sidecar rejected as wrong semantic layer for named signal-flow entities.
- **Compiler change:** `BusEntry` gains `label: Option<String>`. `TsBusDecl` serializes `label` with `skip_serializing_if` (fully backward-compatible). Parser reads `label: "..."` in bus body using existing `Token::Label`. 4 new TDD tests.
- **Fixtures updated:** `04-internal-buses.patch` and `hillsong-mtg.patch` use `label:` where original display names contained `>` or spaces.
- **Test count: 524**

### v0.2.5 — 2026-03-29 (design decisions update)

- **D005 decided: `bridge` vs `route` semantics.** Fixed/configurable axis. `bridge` = manufacturer-hardwired path (Probe does not push). `route` = operator-configured routing (Probe v2 pushes). Updated `language-reference.md`, `compiler.md`, both `SKILL.md` copies.
- **D006 decided: Range size mismatch in `connect` is a hard error (S15).** Implemented in `structural.rs` with 4 tests. `@suppress(structural)` for intentional partial connects. Added to DRC tables in `language-reference.md` and both `SKILL.md` copies.
- **D007 decided: Import aliasing deferred.** Template naming convention elevated from advisory to required in `language-reference.md` and `specification.md`. Qualified references (`yamaha::CL5`) identified as future escape hatch if needed.
- **D008 decided: WordClock ports are `in`/`out`, not `io`.** BNC connectors are never bidirectional. Fixed `compiler.md`, `language-reference.md`, both `SKILL.md` copies. (Fixture files were already correct.)
- **D009 decided: PTPv2 needs no new port type.** PTP runs over Ethernet; grandmaster role is instance metadata. `decisions.md` updated.
- **`specification.md` updated to v0.2.5.** Rewrote §3.3 ports (split in/out table + WordClock), §3.5 connect (S15 note, suppress layer EBNF), §3.6 bridge (fixed/configurable semantics + bridge vs route table), §3.11 use (naming convention required, no aliasing), §3.16 slot (bare identifier), §4.2 index spec ([auto]), §5 complete example (split Dante ports, correct bridge, two connects per cable).
- **`decisions.md` created.** Running log of all design decisions D001–D009 with rationale and rejected alternatives.
- **`debate-context.md` created.** Product context brief for AI debate agents.
- **Test count: 479** (442 unit + 34 integration + 3 doc).

### v0.2.5 — 2026-03-26

- **Ring keyword fully implemented.** Lexer, parser, AST, and DRC rules R01-R04 all complete. Ring added to language specification (EBNF, keywords, examples). Compatibility layer bridges old and new ring formats. Parser error hints updated to include `ring`.
- **Convention DRC checks added.** Four new rules: C01 orphaned devices, C02 duplicate connections, C03 zero-port templates, C04 empty buses.
- **Auto-channel assignment.** `[auto]` index syntax with error codes A01-A03 for conflicts and exhaustion.
- **Multi-file project compilation.** `compile_project()`, `resolve_uses()`, and `parse_manifest()` APIs finalized.
- **Layout validation.** Layout JSON schema validation and cross-validation against compiled graph.
- **Deterministic ID generation.** Port, route, and slot IDs now produced deterministically for stable diffs.
- **Source formatter.** Canonical pretty-printer for `.patch` source files.
- **Fixture overhaul.** 19 fixture files updated to split `io()` into `in()`/`out()` for directional protocols.
- **Test coverage.** 475 tests passing (up from ~134 in early versions).

### v0.2.4 — 2026-03-26

- **Ring keyword confirmed implemented.** 45 tests passing across parser, DRC, and serialization. DRC validates ring member references (instance existence, port existence, protocol matching). Redundant ring patterns work (primary + backup with explicit port refs). Frontend emitter can safely emit `ring` declarations for roundtrip fidelity.
- **Playground layout engine overhauled.** Switched from MSAGL JS post-processing to ELK ORTHOGONAL routing (single-pass, -265KB bundle). Port geometry now passed explicitly to ELK for accurate edge-to-port alignment. Crossing minimization: worship 4→1, broadcast 3→0.
- **Bench tooling.** `npm run bench:clean` wipes generated artifacts. Per-phase timing (compile/build/layout/render). Crossing detail diagnostics showing exactly which edges cross.

### v0.2.3 — 2026-03-25

- **Open questions page added.** Answers to emitter alignment, multi-file loading, project tree sidebar, error display, and WASM integration questions.
- **Frontend integration guide expanded.** Emitter requirements summary table covering cards, slots, config labels, rings, bidirectional cables, and ID format. Error display rules: never block rendering entirely, show partial results.
- **Compiler API clarified.** `compile_project()` receives all files as a map (no filesystem I/O). `resolve_uses()` returns namespace strings, not file paths. `check()` replaces `parse()` for DRC validation.

### v0.2.2 — 2026-03-23

- **Cards and slots.** Templates with `kind: "card"` and `fits` meta for slot compatibility. Slot assignments use bare identifiers (not quoted strings).
- **Config labels.** Use directional port names: `label Dante_Pri_In[1]: "Lead Vocal"`.
- **Bidirectional cables.** Two `connect` statements per cable (forward + return path).
- **Template naming.** Model only (`CL5` not `Yamaha_CL5`).
- **ID separator.** `::` throughout (`pl::template::port`). Old `pl_` underscore format deprecated.
- **Bridges.** Use directional port names: `bridge Mic_In -> Dante_Pri_Out`.

### v0.2.1 — 2026-03-21

- **Multi-file projects.** `use` statements for cross-file references. `project.json` manifest with root entry point, page list, and library paths.
- **Compiler APIs.** `compile_project(filesMap, rootPath)` for multi-file compilation. `resolve_uses(source)` for dependency discovery. Both available in WASM and Python.
- **Layout sidecar.** `.layout.json` schema for persisting canvas positions, group boxes, and viewport. `validate_layout()` API.

### v0.2.0 — 2026-03-19

- **Initial specification.** Rust parser replacing Chevrotain JS parser. Complete grammar for templates, instances, connect, bridge, signal, config, stream, and ring statements.
- **Port direction model.** `in`, `out`, `io` with protocol and connector attributes. `io` reserved for ring/bus protocols (OptoCore, TWINLANe, AVB/Milan, GigaACE).
- **Index specs.** Single indices (`[1]`), ranges (`[1..32]`), and slices on connect/bridge statements.
- **DRC engine.** Seven rule categories: structural, electrical, logical, mechanical, temporal, direction, and ring topology. Suppressions via `@suppress`.
- **Design decisions.** All ratified through structured Socratic debate and cross-agent consensus (appendix).
