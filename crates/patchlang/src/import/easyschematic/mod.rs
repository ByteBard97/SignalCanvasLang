use serde::Deserialize;
use serde_json::Value;

use crate::ast::{InstanceDecl, KeyValue, KvValue, PortDef, PortRef, TemplateDecl};
use crate::builder::PatchProgramBuilder;
use crate::error::Span;

use super::mapping::{
    connector_to_patchlang, es_direction_to_patchlang, sanitize_identifier,
    sanitize_port_name, signal_type_to_attribute,
};
use super::stubs::resolve_stubs;
use super::templates::build_template_assignments;

// ---------------------------------------------------------------------------
// JSON deserialization types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SchematicFile {
    #[allow(dead_code)]
    version: u32,
    #[allow(dead_code)]
    name: String,
    nodes: Vec<RawNode>,
    edges: Vec<RawEdge>,
    #[serde(rename = "customTemplates", default)]
    _custom_templates: Vec<Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct RawNode {
    pub(super) id: String,
    #[serde(rename = "type")]
    pub(super) node_type: String,
    pub(super) position: RawPosition,
    pub(super) data: Value,
    #[serde(rename = "parentId", default)]
    pub(super) parent_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub(super) struct RawPosition {
    pub(super) x: f64,
    pub(super) y: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawEdge {
    #[allow(dead_code)]
    pub(super) id: String,
    pub(super) source: String,
    pub(super) target: String,
    #[serde(rename = "sourceHandle")]
    pub(super) source_handle: Option<String>,
    #[serde(rename = "targetHandle")]
    pub(super) target_handle: Option<String>,
    pub(super) data: Option<RawEdgeData>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawEdgeData {
    #[serde(rename = "signalType")]
    #[allow(dead_code)]
    pub(super) signal_type: Option<String>,
    #[serde(rename = "cableId")]
    pub(super) cable_id: Option<String>,
    #[serde(rename = "cableLength")]
    pub(super) cable_length: Option<String>,
    pub(super) label: Option<String>,
    #[serde(rename = "linkedConnectionId")]
    pub(super) linked_connection_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct EsPort {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) signal_type: String,
    pub(super) direction: String,
    pub(super) connector_type: Option<String>,
}

impl EsPort {
    pub(super) fn from_value(v: &Value) -> Option<Self> {
        Some(EsPort {
            id: v["id"].as_str()?.to_string(),
            label: v["label"].as_str()?.to_string(),
            signal_type: v["signalType"].as_str().unwrap_or("").to_string(),
            direction: v["direction"].as_str().unwrap_or("input").to_string(),
            connector_type: v["connectorType"].as_str().map(|s| s.to_string()),
        })
    }

    /// Returns false for signal types that have no representation in PatchLang:
    /// mains power, USB peripherals, and consumer/legacy video signals.
    pub(super) fn is_signal_flow_relevant(&self) -> bool {
        !matches!(
            self.signal_type.as_str(),
            "usb" | "thunderbolt" | "displayport" | "serial" | "composite" | "vga"
        ) && !self.signal_type.starts_with("power")
    }
}

#[derive(Debug, Clone)]
pub(super) struct EsDeviceData {
    pub(super) label: String,
    pub(super) model: Option<String>,
    pub(super) manufacturer: Option<String>,
    pub(super) model_number: Option<String>,
    pub(super) template_id: Option<String>,
    pub(super) ports: Vec<EsPort>,
}

impl EsDeviceData {
    pub(super) fn from_value(v: &Value) -> Option<Self> {
        let ports = v["ports"]
            .as_array()?
            .iter()
            .filter_map(EsPort::from_value)
            .filter(EsPort::is_signal_flow_relevant)
            .collect();
        Some(EsDeviceData {
            label: v["label"].as_str().unwrap_or("Device").to_string(),
            model: v["model"].as_str().map(|s| s.to_string()),
            manufacturer: v["manufacturer"].as_str().map(|s| s.to_string()),
            model_number: v["modelNumber"].as_str().map(|s| s.to_string()),
            template_id: v["templateId"].as_str().map(|s| s.to_string()),
            ports,
        })
    }
}

// ---------------------------------------------------------------------------
// Public output types
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize)]
pub struct DeviceSummary {
    pub instance_name: String,
    pub template_name: String,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub model_number: Option<String>,
    pub label: String,
}

#[derive(Debug, serde::Serialize)]
pub struct ImportResult {
    pub patch: String,
    pub layout: serde_json::Value,
    /// Connections that couldn't be emitted (e.g. direction violations). Non-fatal.
    pub warnings: Vec<String>,
    pub devices: Vec<DeviceSummary>,
}

#[derive(Debug)]
pub struct ImportError(pub String);

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ImportError {}

impl From<serde_json::Error> for ImportError {
    fn from(e: serde_json::Error) -> Self {
        ImportError(format!("JSON parse error: {e}"))
    }
}

fn build_err(msg: impl Into<String>) -> ImportError {
    ImportError(msg.into())
}

fn null_span() -> Span {
    Span { start: 0, end: 0, file: None }
}

// ---------------------------------------------------------------------------
// Core importer
// ---------------------------------------------------------------------------

pub fn import_easyschematic(json: &str) -> Result<ImportResult, ImportError> {
    use std::collections::HashMap;

    let sf: SchematicFile = serde_json::from_str(json)?;

    let mut device_pairs: Vec<(RawNode, EsDeviceData)> = Vec::new();
    let mut annotation_nodes: Vec<(RawNode, String)> = Vec::new();

    for node in &sf.nodes {
        match node.node_type.as_str() {
            "device" => {
                if let Some(dev) = EsDeviceData::from_value(&node.data) {
                    device_pairs.push((node.clone(), dev));
                }
            }
            "room" | "note" | "annotation" => {
                let label = node.data["label"].as_str().unwrap_or("").to_string();
                annotation_nodes.push((node.clone(), label));
            }
            _ => {}
        }
    }

    // Assign sanitized, deduped instance names from device labels
    let mut used_instance_names: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let mut node_to_instance_name: HashMap<String, String> = HashMap::new();

    for (node, dev) in &device_pairs {
        let base = sanitize_identifier(&dev.label);
        let inst_name = if used_instance_names.contains(&base) {
            let mut n = 2u32;
            loop {
                let candidate = format!("{}_{}", base, n);
                if !used_instance_names.contains(&candidate) {
                    break candidate;
                }
                n += 1;
            }
        } else {
            base
        };
        used_instance_names.insert(inst_name.clone());
        node_to_instance_name.insert(node.id.clone(), inst_name);
    }

    let assignments = build_template_assignments(&device_pairs);

    // Build ordered (original_label, sanitized_name) pairs per template spec
    let mut template_port_names: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for spec in &assignments.specs {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let pairs = spec
            .ports
            .iter()
            .map(|p| (p.label.clone(), sanitize_port_name(&p.label, &mut seen)))
            .collect();
        template_port_names.insert(spec.name.clone(), pairs);
    }

    // Build port_id → (instance_name, sanitized_port_name) via positional zip.
    // NOT label-match: duplicate labels like "Input 1"×16 are common in broadcast.
    let mut port_id_to_ref: HashMap<String, (String, String)> = HashMap::new();
    for (node, dev) in &device_pairs {
        let inst_name = &node_to_instance_name[&node.id];
        let tmpl_name = &assignments.node_to_template[&node.id];
        let tmpl_ports = template_port_names.get(tmpl_name).cloned().unwrap_or_default();
        for (port, (_, sanitized)) in dev.ports.iter().zip(tmpl_ports.iter()) {
            port_id_to_ref.insert(port.id.clone(), (inst_name.clone(), sanitized.clone()));
        }
        for port in dev.ports.iter().skip(tmpl_ports.len()) {
            port_id_to_ref.insert(
                port.id.clone(),
                (inst_name.clone(), sanitize_identifier(&port.label)),
            );
        }
    }

    let logical_edges = resolve_stubs(&sf.nodes, &sf.edges);

    let mut builder = PatchProgramBuilder::new();

    // Add templates
    for spec in &assignments.specs {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let ports: Vec<PortDef> = spec
            .ports
            .iter()
            .map(|p| {
                let name = sanitize_port_name(&p.label, &mut seen);
                let mut attributes: Vec<String> = Vec::new();
                if let Some(attr) = signal_type_to_attribute(&p.signal_type) {
                    attributes.push(attr.to_string());
                }
                let connector = p
                    .connector_type
                    .as_deref()
                    .and_then(connector_to_patchlang)
                    .map(|s| s.to_string());
                PortDef {
                    name,
                    range: None,
                    direction: es_direction_to_patchlang(&p.direction),
                    connector,
                    attributes,
                    named_attributes: Vec::new(),
                    span: null_span(),
                }
            })
            .collect();

        // Collect meta from the first device using this template.
        // manufacturer, model, model_number, and es_template_id are preserved
        // so EasySchematic roundtrip can reconstruct the original JSON fields.
        let first_dev = device_pairs
            .iter()
            .find(|(n, _)| assignments.node_to_template[&n.id] == spec.name)
            .map(|(_, d)| d);
        let mut meta: Vec<KeyValue> = Vec::new();
        let kv_str = |k: &str, v: String| KeyValue { key: k.to_string(), value: KvValue::Str { value: v } };
        if let Some(dev) = first_dev {
            if let Some(m) = &dev.manufacturer  { meta.push(kv_str("manufacturer",   m.clone())); }
            if let Some(m) = &dev.model         { meta.push(kv_str("model",          m.clone())); }
            if let Some(m) = &dev.model_number  { meta.push(kv_str("model_number",   m.clone())); }
            if let Some(t) = &dev.template_id   { meta.push(kv_str("es_template_id", t.clone())); }
        }

        let decl = TemplateDecl {
            name: spec.name.clone(),
            params: Vec::new(),
            version: None,
            meta,
            ports,
            bridges: Vec::new(),
            instances: Vec::new(),
            connects: Vec::new(),
            slots: Vec::new(),
            span: null_span(),
        };
        builder
            .add_template(decl)
            .map_err(|e| build_err(format!("template '{}': {e}", spec.name)))?;
    }

    // Add instances
    for (node, dev) in &device_pairs {
        let inst_name = node_to_instance_name[&node.id].clone();
        let tmpl_name = assignments.node_to_template[&node.id].clone();
        let kv = |k: &str, v: &str| KeyValue { key: k.to_string(), value: KvValue::Str { value: v.to_string() } };
        let mut properties = vec![kv("location", &dev.label)];
        // Preserve the original EasySchematic node ID so an exporter can
        // reconstruct the source JSON without losing identity.
        properties.push(kv("es_node_id", &node.id));
        let decl = InstanceDecl {
            name: inst_name,
            template_name: tmpl_name,
            args: Vec::new(),
            version_constraint: None,
            properties,
            routes: Vec::new(),
            buses: Vec::new(),
            slot_assignments: Vec::new(),
            span: null_span(),
        };
        builder
            .add_instance(decl)
            .map_err(|e| build_err(format!("instance '{}': {e}", dev.label)))?;
    }

    // Add connections
    let mut connection_warnings: Vec<String> = Vec::new();

    for edge in &logical_edges {
        let src_ref = edge.source_port_id.as_deref().and_then(|pid| port_id_to_ref.get(pid));
        let tgt_ref = edge.target_port_id.as_deref().and_then(|pid| port_id_to_ref.get(pid));

        if node_to_instance_name.get(&edge.source_node_id).is_none()
            || node_to_instance_name.get(&edge.target_node_id).is_none()
        {
            continue;
        }

        if let (Some((s_inst, s_port)), Some((t_inst, t_port))) = (src_ref, tgt_ref) {
            let mut properties: Vec<KeyValue> = Vec::new();
            if let Some(cid) = &edge.cable_id {
                properties.push(KeyValue {
                    key: "cable".to_string(),
                    value: KvValue::Str { value: cid.clone() },
                });
            }
            if let Some(cl) = &edge.cable_length {
                properties.push(KeyValue {
                    key: "length".to_string(),
                    value: KvValue::Str { value: cl.clone() },
                });
            }
            let source = PortRef { instance: Some(s_inst.clone()), port: s_port.clone(), index: None };
            let target = PortRef { instance: Some(t_inst.clone()), port: t_port.clone(), index: None };

            if let Err(e) = builder.add_connect(source, target, properties) {
                connection_warnings.push(format!(
                    "skipped {}.{} → {}.{}: {e}",
                    s_inst, s_port, t_inst, t_port
                ));
            }
        }
    }

    let patch = builder.format();

    // Build room-position lookup so child nodes can resolve absolute coordinates.
    // EasySchematic uses ReactFlow parent-child positioning: a node with parentId
    // has a position relative to its parent room, not the canvas.
    let mut room_positions: HashMap<String, (f64, f64)> = HashMap::new();
    for node in &sf.nodes {
        if node.node_type == "room" {
            room_positions.insert(node.id.clone(), (node.position.x, node.position.y));
        }
    }

    let resolve_abs = |node: &RawNode| -> (f64, f64) {
        let (px, py) = node.parent_id.as_deref()
            .and_then(|pid| room_positions.get(pid))
            .copied()
            .unwrap_or((0.0, 0.0));
        (node.position.x + px, node.position.y + py)
    };

    // Build layout sidecar
    let mut positions = serde_json::Map::new();
    for (node, _) in &device_pairs {
        let inst_name = &node_to_instance_name[&node.id];
        let (ax, ay) = resolve_abs(node);
        positions.insert(
            inst_name.clone(),
            serde_json::json!({ "x": ax, "y": ay }),
        );
    }

    let annotations: Vec<serde_json::Value> = annotation_nodes
        .iter()
        .map(|(node, label)| {
            let (ax, ay) = resolve_abs(node);
            serde_json::json!({
                "type": node.node_type,
                "label": label,
                "x": ax,
                "y": ay
            })
        })
        .collect();

    let layout = serde_json::json!({
        "version": 2,
        "positions": positions,
        "annotations": annotations
    });

    let devices: Vec<DeviceSummary> = device_pairs
        .iter()
        .map(|(node, dev)| {
            let instance_name = node_to_instance_name[&node.id].clone();
            let template_name = assignments.node_to_template[&node.id].clone();
            DeviceSummary {
                instance_name,
                template_name,
                model: dev.model.clone(),
                manufacturer: dev.manufacturer.clone(),
                model_number: dev.model_number.clone(),
                label: dev.label.clone(),
            }
        })
        .collect();

    Ok(ImportResult { patch, layout, warnings: connection_warnings, devices })
}

#[cfg(test)]
mod tests;
