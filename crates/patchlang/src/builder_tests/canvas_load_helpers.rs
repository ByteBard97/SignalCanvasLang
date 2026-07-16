//! Shared builders for canvas-load tests.
use crate::builder::canvas_input::*;
use std::collections::HashMap;

pub(super) fn make_iface(id: &str, label: &str, dir: &str, ch: u32) -> InterfaceEmitInput {
    InterfaceEmitInput {
        id: id.into(),
        label: label.into(),
        direction: dir.into(),
        connector: None,
        transport: None,
        channel_count: ch,
        attributes: vec![],
    }
}

pub(super) fn make_inst(name: &str, model: &str, ifaces: Vec<InterfaceEmitInput>) -> InstanceEmitInput {
    InstanceEmitInput {
        name: name.into(),
        device_type: "device".into(),
        manufacturer: Some("QSC".into()),
        model: model.into(),
        category: Some("Processor".into()),
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

