//! Canvas-load tests: cross-device bus outputs/inputs load + roundtrip.
use crate::builder::canvas_emit::emit_from_canvas_input;
use crate::builder::canvas_input::*;
use crate::builder::canvas_load::load_from_patch;
use super::canvas_load_helpers::*;

// ---------------------------------------------------------------------------
// Cross-device bus destinations (Reid's bug)
//
// A bus output (or input) that targets a port on a *different* device is
// represented in .patch as `Instance.Port[n]` — a fully-qualified PortRef
// with `instance: Some("GX4816")`.  load_from_patch's is_valid_port closure
// currently checks only the port component against the *owning* instance's
// template, so "DX_2_Out" is not found in the Avantis template and the whole
// destination is silently dropped.
//
// These tests are RED today.  They will turn GREEN once is_valid_port is
// updated to treat any PortRef with instance.is_some() as always intentional.
// ---------------------------------------------------------------------------

/// The simplest case: a single cross-device bus output destination must survive
/// load_from_patch without being filtered as a garbage sentinel.
///
/// Reproduces the exact shape Reid reported: Avantis bus routing through
/// GX4816.DX_2_Out[1].
#[test]
fn bus_cross_device_output_survives_load() {
    let patch = r#"
template GX4816 {
  meta { manufacturer: "Allen & Heath", model: "GX4816" }
  ports {
    DX_2_Out[1..4]: out
    DX_2_In[1..4]: in
  }
}
template Avantis {
  meta { manufacturer: "Allen & Heath", model: "Avantis" }
  ports {
    AES_In[1..16]: in
    AES_Out[1..16]: out
  }
}
instance GX4816 is GX4816 {}
instance Avantis is Avantis {
  bus Drums {
    input: AES_In[1]
    output "Drums-L": GX4816.DX_2_Out[1]
  }
}
"#;
    let loaded = load_from_patch(patch, "").expect("load must succeed");
    let avantis = loaded.instances.iter().find(|i| i.name == "Avantis")
        .expect("Avantis instance must be present");
    let buses = &avantis.internal_buses;
    assert_eq!(buses.len(), 1, "Avantis must have 1 bus, got {}", buses.len());

    let named_outputs = &buses[0].named_outputs;
    assert_eq!(
        named_outputs.len(),
        1,
        "cross-device output must survive load — expected 1 named_output, got 0 (silently dropped)"
    );
    assert_eq!(named_outputs[0].name, "Drums-L");
    assert_eq!(
        named_outputs[0].output_port, "DX_2_Out",
        "output_port must be the bare port name, got {:?}",
        named_outputs[0].output_port
    );
    assert_eq!(named_outputs[0].output_channels, vec![1]);
}

/// Multi-channel cross-device output: two destinations on the same cross-device
/// port must both survive as separate channel entries on the same named output.
#[test]
fn bus_cross_device_output_multi_channel_survives_load() {
    let patch = r#"
template GX4816 {
  meta { manufacturer: "Allen & Heath", model: "GX4816" }
  ports {
    DX_2_Out[1..4]: out
  }
}
template Avantis {
  meta { manufacturer: "Allen & Heath", model: "Avantis" }
  ports {
    AES_In[1..16]: in
  }
}
instance GX4816 is GX4816 {}
instance Avantis is Avantis {
  bus Drums {
    input: AES_In[1]
    output "Drums-Stereo": GX4816.DX_2_Out[1], GX4816.DX_2_Out[2]
  }
}
"#;
    let loaded = load_from_patch(patch, "").expect("load must succeed");
    let avantis = loaded.instances.iter().find(|i| i.name == "Avantis")
        .expect("Avantis instance must be present");
    let named_outputs = &avantis.internal_buses[0].named_outputs;
    assert_eq!(
        named_outputs.len(),
        1,
        "multi-channel cross-device output must produce 1 named_output, got {}",
        named_outputs.len()
    );
    assert_eq!(
        named_outputs[0].output_channels,
        vec![1, 2],
        "both channels must survive, got {:?}",
        named_outputs[0].output_channels
    );
}

/// Cross-device bus input: `input: GX4816.DX_2_In[1]` must survive as a
/// non-empty input_port on the loaded bus — same is_valid_port bug on the
/// input side.
#[test]
fn bus_cross_device_input_survives_load() {
    let patch = r#"
template GX4816 {
  meta { manufacturer: "Allen & Heath", model: "GX4816" }
  ports {
    DX_2_In[1..4]: in
  }
}
template Avantis {
  meta { manufacturer: "Allen & Heath", model: "Avantis" }
  ports {
    AES_Out[1..16]: out
  }
}
instance GX4816 is GX4816 {}
instance Avantis is Avantis {
  bus Monitor {
    input: GX4816.DX_2_In[1]
    output "Mon-L": AES_Out[1]
  }
}
"#;
    let loaded = load_from_patch(patch, "").expect("load must succeed");
    let avantis = loaded.instances.iter().find(|i| i.name == "Avantis")
        .expect("Avantis instance must be present");
    let bus = &avantis.internal_buses[0];
    assert_eq!(
        bus.input_port, "DX_2_In",
        "cross-device input port must survive load as bare port name, got {:?}",
        bus.input_port
    );
    assert_eq!(
        bus.input_channels,
        vec![1],
        "cross-device input channel must survive, got {:?}",
        bus.input_channels
    );
}

/// Cross-device and garbage in the same bus: the cross-device destination must
/// survive while the Unknown sentinel is still correctly dropped.
#[test]
fn bus_cross_device_output_not_confused_with_garbage_sentinel() {
    let patch = r#"
template GX4816 {
  meta { manufacturer: "Allen & Heath", model: "GX4816" }
  ports {
    DX_2_Out[1..4]: out
  }
}
template Avantis {
  meta { manufacturer: "Allen & Heath", model: "Avantis" }
  ports {
    AES_In[1..4]: in
    AES_Out[1..4]: out
  }
}
instance GX4816 is GX4816 {}
instance Avantis is Avantis {
  bus Mix {
    input: AES_In[1]
    output "Real":    GX4816.DX_2_Out[1]
    output "Phantom": Unknown[1]
  }
}
"#;
    let loaded = load_from_patch(patch, "").expect("load must succeed");
    let avantis = loaded.instances.iter().find(|i| i.name == "Avantis")
        .expect("Avantis instance must be present");
    let named_outputs = &avantis.internal_buses[0].named_outputs;
    assert_eq!(
        named_outputs.len(),
        1,
        "expected exactly 1 named_output (cross-device survives, Unknown dropped), got {}:\n{named_outputs:#?}",
        named_outputs.len()
    );
    assert_eq!(named_outputs[0].name, "Real");
    assert_eq!(named_outputs[0].output_port, "DX_2_Out");
}

// ---------------------------------------------------------------------------
// Cross-device bus roundtrip — emit_from_canvas_input → load_from_patch
//
// These tests prove the full cycle: TypeScript sends a BusOutputEmitInput with
// an instance qualifier, the emitter writes `GX4816.DX_2_Out[1]` into the
// .patch file, and the loader reads it back with output_instance populated.
//
// These tests are RED today because:
//   1. BusOutputEmitInput has no `instance` field (won't compile until added)
//   2. canvas_emit.rs always emits `instance: None` on bus PortRefs
//   3. BusNamedOutput has no `output_instance` field
// ---------------------------------------------------------------------------

/// Full roundtrip: a cross-device bus output destination emitted by the Rust
/// emitter must survive load_from_patch with the instance qualifier intact.
#[test]
fn bus_cross_device_output_roundtrip() {
    let mut inst = make_inst(
        "Avantis",
        "Avantis",
        vec![make_iface("aes_in", "AES_In", "in", 16)],
    );
    // GX4816 is a separate instance — its ports are not in the Avantis template.
    inst.internal_buses = vec![BusEmitInput {
        label: "Drums".into(),
        display_name: None,
        input_interface: "AES_In".into(),
        input_channels: vec![1],
                input_groups: vec![],
                output_interface: "".into(),
        output_channels: vec![],
        named_outputs: vec![BusOutputEmitInput {
            name: "Drums-L".into(),
            instance: Some("GX4816".into()),
            interface: "DX_2_Out".into(),
            channels: vec![1],
        }],
    ..Default::default()
    }];

    let gx_inst = make_inst(
        "GX4816",
        "GX4816",
        vec![make_iface("dx2_out", "DX_2_Out", "out", 4)],
    );

    let patch = emit_from_canvas_input(CanvasEmitInput {
        instances: vec![inst, gx_inst],
        connections: vec![],
        manufacturer_cards: vec![],
    }).expect("emit must succeed");

    // The emitted patch must contain the qualified reference
    assert!(
        patch.contains("GX4816.DX_2_Out"),
        "emitted patch must contain 'GX4816.DX_2_Out', got:\n{patch}"
    );

    let loaded = load_from_patch(&patch, "").expect("load must succeed");
    let avantis = loaded.instances.iter().find(|i| i.name == "Avantis")
        .expect("Avantis must be present after roundtrip");
    let outputs = &avantis.internal_buses[0].named_outputs;
    assert_eq!(outputs.len(), 1, "named_output must survive roundtrip");
    assert_eq!(outputs[0].name, "Drums-L");
    assert_eq!(outputs[0].output_port, "DX_2_Out");
    assert_eq!(
        outputs[0].output_instance,
        Some("GX4816".into()),
        "output_instance must be populated after roundtrip"
    );
    assert_eq!(outputs[0].output_channels, vec![1]);
}

/// Multi-channel cross-device roundtrip: both channels must survive.
#[test]
fn bus_cross_device_output_multi_channel_roundtrip() {
    let mut inst = make_inst(
        "Avantis",
        "Avantis",
        vec![make_iface("aes_in", "AES_In", "in", 16)],
    );
    inst.internal_buses = vec![BusEmitInput {
        label: "Drums".into(),
        display_name: None,
        input_interface: "AES_In".into(),
        input_channels: vec![1],
                input_groups: vec![],
                output_interface: "".into(),
        output_channels: vec![],
        named_outputs: vec![BusOutputEmitInput {
            name: "Drums-Stereo".into(),
            instance: Some("GX4816".into()),
            interface: "DX_2_Out".into(),
            channels: vec![1, 2],
        }],
    ..Default::default()
    }];

    let gx_inst = make_inst(
        "GX4816",
        "GX4816",
        vec![make_iface("dx2_out", "DX_2_Out", "out", 4)],
    );

    let patch = emit_from_canvas_input(CanvasEmitInput {
        instances: vec![inst, gx_inst],
        connections: vec![],
        manufacturer_cards: vec![],
    }).expect("emit must succeed");

    let loaded = load_from_patch(&patch, "").expect("load must succeed");
    let avantis = loaded.instances.iter().find(|i| i.name == "Avantis").unwrap();
    let out = &avantis.internal_buses[0].named_outputs[0];
    assert_eq!(out.output_channels, vec![1, 2], "both channels must survive roundtrip");
    assert_eq!(out.output_instance, Some("GX4816".into()));
}
