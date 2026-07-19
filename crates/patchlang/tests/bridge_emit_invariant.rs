//! Guards for THE INVARIANT's emit half (docs/plans/canvas-dto-plurality.md §3).
//!
//! These exist because two defects reached a fully green Rust suite and were only
//! caught by the frontend:
//!   1. `bridge_index_for_span` matched `iface.label` against a *sanitized directional*
//!      port name, so the omit branch never fired and every bridge emitted an explicit
//!      span — including full-width ones.
//!   2. The span fields on `RouteRuleEmitInput` lacked `#[serde(default)]`, so any
//!      caller built against an older WASM got a hard "missing field `from_start`"
//!      deserialization error instead of falling back to the legacy scalar fields.
//!
//! Nothing in the existing Rust suite exercised either path. These two tests do.

use patchlang::builder::canvas_input::*;
use patchlang::builder::emit_from_canvas_input;
use std::collections::HashMap;

fn iface(id: &str, label: &str, dir: &str, channel_count: u32) -> InterfaceEmitInput {
    InterfaceEmitInput {
        id: id.to_string(),
        label: label.to_string(),
        direction: dir.to_string(),
        connector: Some("XLR".to_string()),
        transport: None,
        channel_count,
        attributes: vec![],
    }
}

fn instance_with(interfaces: Vec<InterfaceEmitInput>, rules: Vec<RouteRuleEmitInput>)
    -> InstanceEmitInput
{
    InstanceEmitInput {
        name: "SB".to_string(),
        device_type: "device".to_string(),
        manufacturer: Some("Test".to_string()),
        model: "Box".to_string(),
        category: None,
        kind: None,
        location: None,
        dante_chipset: None,
        rf_subtype: None,
        rf_min_channels: None,
        rf_max_channels: None,
        rf_band: None,
        rf_active_channels: None,
        iem_modes: None,
        interfaces,
        card_slot_groups: vec![],
        installed_cards: vec![],
        channel_labels: HashMap::new(),
        route_rules: rules,
        instance_routes: vec![],
        internal_buses: vec![],
        tx_streams: vec![],
        rx_streams: vec![],
        is_ring_container: false,
        ring_protocol: None,
        ring_members: vec![],
    }
}

/// A bridge whose span covers the WHOLE port must emit with NO index.
///
/// Regression guard for defect (1): the interface lookup used to compare against
/// `iface.label` ("Mic In") rather than the sanitized directional port name ("Mic_In"),
/// so it never matched and the full-width case emitted `Mic_In[1..16] -> Line_Out[1..16]`.
#[test]
fn full_width_bridge_emits_without_index() {
    let input = CanvasEmitInput {
        instances: vec![instance_with(
            vec![
                iface("i_mic", "Mic In", "in", 16),
                iface("i_line", "Line Out", "out", 16),
            ],
            vec![RouteRuleEmitInput {
                from_interface: "Mic_In".into(),
                from_channel: 1,
                from_start: 1,
                from_end: 16, // == channel_count → full width
                from_instance: None,
                to_interface: "Line_Out".into(),
                to_channel: 1,
                to_start: 1,
                to_end: 16, // == channel_count → full width
                to_instance: None,
            }],
        )],
        connections: vec![],
        manufacturer_cards: vec![],
    };

    let patch = emit_from_canvas_input(input).expect("emit");
    assert!(
        patch.contains("bridge Mic_In -> Line_Out"),
        "full-width span must omit the index entirely:\n{patch}"
    );
    // Scope the negative assertion to bridge lines — the port DECLARATION legitimately
    // reads `Mic_In[1..16]: in(XLR)`, so a whole-document `contains` would false-positive.
    let bridge_lines: Vec<&str> = patch
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("bridge"))
        .collect();
    assert!(
        bridge_lines.iter().all(|l| !l.contains('[')),
        "full-width span must NOT be written explicitly on the bridge line: {bridge_lines:?}"
    );
}

/// A bridge covering only PART of a port must emit the explicit span.
/// The complement of the test above — guards against over-correcting into
/// omitting indices that carry meaning.
#[test]
fn partial_width_bridge_emits_explicit_span() {
    let input = CanvasEmitInput {
        instances: vec![instance_with(
            vec![
                iface("i_mic", "Mic In", "in", 32),
                iface("i_line", "Line Out", "out", 16),
            ],
            vec![RouteRuleEmitInput {
                from_interface: "Mic_In".into(),
                from_channel: 1,
                from_start: 1,
                from_end: 16, // 16 of 32 → partial
                from_instance: None,
                to_interface: "Line_Out".into(),
                to_channel: 1,
                to_start: 1,
                to_end: 16, // 16 of 16 → full
                to_instance: None,
            }],
        )],
        connections: vec![],
        manufacturer_cards: vec![],
    };

    let patch = emit_from_canvas_input(input).expect("emit");
    assert!(
        patch.contains("bridge Mic_In[1..16] -> Line_Out"),
        "partial source span must be written, full-width target must be omitted:\n{patch}"
    );
}

/// An OLD-SHAPE payload — no span fields at all — must still deserialize.
///
/// Regression guard for defect (2). A frontend built against older WASM sends only
/// `from_channel`/`to_channel`. Without `#[serde(default)]` on the span fields this
/// fails outright with `missing field \`from_start\``, which is a hard break for every
/// client that has not upgraded in lockstep.
#[test]
fn legacy_payload_without_span_fields_still_deserializes() {
    let json = r#"{
        "instances": [{
            "name": "SB",
            "device_type": "device",
            "model": "Box",
            "interfaces": [
                {"id":"i_mic","label":"Mic In","direction":"in","channel_count":16,"attributes":[]},
                {"id":"i_line","label":"Line Out","direction":"out","channel_count":16,"attributes":[]}
            ],
            "card_slot_groups": [],
            "installed_cards": [],
            "channel_labels": {},
            "route_rules": [
                {"from_interface":"Mic_In","from_channel":1,
                 "to_interface":"Line_Out","to_channel":1}
            ],
            "instance_routes": [],
            "internal_buses": [],
            "tx_streams": [],
            "rx_streams": [],
            "is_ring_container": false,
            "ring_members": []
        }],
        "connections": [],
        "manufacturer_cards": []
    }"#;

    let parsed: Result<CanvasEmitInput, _> = serde_json::from_str(json);
    assert!(
        parsed.is_ok(),
        "legacy payload without span fields must deserialize (serde default), got: {:?}",
        parsed.err()
    );

    // And it must still emit something sane rather than silently producing nothing.
    let patch = emit_from_canvas_input(parsed.unwrap()).expect("emit from legacy payload");
    assert!(
        patch.contains("bridge Mic_In"),
        "legacy payload should still emit its bridge:\n{patch}"
    );
}

/// A bus fed from two DIFFERENT ports must load as two input groups.
///
/// Regression guard: the grouping logic was computed correctly but the `BusOutput`
/// constructor still passed the CP2 placeholder `input_groups: vec![]`, so the value
/// was dead and every consumer silently fell back to the flattened legacy fields —
/// the exact SignalCanvasLang#29 bug this workstream exists to fix. Rust emitted no
/// unused-variable warning because the vec was mutated via `push`.
#[test]
fn multi_port_bus_loads_as_separate_input_groups() {
    use patchlang::builder::canvas_load::load_from_patch;

    const SRC: &str = r#"
template CL5 {
  ports {
    Fader[1..48]: in(XLR)
    Mix_L[1..1]: in(XLR)
    Matrix_Out[1..8]: out(XLR)
  }
}
instance FOH is CL5 {
  bus Talkback {
    input: Fader[5]
    input: Mix_L[1]
    output "Out": Matrix_Out[1]
  }
}
"#;

    let out = load_from_patch(SRC, "").expect("load");
    let inst = out.instances.iter().find(|i| i.name == "FOH").expect("FOH");
    let bus = inst.internal_buses.first().expect("bus");

    assert_eq!(
        bus.input_groups.len(),
        2,
        "a bus fed from two ports must produce two input groups, got {:?}",
        bus.input_groups
    );

    let fader = bus.input_groups.iter().find(|g| g.input_port == "Fader").expect("Fader group");
    let mix = bus.input_groups.iter().find(|g| g.input_port == "Mix_L").expect("Mix_L group");
    assert_eq!(fader.input_channels, vec![5], "Fader group channels");
    assert_eq!(mix.input_channels, vec![1], "Mix_L group channels");
}

/// Many single-channel inputs on the SAME port must union into one group, not collapse
/// to the first. `input: Mix_Bus[1]` … `input: Mix_Bus[24]` is the normal shape for a
/// mix bus; keying on first-occurrence alone dropped 23 of the 24 channels.
#[test]
fn repeated_same_port_bus_inputs_union_channels() {
    use patchlang::builder::canvas_load::load_from_patch;

    let mut src = String::from(
        "template C { ports { Mix_Bus[1..24]: in(XLR)\n Out[1..2]: out(XLR) } }\ninstance I is C {\n  bus Main {\n",
    );
    for ch in 1..=24 {
        src.push_str(&format!("    input: Mix_Bus[{ch}]\n"));
    }
    src.push_str("    output \"L\": Out[1]\n  }\n}\n");

    let out = load_from_patch(&src, "").expect("load");
    let bus = out.instances[0].internal_buses.first().expect("bus");

    assert_eq!(bus.input_groups.len(), 1, "same port → one group");
    assert_eq!(
        bus.input_groups[0].input_channels,
        (1..=24).collect::<Vec<u32>>(),
        "all 24 channels must survive the union, not just the first"
    );
}

/// Connect properties must survive the load boundary (FrontendV1#202).
///
/// Only `backbone`/`kind`/`from_slot`/`to_slot` used to cross; `cable`, `length` and
/// anything else were dropped, so a round-trip stripped them from the .patch file.
#[test]
fn connect_properties_survive_load() {
    use patchlang::builder::canvas_load::load_from_patch;

    const SRC: &str = r#"
template Box {
  ports {
    Out[1..2]: out(XLR)
    In[1..2]: in(XLR)
  }
}
instance A is Box {}
instance B is Box {}
connect A.Out -> B.In {
  cable: "Cat6a_SL_Pri"
  length: "30m"
}
"#;

    let out = load_from_patch(SRC, "").expect("load");
    let conn = out.connections.first().expect("one connection");
    assert_eq!(
        conn.properties.get("cable").map(String::as_str),
        Some("Cat6a_SL_Pri"),
        "cable property must cross the DTO boundary, got {:?}",
        conn.properties
    );
    assert_eq!(conn.properties.get("length").map(String::as_str), Some("30m"));
}
