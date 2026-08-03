//! TypeScript-compatible AST types for JSON serialization.
//!
//! These mirror the `PatchProgram` shape expected by the SignalCanvas frontend
//! (see `tests/chevrotain-parity/ts-reference/types.ts`). Internal Rust AST
//! types remain unchanged — this module is a pure serialization adapter.

use serde::Serialize;
use std::collections::BTreeMap;

/// Top-level program — matches TS `PatchProgram`.
#[derive(Debug, Clone, Serialize)]
pub struct TsProgram {
    pub r#type: &'static str,
    pub statements: Vec<TsStatement>,
}

/// A top-level statement (tagged union via `type` field).
///
/// `untagged`, not `tag = "type"`: every variant's inner struct already carries its
/// own `type_tag` field renamed to `"type"`, because these structs are *also*
/// serialized standalone — nested inside a template's `instances`/`connects`/`bridges`
/// they never pass through this enum, so their own tag is the only thing identifying
/// them. Adding serde's tag on top emitted `"type"` **twice** in every top-level
/// statement, which is invalid JSON; strict parsers may reject it and lenient ones
/// silently take the last occurrence. Untagged defers to the inner field, so the key
/// appears exactly once at every nesting level.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TsStatement {
    Template(TsTemplateDecl),
    Instance(TsInstanceDecl),
    Connect(TsConnectDecl),
    Bridge(TsBridgeDecl),
    BridgeGroup(TsBridgeGroupDecl),
    LinkGroup(TsLinkGroupDecl),
    Signal(TsSignalDecl),
    Flag(TsFlagDecl),
    Stream(TsStreamDecl),
    Config(TsConfigDecl),
    Use(TsUseDecl),
    Ring(TsRingDecl),
    Network(TsNetworkDecl),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsTemplateDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub name: String,
    pub params: Vec<TsParamDef>,
    pub meta: BTreeMap<String, String>,
    pub ports: Vec<TsPortDef>,
    pub bridges: Vec<TsBridgeDecl>,
    pub instances: Vec<TsInstanceDecl>,
    pub connects: Vec<TsConnectDecl>,
    pub slots: Vec<TsSlotDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsParamDef {
    pub name: String,
    pub default_value: TsParamValue,
}

/// Bare value (number or string) — no tagged union wrapper.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TsParamValue {
    Num(u32),
    Str(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsPortDef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_end: Option<u32>,
    pub direction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector: Option<String>,
    pub attributes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub named_attributes: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsInstanceDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub name: String,
    pub template_name: String,
    pub args: BTreeMap<String, TsArgValue>,
    pub properties: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_constraint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routes: Option<Vec<TsRouteDecl>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buses: Option<Vec<TsBusDecl>>,
    #[serde(rename = "typedSlotAssignments")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typed_slot_assignments: Option<Vec<TsSlotAssign>>,
}

/// Arg values preserve numeric type: `Record<string, number | string>`.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum TsArgValue {
    Num(u32),
    Str(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsConnectDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub source: TsPortRef,
    pub target: TsPortRef,
    pub properties: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppressions: Option<TsSuppression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping: Option<TsMappingSpec>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsBridgeDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub source: TsPortRef,
    pub target: TsPortRef,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsBridgeGroupDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub target: TsPortRef,
    pub sources: Vec<TsPortRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsLinkGroupDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub name: String,
    pub connects: Vec<TsConnectDecl>,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsSignalDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub name: String,
    pub properties: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<TsPortRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsFlagDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub name: String,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsStreamDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub name: String,
    pub properties: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<TsPortRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsConfigDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub name: String,
    pub labels: Vec<TsConfigLabel>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsConfigLabel {
    pub port: TsPortRef,
    pub label: String,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsUseDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub namespace: String,
    pub templates: Vec<String>,
    pub wildcard: bool,
}

/// Port reference — `instance` is always a string (empty for local refs).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsPortRef {
    pub instance: String,
    pub port: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_spec: Option<Vec<TsIndexElement>>,
}

/// Index element — lowercase type tag to match TS convention.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum TsIndexElement {
    #[serde(rename = "single")]
    Single { value: u32 },
    #[serde(rename = "range")]
    Range { start: u32, end: u32 },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsSlotDef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_end: Option<u32>,
    pub slot_type: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsRouteDecl {
    pub from_port: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_index: Option<Vec<TsIndexElement>>,
    pub to_port: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_index: Option<Vec<TsIndexElement>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsBusOutput {
    pub label: String,
    pub destinations: Vec<TsPortRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsBusDecl {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub inputs: Vec<TsPortRef>,
    pub outputs: Vec<TsBusOutput>,
    /// Bus insert legs — ordered, `[L]` mono, `[L, R]` stereo (issue #31).
    /// Omitted when empty so existing consumers of this compat shape are unaffected.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub insert_send: Vec<TsPortRef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub insert_return: Vec<TsPortRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsSlotAssign {
    pub slot_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_index: Option<u32>,
    pub card_type_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsRingMember {
    pub instance_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsRingDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub name: String,
    pub properties: BTreeMap<String, String>,
    pub members: Vec<TsRingMember>,
}

/// The kind of network member reference.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TsNetworkMember {
    DeviceLevel { instance: String },
    PortGroup { instance: String, port_group: String },
    SlotRef { instance: String, index: u32 },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TsNetworkDecl {
    #[serde(rename = "type")]
    pub type_tag: &'static str,
    pub name: String,
    pub properties: BTreeMap<String, String>,
    pub members: Vec<TsNetworkMember>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TsSuppression {
    pub layers: Vec<String>,
}

/// Channel mapping specification.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum TsMappingSpec {
    #[serde(rename = "one-to-one")]
    OneToOne,
    #[serde(rename = "offset")]
    Offset { offset: i64 },
    #[serde(rename = "explicit")]
    Explicit { pairs: Vec<TsMappingPair> },
}

#[derive(Debug, Clone, Serialize)]
pub struct TsMappingPair {
    pub from: u32,
    pub to: u32,
}

/// Compat parse result wrapping the TS-shaped program.
#[derive(Debug, Clone, Serialize)]
pub struct TsParseResult {
    pub program: TsProgram,
    pub errors: Vec<TsParseError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TsParseError {
    pub message: String,
    pub span: TsSpan,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TsSpan {
    pub start: usize,
    pub end: usize,
}
