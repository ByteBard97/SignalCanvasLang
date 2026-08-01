//! Canvas → PatchLang → canvas full round-trip for inserts (issue #31).
//!
//! `tests/insert_roundtrip.rs` covers the load direction from hand-written `.patch`
//! text. This file covers the direction the frontend actually drives: a
//! `CanvasEmitInput` bundle → emitted `.patch` → `load_from_patch` → DTO, asserting
//! the insert legs come back byte-identical. That closed loop is what "sidecar no
//! longer needed" actually means.

use super::canvas_test_helpers::*;
use crate::builder::canvas_input::*;
use crate::builder::canvas_load::load_from_patch;
use crate::builder::insert_endpoints::InsertEndpoint;

fn ep(port: &str, channel: u32) -> InsertEndpoint {
    InsertEndpoint { instance: None, port: port.into(), channel }
}

fn console() -> InstanceEmitInput {
    make_simple_instance(
        "FOH_Console",
        "CL5",
        "Yamaha",
        vec![
            make_interface("mic_in", "Mic In", "in", None, 8, vec![]),
            make_interface("ext_out", "Ext Out", "out", None, 16, vec![]),
            make_interface("ext_in", "Ext In", "in", None, 16, vec![]),
        ],
    )
}

/// Emit, and assert the result parses back (#34).
///
/// The split-io payload below has empty `named_outputs` with non-empty
/// `output_channels` — the legacy fallback shape that emitted an unparseable
/// `output ""`. Checking here covers every insert test rather than one of them.
fn emit(inst: InstanceEmitInput) -> String {
    emit_checked(
        CanvasEmitInput {
            instances: vec![inst],
            connections: vec![],
            manufacturer_cards: vec![],
        },
        "insert emit",
    )
}

#[test]
fn channel_insert_survives_the_full_canvas_round_trip() {
    let mut inst = console();
    inst.channel_labels.insert(
        "mic_in".into(),
        vec![ChannelLabelEmitInput {
            channel_index: 1,
            label: "Kick".into(),
            // Scattered, independent endpoints — the ticket's real-world case.
            insert_send: vec![ep("Ext_Out", 3), ep("Ext_Out", 10)],
            insert_return: vec![ep("Ext_In", 4), ep("Ext_In", 8)],
            ..Default::default()
        }],
    );

    let patch = emit(inst);
    assert!(
        patch.contains("insert_send: \"Ext_Out[3], Ext_Out[10]\""),
        "emitted patch must carry the send legs, got:\n{patch}"
    );

    let out = load_from_patch(&patch, "").expect("load");
    let loaded = &out.instances[0].channel_labels.get("Mic_In").expect("labels")[0];
    assert_eq!(loaded.insert_send, vec![ep("Ext_Out", 3), ep("Ext_Out", 10)]);
    assert_eq!(loaded.insert_return, vec![ep("Ext_In", 4), ep("Ext_In", 8)]);
}

#[test]
fn channel_insert_leg_order_survives_the_full_round_trip() {
    let mut inst = console();
    inst.channel_labels.insert(
        "mic_in".into(),
        vec![ChannelLabelEmitInput {
            channel_index: 1,
            label: "Kick".into(),
            // Descending on purpose: anything that sorts or groups breaks this.
            insert_send: vec![ep("Ext_Out", 10), ep("Ext_Out", 3)],
            ..Default::default()
        }],
    );

    let out = load_from_patch(&emit(inst), "").expect("load");
    let loaded = &out.instances[0].channel_labels.get("Mic_In").expect("labels")[0];
    assert_eq!(
        loaded.insert_send,
        vec![ep("Ext_Out", 10), ep("Ext_Out", 3)],
        "leg order must survive emit + load — a stereo pair would swap otherwise"
    );
}

#[test]
fn unknown_label_properties_survive_the_full_round_trip() {
    // The `stand`/`gain` sidecar stopgap: these have no dedicated field and used to be
    // dropped on the next load. Now they ride the verbatim bag both ways.
    let mut label = ChannelLabelEmitInput {
        channel_index: 1,
        label: "Kick".into(),
        ..Default::default()
    };
    label.properties.insert("stand".into(), "Tall boom".into());
    label.properties.insert("gain".into(), "+12".into());
    let mut inst = console();
    inst.channel_labels.insert("mic_in".into(), vec![label]);

    let out = load_from_patch(&emit(inst), "").expect("load");
    let loaded = &out.instances[0].channel_labels.get("Mic_In").expect("labels")[0];
    assert_eq!(loaded.properties.get("stand").map(String::as_str), Some("Tall boom"));
    assert_eq!(loaded.properties.get("gain").map(String::as_str), Some("+12"));
}

#[test]
fn bus_insert_survives_the_full_canvas_round_trip() {
    let mut inst = console();
    inst.internal_buses = vec![BusEmitInput {
        label: "Main_LR".into(),
        input_interface: "mic_in".into(),
        input_channels: vec![1, 2],
        output_interface: "ext_out".into(),
        output_channels: vec![1],
        named_outputs: vec![BusOutputEmitInput {
            name: "Mix".into(),
            instance: None,
            interface: "ext_out".into(),
            channels: vec![1],
        }],
        insert_send: vec![ep("Ext_Out", 3), ep("Ext_Out", 10)],
        insert_return: vec![ep("Ext_In", 4)],
        ..Default::default()
    }];

    let patch = emit(inst);
    assert!(
        patch.contains("insert_send: Ext_Out[3], Ext_Out[10]"),
        "emitted bus block must carry the send legs, got:\n{patch}"
    );

    let out = load_from_patch(&patch, "").expect("load");
    let bus = out.instances[0].internal_buses.first().expect("bus");
    assert_eq!(bus.insert_send, vec![ep("Ext_Out", 3), ep("Ext_Out", 10)]);
    assert_eq!(bus.insert_return, vec![ep("Ext_In", 4)]);
}

/// A bidirectional interface splits into two template ports; the legs must land on the
/// right one. This is the case that decides how emit resolves an interface id.
///
/// `should_split_io` expands one io/asymmetric interface into `{base}_In` + `{base}_Out`
/// unless it is a ring/bus protocol — and MADI is not one. The ticket's canonical
/// example sends and returns on the SAME MADI interface, so the two legs must emit as
/// `MADI_Out[3]` and `MADI_In[4]`. Nothing in the endpoint DTO distinguishes them: the
/// side comes from which list the leg is in, which only the emitter knows. Sanitizing
/// the interface id alone would emit a bare `MADI[3]` that the template never declares.
#[test]
fn insert_legs_resolve_to_the_directional_port_for_a_split_io_interface() {
    let mut inst = make_simple_instance(
        "FOH_Console",
        "CL5",
        "Yamaha",
        vec![make_interface("madi", "MADI", "io", Some("MADI"), 64, vec![])],
    );
    inst.channel_labels.insert(
        "madi".into(),
        vec![ChannelLabelEmitInput {
            channel_index: 1,
            label: "Kick".into(),
            // Same interface id both ways — the side is the only thing telling them apart.
            insert_send: vec![ep("madi", 3)],
            insert_return: vec![ep("madi", 4)],
            ..Default::default()
        }],
    );
    inst.internal_buses = vec![BusEmitInput {
        label: "Main_LR".into(),
        input_interface: "madi".into(),
        input_channels: vec![1],
        output_interface: "madi".into(),
        output_channels: vec![2],
        named_outputs: vec![],
        insert_send: vec![ep("madi", 5)],
        insert_return: vec![ep("madi", 6)],
        ..Default::default()
    }];

    let patch = emit(inst);
    assert!(
        patch.contains("insert_send: \"MADI_Out[3]\""),
        "label send leg must resolve to the OUTPUT half of the split io port, got:\n{patch}"
    );
    assert!(
        patch.contains("insert_return: \"MADI_In[4]\""),
        "label return leg must resolve to the INPUT half, got:\n{patch}"
    );
    assert!(
        patch.contains("insert_send: MADI_Out[5]"),
        "bus send leg must resolve to the OUTPUT half, got:\n{patch}"
    );
    assert!(
        patch.contains("insert_return: MADI_In[6]"),
        "bus return leg must resolve to the INPUT half, got:\n{patch}"
    );
    assert!(
        !patch.contains("MADI[3]") && !patch.contains("MADI[4]"),
        "a bare MADI[n] is not a declared port — resolution must add the direction:\n{patch}"
    );
}

/// An already-resolved port name must survive untouched, so the emit path stays
/// tolerant of callers that pre-resolve (and of slot-qualified `__` compounds, which
/// deliberately match no interface).
#[test]
fn insert_legs_pass_through_names_that_match_no_interface() {
    let mut inst = console();
    inst.channel_labels.insert(
        "mic_in".into(),
        vec![ChannelLabelEmitInput {
            channel_index: 1,
            label: "Kick".into(),
            insert_send: vec![ep("Some_Card__AES_Out", 2)],
            ..Default::default()
        }],
    );
    let patch = emit(inst);
    assert!(
        patch.contains("insert_send: \"Some_Card__AES_Out[2]\""),
        "unmatched names must fall through unchanged, got:\n{patch}"
    );
}

#[test]
fn bus_insert_does_not_leak_into_bus_inputs_or_outputs() {
    // Insert legs are a DETOUR, not another source or destination. If they ever got
    // folded into `inputs`/`named_outputs` the canvas would draw phantom signal paths.
    let mut inst = console();
    inst.internal_buses = vec![BusEmitInput {
        label: "Main_LR".into(),
        input_interface: "mic_in".into(),
        input_channels: vec![1],
        output_interface: "ext_out".into(),
        output_channels: vec![1],
        named_outputs: vec![BusOutputEmitInput {
            name: "Mix".into(),
            instance: None,
            interface: "ext_out".into(),
            channels: vec![1],
        }],
        insert_send: vec![ep("Ext_Out", 7)],
        insert_return: vec![ep("Ext_In", 7)],
        ..Default::default()
    }];

    let out = load_from_patch(&emit(inst), "").expect("load");
    let bus = out.instances[0].internal_buses.first().expect("bus");
    for group in &bus.input_groups {
        assert!(
            !group.input_channels.contains(&7) || group.input_port != "Ext_In",
            "insert return leaked into bus inputs: {group:?}"
        );
    }
    for out_entry in &bus.named_outputs {
        assert!(
            !(out_entry.output_port == "Ext_Out" && out_entry.output_channels.contains(&7)),
            "insert send leaked into bus outputs: {out_entry:?}"
        );
    }
}
