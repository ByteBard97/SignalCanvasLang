//! Canvas emit tests: templates, ports, and channel labels.

use super::canvas_test_helpers::*;
use crate::builder::canvas_emit::emit_from_canvas_input;
use crate::builder::canvas_input::*;
use std::collections::HashMap;

#[test]
fn canvas_emit_input_deserializes_empty() {
    let json = r#"{"instances":[],"connections":[],"manufacturer_cards":[]}"#;
    let input: CanvasEmitInput = serde_json::from_str(json).unwrap();
    assert_eq!(input.instances.len(), 0);
    assert_eq!(input.connections.len(), 0);
}

#[test]
fn canvas_emit_input_deserializes_instance_with_interfaces() {
    let json = r#"{
        "instances": [{
            "name": "FOH_Console",
            "device_type": "device",
            "manufacturer": "Yamaha",
            "model": "CL5",
            "category": "Console",
            "kind": null,
            "location": null,
            "dante_chipset": null,
            "rf_subtype": null,
            "rf_min_channels": null,
            "rf_max_channels": null,
            "rf_band": null,
            "interfaces": [{
                "id": "dante_pri",
                "label": "Dante_Pri",
                "direction": "io",
                "connector": "etherCON",
                "transport": "Dante",
                "channel_count": 32,
                "attributes": ["primary"]
            }],
            "card_slot_groups": [],
            "installed_cards": [],
            "channel_labels": {},
            "route_rules": [],
            "instance_routes": [],
            "internal_buses": [],
            "tx_streams": [],
            "rx_streams": [],
            "is_ring_container": false,
            "ring_protocol": null,
            "ring_members": [],
            "rf_active_channels": null,
            "iem_modes": null
        }],
        "connections": [],
        "manufacturer_cards": []
    }"#;
    let input: CanvasEmitInput = serde_json::from_str(json).unwrap();
    assert_eq!(input.instances.len(), 1);
    assert_eq!(input.instances[0].name, "FOH_Console");
    assert_eq!(input.instances[0].interfaces.len(), 1);
    assert_eq!(
        input.instances[0].interfaces[0].transport.as_deref(),
        Some("Dante")
    );
}

#[test]
fn emit_produces_template_and_instance() {
    let input = CanvasEmitInput {
        instances: vec![make_simple_instance(
            "FOH_Console",
            "CL5",
            "Yamaha",
            vec![make_interface(
                "d1",
                "Dante_Pri",
                "io",
                Some("Dante"),
                32,
                vec!["primary"],
            )],
        )],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("template CL5"),
        "should emit template:\n{patch}"
    );
    assert!(
        patch.contains("instance FOH_Console is CL5"),
        "should emit instance:\n{patch}"
    );
}

#[test]
fn emit_splits_dante_io_into_in_and_out() {
    let input = CanvasEmitInput {
        instances: vec![make_simple_instance(
            "Stage_Left",
            "Rio3224",
            "Yamaha",
            vec![make_interface(
                "d1",
                "Dante_Pri",
                "io",
                Some("Dante"),
                32,
                vec!["primary"],
            )],
        )],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("Dante_Pri_In[1..32]: in"),
        "Dante io must split to _In:\n{patch}"
    );
    assert!(
        patch.contains("Dante_Pri_Out[1..32]: out"),
        "Dante io must split to _Out:\n{patch}"
    );
    assert!(
        !patch.contains("Dante_Pri: io"),
        "must NOT emit unsplit io port:\n{patch}"
    );
}

#[test]
fn emit_optocore_stays_as_io() {
    let input = CanvasEmitInput {
        instances: vec![make_simple_instance(
            "CL5_1",
            "CL5",
            "Yamaha",
            vec![make_interface(
                "opt1",
                "OptoCore_A",
                "io",
                Some("OptoCore"),
                0,
                vec![],
            )],
        )],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("OptoCore_A: io"),
        "OptoCore must stay as io:\n{patch}"
    );
    assert!(
        !patch.contains("OptoCore_A_In") && !patch.contains("OptoCore_A_Out"),
        "OptoCore must not split:\n{patch}"
    );
}

#[test]
fn emit_channel_labels_appear_in_config_block() {
    let mut labels = HashMap::new();
    labels.insert(
        "d1".to_string(),
        vec![
            ChannelLabelEmitInput {
                channel_index: 1,
                label: "Lead Vocal".into(),
                phantom: true,
                propagated: false,
                source_type: None,
                capsule: None,
                rf_band: None,
                ..Default::default()
            },
            ChannelLabelEmitInput {
                channel_index: 2,
                label: "Kick Drum".into(),
                phantom: false,
                propagated: false,
                source_type: None,
                capsule: None,
                rf_band: None,
                ..Default::default()
            },
        ],
    );
    let mut inst = make_simple_instance(
        "FOH_Console",
        "CL5",
        "Yamaha",
        vec![make_interface(
            "d1",
            "Dante_Pri",
            "io",
            Some("Dante"),
            32,
            vec![],
        )],
    );
    inst.channel_labels = labels;
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("config FOH_Console"),
        "should emit config block:\n{patch}"
    );
    assert!(
        patch.contains("Lead Vocal"),
        "label text must appear:\n{patch}"
    );
    assert!(
        patch.contains("phantom"),
        "phantom flag must appear:\n{patch}"
    );
}

#[test]
fn emit_channel_label_on_card_slot_port() {
    // Riedel Artist 64 with AES67-108 G2 card in slot 1.
    // Label on card's AES67[1] = "Main Mix L".
    // Bug: emit only searches chassis interfaces, falls through to sanitize_id,
    // then builder.set_label() throws PortNotFound and aborts the entire emit.
    let card = CardEmitInput {
        template_name: "AES67_108_G2".into(),
        manufacturer: Some("Riedel".into()),
        model: "AES67-108 G2".into(),
        fits: "Artist_Slot".into(),
        interfaces: vec![make_interface(
            "card_aes67",
            "AES67",
            "io",
            Some("AES67"),
            64,
            vec![],
        )],
    };
    let mut inst = make_simple_instance(
        "Artist_64",
        "Artist64",
        "Riedel",
        vec![make_interface(
            "mgmt",
            "Mgmt",
            "io",
            Some("Ethernet_Mgmt"),
            0,
            vec![],
        )],
    );
    inst.installed_cards = vec![InstalledCardEmitInput {
        slot_label: "Card_Slot".into(),
        slot_index: 1,
        card_template_name: "AES67_108_G2".into(),
    }];
    let mut labels = HashMap::new();
    labels.insert(
        "card_aes67".into(),
        vec![ChannelLabelEmitInput {
            channel_index: 1,
            label: "Main Mix L".into(),
            phantom: false,
            propagated: false,
            source_type: None,
            capsule: None,
            rf_band: None,
            ..Default::default()
        }],
    );
    inst.channel_labels = labels;
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![card],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("config Artist_64"),
        "config block must be emitted:\n{patch}"
    );
    assert!(
        patch.contains("Main Mix L"),
        "label text must appear:\n{patch}"
    );
    assert!(
        patch.contains("label AES67_In[1]: \"Main Mix L\""),
        "label must use correct directional port name from card interface:\n{patch}"
    );
    assert!(
        !patch.contains("card_aes67"),
        "label must NOT emit raw interface id as port name:\n{patch}"
    );
}

