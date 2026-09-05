---
layout: default
title: Examples
permalink: /examples/
---

# Examples

## Single-File Project

A minimal project does not need hierarchy. One `.patch` file, no `project.json`:

```
# Simple worship venue — single file
template Rio3224 {
  meta { manufacturer: "Yamaha", model: "Rio3224", category: "Stagebox" }
  ports {
    Dante_Pri_In[1..32]: in(etherCON) [Dante, primary]
    Dante_Pri_Out[1..32]: out(etherCON) [Dante, primary]
    Mic_In[1..32]: in(XLR)
    Line_Out[1..16]: out(XLR)
  }
  bridge Mic_In -> Dante_Pri_Out
}

template CL5 {
  meta { manufacturer: "Yamaha", model: "CL5", category: "Console" }
  ports {
    Dante_Pri_In[1..72]: in(etherCON) [Dante, primary]
    Dante_Pri_Out[1..24]: out(etherCON) [Dante, primary]
    Mix_Bus[1..24]: out
  }
}

instance Stage_Left is Rio3224 { location: "Stage Left Wing" }
instance FOH_Console is CL5 { location: "Front of House" }

connect Stage_Left.Dante_Pri_Out -> FOH_Console.Dante_Pri_In {
  cable: "Cat6a_SL_Pri"
  length: "30m"
}

bridge Stage_Left.Mic_In[1..32] -> FOH_Console.Dante_Pri_In[1..32]

signal Lead_Vocal {
  origin: Stage_Left.Mic_In[1]
  description: "Worship leader vocal"
}

config FOH_Console {
  label Dante_Pri_In[1]: "Lead Vocal" { phantom: "true" }
  label Dante_Pri_In[2]: "Kick Drum"
}
```

That is a complete, valid project. The compiler parses it, validates it, and the frontend renders it on a canvas. The layout sidecar (`simple-venue.layout.json`) stores block positions. If it does not exist, auto-layout generates one.

---

## Multi-File Project

A real-world project uses `use` statements to import templates from other files. The compiler discovers files by walking `use` statements from the root.

### On disk

```
hillsong-mtg/
  project.json
  campus.patch
  campus.layout.json
  buildings/
    foh.patch
    monitors.patch
    stage.patch
  lib/
    yamaha.patch
```

### `project.json`

```json
{
  "name": "Hillsong MTG",
  "author": "A. Engineer",
  "root": "campus.patch"
}
```

### `campus.patch`

```
# Hillsong MTG — Campus Level
use buildings.foh { FOH_System }
use buildings.monitors { Monitor_System }
use buildings.stage { Stage_System }

instance FOH is FOH_System { location: "Front of House" }
instance Monitors is Monitor_System { location: "Monitor World" }
instance Stage is Stage_System { location: "Main Stage" }

connect FOH.Dante_Out -> Monitors.Dante_In {
  cable: "Cat6a_FOH_MON"
  length: "45m"
}
connect FOH.Dante_Out -> Stage.Dante_In {
  cable: "Cat6a_FOH_STG"
  length: "60m"
}
```

### `buildings/foh.patch`

```
# Front of House — consoles, stageboxes, network
use lib.yamaha { CL5, Rio3224 }

template FOH_System {
  meta { category: "Building" }
  ports {
    Dante_Out: out(etherCON) [Dante]
    Dante_In: in(etherCON) [Dante]
  }
}

instance Console is CL5 { location: "FOH Mix Position" }
instance SL_Rack is Rio3224 { location: "Stage Left" }

connect SL_Rack.Dante_Pri_Out -> Console.Dante_Pri_In {
  cable: "Cat6a_SL"
  length: "30m"
}

bridge SL_Rack.Mic_In[1..32] -> Console.Dante_Pri_In[1..32]
```

The hierarchy is not a special feature. It is just templates importing other templates via `use`. A `.patch` file IS a template at every scale, whether it describes a single device, a room, a building, or an entire campus.

---

## Bus Display Names (`label:`)

Broadcast console software uses `>` as a routing convention in bus names: `SPOTIFY>FOH`, `PQ>MM`, `Shouts > MD Conf`. These characters are invalid in PatchLang identifiers. The `label:` property on a bus body carries the human-readable display name while the identifier remains the stable cross-reference key.

```
instance FOH_Engine is CL5 {
  bus PQ_MM {
    label: "PQ>MM"
    output "Mix L": Mix_Out[1]
    output "Mix R": Mix_Out[2]
  }
  bus SpotifyFOH {
    label: "SPOTIFY>FOH"
    input: Dante_Pri_In[41]
    input: Dante_Pri_In[42]
    output "Main L": Mix_Out[25]
    output "Main R": Mix_Out[26]
  }
}
```

The identifier (`SpotifyFOH`) is used everywhere buses are cross-referenced. The `label` is display-only: it does not affect routing, DRC, or signal tracing. The JSON output omits `label` when not set, so existing files are unaffected.

This is the same identifier-vs-display-name pattern used by `config` port labels (`label Dante_Pri_In[1]: "Lead Vocal"`).

---

## Ring Network (OptoCore)

A DiGiCo system with primary and redundant OptoCore rings. The `ring` keyword declares a fiber loop where member order reflects the physical ring topology.

```
template SD12 {
  meta { manufacturer: "DiGiCo", model: "SD12" }
  ports {
    OptoCore_A: io [OptoCore]
    OptoCore_B: io [OptoCore]
  }
}

template SD_Rack {
  meta { manufacturer: "DiGiCo", model: "SD-Rack" }
  ports {
    OptoCore_A: io [OptoCore]
    OptoCore_B: io [OptoCore]
  }
}

instance Console is SD12 { location: "FOH" }
instance StageRack_1 is SD_Rack { location: "Stage Left" }
instance StageRack_2 is SD_Rack { location: "Stage Right" }
instance MonitorRack is SD_Rack { location: "Monitor World" }

# Primary ring — implicit members (compiler resolves to the single OptoCore port)
ring OptoCore_Primary {
  protocol: "OptoCore"
  member Console
  member StageRack_1
  member StageRack_2
  member MonitorRack
}

# Redundant ring — explicit port references targeting the B ports
ring OptoCore_Redundant {
  protocol: "OptoCore"
  label: "Redundant ring via B ports"
  member Console.OptoCore_B
  member StageRack_1.OptoCore_B
  member StageRack_2.OptoCore_B
  member MonitorRack.OptoCore_B
}
```

Implicit members (`member Console`) work when the device has a single port matching the ring's protocol. Explicit members (`member Console.OptoCore_B`) are needed when you want to specify which port participates, typically for redundant rings using the secondary fiber ports.

---

## Channel Auto-Assignment with `[auto]`

When connecting devices over Dante, you need to specify which channels you're using on each port. For six IEM packs on a monitor console, that means manually counting channel pairs:

```
connect MON.Dante_Pri_Out[1..2]   -> IEM_WL.Dante_In[1..2]
connect MON.Dante_Pri_Out[3..4]   -> IEM_MD.Dante_In[1..2]
connect MON.Dante_Pri_Out[5..6]   -> IEM_Keys.Dante_In[1..2]
# ...tedious and fragile
```

Replace the console's output channels with `[auto]` and let the compiler assign them sequentially:

```
connect MON.Dante_Pri_Out[auto] -> IEM_WL.Dante_In[1..2]
connect MON.Dante_Pri_Out[auto] -> IEM_MD.Dante_In[1..2]
connect MON.Dante_Pri_Out[auto] -> IEM_Keys.Dante_In[1..2]
connect MON.Dante_Pri_Out[auto] -> IEM_Drums.Dante_In[1..2]
connect MON.Dante_Pri_Out[auto] -> IEM_Bass.Dante_In[1..2]
connect MON.Dante_Pri_Out[auto] -> IEM_BV.Dante_In[1..2]
```

The compiler resolves top-to-bottom: first `[auto]` gets `[1..2]`, second gets `[3..4]`, and so on. You can mix explicit and auto: explicit indices are pre-scanned and skipped during auto-allocation:

```
# Recorder always gets channels 33-34 (locked to a Dante preset)
connect MON.Dante_Pri_Out[33..34] -> Recorder.Dante_In[1..2]

# IEMs get whatever's available, starting from channel 1
connect MON.Dante_Pri_Out[auto] -> IEM_WL.Dante_In[1..2]     # resolves to [1..2]
connect MON.Dante_Pri_Out[auto] -> IEM_MD.Dante_In[1..2]     # resolves to [3..4]
```

Use `[auto]` when the specific channel numbers do not matter: the design intent is "each device gets its own pair, no overlaps." Use explicit indices when the channel assignment is locked to hardware (a Dante preset, a saved show file) or when the diagram IS the configuration document.

---

## DRC Suppression with `@suppress`

The compiler runs design rule checks (DRC) across several layers: `direction`, `mechanical`, `electrical`, `logical`, `temporal`, and `convention`. When a connection intentionally violates a rule, suppress the specific layer rather than ignoring the warning:

```
# A loopback cable from a console's record output back to its playback input.
# The DRC flags this as a direction concern (output feeding the same device's input).
connect Console.Dante_Pri_Out[1..2] -> Console.Dante_Pri_In[65..66] {
  @suppress(direction)
  cable: "Cat6a_Loopback"
  length: "1m"
}
```

You can suppress multiple layers on one connection:

```
# Test rig: SDI video output looped back into a Dante audio embedder.
# Intentionally crosses media domains and reverses normal signal flow.
connect Recorder.SDI_Out -> Embedder.SDI_In {
  @suppress(mechanical, logical)
  cable: "BNC_Test"
}
```

Use `@suppress(all)` as a last resort to silence every DRC layer on a single connection. Always prefer naming the specific layer so that other checks still run.

---

## Split Directional Ports (Dante Template)

Network protocols like Dante carry audio in both directions over a single cable. In PatchLang, model this with separate `in()` and `out()` port vectors and `bridge` declarations that map the signal flow:

```
template Rio1608_D2 {
  meta { manufacturer: "Yamaha", model: "Rio1608-D2", category: "Stagebox" }
  ports {
    Dante_Pri_In[1..16]: in(etherCON) [Dante, primary]
    Dante_Pri_Out[1..16]: out(etherCON) [Dante, primary]
    Mic_In[1..16]: in(XLR)
    Line_Out[1..8]: out(XLR)
  }
  bridge Mic_In -> Dante_Pri_Out
  bridge Dante_Pri_In -> Line_Out
}
```

The first bridge says "mic inputs are forwarded out over Dante." The second says "Dante receive channels drive the line outputs." This gives the DRC enough information to trace a signal from a microphone on stage through the network to a console, and back out to an amplifier, without confusing which direction each channel flows.

---

## Slot / Card Installation

Hardware with expansion slots (consoles, stage racks, routers) is modeled with `slot` declarations in the template and slot assignments on the instance:

```
template MADI_Card {
  meta { manufacturer: "AVID", model: "MADI" }
  ports {
    MADI_In[1..48]: in(BNC_75) [MADI]
    MADI_Out[1..48]: out(BNC_75) [MADI]
  }
}

template HDX_Card {
  meta { manufacturer: "AVID", model: "HDX Card" }
  ports {
    USB[1..64]: io(USB)
  }
}

template Venue_FOH_Rack {
  meta { manufacturer: "AVID", model: "Venue FOH Rack" }
  ports {
    AES_In[1..2]: in(XLR) [AES3]
    AES_Out[1..2]: out(XLR) [AES3]
    LINE[1..8]: io(TRS_14)
  }
  slot Expansion[1..3]: HDX_Card
  slot Snake[1..2]: MADI_Card
}

instance FOH_Engine is Venue_FOH_Rack {
  slot Expansion[1]: "HDX_Card"
  slot Expansion[2]: "HDX_Card"
  slot Snake[1]: "MADI_Card"
  slot Snake[2]: "MADI_Card"
}
```

The template declares what card types each slot accepts. The instance declares what is actually installed. Unassigned slots remain empty: the compiler does not require every slot to be populated.

---

## Internal Routes

Routes describe signal paths inside a single device: how an input gets patched to an output within the hardware's internal matrix:

```
template Venue_FOH_Rack {
  meta { manufacturer: "AVID", model: "Venue FOH Rack" }
  ports {
    MADI_In[1..48]: in(BNC_75) [MADI]
    MADI_Out[1..48]: out(BNC_75) [MADI]
    LINE[1..8]: io(TRS_14)
  }
}

instance FOH_Engine is Venue_FOH_Rack {
  route MADI_In[41] -> LINE[1]
  route MADI_In[42] -> LINE[2]
  route MADI_In[43] -> LINE[3]
  route MADI_In[44] -> LINE[4]
  route MADI_In[45] -> LINE[5]
  route MADI_In[46] -> LINE[6]
  route MADI_In[47] -> LINE[7]
  route MADI_In[48] -> LINE[8]
}
```

Routes are distinct from `connect` (which describes a cable between two devices) and `bridge` (which declares a logical signal path for DRC tracing). A route says "inside this specific device instance, input X is patched to output Y." This is how you document a console's internal routing matrix, a DSP's patch bay, or a router's crosspoints.

---

## Config Block (Channel Labels)

A `config` block attaches metadata to individual channels on an instance, typically channel names and per-channel settings like phantom power:

```
template StageBox_16x0 {
  meta { model: "16x0 XLR" }
  ports {
    Stage_Box_Inputs[1..16]: in(XLR) [Analogue]
    Inputs_to_Patch[1..16]: out(XLR) [Analogue]
  }
}

instance Drums is StageBox_16x0 { location: "Stage Left" }

config Drums {
  label Stage_Box_Inputs[1]: "Kick In" { phantom: "true", stand: "Short Boom" }
  label Stage_Box_Inputs[2]: "Kick Out" { stand: "Short Boom" }
  label Stage_Box_Inputs[3]: "Snare Top" { stand: "Short Boom" }
  label Stage_Box_Inputs[4]: "Snare Bottom" { stand: "LP Claw" }
  label Stage_Box_Inputs[5]: "Hats" { phantom: "true" }
  label Stage_Box_Inputs[6]: "Tom 1"
  label Stage_Box_Inputs[7]: "Tom 2"
  label Stage_Box_Inputs[8]: "Tom 3"
  label Stage_Box_Inputs[10]: "OH SR" { phantom: "true", stand: "Tall Boom" }
  label Stage_Box_Inputs[11]: "OH SL" { phantom: "true", stand: "Tall Boom" }
}
```

Labels serve two purposes: they document the input list (what mic goes where) and they carry operational metadata (phantom power, stand type) that the frontend can display. The key-value pairs inside the braces after the label string are free-form: use whatever fields your workflow needs.

---

## Template Composition

A template can contain instances of other templates and internal connections between them. This is how you build reusable subsystems: a "Monitor Rig" template that bundles a console with its IEM transmitters:

```
template PSM1000 {
  meta { manufacturer: "Shure", model: "PSM1000", category: "IEM" }
  ports {
    Dante_In[1..2]: in(etherCON) [Dante, primary]
    RF_Out[1..2]: out(BNC_50) [RF]
  }
}

template PM5D {
  meta { manufacturer: "Yamaha", model: "PM5D", category: "Console" }
  ports {
    Dante_Pri_In[1..48]: in(etherCON) [Dante, primary]
    Dante_Pri_Out[1..24]: out(etherCON) [Dante, primary]
  }
}

template Monitor_Rig {
  meta { category: "Subsystem" }
  ports {
    Dante_In[1..48]: in(etherCON) [Dante, primary]
  }

  instance Console is PM5D
  instance IEM_WL is PSM1000
  instance IEM_MD is PSM1000
  instance IEM_Keys is PSM1000

  connect Console.Dante_Pri_Out[1..2] -> IEM_WL.Dante_In[1..2]
  connect Console.Dante_Pri_Out[3..4] -> IEM_MD.Dante_In[1..2]
  connect Console.Dante_Pri_Out[5..6] -> IEM_Keys.Dante_In[1..2]
}
```

When you instantiate `Monitor_Rig`, you get the console and all three IEM transmitters as a single block on the canvas. Double-clicking it opens the internal view showing the wiring between them. This is the same mechanism that makes multi-file projects work: a building template containing room instances is just a larger-scale composition.
