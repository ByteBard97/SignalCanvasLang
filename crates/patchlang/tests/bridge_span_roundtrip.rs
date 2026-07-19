//! Failing acceptance tests for bridge span round-trip (issue #30).
//! These tests are written against the TARGET shape (from_start/from_end/to_start/to_end)
//! and will NOT compile until those fields are added to RouteRuleOutput / RouteRuleEmitInput.
//! Test 3 (emit_load_emit_is_byte_idempotent) should compile and pass today.

use patchlang::builder::{emit_from_canvas_input, load_from_patch};
use patchlang::builder::canvas_input::*;
use patchlang::builder::canvas_output::CanvasLoadOutput;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// rebuild_input_from_load — copied verbatim from canvas_roundtrip_tests.rs
// ---------------------------------------------------------------------------

fn rebuild_input_from_load(loaded: &CanvasLoadOutput) -> CanvasEmitInput {
    let instances: Vec<InstanceEmitInput> = loaded.instances.iter().map(|inst| {
        let interfaces: Vec<InterfaceEmitInput> = inst.ports.iter().map(|p| {
            InterfaceEmitInput {
                id: format!("pl::{}::{}", inst.template_name, p.name),
                label: p.name.replace('_', " "),
                direction: p.direction.clone(),
                connector: p.connector.clone(),
                transport: p.transport.clone(),
                channel_count: p.channel_count,
                attributes: p.attributes.clone(),
            }
        }).collect();

        let card_slot_groups: Vec<CardSlotGroupEmitInput> = inst.card_slot_groups.iter().map(|g| {
            CardSlotGroupEmitInput {
                label: g.label.clone(),
                slot_count: g.slot_count,
                slot_format: g.slot_format.clone(),
                direction: g.direction.clone(),
                channel_count: g.channel_count,
            }
        }).collect();

        let installed_cards: Vec<InstalledCardEmitInput> = inst.installed_cards.iter().map(|ic| {
            InstalledCardEmitInput {
                slot_label: ic.slot_label.clone(),
                slot_index: ic.slot_index,
                card_template_name: ic.card_template_name.clone(),
            }
        }).collect();

        let channel_labels: HashMap<String, Vec<ChannelLabelEmitInput>> = inst.channel_labels
            .iter()
            .map(|(port, labels)| {
                let emit_labels = labels.iter().map(|cl| ChannelLabelEmitInput {
                    channel_index: cl.channel_index,
                    label: cl.label.clone(),
                    phantom: cl.phantom,
                    propagated: cl.propagated,
                    source_type: cl.source_type.clone(),
                    capsule: cl.capsule.clone(),
                    rf_band: cl.rf_band.clone(),
                }).collect();
                (port.clone(), emit_labels)
            })
            .collect();

        let route_rules: Vec<RouteRuleEmitInput> = inst.route_rules.iter().map(|r| {
            RouteRuleEmitInput {
                from_interface: r.from_port.clone(),
                from_channel: r.from_start,
                from_start: r.from_start,
                from_end: r.from_end,
                from_instance: r.from_instance.clone(),
                to_interface: r.to_port.clone(),
                to_channel: r.to_start,
                to_start: r.to_start,
                to_end: r.to_end,
                to_instance: r.to_instance.clone(),
            }
        }).collect();

        let instance_routes: Vec<RouteRuleEmitInput> = inst.instance_routes.iter().map(|r| {
            RouteRuleEmitInput {
                from_interface: r.from_port.clone(),
                from_channel: r.from_start,
                from_start: r.from_start,
                from_end: r.from_end,
                from_instance: r.from_instance.clone(),
                to_interface: r.to_port.clone(),
                to_channel: r.to_start,
                to_start: r.to_start,
                to_end: r.to_end,
                to_instance: r.to_instance.clone(),
            }
        }).collect();

        let internal_buses: Vec<BusEmitInput> = inst.internal_buses.iter().map(|b| {
            let named_outputs = b.named_outputs.iter().map(|o| BusOutputEmitInput {
                name: o.name.clone(),
                instance: o.output_instance.clone(),
                interface: o.output_port.clone(),
                channels: o.output_channels.clone(),
            }).collect();
            BusEmitInput {
                label: b.name.clone(),
                display_name: b.display_name.clone(),
                input_interface: b.input_port.clone(),
                input_channels: b.input_channels.clone(),
                input_groups: vec![],
                output_interface: b.named_outputs.first()
                    .map(|o| o.output_port.clone())
                    .unwrap_or_default(),
                output_channels: b.named_outputs.first()
                    .map(|o| o.output_channels.clone())
                    .unwrap_or_default(),
                named_outputs,
            }
        }).collect();

        let tx_streams: Vec<StreamEmitInput> = inst.tx_streams.iter().map(|s| StreamEmitInput {
            label: s.label.clone(),
            protocol: s.protocol.clone(),
            channel_count: s.channel_count,
            interface_id: format!("pl::{}::{}", inst.template_name, s.port_name),
        }).collect();

        let rx_streams: Vec<StreamEmitInput> = inst.rx_streams.iter().map(|s| StreamEmitInput {
            label: s.label.clone(),
            protocol: s.protocol.clone(),
            channel_count: s.channel_count,
            interface_id: format!("pl::{}::{}", inst.template_name, s.port_name),
        }).collect();

        InstanceEmitInput {
            name: inst.name.clone(),
            device_type: inst.kind.clone().unwrap_or_else(|| "device".to_string()),
            manufacturer: inst.manufacturer.clone(),
            model: inst.model.clone().unwrap_or_default(),
            category: inst.category.clone(),
            kind: inst.kind.clone(),
            location: inst.location.clone(),
            dante_chipset: inst.dante_chipset.clone(),
            rf_subtype: inst.rf_subtype.clone(),
            rf_min_channels: inst.rf_min_channels,
            rf_max_channels: inst.rf_max_channels,
            rf_band: inst.rf_band.clone(),
            rf_active_channels: inst.rf_active_channels,
            iem_modes: inst.iem_modes.clone(),
            interfaces,
            card_slot_groups,
            installed_cards,
            channel_labels,
            route_rules,
            instance_routes,
            internal_buses,
            tx_streams,
            rx_streams,
            is_ring_container: inst.is_ring_container,
            ring_protocol: inst.ring_protocol.clone(),
            ring_members: vec![],
        }
    }).collect();

    let connections: Vec<ConnectionEmitInput> = loaded.connections.iter().map(|c| {
        ConnectionEmitInput {
            from_instance_name: c.from_instance.clone(),
            to_instance_name: c.to_instance.clone(),
            from_port_id: c.from_port.clone(),
            to_port_id: c.to_port.clone(),
            is_backbone: c.is_backbone,
            channel_mappings: c.channel_mappings.iter().map(|m| ChannelMappingEmitInput {
                from_channel: m.from_channel,
                to_channel: m.to_channel,
                mapping_type: "direct".to_string(),
            }).collect(),
            properties: vec![],
        }
    }).collect();

    let manufacturer_cards: Vec<CardEmitInput> = loaded.card_templates.iter().map(|ct| {
        let interfaces = ct.ports.iter().map(|p| InterfaceEmitInput {
            id: format!("pl::{}::{}", ct.template_name, p.name),
            label: p.name.replace('_', " "),
            direction: p.direction.clone(),
            connector: p.connector.clone(),
            transport: p.transport.clone(),
            channel_count: p.channel_count,
            attributes: p.attributes.clone(),
        }).collect();
        CardEmitInput {
            template_name: ct.template_name.clone(),
            manufacturer: ct.manufacturer.clone(),
            model: ct.model.clone().unwrap_or_default(),
            fits: ct.fits.clone().unwrap_or_default(),
            interfaces,
        }
    }).collect();

    CanvasEmitInput { instances, connections, manufacturer_cards }
}

// ---------------------------------------------------------------------------
// Fixture loader
// ---------------------------------------------------------------------------

fn load_fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/lib/allen-heath-bridges.patch");
    std::fs::read_to_string(path).expect("fixture file should exist")
}

// ---------------------------------------------------------------------------
// Test 1 — GX4816 bridge spans survive load with full width
// ---------------------------------------------------------------------------

#[test]
fn gx4816_bridge_spans_survive_load() {
    let patch = load_fixture();
    let output = load_from_patch(&patch, "{}").expect("load should succeed");

    let sb1 = output.instances.iter()
        .find(|i| i.name == "SB1")
        .expect("SB1 instance should exist");

    assert_eq!(sb1.route_rules.len(), 6, "GX4816 has 6 template bridges");

    let expected: [(String, u32, u32, String, u32, u32); 6] = [
        ("Mic_In".into(), 1, 48, "GX_Out".into(), 1, 48),
        ("GX_In".into(), 1, 16, "Line_Out".into(), 1, 16),
        ("DX_1_In".into(), 1, 32, "GX_Out".into(), 65, 96),
        ("GX_In".into(), 65, 96, "DX_1_Out".into(), 1, 32),
        ("DX_2_In".into(), 1, 32, "GX_Out".into(), 97, 128),
        ("GX_In".into(), 97, 128, "DX_2_Out".into(), 1, 32),
    ];

    for (i, (from_port, from_start, from_end, to_port, to_start, to_end)) in expected.iter().enumerate() {
        let rule = &sb1.route_rules[i];
        assert_eq!(rule.from_port, *from_port, "rule {i} from_port mismatch");
        assert_eq!(rule.from_start, *from_start, "rule {i} from_start mismatch");
        assert_eq!(rule.from_end, *from_end, "rule {i} from_end mismatch");
        assert_eq!(rule.to_port, *to_port, "rule {i} to_port mismatch");
        assert_eq!(rule.to_start, *to_start, "rule {i} to_start mismatch");
        assert_eq!(rule.to_end, *to_end, "rule {i} to_end mismatch");
    }
}

// ---------------------------------------------------------------------------
// Test 2 — DX168 cascade span is not fabricated
// ---------------------------------------------------------------------------

#[test]
fn dx168_cascade_span_is_not_fabricated() {
    let patch = load_fixture();
    let output = load_from_patch(&patch, "{}").expect("load should succeed");

    let dx1 = output.instances.iter()
        .find(|i| i.name == "DX1")
        .expect("DX1 instance should exist");

    // Find the cascade rule: DX_A_In[17..24] -> DX_Cascade_Out[1..8]
    let cascade = dx1.route_rules.iter()
        .find(|r| r.from_port == "DX_A_In" && r.from_start == 17)
        .expect("DX168 cascade rule should exist");

    // The frontend's buildRouteRulesFromWasm inference currently widens this
    // to 17..32 -> 1..16, fabricating 8 channel mappings that were never in
    // the source. This test guards against that fabrication.
    assert_eq!(cascade.from_end, 24, "cascade from span must be exactly 17..24, not widened");
    assert_eq!(cascade.to_port, "DX_Cascade_Out", "cascade to_port mismatch");
    assert_eq!(cascade.to_start, 1, "cascade to_start mismatch");
    assert_eq!(cascade.to_end, 8, "cascade to span must be exactly 1..8, not widened");
}

// ---------------------------------------------------------------------------
// Test 3 — emit → load → emit is byte-idempotent
// ---------------------------------------------------------------------------

#[test]
fn emit_load_emit_is_byte_idempotent() {
    let patch = load_fixture();
    let loaded = load_from_patch(&patch, "{}").expect("first load");
    let rebuilt = rebuild_input_from_load(&loaded);
    let first_emit = emit_from_canvas_input(rebuilt).expect("first emit");

    let loaded2 = load_from_patch(&first_emit, "{}").expect("second load");
    let rebuilt2 = rebuild_input_from_load(&loaded2);
    let second_emit = emit_from_canvas_input(rebuilt2).expect("second emit");

    assert_eq!(first_emit, second_emit, "emit→load→emit must be byte-idempotent");
}
