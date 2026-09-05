---
layout: default
title: Debate Context
permalink: /debate-context/
---

# SignalCanvas & PatchLang: Debate Context Brief

This document exists to give AI agents sufficient context to participate meaningfully in design debates about PatchLang. Read this in full before arguing any position.

---

## What Is SignalCanvas?

SignalCanvas is a signal flow documentation tool for broadcast and live production engineers. The tagline: **"Figma meets a signal flow database."** It is an infinite-canvas documentation tool, not a drawing tool. It is explicitly a **compiler with a visual front-end**. The `.patch` text file is the source of truth; the canvas is a projection of it.

The target user is a **broadcast/AV engineer**: someone who designs, installs, and operates audio/video systems for concerts, broadcast facilities, houses of worship, and live events. They understand signal flow deeply but are not software engineers. They are not IT/network engineers. They work under time pressure (shows have curtain times) and need documentation that is fast to read and unambiguous.

The reference project is `MTG.patch`, a Hillsong mega-church production system, in real production use. This sets the scale: multiple stageboxes, consoles, Dante networks, IEM systems, stage racks.

---

## Killer Features (Why This Product Exists)

### 1. Signal Tracing: Multi-Hop Path Resolution
**This is the #1 killer feature.** In a large broadcast or live production system, tracing a signal from source to destination requires hunting through many Excel spreadsheets one at a time: mic → stagebox patch → Dante subscription → console input → DSP routing → mix bus → output → IEM transmitter → RF → earpiece. This can involve 8–12 hops across separate documents.

SignalCanvas eliminates this. A signal declared with the `signal` keyword propagates through `bridge` and `connect` chains. The Signal Trace tool searches a signal name and highlights its full path across all devices, cables, and network hops in one view.

**`bridge` is the mechanism that carries signal annotations across internal device boundaries. This means the semantic definition of `bridge` is load-bearing for the entire Signal Trace feature.**

### 2. Hardware Audit (SignalCanvasProbe, MVP)
The Probe component ("Is my rig actually patched the way my diagram says it is?") scans the LAN and compares live device state to the `.patch` file. Output: matched, drifted (config differs), missing (in `.patch` but not on LAN), unknown (on LAN but not in `.patch`).

Probe generates `.patch` stubs from discovered devices. Every `ProbeDevice` maps to `template` + `instance` + `connect` + `config`. **Probe is the most important downstream consumer of PatchLang's expressiveness: if PatchLang can't express something Probe discovers, data is lost.**

### 3. Configuration Push (SignalCanvasProbe, v2)
The planned v2 feature: use the `.patch` file as the source of truth and push routing configuration to live hardware via Dante, AES70, NMOS IS-05, Ember+, Q-SYS. **The `.patch` file becomes the control surface for the entire network.**

This means `route` inside an instance is not just documentation, it is eventually an instruction that gets executed on real hardware. The `connect` statements represent Dante subscriptions that could be pushed via the Dante API. This distinction is critical for language design: **keywords that represent configurable state must be clearly distinguishable from keywords that represent fixed hardware facts.**

### 4. Four-Layer DRC
The compiler validates connector compatibility, impedance/voltage, protocol matching, and clock domain conflicts. A diagram can contain errors nobody catches until the show. PatchLang catches them at compile time. **The DRC's reliability depends entirely on the semantic accuracy of what the file declares.**

---

## What Is PatchLang?

PatchLang is the DSL that powers SignalCanvas. Every system is described in a `.patch` file. The compiler (hand-written recursive descent Rust with Logos lexer) reads these files, validates them, and produces a graph for the frontend.

A `.patch` file describes:
1. **Templates**: device types (a Yamaha CL5 console, a Dante stagebox, a video router)
2. **Instances**: physical devices in a specific installation
3. **Connections**: cables, signal paths inside devices, and logical signal flow annotations

### The Three Core Signal Primitives

| Keyword | What it means | Physical medium | Scope | Probe generates this from |
|---------|--------------|----------------|-------|--------------------------|
| `connect` | Physical cable between two ports | Real cable, can be touched | Between instances | `ProbeRoute` structs (Dante subscriptions, NMOS flows) |
| `bridge` | Logical signal mapping, no physical medium implied | None required | Inside template OR between instances at top level | TBD, this is under debate |
| `route` | Current operator-configured internal routing state | None | Inside instance body only | Ember+ matrix crosspoints, Q-SYS mixer routing, console DSP patching |

**All three are traversed by Signal Trace.** All three are generated by Probe. All three must remain semantically distinct.

### Other Keywords

| Keyword | What it does |
|---------|-------------|
| `template` | Defines a device type: ports, internal paths, slot bays |
| `instance` | Declares a physical device in the installation |
| `ring` | Declares a ring/bus network (OptoCore, AVB, etc.) |
| `bridge_group` | Sequential channel-fill bridge pattern |
| `link_group` | Multi-cable logical unit (e.g., quad-link SDI) |
| `config` | Channel labels and metadata (maps to `config` labels discovered by Probe) |
| `signal` | Names a logical signal for multi-hop tracing |
| `flag` | Named status indicator (e.g., `Genlock_OK`) |
| `stream` | Dante virtual channel group |
| `bus` | Named mix bus inside a device. Optional `label:` property carries the human-readable display name (may contain `>`, `-`, spaces). |
| `slot` | Expansion card bay declaration |

---

## The Design Rule Checker (DRC)

The DRC runs after parsing and validates:
- **Direction:** out→out, in→in connections
- **Mechanical:** connector mismatches (XLR into BNC)
- **Electrical:** level mismatches (mic into line level)
- **Logical:** protocol mismatches (Dante → SDI)
- **Temporal:** clock domain mismatches
- **Ring:** ring topology member validation
- **Convention:** advisory warnings

**The DRC's reliability depends entirely on semantic accuracy of what the file declares.** A misclassified `bridge` does not produce a compiler error, it produces confidently wrong DRC output, which is more dangerous than no DRC.

---

## Design Principles

1. **Human-readable first.** An engineer should be able to read a `.patch` file and understand the signal chain without tooling.
2. **LLM-friendly.** Simple enough that AI agents can generate valid `.patch` files from plain English descriptions. Fewer keywords = fewer mistakes.
3. **Git-diffable.** Text diffs show meaningful changes. Adding a mic input is one line.
4. **No ambiguity.** Every statement starts with a unique keyword. Grammar is LL(1).
5. **Domain-specific.** The language models broadcast concepts directly, not through generic data structures.
6. **Single validation layer.** The Rust compiler validates everything. Frontend and backend never reimplement validation.
7. **Self-contained.** `.patch` files must work without the backend.

---

## Protocols and What They Map To

### Has LAN control plane (Probe can query/push)

| Protocol | What Probe does | PatchLang mapping |
|---|---|---|
| **Dante** | Reads subscriptions (routing), channel names, clock master. v2: push subscriptions | `connect` (subscriptions), `config` (channel names), `bridge` (internal mic→Dante paths) |
| **AES67** | Reads SDP stream descriptors, read-only | `connect`, `bridge` |
| **NMOS IS-04/05** | Nodes/Devices/Senders/Receivers, active routing via IS-05 | `connect`, `bridge` |
| **Ember+** | Full parameter tree, matrix crosspoints, gain, routing. v2: push crosspoints | `route` (crosspoints), `config` (channel names), `template` (device identity) |
| **Q-SYS QRC** | Named controls, component tree, mixer routing | `route`, `bus`, `instance` |
| **AES70 / OCA** | Manufacturer, model, gain objects, mute, matrix crosspoints | `template` (identity), `config` (labels), `route` (crosspoints) |
| **Yamaha SCP** | Channel names, patching, Dante routing | `config`, `route` |
| **Behringer OSC** | Channel names, routing, fader state | `config`, `route` |
| **Allen & Heath MIDI-over-TCP** | Channel names, routing, scene state | `config`, `route` |

### No LAN control plane (manual documentation only, always)

| Protocol | PatchLang mapping |
|---|---|
| **OptoCore** | `io` ports + `ring` keyword |
| **TWINLANe** | `io` ports + `ring` keyword |
| **GigaACE** | `io` ports |
| **MADI** | `in`/`out` ports, manual only |
| **AES50** | Manual only |
| **SDI / HD-SDI / 3G / 12G** | `in`/`out` ports, manual only |
| **Analogue / AES3** | `in`/`out` ports, XLR/BNC connectors |
| **WordClock** | `io` management port (direction under debate) |
| **AVB/Milan** | `io` ring/bus (direction under debate) |

---

## What Is In Scope vs. Out of Scope

**In scope:**
- AV device signal flow (audio, video, data, clock sync)
- Physical cables between devices
- Internal signal paths within devices
- Network protocols used as audio/video transport
- Ring/bus protocols
- Expansion card installation
- Channel labeling and configuration
- Multi-file project structure
- Hardware audit (Probe)
- Configuration push (Probe v2)

**Out of scope (deliberately):**
- IT network infrastructure: Ethernet switches, VLANs, switch ports, network topology. Dante and other IP protocols are modeled as logical virtual networks.
- The target user is an AV engineer, not a network engineer.

---

## Decisions Already Made

These are final. Do not re-litigate them.

**D001, IT infrastructure is out of scope.** Switches, VLANs, network topology are not modeled.

**D002, Dante secondary redundancy is annotation metadata.** `redundant_cable:` on the primary connect. No `Dante_Sec_In`/`Dante_Sec_Out` ports in templates.

**D003/D008, WordClock port direction.** Decided: split `in`/`out`. WordClock uses separate physical 75Ω BNC connectors, never bidirectional. Devices always-master declare only `WordClock_Out`; always-slave declare only `WordClock_In`; both if configurable.

**D004, AVB/Milan port direction.** Still pending. Current spec uses `io`.

**IO direction model (decided):** Channel-based protocols (Dante, AES67, MADI, Analogue, AES3, SDI, SoundGrid, NDI, SMPTE2110) and WordClock → explicit `in` + `out` lines. Ring/bus (OptoCore, TWINLANe, AVB/Milan, GigaACE) + management (Ethernet_Mgmt) → `io`.

**No `card` keyword.** Cards are templates with `meta { kind: "card", fits: "FormatName" }`.

**D011, Template classification uses `kind` meta field, not new keywords.** `device_type` is renamed to `kind`. Values: `device`, `card`, `fixed-converter`, `stage-core`, `mic-di`, `mic-splitter`, `rf-system`, `system`, `venue`. `device_type` accepted as deprecated alias during migration.

**Ring members use explicit port references in emitted code.** `member Console.OptoCore_A` not `member Console`.

**Slot assignments use bare identifiers.** `slot MY_Slot[1]: MY16_AUD`, not quoted strings.

**`::` ID separator.** `pl::CL5::Dante_In_1`. Not reversible, changing it is a migration.

**Bidirectional cables = two `connect` statements.** One per direction. Same `cable:` metadata on both.

**D012, Backbone connections use `backbone: true` on `connect`.** Surface-to-engine links (GigaACE, RIVAGE, etc.) are expressed as `connect` with `backbone: true`. No new keyword, no implicit protocol detection. Signal Trace traverses transparently; DRC exempts from direction/protocol checks. Dual redundant = 4 connect statements (2 directions × 2 cables).

**D013, AES67 interop uses existing constructs + metadata.** AES67 TX streams use `stream` with `protocol: "AES67"`. Chipset awareness via `dante_chipset` meta field on templates (Ultimo, Broadway, Brooklyn_II, Brooklyn_3, HC). Instance properties: `aes67_mode: true`, `multicast_prefix: 71`. DRC Flow layer: F01 (flow slot exhaustion), F02 (8-channel limit), F03 (multicast prefix mismatch). C05 (redundancy terminates at AES67 boundary). No parser changes.

---

## Key Vocabulary

| Term | Meaning |
|------|---------|
| **FOH** | Front of House, where the main mixing engineer sits |
| **Stagebox / IO Box** | A box on stage converting mic/line to network audio (e.g., Dante) |
| **Console** | The mixing desk (e.g., Yamaha CL5, DiGiCo SD12) |
| **Dante** | Popular IP audio networking protocol (Audinate). Over standard Ethernet. 512 channels/device. |
| **MADI** | Digital audio protocol, 64 channels over coax or fiber. Point-to-point. |
| **OptoCore** | Proprietary fiber ring protocol used by DiGiCo consoles |
| **Ember+** | Lawo broadcast control protocol, parameter tree, matrix crosspoints |
| **AES70 / OCA** | Open Control Architecture, device parameter control standard |
| **NMOS** | AMWA broadcast media standard, IS-04 discovery, IS-05 routing, IS-08 channel mapping |
| **Q-SYS** | QSC networked audio/video/control platform |
| **WordClock** | Clock reference signal (48kHz square wave, 75Ω BNC) for synchronizing digital audio |
| **Genlock** | Video sync reference (BlackBurst or Tri-Level) for synchronizing cameras |
| **DRC** | Design Rule Checker, the compiler's validation engine |
| **Template** | A device type definition (like a class in OOP) |
| **Instance** | A physical device in a specific installation (like an object instance) |
| **Probe** | The SignalCanvas component that discovers and audits live hardware on the LAN |
| **Signal Trace** | The feature that follows a named signal through all hops from source to sink |
