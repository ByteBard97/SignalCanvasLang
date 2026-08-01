//! Output types for the PatchLang → canvas load direction.
//! Rust parses .patch text; TypeScript maps this to PlacedDevice[].

use super::insert_endpoints::InsertEndpoint;
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
    pub internal_buses: Vec<BusLoadOutput>,
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

#[derive(Debug, Default, Serialize, TS)]
#[ts(export)]
pub struct ChannelLabelOutput {
    pub channel_index: u32,
    pub label: String,
    pub phantom: bool,
    pub propagated: bool,
    pub source_type: Option<String>,
    pub capsule: Option<String>,
    pub rf_band: Option<String>,
    /// Channel insert legs — ordered, `[L]` mono, `[L, R]` stereo. See issue #31 and
    /// `builder::insert_endpoints`. Order is significant; never grouped or unioned.
    /// Empty means absent — a plain `Vec` rather than `Option<Vec>` because ts-rs
    /// ignores `skip_serializing_if` and the `Option` would need `#[ts(optional)]`
    /// to stay honest (see `skip_serialized_option_fields_are_ts_optional` below).
    pub insert_send: Vec<InsertEndpoint>,
    pub insert_return: Vec<InsertEndpoint>,
    /// Every label property that has no dedicated field above, verbatim.
    ///
    /// Without this, any key outside the fixed struct was silently dropped on load —
    /// the reason `insert`/`stand`/`gain` had to live in the layout sidecar (#31).
    ///
    /// Deliberately UNLIKE `ConnectionLoadOutput::properties`, which also keeps the
    /// keys that have dedicated fields: `is_backbone` and friends are a *lossless*
    /// re-read of their string, whereas `insert_send`'s typed field is a *lossy parse*.
    /// If both carried it and the typed field won on re-emit, a malformed-but-intact
    /// source string would be blanked by the very fix meant to preserve it. So a key
    /// lives in exactly one place: parsed cleanly → typed field, key removed here;
    /// malformed → typed field empty, key stays here and re-emits byte-for-byte.
    ///
    /// BTreeMap, not HashMap: HashMap iteration order is non-deterministic, which makes
    /// emit → load → emit reorder properties each run and breaks idempotency.
    pub properties: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct RouteRuleOutput {
    pub from_port: String,
    /// Deprecated: use `from_start`/`from_end` for the full span.
    pub from_channel: u32,
    pub from_start: u32,
    pub from_end: u32,
    /// Owning instance of the `from` port when it's a cross-instance reference
    /// (e.g. a Backbone-paired Engine↔Surface route). `None` for same-instance.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub from_instance: Option<String>,
    pub to_port: String,
    /// Deprecated: use `to_start`/`to_end` for the full span.
    pub to_channel: u32,
    pub to_start: u32,
    pub to_end: u32,
    /// Owning instance of the `to` port when it's a cross-instance reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub to_instance: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct BusInputGroup {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub input_instance: Option<String>,
    pub input_port: String,
    pub input_channels: Vec<u32>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct BusLoadOutput {
    pub name: String,
    pub display_name: Option<String>,
    /// Deprecated: use `input_groups` for multi-port bus inputs.
    pub input_port: String,
    /// Deprecated: use `input_groups` for multi-port bus inputs.
    pub input_channels: Vec<u32>,
    pub input_groups: Vec<BusInputGroup>,
    pub named_outputs: Vec<BusNamedOutput>,
    /// Bus insert legs — ordered, `[L]` mono, `[L, R]` stereo. See issue #31.
    ///
    /// Unlike `input_groups`/`named_outputs` above, these are NOT grouped by
    /// `(instance, port)` and NOT channel-unioned. Bus inputs are set-like so grouping
    /// them is correct; insert legs are an ordered list where a port legitimately
    /// repeats (`send: MADI[3], MADI[10]`), so grouping would reorder the stereo pair.
    pub insert_send: Vec<InsertEndpoint>,
    pub insert_return: Vec<InsertEndpoint>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct BusNamedOutput {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
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
    /// All `connect { ... }` properties verbatim (cable, length, redundant_cable, …).
    ///
    /// `backbone`/`kind`/`from_slot`/`to_slot` are ALSO surfaced as dedicated fields above
    /// because they drive behaviour; they stay in here too so a round-trip re-emits the
    /// block unchanged. Previously nothing but those four crossed the boundary, so every
    /// other property was silently dropped on load. See FrontendV1#202.
    /// BTreeMap, not HashMap: HashMap iteration order is non-deterministic, which made
    /// emit -> load -> emit produce the same properties in a different order each run and
    /// broke idempotency. Sorted by key is stable and predictable.
    pub properties: std::collections::BTreeMap<String, String>,
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

#[cfg(test)]
mod ts_binding_tests {
    use super::*;
    use ts_rs::TS;

    #[test]
    fn skip_serialized_option_fields_are_ts_optional() {
        // Fields serde omits when None (skip_serializing_if) must be TS-optional
        // (`field?: T`), not always-present (`field: T | null`). See issue #28
        // follow-up: ts-rs ignores skip_serializing_if, so #[ts(optional)] is
        // required for binding fidelity.
        let decl = RouteRuleOutput::decl();
        assert!(
            decl.contains("from_instance?:") && decl.contains("to_instance?:"),
            "expected optional instance fields, got:\n{decl}"
        );
    }
}
