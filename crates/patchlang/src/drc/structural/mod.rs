//! Structural DRC checks — rules S01, S03–S15.
//!
//! These catch undefined references, duplicate names, and port reference issues.
//! Structural errors are hard errors that cannot be suppressed (except S14/S15 via
//! @suppress(structural)).
//!
//! Slot checks (S02, S12, S13) are in `slots.rs`.
//! Meta info hints (M-I01, M-I03, M-I04) are in `meta.rs`.

use std::collections::HashMap;

use crate::ast::{
    PatchProgram, Statement,
};
use crate::drc::helpers::{check_card_port_collisions, resolve_effective_port, DRCContext};
use crate::drc::types::{DRCLayer, Diagnostic, Severity};

const LAYER: DRCLayer = DRCLayer::Structural;

mod port_refs;
use port_refs::*;

/// Run all structural checks (including slot and meta checks from submodules).
pub fn check(program: &PatchProgram, ctx: &DRCContext<'_>) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    check_duplicate_instance_names(program, &mut diags);
    check_duplicate_signal_names(program, &mut diags);
    check_instance_template_refs(program, ctx, &mut diags);
    check_connect_port_refs(program, ctx, &mut diags);
    check_connect_range_sizes(program, &mut diags);
    check_route_port_refs(program, ctx, &mut diags);
    check_bus_port_refs(program, ctx, &mut diags);
    check_config_instance_refs(program, ctx, &mut diags);
    check_signal_origin_refs(program, ctx, &mut diags);
    super::slots::check_slot_card_refs(program, ctx, &mut diags);
    super::slots::check_slot_fits_compatibility(program, ctx, &mut diags);
    super::slots::check_fits_format_in_scope(program, ctx, &mut diags);
    check_card_port_collisions(program, ctx, &mut diags);
    super::meta::check_meta_info_hints(program, &mut diags);
    diags
}

/// S10 — Duplicate instance names.
fn check_duplicate_instance_names(program: &PatchProgram, diags: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for stmt in &program.statements {
        if let Statement::Instance(inst) = stmt {
            let count = seen.entry(inst.name.as_str()).or_insert(0);
            *count += 1;
            if *count > 1 {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    layer: LAYER.clone(),
                    message: format!(
                        "Duplicate instance name '{}' — instance names must be unique",
                        inst.name
                    ),
                    span: Some(inst.span.clone()),
                    source: None,
                    target: None,
                    fix: Some("Rename one of the duplicate instances".to_string()),
                });
            }
        }
    }
}

/// S11 — Duplicate signal names.
fn check_duplicate_signal_names(program: &PatchProgram, diags: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for stmt in &program.statements {
        if let Statement::Signal(sig) = stmt {
            let count = seen.entry(sig.name.as_str()).or_insert(0);
            *count += 1;
            if *count > 1 {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    layer: LAYER.clone(),
                    message: format!("Duplicate signal name '{}'", sig.name),
                    span: Some(sig.span.clone()),
                    source: None,
                    target: None,
                    fix: Some("Rename one of the duplicate signals".to_string()),
                });
            }
        }
    }
}

/// S01 — Instance references unknown template.
fn check_instance_template_refs(
    program: &PatchProgram,
    ctx: &DRCContext<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    for stmt in &program.statements {
        if let Statement::Instance(inst) = stmt {
            if !ctx.template_map.contains_key(inst.template_name.as_str()) {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    layer: LAYER.clone(),
                    message: format!(
                        "Instance '{}' references unknown template '{}'",
                        inst.name, inst.template_name
                    ),
                    span: Some(inst.span.clone()),
                    source: None,
                    target: None,
                    fix: Some(format!(
                        "Define template '{}' or fix the template name",
                        inst.template_name
                    )),
                });
            }
        }
    }
}


/// S07 — Config block references unknown instance.
fn check_config_instance_refs(
    program: &PatchProgram,
    ctx: &DRCContext<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    for stmt in &program.statements {
        if let Statement::Config(cfg) = stmt {
            if !ctx.instance_map.contains_key(cfg.name.as_str()) {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    layer: LAYER.clone(),
                    message: format!(
                        "Config block references unknown instance '{}'",
                        cfg.name
                    ),
                    span: Some(cfg.span.clone()),
                    source: None,
                    target: None,
                    fix: Some(format!(
                        "Define instance '{}' or fix the name",
                        cfg.name
                    )),
                });
            }
        }
    }
}

/// S08, S09 — Signal origin references unknown instance or port.
fn check_signal_origin_refs(
    program: &PatchProgram,
    ctx: &DRCContext<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    for stmt in &program.statements {
        if let Statement::Signal(sig) = stmt {
            if let Some(origin) = &sig.origin {
                let instance_name = match &origin.instance {
                    Some(name) => name.as_str(),
                    None => continue,
                };

                // S08 — unknown instance
                let instance = match ctx.instance_map.get(instance_name) {
                    Some(i) => i,
                    None => {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            layer: LAYER.clone(),
                            message: format!(
                                "Signal '{}' origin references unknown instance '{}'",
                                sig.name, instance_name
                            ),
                            span: Some(sig.span.clone()),
                            source: None,
                            target: None,
                            fix: Some(format!(
                                "Define instance '{}' or fix the name",
                                instance_name
                            )),
                        });
                        continue;
                    }
                };

                // S09 — unknown port on the instance (template + card ports)
                if !ctx.template_map.contains_key(instance.template_name.as_str()) {
                    continue;
                }

                if resolve_effective_port(instance_name, &origin.port, ctx).is_none() {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        layer: LAYER.clone(),
                        message: format!(
                            "Signal '{}' origin references port '{}' which does not exist on instance '{}' (template '{}')",
                            sig.name, origin.port, instance_name, instance.template_name
                        ),
                        span: Some(sig.span.clone()),
                        source: None,
                        target: None,
                        fix: Some(format!(
                            "Check the port name on template '{}'",
                            instance.template_name
                        )),
                    });
                }
            }
        }
    }
}
