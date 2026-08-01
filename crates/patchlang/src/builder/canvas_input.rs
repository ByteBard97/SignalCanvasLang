//! Input types for the canvas → PatchLang emit direction.
//! TypeScript assembles this JSON bundle; Rust does all language work.

use super::insert_endpoints::InsertEndpoint;
use serde::Deserialize;
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct CanvasEmitInput {
    pub instances: Vec<InstanceEmitInput>,
    pub connections: Vec<ConnectionEmitInput>,
    pub manufacturer_cards: Vec<CardEmitInput>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct InstanceEmitInput {
    /// Pre-resolved human-readable name (TypeScript does UUID → name mapping).
    pub name: String,
    pub device_type: String,
    pub manufacturer: Option<String>,
    pub model: String,
    pub category: Option<String>,
    pub kind: Option<String>,
    pub location: Option<String>,
    pub dante_chipset: Option<String>,
    pub rf_subtype: Option<String>,
    pub rf_min_channels: Option<u32>,
    pub rf_max_channels: Option<u32>,
    pub rf_band: Option<String>,
    pub rf_active_channels: Option<u32>,
    pub iem_modes: Option<String>,
    pub interfaces: Vec<InterfaceEmitInput>,
    pub card_slot_groups: Vec<CardSlotGroupEmitInput>,
    pub installed_cards: Vec<InstalledCardEmitInput>,
    /// Map from interface id → list of channel labels.
    pub channel_labels: HashMap<String, Vec<ChannelLabelEmitInput>>,
    pub route_rules: Vec<RouteRuleEmitInput>,
    /// Per-instance route entries (emitted inside instance body as `route A[n] -> B[m]`)
    pub instance_routes: Vec<RouteRuleEmitInput>,
    pub internal_buses: Vec<BusEmitInput>,
    pub tx_streams: Vec<StreamEmitInput>,
    pub rx_streams: Vec<StreamEmitInput>,
    pub is_ring_container: bool,
    pub ring_protocol: Option<String>,
    /// For ring container instances: ordered list of member {instance, port} pairs.
    pub ring_members: Vec<RingMemberEmitInput>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct RingMemberEmitInput {
    pub member_name: String,
    pub port_name: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct InterfaceEmitInput {
    pub id: String,
    pub label: String,
    /// "in" | "out" | "io" | "asymmetric"
    pub direction: String,
    pub connector: Option<String>,
    pub transport: Option<String>,
    pub channel_count: u32,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct CardSlotGroupEmitInput {
    pub label: String,
    pub slot_count: u32,
    pub slot_format: String,
    pub direction: String,
    pub channel_count: u32,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct InstalledCardEmitInput {
    pub slot_label: String,
    pub slot_index: u32,
    pub card_template_name: String,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(export)]
pub struct ChannelLabelEmitInput {
    pub channel_index: u32,
    pub label: String,
    pub phantom: bool,
    pub propagated: bool,
    pub source_type: Option<String>,
    pub capsule: Option<String>,
    pub rf_band: Option<String>,
    /// Channel insert legs — ordered, `[L]` mono, `[L, R]` stereo (issue #31).
    ///
    /// `#[serde(default)]` is REQUIRED, same reason as `RouteRuleEmitInput::from_start`
    /// above: a frontend built against older WASM omits these entirely, and without the
    /// default serde rejects the whole payload with "missing field `insert_send`".
    #[serde(default)]
    pub insert_send: Vec<InsertEndpoint>,
    #[serde(default)]
    pub insert_return: Vec<InsertEndpoint>,
    /// Label properties with no dedicated field above, emitted verbatim. Round-trips
    /// with `ChannelLabelOutput::properties` — see that field for the ownership rule.
    #[serde(default)]
    pub properties: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct RouteRuleEmitInput {
    pub from_interface: String,
    pub from_channel: u32,
    /// Span fields. `#[serde(default)]` is REQUIRED for backward compatibility: a
    /// frontend built against an older WASM sends only `from_channel`, and without
    /// the default serde rejects the whole payload with "missing field `from_start`".
    /// `build_bridges` falls back to `from_channel` when these are 0.
    #[serde(default)]
    pub from_start: u32,
    #[serde(default)]
    pub from_end: u32,
    /// Optional owning instance of the `from` port. `None` = the instance that
    /// owns this route (same-instance route). `Some(name)` = a paired/other
    /// instance, e.g. a Backbone-paired Engine↔Surface cross-instance route.
    #[serde(default)]
    #[ts(optional)]
    pub from_instance: Option<String>,
    pub to_interface: String,
    pub to_channel: u32,
    /// See `from_start` — `#[serde(default)]` is required for backward compatibility.
    #[serde(default)]
    pub to_start: u32,
    #[serde(default)]
    pub to_end: u32,
    /// Optional owning instance of the `to` port. See `from_instance`.
    #[serde(default)]
    #[ts(optional)]
    pub to_instance: Option<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct BusInputGroupEmitInput {
    #[serde(default)]
    #[ts(optional)]
    pub instance: Option<String>,
    pub interface: String,
    pub channels: Vec<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(export)]
pub struct BusEmitInput {
    pub label: String,
    /// Human-readable name when it differs from the sanitized identifier, e.g. "PQ>MM"
    pub display_name: Option<String>,
    /// Deprecated: use `input_groups` for multi-port bus inputs.
    pub input_interface: String,
    /// Deprecated: use `input_groups` for multi-port bus inputs.
    pub input_channels: Vec<u32>,
    #[serde(default)]
    pub input_groups: Vec<BusInputGroupEmitInput>,
    pub output_interface: String,
    pub output_channels: Vec<u32>,
    /// Named outputs (e.g. [{name: "Main Output", interface: "Matrix_Out", channels: [1, 2]}])
    pub named_outputs: Vec<BusOutputEmitInput>,
    /// Bus insert legs — ordered, `[L]` mono, `[L, R]` stereo (issue #31).
    /// `#[serde(default)]` required for backward compatibility, see
    /// `ChannelLabelEmitInput::insert_send`.
    #[serde(default)]
    pub insert_send: Vec<InsertEndpoint>,
    #[serde(default)]
    pub insert_return: Vec<InsertEndpoint>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct BusOutputEmitInput {
    pub name: String,
    #[serde(default)]
    pub instance: Option<String>,
    pub interface: String,
    pub channels: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct StreamEmitInput {
    pub label: String,
    pub protocol: String,
    pub channel_count: u32,
    pub interface_id: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct ConnectionEmitInput {
    pub from_instance_name: String,
    pub to_instance_name: String,
    pub from_port_id: String,
    pub to_port_id: String,
    pub is_backbone: bool,
    pub channel_mappings: Vec<ChannelMappingEmitInput>,
    pub properties: Vec<KvEmitInput>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct ChannelMappingEmitInput {
    pub from_channel: u32,
    pub to_channel: u32,
    /// "forward" | "return" | "direct"
    pub mapping_type: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct KvEmitInput {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct CardEmitInput {
    pub template_name: String,
    pub manufacturer: Option<String>,
    pub model: String,
    pub fits: String,
    pub interfaces: Vec<InterfaceEmitInput>,
}

#[cfg(test)]
mod ts_binding_tests {
    use super::*;
    use ts_rs::TS;

    #[test]
    fn route_emit_input_instance_fields_are_ts_optional() {
        // The emit-input instance qualifiers are #[serde(default)] (omittable),
        // so the generated TS type must be `field?: T`, matching the output side.
        let decl = RouteRuleEmitInput::decl();
        assert!(
            decl.contains("from_instance?:") && decl.contains("to_instance?:"),
            "expected optional instance fields, got:\n{decl}"
        );
    }
}
