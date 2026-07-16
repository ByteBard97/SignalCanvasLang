use crate::ast::{
    IndexElement, IndexSpec, KeyValue,
    KvValue, PortRef,
};
use crate::builder::canvas_input::*;
use crate::error::Span;

pub(super) fn builder_span() -> Span {
    Span {
        start: 0,
        end: 0,
        file: None,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Search chassis interfaces first, then fall back to installed card interfaces.
/// Used by emit_streams_for and build_instance_routes.
///
/// Card-slot compound IDs are formed by the frontend as `{slotId}__{cardIfaceId}`
/// (e.g. `slot::Client::0__1__pl::AES67_108_G2::AES67_Out`). Card interface IDs
/// never contain `__` (they use `::` only), so stripping everything up to and
/// including the last `__` recovers the card-relative ID for lookup.
pub(super) fn find_interface<'a>(
    interface_id: &str,
    chassis_ifaces: &'a [InterfaceEmitInput],
    installed_cards: &'a [InstalledCardEmitInput],
    manufacturer_cards: &'a [CardEmitInput],
) -> Option<&'a InterfaceEmitInput> {
    chassis_ifaces
        .iter()
        .find(|i| i.id == interface_id)
        .or_else(|| {
            // Strip slot prefix from compound card-slot IDs (`{slotId}__{cardIfaceId}`).
            // Chassis IDs never contain `__`, so this is safe and unambiguous.
            let card_iface_id = interface_id
                .rfind("__")
                .map(|pos| &interface_id[pos + 2..])
                .unwrap_or(interface_id);

            installed_cards.iter().find_map(|installed| {
                manufacturer_cards
                    .iter()
                    .find(|c| c.template_name == installed.card_template_name)
                    .and_then(|c| c.interfaces.iter().find(|i| i.id == card_iface_id))
            })
        })
}

/// Build the list of (source, target) PortRef pairs for a connection.
///
/// When channel_mappings is empty: one pair with the raw from/to index (or no index).
/// When mappings are contiguous on the source side with a constant from→to offset
/// (this covers sequential 1:1 starting at 1, and any other run-of-N case):
/// single pair with explicit range indices (`Port[start..end]`). The previous
/// "sequential 1:1 from 1 → no index" shortcut was removed because it conflated
/// partial patches (e.g. 2 of an 8-channel port) with full-width 1:1 patches,
/// and the loader's `min(fromCount, toCount)` fallback re-inflated the partial
/// case on reload.
/// When mappings are non-contiguous OR a single entry: one pair per mapping
/// entry, each with a single-index port ref (`Port[N]`).
pub(super) fn build_connect_pairs(
    from_inst: &str,
    from_port: &str,
    from_idx: Option<u32>,
    to_inst: &str,
    to_port: &str,
    to_idx: Option<u32>,
    mappings: &[ChannelMappingEmitInput],
) -> Vec<(PortRef, PortRef)> {
    let from_port_s = sanitize_id(from_port);
    let to_port_s = sanitize_id(to_port);

    if mappings.is_empty() {
        // No mappings — use raw index from port ref.
        let src = PortRef {
            instance: Some(from_inst.to_string()),
            port: from_port_s,
            index: from_idx.map(|i| IndexSpec {
                elements: vec![IndexElement::Single { value: i }],
            }),
        };
        let tgt = PortRef {
            instance: Some(to_inst.to_string()),
            port: to_port_s,
            index: to_idx.map(|i| IndexSpec {
                elements: vec![IndexElement::Single { value: i }],
            }),
        };
        return vec![(src, tgt)];
    }

    // Check sequential contiguous offset: from channels contiguous, to channels = from + constant offset.
    // Sequential 1:1 starting at 1 (e.g. [1→1, 2→2, ...]) is also handled here as
    // offset==0 — it falls through to the range case below and emits explicit
    // ranges (Port[1..N]). The previous "is_sequential_1to1 -> drop index" shortcut
    // was incorrect for partial patches: the loader's min(fromCount, toCount)
    // fallback would re-inflate a partial 2-channel patch to the full port width.
    let first = &mappings[0];
    let offset_i32 = first.to_channel as i32 - first.from_channel as i32;
    let is_contiguous_offset = mappings.iter().enumerate().all(|(i, m)| {
        m.from_channel == first.from_channel + i as u32
            && (m.to_channel as i32 - m.from_channel as i32) == offset_i32
    });
    if is_contiguous_offset && mappings.len() > 1 {
        let from_start = first.from_channel;
        let from_end = mappings.last().unwrap().from_channel;
        let to_start = first.to_channel;
        let to_end = mappings.last().unwrap().to_channel;

        let src = PortRef {
            instance: Some(from_inst.to_string()),
            port: from_port_s,
            index: Some(IndexSpec {
                elements: vec![IndexElement::Range {
                    start: from_start,
                    end: from_end,
                }],
            }),
        };
        let tgt = PortRef {
            instance: Some(to_inst.to_string()),
            port: to_port_s,
            index: Some(IndexSpec {
                elements: vec![IndexElement::Range {
                    start: to_start,
                    end: to_end,
                }],
            }),
        };
        return vec![(src, tgt)];
    }

    // Non-sequential: one connect per mapping pair.
    mappings
        .iter()
        .map(|m| {
            let src = PortRef {
                instance: Some(from_inst.to_string()),
                port: from_port_s.clone(),
                index: Some(IndexSpec {
                    elements: vec![IndexElement::Single {
                        value: m.from_channel,
                    }],
                }),
            };
            let tgt = PortRef {
                instance: Some(to_inst.to_string()),
                port: to_port_s.clone(),
                index: Some(IndexSpec {
                    elements: vec![IndexElement::Single {
                        value: m.to_channel,
                    }],
                }),
            };
            (src, tgt)
        })
        .collect()
}

/// Parse `"PortName[N]"` into `(port_name, Some(N))`, or a bare
/// `"PortName"` into `("PortName", None)`.
///
/// This is used to extract the trailing channel index from a port ID before
/// sanitizing the name so that `SDI_Out[1]` becomes port `SDI_Out` with
/// index `1` rather than the garbled `SDI_Out_1_`.
pub(super) fn parse_port_ref(port_id: &str) -> (String, Option<u32>) {
    if let Some(bracket_pos) = port_id.rfind('[') {
        let port_name = &port_id[..bracket_pos];
        let rest = &port_id[bracket_pos + 1..];
        let index_str = rest.trim_end_matches(']');
        if let Ok(idx) = index_str.parse::<u32>() {
            return (port_name.to_string(), Some(idx));
        }
    }
    (port_id.to_string(), None)
}

/// Coerce an arbitrary string into a valid PatchLang identifier
/// (`[a-zA-Z_][a-zA-Z0-9_]*`).
/// Matches TypeScript `sanitizeIdentifier` behavior exactly:
/// - Replace invalid chars with underscore (keep alphanumeric and underscore)
/// - If starts with a digit, prefix with underscore
/// - Never strip leading underscores
pub(super) fn sanitize_id(s: &str) -> String {
    if s.is_empty() {
        return "Device".to_string();
    }
    // Replace invalid chars with underscore (keep alphanumeric and underscore)
    let r: String = s.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if r.is_empty() {
        return "Device".to_string();
    }
    // If starts with a digit, prefix with underscore (matches TS sanitizeIdentifier behavior)
    if r.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        format!("_{}", r)
    } else {
        r
    }
}

pub(super) fn kv_str(key: &str, value: &str) -> KeyValue {
    KeyValue {
        key: key.to_string(),
        value: KvValue::Str {
            value: value.to_string(),
        },
    }
}

pub(super) fn kv_num(key: &str, value: u32) -> KeyValue {
    KeyValue {
        key: key.to_string(),
        value: KvValue::Num { value },
    }
}
