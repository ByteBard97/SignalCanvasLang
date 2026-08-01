//! Shared builders for canvas emit/roundtrip tests.

use crate::builder::canvas_input::*;
use std::collections::HashMap;

pub(super) fn make_interface(
    id: &str,
    label: &str,
    direction: &str,
    transport: Option<&str>,
    channel_count: u32,
    attributes: Vec<&str>,
) -> InterfaceEmitInput {
    InterfaceEmitInput {
        id: id.into(),
        label: label.into(),
        direction: direction.into(),
        connector: Some("etherCON".into()),
        transport: transport.map(|s| s.into()),
        channel_count,
        attributes: attributes.into_iter().map(String::from).collect(),
    }
}

pub(super) fn make_simple_instance(
    name: &str,
    model: &str,
    manufacturer: &str,
    ifaces: Vec<InterfaceEmitInput>,
) -> InstanceEmitInput {
    InstanceEmitInput {
        name: name.into(),
        device_type: "device".into(),
        manufacturer: Some(manufacturer.into()),
        model: model.into(),
        category: Some("Console".into()),
        kind: None,
        location: None,
        dante_chipset: None,
        rf_subtype: None,
        rf_min_channels: None,
        rf_max_channels: None,
        rf_band: None,
        rf_active_channels: None,
        iem_modes: None,
        interfaces: ifaces,
        card_slot_groups: vec![],
        installed_cards: vec![],
        channel_labels: HashMap::new(),
        route_rules: vec![],
        instance_routes: vec![],
        internal_buses: vec![],
        tx_streams: vec![],
        rx_streams: vec![],
        is_ring_container: false,
        ring_protocol: None,
        ring_members: vec![],
    }
}

pub(super) fn make_aes67_card() -> CardEmitInput {
    CardEmitInput {
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
    }
}


pub(super) fn make_artist_with_card() -> InstanceEmitInput {
    let mut inst = make_simple_instance("Artist_64", "Artist64", "Riedel", vec![]);
    inst.installed_cards = vec![InstalledCardEmitInput {
        slot_label: "Card_Slot".into(),
        slot_index: 1,
        card_template_name: "AES67_108_G2".into(),
    }];
    inst
}

/// Assert that whatever `emit_from_canvas_input` produces, `load_from_patch` accepts.
///
/// The emitter and the parser must agree. They did not: a bus with no `named_outputs`
/// but non-empty `output_channels` took the legacy fallback, which emitted `output ""`,
/// and `parse_bus_entry` rejects an empty bus-output label — so canvas round-tripping
/// through that path was broken (#34).
///
/// Existing bus tests could not catch it because they assert only `patch.contains(...)`
/// and never parse the result back. They were producing the malformed text all along.
///
/// NOTE — this catches "does it parse", NOT "does the value survive". Issue #35
/// (unescaped quotes in `emit_bus_entry`) produces output that parses cleanly and
/// silently truncates the value; this helper is structurally blind to it. Do not
/// extend this to cover #35 — it needs a different assertion.
pub(super) fn assert_emit_parses(input: CanvasEmitInput, what: &str) {
    let patch = crate::builder::canvas_emit::emit_from_canvas_input(input).expect("emit");
    if let Err(e) = crate::builder::canvas_load::load_from_patch(&patch, "") {
        panic!("emit produced text that load_from_patch rejects ({what}): {e}\n--- emitted ---\n{patch}");
    }
}
