//! Output types for the PatchLang → canvas load direction.
//! Rust parses .patch text; TypeScript maps this to PlacedDevice[].

use serde::Serialize;
use std::collections::HashMap;
use ts_rs::TS;

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct CanvasLoadOutput {
    pub instances: Vec<InstanceLoadOutput>,
    pub connections: Vec<ConnectionLoadOutput>,
    pub card_templates: Vec<CardTemplateOutput>,
    pub rings: Vec<RingLoadOutput>,
    pub networks: Vec<NetworkLoadOutput>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct InstanceLoadOutput {
    pub name: String,
    pub template_name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
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
    pub ports: Vec<PortLoadOutput>,
    pub card_slot_groups: Vec<CardSlotGroupOutput>,
    pub installed_cards: Vec<InstalledCardOutput>,
    /// keyed by port name (canonical, directional)
    pub channel_labels: HashMap<String, Vec<ChannelLabelOutput>>,
    /// Template-level bridges (e.g. `bridge Mic_In -> Dante_Out`) → UserDevice.routeRules
    pub route_rules: Vec<RouteRuleOutput>,
    /// Per-instance route entries (`route A[n] -> B[m]` in instance body) → pd.internalRoutes
    pub instance_routes: Vec<RouteRuleOutput>,
    pub internal_buses: Vec<BusOutput>,
    pub tx_streams: Vec<StreamOutput>,
    pub rx_streams: Vec<StreamOutput>,
    pub is_ring_container: bool,
    pub ring_protocol: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct PortLoadOutput {
    pub name: String,
    /// "in" | "out" | "io"
    pub direction: String,
    pub connector: Option<String>,
    pub channel_count: u32,
    pub transport: Option<String>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct CardSlotGroupOutput {
    pub label: String,
    pub slot_count: u32,
    pub slot_format: String,
    pub direction: String,
    pub channel_count: u32,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct InstalledCardOutput {
    pub slot_label: String,
    pub slot_index: u32,
    pub card_template_name: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ChannelLabelOutput {
    pub channel_index: u32,
    pub label: String,
    pub phantom: bool,
    pub propagated: bool,
    pub source_type: Option<String>,
    pub capsule: Option<String>,
    pub rf_band: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct RouteRuleOutput {
    pub from_port: String,
    pub from_channel: u32,
    /// Owning instance of the `from` port when it's a cross-instance reference
    /// (e.g. a Backbone-paired Engine↔Surface route). `None` for same-instance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_instance: Option<String>,
    pub to_port: String,
    pub to_channel: u32,
    /// Owning instance of the `to` port when it's a cross-instance reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_instance: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct BusOutput {
    pub name: String,
    pub display_name: Option<String>,
    pub input_port: String,
    pub input_channels: Vec<u32>,
    pub named_outputs: Vec<BusNamedOutput>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct BusNamedOutput {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_instance: Option<String>,
    pub output_port: String,
    pub output_channels: Vec<u32>,
}

#[derive(Debug, Serialize, Clone, TS)]
#[ts(export)]
pub struct StreamOutput {
    pub label: String,
    pub protocol: String,
    pub channel_count: u32,
    pub port_name: String,
    pub direction: String,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ConnectionLoadOutput {
    pub from_instance: String,
    pub to_instance: String,
    pub from_port: String,
    pub to_port: String,
    pub is_backbone: bool,
    pub channel_mappings: Vec<ChannelMappingOutput>,
    pub from_slot: Option<String>,
    pub to_slot: Option<String>,
    /// Raw mapping text from PatchLang (e.g. "offset -8", "1->3, 2->4") for TypeScript to process
    pub mapping_text: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ChannelMappingOutput {
    pub from_channel: u32,
    pub to_channel: u32,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct CardTemplateOutput {
    pub template_name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub fits: Option<String>,
    pub ports: Vec<PortLoadOutput>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct RingLoadOutput {
    pub name: String,
    pub protocol: Option<String>,
    pub members: Vec<RingMemberOutput>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct RingMemberOutput {
    pub instance_name: String,
    pub port_name: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct NetworkLoadOutput {
    pub name: String,
    pub protocol: Option<String>,
    pub label: Option<String>,
    pub members: Vec<NetworkMemberLoadOutput>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct NetworkMemberLoadOutput {
    pub member_type: String,
    pub instance_name: String,
    pub port_group: Option<String>,
    pub slot_index: Option<u32>,
}
