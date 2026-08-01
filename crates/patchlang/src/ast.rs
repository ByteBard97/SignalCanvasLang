use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::Span;

/// Top-level program — a list of statements.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PatchProgram {
    pub statements: Vec<Statement>,
}

/// A top-level statement.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum Statement {
    Template(TemplateDecl),
    Instance(InstanceDecl),
    Connect(ConnectDecl),
    Bridge(BridgeDecl),
    BridgeGroup(BridgeGroupDecl),
    LinkGroup(LinkGroupDecl),
    Signal(SignalDecl),
    Flag(FlagDecl),
    Stream(StreamDecl),
    Config(ConfigDecl),
    Use(UseDecl),
    Ring(RingDecl),
    Network(NetworkDecl),
    /// Placeholder for recovered error regions.
    Error(Span),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TemplateDecl {
    pub name: String,
    pub params: Vec<ParamDef>,
    pub version: Option<String>,
    pub meta: Vec<KeyValue>,
    pub ports: Vec<PortDef>,
    pub bridges: Vec<BridgeDecl>,
    pub instances: Vec<InstanceDecl>,
    pub connects: Vec<ConnectDecl>,
    pub slots: Vec<SlotDef>,
    pub span: Span,
}

/// A template parameter with a name and default value.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ParamDef {
    pub name: String,
    pub default_value: ParamValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct InstanceDecl {
    pub name: String,
    pub template_name: String,
    pub args: Vec<KeyValue>,
    pub version_constraint: Option<String>,
    pub properties: Vec<KeyValue>,
    pub routes: Vec<RouteEntry>,
    pub buses: Vec<BusEntry>,
    pub slot_assignments: Vec<SlotAssignment>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConnectDecl {
    pub source: PortRef,
    pub target: PortRef,
    pub properties: Vec<KeyValue>,
    pub suppressions: Vec<String>,
    pub mapping: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BridgeDecl {
    pub source: PortRef,
    pub target: PortRef,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BridgeGroupDecl {
    pub target: PortRef,
    pub sources: Vec<PortRef>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LinkGroupDecl {
    pub name: String,
    pub connects: Vec<ConnectDecl>,
    pub properties: Vec<KeyValue>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SignalDecl {
    pub name: String,
    pub properties: Vec<KeyValue>,
    pub origin: Option<PortRef>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FlagDecl {
    pub name: String,
    pub properties: Vec<KeyValue>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StreamDecl {
    pub name: String,
    pub properties: Vec<KeyValue>,
    pub source: Option<PortRef>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConfigDecl {
    pub name: String,
    pub labels: Vec<ConfigLabel>,
    pub span: Span,
}

/// A label entry inside a config block.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConfigLabel {
    pub port: PortRef,
    pub label: String,
    pub properties: Vec<KeyValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UseDecl {
    pub namespace: String,
    pub templates: Vec<String>,
    pub wildcard: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PortDef {
    pub name: String,
    pub range: Option<RangeSpec>,
    pub direction: PortDirection,
    pub connector: Option<String>,
    pub attributes: Vec<String>,
    pub named_attributes: Vec<KeyValue>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum PortDirection {
    In,
    Out,
    Io,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RangeSpec {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PortRef {
    pub instance: Option<String>,
    pub port: String,
    pub index: Option<IndexSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct IndexSpec {
    pub elements: Vec<IndexElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum IndexElement {
    Single { value: u32 },
    Range { start: u32, end: u32 },
    Auto,
}

/// Which side of a connection an auto-resolution applies to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum PortSide {
    Source,
    Target,
}

/// A single resolved `[auto]` — maps a connection + side to concrete indices.
#[derive(Debug, Clone)]
pub struct AutoResolution {
    /// The connection span this resolution applies to
    pub span: Span,
    /// Which side of the connection
    pub side: PortSide,
    /// The resolved concrete indices
    pub resolved: IndexSpec,
}

/// Collection of all auto-resolutions from a compilation.
#[derive(Debug, Clone, Default)]
pub struct AutoResolutions {
    pub resolutions: Vec<AutoResolution>,
}

/// Value in a key-value pair: string, number, or port reference.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum ParamValue {
    Str { value: String },
    Num { value: u32 },
}

/// Value in a key-value property pair.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind")]
pub enum KvValue {
    Str { value: String },
    Num { value: u32 },
    PortRef(PortRef),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct KeyValue {
    pub key: String,
    pub value: KvValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SlotDef {
    pub name: String,
    pub range: Option<RangeSpec>,
    pub slot_type: String,
    pub properties: Vec<KeyValue>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RouteEntry {
    pub source: PortRef,
    pub target: PortRef,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BusOutput {
    /// User-defined name for this output, e.g. "Link 1-L". Required, non-empty.
    pub label: String,
    /// Where this output is routed. Empty = declared but unrouted.
    pub destinations: Vec<PortRef>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BusEntry {
    pub name: String,
    /// Human-readable display name. May contain characters invalid in identifiers
    /// (e.g. `"SPOTIFY>FOH"`). The `name` field remains the stable cross-reference key.
    pub label: Option<String>,
    pub inputs: Vec<PortRef>,
    pub outputs: Vec<BusOutput>,
    /// Bus insert legs — ordered, `[L]` mono, `[L, R]` stereo (issue #31).
    ///
    /// `#[serde(default)]` is REQUIRED, not cosmetic: `patchlang-wasm`'s `add_bus` and
    /// `update_bus` deserialize `BusEntry` *directly* from frontend JSON rather than
    /// going through a `*EmitInput` DTO. Without the default, serde rejects the whole
    /// payload with "missing field `insert_send`" and every existing frontend build
    /// breaks the moment this ships.
    #[serde(default)]
    pub insert_send: Vec<PortRef>,
    #[serde(default)]
    pub insert_return: Vec<PortRef>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SlotAssignment {
    pub slot_name: String,
    pub index: Option<u32>,
    pub card_name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RingMember {
    pub instance_name: String,
    pub port_name: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RingDecl {
    pub name: String,
    pub properties: Vec<KeyValue>,
    pub members: Vec<RingMember>,
    pub span: Span,
}

/// A member of a `network` block.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind")]
pub enum NetworkMember {
    /// Device-level membership: just the instance name (e.g. `member SL_StageBox`).
    DeviceLevel { instance: String, span: Span },
    /// Port-group membership: `member Instance.PortGroup`.
    PortGroup { instance: String, port_group: String, span: Span },
    /// Slot reference: `member Instance.slot[N]`.
    SlotRef { instance: String, index: u32, span: Span },
}

impl NetworkMember {
    pub fn instance_name(&self) -> &str {
        match self {
            NetworkMember::DeviceLevel { instance, .. } => instance,
            NetworkMember::PortGroup { instance, .. } => instance,
            NetworkMember::SlotRef { instance, .. } => instance,
        }
    }

    pub fn span(&self) -> &Span {
        match self {
            NetworkMember::DeviceLevel { span, .. } => span,
            NetworkMember::PortGroup { span, .. } => span,
            NetworkMember::SlotRef { span, .. } => span,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NetworkDecl {
    pub name: String,
    pub properties: Vec<KeyValue>,
    pub members: Vec<NetworkMember>,
    pub span: Span,
}
