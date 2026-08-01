---
layout: default
title: Design Decisions
permalink: /decisions/
---

# PatchLang Design Decisions

A running log of significant design decisions — what we chose, what we rejected, and why. Exists so we don't re-litigate settled questions.

---

## Format

Each entry follows this structure:

**Date** | **Status** (decided / pending / revisit)
**Question:** What were we deciding?
**Decision:** What did we choose?
**Rejected alternative:** What did we not choose, and why?
**Rationale:** The reasoning that drove the decision.

---

## Decisions

---

### D001 — IT Infrastructure Scope
**2026-03-28** | **Decided**

**Question:** Should PatchLang model IT network infrastructure — Ethernet switches, switch ports, VLANs, network topology?

**Decision:** Out of scope. PatchLang does not model switches or network topology.

**Rejected alternative:** Lightweight infrastructure nodes (e.g., `instance Core_Switch is ManagedSwitch`) that could document which switch port each device connects to, enabling DRC checks like "primary and secondary Dante paths traverse separate switches."

**Rationale:** The target user is an AV engineer, not an IT/network engineer. Dante and other IP audio protocols are modeled as logical virtual networks — all Dante devices can connect to each other without explicitly routing through switch infrastructure. Pulling IT infrastructure into scope would significantly expand language complexity, attract a different user profile, and pull engineering effort away from AV signal flow. The DRC benefit (catching same-switch redundancy failures) does not justify the scope expansion for this audience.

---

### D002 — Dante Secondary Redundancy Modeling
**2026-03-28** | **Decided**

**Question:** Should Dante secondary redundancy ports be modeled as explicit `connect` statements (same as primary), or as annotation metadata on the primary connect?

**Decision:** Annotation metadata on the primary connect. Templates should not declare `Dante_Sec_In` / `Dante_Sec_Out` ports.

```patch
connect Stage.Dante_Pri_Out[1..32] -> Console.Dante_Pri_In[1..32] {
  cable: "Cat6a_Pri"
  redundant_cable: "Cat6a_Sec"
}
```

**Rejected alternative:** Explicit secondary ports declared in templates and wired with full `connect` statements — treating secondary identical to primary in the language model.

**Rationale:** The only compelling argument for explicit secondary connects was DRC validation of switch topology (verifying that primary and secondary paths traverse separate physical switches). That argument collapsed when we decided IT infrastructure is out of scope (D001) — if PatchLang doesn't model switches, there is nothing to validate secondary connects against. With that DRC benefit gone, explicit secondary connects impose a 2x verbosity cost (40 connects for a 10-device system instead of 20) for zero analytical benefit. Secondary Dante carries no independent signal — it is a mirror of primary managed automatically by Dante Controller, requiring no engineering decisions. It is a property of the primary connection, not a new signal relationship, and should be modeled as such.

---

### D003 — WordClock Port Direction
**2026-03-28** | **Pending**

**Question:** Should WordClock ports use `io` (current spec) or split `in`/`out`?

**Context:** WordClock is physically directional (master → slaves, separate BNC connectors for In and Out on most gear). Using `in`/`out` would enable DRC to catch topology errors. The complication is that some devices can be either master or slave depending on configuration.

**See also:** D004 (AVB/Milan — same class of question)

---

### D004 — AVB/Milan Port Direction
**2026-03-28** | **Pending**

**Question:** Should AVB/Milan ports use `io` (current spec) or split `in`/`out`?

**Context:** Same question as WordClock but with a different physical reality — AVB/Milan devices typically have one Ethernet jack carrying both directions simultaneously (unlike WordClock's separate BNC connectors). Devices can be simultaneous Talkers and Listeners.

**See also:** D003 (WordClock — same class of question)

---

### D005 — `bridge` vs `route` Semantics
**2026-03-28** | **Decided**

**Question:** What should `bridge` mean — the physical/logical axis ("no physical cable") or the fixed/configurable axis ("manufacturer-hardwired guarantee")? And what distinguishes `bridge` from `route`?

**Decision:** Fixed/configurable axis. Two distinct scopes:

| Scope | Keyword | Meaning | Probe v2 behavior |
|---|---|---|---|
| Inside `template` | `bridge Mic_In -> Dante_Pri_Out` | Signal path guaranteed by device design. Exists in every unit of this template regardless of software configuration. DRC treats as invariant. | Do NOT push — it is fixed hardware behavior |
| Inside `instance` | `route Dante_In[1] -> Line_Out[3]` | Operator-configured routing state for this specific device. May change between shows. | Push via SCP / Ember+ / AES70 / Q-SYS in Probe v2 |
| Top-level between instances | `bridge Stage.Mic_In[1..16] -> Console.Dante_Pri_In[1..16]` | System designer's DRC assertion. "For signal tracing, treat this as a guaranteed path." | Read-only — documents logical signal flow, not pushed |

**Rejected alternative:** Physical/logical axis — `bridge` = "no physical cable" regardless of whether the path is hardwired or operator-configured. Signal Trace works correctly under either axis. The physical/logical axis fails specifically when Probe v2 pushes configuration to live hardware: under that model, all internal paths in a software-defined console (CL5, SD12) become `bridge`, giving Probe no way to distinguish "do not touch — hardwired" from "push this — operator-configured." That makes correct push implementation impossible.

**No new keyword needed.** The physical/logical axis concept — "all non-cable paths" — is fully covered by `bridge` + `route` together. Signal Trace traverses both. The only thing the physical/logical axis offered was collapsing them into one keyword, which is exactly what Probe v2 prevents.

**Rationale:** The Configuration Push feature (Probe v2) is decisive. When Probe pushes routing to live hardware, it must have a deterministic manifest of what to touch. `route` = push. `bridge` = do not touch. This round-trip — Probe reads live state → writes `route` in `.patch` → push reads `route` and sends to device — only works if the keyword carries the "is pushable" semantic. Additionally, Signal Trace gains richer annotation under this model: `bridge` hops can be labeled "guaranteed by hardware design" while `route` hops can be labeled "depends on current operator configuration" — information that is genuinely useful when tracing a fault at 11pm.

**Implications for existing spec:**
- The `language-reference.md` definition of `bridge` ("logical signal mapping between ports, no physical cable") must be updated to reflect fixed/configurable semantics.
- `compiler.md` must add a section defining the semantic contract for `bridge` vs `route`.
- The PatchLang skill (`SKILL.md`) must be updated — LLMs generating device templates need to know: use `bridge` only for manufacturer-hardwired paths; operator-configurable internal routing belongs in the instance as `route`.
- Existing template examples (stageboxes using `bridge Mic_In -> Dante_Pri_Out`) are correct — these are manufacturer-hardwired paths.
- Consoles with fully software-defined internal routing (CL5, SD12) should have no `bridge` declarations in their templates; routing state is documented as `route` in instances.

---

### D006 — Range Size Mismatch in connect
**2026-03-29** | **Decided**

**Question:** What should happen when the left and right sides of a `connect` range have different sizes?

```patch
connect Stage.Dante_Pri_Out[1..16] -> Console.Dante_Pri_In[1..8]  # 16 ≠ 8
```

**Decision:** Hard compile error (DRC code `S15`, Structural layer, severity: Error). The file does not compile until the engineer explicitly resolves the mismatch.

**Rejected alternatives:**
- *Warning-only:* Engineers under time pressure (show in 2 hours) dismiss warnings. A warning on a mismatched range produces a file that appears valid while Signal Trace outputs are silently wrong for the unmatched channels.
- *Silent truncation to smaller side:* Introduces two ambiguities — which end truncates, and whether the mismatch was intentional. Produces confidently wrong DRC output ("channels 9–16 are unconnected") with no flag to the engineer. This is the most dangerous option.
- *Defined `partial` keyword:* A valid long-term enhancement but not needed now. YAGNI — hard error is the safe default, and `partial` can be added deliberately when the use case is well-understood.

**Rationale:** The cost asymmetry is decisive. A hard error costs 30 seconds: read the message, fix the typo, recompile. A silent wrong costs a show: engineer patches channels 9–16, hears nothing, debugs Dante Controller, SFP, console routing — never opening the `.patch` file because it compiled successfully. Signal Trace reliability depends on correct range semantics at every hop; a silently-truncated connect poisons the trace.

The `@suppress(structural)` mechanism already handles intentional deviation from structural rules. An engineer documenting a partial system (32 channels into a 64-input console) can write:

```patch
connect Stage.Dante_Pri_Out[1..32] -> Console.Dante_Pri_In[1..32] {
  @suppress(structural)   # intentional: console has 64 inputs, only 32 wired this show
}
```

This forces explicit intent at zero extra cost to engineers who know what they're doing, while protecting engineers who made a typo.

**Analogy:** VHDL and Verilog both hard-error on port-width mismatches by default.

**Affects:** `compiler.md` DRC table, `language-reference.md` DRC table (new S15 entry).

---

### D007 — Import Aliasing
**2026-03-29** | **Decided**

**Question:** Should PatchLang's `use` statement support `as` aliasing to resolve template name collisions between libraries?

```patch
# Proposed aliasing syntax (rejected for now)
use yamaha { CL5 as Yamaha_CL5 }
use corporate.racks { Patch_Bay as Corp_Patch_Bay }
```

**Decision:** Defer aliasing. Do not add `as` aliasing syntax now. Codify the naming convention as a spec requirement instead. Revisit if a confirmed real-world collision is reported.

**Rejected alternative:** Add `as` aliasing to the `use` statement immediately. The for case was structurally sound — without aliasing, a collision between two third-party libraries is an unresolvable hard error — but YAGNI wins: zero collisions exist in the current library, and the naming convention demonstrably prevents them.

**Naming convention (now required, not just advisory):** Template names in shared libraries must use a manufacturer prefix or model number — not generic names. `CL5`, `Rio3224`, `SD12`, `5601MSC` are correct. `Patch_Bay`, `Power_Amp`, `Line_Level_Converter` are not acceptable as standalone names in a shared library; they must be prefixed (`Neutrik_Patch_Bay`, `Yamaha_Power_Amp`). Generic names are acceptable only in project-local templates that are never published as a library.

**Future escape hatch:** If collision support ever becomes necessary, the preferred design is **qualified references** (`yamaha::CL5`) rather than `as` aliasing. Qualified references preserve the original template identity at every use site — which matters for both human readers and LLM code generation — while still disambiguating. They align naturally with the existing library path structure (`lib/audio/yamaha.patch`). This is not in scope now but is the right direction if the language grows there.

**Rationale:** Aliasing creates a local fiction — `Corp_Patch_Bay` exists in one file and nowhere else, severing context for every engineer who reads the project later. It also rewards bad library naming. Model numbers are globally unique by industrial convention. The existing library has no collisions. Adding aliasing now solves a hypothetical at real readability and LLM-generability cost.

**Affects:** `language-reference.md` — strengthen naming convention from advisory to required in the `use`/import section.

---

### D008 — WordClock Port Direction
**2026-03-29** | **Decided** (input from broadcast engineers)

**Question:** Should WordClock ports use `io` (current spec) or split `in`/`out`?

**Decision:** Split `in`/`out`. WordClock ports are directional and must be declared as separate ports:

```patch
WordClock_In:  in(BNC_75)  [WordClock]
WordClock_Out: out(BNC_75) [WordClock]
```

Devices that are always clock masters (SPGs, grandmaster appliances) declare only `WordClock_Out`. Devices that are always clock slaves declare only `WordClock_In`. Devices that can be either (e.g., a console that can be master or slave) declare both.

**Rejected alternative:** `io(BNC_75) [WordClock]` — the current spec default.

**Rationale:** "I've never seen a BNC that is bidirectional." WordClock uses separate physical 75Ω BNC connectors for input and output on every device. The `io` classification was wrong from the start — it implies a shared bidirectional connector that does not exist in the real world. Splitting to `in`/`out` also enables the DRC to catch real wiring errors: two clock outputs connected together (two masters fighting), or a device with no clock input connected (unsynced).

**Note on embedded clock in protocols:** MADI, Dante, and other audio protocols carry a word clock signal implicitly inside the protocol stream. This does not require a separate WordClock port — it rides along with the existing `Dante_Pri_In`/`Dante_Pri_Out` or `MADI_In`/`MADI_Out` ports. No change needed for protocol-embedded clocking.

**Affects:** `compiler.md` IO direction model table, `SKILL.md` Critical Rule #1, generated fixture files (`focusrite-rednet-a16r.patch`, `evertz-5601msc.patch`).

---

### D009 — PTPv2 (IEEE 1588) Port Modeling
**2026-03-29** | **Decided** (research-based)

**Question:** Does PTPv2 — used by AES67, SMPTE 2110, Ravenna, Q-LAN — need its own dedicated port type in PatchLang?

**Decision:** No new port type. PTP is represented as:
1. A protocol attribute tag on existing Ethernet ports: `[AES67, PTP]` or `[SMPTE_2110]`
2. Instance-level metadata for grandmaster role and domain configuration:

```patch
instance House_GM is Evertz_5700MSC_IP {
  location: "Master Clock Rack"
  ip: "10.0.1.10"
  ptp_role: "grandmaster"
  ptp_domain: 127
  ptp_priority1: 0
}
```

**Rejected alternative:** A dedicated `PTP_Out` port on grandmaster devices or `PTP_In` on slave devices.

**Rationale:** There is no physical "PTP In" or "PTP Out" connector on any AV device. PTP is a multicast UDP protocol (ports 319/320) that runs entirely inside the Ethernet layer over the same jack used for audio/video media. Grandmaster/slave roles are elected dynamically at runtime via the Best Master Clock Algorithm (BMCA) — they are not fixed at patch time. Adding a `PTP_Out` port would fabricate a physical port that does not exist and imply that slaves need a dedicated PTP cable routed from the grandmaster, which is false.

NMOS IS-04/IS-09 (the industry standard for IP broadcast device registration) models PTP as device registration metadata and system configuration parameters — not as port connections. This confirms the correct model: PTP domain membership is a configuration attribute on the device, not a signal path between devices.

**Multi-domain edge case:** In facilities running both Dante (PTPv2 domain 0) and SMPTE 2110 (PTPv2 domain 127) on the same network, domain membership can be documented with protocol attribute tags: `[SMPTE_2110, PTP_domain_127]`. This is an enhancement for future consideration — not required for MVP.

**Note on Dante and PTP:** Dante uses PTPv1 internally. When a Dante domain is placed in AES67 mode, it bridges to PTPv2 domain 0. When in SMPTE mode, it bridges to PTPv2 domain 127. This cross-domain bridging is handled by Dante Domain Manager and does not affect PatchLang port modeling.

---

### D010 — Intercom Port Modeling Scope
**2026-03-29** | **Decided**

**Question:** Which intercom ports should be modeled as signal flow edges in SignalCanvas? Should headset/partyline XLR ports split `in`/`out` like Dante? Should management/control ports be modeled at all?

**Decision:** Model the matrix and physical signal sources only. Three tiers:

1. **Intercom matrices** (Eclipse HX, Artist, ADAM-M) — model fully. These are routing devices and belong in the signal graph.

2. **Panel physical audio inputs** that source into the matrix — model as `in()` ports. Example: a mic or program input on a panel that feeds audio up to the matrix and out to the rest of the system. These are real signal origins.

3. **Headset/monitoring ports, management LAN, control interfaces** — do not model. Headset connections are local user I/O, not system signal flow. Control interfaces (`LAN: io(RJ45)`) are infrastructure. Neither is something you would draw a cable to in SignalCanvas.

**On partyline loops from a matrix:** A matrix's `Partyline[1..4]: io(XLR)` ports connect to beltpacks via physical XLR cables — these ARE signal flow edges and should be split `in`/`out` on the matrix template.

**Rejected alternative:** Modeling every panel and beltpack port as a first-class signal flow edge. The guiding principle is to document the matrix and physical signal sources — panels appear as endpoints, not routing nodes.

**On control interfaces:** Control interfaces are out of scope, consistent with D001 (IT infrastructure out of scope).

**Rationale:** SignalCanvas documents signal flow paths that an AV engineer cares about tracing — sources, routes, and destinations. A headset plugged into a beltpack is a local I/O connection for the operator wearing it, not a system signal path. The matrix is the signal routing hub; that is what gets documented.

**Implications for library files:**
- `Eclipse_HX.Partyline[1..4]: io(XLR)` → split to `Partyline_In[1..4]` / `Partyline_Out[1..4]`
- `V12.Headset: io(XLR)` → leave as `io()` or omit
- `LAN: io(RJ45)` (all devices) → leave as `io()`, excluded from signal graph

**See also:** D004 (AVB/Milan — same class of question, still pending)

---

### D011 — Template Classification (`kind` Meta Field)
**2026-04-01** | **Decided**

**Question:** How should PatchLang distinguish different types of templates (devices, rooms, buildings, venues)? Should the language add new keywords (`device`, `system`, `venue`), or use metadata?

**Decision:** No new keywords. Keep `template` as the sole declaration keyword. Rename the existing `device_type` meta field to `kind` and expand it with hierarchy values:

**Device kinds** (physical hardware):
- `device` (default when absent), `card`, `fixed-converter`, `stage-core`, `mic-di`, `mic-splitter`, `rf-system`

**Composition kinds** (organizational groupings):
- `system` — logical grouping of devices (FOH rack, stage system, monitor world)
- `venue` — top-level facility or building

```patch
# A device
template CL5 {
  meta { kind: "device", manufacturer: "Yamaha", model: "CL5" }
  ports { ... }
}

# A system (room-level composition)
template FOH_System {
  meta { kind: "system" }
  instance Console is CL5 { location: "FOH Mix Position" }
  instance Playback is RME_Digiface { location: "FOH Rack" }
  connect Playback.Dante_Out[1..8] -> Console.Dante_Pri_In[33..40]
}

# A venue (top-level)
template MTG_Campus {
  meta { kind: "venue" }
  instance FOH is FOH_System { location: "Front of House" }
  instance Stage is Stage_System { location: "Main Stage" }
  connect Stage.Dante_Tie -> FOH.Stage_Tie
}
```

**DRC rules keyed on `kind`:**
- `kind: "device"` in stock libraries requires `manufacturer` and `model`
- `kind: "venue"` must not declare physical ports
- `kind: "system"` and `kind: "venue"` must contain at least one `instance`

**Backward compatibility:** `device_type` is accepted as an alias for `kind` during migration. The compiler emits an info-level deprecation warning (M-I02) when `device_type` is encountered.

**Rejected alternatives:**

1. *Typed keywords (`device`, `system`, `venue`):* Would require 2–3 new AST nodes, parser branches, and DRC paths. Contradicts the D005 card precedent (metadata over keywords). Creates classification ambiguity at edge cases (is a rack-mounted stagebox with internal DSP a `device` or a `system`?). Breaks compositional neutrality — templates can no longer nest freely.

2. *Two separate fields (`kind` + `device_type`):* Introduces cross-validation burden (what if `kind: "venue", device_type: "card"`?). The DRC code in `meta.rs` treats `device_type` as a single flat discriminator — splitting it into two axes adds complexity for no current consumer. YAGNI.

3. *Keep `device_type` unchanged:* `device_type: "venue"` is semantically wrong — a venue is not a device type. The field name misleads both human readers and LLMs.

**Rationale:** The D005 card decision established the precedent: metadata over keywords for template classification. This decision extends that pattern up the hierarchy. A validated `kind` field captures 80–90% of the benefit of typed keywords (DRC scoping, Probe clarity, readability) at roughly 10% of the cost (no grammar changes, no migration of existing syntax, no compositional restrictions). The rename from `device_type` to `kind` reflects the broadened scope — `kind` covers both device subcategories and hierarchy levels in a single flat enum. The name `kind` was chosen over `role` (fails at `role: "device"` — circular), `type` (reserved word in Rust/TypeScript/Python), and `category` (already used for freeform grouping).

**Affects:** `compiler.md` Meta Schema and Device Types sections, `language-reference.md` Meta Block, `debate-context.md` Decisions Already Made, `catalog.rs` KNOWN_DEVICE_TYPES → KNOWN_KINDS, `meta.rs` validation logic, SKILL.md, all fixture `.patch` files containing `device_type`.

---

### D012 — Backbone Connection Syntax
**2026-04-02** | **Decided** (Socratic debate)

**Question:** How should PatchLang express transparent backbone connections — the surface-to-engine link that fuses two devices into one logical system (A&H dLive S7000 ↔ DM64 via GigaACE, Yamaha RIVAGE Surface ↔ DSP Engine, DiGiCo SD-Rack ↔ Console via OptoCore)?

**Decision:** Use `backbone: true` as a boolean key-value property on existing `connect` statements. No new keyword, no parser changes.

```patch
connect S7000.GigaACE_Pri_Out -> DM64.GigaACE_Pri_In {
  backbone: true
  cable: "GigaACE_Pri"
}
connect DM64.GigaACE_Pri_Out -> S7000.GigaACE_Pri_In {
  backbone: true
  cable: "GigaACE_Pri"
}
```

Dual redundant GigaACE = 4 connect statements (2 directions × 2 cables), each with `backbone: true`. Consistent with the existing bidirectional cable convention.

**Backbone semantics:**
- Signal Trace traverses backbone connections transparently (no visible hop displayed to user)
- DRC exempts backbone connections from direction/protocol validation — the two devices operate as one routing grid
- DRC warns if a backbone connect is missing its return direction
- DRC may offer advisory warnings for unusual device kind pairings (e.g., two stageboxes), never hard errors
- Backbone connections are not user-patchable — they represent infrastructure

**Frontend rendering:**
- Two separate canvas nodes, each showing their own physical IO
- Backbone wire renders subtle/invisible (distinct from normal patchable connections)
- Internal routing opens a combined view showing IO from both devices
- Signal trace flows transparently across backbone

**Manufacturers covered by this pattern:**
- A&H dLive (GigaACE over Cat5e, 700+ channels, dual redundancy)
- Yamaha RIVAGE PM (proprietary Ethernet, Surface ↔ DSP Engine)
- DiGiCo SD-Range (OptoCore backbone, SD-Rack ↔ Console)
- Lawo mc² (RAVENNA backbone to A__UHD Core / Nova73)
- Calrec Summa/Impulse (Hydra2/BlueFin2 backbone to Modular I/O)
- SSL System T (Dante backbone with proprietary control layer to Network I/O)
- Studer Vista (A-Link backbone to D21m I/O)
- Exceptions: integrated consoles (A&H Avantis/SQ, Yamaha CSD-R7/PM7) — engine built into surface, no backbone needed

**Rejected alternatives:**

1. *`mode: "backbone"` (original proposal):* The `mode` field already carries video transport semantics (`mode: "quad_link_4K"` for SDI). Using it for connection classification (backbone vs normal) overloads a single field with two unrelated semantic axes — "how a signal is transported" vs "what role this connection plays." A dedicated boolean avoids future collision.

2. *Implicit detection via `Console Link` protocol:* When both interfaces use `Console Link` protocol, auto-detect as backbone without explicit annotation. Rejected because it violates PatchLang's no-ambiguity principle (design principle 4) — identical syntax would produce different semantic behavior depending on protocol metadata in template files. An engineer reading the `.patch` file cannot tell whether a connection is backbone without cross-referencing templates. If auto-detection is wanted later, it should be a DRC *suggestion* ("this looks like a backbone — did you mean `backbone: true`?"), not silent reclassification.

3. *`bridge` for backbone connections:* `bridge` means "logical signal mapping, no physical medium implied." GigaACE is a physical Cat5e cable — using `bridge` for it would misrepresent the physical reality. Additionally, Probe would need to emit `bridge` for a physical cable, which is semantically wrong.

4. *New `backbone` keyword:* Would require a new lexer token, AST node, and parser rule. The D011 card precedent is dispositive: if cards did not get a keyword, backbones should not either. Key-value metadata on existing constructs is the established pattern.

5. *`link_group` for redundant pairs:* GigaACE primary + secondary could be bundled in a `link_group`. While `link_group` was designed for multi-cable logical units (quad-link SDI), backbone redundancy is a different concept — the cables are independent infrastructure paths, not parts of one signal. The frontend can group backbone connects visually without needing emission changes.

**Rationale:** The Socratic debate surfaced a genuine semantic tension: `connect` means "physical cable between two ports" while backbone is described as "not patchable, not visible in signal trace." The devil's advocate argued this is a contradiction — `backbone: true` negates `connect`'s own definition. The resolution: GigaACE IS a physical cable you can touch, so `connect` is the correct physical primitive. The `backbone: true` flag changes how downstream consumers (DRC, Signal Trace, renderer) interpret the connection, not what the connection physically is. This follows the same pattern as `@suppress(structural)` — metadata that modifies how validation interprets a statement without changing the statement's physical meaning.

The key design constraint is that no parser changes are needed. The compiler already accepts arbitrary key-value pairs in connect bodies. `backbone: true` is purely a semantic annotation consumed by the DRC and frontend.

**Affects:** `debate-context.md` Decisions Already Made, `language-reference.md` Connect section (add backbone property documentation), `SKILL.md` (add backbone examples), example fixtures (add dLive/RIVAGE backbone examples). Frontend: rendering logic for backbone connections.

**Related issues:** ByteBard97/SignalCanvas#68, ByteBard97/SignalCanvas#38

---

### D013 — AES67 Interop Modeling
**2026-04-03** | **Decided**

**Question:** How should PatchLang model Dante devices operating in AES67 compatibility mode — TX stream declarations, flow slot constraints, multicast prefix matching, and redundancy limitations?

**Decision:** No new syntax. Use existing constructs plus metadata:

1. **AES67 TX streams** use the existing `stream` keyword with `protocol: "AES67"`.
2. **Chipset awareness** via `dante_chipset` meta field on templates (values: `Ultimo`, `Broadway`, `Brooklyn_II`, `Brooklyn_3`, `HC`).
3. **AES67 mode** via `aes67_mode: true` instance property.
4. **Multicast prefix** via `multicast_prefix: 71` instance property.
5. **DRC rules** (new `Flow` layer):
   - F01: Flow slot exhaustion — count streams per device vs chipset limit
   - F02: AES67 stream max 8 channels — warn if exceeded (hardware auto-splits)
   - F03: Multicast prefix mismatch — error when TX/RX prefixes differ (silent audio failure)
   - C05: Redundancy terminates at AES67 boundary — advisory warning
6. **PTP clocking** already handled by D009 (instance metadata, not ports).

```patch
template Shure_MXA910 {
  meta {
    manufacturer: "Shure"
    model: "MXA910"
    kind: "device"
    dante_chipset: "Brooklyn_II"
  }
  ports {
    Dante_Pri_In[1..10]: in(etherCON) [Dante, primary]
    Dante_Pri_Out[1..10]: out(etherCON) [Dante, primary]
  }
}

instance Ceiling_Mic is Shure_MXA910 {
  aes67_mode: true
  multicast_prefix: 71
}

stream Ceiling_AES67 {
  source: Ceiling_Mic.Dante_Pri
  channels: 8
  protocol: "AES67"
}
```

**Rejected alternatives:**

1. *New `aes67_stream` keyword:* Violates the D011/D012 precedent — metadata over keywords. The existing `stream` with `protocol: "AES67"` is sufficient.

2. *Dedicated AES67 port types:* AES67 streams use the same physical Ethernet port as native Dante. No separate connector exists.

3. *Full constraint modeling (firmware versions, DDM requirements, SMPTE domain locking):* YAGNI. The chipset-level constraints (flow slots, prefix matching, redundancy) catch the most common real-world failures. Firmware-level constraints can be added later if needed.

**Rationale:** The research (see `docs/research/Dante AES67 Compatibility Technical Report.md`) shows that AES67 interop failures in the field are dominated by three causes: flow slot exhaustion, multicast prefix mismatch, and unexpected redundancy loss at protocol boundaries. All three are detectable with static analysis using only chipset type and instance configuration — no runtime state needed. The existing `stream` keyword naturally models AES67 TX flows. No parser changes required.

**Affects:** `drc/catalog.rs` (chipset lookup table), `drc/meta.rs` (dante_chipset validation), new `drc/flow.rs` module, `drc/convention.rs` (C05 redundancy warning), `TODO.md` section 1.9.

**Related issues:** ByteBard97/SignalCanvas#42

---

### D014 — (Record Not Found)
**~2026-04-04** | **Lost**

This entry is missing from the decision log. D013 (AES67 Interop, 2026-04-03) and D015 (S04/S05 Effective Ports, 2026-04-05) are adjacent — D014 was likely discussed in that window but the entry was not recorded. If the decision is recovered from session history, replace this placeholder with the full record.

---

### D015 — S04/S05 Route and Bus Checks Must Use Effective Ports
**2026-04-05** | **Decided**

**Question:** Should route (S04) and bus (S05) DRC checks validate port references against only the template's declared ports, or against the instance's effective port namespace (template ports + card-provided ports)?

**Decision:** Use effective ports. Routes and buses inside an instance body may reference card-provided ports. `route MADI[41] -> LINE[1]` is valid when `MADI` comes from a card installed via a slot assignment.

**Rejected alternative:** "Route/bus checks unchanged — only template ports are valid targets for internal routing." This was the original spec (compiler.md, pre-2026-04-05). It caused 130 false S04/S05 errors on the Hillsong MTG project, where the Venue FOH Rack's buses reference `MADI` ports from an installed MADI card.

**Rationale:** The semantic distinction is between `resolve_port_on_template()` (template only) and `resolve_effective_port()` (template + cards). Connect checks (S03/S16) already use effective ports. Routes and buses are also instance-level constructs — they describe how *this specific instance* with *these specific cards installed* routes signals internally. There is no reason to restrict them to template-only ports when the card ports are physically present on the device.

**Affects:** `crates/patchlang/src/drc/structural.rs` — `check_route_port_refs()`, `check_bus_port_refs()`.

**Related issues:** ByteBard97/SignalCanvasLang#4, ByteBard97/SignalCanvasLang#5

---

### D016 — Case Sensitivity Policy
**2026-04-05** | **Decided**

**Question:** Should PatchLang be case-sensitive or case-insensitive? The spec never stated this. The compiler used exact string matching (case-sensitive) by accident. This caused `Analogue` and `analog` to both fall through to `TagCategory::Unknown` in the DRC catalog — neither triggered protocol matching, level checking, or any DRC rule. 7 library files used `Analogue`, 49 used `analog`. Manufacturer names varied (`YAMAHA` vs `Yamaha`).

**Decision:** Case-insensitive for attributes, connectors, and meta values. Case-sensitive for identifiers (template names, instance names, port names).

- **Attributes** like `Dante`, `analog`, `primary`, `redundant` — case-insensitive. `analog` matches `Analogue` matches `ANALOG`.
- **Connectors** like `XLR`, `etherCON`, `BNC_75` — case-insensitive. `xlr` matches `XLR`.
- **Meta values** like `manufacturer`, `model` — case-insensitive for catalog lookups (e.g., `"YAMAHA"` and `"Yamaha"` are the same manufacturer).
- **Identifiers** (template names, instance names, port names, slot names) — case-sensitive. `FOH_Console` ≠ `foh_console`. These are user-defined names that function like variable names and must remain distinct for ID generation and cross-referencing.

**Rejected alternatives:**

1. *Fully case-sensitive (Option A):* Requires auditing and fixing 373+ templates to match canonical forms. High migration cost. Users silently get `TagCategory::Unknown` for casing typos with no feedback.

2. *Fully case-insensitive (Option C):* Risks silent identifier collisions (`FOH_Console` vs `foh_console`). Breaks deterministic ID generation (`pl::CL5::Dante_In_1`). Unusual for a modern DSL.

3. *Case-sensitive with DRC "did you mean?" warnings (Option D):* Principled but creates maintenance burden for a canonical spelling catalog. The emitter generates code from UI input — if it outputs `analog` but the catalog says `Analogue`, the user gets a warning they can't fix without editing generated code. Can be layered on top of this decision later for identifiers (template name typo detection) without conflict.

**Rationale:** Determined via Socratic debate (4 perspectives). The key factor: the emitter generates `.patch` files from frontend UI input. If the language is strict about casing for things the user doesn't directly control (protocol names, connector types), every casing mismatch between the emitter and the catalog becomes a bug the user can't fix. Case-insensitive matching at the catalog boundary eliminates this entire class of problem.

This matches the CSS/HTML model (properties case-insensitive, selectors case-sensitive) that web-adjacent users and LLMs already understand. VHDL, the closest hardware-description language, is fully case-insensitive.

**Implementation:**

1. Normalize to lowercase in `tag_category()`, `are_connectors_compatible()`, `are_protocols_compatible()` — add `.to_ascii_lowercase()` at the comparison boundary.
2. Add `Analog` / `Analogue` to the catalog as a known protocol tag (both currently missing).
3. Choose a canonical display form for `format_source()` output.
4. Identifiers remain exact-match — no normalization.

**Affects:** `crates/patchlang/src/drc/catalog.rs` — `tag_category()`, `are_connectors_compatible()`, `are_protocols_compatible()`, `CONNECTOR_MATES`.

**Related issues:** Hillsong MTG fixture 613→0 DRC error fix, stock library `Analogue` vs `analog` inconsistency

---

### D017 — Bus Output Syntax: Named Outputs with Optional Destinations
**2026-04-13** | **Decided**

**Question:** How should named bus outputs be represented in PatchLang? The frontend `InternalBusOutput.name` (a required string) was being silently dropped by the emitter — no syntax existed for it. What syntax and AST shape should replace the old `output: Port` form?

**Decision:** Output labels are **required**. Multi-destination outputs are supported via comma-separated port refs. Unrouted outputs (no destination) are valid. Old `output: Port` (unlabeled) syntax is removed.

```
bus Link_1 {
  input: Fader[1..8]
  output "Link 1-L": MADI_1_Out[1]              # labeled, single destination
  output "Link 1-R": MADI_1_Out[2], Dante[5]    # labeled, multi-destination
  output "Link 1-C"                              # labeled, unrouted
}
```

AST: `BusEntry.outputs` changes from `Vec<PortRef>` to `Vec<BusOutput>` where `BusOutput { label: String, destinations: Vec<PortRef> }`.

**Rejected alternative — optional labels:** Keeping labels optional would perpetuate the data-loss bug. The frontend `InternalBusOutput.name: string` is non-optional — every output always has a name in the UI. Making it optional in PatchLang creates a permanent class of round-trip data loss.

**Rejected alternative — `Option<PortRef>` instead of `Vec<PortRef>` for destinations:** The frontend `InternalBusOutput.destinations` is a Vec (one output can route to multiple ports). Using `Option<PortRef>` would cap destinations at one and lose data for multi-routed outputs.

**Rejected alternative — unified `BusPort` struct for both inputs and outputs:** `InternalBusInput` has no name field — inputs are bare channel references. Wrapping inputs in a named struct would add an invalid state (`Option<PortRef>` on inputs) that the domain doesn't support. Asymmetry is correct here.

**Rationale:** Determined via Socratic debate (4 perspectives) + review of frontend `internalRouting.ts` and `emitterBuilder.ts`. The emitter comment `// KNOWN LIMITATION (C6): PatchLang InstanceBusDecl.outputs is PortRef[]. Named outputs with zero destinations are silently dropped` confirmed the exact problem. No backward compat needed — language not yet deployed to users.

**Also decided (Gap 2 — bus display names):** The `label: "..."` body key is retained as-is (parser already reads it). The formatter is fixed to emit it. No grammar change to the `bus-entry` production. No inline syntax (`bus PQMM "PQ>MM" {` was considered and rejected — body form is consistent with `config` block label style).

**Spec:** `docs/superpowers/specs/2026-04-13-bus-named-outputs-design.md`
**Ticket:** ByteBard97/SignalCanvasLang#9

---

### D018 — IT Infrastructure Scope: Deferred, Not Foreclosed
**2026-06-16** | **Decided**

**Question:** Should PatchLang model IT network infrastructure — Ethernet switches, VLANs, port membership — to enable path-diversity DRC (e.g., verifying that primary and secondary Dante paths traverse separate physical switches)?

**Decision:** Out of scope for now. The existing `network` construct (D011 precedent: metadata over keywords) is the right shape if scope expands, but no switch/VLAN model is added at this time.

**Rejected alternative:** A thin `infrastructure` layer with switch templates and port membership — enabling the SMPTE 2022-7 path-diversity check (redundant paths through separate physical switches). Deferred to a future opt-in annotation on `connect` if/when the product pushes seriously into broadcast/ST 2110 workflows.

**Rationale:**
1. **Product persona.** SignalCanvas targets AV engineers, not IT/network engineers. Dante and IP audio protocols are modeled as logical virtual networks (D001) — adding switch topology would expand scope into NetBox territory and attract a different user profile.
2. **The specific DRC benefit is narrow.** The one high-value check enabled by IT modeling (verifying ST 2022-7 path diversity) can be captured later as a thin, opt-in `path_diversity: true` annotation on primary `connect` statements — no switch model required. Adding it only if/when broadcast deployments demand it is the YAGNI-correct call.
3. **Competitive scope.** Pitting SignalCanvas against NetBox at launch is a losing position. The moat is AV signal flow, not network topology.
4. **No foreclosure.** The D002 metadata approach (`redundant_cable:` on connect) and the `network` construct already anticipate this direction. A future annotation-on-`connect` fits cleanly without breaking existing files.

**Affects:** `debate-context.md` (Decisions Already Made section), `overview.md` scope note.

**Related issues:** ByteBard97/SignalCanvasLang#22

---

### D019 — Signal-Trace Reachability DRC (T01/T02)
**2026-06-16** | **Decided**

**Question:** What should a signal-trace completeness DRC rule check, and at what severity? When origin port has connections but the trace never escapes to an output port, how should that be reported?

**Decision:** Two Warning-severity rules under a new `Trace` DRC layer:

- **T01 — Origin not connected:** Signal has `origin:` declared but the origin port has zero outgoing edges in the directed signal-flow graph (no connects, bridges, or template-internal bridges leading FROM that port). The signal is completely dead at its source.
- **T02 — Cannot reach output port:** Signal has outgoing edges from origin, but the BFS/DFS traversal visits no port with direction `Out` or `Io` (including origin itself). The signal flows somewhere but never exits any device via an output port.

Both are `Severity::Warning`. A third sub-rule ("trace terminates at a non-output port" — all terminal leaf nodes in the graph are `In` ports even if `Out`/`Io` ports were visited along the way) is **deferred**: it false-positives on valid fixtures where a consumer device's `In` port IS the intended final destination (e.g., `FOH_Console.Dante_Pri_In` in worship-venue).

**Directed graph construction:** Edges are collected from (a) top-level connects and link-groups, (b) top-level bridges and bridge-groups, (c) template-internal bridges applied to each instance (where `TemplateDecl.bridges` are expanded per instance), (d) instance routes. Channel indices are ignored — only port names are tracked, because channel-level precision belongs to the structural rules (S06/S14/S15).

**Skip conditions:**
- Signal has no `origin:` → skip silently (signals without origin are purely documentary).
- Origin references an unknown instance or port → skip silently (S08/S09 already fired).

**Severity rationale:** Warning, not Error. A completeness check can legitimately false-positive on systems where intra-device routing is partially modeled (especially subsystem templates). Blocking compilation on an incomplete trace model would be too aggressive; a non-blocking Warning matches D006 ("silently wrong is worst") while remaining overridable.

**`@suppress` support:** Deferred. `SignalDecl` has no `suppressions` field; adding suppress support requires an AST/parser change. The Trace layer will be added to the suppress vocabulary in that future change.

**Multi-file scope:** The DRC already runs on the merged program after multi-file resolution, so multi-file coverage is automatic.

**Rejected alternatives:**
- Error severity — too strict for a model that may intentionally omit intermediate device internals.
- "Trace terminates at non-output port" as an immediate check — false-positives on valid fixtures; deferred until a fixture requires it.
- Implementing only at the graph layer (reusing `compile_to_graph`) — DRC runs on the AST before graph compilation; duplicating graph traversal in the AST layer is acceptable and keeps the dependency clean.

**Affects:** `drc/trace.rs` (new), `drc/types.rs` (new `Trace` layer), `drc/mod.rs`, `language-reference.md` DRC table, `SKILL.md`.

**Related issues:** ByteBard97/SignalCanvasLang#18

---

### D020 — Network Block: Principle 7 Carve-Out
**2026-06-16** | **Decided**

**Question:** Should cross-network connect validation be moved into the compiler (extend N-series DRC rules beyond N01), or should `network` receive an explicit Principle 7 carve-out documenting it as an intentional exception?

**Decision:** Principle 7 carve-out. `network` is L2 membership documentation; cross-network topology warnings require switch-level context that PatchLang deliberately does not model. N01 (unknown instance reference) remains the only Network-layer DRC rule. The compiler emits structured `network_membership` data in its output so the UI and backend can produce topology-aware warnings without the compiler speculating about physical fabric topology.

**Why the carve-out is correct:**
The compiler can verify that declared `network` members exist (N01). It cannot verify that two connected ports are truly on separate switched fabrics, because:
1. A device absent from a `network` block is not necessarily disconnected from that fabric — it may be undeclared, multi-homed, or reachable via a trunk port. The compiler cannot distinguish these cases.
2. The only check the compiler *could* do — "warn when the intersection of two devices' network membership sets is empty" — produces false positives on partial declarations, which are the common case in incrementally-documented rigs.
3. A false-positive DRC warning trains engineers to ignore warnings, degrading the entire DRC system's credibility.

**Why this doesn't violate Principle 7:**
Principle 7 prohibits duplicating validation logic across layers. It does not require the compiler to emit warnings it cannot make reliably. The correct reading is "the compiler validates everything it can validate with the information it has." Cross-network topology warnings require switch-level context (physical switch adjacency, VLAN membership) that PatchLang deliberately does not model (D018). This is the same reasoning that prevented adding switch topology in D018 — the carve-out makes that boundary explicit for the `network` construct.

**What the compiler does provide:**
The `network_membership` data (which devices belong to which named networks) is already present in the AST and will flow through to the compiler's JSON output. The UI layer reads this to visually group devices by network and can surface cross-network connect warnings with the richer context it has (e.g., knowing which devices the user has actually configured, whether a connection crosses VLANs).

**Deferred N02:**
One narrow compiler-checkable rule is deferred: N02 — "a port appears as a member of two named networks of the same protocol on the same device that are declared mutually exclusive." This is a well-defined contradiction the compiler can detect without topology context. Deferred until a real fixture requires it.

**Rejected alternative:** Move validation into the compiler (extend N-series rules). Rejected because any rule the compiler can write without switch topology data fires false positives on partial declarations, multi-homed devices, and undeclared devices — all common in real rigs.

**Affects:** `language-reference.md` (network section, DRC table), `overview.md` (Principle 7 note).

**Related issues:** ByteBard97/SignalCanvasLang#19

---

### D021 — Minimal Template Versioning
**2026-06-16** | **Decided**

**Question:** What version pinning and resolution must be designed before the stock device library can be opened to community contribution?

**Decision:** The `@version` annotation already exists on template declarations. The `dependencies` field in `project.json` adopts SemVer constraints. The compiler resolves "latest version satisfying all constraints" from the backend library tier system. Breaking changes (port schema changes) require a major version bump.

**Version annotation on templates:**
```
template Rio3224 @version("2.1.0") {
  ...
}
```
The `@version` string is a SemVer string. The compiler validates the format but does not enforce it — version enforcement happens at dependency resolution time.

**Project manifest pinning:**
```json
{
  "dependencies": {
    "@stock/yamaha": "^2.0.0",
    "@stock/shure-wireless": "^1.0.0"
  }
}
```
Keys are package names in the `@tier/package` namespace. Values are SemVer constraint strings (caret, tilde, exact, range — same semantics as npm).

**What constitutes a breaking change (major version bump required):**
- Renaming a port
- Removing a port
- Changing a port's direction
- Changing a port's channel count (declared range)

Adding a new port, changing a port's connector type, or updating metadata is non-breaking (minor/patch).

**Resolution rules:**
1. Local `use` resolution takes precedence over dependency constraints — if a template resolves locally, the dependency pin is not checked for that template.
2. The compiler resolves the latest version of each package satisfying all stated constraints from the backend library tier.
3. If two constraints for the same package are incompatible (e.g., `^1.0.0` and `^2.0.0`), the compiler errors with the conflicting constraints and their sources.

**What is deferred:**
- Lock files (`project.lock`) — deterministic reproducible builds. Deferred: the backend serves the same resolution for a given constraint, so drift is bounded.
- Private registry hosting — the backend hosts the library tiers; registry tooling (like npm publish) is not yet designed.
- Scoped namespace conflicts — two packages providing a template with the same name. Deferred until namespace collision is observed in practice.

**Affects:** `project-structure.md` (`dependencies` field documentation), `project.json` schema.

**Related issues:** ByteBard97/SignalCanvasLang#24

---

### D022 — Library Open-Sourcing Split and License
**2026-06-16** | **Decided**

**Question:** Under what license should the stock device library be open-sourced, and what is the tier architecture?

**Decision:** The stock device library is released under **ODbL (Open Database License)**. Verified and private library tiers remain closed. Creative content within the library (descriptions, documentation, artwork) is separately licensable under CC-BY-SA 4.0.

**Tier architecture:**

| Tier | Who creates it | License | Availability |
|------|---------------|---------|--------------|
| `@stock` | Community + SignalCanvas | ODbL | Public, free |
| `@verified` | SignalCanvas-curated | Proprietary | SignalCanvas subscribers |
| `@org` / `@user` | Organization / individual | Private | Owner only |

**Why ODbL over CC-BY-SA:**
Device templates are structured factual data (port names, counts, directions, connector types, protocols). This is a database, not a creative work. Key differences:
1. US copyright law provides thin or no protection for factual data (*Feist v. Rural Telephone*). CC-BY-SA relies on copyright; it may be unenforceable against someone who copies only facts.
2. ODbL explicitly invokes both copyright and the EU database right (sui generis), closing the US factual-data gap by treating the database itself as the protected object.
3. ODbL's share-alike clause targets the *database as a whole* — a competitor who extracts the device library and ships it in a proprietary product must open-source the derived database. CC-BY-SA would not reliably achieve this for factual data.
4. ODbL distinguishes "produced works" (rendered outputs like a canvas diagram) from the database itself — proprietary products can consume the library to produce outputs without licensing the output, which is the correct permission model for downstream tooling.
5. OpenStreetMap uses ODbL for exactly this reason — geographic data is largely factual.

**What stays closed:**
- Probe device-identification intelligence (the real competitive moat — how SignalCanvas identifies unknown devices)
- The `@verified` tier (the trust and curation moat)
- All `@org` and `@user` private libraries

**Community contribution:**
Contributors to `@stock` grant their contribution under ODbL. A plain-language CONTRIBUTING.md note will explain: "You're licensing the dataset, not individual files. Creative content like descriptions and artwork can be noted separately as CC-BY-SA 4.0."

**Affects:** Library repo, `project-structure.md` (library tiers section), future CONTRIBUTING.md.

**Related issues:** ByteBard97/SignalCanvasLang#26

---

### D023 — Insert Send/Return Representation and Endpoint Resolution
**2026-08-01** | **Decided**

**Question:** How should channel and bus inserts be represented in PatchLang, and how should an insert endpoint's port be resolved on the emit side?

**Decision:** Inserts become first-class `.patch` data with **two different syntaxes**, and endpoints resolve **in Rust** via the existing `resolve_route_endpoint` path.

**Why an insert belongs in `.patch` at all:** an insert breaks a channel or bus out to an external processor and back. The signal physically leaves the console, so the send/return endpoints are real signal flow — not presentation. They were sidecar-only because the canvas DTO could not round-trip them, not because they belonged there.

**Why two syntaxes.** The asymmetry is forced by what each carrier can hold, not by preference:

| Carrier | Syntax | Reason |
|---------|--------|--------|
| Channel label | `insert_send: "Ext_Out[3], Ext_Out[10]"` (quoted string) | A label property value holds at most one `KvValue::PortRef`; a leg list is inherently multi-valued. Extending kv parsing to comma-lists would change property syntax globally. |
| Bus | `insert_send: Ext_Out[3], Ext_Out[10]` (native grammar) | `BusEntry` has no property block at all, so there is nothing to encode into. |

A grammar-native `insert Mic_In[1] send: ... return: ...` statement was considered and rejected: the property-bag route is the ticket's own proposal and is far cheaper.

**Leg semantics.** Each list is **ordered** — `[L]` mono, `[L, R]` stereo; reversing it swaps the stereo image. Legs are **independent**: no adjacency and no equal-width constraint, so a send on MADI 3 and 10 returning on 4 and 8 is valid, and the two lists may differ in length. Unlike bus `input:` entries — which are set-like and therefore grouped by `(instance, port)` and channel-unioned — insert legs are never grouped, unioned, or deduplicated, and a port may legitimately repeat. A range index in a leg is rejected rather than expanded; expanding `[1..2]` would silently double the insert's width. An insert is a **detour, not a destination**: legs never appear among the bus's inputs or outputs.

**Endpoint resolution — the substantive fork.** The frontend sends `{ interfaceId, channel }`; PatchLang needs a template port name. Two conventions already existed in `canvas_emit/`: channel labels and instance routes resolve in Rust via `find_interface(..).map(directional_port_name).unwrap_or_else(sanitize_id)` (`routes.rs:87-89`), while bus inputs/outputs assumed TypeScript pre-resolved and only sanitized.

Inserts follow the **Rust-side resolution** convention. The deciding fact is `should_split_io`: one io/asymmetric interface expands into **two** template ports (`{base}_In` / `{base}_Out`) for every non-ring/bus protocol, MADI included. The canonical insert case sends and returns on the *same* interface, so the legs must emit as `MADI_Out[3]` and `MADI_In[4]`. Only the side distinguishes them, and the side is known solely from which list the leg sits in — the emitter has it, TypeScript does not. Sanitizing alone emits a bare `MADI[3]` that the template never declares, which the loader then rejects and silently drops.

Two further reasons: this reduces the number of conventions from two to one rather than adding a third, and it keeps the io-splitting rule in one place instead of requiring a third copy of `is_ring_bus_protocol` in TypeScript (DRY, rule 4).

The `unwrap_or_else(sanitize_id)` fallback is load-bearing and retained: already-resolved port names and slot-qualified `__` compounds match no interface and must pass through untouched. Instance-qualified legs resolve against the named instance's own interfaces, matching route behaviour.

**Lossy-boundary fixes shipped alongside:**
- `ChannelLabelOutput` gained a verbatim `properties` bag. Any label key without a dedicated field was silently dropped on load — the reason `stand`/`gain` also needed the sidecar. A key is carried by the typed field **or** the bag, never both: a clean insert parse claims the key, a malformed one leaves the raw string in the bag so re-emit stays byte-faithful. This deliberately differs from `ConnectionLoadOutput::properties` (D-adjacent, v0.3.1), which keeps duplicates — connect's dedicated fields are a lossless re-read, whereas the insert typed field is a lossy parse.
- `kv_map` returned `None` for `KvValue::PortRef`, so an unquoted `insert_send: Ext_Out[3]` parsed cleanly and then vanished at the DTO boundary. Now stringified, matching `graph::kv_to_string_map`. Affected all label properties, not just inserts.

**No DRC.** A "send count ≠ return count" rule would contradict the stated independence of legs.

**Compatibility.** On v0.3.1 and earlier, an `insert_send:` line inside `bus { }` hits `parse_bus_entry`'s catch-all `_ => { self.advance(); continue; }` and is silently skipped — no error, data lost. Tools emitting this syntax must require v0.3.2+. All new deserialized fields are `#[serde(default)]`; `patchlang-wasm`'s `add_bus`/`update_bus` deserialize `ast::BusEntry` directly from frontend JSON, where a missing field would otherwise reject the entire payload.

**Known follow-up:** `patchlang-python`'s `set_label` hard-codes `HashMap::new()`, so the Python binding forwards no label properties at all. Out of scope here; filed separately.

**Affects:** `SPEC.md` (§3.10 config), `docs/language-reference.md` (bus entry), `builder/insert_endpoints.rs`, `builder/canvas_{input,output,load}.rs`, `canvas_emit/{mod,routes}.rs`, `ast.rs`, `body_parser.rs`, `formatter_emit.rs`, `compat{,_types}.rs`.

**Related issues:** ByteBard97/SignalCanvasLang#31
