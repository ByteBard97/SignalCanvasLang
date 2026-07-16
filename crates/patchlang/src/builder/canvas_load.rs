//! PatchLang → canvas load direction.
//!
//! `load_from_patch` parses .patch source text and returns a `CanvasLoadOutput`
//! JSON bundle that TypeScript maps to PlacedDevice[] / DeviceConnection[].
//! All language logic (port extraction, template resolution, slot/route/bus
//! restoration, config labels) happens here in Rust.

use std::collections::HashMap;

use crate::ast::{
    IndexElement, KvValue, NetworkMember, PortDirection, Statement,
};
use crate::builder::canvas_output::*;
use crate::builder::error::BuilderError;
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
            let label_entry = ChannelLabelOutput {
                channel_index,
                label: cl.label.clone(),
                phantom: props.get("phantom").map(|v| v == "true").unwrap_or(false),
                propagated: props.get("propagated").map(|v| v == "true").unwrap_or(false),
                source_type: props.get("source_type").cloned(),
                capsule: props.get("capsule").cloned(),
                rf_band: props.get("rf_band").cloned(),
            };
            let channel_vec = inst_labels.entry(port_name).or_default();
            // Extend vec to fit the channel_index (1-based → 0-based)
            let idx = (channel_index as usize).saturating_sub(1);
            if channel_vec.len() <= idx {
                channel_vec.resize_with(idx + 1, || ChannelLabelOutput {
                    channel_index: 0,
                    label: String::new(),
                    phantom: false,
                    propagated: false,
                    source_type: None,
                    capsule: None,
                    rf_band: None,
                });
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
        let route_rules: Vec<RouteRuleOutput> = tmpl.bridges.iter().map(|b| {
            let from_channel = extract_single_index(&b.source.index).unwrap_or(1);
            let to_channel = extract_single_index(&b.target.index).unwrap_or(1);
            RouteRuleOutput {
                from_port: b.source.port.clone(),
                from_channel,
                from_instance: b.source.instance.clone(),
                to_port: b.target.port.clone(),
                to_channel,
                to_instance: b.target.instance.clone(),
            }
        }).collect();

        // Instance routes
        let instance_routes: Vec<RouteRuleOutput> = inst.routes.iter().map(|r| {
            let from_channel = extract_single_index(&r.source.index).unwrap_or(1);
            let to_channel = extract_single_index(&r.target.index).unwrap_or(1);
            RouteRuleOutput {
                from_port: r.source.port.clone(),
                from_channel,
                from_instance: r.source.instance.clone(),
                to_port: r.target.port.clone(),
                to_channel,
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
        let internal_buses: Vec<BusOutput> = inst.buses.iter().map(|bus| {
            let display_name = bus.label.clone().filter(|n| !n.is_empty());

            // Input port: blank out if the port name is a garbage sentinel.
            let first_input = bus.inputs.iter()
                .find(|p| is_valid_port(p));
            let input_port = first_input.map(|p| p.port.clone()).unwrap_or_default();
            let input_channels: Vec<u32> = bus.inputs.iter()
                .filter(|p| is_valid_port(p))
                .map(|p| extract_single_index(&p.index).unwrap_or(1))
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

                let output_port = real_dests.first()
                    .map(|p| p.port.clone())
                    .unwrap_or_default();
                let output_channels: Vec<u32> = real_dests.iter()
                    .map(|p| extract_single_index(&p.index).unwrap_or(1))
                    .collect();
                Some(BusNamedOutput {
                    name: out.label.clone(),
                    output_instance: real_dests.first().and_then(|p| p.instance.clone()),
                    output_port,
                    output_channels,
                })
            }).collect();

            BusOutput {
                name: bus.name.clone(),
                display_name,
                input_port,
                input_channels,
                named_outputs,
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
    kvs.iter().filter_map(|kv| {
        if let KvValue::Str { value } = &kv.value {
            Some((kv.key.clone(), value.clone()))
        } else if let KvValue::Num { value } = &kv.value {
            Some((kv.key.clone(), value.to_string()))
        } else {
            None
        }
    }).collect()
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

fn extract_single_index(index: &Option<crate::ast::IndexSpec>) -> Option<u32> {
    index.as_ref().and_then(|spec| {
        spec.elements.first().and_then(|el| match el {
            IndexElement::Single { value } => Some(*value),
            IndexElement::Range { start, .. } => Some(*start),
            IndexElement::Auto => None,
        })
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
