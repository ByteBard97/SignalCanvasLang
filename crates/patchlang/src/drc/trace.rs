//! Trace DRC checks — rules TR01, TR02.
//!
//! TR01: Signal origin has no outgoing connections.
//! TR02: Signal origin has connections but reachable set contains no Out or Io port.
//!
//! See D019 in docs/decisions.md for rationale.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::ast::{PatchProgram, PortDirection, Statement};
use crate::drc::helpers::{collect_all_connects, DRCContext};
use crate::drc::types::{DRCLayer, Diagnostic, Severity};

const LAYER: DRCLayer = DRCLayer::Trace;

/// (instance_name, port_name) — channel indices are ignored (port-level only).
type PortKey = (String, String);

pub fn check(program: &PatchProgram, ctx: &DRCContext<'_>) -> Vec<Diagnostic> {
    let edges = build_edge_map(program, ctx);
    let mut diags = Vec::new();

    for stmt in &program.statements {
        if let Statement::Signal(sig) = stmt {
            let origin = match &sig.origin {
                Some(o) => o,
                None => continue,
            };
            let instance_name = match &origin.instance {
                Some(n) => n.as_str(),
                None => continue,
            };

            // Skip if instance or port unknown — S08/S09 already fired.
            if !ctx.instance_map.contains_key(instance_name) {
                continue;
            }
            if ctx
                .effective_ports
                .get(instance_name)
                .is_none_or(|ports| {
                    !ports.iter().any(|ep| ep.port_def.name == origin.port)
                })
            {
                continue;
            }

            let key: PortKey = (instance_name.to_string(), origin.port.clone());
            let neighbors = edges.get(&key).map(|v| v.as_slice()).unwrap_or(&[]);

            if neighbors.is_empty() {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    layer: LAYER.clone(),
                    message: format!(
                        "Signal '{}' origin '{}.{}' has no outgoing connections — add a bridge or connect from this port",
                        sig.name, instance_name, origin.port
                    ),
                    span: Some(sig.span.clone()),
                    source: Some(format!("{}.{}", instance_name, origin.port)),
                    target: None,
                    fix: Some(format!(
                        "Add 'bridge {}.{} -> ...' or 'connect {}.{} -> ...'",
                        instance_name, origin.port, instance_name, origin.port
                    )),
                });
                continue;
            }

            // BFS: check if any reachable port (including origin) has direction Out or Io.
            if !reachable_has_output(&key, &edges, ctx) {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    layer: LAYER.clone(),
                    message: format!(
                        "Signal '{}' cannot reach any output port — trace from '{}.{}' only visits input ports",
                        sig.name, instance_name, origin.port
                    ),
                    span: Some(sig.span.clone()),
                    source: Some(format!("{}.{}", instance_name, origin.port)),
                    target: None,
                    fix: Some(
                        "Ensure connects or bridges lead to a port with direction 'out' or 'io'"
                            .to_string(),
                    ),
                });
            }
        }
    }

    diags
}

/// BFS from `start`; returns true if any reachable node (including start) is Out or Io.
fn reachable_has_output(
    start: &PortKey,
    edges: &HashMap<PortKey, Vec<PortKey>>,
    ctx: &DRCContext<'_>,
) -> bool {
    let mut visited: HashSet<PortKey> = HashSet::new();
    let mut queue: VecDeque<PortKey> = VecDeque::new();

    queue.push_back(start.clone());
    visited.insert(start.clone());

    while let Some(current) = queue.pop_front() {
        if port_is_output(&current, ctx) {
            return true;
        }
        if let Some(neighbors) = edges.get(&current) {
            for neighbor in neighbors {
                if visited.insert(neighbor.clone()) {
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }

    false
}

/// Returns true if the port at `(instance, port_name)` has direction Out or Io.
fn port_is_output(key: &PortKey, ctx: &DRCContext<'_>) -> bool {
    let ports = match ctx.effective_ports.get(key.0.as_str()) {
        Some(p) => p,
        None => return false,
    };
    ports.iter().any(|ep| {
        ep.port_def.name == key.1
            && matches!(
                ep.port_def.direction,
                PortDirection::Out | PortDirection::Io
            )
    })
}

/// Build directed edge map from all connects, bridges, bridge-groups, template bridges, and routes.
fn build_edge_map(program: &PatchProgram, ctx: &DRCContext<'_>) -> HashMap<PortKey, Vec<PortKey>> {
    let mut edges: HashMap<PortKey, Vec<PortKey>> = HashMap::new();

    // Top-level connects (including link-groups via collect_all_connects).
    for conn in collect_all_connects(program) {
        let src_inst = match &conn.source.instance {
            Some(n) => n.clone(),
            None => continue,
        };
        let tgt_inst = match &conn.target.instance {
            Some(n) => n.clone(),
            None => continue,
        };
        edges
            .entry((src_inst, conn.source.port.clone()))
            .or_default()
            .push((tgt_inst, conn.target.port.clone()));
    }

    // Top-level bridges.
    for stmt in &program.statements {
        if let Statement::Bridge(b) = stmt {
            let src_inst = match &b.source.instance {
                Some(n) => n.clone(),
                None => continue,
            };
            let tgt_inst = match &b.target.instance {
                Some(n) => n.clone(),
                None => continue,
            };
            edges
                .entry((src_inst, b.source.port.clone()))
                .or_default()
                .push((tgt_inst, b.target.port.clone()));
        }
    }

    // Top-level bridge-groups.
    for stmt in &program.statements {
        if let Statement::BridgeGroup(bg) = stmt {
            let tgt_inst = match &bg.target.instance {
                Some(n) => n.clone(),
                None => continue,
            };
            for src in &bg.sources {
                let src_inst = match &src.instance {
                    Some(n) => n.clone(),
                    None => continue,
                };
                edges
                    .entry((src_inst, src.port.clone()))
                    .or_default()
                    .push((tgt_inst.clone(), bg.target.port.clone()));
            }
        }
    }

    // Template-internal bridges applied to each instance.
    for (inst_name, inst) in &ctx.instance_map {
        if let Some(tmpl) = ctx.template_map.get(inst.template_name.as_str()) {
            for b in &tmpl.bridges {
                // Template bridges use bare port names (no instance prefix).
                edges
                    .entry((inst_name.to_string(), b.source.port.clone()))
                    .or_default()
                    .push((inst_name.to_string(), b.target.port.clone()));
            }
        }
    }

    // Instance routes. An endpoint may carry a cross-instance qualifier
    // (Backbone-paired Engine↔Surface routing); when present the edge node must
    // be keyed on that referenced instance, not the route's owning instance, so
    // reachability follows the real signal path (issue #28).
    for (inst_name, inst) in &ctx.instance_map {
        for route in &inst.routes {
            let src_inst = route
                .source
                .instance
                .clone()
                .unwrap_or_else(|| inst_name.to_string());
            let tgt_inst = route
                .target
                .instance
                .clone()
                .unwrap_or_else(|| inst_name.to_string());
            edges
                .entry((src_inst, route.source.port.clone()))
                .or_default()
                .push((tgt_inst, route.target.port.clone()));
        }
    }

    edges
}
