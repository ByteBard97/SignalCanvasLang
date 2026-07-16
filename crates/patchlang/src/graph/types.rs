//! Graph output types matching the TypeScript `CompileToGraphResult` interface.
//!
//! All structs use `#[serde(rename_all = "camelCase")]` so the JSON output
//! matches the camelCase keys the frontend expects.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// Top-level result from `compile_to_graph`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CompileToGraphResult {
    pub levels: BTreeMap<String, GraphLevel>,
    pub signals: BTreeMap<String, SignalIdentity>,
    pub streams: BTreeMap<String, StreamIdentity>,
}

/// A single level in the hierarchical graph ("root" or a drillable instance).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct GraphLevel {
    pub id: String,
    pub parent_id: Option<String>,
    pub label: String,
    pub nodes: BTreeMap<String, DeviceNode>,
    pub edges: BTreeMap<String, GraphEdge>,
}

/// A device/instance node in the graph.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DeviceNode {
    pub id: String,
    pub label: String,
    pub template_name: String,
    pub ports: Vec<PortInfo>,
    pub properties: BTreeMap<String, String>,
    pub drillable: bool,
}

/// A port on a device node.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    pub id: String,
    pub name: String,
    pub direction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub connector: Option<String>,
    pub attributes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub connected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub signal_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub label_properties: Option<BTreeMap<String, String>>,
    /// Scene interface ID: `pl::{TemplateName}::{PortDefName}`.
    /// Groups ranged ports under one key per definition (e.g., `Mic_In`, not `Mic_In_1`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_key: Option<String>,
}

/// An edge between two ports.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source_node: String,
    pub source_port: String,
    pub target_node: String,
    pub target_port: String,
    pub edge_type: String,
    pub properties: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bus_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bus_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bus_size: Option<usize>,
}

/// Signal identity metadata.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SignalIdentity {
    pub name: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub origin_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub origin_port: Option<String>,
}

/// Stream identity metadata.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct StreamIdentity {
    pub name: String,
    pub properties: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_port: Option<String>,
}
