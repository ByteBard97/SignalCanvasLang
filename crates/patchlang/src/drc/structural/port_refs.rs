//! Structural DRC checks for port references in connects, routes, and buses
//! (S03–S06, S14, S15). Split out of `structural` to keep files focused.

use std::collections::HashSet;

use crate::ast::{
    ConnectDecl, IndexElement, PatchProgram, PortRef, Statement,
};
use crate::drc::helpers::{collect_all_connects, expand_index_spec, is_suppressed, resolve_effective_port, DRCContext, port_ref_label};
use crate::drc::types::{Diagnostic, Severity};
use super::LAYER;

/// S03, S06 — Connect references unknown port or channel out of range.
pub(super) fn check_connect_port_refs(
    program: &PatchProgram,
    ctx: &DRCContext<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    let connects = collect_all_connects(program);
    for conn in &connects {
        check_port_ref_exists(&conn.source, ctx, conn, diags);
        check_port_ref_exists(&conn.target, ctx, conn, diags);
    }
}

/// Check a single PortRef in a connect — S03 (port exists) and S06 (channel range).
fn check_port_ref_exists(
    port_ref: &PortRef,
    ctx: &DRCContext<'_>,
    conn: &ConnectDecl,
    diags: &mut Vec<Diagnostic>,
) {
    let instance_name = match &port_ref.instance {
        Some(name) => name.as_str(),
        None => return, // local port ref — skip structural check
    };

    // Skip if instance itself is unknown (S01 already catches that)
    let instance = match ctx.instance_map.get(instance_name) {
        Some(i) => i,
        None => return,
    };

    // Template lookup for error messages only
    if !ctx.template_map.contains_key(instance.template_name.as_str()) {
        return; // template unknown, S01 handles this
    }

    // Use effective port map (template ports + card ports)
    let port_def = resolve_effective_port(instance_name, &port_ref.port, ctx);
    match port_def {
        None => {
            diags.push(Diagnostic {
                severity: Severity::Error,
                layer: LAYER.clone(),
                message: format!(
                    "Port '{}' does not exist on instance '{}' (template '{}')",
                    port_ref.port, instance_name, instance.template_name
                ),
                span: Some(conn.span.clone()),
                source: None,
                target: None,
                fix: Some(format!(
                    "Check the port name on template '{}'",
                    instance.template_name
                )),
            });
        }
        Some(pd) => {
            // S14 — vector port without index
            check_vector_port_indexed(port_ref, pd, instance_name, &conn.span, &conn.suppressions, diags);

            // S06 — check channel index bounds
            if let Some(index_spec) = &port_ref.index {
                let channels = crate::drc::helpers::expand_index_spec(index_spec);
                if let Some(range) = &pd.range {
                    for ch in &channels {
                        if *ch < range.start || *ch > range.end {
                            let label =
                                port_ref_label(instance_name, &port_ref.port, Some(*ch));
                            diags.push(Diagnostic {
                                severity: Severity::Error,
                                layer: LAYER.clone(),
                                message: format!(
                                    "Channel index [{}] is out of range for port '{}' (range [{}..{}])",
                                    ch, port_ref.port, range.start, range.end
                                ),
                                span: Some(conn.span.clone()),
                                source: Some(label),
                                target: None,
                                fix: Some(format!(
                                    "Use an index between {} and {}",
                                    range.start, range.end
                                )),
                            });
                        }
                    }
                }
            }
        }
    }
}

/// S14 — Vector port referenced without channel index.
fn check_vector_port_indexed(
    port_ref: &PortRef,
    port_def: &crate::ast::PortDef,
    instance_name: &str,
    span: &crate::error::Span,
    suppressions: &[String],
    diags: &mut Vec<Diagnostic>,
) {
    if is_suppressed(suppressions, "structural") {
        return;
    }
    if port_ref.index.is_none() {
        if let Some(range) = &port_def.range {
            diags.push(Diagnostic {
                severity: Severity::Warning,
                layer: LAYER.clone(),
                message: format!(
                    "Port '{}' on '{}' is a vector port [{}..{}] — no channel index specified",
                    port_ref.port, instance_name, range.start, range.end
                ),
                span: Some(span.clone()),
                source: None,
                target: None,
                fix: Some(format!(
                    "Specify channels, e.g. {}.{}[1..2], or use [auto] for auto-assignment",
                    instance_name, port_ref.port
                )),
            });
        }
    }
}

/// S15 — Range size mismatch: left and right sides of a connect have different channel counts.
///
/// Skipped if either side uses `[auto]` (auto-assignment resolves the count).
/// Can be suppressed with `@suppress(structural)` for intentional partial connects.
pub(super) fn check_connect_range_sizes(program: &PatchProgram, diags: &mut Vec<Diagnostic>) {
    let connects = collect_all_connects(program);
    for conn in &connects {
        if is_suppressed(&conn.suppressions, "structural") {
            continue;
        }

        let src_index = match &conn.source.index {
            Some(i) => i,
            None => continue, // no index spec — S14 handles unindexed vector ports
        };
        let tgt_index = match &conn.target.index {
            Some(i) => i,
            None => continue,
        };

        // Skip if either side uses [auto] — the compiler resolves the count
        let src_has_auto = src_index.elements.iter().any(|e| matches!(e, IndexElement::Auto));
        let tgt_has_auto = tgt_index.elements.iter().any(|e| matches!(e, IndexElement::Auto));
        if src_has_auto || tgt_has_auto {
            continue;
        }

        let src_channels = expand_index_spec(src_index);
        let tgt_channels = expand_index_spec(tgt_index);

        if src_channels.len() != tgt_channels.len() {
            let src_label = port_ref_label(
                conn.source.instance.as_deref().unwrap_or(""),
                &conn.source.port,
                None,
            );
            let tgt_label = port_ref_label(
                conn.target.instance.as_deref().unwrap_or(""),
                &conn.target.port,
                None,
            );
            diags.push(Diagnostic {
                severity: Severity::Error,
                layer: LAYER.clone(),
                message: format!(
                    "Range size mismatch: '{}' maps {} channel(s) but '{}' has {} channel(s)",
                    src_label,
                    src_channels.len(),
                    tgt_label,
                    tgt_channels.len(),
                ),
                span: Some(conn.span.clone()),
                source: Some(src_label),
                target: Some(tgt_label),
                fix: Some(
                    "Make both ranges the same size, or add @suppress(structural) \
                     if this partial connect is intentional"
                        .to_string(),
                ),
            });
        }
    }
}

/// Emit a diagnostic when a port name does not exist on a template.
fn emit_missing_port_diagnostic(
    port_name: &str,
    template_name: &str,
    context_label: &str,
    span: &crate::error::Span,
    diags: &mut Vec<Diagnostic>,
) {
    diags.push(Diagnostic {
        severity: Severity::Error,
        layer: LAYER.clone(),
        message: format!(
            "{context_label} '{port_name}' does not exist on template '{template_name}'"
        ),
        span: Some(span.clone()),
        source: None,
        target: None,
        fix: Some(format!("Check the port name on template '{template_name}'")),
    });
}

/// S04 — Route references port that doesn't exist on the instance (template + card ports).
pub(super) fn check_route_port_refs(
    program: &PatchProgram,
    ctx: &DRCContext<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    for stmt in &program.statements {
        if let Statement::Instance(inst) = stmt {
            if !ctx.template_map.contains_key(inst.template_name.as_str()) {
                continue; // S01 handles unknown template
            }

            for route in &inst.routes {
                check_route_endpoint(&route.source, inst, &route.span, ctx, diags);
                check_route_endpoint(&route.target, inst, &route.span, ctx, diags);
            }
        }
    }
}

/// Validate one route endpoint against the template that actually owns the port.
///
/// A route endpoint may carry a cross-instance qualifier (`PairedInstance.Port`,
/// used for Backbone-paired Engine↔Surface routing). When present the port is
/// resolved against that referenced instance's template rather than the owning
/// instance's — otherwise a legitimate cross-instance route would be flagged as
/// a missing-port error (issue #28). Unknown instances/templates are skipped
/// here because S01 already reports them.
fn check_route_endpoint(
    port_ref: &PortRef,
    owning_inst: &crate::ast::InstanceDecl,
    span: &crate::error::Span,
    ctx: &DRCContext<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    let (inst_name, template_name) = match &port_ref.instance {
        Some(name) => match ctx.instance_map.get(name.as_str()) {
            Some(other) => {
                if !ctx.template_map.contains_key(other.template_name.as_str()) {
                    return; // S01 handles the unknown template
                }
                (name.as_str(), other.template_name.as_str())
            }
            None => return, // S01 handles the unknown instance
        },
        None => (owning_inst.name.as_str(), owning_inst.template_name.as_str()),
    };

    match resolve_effective_port(inst_name, &port_ref.port, ctx) {
        None => emit_missing_port_diagnostic(
            &port_ref.port,
            template_name,
            "Route references port",
            span,
            diags,
        ),
        Some(pd) => check_vector_port_indexed(port_ref, pd, inst_name, span, &[], diags),
    }
}

/// S05 — Bus output references port that doesn't exist on the instance (template + card ports).
pub(super) fn check_bus_port_refs(
    program: &PatchProgram,
    ctx: &DRCContext<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    for stmt in &program.statements {
        if let Statement::Instance(inst) = stmt {
            if !ctx.template_map.contains_key(inst.template_name.as_str()) {
                continue;
            }

            for bus in &inst.buses {
                for output in &bus.outputs {
                    for dest in &output.destinations {
                        if dest.instance.is_some() {
                            continue; // cross-device ref — validated by target instance
                        }
                        match resolve_effective_port(&inst.name, &dest.port, ctx) {
                            None => emit_missing_port_diagnostic(
                                &dest.port,
                                &inst.template_name,
                                "Bus output",
                                &bus.span,
                                diags,
                            ),
                            Some(pd) => check_vector_port_indexed(
                                dest, pd, &inst.name, &bus.span, &[], diags,
                            ),
                        }
                    }
                    // Unrouted outputs (destinations empty) skip S05 validation.
                }

                // C-new: Duplicate bus output label warning.
                let mut seen_labels: HashSet<&str> = HashSet::new();
                for output in &bus.outputs {
                    if !output.label.is_empty() && !seen_labels.insert(output.label.as_str()) {
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            layer: LAYER.clone(),
                            message: format!(
                                "Duplicate bus output label \"{}\" in bus '{}'",
                                output.label, bus.name
                            ),
                            span: Some(output.span.clone()),
                            source: None,
                            target: None,
                            fix: Some(format!(
                                "Rename one of the outputs with label \"{}\"",
                                output.label
                            )),
                        });
                    }
                }

                for input in &bus.inputs {
                    if input.instance.is_some() {
                        continue; // cross-device ref — validated by target instance
                    }
                    match resolve_effective_port(&inst.name, &input.port, ctx) {
                        None => emit_missing_port_diagnostic(
                            &input.port,
                            &inst.template_name,
                            "Bus input",
                            &bus.span,
                            diags,
                        ),
                        Some(pd) => check_vector_port_indexed(
                            input, pd, &inst.name, &bus.span, &[], diags,
                        ),
                    }
                }
            }
        }
    }
}
