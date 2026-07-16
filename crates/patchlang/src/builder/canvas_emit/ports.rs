use crate::ast::{
    PortDef, PortDirection, RangeSpec,
};
use crate::builder::canvas_input::*;
use super::helpers::*;

// ---------------------------------------------------------------------------
// Ports
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
pub(super) enum PortSide {
    Input,
    Output,
}

pub(super) fn build_ports_for_interfaces(ifaces: &[InterfaceEmitInput]) -> Vec<PortDef> {
    let mut ports = Vec::new();
    for iface in ifaces {
        for port in expand_interface_to_ports(iface) {
            ports.push(port);
        }
    }
    ports
}

/// Channel-based protocols that split io into separate `_In` + `_Out` ports.
/// Ring/bus protocols stay as `io` — everything else with io/asymmetric
/// direction is split into _In + _Out. Matches portUtils.ts isRingBusInterface.
pub(super) fn is_ring_bus_protocol(transport: &str) -> bool {
    matches!(transport, "OptoCore" | "TWINLANe" | "AVB" | "GigaACE")
}

pub(super) fn should_split_io(iface: &InterfaceEmitInput) -> bool {
    if iface.direction != "io" && iface.direction != "asymmetric" {
        return false;
    }
    // Split unless explicitly a ring/bus protocol; unknown/absent transport → split
    match &iface.transport {
        Some(t) => !is_ring_bus_protocol(t),
        None => true,
    }
}

pub(super) fn expand_interface_to_ports(iface: &InterfaceEmitInput) -> Vec<PortDef> {
    let connector = iface.connector.as_ref().map(|c| sanitize_id(c));
    let attributes = build_port_attributes(iface);
    let range = if iface.channel_count > 1 {
        Some(RangeSpec {
            start: 1,
            end: iface.channel_count,
        })
    } else {
        None
    };
    let base = sanitize_id(&iface.label);

    if should_split_io(iface) {
        return vec![
            PortDef {
                name: format!("{base}_In"),
                range: range.clone(),
                direction: PortDirection::In,
                connector: connector.clone(),
                attributes: attributes.clone(),
                named_attributes: Vec::new(),
                span: builder_span(),
            },
            PortDef {
                name: format!("{base}_Out"),
                range,
                direction: PortDirection::Out,
                connector,
                attributes,
                named_attributes: Vec::new(),
                span: builder_span(),
            },
        ];
    }

    let direction = match iface.direction.as_str() {
        "in" => PortDirection::In,
        "out" => PortDirection::Out,
        _ => PortDirection::Io,
    };

    vec![PortDef {
        name: base,
        range,
        direction,
        connector,
        attributes,
        named_attributes: Vec::new(),
        span: builder_span(),
    }]
}

pub(super) fn build_port_attributes(iface: &InterfaceEmitInput) -> Vec<String> {
    let mut attrs = Vec::new();
    if let Some(t) = &iface.transport {
        let sanitized = sanitize_id(t);
        if !sanitized.is_empty() {
            attrs.push(sanitized);
        }
    }
    for a in &iface.attributes {
        let sanitized = sanitize_id(a);
        if !sanitized.is_empty() {
            attrs.push(sanitized);
        }
    }
    attrs
}

/// For a channel-based io interface, return the directional port name on the
/// requested side (e.g. `Dante_Pri_In` / `Dante_Pri_Out`). For non-split
/// interfaces, return the base sanitized label.
pub(super) fn directional_port_name(iface: &InterfaceEmitInput, side: PortSide) -> String {
    let base = sanitize_id(&iface.label);
    if should_split_io(iface) {
        match side {
            PortSide::Input => format!("{base}_In"),
            PortSide::Output => format!("{base}_Out"),
        }
    } else {
        base
    }
}

