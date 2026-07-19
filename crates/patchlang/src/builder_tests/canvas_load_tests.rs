//! Canvas-load tests: unknown templates, streams, bus drop-on-load, named-output roundtrip.
use crate::builder::canvas_emit::emit_from_canvas_input;
use crate::builder::canvas_input::*;
use crate::builder::canvas_load::load_from_patch;
use super::canvas_load_helpers::*;

#[test]
fn load_instance_with_unknown_template_returns_error() {
    let patch = r#"
instance FOH is UnknownConsole {
  location: "Front of House"
}
"#;
    let result = load_from_patch(patch, "");
    assert!(
        result.is_err(),
        "expected error for unknown template, got Ok with {} instances",
        result.as_ref().map(|o| o.instances.len()).unwrap_or(0)
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("UnknownConsole"),
        "error should name the missing template, got: {err}"
    );
}

/// Multiple instances, some with known templates and some with unknown — the
/// first unknown template encountered must trigger an error rather than being
/// silently discarded.
#[test]
fn load_mixed_known_and_unknown_templates_returns_error() {
    let patch = r#"
template Rio3224 {
  meta { manufacturer: "Yamaha", model: "Rio3224" }
  ports {
    Dante_Pri_Out[1..32]: out(etherCON) [Dante, primary]
  }
}
instance SL is Rio3224
instance FOH is UndefinedConsole {
  location: "Front of House"
}
"#;
    let result = load_from_patch(patch, "");
    assert!(
        result.is_err(),
        "expected error for unknown template 'UndefinedConsole'"
    );
}

// ---------------------------------------------------------------------------
// C4 — Streams orphaned when source has no instance qualifier
// ---------------------------------------------------------------------------

/// A stream declaration with no `source:` line has no instance to attach to.
/// This is invalid PatchLang — the canvas can't display a sourceless stream.
/// Previously the stream was silently dropped; now it must return an error.
#[test]
fn load_stream_with_no_source_is_not_silently_dropped() {
    let patch = r#"
template Rio3224 {
  meta { manufacturer: "Yamaha", model: "Rio3224" }
  ports {
    Dante_Pri_Out[1..32]: out(etherCON) [Dante, primary]
  }
}
instance SL is Rio3224

stream My_Stream {
  channels: 32
  protocol: "Dante"
}
"#;
    // A sourceless stream can't be attached to any instance — must be an error.
    let result = load_from_patch(patch, "");
    assert!(
        result.is_err(),
        "stream with no source should return an error, not silently drop"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("My_Stream"),
        "error should name the orphaned stream, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Bus corruption from old saves — output_port = "Unknown"
//
// Old TypeScript code wrote `output "Name": Unknown[n]` as a fallback when an
// interface ID couldn't be resolved. load_from_patch must not return these as
// real destinations — they should be dropped (output_port: "", channels: []).
//
// These tests FAIL today because load_from_patch faithfully round-trips the
// "Unknown" port name. The fix belongs in load_from_patch: cross-reference
// bus output port names against the template's declared ports and drop any
// destination that doesn't match a real port.
// ---------------------------------------------------------------------------

/// Regression: a bus output with a single `Unknown` destination (written by old
/// TS code) must NOT survive load — it must come back with output_port="" and
/// no channels. Currently FAILS because load_from_patch returns output_port="Unknown".
#[test]
fn bus_output_unknown_dest_is_dropped_on_load() {
    let patch = r#"
template Core_250i {
  meta { manufacturer: "QSC", model: "Core 250i" }
  ports {
    QLAN_Out[1..128]: out [QLAN]
    AES67_In[1..64]: in [AES67]
  }
}
instance Core is Core_250i {
  bus Main_Bus {
    input: AES67_In[1]
    output "Ch 1": Unknown[1]
  }
}
"#;
    let loaded = load_from_patch(patch, "").expect("load must succeed");
    let named_outputs = &loaded.instances[0].internal_buses[0].named_outputs;
    assert!(
        named_outputs.is_empty(),
        "garbage-only output must be dropped entirely, got {:?}",
        named_outputs
    );
}

/// Regression: a bus with 1 real output + 11 phantom Unknown outputs (the exact
/// shape Reid saw) must load as exactly 1 named_output with the real port, not 12.
/// Currently FAILS — load_from_patch returns all 12 entries faithfully.
#[test]
fn bus_phantom_unknown_outputs_are_dropped_on_load() {
    // Build .patch with 1 real output and 11 Unknown phantoms (channels 2-12)
    let mut outputs = String::from("    output \"Ch 1\": QLAN_Out[1]\n");
    for ch in 2..=12 {
        outputs.push_str(&format!("    output \"Ch {ch}\": Unknown[{ch}]\n"));
    }

    let patch = format!(r#"
template Core_250i {{
  meta {{ manufacturer: "QSC", model: "Core 250i" }}
  ports {{
    QLAN_Out[1..128]: out [QLAN]
    AES67_In[1..64]: in [AES67]
  }}
}}
instance Core is Core_250i {{
  bus Main_Bus {{
    input: AES67_In[1]
{outputs}  }}
}}
"#);

    let loaded = load_from_patch(&patch, "").expect("load must succeed");
    let named_outputs = &loaded.instances[0].internal_buses[0].named_outputs;

    assert_eq!(
        named_outputs.len(),
        1,
        "phantom Unknown outputs must be dropped — expected 1, got {}:\n{named_outputs:#?}",
        named_outputs.len()
    );
    assert_eq!(named_outputs[0].output_port, "QLAN_Out");
    assert_eq!(named_outputs[0].output_channels, vec![1]);
}

/// Variant: "Device" was the other garbage sentinel — sanitize_id("") returns
/// "Device". Old code using the empty-interface fallback would write
/// `output "Name": Device[1]`. Must be treated the same as "Unknown".
#[test]
fn bus_output_device_sentinel_dest_is_dropped_on_load() {
    let patch = r#"
template Core_250i {
  meta { manufacturer: "QSC", model: "Core 250i" }
  ports {
    QLAN_Out[1..128]: out [QLAN]
    AES67_In[1..64]: in [AES67]
  }
}
instance Core is Core_250i {
  bus Main_Bus {
    input: AES67_In[1]
    output "Ch 1": Device[1]
  }
}
"#;
    let loaded = load_from_patch(patch, "").expect("load must succeed");
    let named_outputs = &loaded.instances[0].internal_buses[0].named_outputs;
    assert!(
        named_outputs.is_empty(),
        "Device sentinel output must be dropped entirely, got {:?}",
        named_outputs
    );
}

/// Variant: a single output with a mix of one real destination and one Unknown
/// phantom — the phantom channel must be stripped, leaving only the real channel.
#[test]
fn bus_output_mixed_real_and_unknown_channels_strips_phantom() {
    let patch = r#"
template Core_250i {
  meta { manufacturer: "QSC", model: "Core 250i" }
  ports {
    QLAN_Out[1..128]: out [QLAN]
    AES67_In[1..64]: in [AES67]
  }
}
instance Core is Core_250i {
  bus Main_Bus {
    input: AES67_In[1]
    output "Ch 1": QLAN_Out[1], Unknown[2]
  }
}
"#;
    let loaded = load_from_patch(patch, "").expect("load must succeed");
    let out = &loaded.instances[0].internal_buses[0].named_outputs[0];
    assert_eq!(out.output_port, "QLAN_Out");
    assert_eq!(
        out.output_channels, vec![1],
        "phantom channel 2 (Unknown) must be stripped, got {:?}",
        out.output_channels
    );
}

/// Variant: bus input port with a garbage name must be treated as unrouted
/// (input_port: "", input_channels: []).
#[test]
fn bus_input_unknown_port_is_dropped_on_load() {
    let patch = r#"
template Core_250i {
  meta { manufacturer: "QSC", model: "Core 250i" }
  ports {
    QLAN_Out[1..128]: out [QLAN]
    AES67_In[1..64]: in [AES67]
  }
}
instance Core is Core_250i {
  bus Main_Bus {
    input: Unknown[1]
    output "Ch 1": QLAN_Out[1]
  }
}
"#;
    let loaded = load_from_patch(patch, "").expect("load must succeed");
    let bus = &loaded.instances[0].internal_buses[0];
    assert_eq!(
        bus.input_port, "",
        "Unknown input port must be dropped, got {:?}",
        bus.input_port
    );
    assert!(
        bus.input_channels.is_empty(),
        "Unknown input port must yield empty channels, got {:?}",
        bus.input_channels
    );
}

// ---------------------------------------------------------------------------
// Bus named-output round-trip — emit_from_canvas_input → load_from_patch
//
// These tests reproduce Reid's bug: AES67/QLAN stream channel labels vanish
// after save+reload because bus named_outputs don't survive the round-trip.
// The corruption manifests as output_port = "" or "Unknown" (old projects)
// and duplicate/phantom entries in named_outputs.
// ---------------------------------------------------------------------------

/// Chassis port: one named output → one destination survives emit → load.
/// If this fails, the Rust load path is corrupting named_outputs for plain
/// port names (QLAN_Out, AES67_Out, etc).
#[test]
fn bus_named_output_chassis_port_survives_roundtrip() {
    let mut inst = make_inst(
        "Core",
        "Core_250i",
        vec![
            make_iface("aes67_in", "AES67_In", "in", 64),
            make_iface("qlan_out", "QLAN_Out", "out", 128),
        ],
    );
    inst.internal_buses = vec![BusEmitInput {
        label: "Main_Bus".into(),
        display_name: None,
        input_interface: "AES67_In".into(),
        input_channels: vec![1, 2],
        input_groups: vec![],
output_interface: "".into(),
        output_channels: vec![],
        named_outputs: vec![
            BusOutputEmitInput { instance: None, name: "Ch 1".into(), interface: "QLAN_Out".into(), channels: vec![1] },
            BusOutputEmitInput { instance: None, name: "Ch 2".into(), interface: "QLAN_Out".into(), channels: vec![2] },
        ],
    }];
    let patch = emit_from_canvas_input(CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![],
    }).expect("emit must succeed");

    // Sanity: emitted patch must not contain "Unknown"
    assert!(
        !patch.contains("Unknown"),
        "emitted patch must not contain 'Unknown' port refs:\n{patch}"
    );

    let loaded = load_from_patch(&patch, "").expect("load must succeed");
    let buses = &loaded.instances[0].internal_buses;
    assert_eq!(buses.len(), 1, "expected 1 bus after round-trip, got {}", buses.len());

    let outputs = &buses[0].named_outputs;
    assert_eq!(
        outputs.len(),
        2,
        "expected 2 named_outputs after round-trip, got {} — phantom entries detected:\n{outputs:#?}",
        outputs.len()
    );
    assert_eq!(outputs[0].name, "Ch 1");
    assert_eq!(outputs[0].output_port, "QLAN_Out", "output_port corrupted to {:?}", outputs[0].output_port);
    assert_eq!(outputs[0].output_channels, vec![1]);

    assert_eq!(outputs[1].name, "Ch 2");
    assert_eq!(outputs[1].output_port, "QLAN_Out");
    assert_eq!(outputs[1].output_channels, vec![2]);
}

/// Card-slot port: bus named output targeting a slot-qualified port name must
/// also survive the full round-trip without corruption.
#[test]
fn bus_named_output_card_slot_port_survives_roundtrip() {
    use crate::builder::canvas_input::{CardEmitInput, CardSlotGroupEmitInput, InstalledCardEmitInput};

    let aes67_card = CardEmitInput {
        template_name: "AES67_108_G2".into(),
        manufacturer: Some("QSC".into()),
        model: "AES67 108 G2".into(),
        fits: "QSC_Slot".into(),
        interfaces: vec![
            make_iface("aes67_out", "AES67_Out", "out", 108),
        ],
    };

    let mut inst = make_inst(
        "Core",
        "Core_250i",
        vec![
            make_iface("qlan_in", "QLAN_In", "in", 128),
        ],
    );
    inst.card_slot_groups = vec![CardSlotGroupEmitInput {
        label: "Client".into(),
        slot_count: 4,
        slot_format: "QSC_Slot".into(),
        direction: "io".into(),
        channel_count: 128,
    }];
    inst.installed_cards = vec![InstalledCardEmitInput {
        slot_label: "Client".into(),
        slot_index: 1,
        card_template_name: "AES67_108_G2".into(),
    }];

    // The TypeScript pre-resolves compound slot IDs to slot-qualified port names
    // before calling emit. Simulate that: "AES67_Out__Client_1" is the resolved name.
    inst.internal_buses = vec![BusEmitInput {
        label: "AES67_Bus".into(),
        display_name: None,
        input_interface: "QLAN_In".into(),
        input_channels: vec![1],
        input_groups: vec![],
output_interface: "".into(),
        output_channels: vec![],
        named_outputs: vec![
            BusOutputEmitInput { instance: None,
                name: "AES67 Ch 1".into(),
                interface: "AES67_Out__Client_1".into(),
                channels: vec![1],
            },
        ],
    }];

    let patch = emit_from_canvas_input(CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![aes67_card],
    }).expect("emit must succeed");

    assert!(
        !patch.contains("Unknown"),
        "emitted patch must not contain 'Unknown':\n{patch}"
    );
    assert!(
        patch.contains("AES67_Out__Client_1") || patch.contains("AES67_Out"),
        "emitted patch must reference the AES67 port:\n{patch}"
    );

    let loaded = load_from_patch(&patch, "").expect("load must succeed");
    let buses = &loaded.instances[0].internal_buses;
    assert_eq!(buses.len(), 1);
    let outputs = &buses[0].named_outputs;
    assert_eq!(
        outputs.len(),
        1,
        "expected 1 named_output, got {} — phantom entries detected:\n{outputs:#?}",
        outputs.len()
    );
    assert_eq!(outputs[0].name, "AES67 Ch 1");
    assert!(
        !outputs[0].output_port.is_empty(),
        "output_port must not be empty after round-trip"
    );
    assert!(
        outputs[0].output_port != "Unknown",
        "output_port must not be 'Unknown'"
    );
    assert_eq!(outputs[0].output_channels, vec![1]);
}

