//! Flow DRC checks — rules F01–F03.
//!
//! AES67 interoperability diagnostics:
//! - F01: Flow slot exhaustion (stream count vs chipset limit)
//! - F02: AES67 stream channel limit (max 8 per flow)
//! - F03: Multicast prefix mismatch between AES67 devices

use std::collections::HashMap;

use crate::ast::{KvValue, PatchProgram, Statement};
use crate::drc::catalog;
use crate::drc::helpers::{collect_all_connects, DRCContext};
use crate::drc::types::{DRCLayer, Diagnostic, Severity};

const LAYER: DRCLayer = DRCLayer::Flow;

/// Run all flow checks.
pub fn check(program: &PatchProgram, ctx: &DRCContext<'_>) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    check_flow_slot_exhaustion(program, ctx, &mut diags);
    check_aes67_channel_limit(program, &mut diags);
    check_multicast_prefix_mismatch(program, ctx, &mut diags);
    diags
}

/// F01 — Count stream declarations per source device and compare against chipset flow limit.
fn check_flow_slot_exhaustion(
    program: &PatchProgram,
    ctx: &DRCContext<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    // Count streams per source instance
    let mut stream_counts: HashMap<&str, (u32, crate::error::Span)> = HashMap::new();

    for stmt in &program.statements {
        if let Statement::Stream(stream) = stmt {
            if let Some(source_ref) = &stream.source {
                if let Some(instance_name) = &source_ref.instance {
                    let entry = stream_counts
                        .entry(instance_name.as_str())
                        .or_insert((0, stream.span.clone()));
                    entry.0 += 1;
                }
            }
        }
    }

    // Check each instance's stream count against its chipset limit
    for (instance_name, (count, span)) in &stream_counts {
        let instance = match ctx.instance_map.get(instance_name) {
            Some(i) => i,
            None => continue,
        };
        let template = match ctx.template_map.get(instance.template_name.as_str()) {
            Some(t) => t,
            None => continue,
        };

        // Look for dante_chipset in template meta
        let chipset = template.meta.iter().find_map(|kv| {
            if kv.key == "dante_chipset" {
                if let KvValue::Str { value } = &kv.value {
                    return Some(value.as_str());
                }
            }
            None
        });

        if let Some(chipset_name) = chipset {
            if let Some(max_flows) = catalog::dante_chipset_max_flows(chipset_name) {
                if *count > max_flows {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        layer: LAYER.clone(),
                        message: format!(
                            "Instance '{}' has {} streams but {} chipset supports at most {} flow slots.",
                            instance_name, count, chipset_name, max_flows
                        ),
                        span: Some(span.clone()),
                        source: Some(instance_name.to_string()),
                        target: None,
                        fix: Some(format!(
                            "Reduce stream count to {} or fewer for {} devices",
                            max_flows, chipset_name
                        )),
                    });
                }
            }
        }
    }
}

/// F02 — AES67 streams are limited to 8 channels per flow.
fn check_aes67_channel_limit(program: &PatchProgram, diags: &mut Vec<Diagnostic>) {
    for stmt in &program.statements {
        if let Statement::Stream(stream) = stmt {
            let is_aes67 = stream.properties.iter().any(|kv| {
                kv.key == "protocol"
                    && matches!(&kv.value, KvValue::Str { value } if value == "AES67")
            });

            if !is_aes67 {
                continue;
            }

            let channels = declared_channels(&stream.properties);

            if let Some(ch) = channels {
                if ch > 8 {
                    diags.push(Diagnostic {
                        severity: Severity::Info,
                        layer: LAYER.clone(),
                        message: format!(
                            "AES67 streams are limited to 8 channels per flow. \
                             Stream '{}' declares {} channels — hardware will auto-split \
                             into multiple flows, each consuming a flow slot.",
                            stream.name, ch
                        ),
                        span: Some(stream.span.clone()),
                        source: None,
                        target: None,
                        fix: Some(format!(
                            "Split '{}' into multiple streams of 8 channels or fewer",
                            stream.name
                        )),
                    });
                }
            }
        }
    }
}

/// Read a stream's declared `channels` property.
///
/// Accepts both `channels: 8` (hand-authored) and `channels: "8"` (canvas-emitted,
/// which writes every stream property as a string). Reading only `Num` is what left
/// F02 dead on every canvas-emitted file.
fn declared_channels(properties: &[crate::ast::KeyValue]) -> Option<u32> {
    properties.iter().find_map(|kv| {
        if kv.key != "channels" {
            return None;
        }
        match &kv.value {
            KvValue::Num { value } => Some(*value),
            KvValue::Str { value } => value.trim().parse::<u32>().ok(),
            _ => None,
        }
    })
}

/// F03 — Multicast prefix mismatch between AES67 devices.
fn check_multicast_prefix_mismatch(
    program: &PatchProgram,
    ctx: &DRCContext<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    let connects = collect_all_connects(program);

    for conn in &connects {
        let src_name = match &conn.source.instance {
            Some(n) => n.as_str(),
            None => continue,
        };
        let tgt_name = match &conn.target.instance {
            Some(n) => n.as_str(),
            None => continue,
        };

        let src_inst = match ctx.instance_map.get(src_name) {
            Some(i) => i,
            None => continue,
        };
        let tgt_inst = match ctx.instance_map.get(tgt_name) {
            Some(i) => i,
            None => continue,
        };

        // Both must have aes67_mode: true
        let src_aes67 = has_bool_property(&src_inst.properties, "aes67_mode");
        let tgt_aes67 = has_bool_property(&tgt_inst.properties, "aes67_mode");

        if !src_aes67 || !tgt_aes67 {
            continue;
        }

        let src_prefix = get_num_property(&src_inst.properties, "multicast_prefix");
        let tgt_prefix = get_num_property(&tgt_inst.properties, "multicast_prefix");

        if let (Some(sp), Some(tp)) = (src_prefix, tgt_prefix) {
            if sp != tp {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    layer: LAYER.clone(),
                    message: format!(
                        "Multicast prefix mismatch \u{2014} TX prefix {} on '{}' \
                         does not match RX prefix {} on '{}'. Audio will silently fail.",
                        sp, src_name, tp, tgt_name
                    ),
                    span: Some(conn.span.clone()),
                    source: Some(src_name.to_string()),
                    target: Some(tgt_name.to_string()),
                    fix: Some(
                        "Set both instances to the same multicast_prefix value".to_string()
                    ),
                });
            }
        }
    }
}

/// Check if an instance has a boolean-like property set to true.
/// Note: the parser treats bare `true` as a PortRef with port name "true".
fn has_bool_property(properties: &[crate::ast::KeyValue], key: &str) -> bool {
    properties.iter().any(|kv| {
        kv.key == key
            && match &kv.value {
                KvValue::Str { value } => value == "true",
                KvValue::PortRef(pr) => pr.instance.is_none() && pr.port == "true",
                _ => false,
            }
    })
}

/// Get a numeric property value from an instance.
fn get_num_property(properties: &[crate::ast::KeyValue], key: &str) -> Option<u32> {
    properties.iter().find_map(|kv| {
        if kv.key == key {
            match &kv.value {
                KvValue::Num { value } => Some(*value),
                _ => None,
            }
        } else {
            None
        }
    })
}
