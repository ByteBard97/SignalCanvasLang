//! Canvas emit tests: connections, dedup, streams, and instance routes.

use super::canvas_test_helpers::*;
use crate::builder::canvas_emit::emit_from_canvas_input;
use crate::builder::canvas_input::*;

#[test]
fn emit_connection_between_instances() {
    let iface_out = make_interface("d_out", "Dante_Pri", "io", Some("Dante"), 32, vec![]);
    let iface_in = make_interface("d_in", "Dante_Pri", "io", Some("Dante"), 32, vec![]);
    let input = CanvasEmitInput {
        instances: vec![
            make_simple_instance("Stage_Left", "Rio3224", "Yamaha", vec![iface_out]),
            make_simple_instance("FOH_Console", "CL5", "Yamaha", vec![iface_in]),
        ],
        connections: vec![ConnectionEmitInput {
            from_instance_name: "Stage_Left".into(),
            to_instance_name: "FOH_Console".into(),
            from_port_id: "Dante_Pri_Out".into(),
            to_port_id: "Dante_Pri_In".into(),
            is_backbone: false,
            channel_mappings: vec![],
            properties: vec![KvEmitInput {
                key: "cable".into(),
                value: "Cat6a".into(),
            }],
        }],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("connect Stage_Left.Dante_Pri_Out -> FOH_Console.Dante_Pri_In"),
        "should emit connect statement:\n{patch}"
    );
}

#[test]
fn emit_deduplicates_templates_for_same_model() {
    let input = CanvasEmitInput {
        instances: vec![
            make_simple_instance("Console_1", "CL5", "Yamaha", vec![]),
            make_simple_instance("Console_2", "CL5", "Yamaha", vec![]),
        ],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    let count = patch.matches("template CL5").count();
    assert_eq!(count, 1, "should deduplicate templates:\n{patch}");
    assert!(
        patch.contains("instance Console_1 is CL5"),
        "should emit both instances:\n{patch}"
    );
    assert!(
        patch.contains("instance Console_2 is CL5"),
        "should emit both instances:\n{patch}"
    );
}

// ---------------------------------------------------------------------------
// Stream emit — chassis and card-slot ports
// ---------------------------------------------------------------------------

#[test]
fn emit_stream_on_chassis_port_is_included() {
    let mut inst = make_simple_instance(
        "Stage_Left",
        "Rio3224",
        "Yamaha",
        vec![make_interface(
            "dante_pri",
            "Dante_Pri",
            "io",
            Some("Dante"),
            32,
            vec!["primary"],
        )],
    );
    inst.tx_streams = vec![StreamEmitInput {
        label: "SL_Dante_TX".into(),
        protocol: "Dante".into(),
        channel_count: 32,
        interface_id: "dante_pri".into(),
    }];
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("stream SL_Dante_TX"),
        "stream on chassis port must be emitted:\n{patch}"
    );
}

#[test]
fn emit_stream_on_card_slot_port_is_not_dropped() {
    // Riedel Artist 64 with an AES67-108 G2 card in slot 1.
    // The stream's interface_id points to the card's interface, not the chassis.
    // Bug: emit_streams_for only searches inst.interfaces and silently drops the stream.
    let card = CardEmitInput {
        template_name: "AES67_108_G2".into(),
        manufacturer: Some("Riedel".into()),
        model: "AES67-108 G2".into(),
        fits: "Artist_Slot".into(),
        interfaces: vec![make_interface(
            "card_aes67_out",
            "AES67_Out",
            "out",
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
    inst.tx_streams = vec![StreamEmitInput {
        label: "Artist_AES67_TX".into(),
        protocol: "AES67".into(),
        channel_count: 64,
        interface_id: "card_aes67_out".into(),
    }];
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![card],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("stream Artist_AES67_TX"),
        "AES67 stream on card-slot port must not be silently dropped:\n{patch}"
    );
}

#[test]
fn emit_instance_route_via_card_slot_port() {
    // Riedel Artist 64 with AES67-108 G2 card in slot 1.
    // Route from card's AES67_Out[1] to chassis Mgmt[1].
    // Bug: build_instance_routes only searches inst.interfaces (chassis)
    // and emits wrong port name for card-slot interfaces.
    let card = CardEmitInput {
        template_name: "AES67_108_G2".into(),
        manufacturer: Some("Riedel".into()),
        model: "AES67-108 G2".into(),
        fits: "Artist_Slot".into(),
        interfaces: vec![make_interface(
            "card_aes67_out",
            "AES67_Out",
            "out",
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
    inst.instance_routes = vec![RouteRuleEmitInput {
        from_interface: "card_aes67_out".into(),
        from_channel: 1,
                from_start: 1,
                from_end: 1,
        from_instance: None,
        to_interface: "mgmt".into(),
        to_channel: 1,
                to_start: 1,
                to_end: 1,
        to_instance: None,
    }];
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![card],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("route AES67_Out[1] -> Mgmt_Out[1]"),
        "route on card-slot port must use correct directional port name, got:\n{patch}"
    );
    assert!(
        !patch.contains("route card_aes67_out"),
        "route must NOT emit raw interface id as port name:\n{patch}"
    );
}

// ---------------------------------------------------------------------------
// RX stream port direction
// ---------------------------------------------------------------------------

/// TX streams (data leaving the device) must reference the _Out port.
/// RX streams (data arriving at the device) must reference the _In port.
/// Bug: emit_streams_for used PortSide::Output for both, so RX streams were
/// emitted with the wrong port name (e.g. `source: FOH.Dante_Pri_Out`
/// instead of `source: FOH.Dante_Pri_In`).
#[test]
fn emit_rx_stream_uses_input_port_name() {
    let mut inst = make_simple_instance(
        "FOH_Console",
        "CL5",
        "Yamaha",
        vec![make_interface(
            "dante_pri",
            "Dante_Pri",
            "io",
            Some("Dante"),
            72,
            vec!["primary"],
        )],
    );
    inst.rx_streams = vec![StreamEmitInput {
        label: "FOH_Dante_RX".into(),
        protocol: "Dante".into(),
        channel_count: 72,
        interface_id: "dante_pri".into(),
    }];
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("stream FOH_Dante_RX"),
        "RX stream must be emitted:\n{patch}"
    );
    assert!(
        patch.contains("source: FOH_Console.Dante_Pri_In"),
        "RX stream must reference the _In port:\n{patch}"
    );
    assert!(
        !patch.contains("source: FOH_Console.Dante_Pri_Out"),
        "RX stream must NOT reference the _Out port:\n{patch}"
    );
}

#[test]
fn emit_tx_stream_uses_output_port_name() {
    let mut inst = make_simple_instance(
        "Stage_Left",
        "Rio3224",
        "Yamaha",
        vec![make_interface(
            "dante_pri",
            "Dante_Pri",
            "io",
            Some("Dante"),
            32,
            vec!["primary"],
        )],
    );
    inst.tx_streams = vec![StreamEmitInput {
        label: "SL_Dante_TX".into(),
        protocol: "Dante".into(),
        channel_count: 32,
        interface_id: "dante_pri".into(),
    }];
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("source: Stage_Left.Dante_Pri_Out"),
        "TX stream must reference the _Out port:\n{patch}"
    );
    assert!(
        !patch.contains("source: Stage_Left.Dante_Pri_In"),
        "TX stream must NOT reference the _In port:\n{patch}"
    );
}

// ---------------------------------------------------------------------------
// Card-slot coverage: connections, buses, bridges
// ---------------------------------------------------------------------------


