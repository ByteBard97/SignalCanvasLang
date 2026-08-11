//! Stream source channel selection on the load path and end-to-end (#37, Phase 3).
//!
//! The selection is an ordered list of source channels on an AES67/Dante flow.
//! Position is semantically significant — a receiver maps channels by position —
//! so every assertion here is about ORDER, not membership.

use super::canvas_test_helpers::*;
use crate::builder::canvas_input::*;
use crate::builder::canvas_load::load_from_patch;
use crate::builder::canvas_output::StreamOutput;

// ---------------------------------------------------------------------------
// Hand-authored source → load
// ---------------------------------------------------------------------------

/// A one-instance patch whose single TX stream sources `AES67_Out` with the
/// given index text (e.g. `[7, 1, 5, 3]`, or `""` for no index at all).
fn patch_with_stream_index(index: &str) -> String {
    format!(
        r#"
template Core_250i {{
  meta {{ manufacturer: "QSC", model: "Core 250i" }}
  ports {{
    AES67_Out[1..64]: out(etherCON) [AES67]
  }}
}}
instance DSP is Core_250i

stream Talkback {{
  source: DSP.AES67_Out{index}
  channels: 4
  direction: "tx"
  protocol: "AES67"
}}
"#
    )
}

/// Load `patch_with_stream_index(index)` and return the one TX stream on `DSP`.
fn load_only_tx_stream(index: &str) -> StreamOutput {
    let patch = patch_with_stream_index(index);
    let loaded = load_from_patch(&patch, "")
        .unwrap_or_else(|e| panic!("load must succeed for index `{index}`: {e}\n{patch}"));
    let inst = loaded
        .instances
        .iter()
        .find(|i| i.name == "DSP")
        .expect("instance DSP must load");
    assert_eq!(
        inst.tx_streams.len(),
        1,
        "expected exactly one TX stream for index `{index}`"
    );
    inst.tx_streams[0].clone()
}

/// The selection must arrive in declaration order. Sorting it would silently
/// re-patch the flow: `[7, 1, 5, 3]` and `[1, 3, 5, 7]` describe different wiring.
#[test]
fn load_preserves_non_monotonic_selection_order() {
    let stream = load_only_tx_stream("[7, 1, 5, 3]");
    assert_eq!(
        stream.source_channels,
        vec![7, 1, 5, 3],
        "selection must load in declaration order, unsorted"
    );
}

// ---------------------------------------------------------------------------
// Direction defaulting (#38)
// ---------------------------------------------------------------------------

/// A stream with no `direction` must load as TX, not vanish.
///
/// `direction` was read with `unwrap_or_default()` and the tx/rx split matched
/// `== "tx"` / `== "rx"` exactly, so an undirected stream fell in neither bucket and
/// was dropped with no diagnostic. Half the streams in the production `MTG.patch`
/// (`COMMSQSYS`, `to_Comms`) disappeared on load, and any save afterwards wrote them
/// out of existence.
///
/// Reid's spec call on #38: a stream is definitionally a *transmit* construct — in
/// both Dante and AES67 a flow is created and advertised by the talker, and a receiver
/// merely subscribes. So an undirected `stream { source: ... }` IS a TX stream, and
/// defaulting to `tx` recovers those streams rather than guessing at a convention.
#[test]
fn stream_without_direction_loads_as_tx() {
    let patch = r#"
template Core_250i {
  meta { manufacturer: "QSC", model: "Core 250i" }
  ports { AES67_Out[1..64]: out(etherCON) [AES67] }
}
instance DSP is Core_250i

stream COMMSQSYS {
  source: DSP.AES67_Out
  channels: 8
  protocol: "AES67"
}
"#;
    let loaded = load_from_patch(patch, "").expect("load must succeed");
    let inst = loaded
        .instances
        .iter()
        .find(|i| i.name == "DSP")
        .expect("instance DSP must load");
    assert_eq!(
        inst.tx_streams.len(),
        1,
        "an undirected stream must load as TX, not be silently dropped"
    );
    assert_eq!(inst.tx_streams[0].label, "COMMSQSYS");
    assert_eq!(
        inst.tx_streams[0].direction, "tx",
        "the defaulted direction must be reported as tx, not left empty"
    );
    assert!(
        inst.rx_streams.is_empty(),
        "defaulting must not also file it as a receive stream"
    );
}

/// An explicit `direction: "rx"` still means a subscribed/received flow.
#[test]
fn explicit_rx_direction_is_preserved() {
    let patch = r#"
template Core_250i {
  meta { manufacturer: "QSC", model: "Core 250i" }
  ports { AES67_In[1..64]: in(etherCON) [AES67] }
}
instance DSP is Core_250i

stream QSYSCOMMS {
  source: DSP.AES67_In
  channels: 8
  direction: "rx"
  protocol: "AES67"
}
"#;
    let loaded = load_from_patch(patch, "").expect("load must succeed");
    let inst = &loaded.instances[0];
    assert_eq!(inst.rx_streams.len(), 1, "explicit rx must stay rx");
    assert!(inst.tx_streams.is_empty(), "rx must not be defaulted into tx");
}

/// A repeated index is legitimate replication — one mono source landing on two
/// receiver positions (the same reason F05 is an Info phrased as a question, not a
/// fault) — so it must survive load intact.
///
/// The fixture repeats a channel BOTH adjacently (`3, 3`) and at a distance (the
/// later `3`), and that is deliberate: `Vec::dedup` only collapses *consecutive*
/// duplicates, so a selection like `[3, 1, 3]` survives it untouched and would let
/// the very refactor this test exists to catch ship green. The adjacent pair catches
/// `dedup()`; the distant one catches a sort-then-dedup or a set-based collect.
#[test]
fn load_preserves_repeated_channels_adjacent_and_distant() {
    let stream = load_only_tx_stream("[3, 3, 1, 3]");
    assert_eq!(
        stream.source_channels,
        vec![3, 3, 1, 3],
        "a repeat is replication, not a mistake to clean up"
    );
}

/// Ranges expand inclusively, in place, without disturbing the surrounding singles.
#[test]
fn load_expands_range_inside_selection_in_declaration_order() {
    let stream = load_only_tx_stream("[9, 1..4, 7]");
    assert_eq!(
        stream.source_channels,
        vec![9, 1, 2, 3, 4, 7],
        "`1..4` must expand inclusive and stay between the 9 and the 7"
    );
}

/// No index means "the whole interface" — an empty selection, not a fabricated one.
#[test]
fn load_absent_index_yields_empty_selection() {
    let stream = load_only_tx_stream("");
    assert!(
        stream.source_channels.is_empty(),
        "absent index must load as an empty selection, got {:?}",
        stream.source_channels
    );
}

/// `[auto]` carries no channel meaning in a stream source, so it flattens away.
/// The drop is deliberate; Phase 4 makes it loud with an `Info` diagnostic.
#[test]
fn load_auto_index_yields_empty_selection() {
    let stream = load_only_tx_stream("[auto]");
    assert!(
        stream.source_channels.is_empty(),
        "`[auto]` must load as an empty selection, got {:?}",
        stream.source_channels
    );
}

// ---------------------------------------------------------------------------
// Full round trip: canvas → emit → format → parse → load → canvas
// ---------------------------------------------------------------------------

/// Emit a one-stream canvas payload, re-format the text, and load it back.
///
/// The interface is `io`, so emit resolves it through `directional_port_name` to
/// `AES67_Out` while load returns the raw `source.port`. Asserting `port_name` as
/// well as the selection is what pins those two halves together — a selection-only
/// assertion would let an `_In`/`_Out` regression ride through green.
fn roundtrip_tx_stream(channel_count: u32, source_channels: Vec<u32>) -> StreamOutput {
    let mut inst = make_simple_instance(
        "DSP",
        "QSys_Core",
        "QSC",
        vec![make_interface("aes67", "AES67", "io", Some("AES67"), 8, vec![])],
    );
    inst.tx_streams = vec![StreamEmitInput {
        label: "Talkback".into(),
        protocol: "AES67".into(),
        channel_count,
        interface_id: "aes67".into(),
        source_channels,
    }];
    let emitted = emit_checked(
        CanvasEmitInput {
            instances: vec![inst],
            connections: vec![],
            manufacturer_cards: vec![],
        },
        "stream selection roundtrip",
    );
    let formatted = crate::formatter::format_source(&emitted)
        .unwrap_or_else(|e| panic!("format must succeed: {e}\n{emitted}"));
    let loaded = load_from_patch(&formatted, "")
        .unwrap_or_else(|e| panic!("load must succeed: {e}\n{formatted}"));
    let inst = loaded
        .instances
        .iter()
        .find(|i| i.name == "DSP")
        .expect("instance DSP must survive the round trip");
    assert_eq!(inst.tx_streams.len(), 1, "expected exactly one TX stream");
    inst.tx_streams[0].clone()
}

#[test]
fn roundtrip_returns_the_identical_ordered_selection() {
    let stream = roundtrip_tx_stream(4, vec![7, 1, 5, 3]);
    assert_eq!(
        stream.source_channels,
        vec![7, 1, 5, 3],
        "the selection must survive emit → format → parse → load in order"
    );
    assert_eq!(
        stream.port_name, "AES67_Out",
        "a TX stream on an io interface must round-trip to the _Out port name"
    );
    assert_eq!(
        stream.channel_count, 4,
        "channels must match the selection width"
    );
}

/// The single-channel case is the one most likely to ship broken: it is legal
/// (a lone talkback or timecode flow), and it fails silently rather than loudly.
#[test]
fn roundtrip_single_channel_selection_survives() {
    let stream = roundtrip_tx_stream(1, vec![3]);
    assert_eq!(
        stream.source_channels,
        vec![3],
        "a 1-channel selection must round-trip as [3], not empty and not [1]"
    );
    assert_eq!(
        stream.port_name, "AES67_Out",
        "a 1-channel selection must not corrupt the port name"
    );
}

/// A stream with no selection must still round-trip its port name and count —
/// the pre-#37 behaviour that every existing canvas file depends on.
#[test]
fn roundtrip_without_selection_is_unchanged() {
    let stream = roundtrip_tx_stream(8, vec![]);
    assert!(
        stream.source_channels.is_empty(),
        "no selection must stay no selection, got {:?}",
        stream.source_channels
    );
    assert_eq!(stream.port_name, "AES67_Out", "port name must survive");
    assert_eq!(stream.channel_count, 8, "channel count must survive");
}
