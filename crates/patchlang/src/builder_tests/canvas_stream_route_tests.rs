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
        source_channels: vec![],
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

/// Two streams whose labels sanitize to the same identifier must BOTH survive.
///
/// `add_stream` rejects duplicate names and the emitter swallowed that with
/// `continue`, so the second stream vanished from the file with no diagnostic —
/// the same silent-data-loss shape as #38. #37 makes the collision likelier: one
/// label, two different channel selections is a natural way to split a flow.
#[test]
fn emit_streams_with_colliding_labels_both_survive() {
    let mut inst = make_simple_instance(
        "DSP",
        "Rio3224",
        "Yamaha",
        vec![
            make_interface("dante_a", "Dante_A", "io", Some("Dante"), 32, vec![]),
            make_interface("dante_b", "Dante_B", "io", Some("Dante"), 32, vec![]),
        ],
    );
    // Same user-facing label, two different interfaces and channel selections.
    inst.tx_streams = vec![
        StreamEmitInput {
            label: "Drums".into(),
            protocol: "AES67".into(),
            channel_count: 2,
            interface_id: "dante_a".into(),
            source_channels: vec![1, 3],
        },
        StreamEmitInput {
            label: "Drums".into(),
            protocol: "AES67".into(),
            channel_count: 2,
            interface_id: "dante_b".into(),
            source_channels: vec![5, 7],
        },
    ];
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert_eq!(
        patch.matches("stream ").count(),
        2,
        "both streams must be emitted, neither silently dropped:\n{patch}"
    );
    // `io` interfaces split into directional ports, hence the `_Out` suffix.
    assert!(
        patch.contains("Dante_A_Out[1, 3]") && patch.contains("Dante_B_Out[5, 7]"),
        "each stream must keep its own source and selection:\n{patch}"
    );
    // The collision is resolved by renaming, not by dropping — so the second
    // stream is present under a suffixed name rather than missing.
    assert!(
        patch.contains("stream Drums ") && patch.contains("stream Drums_2 "),
        "the colliding stream must be renamed, not discarded:\n{patch}"
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
        source_channels: vec![],
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
        source_channels: vec![],
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
        source_channels: vec![],
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


// ---------------------------------------------------------------------------
// Stream source channel selection (#37, Phase 2)
// ---------------------------------------------------------------------------

/// Build a one-instance canvas payload carrying a single TX stream.
fn emit_tx_stream(channel_count: u32, source_channels: Vec<u32>) -> String {
    let mut inst = make_simple_instance(
        "DSP",
        "QSys_Core",
        "QSC",
        vec![make_interface(
            "aes67",
            "AES67",
            "io",
            Some("AES67"),
            8,
            vec![],
        )],
    );
    inst.tx_streams = vec![StreamEmitInput {
        label: "Talkback".into(),
        protocol: "AES67".into(),
        channel_count,
        interface_id: "aes67".into(),
        source_channels,
    }];
    emit_from_canvas_input(CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![],
    })
    .unwrap()
}

/// Extract the `stream <name> { ... }` block from an emitted patch, braces included.
fn stream_block(patch: &str, name: &str) -> String {
    let start = patch
        .find(&format!("stream {name} "))
        .unwrap_or_else(|| panic!("no `stream {name}` in:\n{patch}"));
    let end = patch[start..]
        .find("\n}")
        .unwrap_or_else(|| panic!("unterminated stream block in:\n{patch}"));
    patch[start..start + end + 2].to_string()
}

/// Position is semantically significant on an AES67 flow — the receiver maps by
/// position — so the selection must survive emit in the caller's order, never
/// sorted and never coalesced into a range.
#[test]
fn emit_stream_preserves_non_monotonic_channel_selection() {
    let patch = emit_tx_stream(4, vec![7, 1, 5, 3]);
    let block = stream_block(&patch, "Talkback");
    assert!(
        block.contains("source: DSP.AES67_Out[7, 1, 5, 3]"),
        "selection must be emitted in caller order:\n{block}"
    );
    assert!(
        !block.contains("[1, 3, 5, 7]"),
        "selection must NOT be sorted:\n{block}"
    );
    assert!(
        !block.contains(".."),
        "selection must NOT be coalesced into a range:\n{block}"
    );
}

/// A contiguous ascending run is still written as individual singles — coalescing
/// it into `1..4` would lose the guarantee that what comes back out is what went in.
#[test]
fn emit_stream_does_not_coalesce_a_contiguous_selection() {
    let patch = emit_tx_stream(4, vec![1, 2, 3, 4]);
    assert!(
        patch.contains("source: DSP.AES67_Out[1, 2, 3, 4]"),
        "contiguous selection must stay as singles:\n{patch}"
    );
}

/// An empty selection is "the whole interface" and must emit exactly what the
/// pre-#37 emitter wrote — every existing canvas file depends on it.
#[test]
fn emit_stream_without_selection_is_unchanged() {
    let patch = emit_tx_stream(8, vec![]);
    assert_eq!(
        stream_block(&patch, "Talkback"),
        "stream Talkback {\n  \
           source: DSP.AES67_Out\n  \
           channels: \"8\"\n  \
           direction: \"tx\"\n  \
           protocol: \"AES67\"\n}",
        "no selection must emit byte-identical output to the pre-#37 emitter:\n{patch}"
    );
}

/// With a selection present, `channels` is derived from the selection length rather
/// than echoing the frontend's `channel_count`, so a canvas-emitted file can never be
/// born self-inconsistent. Here the payload disagrees on purpose: 8 vs a 3-wide pick.
#[test]
fn emit_stream_derives_channels_from_selection_length() {
    let patch = emit_tx_stream(8, vec![2, 4, 6]);
    assert!(
        patch.contains("channels: \"3\""),
        "channels must be derived from the selection length:\n{patch}"
    );
    assert!(
        !patch.contains("channels: \"8\""),
        "the frontend's channel_count must not win over the selection:\n{patch}"
    );
}
