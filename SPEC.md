# PatchLang Language Specification

**Version:** 0.1.0
**Status:** Draft

PatchLang is a domain-specific language for describing signal flow in broadcast and live production environments. It defines device templates, physical instances, cable connections, logical signal mappings, and channel configuration. PatchLang files (`.patch`) are the source of truth — they are human-readable, git-diffable, and designed for LLM generation.

---

## 1. Lexical Structure

### 1.1 Comments

Lines starting with `#` are comments. Comments extend to the end of the line.

```ebnf
comment = "#" { any-char-except-newline } ;
```

```
# This is a comment
template Amp { }  # inline comments work too
```

### 1.2 Whitespace

Spaces, tabs, carriage returns, and newlines are whitespace. Whitespace separates tokens but is otherwise insignificant. There is no significant indentation.

### 1.3 Identifiers

```ebnf
identifier = ( letter | "_" ) { letter | digit | "_" } ;
letter     = "A".."Z" | "a".."z" ;
digit      = "0".."9" ;
```

Identifiers name templates, instances, ports, signals, and properties. They must start with a letter or underscore. Hyphens are not allowed — use underscores.

```
FOH_Console       (* valid *)
_80_ch_Splitter   (* valid — leading underscore for names starting with digits *)
Stage_Left        (* valid *)
my-device         (* INVALID — no hyphens *)
```

### 1.4 Keywords

The following identifiers are reserved keywords:

```
template  instance  is  connect  bridge  bridge_group  link_group
signal  flag  stream  config  ports  meta  in  out  io
for  over  generate  use  slot  routing  route  bus  label
```

Keywords can be used as property keys (see §3.10) but not as names for templates, instances, or ports.

### 1.5 Annotations

```
@suppress    @version
```

### 1.6 Literals

```ebnf
number         = "0" | ( digit-nonzero { digit } ) ;
string-literal = '"' { string-char } '"' ;
string-char     = ? any character except '"' and '\' ? | escape-sequence ;
escape-sequence = '\' ( '"' | '\' | 'n' | 'r' | 't' ) ;
digit-nonzero  = "1".."9" ;
```

A quoted string may contain any character. `"` and `\` **must** be escaped; `\n`, `\r`
and `\t` are also recognised. Any other escape (e.g. `\q`) is a lex error rather than
being silently reinterpreted.

The formatter always escapes on the way out, so a user-supplied value like a bus named
`The "Big" Mix` survives a format → parse round trip intact. Before this existed, such a
value was silently truncated to `The ` — the emitted text still parsed, just with a
different value. See D026.

A raw newline inside quotes is still accepted, so files written before escaping existed
keep parsing; it is simply re-emitted as `\n` to keep `.patch` files line-oriented.


### 1.7 Punctuation

```
->  ..  .  {  }  (  )  [  ]  :  ,  *
```

---

## 2. Grammar

### 2.1 Program

A PatchLang file is a sequence of top-level statements.

```ebnf
program   = { statement } ;
statement = template-decl | instance-decl | connect-decl | bridge-decl
          | bridge-group-decl | link-group-decl | signal-decl | flag-decl
          | stream-decl | config-decl | use-decl ;
```

---

## 3. Statements

### 3.1 Template Declaration

Defines a reusable device type.

```ebnf
template-decl = "template" identifier [ param-list ] [ version-annotation ]
                "{" { template-block } "}" ;

template-block = meta-block | ports-block | template-bridge
               | template-instance | template-connect | slot-def ;

param-list = "(" param-def { "," param-def } ")" ;
param-def  = identifier ":" ( number | string-literal ) ;

version-annotation = "@version" "(" string-literal ")" ;
```

```
template Rio3224(mic_count: 32) @version("2.0") {
  meta {
    manufacturer: "Yamaha"
    model: "Rio3224"
    category: "Stagebox"
  }
  ports {
    Dante_Pri: io(etherCON) [Dante, primary]
    Mic_In[1..32]: in(XLR)
    Line_Out[1..16]: out(XLR)
  }
  bridge Mic_In -> Dante_Pri
  slot MY_Slot[1..3]: MY_Card
}
```

### 3.2 Meta Block

Key-value metadata inside a template.

```ebnf
meta-block = "meta" "{" { key-value-pair } "}" ;
```

Standard keys: `manufacturer`, `model`, `category`. Custom keys are allowed.

### 3.3 Ports Block

Defines the device's physical interfaces.

```ebnf
ports-block = "ports" "{" { port-def } "}" ;

port-def = identifier [ range-spec ] ":" port-direction
           [ connector-spec ] [ attribute-list ] ;

range-spec     = "[" number ".." number "]" ;
port-direction = "in" | "out" | "io" ;
connector-spec = "(" identifier ")" ;
attribute-list = "[" attribute { "," attribute } "]" ;
attribute      = identifier [ ":" identifier ] ;
```

```
Mic_In[1..32]: in(XLR)                     # 32 XLR inputs
Dante_Pri: io(etherCON) [Dante, primary]   # bidirectional with attributes
Mix_Bus[1..24]: out                         # outputs, no connector specified
SDI[1..72]: io(SDI_BNC) [SDI]              # with protocol attribute
MADI[1..64]: io(SC_Fiber) [MADI, redundant] # with named attribute
```

#### Connectors

Common connector identifiers: `XLR`, `BNC_75`, `RJ45`, `etherCON`, `SFP`, `LC_Fiber`, `SC_Fiber`, `MTRJ_Fiber`, `opticalCON_DUO`, `opticalCON_QUAD`, `SpeakON`, `TRS_14`, `TRS_3`, `DB25`, `HDMI`, `SDI_BNC`, `USB`, `SMA`.

`opticalCON_DUO` (2-fiber) and `opticalCON_QUAD` (4-fiber) are Neutrik ruggedised fiber housings used by SSL Live consoles for the Blacklight II and X-Light protocols. They mate themselves and the generic `opticalCON`, but DUO and QUAD cannot mate each other (M01).

Custom connector names are allowed.

#### Attributes

Attributes describe transport protocols and properties: `Dante`, `AES67`, `MADI`, `AES3`, `SDI`, `NDI`, `SMPTE2110`, `Analogue`, `WordClock`, `Ethernet`, `primary`, `secondary`, `redundant`, `Gigabit`.

Named attributes use `key: value` syntax: `[protocol: SDI, format: UHD]`.

### 3.4 Instance Declaration

Creates a physical device from a template.

```ebnf
instance-decl = "instance" identifier "is" identifier
                [ arg-list ] [ version-constraint ] [ instance-body ] ;

arg-list           = "(" arg-def { "," arg-def } ")" ;
arg-def            = identifier ":" ( number | string-literal ) ;
version-constraint = "@version" "(" string-literal ")" ;

instance-body = "{" { instance-entry } "}" ;
instance-entry = route-entry | bus-entry | slot-assignment | key-value-pair ;
```

```
instance Stage_Left is Rio3224 {
  location: "Stage Left Wing"
  ip: "192.168.1.31"
}

instance FOH_Console is CL5(mic_count: 48) @version(">=4.0") {
  location: "Front of House"
  route Dante_In[1] -> Fader[1]
  route Dante_In[2] -> Fader[2]
  bus Main_LR {
    input: Fader[1..8]
    insert_send: Ext_Out[3], Ext_Out[10]
    insert_return: Ext_In[4], Ext_In[8]
    output: Matrix_Out[1..2]
  }
  slot MY_Slot[1]: "Dante_Card"
}
```

### 3.5 Connect Declaration

Defines a physical cable between two ports.

```ebnf
connect-decl = "connect" port-ref "->" port-ref [ connect-body ] ;

connect-body = "{" [ suppress-annotation ] { key-value-pair } "}" ;

suppress-annotation = "@suppress" "(" identifier { "," identifier } ")" ;
```

```
connect Stage_Left.Dante_Pri -> Dante_Switch.Port[1] {
  cable: "Cat6a_SL_Pri"
  length: "30m"
}

connect SL.Analog_Out[1..4] -> FOH.Line_In[1..4] {
  @suppress(protocol_mismatch)
  mapping: "1:1"
}
```

#### Mapping Property

The `mapping` property specifies channel-level routing. Three formats:

```
mapping: "1:1"                    # sequential one-to-one (default)
mapping: "offset 16"              # shifted by N channels
mapping: "1->3, 2->4, 3->1"      # explicit per-channel pairs
```

### 3.6 Bridge Declaration

Logical signal mapping between ports (no physical cable).

```ebnf
bridge-decl = "bridge" port-ref "->" port-ref ;
```

Inside templates, port references can be local (no instance prefix):

```ebnf
template-bridge = "bridge" port-ref-or-local "->" port-ref-or-local ;
```

```
# Top-level (between instances)
bridge Stage_Left.Mic_In[1..32] -> FOH_Console.Dante_Ch[1..32]

# Inside a template (local port names)
template Stagebox {
  ports { Mic_In[1..16]: in(XLR)  Network: io(etherCON) [Dante] }
  bridge Mic_In -> Network
}
```

### 3.7 Bridge Group Declaration

Sequential channel mapping — multiple sources auto-fill a destination range.

```ebnf
bridge-group-decl = "bridge_group" port-ref "{" { port-ref } "}" ;
```

```
bridge_group FOH.Dante_Ch {
  SL.Mic_In[1..4]     # maps to Ch[1..4]
  SR.Mic_In[1..4]     # maps to Ch[5..8]
}
```

### 3.8 Link Group Declaration

Groups connections as a logical unit.

```ebnf
link-group-decl = "link_group" identifier "{" { connect-decl | key-value-pair } "}" ;
```

```
link_group Cam1_UHD {
  connect Cam1.SDI_Out[1] -> Router.SDI_In[1]
  connect Cam1.SDI_Out[2] -> Router.SDI_In[2]
  connect Cam1.SDI_Out[3] -> Router.SDI_In[3]
  connect Cam1.SDI_Out[4] -> Router.SDI_In[4]
  mode: "quad_link_4K"
}
```

### 3.9 Signal, Flag, Stream Declarations

```ebnf
signal-decl = "signal" identifier [ "{" { key-value-pair } "}" ] ;
flag-decl   = "flag"   identifier [ "{" { key-value-pair } "}" ] ;
stream-decl = "stream" identifier [ "{" { key-value-pair } "}" ] ;
```

The `origin` property in signals and `source` property in streams accept port references as values.

A stream's `source` may carry an **index spec** naming which of the source interface's
channels are on the flow — an AES67 TX stream is a selection of existing Dante output
channels re-sent as a multicast flow. The selection is **ordered and position-significant**,
because an AES67 receiver maps by position: `[7, 1, 5, 3]` and `[1, 3, 5, 7]` are different
flows. It is never sorted or deduplicated on emit, and a repeated index is legitimate
replication of one source onto two receiver positions, not an error. Omitting the index
means the whole interface. See D025.

```
stream Drums_AES67 {
  source: DM7.Dante_Out[1, 3, 5, 7]
  channels: "4"
  protocol: "AES67"
}
```

A `stream` is a **transmit** construct. In both Dante and AES67 a flow is created and
advertised by the *talker*; a receiver never originates a flow, it only subscribes to
someone else's. So `direction` **defaults to `tx`** when absent — an undirected
`stream { source: ... }` is a transmit by definition, not by convention.

`direction: "rx"` is the explicit marker for a flow this device is *subscribed to
receive*, which SignalCanvas documents on the receiving device. There is no third,
undirected state. Loading a stream with no `direction` emits the F06 advisory naming
what it was treated as. See D027.


```
signal Lead_Vocal {
  origin: Stage_Left.Mic_In[1]
  channel: "1"
  description: "Worship leader vocal"
}

stream SL_Dante_Primary {
  source: Stage_Left.Dante_Pri
  channels: "32"
  protocol: "Dante"
}

flag Genlock_OK {
  description: "All cameras locked to house sync"
  severity: "warning"
}
```

### 3.10 Config Declaration

Per-instance channel labels and metadata.

```ebnf
config-decl  = "config" identifier "{" { config-label } "}" ;
config-label = "label" port-ref-or-local ":" string-literal
               [ "{" { key-value-pair } "}" ] ;
```

```
config FOH_Console {
  label Dante_In[1]: "Pastor Mic" {
    phantom: "true"
    stand: "tall boom"
  }
  label Dante_In[2]: "Worship Leader" {
    phantom: "true"
  }
  label Fader[1]: "Lead Vocal"
}
```

#### Channel Inserts

`insert_send` / `insert_return` break a channel out to an external processor and back.
Their value is a **quoted, comma-separated list of port references**, one per leg —
`[L]` mono, `[L, R]` stereo:

```
config FOH_Console {
  label Mic_In[1]: "Kick" {
    insert_send: "Ext_Out[3], Ext_Out[10]"
    insert_return: "Ext_In[4], Ext_In[8]"
  }
}
```

Order is significant (reversing the list swaps the stereo image), legs are independent
(no adjacency or equal-width constraint), and each leg carries a single-channel index —
a range such as `Ext_Out[1..2]` is not accepted.

**Quote the value.** A property value written bare (`insert_send: Ext_Out[3]`) parses as
a single port reference and can carry only one leg; the quoted string form is canonical
and is what the compiler emits. See the [bus insert](docs/language-reference.md) syntax
for the equivalent on buses, which uses native grammar rather than a string because a
bus has no property block.

Added in PatchLang v0.3.2.

### 3.11 Use Declaration

Imports templates from a library namespace.

```ebnf
use-decl = "use" namespace [ "." "*" ] [ "{" identifier { "," identifier } "}" ] ;
namespace = identifier { "." identifier } ;
```

```
use audio.yamaha { CL5, Rio3224 }
use video.blackmagic.*
use infrastructure.dante
```

### 3.12 Slot Definition (inside templates)

```ebnf
slot-def = "slot" identifier [ range-spec ] ":" identifier ;
```

```
slot MY_Slot[1..3]: MY_Card
slot Expansion[1..8]: Expansion
```

### 3.13 Route Entry (inside instance body)

```ebnf
route-entry = "route" port-ref-or-local "->" port-ref-or-local ;
```

### 3.14 Bus Entry (inside instance body)

```ebnf
bus-entry      = "bus" identifier "{" { bus-port-entry } "}" ;
bus-port-entry = ( "input" | "output" | "in" | "out" ) ":" port-ref-or-local ;
```

### 3.15 Slot Assignment (inside instance body)

```ebnf
slot-assignment = "slot" identifier [ "[" number "]" ] ":" string-literal ;
```

---

## 4. Common Productions

### 4.1 Port Reference

```ebnf
port-ref          = identifier "." identifier [ index-spec ] ;
port-ref-or-local = identifier [ "." identifier ] [ index-spec ] ;
```

A `port-ref` is always fully qualified: `Instance.Port`. A `port-ref-or-local` is used inside templates and instance bodies where the instance prefix is optional.

### 4.2 Index Spec

```ebnf
index-spec    = "[" index-element { "," index-element } "]" ;
index-element = number [ ".." number ] | "auto" ;
```

Supports single indices, ranges, mixed, and auto-assignment:

```
[1]            # single channel
[1..32]        # range from 1 to 32
[1..4,7,9]     # mixed: channels 1,2,3,4,7,9
[auto]         # auto-assign contiguous channels at compile time
```

**`[auto]` keyword:** When used on one side of a connection, the compiler allocates the next N contiguous available channels from the port's declared range, where N is inferred from the other side. Channels are allocated in declaration order and skip explicitly claimed indices. `[auto]` must be the sole element in the index spec (cannot mix with numbers). Both sides of a connection cannot use `[auto]` simultaneously. `[auto]` is not valid in `route` or `bus` declarations. Referencing a vector port without any index spec emits a warning (S14).

### 4.3 Key-Value Pair

```ebnf
key-value-pair = property-key ":" property-value ;
property-key   = identifier | "label" | "stream" | "route" | "bus"
               | "routing" | "config" ;
property-value = string-literal | number | port-ref ;
```

Keywords can be used as property keys. Values can be strings, numbers, or port references.

---

## 5. Complete Example

```
# Worship venue — Dante audio network

template Rio3224 {
  meta {
    manufacturer: "Yamaha"
    model: "Rio3224"
    category: "Stagebox"
  }
  ports {
    Dante_Pri: io(etherCON) [Dante, primary]
    Mic_In[1..32]: in(XLR)
    Line_Out[1..16]: out(XLR)
  }
  bridge Mic_In -> Dante_Pri
}

template CL5 {
  meta {
    manufacturer: "Yamaha"
    model: "CL5"
    category: "Console"
  }
  ports {
    Dante_Pri: io(etherCON) [Dante, primary]
    Dante_Ch[1..72]: in [Dante]
    Mix_Bus[1..24]: out
  }
}

instance Stage_Left is Rio3224 {
  location: "Stage Left Wing"
  ip: "192.168.1.31"
}

instance FOH_Console is CL5 {
  location: "Front of House"
  ip: "192.168.1.10"
}

connect Stage_Left.Dante_Pri -> FOH_Console.Dante_Pri {
  cable: "Cat6a_SL_Pri"
  length: "30m"
}

bridge Stage_Left.Mic_In[1..32] -> FOH_Console.Dante_Ch[1..32]

signal Lead_Vocal {
  origin: Stage_Left.Mic_In[1]
  channel: "1"
  description: "Worship leader vocal"
}

config FOH_Console {
  label Dante_Ch[1]: "Lead Vocal" { phantom: "true" }
  label Dante_Ch[2]: "Kick Drum"
}
```

---

## 6. File Conventions

- **Extension:** `.patch`
- **Encoding:** UTF-8
- **Line endings:** LF or CRLF (both accepted)
- **Layout sidecar:** `.layout.json` stores canvas positions and UI state (not part of PatchLang)
- **Config files:** `.config.patch` can hold `config` blocks imported via `use`

---

## 7. Design Principles

1. **Human-readable first.** A broadcast engineer should be able to read a `.patch` file and understand the signal chain without special tooling.
2. **LLM-friendly.** The syntax is simple enough that language models can generate valid `.patch` files from plain English descriptions.
3. **Git-diffable.** Text diffs show meaningful changes. Adding a mic input is one line, not a JSON blob.
4. **No ambiguity.** Every statement starts with a unique keyword. The grammar is LL(1).
5. **Domain-specific.** The language models broadcast concepts (ports, connectors, protocols, signal chains) directly — not through generic data structures.
