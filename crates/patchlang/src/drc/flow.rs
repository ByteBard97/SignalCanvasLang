//! Flow DRC checks — rules F01–F06.
//!
//! AES67 interoperability diagnostics:
//! - F01: Flow slot exhaustion (stream count vs chipset limit)
//! - F02: AES67 stream channel limit (max 8 per flow)
//! - F03: Multicast prefix mismatch between AES67 devices
//! - F04: `channels` disagrees with the source channel selection length
//! - F05: The same source channel appears at more than one position in a flow
//! - F06: A stream declares no `direction` and is being treated as transmit

use std::collections::{HashMap, HashSet};

use crate::ast::{IndexElement, KvValue, PatchProgram, Statement, StreamDecl};
use crate::drc::catalog;
use crate::drc::helpers::{collect_all_connects, expand_index_spec, DRCContext};
use crate::drc::types::{DRCLayer, Diagnostic, Severity};

const LAYER: DRCLayer = DRCLayer::Flow;

/// Run all flow checks.
pub fn check(program: &PatchProgram, ctx: &DRCContext<'_>) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    check_flow_slot_exhaustion(program, ctx, &mut diags);
    check_aes67_channel_limit(program, &mut diags);
    check_multicast_prefix_mismatch(program, ctx, &mut diags);
    check_channel_count_mismatch(program, &mut diags);
    check_duplicate_source_channels(program, &mut diags);
    check_auto_in_stream_source(program, &mut diags);
    check_missing_stream_direction(program, &mut diags);
    diags
}

/// Iterate the program's stream declarations.
fn streams(program: &PatchProgram) -> impl Iterator<Item = &StreamDecl> {
    program.statements.iter().filter_map(|stmt| match stmt {
        Statement::Stream(stream) => Some(stream),
        _ => None,
    })
}

/// The ordered source channel selection of a stream, plus whether `[auto]`
/// appeared in it. The selection is empty when the stream has no index.
///
/// Expansion is delegated to `helpers::expand_index_spec` — this module must not grow
/// its own copy of that loop. The only thing added here is `has_auto`: the shared
/// expander drops `Auto` silently, and the auto diagnostic exists precisely because
/// that drop is otherwise invisible.
fn source_selection(stream: &StreamDecl) -> (Vec<u32>, bool) {
    let spec = match stream.source.as_ref().and_then(|source| source.index.as_ref()) {
        Some(spec) => spec,
        None => return (Vec::new(), false),
    };

    let has_auto = spec
        .elements
        .iter()
        .any(|element| matches!(element, IndexElement::Auto));
    (expand_index_spec(spec), has_auto)
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

            // An explicit selection is the authority on how wide the flow is;
            // `channels` is only the fallback. F04 reports any disagreement.
            let (selection, _) = source_selection(stream);
            let channels = if selection.is_empty() {
                declared_channels(&stream.properties)
            } else {
                Some(selection.len() as u32)
            };

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

/// F04 — The declared `channels` count disagrees with the source selection length.
///
/// The selection is kept as the user wrote it rather than silently recomputing
/// `channels`, so the disagreement is reported instead. Streams without a
/// selection have nothing to disagree with and are skipped.
fn check_channel_count_mismatch(program: &PatchProgram, diags: &mut Vec<Diagnostic>) {
    for stream in streams(program) {
        let (selection, _) = source_selection(stream);
        if selection.is_empty() {
            continue;
        }
        // Read as Num *or* Str from the first line — reading only Num is what
        // left F02 dead for its entire life.
        let declared = match declared_channels(&stream.properties) {
            Some(d) => d,
            None => continue,
        };
        let selected = selection.len() as u32;
        if declared == selected {
            continue;
        }
        diags.push(Diagnostic {
            severity: Severity::Warning,
            layer: LAYER.clone(),
            message: format!(
                "Stream '{}' declares {} channels but its source selects {} \
                 ({}). The two must agree.",
                stream.name,
                declared,
                selected,
                selection
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            span: Some(stream.span.clone()),
            source: None,
            target: None,
            fix: Some(format!(
                "Set channels to {} on '{}', or correct the source channel selection",
                selected, stream.name
            )),
        });
    }
}

/// F05 — The same source channel appears at more than one position in a flow.
///
/// Informational, not a warning: position is significant, so `[3, 1, 3]` is a
/// legitimate replication of one mono source onto two receiver positions. The
/// message asks whether the repeat is intended; it does not assert a fault.
fn check_duplicate_source_channels(program: &PatchProgram, diags: &mut Vec<Diagnostic>) {
    for stream in streams(program) {
        let (selection, _) = source_selection(stream);
        let mut seen: HashSet<u32> = HashSet::new();
        let mut reported: HashSet<u32> = HashSet::new();
        for channel in &selection {
            if seen.insert(*channel) || !reported.insert(*channel) {
                continue;
            }
            diags.push(Diagnostic {
                severity: Severity::Info,
                layer: LAYER.clone(),
                message: format!(
                    "Stream '{}' selects source channel {} at more than one position. \
                     Position is significant, so this may be deliberate replication \
                     of one source onto several receiver channels — is the repeat intended?",
                    stream.name, channel
                ),
                span: Some(stream.span.clone()),
                source: None,
                target: None,
                fix: Some(format!(
                    "If the repeat was not intended, remove the duplicate {} \
                     from the source selection on '{}'",
                    channel, stream.name
                )),
            });
        }
    }
}

/// `[auto]` in a stream source has no meaning and is dropped from the selection.
///
/// Reported because `[auto]` and an absent index both flatten to an empty
/// selection, so without this the drop would be completely silent.
fn check_auto_in_stream_source(program: &PatchProgram, diags: &mut Vec<Diagnostic>) {
    for stream in streams(program) {
        let (_, has_auto) = source_selection(stream);
        if !has_auto {
            continue;
        }

        diags.push(Diagnostic {
            severity: Severity::Info,
            layer: LAYER.clone(),
            message: format!(
                "Stream '{}' uses 'auto' in its source channel selection. \
                 'auto' has no meaning for a stream source and is dropped, \
                 so the selection it contributes is empty.",
                stream.name
            ),
            span: Some(stream.span.clone()),
            source: None,
            target: None,
            fix: Some(format!(
                "Replace 'auto' with explicit channel indices on '{}', \
                 or drop the index entirely",
                stream.name
            )),
        });
    }
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

/// F06 — A stream declares no `direction`, so it is treated as transmit.
///
/// Info, not a warning: this is well-defined, not a fault. A stream is a *transmit*
/// construct — in both Dante and AES67 a flow is created and advertised by the talker,
/// and a receiver only ever subscribes to someone else's flow — so an undirected
/// `stream { source: ... }` is a TX by definition (#38).
///
/// The diagnostic exists because that default was previously a silent DROP: the loader
/// matched `"tx"`/`"rx"` exactly, so an undirected stream fell in neither bucket and
/// disappeared, taking half the streams in the production MTG.patch with it. The default
/// fixes the loss; this makes the assumption visible so a genuinely malformed file is
/// loud rather than quietly reinterpreted.
fn check_missing_stream_direction(program: &PatchProgram, diags: &mut Vec<Diagnostic>) {
    for stmt in &program.statements {
        let Statement::Stream(stream) = stmt else {
            continue;
        };
        let declared = stream.properties.iter().find(|kv| kv.key == "direction").and_then(|kv| {
            if let KvValue::Str { value } = &kv.value {
                Some(value.trim())
            } else {
                None
            }
        });
        if declared.is_some_and(|d| !d.is_empty()) {
            continue;
        }
        diags.push(Diagnostic {
            severity: Severity::Info,
            layer: LAYER.clone(),
            message: format!(
                "Stream '{}' declares no direction; treating as tx (a stream is a transmit construct).",
                stream.name
            ),
            span: Some(stream.span.clone()),
            source: None,
            target: None,
            fix: Some(format!(
                "Add direction: \"tx\" to '{}' to state it explicitly, or direction: \"rx\" if it documents a flow this device receives",
                stream.name
            )),
        });
    }
}
