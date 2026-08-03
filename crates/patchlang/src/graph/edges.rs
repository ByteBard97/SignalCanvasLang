//! Edge expansion — converts connect, bridge, and bridge-group declarations into edges.

use std::collections::BTreeMap;

use crate::ast::{BridgeDecl, BridgeGroupDecl, ConnectDecl, IndexSpec, PortRef};
use crate::compat::parse_mapping_spec;
use crate::compat_types::TsMappingSpec;

use super::types::GraphEdge;

// ---------------------------------------------------------------------------
// Index helpers
// ---------------------------------------------------------------------------

/// Flatten an `IndexSpec` into a list of concrete indices.
pub(crate) fn flatten_index_spec(spec: &IndexSpec) -> Vec<u32> {
    spec.flatten()
}

/// Flatten an optional `IndexSpec` into indices. Returns `vec![]` for None.
fn flatten_opt(spec: &Option<IndexSpec>) -> Vec<Option<u32>> {
    match spec {
        Some(s) => {
            let flat = flatten_index_spec(s);
            if flat.is_empty() {
                vec![None]
            } else {
                flat.into_iter().map(Some).collect()
            }
        }
        None => vec![None],
    }
}

/// Convert KeyValue vec to BTreeMap<String, String> for edge properties.
fn kv_to_map(kvs: &[crate::ast::KeyValue]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for kv in kvs {
        let val = match &kv.value {
            crate::ast::KvValue::Str { value } => value.clone(),
            crate::ast::KvValue::Num { value } => value.to_string(),
            crate::ast::KvValue::PortRef(pr) => {
                let inst = pr.instance.as_deref().unwrap_or("");
                format!("{}.{}", inst, pr.port)
            }
        };
        map.insert(kv.key.clone(), val);
    }
    map
}

// ---------------------------------------------------------------------------
// Bridge expansion
// ---------------------------------------------------------------------------

/// Expand a bridge declaration into edges.
///
/// - Equal-length ranges: 1:1 mapping with busId/busIndex/busSize.
/// - Unequal lengths: fan-out/fan-in (every source to every target).
pub(crate) fn expand_bridge(
    bridge: &BridgeDecl,
    src_prefix: &str,
    tgt_prefix: &str,
    edge_prefix: &str,
    source_node: &str,
    target_node: &str,
) -> BTreeMap<String, GraphEdge> {
    let mut edges = BTreeMap::new();
    let src_indices = flatten_opt(&bridge.source.index);
    let tgt_indices = flatten_opt(&bridge.target.index);

    if src_indices.len() == tgt_indices.len() {
        // 1:1 mapping
        let is_bus = src_indices.len() > 1 && src_indices[0].is_some();
        let bus_id = if is_bus {
            Some(format!("{edge_prefix}_bus"))
        } else {
            None
        };

        for (i, (src, tgt)) in src_indices.iter().zip(tgt_indices.iter()).enumerate() {
            let src_suffix = src.map_or(String::new(), |v| format!("_{v}"));
            let tgt_suffix = tgt.map_or(String::new(), |v| format!("_{v}"));
            let src_port_id = format!("{src_prefix}{}{src_suffix}", bridge.source.port);
            let tgt_port_id = format!("{tgt_prefix}{}{tgt_suffix}", bridge.target.port);
            let edge_id = format!("{edge_prefix}{src_suffix}_to{tgt_suffix}");

            let mut edge = GraphEdge {
                id: edge_id.clone(),
                source_node: source_node.to_string(),
                source_port: src_port_id,
                target_node: target_node.to_string(),
                target_port: tgt_port_id,
                edge_type: "bridge".to_string(),
                properties: BTreeMap::new(),
                bus_id: None,
                bus_index: None,
                bus_size: None,
            };
            if is_bus {
                edge.bus_id = bus_id.clone();
                edge.bus_index = Some(i);
                edge.bus_size = Some(src_indices.len());
            }
            edges.insert(edge_id, edge);
        }
    } else {
        // Fan-out/fan-in
        for src in &src_indices {
            for tgt in &tgt_indices {
                let src_suffix = src.map_or(String::new(), |v| format!("_{v}"));
                let tgt_suffix = tgt.map_or(String::new(), |v| format!("_{v}"));
                let src_port_id = format!("{src_prefix}{}{src_suffix}", bridge.source.port);
                let tgt_port_id = format!("{tgt_prefix}{}{tgt_suffix}", bridge.target.port);
                let edge_id = format!("{edge_prefix}{src_suffix}_to{tgt_suffix}");

                edges.insert(
                    edge_id.clone(),
                    GraphEdge {
                        id: edge_id,
                        source_node: source_node.to_string(),
                        source_port: src_port_id,
                        target_node: target_node.to_string(),
                        target_port: tgt_port_id,
                        edge_type: "bridge".to_string(),
                        properties: BTreeMap::new(),
                        bus_id: None,
                        bus_index: None,
                        bus_size: None,
                    },
                );
            }
        }
    }

    edges
}

// ---------------------------------------------------------------------------
// Connect edge expansion
// ---------------------------------------------------------------------------

/// Expand all connect declarations into edges.
pub(crate) fn expand_connect_edges(
    connects: &[ConnectDecl],
    edges: &mut BTreeMap<String, GraphEdge>,
) {
    for conn in connects {
        let mapping = conn
            .mapping
            .as_ref()
            .and_then(|raw| parse_mapping_spec(raw));
        match mapping {
            Some(TsMappingSpec::Explicit { ref pairs }) => {
                expand_explicit_mapping(conn, pairs, edges);
            }
            Some(TsMappingSpec::Offset { offset }) => {
                expand_offset_mapping(conn, offset, edges);
            }
            _ => {
                // OneToOne or no mapping
                expand_default_mapping(conn, edges);
            }
        }
    }
}

fn src_inst(pr: &PortRef) -> &str {
    pr.instance.as_deref().unwrap_or("")
}

fn expand_explicit_mapping(
    conn: &ConnectDecl,
    pairs: &[crate::compat_types::TsMappingPair],
    edges: &mut BTreeMap<String, GraphEdge>,
) {
    let props = kv_to_map(&conn.properties);
    let si = src_inst(&conn.source);
    let ti = src_inst(&conn.target);

    for pair in pairs {
        let src_port_id = format!("{si}:{}_{}", conn.source.port, pair.from);
        let tgt_port_id = format!("{ti}:{}_{}", conn.target.port, pair.to);
        let edge_id = format!(
            "connect_{si}_{}_{}_{}_{}_{}",
            conn.source.port, pair.from, ti, conn.target.port, pair.to
        );

        edges.insert(
            edge_id.clone(),
            GraphEdge {
                id: edge_id,
                source_node: si.to_string(),
                source_port: src_port_id,
                target_node: ti.to_string(),
                target_port: tgt_port_id,
                edge_type: "connect".to_string(),
                properties: props.clone(),
                bus_id: None,
                bus_index: None,
                bus_size: None,
            },
        );
    }
}

fn expand_offset_mapping(
    conn: &ConnectDecl,
    offset: i64,
    edges: &mut BTreeMap<String, GraphEdge>,
) {
    let props = kv_to_map(&conn.properties);
    let si = src_inst(&conn.source);
    let ti = src_inst(&conn.target);
    let src_indices = flatten_opt(&conn.source.index);

    for src in &src_indices {
        let src_suffix = src.map_or(String::new(), |v| format!("_{v}"));
        let tgt_idx = src.map(|v| (v as i64 + offset) as u32);
        let tgt_suffix = tgt_idx.map_or(String::new(), |v| format!("_{v}"));
        let src_port_id = format!("{si}:{}{src_suffix}", conn.source.port);
        let tgt_port_id = format!("{ti}:{}{tgt_suffix}", conn.target.port);
        let edge_id = format!(
            "connect_{si}_{}{src_suffix}_{ti}_{}{tgt_suffix}",
            conn.source.port, conn.target.port
        );

        edges.insert(
            edge_id.clone(),
            GraphEdge {
                id: edge_id,
                source_node: si.to_string(),
                source_port: src_port_id,
                target_node: ti.to_string(),
                target_port: tgt_port_id,
                edge_type: "connect".to_string(),
                properties: props.clone(),
                bus_id: None,
                bus_index: None,
                bus_size: None,
            },
        );
    }
}

fn expand_default_mapping(conn: &ConnectDecl, edges: &mut BTreeMap<String, GraphEdge>) {
    let props = kv_to_map(&conn.properties);
    let si = src_inst(&conn.source);
    let ti = src_inst(&conn.target);
    let src_indices = flatten_opt(&conn.source.index);
    let tgt_indices = flatten_opt(&conn.target.index);

    // Range mismatch check: both > 1 but unequal is an error — skip silently
    if src_indices.len() != tgt_indices.len()
        && src_indices.len() > 1
        && tgt_indices.len() > 1
    {
        return;
    }

    let count = src_indices.len().max(tgt_indices.len());
    for i in 0..count {
        let src = if src_indices.len() > 1 {
            src_indices[i]
        } else {
            src_indices[0]
        };
        let tgt = if tgt_indices.len() > 1 {
            tgt_indices[i]
        } else {
            tgt_indices[0]
        };

        let src_suffix = src.map_or(String::new(), |v| format!("_{v}"));
        let tgt_suffix = tgt.map_or(String::new(), |v| format!("_{v}"));
        let src_port_id = format!("{si}:{}{src_suffix}", conn.source.port);
        let tgt_port_id = format!("{ti}:{}{tgt_suffix}", conn.target.port);
        let edge_id = format!(
            "connect_{si}_{}{src_suffix}_{ti}_{}{tgt_suffix}",
            conn.source.port, conn.target.port
        );

        edges.insert(
            edge_id.clone(),
            GraphEdge {
                id: edge_id,
                source_node: si.to_string(),
                source_port: src_port_id,
                target_node: ti.to_string(),
                target_port: tgt_port_id,
                edge_type: "connect".to_string(),
                properties: props.clone(),
                bus_id: None,
                bus_index: None,
                bus_size: None,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Bridge group expansion
// ---------------------------------------------------------------------------

/// Expand bridge group declarations into edges with sequential channel mapping.
pub(crate) fn expand_bridge_group_edges(
    bridge_groups: &[BridgeGroupDecl],
    edges: &mut BTreeMap<String, GraphEdge>,
) {
    for bg in bridge_groups {
        // Compute total bus size
        let mut total_bus_size: usize = 0;
        for source in &bg.sources {
            let indices = flatten_opt(&source.index);
            total_bus_size += indices.len();
        }
        let tgt_inst = src_inst(&bg.target);
        let bus_id = format!("bridge_group_{tgt_inst}_{}_bus", bg.target.port);
        let is_bus = total_bus_size > 1;

        let tgt_range_start = bg
            .target
            .index
            .as_ref()
            .and_then(|s| flatten_index_spec(s).first().copied())
            .unwrap_or(1);

        let mut offset: usize = 0;
        for source in &bg.sources {
            let source_indices = flatten_opt(&source.index);
            let count = source_indices.len();
            let s_inst = src_inst(source);

            for (i, src_idx) in source_indices.iter().enumerate() {
                let tgt_idx = tgt_range_start + (offset + i) as u32;
                let src_suffix = src_idx.map_or(String::new(), |v| format!("_{v}"));
                let tgt_suffix = format!("_{tgt_idx}");
                let src_port_id = format!("{s_inst}:{}{src_suffix}", source.port);
                let tgt_port_id = format!("{tgt_inst}:{}{tgt_suffix}", bg.target.port);
                let edge_id =
                    format!("bridge_group_{tgt_inst}_{}_{}", bg.target.port, offset + i);

                let mut edge = GraphEdge {
                    id: edge_id.clone(),
                    source_node: s_inst.to_string(),
                    source_port: src_port_id,
                    target_node: tgt_inst.to_string(),
                    target_port: tgt_port_id,
                    edge_type: "bridge".to_string(),
                    properties: BTreeMap::new(),
                    bus_id: None,
                    bus_index: None,
                    bus_size: None,
                };
                if is_bus {
                    edge.bus_id = Some(bus_id.clone());
                    edge.bus_index = Some(offset + i);
                    edge.bus_size = Some(total_bus_size);
                }
                edges.insert(edge_id, edge);
            }

            offset += count;
        }
    }
}
