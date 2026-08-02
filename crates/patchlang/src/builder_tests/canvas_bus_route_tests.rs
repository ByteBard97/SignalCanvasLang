//! Canvas emit/roundtrip tests: buses, bridges, backbone, routes.

use super::canvas_test_helpers::*;
use crate::builder::canvas_emit::emit_from_canvas_input;
use crate::builder::canvas_input::*;

/// A connection from a card-slot port falls back to unvalidated AST construction
/// because the port isn't on the device template. Verify the connect statement
/// is still emitted with the correct port names.
#[test]
fn emit_connection_from_card_slot_port_is_not_dropped() {
    let dst_inst = make_simple_instance(
        "FOH_Console",
        "CL5",
        "Yamaha",
        vec![make_interface("dante_in", "Dante_Pri", "io", Some("Dante"), 64, vec![])],
    );
    let input = CanvasEmitInput {
        instances: vec![make_artist_with_card(), dst_inst],
        connections: vec![ConnectionEmitInput {
            from_instance_name: "Artist_64".into(),
            to_instance_name: "FOH_Console".into(),
            // TypeScript pre-resolves card port to directional name
            from_port_id: "AES67_Out".into(),
            to_port_id: "Dante_Pri_In".into(),
            is_backbone: false,
            channel_mappings: vec![],
            properties: vec![],
        }],
        manufacturer_cards: vec![make_aes67_card()],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("connect Artist_64.AES67_Out -> FOH_Console.Dante_Pri_In"),
        "connection from card-slot port must be emitted via fallback path:\n{patch}"
    );
}

/// A bus whose input interface is on an installed card must emit using the
/// pre-resolved port name (TypeScript resolves card interface IDs before
/// sending to the emitter).
#[test]
fn emit_bus_with_card_slot_input_port() {
    let mut inst = make_artist_with_card();
    inst.internal_buses = vec![BusEmitInput {
        label: "Card_Mix".into(),
        display_name: None,
        // TypeScript pre-resolves the card interface ID to its port name
        input_interface: "AES67_Out".into(),
        input_channels: vec![1, 2],
        input_groups: vec![],
output_interface: "AES67_Out".into(),
        output_channels: vec![3, 4],
        named_outputs: vec![],
        ..Default::default()
    }];
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![make_aes67_card()],
    };
    // Parse-checked: this payload has empty `named_outputs` with non-empty
    // `output_channels`, the shape that emitted `output ""` and could not be read back
    // (#34). It was producing that malformed text for as long as it has existed —
    // invisible because the assertions below only check containment.
    let patch = emit_checked(input, "bus on card-slot port (legacy output fallback)");
    assert!(
        patch.contains("bus Card_Mix"),
        "bus on card-slot port must be emitted:\n{patch}"
    );
    assert!(
        patch.contains("AES67_Out[1]"),
        "bus must reference the resolved card port name:\n{patch}"
    );
    assert!(
        !patch.contains("card_aes67_out"),
        "bus must NOT use raw card interface id:\n{patch}"
    );
}

/// A template bridge (route_rule) where the source port is on an installed card
/// must emit the correct directional port name. TypeScript pre-resolves card
/// interface IDs to directional names before calling the emitter.
#[test]
fn emit_bridge_with_card_slot_source_port() {
    let mut inst = make_artist_with_card();
    inst.route_rules = vec![RouteRuleEmitInput {
        // TypeScript pre-resolves card interface to directional port name
        from_interface: "AES67_Out".into(),
        from_channel: 1,
                from_start: 1,
                from_end: 1,
        from_instance: None,
        to_interface: "AES67_Out".into(),
        to_channel: 2,
                to_start: 2,
                to_end: 2,
        to_instance: None,
    }];
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![make_aes67_card()],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    // The rule is a single-channel bridge (from_start==from_end==1), so the source
    // index is written explicitly. The previous expectation omitted it, which claimed
    // the whole 64ch port as the source of a 1ch mapping — the malformed asymmetric
    // form THE INVARIANT exists to prevent. Card-slot ports aren't in `inst.interfaces`,
    // so width can't be verified and the span is always written. Port-name resolution
    // (this test's actual subject) is unchanged and still asserted below.
    assert!(
        patch.contains("bridge AES67_Out[1] -> AES67_Out[2]"),
        "bridge with card-slot port must be emitted:\n{patch}"
    );
    assert!(
        !patch.contains("card_aes67_out"),
        "bridge must NOT use raw card interface id:\n{patch}"
    );
}

// ---------------------------------------------------------------------------
// Backbone connections (D012)
// ---------------------------------------------------------------------------

/// A connection with `is_backbone: true` must emit `backbone: true` in the
/// connect body so Signal Trace can treat the pair as a transparent unit.
/// GigaACE is a ring/bus protocol — ports stay as `io` (no _In/_Out split).
#[test]
fn emit_backbone_connection_includes_backbone_property() {
    let iface = make_interface("gc", "GigaACE_Pri", "io", Some("GigaACE"), 0, vec![]);
    let iface2 = make_interface("gc", "GigaACE_Pri", "io", Some("GigaACE"), 0, vec![]);
    let input = CanvasEmitInput {
        instances: vec![
            make_simple_instance("S7000", "S7000", "Allen_Heath", vec![iface]),
            make_simple_instance("DM64", "DM64", "Allen_Heath", vec![iface2]),
        ],
        connections: vec![ConnectionEmitInput {
            from_instance_name: "S7000".into(),
            to_instance_name: "DM64".into(),
            from_port_id: "GigaACE_Pri".into(),
            to_port_id: "GigaACE_Pri".into(),
            is_backbone: true,
            channel_mappings: vec![],
            properties: vec![KvEmitInput {
                key: "cable".into(),
                value: "GigaACE_Pri".into(),
            }],
        }],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("connect S7000.GigaACE_Pri -> DM64.GigaACE_Pri"),
        "backbone connect statement must be emitted:\n{patch}"
    );
    assert!(
        patch.contains("backbone: \"true\""),
        "backbone property must appear in connect body:\n{patch}"
    );
}

/// Full roundtrip: emit a backbone connection → parse → load → assert
/// `is_backbone` survives the round trip.
#[test]
fn backbone_connection_roundtrips_is_backbone_flag() {
    use crate::builder::canvas_load::load_from_patch;

    let iface = make_interface("gc", "GigaACE_Pri", "io", Some("GigaACE"), 0, vec![]);
    let iface2 = make_interface("gc", "GigaACE_Pri", "io", Some("GigaACE"), 0, vec![]);
    let emit_input = CanvasEmitInput {
        instances: vec![
            make_simple_instance("S7000", "S7000", "Allen_Heath", vec![iface]),
            make_simple_instance("DM64",  "DM64",  "Allen_Heath", vec![iface2]),
        ],
        connections: vec![ConnectionEmitInput {
            from_instance_name: "S7000".into(),
            to_instance_name:   "DM64".into(),
            from_port_id: "GigaACE_Pri".into(),
            to_port_id:   "GigaACE_Pri".into(),
            is_backbone: true,
            channel_mappings: vec![],
            properties: vec![],
        }],
        manufacturer_cards: vec![],
    };

    let patch = emit_from_canvas_input(emit_input).unwrap();
    let loaded = load_from_patch(&patch, "{}").expect("patch must parse and load");

    let conn = loaded
        .connections
        .iter()
        .find(|c| c.from_instance == "S7000" && c.to_instance == "DM64")
        .expect("backbone connection must survive roundtrip");

    assert!(
        conn.is_backbone,
        "is_backbone must be true after roundtrip; loaded connection: {conn:?}"
    );
}

// ---------------------------------------------------------------------------
// Bus output — named outputs without wired destinations
// ---------------------------------------------------------------------------

/// A bus with named outputs that have no wired destination (empty interface)
/// must emit `output "Name"` with no port reference — not `output "Name": Unknown`.
/// Bug: build_instance_buses created PortRefs pointing to sanitize_id("") even
/// when the output had no destination, producing junk port references on reload.
#[test]
fn emit_bus_named_output_without_destination_omits_port_ref() {
    let mut inst = make_simple_instance(
        "ULTRIX_FR2",
        "ULTRIX_FR2",
        "Ross",
        vec![make_interface("madi_out", "MADI_1_Out", "out", None, 64, vec![])],
    );
    inst.internal_buses = vec![BusEmitInput {
        label: "Link_1".into(),
        display_name: None,
        input_interface: "madi_out".into(),
        input_channels: vec![1, 2],
        input_groups: vec![],
        // No destination — output declared but unrouted
        output_interface: "".into(),
        output_channels: vec![],
        named_outputs: vec![
            BusOutputEmitInput { instance: None, name: "Link 1-L".into(), interface: "".into(), channels: vec![] },
            BusOutputEmitInput { instance: None, name: "Link 1-R".into(), interface: "".into(), channels: vec![] },
        ],
    ..Default::default()
    }];
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(patch.contains("bus Link_1"), "bus must be emitted:\n{patch}");
    assert!(
        patch.contains("output \"Link 1-L\""),
        "named output must be emitted:\n{patch}"
    );
    assert!(
        patch.contains("output \"Link 1-R\""),
        "named output must be emitted:\n{patch}"
    );
    assert!(
        !patch.contains("Unknown") && !patch.contains(": _") && !patch.contains(": ["),
        "unrouted output must NOT emit a destination port ref:\n{patch}"
    );
}

/// A bus with named outputs that DO have a wired destination must still emit
/// the destination port reference correctly.
#[test]
fn emit_bus_named_output_with_destination_includes_port_ref() {
    let mut inst = make_simple_instance(
        "ULTRIX_FR2",
        "ULTRIX_FR2",
        "Ross",
        vec![make_interface("madi_out", "MADI_1_Out", "out", None, 64, vec![])],
    );
    inst.internal_buses = vec![BusEmitInput {
        label: "Link_1".into(),
        display_name: None,
        input_interface: "madi_out".into(),
        input_channels: vec![1, 2],
        input_groups: vec![],
output_interface: "".into(),
        output_channels: vec![],
        named_outputs: vec![
            BusOutputEmitInput { instance: None,
                name: "Link 1-L".into(),
                interface: "MADI_1_Out".into(),
                channels: vec![1],
            },
            BusOutputEmitInput { instance: None,
                name: "Link 1-R".into(),
                interface: "MADI_1_Out".into(),
                channels: vec![2],
            },
        ],
    ..Default::default()
    }];
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("output \"Link 1-L\": MADI_1_Out[1]"),
        "routed output must include port ref:\n{patch}"
    );
    assert!(
        patch.contains("output \"Link 1-R\": MADI_1_Out[2]"),
        "routed output must include port ref:\n{patch}"
    );
}

/// Card-slot AES67 streams use compound interface IDs (`{slotId}__{cardIfaceId}`)
/// that must survive the emit→parse roundtrip. Before the fix, `find_interface`
/// did an exact match against card-relative IDs and silently dropped card-slot streams.
#[test]
fn emit_card_slot_stream_survives_roundtrip() {
    // Card template that contributes an AES67 interface.
    let card = CardEmitInput {
        template_name: "AES67_108_G2".into(),
        manufacturer: Some("Riedel".into()),
        model: "AES67-108 G2".into(),
        fits: "Artist_64".into(),
        interfaces: vec![make_interface(
            "pl::AES67_108_G2::AES67_Out",
            "AES67 Out",
            "out",
            Some("AES67"),
            8,
            vec![],
        )],
    };

    // Device chassis has no AES67 interface of its own.
    let mut inst = make_simple_instance(
        "Artist_64",
        "Artist 64",
        "Riedel",
        vec![make_interface("pl::Artist_64::MADI_Out", "MADI Out", "out", Some("MADI"), 64, vec![])],
    );
    inst.installed_cards = vec![InstalledCardEmitInput {
        slot_label: "Client".into(),
        slot_index: 1,
        card_template_name: "AES67_108_G2".into(),
    }];
    // Compound ID: `{slotGroupId}__{slotIndex}__{cardIfaceId}`.
    // The slotGroupId is `slot::Client::0`, so the full slot ID is `slot::Client::0__1`.
    inst.tx_streams = vec![StreamEmitInput {
        label: "Artist_64_to_QSYS".into(),
        protocol: "AES67".into(),
        channel_count: 8,
        interface_id: "slot::Client::0__1__pl::AES67_108_G2::AES67_Out".into(),
        source_channels: vec![],
    }];

    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![card],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        patch.contains("stream Artist_64_to_QSYS"),
        "card-slot AES67 stream must be emitted; got:\n{patch}"
    );
    assert!(
        patch.contains("protocol: \"AES67\""),
        "emitted stream must declare protocol:\n{patch}"
    );
    assert!(
        patch.contains("channels: \"8\""),
        "emitted stream must declare channel count:\n{patch}"
    );
}

#[test]
fn unresolvable_instance_routes_are_dropped() {
    // RF sentinel ports (__rf_receive__, __rf_transmit__) have no corresponding
    // interface in the template. The emitter must drop them silently instead of
    // emitting broken route declarations that always fail DRC.
    let iface = make_interface("pl::AD4Q::XLR_Out", "XLR Out", "out", None, 4, vec!["Analogue"]);
    let mut inst = make_simple_instance("Vox_1", "AD4Q", "Shure", vec![iface]);
    inst.instance_routes = vec![
        // RF sentinel → real port: unresolvable, must be dropped
        RouteRuleEmitInput {
            from_interface: "__rf_receive__".into(),
            from_channel: 1,
                from_start: 1,
                from_end: 1,
            from_instance: None,
            to_interface: "pl::AD4Q::XLR_Out".into(),
            to_channel: 1,
                to_start: 1,
                to_end: 1,
            to_instance: None,
        },
        // Both unresolvable: must be dropped
        RouteRuleEmitInput {
            from_interface: "pl::PSM1000::Input".into(),
            from_channel: 1,
                from_start: 1,
                from_end: 1,
            from_instance: None,
            to_interface: "__rf_transmit__".into(),
            to_channel: 1,
                to_start: 1,
                to_end: 1,
            to_instance: None,
        },
    ];
    let input = CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![],
    };
    let patch = emit_from_canvas_input(input).unwrap();
    assert!(
        !patch.contains("route"),
        "unresolvable RF sentinel routes must not be emitted; got:\n{patch}"
    );
    assert!(
        !patch.contains("__rf_receive__") && !patch.contains("__rf_transmit__"),
        "RF sentinel port names must not appear in emitted patch:\n{patch}"
    );
}

#[test]
fn cross_instance_route_survives_roundtrip() {
    // Backbone-paired devices (Engine + Surface) form one routing domain: a route
    // owned by one that references a port on the other must survive emit -> load
    // and pass DRC, mirroring cross-device internal buses. Regression for issue #28.
    use crate::builder::canvas_load::load_from_patch;

    let mut engine = make_simple_instance(
        "MON_Engine",
        "Engine",
        "Waves",
        vec![make_interface("gigace_in", "GigACE_In", "in", None, 64, vec![])],
    );
    // to-side qualifier: the Engine routes into the Surface's port.
    engine.instance_routes = vec![RouteRuleEmitInput {
        from_interface: "gigace_in".into(),
        from_channel: 1,
                from_start: 1,
                from_end: 1,
        from_instance: None,
        to_interface: "line_out".into(),
        to_channel: 1,
                to_start: 1,
                to_end: 1,
        to_instance: Some("MON_Surface".into()),
    }];
    let mut surface = make_simple_instance(
        "MON_Surface",
        "Surface",
        "Waves",
        vec![
            make_interface("line_out", "Line_Out", "out", None, 8, vec![]),
            make_interface("sg_out", "SG_Out", "out", None, 8, vec![]),
        ],
    );
    // from-side qualifier: a Surface route that reads a port on the Engine.
    surface.instance_routes = vec![RouteRuleEmitInput {
        from_interface: "gigace_in".into(),
        from_channel: 2,
                from_start: 2,
                from_end: 2,
        from_instance: Some("MON_Engine".into()),
        to_interface: "sg_out".into(),
        to_channel: 1,
                to_start: 1,
                to_end: 1,
        to_instance: None,
    }];

    let patch = emit_from_canvas_input(CanvasEmitInput {
        instances: vec![engine, surface],
        connections: vec![],
        manufacturer_cards: vec![],
    })
    .expect("emit should succeed");

    // Both qualifiers must appear in the emitted routes (before the fix the
    // cross-instance ref was dropped, emitting an unqualified local port).
    assert!(
        patch.contains("-> MON_Surface.Line_Out[1]"),
        "to-side cross-instance qualifier missing:\n{patch}"
    );
    assert!(
        patch.contains("MON_Engine.GigACE_In[2] ->"),
        "from-side cross-instance qualifier missing:\n{patch}"
    );

    // It must be structurally valid — S04 must resolve each qualified endpoint
    // against the paired instance's template, not the owning one.
    let errors: Vec<String> = crate::check(&patch)
        .diagnostics
        .into_iter()
        .filter(|d| d.severity == crate::drc::types::Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(errors.is_empty(), "cross-instance route raised DRC errors: {errors:?}\n{patch}");

    // Both qualifiers must survive a save/reload.
    let out = load_from_patch(&patch, "{}").expect("load should succeed");
    let first_route = |name: &str| {
        out.instances
            .iter()
            .find(|i| i.name == name)
            .and_then(|i| i.instance_routes.first())
    };
    assert_eq!(
        first_route("MON_Engine").and_then(|r| r.to_instance.as_deref()),
        Some("MON_Surface"),
        "to-side qualifier lost on reload:\n{patch}"
    );
    assert_eq!(
        first_route("MON_Surface").and_then(|r| r.from_instance.as_deref()),
        Some("MON_Engine"),
        "from-side qualifier lost on reload:\n{patch}"
    );
}


// ---------------------------------------------------------------------------
// #34 — emit must produce text our own parser accepts
// ---------------------------------------------------------------------------

/// The synthesized label prefers `display_name` over the sanitized identifier, so a bus
/// shown as "Main L/R" is not frozen into the file as "Main_LR".
#[test]
fn legacy_bus_output_fallback_uses_display_name_not_sanitized_id() {
    let mut inst = make_artist_with_card();
    inst.internal_buses = vec![BusEmitInput {
        label: "Main L/R".into(),
        display_name: Some("Main L/R".into()),
        input_interface: "AES67_Out".into(),
        input_channels: vec![1],
        input_groups: vec![],
        output_interface: "AES67_Out".into(),
        output_channels: vec![3],
        named_outputs: vec![],
        ..Default::default()
    }];
    let patch = emit_from_canvas_input(CanvasEmitInput {
        instances: vec![inst],
        connections: vec![],
        manufacturer_cards: vec![make_aes67_card()],
    })
    .expect("emit");
    assert!(
        patch.contains(r#"output "Main L/R""#),
        "expected the human-readable display name, not the sanitized id, got:\n{patch}"
    );
}
