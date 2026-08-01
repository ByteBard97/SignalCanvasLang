//! PatchLang → canvas load direction.
//!
//! `load_from_patch` parses .patch source text and returns a `CanvasLoadOutput`
//! JSON bundle that TypeScript maps to PlacedDevice[] / DeviceConnection[].
//! All language logic (port extraction, template resolution, slot/route/bus
//! restoration, config labels) happens here in Rust.

use std::collections::HashMap;

use crate::ast::{
    IndexElement, KvValue, NetworkMember, PortDirection, PortRef, Statement,
};
use crate::builder::canvas_output::*;
use crate::builder::error::BuilderError;
use crate::builder::insert_endpoints::{parse_insert_list, InsertEndpoint};
use crate::parser::parse;

/// Parse PatchLang source text and return a canvas-ready bundle.
///
/// The `_layout_json` parameter is reserved for future sidecar integration;
/// position data stays in TypeScript for now.
pub fn load_from_patch(patch_source: &str, _layout_json: &str) -> Result<CanvasLoadOutput, BuilderError> {
    let result = parse(patch_source);
    if !result.errors.is_empty() {
        let msg = result.errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ");
        return Err(BuilderError::ValidationError(format!("parse error(s): {msg}")));
    }
    let program = result.program;

    // Separate card templates from device templates.
    // Use ordered Vecs + HashMaps to preserve parse order for deterministic output.
    let mut device_template_order: Vec<String> = Vec::new();
    let mut device_templates: HashMap<String, crate::ast::TemplateDecl> = HashMap::new();
    let mut card_template_order: Vec<String> = Vec::new();
    let mut card_templates_map: HashMap<String, crate::ast::TemplateDecl> = HashMap::new();
    let mut rings_out: Vec<RingLoadOutput> = Vec::new();
    let mut networks_out: Vec<NetworkLoadOutput> = Vec::new();
    let mut connections_raw: Vec<crate::ast::ConnectDecl> = Vec::new();
    let mut configs: Vec<crate::ast::ConfigDecl> = Vec::new();
    let mut streams_raw: Vec<crate::ast::StreamDecl> = Vec::new();
    let mut instances_raw: Vec<crate::ast::InstanceDecl> = Vec::new();

    for stmt in program.statements {
        match stmt {
            Statement::Template(t) => {
                let is_card = t.meta.iter().any(|kv| {
                    kv.key == "kind" && matches!(&kv.value, KvValue::Str { value } if value == "card")
                });
                if is_card {
                    if !card_templates_map.contains_key(&t.name) {
                        card_template_order.push(t.name.clone());
                    }
                    card_templates_map.insert(t.name.clone(), t);
                } else {
                    if !device_templates.contains_key(&t.name) {
                        device_template_order.push(t.name.clone());
                    }
                    device_templates.insert(t.name.clone(), t);
                }
            }
            Statement::Instance(i) => instances_raw.push(i),
            Statement::Connect(c) => connections_raw.push(c),
            Statement::Config(c) => configs.push(c),
            Statement::Stream(s) => streams_raw.push(s),
            Statement::Ring(r) => {
                rings_out.push(RingLoadOutput {
                    name: r.name.clone(),
                    protocol: r.properties.iter().find(|kv| kv.key == "protocol").and_then(|kv| {
                        if let KvValue::Str { value } = &kv.value { Some(value.clone()) } else { None }
                    }),
                    members: r.members.iter().map(|m| RingMemberOutput {
                        instance_name: m.instance_name.clone(),
                        port_name: m.port_name.clone(),
                    }).collect(),
                });
            }
            Statement::Network(n) => {
                networks_out.push(NetworkLoadOutput {
                    name: n.name.clone(),
                    protocol: n.properties.iter().find(|kv| kv.key == "protocol").and_then(|kv| {
                        if let KvValue::Str { value } = &kv.value { Some(value.clone()) } else { None }
                    }),
                    label: n.properties.iter().find(|kv| kv.key == "label").and_then(|kv| {
                        if let KvValue::Str { value } = &kv.value { Some(value.clone()) } else { None }
                    }),
                    members: n.members.iter().map(|m| match m {
                        NetworkMember::DeviceLevel { instance, .. } => NetworkMemberLoadOutput {
                            member_type: "device_level".to_string(),
                            instance_name: instance.clone(),
                            port_group: None,
                            slot_index: None,
                        },
                        NetworkMember::PortGroup { instance, port_group, .. } => NetworkMemberLoadOutput {
                            member_type: "port_group".to_string(),
                            instance_name: instance.clone(),
                            port_group: Some(port_group.clone()),
                            slot_index: None,
                        },
                        NetworkMember::SlotRef { instance, index, .. } => NetworkMemberLoadOutput {
                            member_type: "slot_ref".to_string(),
                            instance_name: instance.clone(),
                            port_group: None,
                            slot_index: Some(*index),
                        },
                    }).collect(),
                });
            }
            _ => {}
        }
    }

    // Build card template outputs in parse order
    let card_templates: Vec<CardTemplateOutput> = card_template_order.iter().filter_map(|name| {
        let tmpl = card_templates_map.get(name)?;
        let manufacturer = meta_str(tmpl, "manufacturer");
        let model = meta_str(tmpl, "model");
        let fits = meta_str(tmpl, "fits");
        Some(CardTemplateOutput {
            template_name: name.clone(),
            manufacturer,
            model,
            fits,
            ports: extract_ports(tmpl),
        })
    }).collect();

    // Build config label map: instance_name → port_name → Vec<ChannelLabelOutput>
    let mut label_map: HashMap<String, HashMap<String, Vec<ChannelLabelOutput>>> = HashMap::new();
    for config in &configs {
        let inst_labels = label_map.entry(config.name.clone()).or_default();
        for cl in &config.labels {
            let port_name = cl.port.port.clone();
            let channel_index = extract_single_index(&cl.port.index).unwrap_or(1);
            let props = kv_map(&cl.properties);
            // Insert legs (#31). All-or-nothing: a clean parse takes ownership of the
            // key and removes it from the leftovers bag; a malformed one leaves the
            // typed field empty and the raw string in the bag, so it re-emits intact.
            let insert_send = parse_insert_list_prop(&props, "insert_send");
            let insert_return = parse_insert_list_prop(&props, "insert_return");
            let label_entry = ChannelLabelOutput {
                channel_index,
                label: cl.label.clone(),
                phantom: props.get("phantom").map(|v| v == "true").unwrap_or(false),
                propagated: props.get("propagated").map(|v| v == "true").unwrap_or(false),
                source_type: props.get("source_type").cloned(),
                capsule: props.get("capsule").cloned(),
                rf_band: props.get("rf_band").cloned(),
                properties: leftover_label_props(&props, &insert_send, &insert_return),
                insert_send,
                insert_return,
            };
            let channel_vec = inst_labels.entry(port_name).or_default();
            // Extend vec to fit the channel_index (1-based → 0-based)
            let idx = (channel_index as usize).saturating_sub(1);
            if channel_vec.len() <= idx {
                channel_vec.resize_with(idx + 1, ChannelLabelOutput::default);
            }
            channel_vec[idx] = label_entry;
        }
    }

    // Build stream lookup by port name: instance_name → Vec<StreamOutput>
    let mut stream_map: HashMap<String, Vec<StreamOutput>> = HashMap::new();
    for stream in &streams_raw {
        let source = stream.source.as_ref().ok_or_else(|| {
            BuilderError::ValidationError(format!(
                "stream '{}' has no source — every stream must declare 'source: Instance.Port'",
                stream.name
            ))
        })?;
        let inst_name = source.instance.as_ref().ok_or_else(|| {
            BuilderError::ValidationError(format!(
                "stream '{}' source has no instance qualifier — use 'source: InstanceName.PortName'",
                stream.name
            ))
        })?;
        let protocol = stream.properties.iter().find(|kv| kv.key == "protocol")
            .and_then(|kv| if let KvValue::Str { value } = &kv.value { Some(value.clone()) } else { None })
            .unwrap_or_default();
        let channel_count = stream.properties.iter().find(|kv| kv.key == "channels")
            .and_then(|kv| match &kv.value {
                KvValue::Num { value } => Some(*value),
                KvValue::Str { value } => value.parse().ok(),
                _ => None,
            }).unwrap_or(0);
        let direction = stream.properties.iter().find(|kv| kv.key == "direction")
            .and_then(|kv| if let KvValue::Str { value } = &kv.value { Some(value.clone()) } else { None })
            .unwrap_or_default();
        stream_map.entry(inst_name.clone()).or_default().push(StreamOutput {
            label: stream.name.clone(),
            protocol,
            channel_count,
            port_name: source.port.clone(),
            direction,
        });
    }

    // Build instance outputs (in parse order)
    let mut instances: Vec<InstanceLoadOutput> = Vec::new();
    for inst in &instances_raw {
        let tmpl = device_templates.get(&inst.template_name)
            .or_else(|| card_templates_map.get(&inst.template_name))
            .ok_or_else(|| BuilderError::ValidationError(format!(
                "instance '{}' references unknown template '{}'",
                inst.name, inst.template_name
            )))?;

        let props = kv_map(&inst.properties);
        let manufacturer = meta_str(tmpl, "manufacturer");
        let model = meta_str(tmpl, "model");
        let category = meta_str(tmpl, "category");
        let kind = meta_str(tmpl, "kind");
        let dante_chipset = meta_str(tmpl, "dante_chipset");
        let rf_subtype = meta_str(tmpl, "rf_subtype");
        let rf_min_channels = meta_num(tmpl, "rf_min_channels");
        let rf_max_channels = meta_num(tmpl, "rf_max_channels");

        let is_ring_container = kind.as_deref() == Some("ring")
            || kind.as_deref() == Some("optocore-ring");

        let ports = extract_ports(tmpl);
        let card_slot_groups = extract_slot_groups(tmpl);

        // Slot assignments from instance body
        let installed_cards: Vec<InstalledCardOutput> = inst.slot_assignments.iter().map(|sa| {
            let slot_index = sa.index.unwrap_or(1);
            InstalledCardOutput {
                slot_label: sa.slot_name.clone(),
                slot_index,
                card_template_name: sa.card_name.clone(),
            }
        }).collect();

        // Template bridges → route_rules on UserDevice (hardwired internal paths)
        let port_spans: HashMap<&str, (u32, u32)> = tmpl.ports.iter().map(|p| {
            let span = p.range.as_ref().map(|r| (r.start, r.end)).unwrap_or((1, 1));
            (p.name.as_str(), span)
        }).collect();

        let route_rules: Vec<RouteRuleOutput> = tmpl.bridges.iter().filter_map(|b| {
            let (from_start, from_end) = bridge_endpoint_span(
                &b.source, &port_spans, &tmpl.name, "source"
            )?;
            let (to_start, to_end) = bridge_endpoint_span(
                &b.target, &port_spans, &tmpl.name, "target"
            )?;
            Some(RouteRuleOutput {
                from_port: b.source.port.clone(),
                from_channel: from_start,
                from_start,
                from_end,
                from_instance: b.source.instance.clone(),
                to_port: b.target.port.clone(),
                to_channel: to_start,
                to_start,
                to_end,
                to_instance: b.target.instance.clone(),
            })
        }).collect();

        // Instance routes
        let instance_routes: Vec<RouteRuleOutput> = inst.routes.iter().map(|r| {
            let from_channel = extract_single_index(&r.source.index).unwrap_or(1);
            let to_channel = extract_single_index(&r.target.index).unwrap_or(1);
            RouteRuleOutput {
                from_port: r.source.port.clone(),
                from_channel,
                from_start: from_channel,
                from_end: from_channel,
                from_instance: r.source.instance.clone(),
                to_port: r.target.port.clone(),
                to_channel,
                to_start: to_channel,
                to_end: to_channel,
                to_instance: r.target.instance.clone(),
            }
        }).collect();

        // Collect declared port names for this template. Slot-qualified port
        // names (e.g. "AES67_Out__Client_1") are also valid — TypeScript writes
        // them when a bus targets a card-slot port. We recognise them by the
        // "__" separator convention rather than building a full card-port set.
        let valid_port_names: std::collections::HashSet<&str> =
            tmpl.ports.iter().map(|p| p.name.as_str()).collect();

        // A PortRef is valid if it's a cross-device reference (instance is Some),
        // or if the port name matches a declared template port, or if it's a
        // slot-qualified port name (contains "__") written by TypeScript.
        let is_valid_port = |p: &crate::ast::PortRef| -> bool {
            p.instance.is_some()
                || valid_port_names.contains(p.port.as_str())
                || p.port.contains("__")
        };

        // Internal buses
        let internal_buses: Vec<BusLoadOutput> = inst.buses.iter().map(|bus| {
            let display_name = bus.label.clone().filter(|n| !n.is_empty());

            // Input: filter valid, then group by (instance, port) preserving first-seen order.
            let valid_inputs: Vec<_> = bus.inputs.iter()
                .filter(|p| is_valid_port(p))
                .collect();
            let input_port = valid_inputs.first()
                .map(|p| p.port.clone())
                .unwrap_or_default();
            let input_channels: Vec<u32> = valid_inputs.iter()
                .flat_map(|p| expand_index(&p.index))
                .collect();
            // Union channels across every input sharing an (instance, port) key, in
            // first-seen order. A bus is normally fed by MANY single-channel inputs on
            // the same port (`input: Mix_Bus[1]` … `input: Mix_Bus[24]`), so keying on
            // first-occurrence only — and discarding the rest — would drop 23 of 24
            // channels. Mirrors the named-output grouping below, which unions the same way.
            let mut group_map: std::collections::HashMap<(Option<String>, String), Vec<u32>> =
                std::collections::HashMap::new();
            let mut group_order: Vec<(Option<String>, String)> = Vec::new();
            for p in &valid_inputs {
                let key = (p.instance.clone(), p.port.clone());
                if !group_map.contains_key(&key) {
                    group_order.push(key.clone());
                }
                let channels = expand_index(&p.index);
                let entry = group_map.entry(key).or_default();
                if channels.is_empty() {
                    entry.push(1);
                } else {
                    entry.extend(channels);
                }
            }
            let input_groups: Vec<BusInputGroup> = group_order
                .into_iter()
                .filter_map(|key| {
                    group_map.remove(&key).map(|channels| BusInputGroup {
                        input_instance: key.0,
                        input_port: key.1,
                        input_channels: channels,
                    })
                })
                .collect();

            let named_outputs: Vec<BusNamedOutput> = bus.outputs.iter().filter_map(|out| {
                // Keep only destinations with a valid port name. Old saves may
                // contain "Unknown" or "Device" as garbage sentinels.
                let real_dests: Vec<_> = out.destinations.iter()
                    .filter(|p| is_valid_port(p))
                    .collect();

                // If the output had destinations in the file but all were garbage,
                // drop the entry entirely (phantom from old TS code). Legitimately
                // unrouted outputs have no destinations at all and are preserved.
                if !out.destinations.is_empty() && real_dests.is_empty() {
                    return None;
                }

                // Emit one BusNamedOutput per distinct (instance, port), all sharing the label.
                // Multiple indices on the same port (e.g. Port[1], Port[2]) are merged into
                // one output entry with a unioned channel set, preserving first-seen order.
                let mut output_map: std::collections::HashMap<(Option<String>, String), Vec<u32>> = std::collections::HashMap::new();
                let mut output_order: Vec<(Option<String>, String)> = Vec::new();
                for p in &real_dests {
                    let key = (p.instance.clone(), p.port.clone());
                    if !output_map.contains_key(&key) {
                        output_order.push(key.clone());
                    }
                    let channels = expand_index(&p.index);
                    let entry = output_map.entry(key).or_default();
                    if channels.is_empty() {
                        entry.push(1);
                    } else {
                        entry.extend(channels);
                    }
                }
                let mut outputs: Vec<BusNamedOutput> = Vec::new();
                for key in output_order {
                    if let Some(channels) = output_map.remove(&key) {
                        outputs.push(BusNamedOutput {
                            name: out.label.clone(),
                            output_instance: key.0.clone(),
                            output_port: key.1.clone(),
                            output_channels: channels,
                        });
                    }
                }
                if outputs.is_empty() {
                    // Preserve legitimately unrouted outputs with a single empty entry
                    outputs.push(BusNamedOutput {
                        name: out.label.clone(),
                        output_instance: None,
                        output_port: String::new(),
                        output_channels: vec![],
                    });
                }
                Some(outputs)
            }).flatten().collect();

            // Insert legs (#31). Deliberately NOT run through the grouping/union above
            // and NOT filtered by `is_valid_port`: order is significant, a port
            // legitimately repeats across legs, and this ticket is about not losing
            // data — a sentinel-looking port is preserved and left for DRC to flag.
            let insert_send = bus_insert_endpoints(&bus.insert_send);
            let insert_return = bus_insert_endpoints(&bus.insert_return);

            BusLoadOutput {
                name: bus.name.clone(),
                display_name,
                input_port,
                input_channels,
                input_groups,
                named_outputs,
                insert_send,
                insert_return,
            }
        }).collect();

        // Streams for this instance
        let all_streams: Vec<StreamOutput> = stream_map.remove(&inst.name).unwrap_or_default();
        let tx_streams: Vec<StreamOutput> = all_streams.iter()
            .filter(|s| s.direction == "tx")
            .cloned()
            .collect();
        let rx_streams: Vec<StreamOutput> = all_streams.iter()
            .filter(|s| s.direction == "rx")
            .cloned()
            .collect();

        let channel_labels = label_map.remove(&inst.name).unwrap_or_default();

        instances.push(InstanceLoadOutput {
            name: inst.name.clone(),
            template_name: inst.template_name.clone(),
            manufacturer,
            model,
            category,
            kind,
            location: props.get("location").cloned(),
            dante_chipset,
            rf_subtype,
            rf_min_channels,
            rf_max_channels,
            rf_band: props.get("rf_band").cloned(),
            rf_active_channels: props.get("rf_active_channels")
                .and_then(|v| v.parse().ok()),
            iem_modes: props.get("iem_modes").cloned(),
            ports,
            card_slot_groups,
            installed_cards,
            channel_labels,
            route_rules,
            instance_routes,
            internal_buses,
            tx_streams,
            rx_streams,
            is_ring_container,
            ring_protocol: props.get("ring_protocol").cloned(),
        });
    }

    // Build connections
    let mut connections: Vec<ConnectionLoadOutput> = Vec::new();
    for conn in &connections_raw {
        let from_instance = conn.source.instance.clone().unwrap_or_default();
        let to_instance = conn.target.instance.clone().unwrap_or_default();
        if from_instance.is_empty() || to_instance.is_empty() {
            continue;
        }
        let conn_props = kv_map(&conn.properties);
        let is_backbone = conn_props.get("backbone").map(|v| v == "true").unwrap_or(false)
            || conn_props.get("kind").map(|v| v == "console_link").unwrap_or(false);
        let from_slot = conn_props.get("from_slot").cloned();
        let to_slot = conn_props.get("to_slot").cloned();

        let from_port = format_port_ref(&conn.source.port, &conn.source.index);
        let to_port = format_port_ref(&conn.target.port, &conn.target.index);

        // Build channel mappings from index specs when no explicit mapping text
        let channel_mappings = build_channel_mappings_from_indices(
            &conn.source.index,
            &conn.target.index,
            &conn.mapping,
        );

        connections.push(ConnectionLoadOutput {
            properties: conn_props.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            from_instance,
            to_instance,
            from_port,
            to_port,
            is_backbone,
            channel_mappings,
            from_slot,
            to_slot,
            mapping_text: conn.mapping.clone(),
        });
    }

    Ok(CanvasLoadOutput {
        instances,
        connections,
        card_templates,
        rings: rings_out,
        networks: networks_out,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn meta_str(tmpl: &crate::ast::TemplateDecl, key: &str) -> Option<String> {
    tmpl.meta.iter().find(|kv| kv.key == key).and_then(|kv| {
        if let KvValue::Str { value } = &kv.value { Some(value.clone()) } else { None }
    })
}

fn meta_num(tmpl: &crate::ast::TemplateDecl, key: &str) -> Option<u32> {
    tmpl.meta.iter().find(|kv| kv.key == key).and_then(|kv| {
        match &kv.value {
            KvValue::Num { value } => Some(*value),
            KvValue::Str { value } => value.parse().ok(),
            _ => None,
        }
    })
}

fn kv_map(kvs: &[crate::ast::KeyValue]) -> HashMap<String, String> {
    kvs.iter().map(|kv| {
        match &kv.value {
            KvValue::Str { value } => (kv.key.clone(), value.clone()),
            KvValue::Num { value } => (kv.key.clone(), value.to_string()),
            // Previously `None` — every port-ref-valued property was silently dropped
            // on load. `parse_key_value_full` accepts a bare identifier as a PortRef,
            // so `insert_send: Ext_Out[3]` (unquoted — right-looking, and what an LLM
            // writes by default) parsed fine and then vanished here. Stringify instead,
            // matching what `graph::kv_to_string_map` already does. Issue #31.
            KvValue::PortRef(pr) => (kv.key.clone(), port_ref_to_string(pr)),
        }
    }).collect()
}

/// Convert parsed bus insert `PortRef`s to endpoint DTOs — exactly one entry per leg.
///
/// The leg count is preserved unconditionally. `parse_bus_entry` already rejects any
/// leg that does not name exactly one channel, so a non-single index cannot reach here
/// through `load_from_patch` (parse errors abort the load). The channel-1 fallback
/// covers only ASTs built programmatically via the builder API, and mirrors what the
/// bus-input grouping does for an empty index.
///
/// Dropping a leg here would be the wrong failure mode whatever the cause: it silently
/// changes how many channels the insert claims — mono where the file said stereo — and
/// silent loss at this boundary is precisely the bug this feature exists to fix.
fn bus_insert_endpoints(refs: &[PortRef]) -> Vec<InsertEndpoint> {
    refs.iter()
        .map(|pr| InsertEndpoint {
            instance: pr.instance.clone(),
            port: pr.port.clone(),
            channel: expand_index(&pr.index).first().copied().unwrap_or(1),
        })
        .collect()
}

/// Render a `PortRef` back to its source form: `Port[3]` or `Instance.Port[3]`.
fn port_ref_to_string(pr: &PortRef) -> String {
    let mut out = String::new();
    if let Some(inst) = &pr.instance {
        out.push_str(inst);
        out.push('.');
    }
    out.push_str(&pr.port);
    if let Some(index) = &pr.index {
        let channels = expand_index(&Some(index.clone()));
        if let [single] = channels.as_slice() {
            out.push_str(&format!("[{single}]"));
        }
    }
    out
}

/// Decode an insert leg list from a label property, per issue #31.
///
/// Empty when the key is absent OR when the value is malformed — in the malformed case
/// [`leftover_label_props`] keeps the raw string so nothing is lost.
fn parse_insert_list_prop(
    props: &HashMap<String, String>,
    key: &str,
) -> Vec<InsertEndpoint> {
    props
        .get(key)
        .and_then(|raw| parse_insert_list(raw))
        .unwrap_or_default()
}

/// Every label property with no dedicated field on `ChannelLabelOutput`.
///
/// An insert key is removed only when its typed field actually took ownership of the
/// value (i.e. it parsed cleanly). A malformed insert string stays, so re-emit is
/// byte-faithful rather than blanked. See `ChannelLabelOutput::properties`.
fn leftover_label_props(
    props: &HashMap<String, String>,
    insert_send: &[InsertEndpoint],
    insert_return: &[InsertEndpoint],
) -> std::collections::BTreeMap<String, String> {
    const DEDICATED: [&str; 5] =
        ["phantom", "propagated", "source_type", "capsule", "rf_band"];
    let claimed_by_typed_field = |key: &str| match key {
        // An insert key is claimed only when its typed field actually parsed. When the
        // parse failed the raw string must stay here or the value is lost on re-emit.
        "insert_send" => !insert_send.is_empty(),
        "insert_return" => !insert_return.is_empty(),
        other => DEDICATED.contains(&other),
    };
    props
        .iter()
        .filter(|(k, _)| !claimed_by_typed_field(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}


fn extract_ports(tmpl: &crate::ast::TemplateDecl) -> Vec<PortLoadOutput> {
    tmpl.ports.iter().map(|p| {
        let direction = match p.direction {
            PortDirection::In => "in",
            PortDirection::Out => "out",
            PortDirection::Io => "io",
        };
        let channel_count = p.range.as_ref().map(|r| r.end - r.start + 1).unwrap_or(1);
        let transport = p.attributes.first().cloned();
        let attributes = p.attributes.iter().skip(1).cloned().collect();
        PortLoadOutput {
            name: p.name.clone(),
            direction: direction.to_string(),
            connector: p.connector.clone(),
            channel_count,
            transport,
            attributes,
        }
    }).collect()
}

fn extract_slot_groups(tmpl: &crate::ast::TemplateDecl) -> Vec<CardSlotGroupOutput> {
    tmpl.slots.iter().map(|s| {
        let slot_count = s.range.as_ref().map(|r| r.end - r.start + 1).unwrap_or(1);
        CardSlotGroupOutput {
            label: s.name.clone(),
            slot_count,
            slot_format: s.slot_type.clone(),
            direction: String::new(),
            channel_count: 0,
        }
    }).collect()
}

/// Resolve a bridge endpoint's channel span per THE INVARIANT.
/// Returns None when the index is multi-element (non-contiguous) — caller must skip the bridge.
fn bridge_endpoint_span(
    port_ref: &crate::ast::PortRef,
    port_spans: &HashMap<&str, (u32, u32)>,
    template_name: &str,
    which: &str,            // "source" or "target", for the warning text
) -> Option<(u32, u32)> {
    match index_span(&port_ref.index) {
        Some((s, e)) => Some((s, e)),
        None => {
            // Distinguish absent index (None/Auto/empty) from multi-element
            let is_absent = port_ref.index.is_none()
                || port_ref.index.as_ref().map(|spec| spec.elements.is_empty()).unwrap_or(true)
                || port_ref.index.as_ref().map(|spec| {
                    spec.elements.len() == 1 && matches!(&spec.elements[0], IndexElement::Auto)
                }).unwrap_or(false);
            if is_absent {
                let span = port_spans.get(port_ref.port.as_str()).copied().unwrap_or_else(|| {
                    eprintln!("warning: bridge {} port '{}' not found in template '{}', falling back to (1,1)", which, port_ref.port, template_name);
                    (1, 1)
                });
                Some(span)
            } else {
                eprintln!("warning: bridge '{}' in template '{}' has a multi-element index on {}; skipping (non-contiguous)", port_ref.port, template_name, which);
                None
            }
        }
    }
}

fn extract_single_index(index: &Option<crate::ast::IndexSpec>) -> Option<u32> {
    index.as_ref().and_then(|spec| {
        spec.elements.first().and_then(|el| match el {
            IndexElement::Single { value } => Some(*value),
            IndexElement::Range { start, .. } => Some(*start),
            IndexElement::Auto => None,
        })
    })
}

/// Contiguous span of an index spec, per THE INVARIANT in docs/plans/canvas-dto-plurality.md.
///
/// `[a..b]`      → `Some((a, b))`
/// `[n]`         → `Some((n, n))`
/// absent / Auto → `None` — the caller applies the full port width.
/// multi-element (`[1,3,5]`) → `None` — no honest contiguous span exists; the caller
///   must skip and log rather than widening to `min..max`, which would invent channels.
fn index_span(index: &Option<crate::ast::IndexSpec>) -> Option<(u32, u32)> {
    index.as_ref().and_then(|spec| {
        match spec.elements.len() {
            0 => None,
            1 => match &spec.elements[0] {
                IndexElement::Single { value } => Some((*value, *value)),
                IndexElement::Range { start, end } => Some((*start, *end)),
                IndexElement::Auto => None,
            },
            _ => None,
        }
    })
}

fn format_port_ref(port: &str, index: &Option<crate::ast::IndexSpec>) -> String {
    match extract_single_index(index) {
        Some(idx) => format!("{port}[{idx}]"),
        None => port.to_string(),
    }
}

/// Build channel mappings from source/target index specs or mapping text.
fn build_channel_mappings_from_indices(
    src_index: &Option<crate::ast::IndexSpec>,
    tgt_index: &Option<crate::ast::IndexSpec>,
    mapping: &Option<String>,
) -> Vec<ChannelMappingOutput> {
    // Explicit mapping text takes precedence
    if let Some(mapping_str) = mapping {
        return parse_mapping_str(mapping_str);
    }

    // Extract all channels from index specs
    let src_channels = expand_index(src_index);
    let tgt_channels = expand_index(tgt_index);

    if src_channels.is_empty() && tgt_channels.is_empty() {
        return Vec::new(); // full-width, no explicit channel selection
    }

    let count = src_channels.len().max(tgt_channels.len());
    (0..count).filter_map(|i| {
        let from_ch = src_channels.get(i).copied().or_else(|| Some(i as u32 + 1))?;
        let to_ch = tgt_channels.get(i).copied().or_else(|| Some(i as u32 + 1))?;
        Some(ChannelMappingOutput { from_channel: from_ch, to_channel: to_ch })
    }).collect()
}

fn expand_index(index: &Option<crate::ast::IndexSpec>) -> Vec<u32> {
    let Some(spec) = index else { return Vec::new() };
    let mut channels = Vec::new();
    for el in &spec.elements {
        match el {
            IndexElement::Single { value } => channels.push(*value),
            IndexElement::Range { start, end } => {
                for ch in *start..=*end {
                    channels.push(ch);
                }
            }
            IndexElement::Auto => {}
        }
    }
    channels
}

/// Parse explicit mapping string: "1:1", "offset N", or "A->B, C->D, ..."
fn parse_mapping_str(mapping: &str) -> Vec<ChannelMappingOutput> {
    let m = mapping.trim();
    if m == "1:1" {
        return Vec::new(); // full-width sequential, no explicit mapping needed
    }
    if let Some(offset_str) = m.strip_prefix("offset ") {
        if let Ok(offset) = offset_str.trim().parse::<i32>() {
            // Caller must supply count — we return empty to signal "use offset logic"
            // TypeScript handles offset range mapping
            let _ = offset;
        }
        return Vec::new();
    }
    // Parse "A->B" pairs
    m.split(',').filter_map(|pair| {
        let pair = pair.trim();
        let (a, b) = pair.split_once("->")?;
        let from_ch: u32 = a.trim().parse().ok()?;
        let to_ch: u32 = b.trim().parse().ok()?;
        Some(ChannelMappingOutput { from_channel: from_ch, to_channel: to_ch })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_single(n: u32) -> Option<crate::ast::IndexSpec> {
        Some(crate::ast::IndexSpec {
            elements: vec![IndexElement::Single { value: n }],
        })
    }

    fn spec_range(start: u32, end: u32) -> Option<crate::ast::IndexSpec> {
        Some(crate::ast::IndexSpec {
            elements: vec![IndexElement::Range { start, end }],
        })
    }

    fn spec_auto() -> Option<crate::ast::IndexSpec> {
        Some(crate::ast::IndexSpec {
            elements: vec![IndexElement::Auto],
        })
    }

    fn spec_multi() -> Option<crate::ast::IndexSpec> {
        Some(crate::ast::IndexSpec {
            elements: vec![
                IndexElement::Single { value: 1 },
                IndexElement::Single { value: 3 },
                IndexElement::Single { value: 5 },
            ],
        })
    }

    fn spec_two_ranges() -> Option<crate::ast::IndexSpec> {
        Some(crate::ast::IndexSpec {
            elements: vec![
                IndexElement::Range { start: 1, end: 4 },
                IndexElement::Range { start: 5, end: 8 },
            ],
        })
    }

    fn spec_mixed() -> Option<crate::ast::IndexSpec> {
        Some(crate::ast::IndexSpec {
            elements: vec![
                IndexElement::Single { value: 1 },
                IndexElement::Range { start: 3, end: 5 },
            ],
        })
    }

    #[test]
    fn index_span_single() {
        assert_eq!(index_span(&spec_single(5)), Some((5, 5)));
    }

    #[test]
    fn index_span_range() {
        assert_eq!(index_span(&spec_range(10, 20)), Some((10, 20)));
    }

    #[test]
    fn index_span_absent() {
        assert_eq!(index_span(&None), None);
    }

    #[test]
    fn index_span_auto() {
        assert_eq!(index_span(&spec_auto()), None);
    }

    #[test]
    fn index_span_empty() {
        let empty = Some(crate::ast::IndexSpec { elements: vec![] });
        assert_eq!(index_span(&empty), None);
    }

    #[test]
    fn index_span_multi_element() {
        assert_eq!(index_span(&spec_multi()), None);
    }

    #[test]
    fn index_span_two_ranges() {
        assert_eq!(index_span(&spec_two_ranges()), None);
    }

    #[test]
    fn index_span_mixed_single_range() {
        assert_eq!(index_span(&spec_mixed()), None);
    }

    /// FIX 1 regression guard: a port declared with a non-1 start (e.g. [17..24])
    /// and a bridge with NO index must load as span (17, 24), not (1, 8).
    #[test]
    fn bridge_absent_index_uses_port_declared_span() {
        let patch = r#"
template T {
  ports {
    P[17..24]: in(XLR)
    Q[1..8]: out(XLR)
  }
  bridge P -> Q
}
instance I is T {}
"#;
        let loaded = load_from_patch(patch, "").expect("load must succeed");
        let inst = loaded.instances.iter().find(|i| i.name == "I").expect("I must exist");
        assert_eq!(inst.route_rules.len(), 1, "expected one bridge rule");
        let rule = &inst.route_rules[0];
        assert_eq!(rule.from_port, "P", "from_port mismatch");
        assert_eq!(rule.from_start, 17, "absent index on P[17..24] must yield start=17, not 1");
        assert_eq!(rule.from_end, 24, "absent index on P[17..24] must yield end=24, not 8");
        assert_eq!(rule.to_port, "Q", "to_port mismatch");
        assert_eq!(rule.to_start, 1, "absent index on Q[1..8] must yield start=1");
        assert_eq!(rule.to_end, 8, "absent index on Q[1..8] must yield end=8");
    }
}
