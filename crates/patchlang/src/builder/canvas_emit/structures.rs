use crate::ast::{
    BridgeDecl, IndexElement, IndexSpec, PortRef, RangeSpec,
    SlotDef, StreamDecl,
};
use crate::builder::canvas_input::*;
use crate::builder::error::BuilderError;
use crate::builder::PatchProgramBuilder;
use super::ports::*;
use super::helpers::*;

// ---------------------------------------------------------------------------
// Slots + bridges
// ---------------------------------------------------------------------------

pub(super) fn build_slots(groups: &[CardSlotGroupEmitInput]) -> Vec<SlotDef> {
    groups
        .iter()
        .map(|g| {
            let name = sanitize_id(&g.label);
            let range = if g.slot_count > 1 {
                Some(RangeSpec {
                    start: 1,
                    end: g.slot_count,
                })
            } else {
                None
            };
            let mut props = Vec::new();
            if g.direction != "any" && !g.direction.is_empty() {
                props.push(kv_str("direction", &g.direction));
            }
            if g.channel_count > 0 {
                props.push(kv_num("channels", g.channel_count));
            }
            SlotDef {
                name: name.clone(),
                range,
                slot_type: sanitize_id(&g.slot_format),
                properties: props,
                span: builder_span(),
            }
        })
        .collect()
}

pub(super) fn build_bridges(
    rules: &[RouteRuleEmitInput],
    _ifaces: &[InterfaceEmitInput],
) -> Vec<BridgeDecl> {
    let mut bridges = Vec::new();
    for rule in rules {
        // The TypeScript assembler pre-resolves from_interface / to_interface
        // to their directional port names (e.g. "Mic_In", "Dante_Out").
        // Use them directly — no interface lookup or directional resolution here.
        let source_port = rule.from_interface.clone();
        let target_port = rule.to_interface.clone();

        // When both source and target start at channel 1, the TypeScript
        // emitter omits the index entirely (full-width rangeless bridge).
        let src_index = if rule.from_channel == 1 {
            None
        } else {
            Some(IndexSpec {
                elements: vec![IndexElement::Single {
                    value: rule.from_channel,
                }],
            })
        };
        let tgt_index = if rule.to_channel == 1 {
            None
        } else {
            Some(IndexSpec {
                elements: vec![IndexElement::Single {
                    value: rule.to_channel,
                }],
            })
        };

        bridges.push(BridgeDecl {
            source: PortRef {
                instance: None,
                port: source_port,
                index: src_index,
            },
            target: PortRef {
                instance: None,
                port: target_port,
                index: tgt_index,
            },
            span: builder_span(),
        });
    }
    bridges
}

// ---------------------------------------------------------------------------
// Streams
// ---------------------------------------------------------------------------

pub(super) fn emit_streams_for(
    builder: &mut PatchProgramBuilder,
    inst: &InstanceEmitInput,
    manufacturer_cards: &[CardEmitInput],
    streams: &[StreamEmitInput],
    direction: &str,
) -> Result<(), BuilderError> {
    for stream in streams {
        // Search chassis interfaces first, then fall back to installed card interfaces.
        // Card ports flat-merge into the instance namespace (spec §card-slot).
        let iface = find_interface(
            &stream.interface_id,
            &inst.interfaces,
            &inst.installed_cards,
            manufacturer_cards,
        );
        let Some(iface) = iface else {
            // Interface not resolved — skip this stream rather than emitting a broken decl.
            // Legitimate when the frontend sends a compound card-slot ID that pre-dates
            // the rfind("__") fix; should not occur after the fix ships.
            continue;
        };
        let side = if direction == "rx" { PortSide::Input } else { PortSide::Output };
        let port_name = directional_port_name(iface, side);
        let mut props = vec![
            kv_str("channels", &stream.channel_count.to_string()),
            kv_str("direction", direction),
        ];
        if !stream.protocol.is_empty() {
            props.push(kv_str("protocol", &stream.protocol));
        }
        let name = sanitize_id(&stream.label);
        let decl = StreamDecl {
            name,
            properties: props,
            source: Some(PortRef {
                instance: Some(inst.name.clone()),
                port: port_name,
                index: None,
            }),
            span: builder_span(),
        };
        // Tolerate duplicate names (different interfaces may share a label).
        match builder.add_stream(decl) {
            Ok(()) => {}
            Err(BuilderError::DuplicateName(_)) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

