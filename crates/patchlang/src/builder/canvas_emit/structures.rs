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
    ifaces: &[InterfaceEmitInput],
) -> Vec<BridgeDecl> {
    let mut bridges = Vec::new();
    for rule in rules {
        let source_port = rule.from_interface.clone();
        let target_port = rule.to_interface.clone();

        let (from_start, from_end) = if rule.from_start == 0 {
            (rule.from_channel, rule.from_channel)
        } else {
            (rule.from_start, rule.from_end)
        };
        let (to_start, to_end) = if rule.to_start == 0 {
            (rule.to_channel, rule.to_channel)
        } else {
            (rule.to_start, rule.to_end)
        };

        let src_index = bridge_index_for_span(from_start, from_end, &source_port, ifaces);
        let tgt_index = bridge_index_for_span(to_start, to_end, &target_port, ifaces);

        bridges.push(BridgeDecl {
            source: PortRef {
                instance: rule.from_instance.clone(),
                port: source_port,
                index: src_index,
            },
            target: PortRef {
                instance: rule.to_instance.clone(),
                port: target_port,
                index: tgt_index,
            },
            span: builder_span(),
        });
    }
    bridges
}

/// Emit an index for a bridge span per THE INVARIANT.
/// Omit the index only when the span is CERTAINLY the full port width
/// (start == 1 and end == channel_count of the resolved interface).
/// Otherwise always write the explicit span — never guess.
fn bridge_index_for_span(
    start: u32,
    end: u32,
    port_name: &str,
    ifaces: &[InterfaceEmitInput],
) -> Option<IndexSpec> {
    if start == 1 {
        if let Some(iface) = ifaces.iter().find(|i| {
            directional_port_name(i, PortSide::Input) == port_name
                || directional_port_name(i, PortSide::Output) == port_name
        }) {
            if end == iface.channel_count {
                return None;
            }
        }
    }
    if end == start {
        Some(IndexSpec {
            elements: vec![IndexElement::Single { value: start }],
        })
    } else {
        Some(IndexSpec {
            elements: vec![IndexElement::Range { start, end }],
        })
    }
}

// ---------------------------------------------------------------------------
// Streams
// ---------------------------------------------------------------------------

/// Build a stream source's channel selection (#37).
///
/// Deliberately unlike `bridge_index_for_span`: the selection is user intent and its
/// order is semantically significant — an AES67 receiver maps channels by position —
/// so the values are written as ordered `Single`s and are never sorted, deduplicated,
/// or coalesced into a `Range`. An empty selection means "the whole interface" and
/// emits no index at all, byte-identical to the pre-#37 emitter.
fn stream_source_index(source_channels: &[u32]) -> Option<IndexSpec> {
    if source_channels.is_empty() {
        return None;
    }
    Some(IndexSpec {
        elements: source_channels
            .iter()
            .map(|&value| IndexElement::Single { value })
            .collect(),
    })
}

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
        let index = stream_source_index(&stream.source_channels);
        // A selection is the authoritative width: derive `channels` from it so a
        // canvas-emitted file is self-consistent by construction and can never be
        // born tripping the channels-vs-selection DRC rule. With no selection the
        // frontend's count is emitted verbatim, exactly as before (#37).
        let channels = if stream.source_channels.is_empty() {
            stream.channel_count
        } else {
            stream.source_channels.len() as u32
        };
        let mut props = vec![
            kv_str("channels", &channels.to_string()),
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
                index,
            }),
            span: builder_span(),
        };
        add_stream_under_a_free_name(builder, decl)?;
    }
    Ok(())
}

/// Add a stream, suffixing its name until it does not collide.
///
/// Two interfaces legitimately share a user-facing label — one label with two channel
/// selections is a natural way to split a flow across ports (#37) — but a stream's name
/// is an identifier and `add_stream` rejects duplicates. This previously swallowed that
/// rejection with `continue`, so the second stream vanished from the emitted file with no
/// diagnostic at all: silent data loss, the same shape as #38.
///
/// Renaming is the lesser evil, on the same reasoning as D024: the suffix is user-visible
/// and self-perpetuating once written, which is a real cost, but losing the stream
/// outright is worse and invisible. `_2` is appended (then `_3`, …) rather than a hash, so
/// the result stays predictable and hand-editable.
fn add_stream_under_a_free_name(
    builder: &mut PatchProgramBuilder,
    decl: StreamDecl,
) -> Result<(), BuilderError> {
    let base = decl.name.clone();
    let mut candidate = decl;
    let mut suffix = 2u32;
    loop {
        match builder.add_stream(candidate.clone()) {
            Ok(()) => return Ok(()),
            Err(BuilderError::DuplicateName(_)) => {
                candidate.name = format!("{base}_{suffix}");
                suffix += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

