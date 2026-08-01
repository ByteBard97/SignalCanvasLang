---
name: patchlang
description: >
  PatchLang v0.3.2 language reference for writing, editing, and migrating .patch files.
  Use this skill whenever you are writing or editing a .patch file, converting a hardware
  datasheet into PatchLang, updating legacy PatchLang syntax to the current version, or
  generating templates/instances/connections for SignalCanvas. Also use it when the user
  mentions PatchLang, patch files, signal flow documentation, or device templates even if
  they don't say "PatchLang" explicitly.
---

# PatchLang v0.3.2

You are writing **PatchLang v0.3.2**. The language describes broadcast/AV signal flow:
templates define device types, instances are physical devices, and connections are cables.

When you need annotated examples (Dante systems, ring networks, slot/cards, composition),
read `references/examples.md`. Load it any time you're building something non-trivial or
converting a datasheet.

---

## Critical Rules

These are the mistakes Claude most often makes. Read them before writing anything.

**1. Split directional ports — never `io` for channel protocols or WordClock.**
Dante, MADI, AES67, AES3, SDI, Analogue, SoundGrid, NDI, SMPTE2110, and WordClock each need two lines:
```
Dante_Pri_In[1..32]:  in(etherCON)  [Dante, primary]
Dante_Pri_Out[1..32]: out(etherCON) [Dante, primary]

WordClock_In:  in(BNC_75)  [WordClock]
WordClock_Out: out(BNC_75) [WordClock]
```
WordClock uses separate physical BNC connectors — it is never bidirectional. Devices that are always
clock masters declare only `WordClock_Out`; always-slaves declare only `WordClock_In`; devices that
can be either (e.g., a console) declare both.

`io` is only for ring/bus protocols (OptoCore, TWINLANe, AVB/Milan, GigaACE) and
management ports (Ethernet_Mgmt).

**2. Two connects per bidirectional cable.**
Every cable that carries signal in both directions needs two `connect` statements — one
per direction. Same cable metadata on both. This is not optional:
```
connect A.Dante_Pri_Out -> B.Dante_Pri_In { cable: "Cat6a_01" }
connect B.Dante_Pri_Out -> A.Dante_Pri_In { cable: "Cat6a_01" }
```

**3. `bridge` is for manufacturer-hardwired paths only. `route` is for operator-configured routing.**
Inside a template, `bridge Mic_In -> Dante_Pri_Out` means "the manufacturer hardwired this path — it exists in every unit of this device regardless of software configuration." Use `bridge` only when you can say: "every unit that ships from the factory has this path, and no operator action can remove it."

Operator-configurable routing (console DSP assignments, matrix crosspoints, mix bus patching) belongs in the **instance body** as `route`, not in the template as `bridge`.

```
# Correct — stagebox mic preamp is hardwired to Dante by the manufacturer
template Rio1608 {
  bridge Mic_In -> Dante_Pri_Out   # hardwired, correct
}

# Correct — CL5 has no manufacturer-hardwired signal paths; routing is all software
template CL5 {
  # no bridge declarations — all routing documented as route in instances
}

# Correct — routing state documented in the instance
instance FOH_Console is CL5 {
  route Dante_In[1] -> Fader[1]
  route Dante_In[2] -> Fader[2]
}
```

The bridge target is where the signal is *going*, not where it comes from.

**4. No hyphens in identifiers — use underscores.**
`FOH_Console` is valid. `FOH-Console` is a parse error.

**5. `card` is not a keyword.**
Cards are regular templates with `meta { kind: "card", fits: "SlotFormat" }`.
There is no `card` declaration syntax.

The `kind` field classifies a template's role. Valid values:

| Kind | Meaning |
|------|---------|
| `device` | Physical hardware (default when `kind` is absent) |
| `card` | Expansion card — requires `fits` |
| `fixed-converter` | Deterministic routing device (stagebox, bridge) |
| `stage-core` | Passive XLR loom/snake |
| `mic-di` | Single microphone or DI box |
| `mic-splitter` | Multi-way analogue signal splitter |
| `rf-system` | Wireless mic receiver, IEM transmitter |
| `system` | Logical grouping of devices (room, rack, subsystem) |
| `venue` | Top-level facility or building |

`device_type` is a deprecated alias for `kind` — the compiler accepts it but emits an M-I02 warning. Use `kind` in all new files.

**6. Slot assignments use bare identifiers, not quoted strings.**
```
slot MY_Slot[1]: MY16_AUD       # correct
slot MY_Slot[1]: "MY16_AUD"     # wrong (legacy — do not emit)
```

**7. Ring members must use explicit port references in emitted code.**
```
member Console.OptoCore_A       # correct — explicit, survives device changes
member Console                  # implicit — fragile if device gains a second ring port
```
The implicit form is valid source syntax; the emitter must always produce the explicit form.
R04 fires when implicit resolution is ambiguous (zero or multiple matching ports).

**8. Config labels reference split port names.**
```
config FOH_Console {
  label Dante_Pri_In[1]: "Lead Vocal"   # correct — uses the directional name
}
```

**9. ID format uses `::` separator.**
Port IDs: `pl::TemplateName::PortName`. Route IDs: `rule::template::src::dst`.
The old `pl_TemplateName_PortName` underscore format is legacy — do not generate it.

**10. Keywords cannot be template/instance/port names.**
`template`, `instance`, `is`, `connect`, `bridge`, `bridge_group`, `link_group`,
`signal`, `flag`, `stream`, `config`, `ports`, `meta`, `in`, `out`, `io`, `ring`, `member`
are all reserved. `card` and `auto` (outside brackets) are valid identifiers.
`for`, `over`, `generate` are reserved for future parametric generation — no grammar exists yet.

**11. Template naming is required (not advisory) in shared libraries.**
Model numbers and manufacturer-prefixed names: `CL5`, `Rio3224`, `Neutrik_Patch_Bay`. Generic
names (`Patch_Bay`, `Power_Amp`) are only acceptable in project-local templates never published.
Violations cause namespace collisions in multi-file projects.

**12. S15 (range size mismatch) is a hard error.**
Left and right channel counts on a `connect` must match. Suppress with `@suppress(structural)`
only when an intentional partial connect is documented.

**13. `[auto]` has four distinct error codes.**
A01: used in `route`/`bus` (not allowed). A02: both sides use `[auto]`. A03: scalar port or
cannot infer count. A04: overflowed port range. A05: enough channels exist but no contiguous block.

**14. Three channel mechanisms — use the right one.**
- **`[1..32]`** on a port declaration: declares how many channels the port has. Not for connects.
- **`[auto]`** on one side of a `connect`/`bridge`: compiler fills the next N available channels. Use when daisy-chaining stageboxes into a console.
- **`mapping: "offset 16"`** or `"1->3, 2->4"` on a `connect`: explicit channel routing. Use for non-sequential mapping or when offset is fixed regardless of other connects.
- `mapping: "1:1"` is the default and can be omitted.
- `[auto]` vs `mapping: "offset N"`: prefer `[auto]` when the compiler can track the offset; use `mapping:` when the offset must be stated explicitly.

---

## Compact Syntax Reference

### Template

```
template TemplateName {
  meta {
    manufacturer: "Acme"
    model: "Model_X"
    category: "Console"         # Camera | Stagebox | Console | Router | IEM | Sync | etc.
    kind: "device"              # see kind table above; omit for plain hardware
    dante_chipset: "Brooklyn_II"  # optional: Ultimo | Broadway | Brooklyn_II | Brooklyn_3 | HC
  }
  ports {
    PortName[1..N]: direction(Connector) [attr1, attr2]
  }
  bridge LocalPort -> LocalPort  # logical signal path inside the template
  slot SlotName[1..N]: SlotFormat
  slot SlotName[1..N]: SlotFormat { direction: "any", channels: 16 }  # with body
}
```

#### Template parameters with defaults

```
template StageSplit(mic_count: 32, name: "Default") {
  ports {
    Mic_In[1..32]: in(XLR)
    Dante_Out[1..32]: out(etherCON) [Dante]
  }
}

instance SL_Split is StageSplit(mic_count: 16)
```

#### Template composition

Templates can contain nested `instance` and `connect` to represent rooms, racks, or systems.

```
template FOH_Rack {
  ports {
    Dante_In[1..64]: in(etherCON) [Dante]
    Dante_Out[1..64]: out(etherCON) [Dante]
  }
  instance Console is CL5
  instance Stagebox is Rio3224
  connect Stagebox.Dante_Pri_Out -> Console.Dante_Pri_In
}
```

### Instance

```
instance DeviceName is TemplateName {
  location: "Stage Left"
  ip: "192.168.1.10"
  slot SlotName[1]: CardTemplateName
  route Port_In[1] -> Port_Out[1]
  bus Main_LR {
    label: "SPOTIFY>FOH"        # optional display name — use when bus name is not human-readable
    input: Fader[1..8]
    insert_send: Ext_Out[3], Ext_Out[10]     # optional — external processor send
    insert_return: Ext_In[4], Ext_In[8]      # optional — and its return
    output: Matrix_Out[1..2]
  }
}

instance FOH_Console is CL5(mic_count: 48) @version(">=4.0") {
  location: "Front of House"
}
```

### Connect

```
connect Instance_A.Port_Out[1..4] -> Instance_B.Port_In[1..4] {
  cable: "Cable_Label"
  length: "30m"
  @suppress(logical)            # optional: suppress specific DRC layers
  mapping: "1:1"                # optional: channel mapping (see below)
}
```

#### Mapping property

```
mapping: "1:1"                    # sequential one-to-one (default)
mapping: "offset 16"              # shifted — dst channels start at 17
mapping: "1->3, 2->4, 3->1"      # explicit per-channel pairs
```

### Bridge (top-level)

```
bridge Instance_A.Port_Out[1..32] -> Instance_B.Port_In[1..32]
```

### Bridge Group

Sequential channel auto-fill across multiple sources:

```
bridge_group FOH.Dante_Pri_In {
  SL.Mic_In[1..4]     # maps to In[1..4]
  SR.Mic_In[1..4]     # maps to In[5..8]
}
```

### Link Group

Groups multiple connects as a logical unit (e.g., quad-link 4K SDI):

```
link_group Cam1_UHD {
  connect Cam1.SDI_Out[1] -> Router.SDI_In[1]
  connect Cam1.SDI_Out[2] -> Router.SDI_In[2]
  connect Cam1.SDI_Out[3] -> Router.SDI_In[3]
  connect Cam1.SDI_Out[4] -> Router.SDI_In[4]
  mode: "quad_link_4K"
}
```

### Ring

```
ring RingName {
  protocol: "OptoCore"          # or TWINLANe, AVB, Milan, GigaACE
  member Instance_A.Port_A
  member Instance_B.Port_A
  member Instance_C.Port_A
}
```

### Signal / Flag / Stream

```
signal Lead_Vocal {
  origin: Stagebox.Mic_In[1]
  channel: "1"
  description: "Worship leader vocal"
}

stream SL_Dante_Primary {
  source: Stagebox.Dante_Pri_Out
  channels: "32"
  protocol: "Dante"
}

flag Genlock_OK {
  description: "All cameras locked to house sync"
  severity: "warning"
}
```

### Config

```
config InstanceName {
  label Port_In[1]: "Channel Label" { phantom: "true" }
  label Port_In[2]: "Kick Drum"
  label Port_In[3]: "Lead Vocal" {
    insert_send: "Ext_Out[3], Ext_Out[10]"   # channel insert — QUOTED string
    insert_return: "Ext_In[4], Ext_In[8]"
  }
}
```

### Inserts (v0.3.2+)

An insert breaks a channel or bus out to an external processor and back. Because the
signal physically leaves the device it belongs in `.patch`, not the layout sidecar.

Two syntaxes, because of what each carrier can hold — this is not a style choice:

| Carrier | Syntax | Why |
|---------|--------|-----|
| Channel label | `insert_send: "Ext_Out[3], Ext_Out[10]"` — **quoted string** | a label property value holds only one port ref |
| Bus | `insert_send: Ext_Out[3], Ext_Out[10]` — **bare, no quotes** | a bus has no property block |

Rules for both:

- **Ordered.** One leg for mono, two for stereo (`[L, R]`). Reversing the list swaps
  the stereo image, so never sort or reorder them.
- **Independent legs.** No adjacency and no equal-width constraint — a send on 3 and 10
  returning on 4 and 8 is correct and common, and a send list may be a different length
  than its return list. Do not "tidy" scattered legs into a range.
- **One channel per leg.** Write `Ext_Out[1], Ext_Out[2]`, never `Ext_Out[1..2]`. On a
  bus a range is a parse error; on a label it silently fails to parse as an insert.
- **A detour, not a destination.** Insert legs are not bus inputs or outputs. Do not
  also add them to `input:` / `output`.
- **Quote the label form.** Unquoted on a label parses as a single port ref and can
  carry only one leg.

### Multi-file import

```
use yamaha { CL5, Rio3224 }    # selective
use shure.*                     # wildcard
use buildings.foh               # bare namespace → buildings/foh.patch
```

### Auto-assignment

```
connect Stage.Dante_Pri_Out[1..16] -> Console.Dante_Pri_In[auto]
# Compiler fills [auto] with next N available contiguous channels in declaration order.
# Do not use [auto] on both sides (A02). Do not use [auto] with route or bus (A01).
```
> **Note:** `[auto]` is specified in v0.2.5 but may not be in the current WASM build.
> Verify before using; fall back to explicit ranges if the parser rejects it.

### Port directions

| Direction | Use when |
|-----------|----------|
| `in(Connector)` | Signal enters the device |
| `out(Connector)` | Signal leaves the device |
| `io(Connector)` | Ring/bus protocols and management only (OptoCore, TWINLANe, AVB/Milan, GigaACE, Ethernet_Mgmt) |

### Common connectors

`XLR` `BNC_75` `BNC_50` `etherCON` `RJ45` `SFP` `LC_Fiber` `SC_Fiber`
`opticalCON_DUO` `opticalCON_QUAD` `HDMI` `USB` `TRS_14` `TRS_3` `DB25` `SpeakON` `SMA`

`opticalCON_DUO` (2-fiber) and `opticalCON_QUAD` (4-fiber) are Neutrik ruggedised fiber housings — used by SSL Live consoles for the Blacklight II / X-Light protocols. They mate themselves and the generic `opticalCON`, but DUO and QUAD cannot mate each other.

### Common attributes

`Dante` `primary` `secondary` `MADI` `AES3` `AES67` `SDI` `SMPTE2110`
`NDI` `Analogue` `OptoCore` `TWINLANe` `WordClock` `RF` `USB` `redundant`

---

## Migration Cheat Sheet

These patterns appear in pre-v0.2.0 files. Update them when you encounter them.

| Old (legacy) | New (v0.2.5) |
|---|---|
| `meta { device_type: "card" }` | `meta { kind: "card" }` — `device_type` is deprecated (M-I02) |
| `Dante_io[1..32]: io(etherCON) [Dante]` | Two lines: `Dante_In[1..32]: in(...)` + `Dante_Out[1..32]: out(...)` |
| Single `connect A.Dante -> B.Dante` | Two connects: forward path + return path |
| `bridge A.Dante_io -> B.Dante_io` | `bridge A.Mic_In -> A.Dante_Pri_Out` (inside template, directional) |
| `pl_CL5_Dante_In` (port ID) | `pl::CL5::Dante_In` |
| `slot MY_Slot[1]: "MY16_AUD"` (quoted) | `slot MY_Slot[1]: MY16_AUD` (bare identifier) |
| `member Console` (implicit ring member) | `member Console.OptoCore_A` (explicit) |
| `label Dante_io[1]: "Label"` | `label Dante_Pri_In[1]: "Label"` (directional port name) |
| `config label` referring to `io` port | Update to the split `in` port name |

---

## DRC Quick Reference

Full error code table:

| Code | Layer | Sev | Meaning | Fix |
|------|-------|-----|---------|-----|
| D01 | Direction | Error | `out→out` connection | Swap direction; check split port names |
| D02 | Direction | Error | `in→in` connection | Swap direction |
| D03 | Direction | Error | Direction violation (general) | Check port directions |
| S01 | Structural | Error | Instance references unknown template | Fix template name |
| S02 | Structural | Error | Slot assignment references unknown card template | Fix card template name |
| S03 | Structural | Error | Unknown port on instance | Port name typo, or using old `io` name after split migration |
| S04 | Structural | Error | Route references unknown port on instance (effective ports: template + card) | Fix port name in route |
| S05 | Structural | Error | Bus references unknown port on instance (effective ports: template + card) | Fix port name in bus |
| S06 | Structural | Error | Channel index out of declared range | Index exceeds `[1..N]` |
| S07 | Structural | Error | Config block references unknown instance | Fix instance name in config |
| S08 | Structural | Error | Signal origin references unknown instance | Fix instance name |
| S09 | Structural | Error | Signal origin references unknown port | Fix port name |
| S10 | Structural | Error | Duplicate instance name | Rename one instance |
| S11 | Structural | Error | Duplicate signal name | Rename one signal |
| S12 | Structural | Warning | Card `fits` does not match slot format | Check card meta vs slot declaration |
| S13 | Structural | Warning | Card `fits` but no slot uses that format | Verify slot format name |
| S14 | Structural | Warning | Vector port referenced without index | Add `[1..N]` or `[auto]` |
| S15 | Structural | **Error** | Range size mismatch on `connect` | Fix channel counts; `@suppress(structural)` only for intentional partial connect |
| S16 | Structural | Error | Card port name collision — card port conflicts with template port or another card's port | Rename the conflicting port; not suppressible |
| M01 | Mechanical | Error | Connector mismatch (XLR → BNC) | `@suppress(mechanical)` if intentional |
| E01 | Electrical | Error | Level mismatch — may damage equipment | Check levels |
| E02 | Electrical | Warning | Level mismatch — may need a pad | Add pad or `@suppress(electrical)` |
| L01 | Logical | Error | Protocol mismatch (Dante → MADI, etc.) | Check protocol attributes; `@suppress(logical)` if intentional |
| T01 | Temporal | Warning | Clock domain mismatch | Verify clocking; `@suppress(temporal)` if intentional |
| N01 | Network | Error | Network member references unknown instance | Fix instance name. Cross-network connects are intentionally not flagged — see D020 |
| R01 | Ring | Error | Ring member references unknown instance | Fix instance name |
| R02 | Ring | Error | Ring member references unknown port (explicit form) | Fix port name |
| R03 | Ring | Warning | Ring member port missing ring protocol attribute | Check port attributes |
| R04 | Ring | Error | Implicit member ambiguous (zero or multiple matching ports) | Use explicit `member Instance.Port` |
| C01 | Convention | Info | Orphaned device (no connections/bridges/rings) | Expected on leaf devices |
| C02 | Convention | Warning | Duplicate connection (same source-target pair) | Remove duplicate |
| C03 | Convention | Info | Template declared with zero ports | Intentional or add ports |
| C04 | Convention | Info | Bus declared with zero outputs | Add outputs |
| C05 | Convention | Info | Redundancy terminates at AES67 boundary — Primary port only | Informational; no action required |
| TR01 | Trace | Warning | Signal origin has no outgoing connections | Add `bridge origin.Port -> ...` or `connect origin.Port -> ...` |
| TR02 | Trace | Warning | Signal cannot reach any output (`out`/`io`) port | Ensure connects/bridges lead to a port with direction `out` or `io` |
| F01 | Flow | Warning | Flow slot exhaustion — stream count exceeds Dante chipset limit | Reduce stream count or use a higher-capacity chipset |
| F02 | Flow | Info | AES67 stream exceeds 8 channels — hardware auto-splits into multiple flows | Informational |
| F03 | Flow | Error | Multicast prefix mismatch between AES67 devices — silent audio failure | Align multicast prefixes across devices |
| A01 | Auto | Error | `[auto]` used in `route` or `bus` | Use explicit index |
| A02 | Auto | Error | Both sides of connection use `[auto]` | Fix one side to explicit range |
| A03 | Auto | Error | Scalar port or cannot infer count | Add explicit range |
| A04 | Auto | Error | Auto-assignment overflowed port range | Reduce channels or expand port range |
| A05 | Auto | Error | No contiguous block available (fragmented) | Reorder explicit assignments |
| M-I01 | Meta | Info | Unknown `kind` value | Check kind table above |
| M-I02 | Meta | Info | Deprecated `device_type` used — migrate to `kind` | Run `scripts/migrate-device-type-to-kind.py` or rename manually |
| M-I03 | Meta | Info | Unknown `rf_subtype` value | Check meta spec |
| M-I04 | Meta | Info | `rf_band` set but `kind` is not `rf-system` | Add `kind: "rf-system"` or remove `rf_band` |
| M-I05 | Meta | Warning | `rf_min_channels` must be positive | Fix RF channel count |
| M-I06 | Meta | Warning | `rf_max_channels` < `rf_min_channels` | Fix RF channel range |
| M-I07 | Meta | Info | Unknown `dante_chipset` value | Check chipset values: `Ultimo`, `Broadway`, `Brooklyn_II`, `Brooklyn_3`, `HC` |
| M-I08 | Meta | Warning | Ultimo chipset does not support AES67 RTP flows | Use Brooklyn_II or higher for AES67 |

#### Suppression layers

Use `@suppress(layer)` inside a connect body (must be first property):

| Layer | Suppresses |
|-------|-----------|
| `direction` | D01-D03 |
| `mechanical` | M01 |
| `electrical` | E01-E02 |
| `logical` | L01 |
| `temporal` | T01 |
| `structural` | S14, S15 |
| `all` | All suppressible checks |

---

## Datasheet → PatchLang Workflow

When given a hardware spec sheet, follow this order:

1. **Name the template** after the model number only — `CL5`, not `Yamaha_CL5`.

2. **Fill `meta`** — manufacturer, model, category. For expansion cards add
   `kind: "card"` and `fits: "SlotFormat"`.

3. **Map physical ports** — for each physical connector on the device:
   - Identify connector type (`XLR`, `BNC_75`, `etherCON`, etc.)
   - Identify protocol (`Dante`, `MADI`, `SDI`, `Analogue`, etc.)
   - Channel-based protocol? → write `in` + `out` lines with channel count as range
   - Ring/bus protocol (OptoCore, TWINLANe, AVB)? → write `io` line
   - No protocol? → omit attribute list

4. **Add bridges** for internal signal paths — mic preamps → Dante out,
   Dante in → line outputs, SDI in → SDI out (router passthrough).

5. **Declare slots** if the device has expansion bays:
   `slot BayName[1..N]: SlotFormatIdentifier`

6. **Write instances** with `location` and `ip` from the spec.

7. **Write connections** — one `connect` per direction per cable.
   Bidirectional cables always get two connects with matching `cable:` metadata.
