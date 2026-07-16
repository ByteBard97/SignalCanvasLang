//! Canvas → PatchLang emission.
//!
//! `emit_from_canvas_input` consumes a `CanvasEmitInput` bundle (assembled by
//! the TypeScript frontend) and produces canonical PatchLang source text
//! using the validated `PatchProgramBuilder`. This is the Rust replacement
//! for the TypeScript `emitterBuilder.ts` pipeline.

use std::collections::{HashMap, HashSet};

use crate::ast::{
    ConnectDecl, InstanceDecl, KeyValue,
    KvValue, RingDecl, RingMember, Statement, TemplateDecl,
};
use crate::builder::canvas_input::*;
use crate::builder::error::BuilderError;
use crate::builder::PatchProgramBuilder;

mod ports;
mod routes;
mod structures;
mod helpers;

use ports::*;
use routes::*;
use structures::*;
use helpers::*;

/// Emit canonical PatchLang text from a canvas-side bundle.
///
/// Phases (matches the TypeScript emitter):
///   1. Card templates
///   2. Device templates (deduplicated by model)
///   3. Instances
///   4. Connections (skips backbone)
///   5. Rings
///   6. Config blocks (channel labels)
///   7. Streams (TX + RX)
pub fn emit_from_canvas_input(input: CanvasEmitInput) -> Result<String, BuilderError> {
    let mut builder = PatchProgramBuilder::new();

    // Phase 1: card templates.
    for card in &input.manufacturer_cards {
        let tmpl = build_card_template(card);
        // Skip silently if the same card template was already added.
        if builder.get_template(&tmpl.name).is_none() {
            builder.add_template(tmpl)?;
        }
    }

    // Phase 2: device templates, deduplicated by (manufacturer, model) pair.
    // Map (manufacturer, model) -> chosen template name (handles `_2`, `_3`, ... collisions).
    let mut model_to_template: HashMap<String, String> = HashMap::new();
    let mut used_template_names: HashSet<String> = HashSet::new();
    for card in &input.manufacturer_cards {
        used_template_names.insert(sanitize_id(&card.template_name));
    }

    for inst in &input.instances {
        if inst.is_ring_container {
            continue;
        }
        // Key on (manufacturer, model) so same model from different manufacturers gets distinct templates
        let dedup_key = format!(
            "{}::{}",
            inst.manufacturer.as_deref().unwrap_or(""),
            &inst.model
        );
        if model_to_template.contains_key(&dedup_key) {
            continue;
        }
        let base = sanitize_id(&inst.model);
        let mut name = base.clone();
        let mut counter = 2u32;
        while used_template_names.contains(&name) {
            name = format!("{base}_{counter}");
            counter += 1;
        }
        used_template_names.insert(name.clone());
        let tmpl = build_device_template(inst, &name);
        builder.add_template(tmpl)?;
        model_to_template.insert(dedup_key, name);
    }

    // Phase 3: instances (and slot assignments).
    let mut card_template_for_id: HashMap<String, String> = HashMap::new();
    for card in &input.manufacturer_cards {
        card_template_for_id.insert(card.template_name.clone(), sanitize_id(&card.template_name));
    }

    for inst in &input.instances {
        if inst.is_ring_container {
            continue;
        }
        let dedup_key = format!(
            "{}::{}",
            inst.manufacturer.as_deref().unwrap_or(""),
            &inst.model
        );
        let template_name = model_to_template
            .get(&dedup_key)
            .cloned()
            .ok_or_else(|| {
                BuilderError::ValidationError(format!(
                    "no template emitted for instance '{}' (model '{}')",
                    inst.name, inst.model
                ))
            })?;
        let decl = build_instance_decl(inst, &template_name, &input.manufacturer_cards, &input.instances);
        builder.add_instance(decl)?;

        // Slot assignments (best-effort — skip if card template missing).
        for installed in &inst.installed_cards {
            if let Some(card_template_name) = card_template_for_id.get(&installed.card_template_name)
            {
                let slot_name = sanitize_id(&installed.slot_label);
                // Skip if either the slot or the card template doesn't exist on the
                // builder — `set_slot` validates eagerly.
                let res = builder.set_slot(
                    &inst.name,
                    &slot_name,
                    Some(installed.slot_index),
                    card_template_name,
                );
                if let Err(BuilderError::SlotNotFound { .. }) = res {
                    // Tolerated: slot group label may differ from emitted slot name.
                    continue;
                }
                res?;
            }
        }
    }

    // Phase 4: connections.
    for conn in &input.connections {
        let mut props: Vec<KeyValue> = conn
            .properties
            .iter()
            .map(|kv| KeyValue {
                key: kv.key.clone(),
                value: KvValue::Str {
                    value: kv.value.clone(),
                },
            })
            .collect();
        if conn.is_backbone {
            props.push(kv_str("backbone", "true"));
        }

        let (from_port_name, from_idx) = parse_port_ref(&conn.from_port_id);
        let (to_port_name, to_idx) = parse_port_ref(&conn.to_port_id);

        // Generate one or more (source, target) pairs based on channel mappings.
        let pairs = build_connect_pairs(
            &conn.from_instance_name,
            &from_port_name,
            from_idx,
            &conn.to_instance_name,
            &to_port_name,
            to_idx,
            &conn.channel_mappings,
        );

        // Connect validation may fail (port not found, direction mismatch).
        // On PortNotFound, fall back to unvalidated AST construction so that
        // card-contributed ports (which aren't on the device template) still emit.
        for (source, target) in pairs {
            match builder.add_connect(source.clone(), target.clone(), props.clone()) {
                Ok(_) => {}
                Err(BuilderError::PortNotFound { .. }) | Err(BuilderError::NotFound(_)) => {
                    // Port may belong to an installed card template — emit without validation.
                    let decl = ConnectDecl {
                        source,
                        target,
                        properties: props.clone(),
                        suppressions: Vec::new(),
                        mapping: None,
                        span: builder_span(),
                    };
                    builder.program_mut().statements.push(Statement::Connect(decl));
                }
                Err(BuilderError::DirectionViolation { .. }) => continue,
                Err(e) => return Err(e),
            }
        }
    }

    // Phase 5: rings.
    for inst in &input.instances {
        if !inst.is_ring_container {
            continue;
        }
        let mut props = Vec::new();
        if let Some(proto) = &inst.ring_protocol {
            props.push(KeyValue {
                key: "protocol".into(),
                value: KvValue::Str {
                    value: proto.clone(),
                },
            });
        }
        // Infer protocol from members' first connection if not on the ring container itself.
        let effective_proto = inst.ring_protocol.as_deref().unwrap_or("");
        let ring = RingDecl {
            name: inst.name.clone(),
            properties: props,
            members: inst
                .ring_members
                .iter()
                .map(|m| RingMember {
                    instance_name: m.member_name.clone(),
                    port_name: Some(sanitize_id(&m.port_name)),
                    span: builder_span(),
                })
                .collect(),
            span: builder_span(),
        };
        builder.add_ring(ring)?;
        let _ = effective_proto; // suppress unused warning
    }

    // Phase 6: config blocks (channel labels).
    for inst in &input.instances {
        if inst.is_ring_container {
            continue;
        }
        let mut label_entries: Vec<(&String, &Vec<ChannelLabelEmitInput>)> =
            inst.channel_labels.iter().collect();
        label_entries.sort_by_key(|(k, _)| *k);
        for (iface_id, labels) in label_entries {
            if labels.is_empty() {
                continue;
            }
            // Resolve the interface so we can pick the correct directional
            // port name (channel-based io interfaces split into _In/_Out;
            // labels conventionally hang off the input side).
            // Search chassis interfaces first, then fall back to installed card interfaces.
            let iface = find_interface(
                iface_id,
                &inst.interfaces,
                &inst.installed_cards,
                &input.manufacturer_cards,
            );
            let port_name = if let Some(iface) = iface {
                directional_port_name(iface, PortSide::Input)
            } else {
                sanitize_id(iface_id)
            };

            for label in labels {
                let mut props: HashMap<String, String> = HashMap::new();
                if label.phantom {
                    props.insert("phantom".into(), "true".into());
                }
                if label.propagated {
                    props.insert("propagated".into(), "true".into());
                }
                if let Some(st) = &label.source_type {
                    if !st.is_empty() {
                        props.insert("source_type".into(), st.clone());
                    }
                }
                if let Some(cap) = &label.capsule {
                    if !cap.is_empty() {
                        props.insert("capsule".into(), cap.clone());
                    }
                }
                if let Some(band) = &label.rf_band {
                    if !band.is_empty() {
                        props.insert("rf_band".into(), band.clone());
                    }
                }
                builder.set_label(
                    &inst.name,
                    &port_name,
                    label.channel_index,
                    &label.label,
                    props,
                )?;
            }
        }
    }

    // Phase 7: streams (TX + RX).
    for inst in &input.instances {
        if inst.is_ring_container {
            continue;
        }
        emit_streams_for(&mut builder, inst, &input.manufacturer_cards, &inst.tx_streams, "tx")?;
        emit_streams_for(&mut builder, inst, &input.manufacturer_cards, &inst.rx_streams, "rx")?;
    }

    Ok(builder.format())
}

// ---------------------------------------------------------------------------
// Template + instance construction
// ---------------------------------------------------------------------------

fn build_card_template(card: &CardEmitInput) -> TemplateDecl {
    let mut meta: Vec<KeyValue> = Vec::new();
    if let Some(mfr) = &card.manufacturer {
        meta.push(kv_str("manufacturer", mfr));
    }
    meta.push(kv_str("model", &card.model));
    meta.push(kv_str("kind", "card"));
    meta.push(kv_str("fits", &card.fits));

    let ports = build_ports_for_interfaces(&card.interfaces);

    TemplateDecl {
        name: sanitize_id(&card.template_name),
        params: Vec::new(),
        version: None,
        meta,
        ports,
        bridges: Vec::new(),
        instances: Vec::new(),
        connects: Vec::new(),
        slots: Vec::new(),
        span: builder_span(),
    }
}

fn build_device_template(inst: &InstanceEmitInput, name: &str) -> TemplateDecl {
    let mut meta: Vec<KeyValue> = Vec::new();
    if let Some(mfr) = &inst.manufacturer {
        meta.push(kv_str("manufacturer", mfr));
    }
    meta.push(kv_str("model", &inst.model));
    if let Some(cat) = &inst.category {
        meta.push(kv_str("category", cat));
    }
    if let Some(kind) = &inst.kind {
        if kind != "device" {
            meta.push(kv_str("kind", kind));
        }
    }
    if let Some(chipset) = &inst.dante_chipset {
        meta.push(kv_str("dante_chipset", chipset));
    }
    if let Some(rf_subtype) = &inst.rf_subtype {
        meta.push(kv_str("rf_subtype", rf_subtype));
    }
    if let Some(min) = inst.rf_min_channels {
        meta.push(kv_num("rf_min_channels", min));
    }
    if let Some(max) = inst.rf_max_channels {
        meta.push(kv_num("rf_max_channels", max));
    }
    if let Some(band) = &inst.rf_band {
        meta.push(kv_str("rf_band", band));
    }

    let ports = build_ports_for_interfaces(&inst.interfaces);
    let slots = build_slots(&inst.card_slot_groups);
    let bridges = build_bridges(&inst.route_rules, &inst.interfaces);

    TemplateDecl {
        name: name.to_string(),
        params: Vec::new(),
        version: None,
        meta,
        ports,
        bridges,
        instances: Vec::new(),
        connects: Vec::new(),
        slots,
        span: builder_span(),
    }
}

fn build_instance_decl(inst: &InstanceEmitInput, template_name: &str, manufacturer_cards: &[CardEmitInput], all_instances: &[InstanceEmitInput]) -> InstanceDecl {
    let mut properties: Vec<KeyValue> = Vec::new();
    if let Some(loc) = &inst.location {
        properties.push(kv_str("location", loc));
    }
    if let Some(band) = &inst.rf_band {
        properties.push(kv_str("rf_band", band));
    }
    if let Some(active) = inst.rf_active_channels {
        properties.push(kv_str("rf_active_channels", &active.to_string()));
    }
    if let Some(modes) = &inst.iem_modes {
        if !modes.is_empty() {
            properties.push(kv_str("iem_modes", modes));
        }
    }

    let routes = build_instance_routes(
        &inst.instance_routes,
        &inst.interfaces,
        &inst.installed_cards,
        manufacturer_cards,
        all_instances,
    );
    let buses = build_instance_buses(&inst.internal_buses, &inst.interfaces);

    InstanceDecl {
        name: inst.name.clone(),
        template_name: template_name.to_string(),
        args: Vec::new(),
        version_constraint: None,
        properties,
        routes,
        buses,
        slot_assignments: Vec::new(),
        span: builder_span(),
    }
}

