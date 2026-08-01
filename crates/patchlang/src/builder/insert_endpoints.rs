//! Insert send/return endpoint lists (issue #31).
//!
//! An **insert** breaks a channel or bus out to an external send/return path. Because
//! the signal physically leaves the console, the endpoints are real signal-flow data
//! and belong in `.patch`, not the layout sidecar.
//!
//! Each of `insert_send` / `insert_return` is an **ordered list** of endpoints —
//! `[L]` mono, `[L, R]` stereo. Endpoints are independent: no adjacency and no width
//! constraint, so a scattered real-world patch (send on MADI 3 and 10, return on MADI
//! 4 and 8) is representable. Order is significant; L and R must never be reordered.
//!
//! Channel labels carry this list as a **string-valued property**, because a label
//! property value holds at most one `KvValue::PortRef` and an insert list is inherently
//! multi-valued. Buses carry it natively as `Vec<PortRef>` in the `bus { }` grammar,
//! because `BusEntry` has no property bag at all. This module is the single encoder /
//! decoder for the string form, shared by the load and emit directions.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One leg of an insert send or return.
///
/// Same-device by convention — the frontend's `InsertEndpoint` points at an interface
/// on the SAME device. `instance` exists only so an instance-qualified reference
/// round-trips unchanged if one ever appears; nothing is built on it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct InsertEndpoint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub instance: Option<String>,
    pub port: String,
    pub channel: u32,
}

/// Decode the string form, e.g. `"Ext_Out[3], Ext_Out[10]"`.
///
/// All-or-nothing on purpose: returns `Some` only when **every** comma-separated leg
/// parses cleanly, `None` if any leg is malformed. Callers pair this with the verbatim
/// property bag — a clean parse means the typed field owns the data and the key is
/// dropped from the bag; `None` means the key stays in the bag and is re-emitted
/// byte-for-byte. So data is always carried by exactly one mechanism and a malformed
/// (but intact) source string is never blanked by the very fix meant to preserve it.
pub fn parse_insert_list(raw: &str) -> Option<Vec<InsertEndpoint>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.split(',').map(parse_endpoint).collect()
}

/// Parse a single `Port[3]` or `Instance.Port[3]` leg.
fn parse_endpoint(leg: &str) -> Option<InsertEndpoint> {
    let leg = leg.trim();
    let open = leg.find('[')?;
    if !leg.ends_with(']') {
        return None;
    }
    let path = leg[..open].trim();
    let index = leg[open + 1..leg.len() - 1].trim();

    // A range or multi-element index (`[1..2]`, `[1,2]`) makes the whole leg malformed.
    // It must never be expanded into two endpoints or clamped to one — either would
    // silently change the insert's width.
    let channel: u32 = index.parse().ok()?;

    let (instance, port) = match path.split_once('.') {
        Some((inst, port)) => (Some(inst.trim().to_string()), port.trim()),
        None => (None, path),
    };
    if port.is_empty() || !is_identifier(port) {
        return None;
    }
    if let Some(inst) = &instance {
        if inst.is_empty() || !is_identifier(inst) {
            return None;
        }
    }
    Some(InsertEndpoint { instance, port: port.to_string(), channel })
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Encode the canonical string form. Inverse of [`parse_insert_list`].
///
/// Takes endpoints whose `port` is already a resolved PatchLang port name. The emit
/// path resolves interface ids first (see `canvas_emit::routes::insert_legs_to_port_refs`)
/// and then calls [`format_port_ref_list`]; this entry point is for endpoints that are
/// already port-named, such as anything that came back from [`parse_insert_list`].
pub fn format_insert_list(endpoints: &[InsertEndpoint]) -> String {
    endpoints
        .iter()
        .map(|e| match &e.instance {
            Some(inst) => format!("{}.{}[{}]", inst, e.port, e.channel),
            None => format!("{}[{}]", e.port, e.channel),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Encode resolved `PortRef`s into the canonical string form used by label properties.
///
/// A leg whose index is missing or is a range is dropped rather than emitted with a
/// wrong or absent channel — the same all-or-nothing stance [`parse_insert_list`] takes
/// on the way in. In practice resolution always produces a single-channel index.
pub fn format_port_ref_list(refs: &[crate::ast::PortRef]) -> String {
    refs.iter()
        .filter_map(|pr| {
            let elements = &pr.index.as_ref()?.elements;
            let [crate::ast::IndexElement::Single { value }] = elements.as_slice() else {
                return None;
            };
            Some(match &pr.instance {
                Some(inst) => format!("{}.{}[{}]", inst, pr.port, value),
                None => format!("{}[{}]", pr.port, value),
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(port: &str, channel: u32) -> InsertEndpoint {
        InsertEndpoint { instance: None, port: port.into(), channel }
    }

    #[test]
    fn mono_single_endpoint() {
        assert_eq!(parse_insert_list("Ext_Out[3]"), Some(vec![ep("Ext_Out", 3)]));
    }

    #[test]
    fn stereo_preserves_order() {
        // L/R order is significant — a reorder silently swaps the stereo image.
        let parsed = parse_insert_list("Ext_L[1], Ext_R[2]").expect("parses");
        assert_eq!(parsed, vec![ep("Ext_L", 1), ep("Ext_R", 2)]);
    }

    #[test]
    fn scattered_channels_do_not_collapse_to_a_range() {
        // The ticket's real-world case: send on MADI 3 and 10.
        let parsed = parse_insert_list("MADI[3], MADI[10]").expect("parses");
        assert_eq!(parsed, vec![ep("MADI", 3), ep("MADI", 10)]);
    }

    #[test]
    fn interleaved_ports_are_not_grouped_or_unioned() {
        // The neighbouring bus-input code groups by (instance, port) and unions
        // channels because bus inputs are set-like. Insert legs are NOT: grouping
        // would turn [A1, B1, A2] into [A1, A2, B1] and swap the stereo pair.
        let parsed = parse_insert_list("A[1], B[1], A[2]").expect("parses");
        assert_eq!(parsed, vec![ep("A", 1), ep("B", 1), ep("A", 2)]);
    }

    #[test]
    fn range_index_makes_the_whole_leg_malformed() {
        // Never expand to two endpoints, never clamp to one.
        assert_eq!(parse_insert_list("Ext_Out[1..2]"), None);
        assert_eq!(parse_insert_list("Ext_Out[1], Ext_Out[2..3]"), None);
    }

    #[test]
    fn malformed_legs_reject_the_whole_list() {
        // All-or-nothing: the caller keeps the raw string in the property bag instead.
        assert_eq!(parse_insert_list("Ext_Out[bogus]"), None);
        assert_eq!(parse_insert_list("Ext_Out"), None);
        assert_eq!(parse_insert_list("[3]"), None);
        assert_eq!(parse_insert_list("Ext_Out[3"), None);
        assert_eq!(parse_insert_list(""), None);
        assert_eq!(parse_insert_list("   "), None);
        // One good leg does not rescue a bad one.
        assert_eq!(parse_insert_list("Ext_Out[3], nope"), None);
    }

    #[test]
    fn instance_qualified_endpoint_round_trips() {
        let parsed = parse_insert_list("Engine.Ext_Out[3]").expect("parses");
        assert_eq!(
            parsed,
            vec![InsertEndpoint {
                instance: Some("Engine".into()),
                port: "Ext_Out".into(),
                channel: 3,
            }]
        );
        assert_eq!(format_insert_list(&parsed), "Engine.Ext_Out[3]");
    }

    #[test]
    fn format_is_the_inverse_of_parse() {
        for src in ["Ext_Out[3]", "Ext_L[1], Ext_R[2]", "MADI[3], MADI[10]", "A[1], B[1], A[2]"] {
            let parsed = parse_insert_list(src).expect("parses");
            assert_eq!(format_insert_list(&parsed), src, "round-trip differs for {src}");
            assert_eq!(parse_insert_list(&format_insert_list(&parsed)), Some(parsed));
        }
    }

    #[test]
    fn whitespace_is_tolerated_and_normalised() {
        let parsed = parse_insert_list("  Ext_L[1] ,Ext_R[ 2 ]  ").expect("parses");
        assert_eq!(parsed, vec![ep("Ext_L", 1), ep("Ext_R", 2)]);
        assert_eq!(format_insert_list(&parsed), "Ext_L[1], Ext_R[2]");
    }

    #[test]
    fn empty_endpoint_list_formats_empty() {
        assert_eq!(format_insert_list(&[]), "");
    }
}
