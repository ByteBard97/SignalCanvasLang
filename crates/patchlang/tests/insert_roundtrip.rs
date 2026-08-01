//! Insert send/return round-trip across the canvas DTO boundary (issue #31).
//!
//! Inserts were sidecar-only in the frontend because `.patch` text survived `format()`
//! but died on `load_from_patch()`: `ChannelLabelOutput` was a fixed struct that
//! dropped every unlisted property, and `BusOutput` had no extension point at all.
//! These tests pin the two carriers — channel labels (string-encoded property) and
//! buses (native `bus { }` grammar) — plus the invariants that make an insert an
//! insert: ordered legs, independent endpoints, no grouping.

use patchlang::builder::canvas_load::load_from_patch;
use patchlang::builder::insert_endpoints::InsertEndpoint;

/// Two ports so a leg can reference a port other than the one it inserts on.
const TEMPLATE: &str = r#"
template Desk {
  ports {
    Mic_In[1..8]: in(XLR)
    Ext_Out[1..16]: out(XLR)
    Ext_In[1..16]: in(XLR)
    A[1..8]: out(XLR)
    B[1..8]: out(XLR)
  }
}
instance FOH is Desk {}
"#;

fn label_patch(props: &str) -> String {
    format!("{TEMPLATE}\nconfig FOH {{\n  label Mic_In[1]: \"Kick\" {{\n{props}\n  }}\n}}\n")
}

fn bus_patch(body: &str) -> String {
    format!(
        "{}\ninstance Console is Desk {{\n  bus Main_LR {{\n    input: Mic_In[1]\n{body}\n    output \"Mix\": A[1]\n  }}\n}}\n",
        TEMPLATE
    )
}

fn ep(port: &str, channel: u32) -> InsertEndpoint {
    InsertEndpoint { instance: None, port: port.into(), channel }
}

fn load_label_inserts(src: &str) -> (Vec<InsertEndpoint>, Vec<InsertEndpoint>) {
    let out = load_from_patch(src, "").expect("load");
    let inst = out.instances.iter().find(|i| i.name == "FOH").expect("FOH instance");
    let labels = inst.channel_labels.get("Mic_In").expect("Mic_In labels");
    let label = labels.first().expect("channel 1 label");
    (label.insert_send.clone(), label.insert_return.clone())
}

fn load_bus_inserts(src: &str) -> (Vec<InsertEndpoint>, Vec<InsertEndpoint>) {
    let out = load_from_patch(src, "").expect("load");
    let inst = out.instances.iter().find(|i| i.name == "Console").expect("Console instance");
    let bus = inst.internal_buses.first().expect("one bus");
    (bus.insert_send.clone(), bus.insert_return.clone())
}

// ---------------------------------------------------------------------------
// Channel labels
// ---------------------------------------------------------------------------

#[test]
fn label_mono_insert_survives_load() {
    let (send, ret) = load_label_inserts(&label_patch(
        "    insert_send: \"Ext_Out[3]\"\n    insert_return: \"Ext_In[3]\"",
    ));
    assert_eq!(send, vec![ep("Ext_Out", 3)]);
    assert_eq!(ret, vec![ep("Ext_In", 3)]);
}

#[test]
fn label_stereo_insert_preserves_leg_order() {
    // If L and R are ever reordered the stereo image silently swaps.
    let (send, _) = load_label_inserts(&label_patch(
        "    insert_send: \"Ext_Out[2], Ext_Out[1]\"",
    ));
    assert_eq!(
        send,
        vec![ep("Ext_Out", 2), ep("Ext_Out", 1)],
        "descending order must be preserved verbatim, not sorted"
    );
}

#[test]
fn label_scattered_endpoints_do_not_collapse() {
    // The ticket's real-world case: send on 3 & 10, return on 4 & 8. Endpoints are
    // independent — no adjacency constraint, so this must NOT become a range.
    let (send, ret) = load_label_inserts(&label_patch(
        "    insert_send: \"Ext_Out[3], Ext_Out[10]\"\n    insert_return: \"Ext_In[4], Ext_In[8]\"",
    ));
    assert_eq!(send, vec![ep("Ext_Out", 3), ep("Ext_Out", 10)]);
    assert_eq!(ret, vec![ep("Ext_In", 4), ep("Ext_In", 8)]);
}

#[test]
fn label_interleaved_ports_are_not_grouped() {
    // The bus-input loader groups by (instance, port) and unions channels because bus
    // inputs are set-like. Insert legs must NOT: grouping turns [A1, B1, A2] into
    // [A1, A2, B1] and swaps the pair.
    let (send, _) = load_label_inserts(&label_patch("    insert_send: \"A[1], B[1], A[2]\""));
    assert_eq!(send, vec![ep("A", 1), ep("B", 1), ep("A", 2)]);
}

#[test]
fn label_unknown_properties_survive_via_the_bag() {
    // The mechanism that ends the sidecar stopgap: any key with no dedicated field
    // used to be silently dropped on load.
    let src = label_patch("    stand: \"Tall boom\"\n    gain: \"+12\"");
    let out = load_from_patch(&src, "").expect("load");
    let inst = out.instances.iter().find(|i| i.name == "FOH").unwrap();
    let label = &inst.channel_labels.get("Mic_In").unwrap()[0];
    assert_eq!(label.properties.get("stand").map(String::as_str), Some("Tall boom"));
    assert_eq!(label.properties.get("gain").map(String::as_str), Some("+12"));
}

#[test]
fn label_dedicated_keys_are_not_duplicated_into_the_bag() {
    // A key lives in exactly one place — typed field OR bag, never both.
    let src = label_patch(
        "    phantom: \"true\"\n    capsule: \"KSM8\"\n    insert_send: \"Ext_Out[3]\"",
    );
    let out = load_from_patch(&src, "").expect("load");
    let inst = out.instances.iter().find(|i| i.name == "FOH").unwrap();
    let label = &inst.channel_labels.get("Mic_In").unwrap()[0];
    assert!(label.phantom);
    assert_eq!(label.capsule.as_deref(), Some("KSM8"));
    assert_eq!(label.insert_send, vec![ep("Ext_Out", 3)]);
    for key in ["phantom", "capsule", "insert_send"] {
        assert!(
            !label.properties.contains_key(key),
            "'{key}' has a dedicated field and must not also sit in the bag, got {:?}",
            label.properties
        );
    }
}

#[test]
fn label_malformed_insert_is_preserved_not_blanked() {
    // Typed parse is all-or-nothing. When it fails the raw string must stay in the bag
    // so a round-trip re-emits it intact — the fix must not destroy the data it exists
    // to preserve.
    let src = label_patch("    insert_send: \"Ext_Out[bogus]\"");
    let out = load_from_patch(&src, "").expect("load");
    let inst = out.instances.iter().find(|i| i.name == "FOH").unwrap();
    let label = &inst.channel_labels.get("Mic_In").unwrap()[0];
    assert!(label.insert_send.is_empty(), "malformed legs must not be half-parsed");
    assert_eq!(
        label.properties.get("insert_send").map(String::as_str),
        Some("Ext_Out[bogus]"),
        "raw string must survive in the bag so re-emit is byte-faithful"
    );
}

#[test]
fn label_range_index_does_not_silently_double_the_width() {
    // `[1..2]` must never expand into two endpoints — that would change how many
    // channels the insert claims.
    let src = label_patch("    insert_send: \"Ext_Out[1..2]\"");
    let out = load_from_patch(&src, "").expect("load");
    let inst = out.instances.iter().find(|i| i.name == "FOH").unwrap();
    let label = &inst.channel_labels.get("Mic_In").unwrap()[0];
    assert!(label.insert_send.is_empty());
    assert_eq!(
        label.properties.get("insert_send").map(String::as_str),
        Some("Ext_Out[1..2]")
    );
}

#[test]
fn unquoted_port_ref_property_is_not_silently_dropped() {
    // `parse_key_value_full` accepts a bare identifier as `KvValue::PortRef`, so the
    // unquoted form parses cleanly — and `kv_map` used to return None for it, dropping
    // the value on the floor. Right-looking syntax, data gone.
    let src = label_patch("    insert_send: Ext_Out[3]");
    let out = load_from_patch(&src, "").expect("load");
    let inst = out.instances.iter().find(|i| i.name == "FOH").unwrap();
    let label = &inst.channel_labels.get("Mic_In").unwrap()[0];
    assert_eq!(
        label.insert_send,
        vec![ep("Ext_Out", 3)],
        "unquoted port-ref property must reach the DTO, not vanish in kv_map"
    );
}

// ---------------------------------------------------------------------------
// Buses
// ---------------------------------------------------------------------------

#[test]
fn bus_mono_insert_survives_load() {
    let (send, ret) = load_bus_inserts(&bus_patch(
        "    insert_send: Ext_Out[3]\n    insert_return: Ext_In[3]",
    ));
    assert_eq!(send, vec![ep("Ext_Out", 3)]);
    assert_eq!(ret, vec![ep("Ext_In", 3)]);
}

#[test]
fn bus_stereo_insert_preserves_leg_order() {
    let (send, _) = load_bus_inserts(&bus_patch("    insert_send: Ext_Out[2], Ext_Out[1]"));
    assert_eq!(send, vec![ep("Ext_Out", 2), ep("Ext_Out", 1)]);
}

#[test]
fn bus_scattered_endpoints_do_not_collapse() {
    let (send, ret) = load_bus_inserts(&bus_patch(
        "    insert_send: Ext_Out[3], Ext_Out[10]\n    insert_return: Ext_In[4], Ext_In[8]",
    ));
    assert_eq!(send, vec![ep("Ext_Out", 3), ep("Ext_Out", 10)]);
    assert_eq!(ret, vec![ep("Ext_In", 4), ep("Ext_In", 8)]);
}

#[test]
fn bus_interleaved_ports_are_not_grouped_or_unioned() {
    // This is the one that catches a copy-paste of the neighbouring bus-input grouping.
    let (send, _) = load_bus_inserts(&bus_patch("    insert_send: A[1], B[1], A[2]"));
    assert_eq!(
        send,
        vec![ep("A", 1), ep("B", 1), ep("A", 2)],
        "insert legs must stay a flat ordered list — bus INPUTS are grouped, legs are not"
    );
}

#[test]
fn bus_without_inserts_still_loads() {
    let (send, ret) = load_bus_inserts(&bus_patch("    input: Mic_In[2]"));
    assert!(send.is_empty() && ret.is_empty());
}

// ---------------------------------------------------------------------------
// Formatter / idempotency
// ---------------------------------------------------------------------------

#[test]
fn bus_inserts_survive_parse_format_parse() {
    use patchlang::formatter::format_program;
    use patchlang::parser::parse;

    let src = bus_patch("    insert_send: Ext_Out[3], Ext_Out[10]\n    insert_return: Ext_In[4]");
    let first = format_program(&parse(&src).program);
    assert!(
        first.contains("insert_send: Ext_Out[3], Ext_Out[10]"),
        "formatter must emit the send legs in order, got:\n{first}"
    );
    assert!(first.contains("insert_return: Ext_In[4]"), "got:\n{first}");

    // Idempotent: formatting the formatted output changes nothing.
    let second = format_program(&parse(&first).program);
    assert_eq!(first, second, "format must be idempotent");

    // And the legs still round-trip through the DTO after a format pass.
    let (send, ret) = load_bus_inserts(&first);
    assert_eq!(send, vec![ep("Ext_Out", 3), ep("Ext_Out", 10)]);
    assert_eq!(ret, vec![ep("Ext_In", 4)]);
}

#[test]
fn label_inserts_survive_parse_format_parse() {
    use patchlang::formatter::format_program;
    use patchlang::parser::parse;

    let src = label_patch("    insert_send: \"Ext_Out[3], Ext_Out[10]\"");
    let first = format_program(&parse(&src).program);
    let second = format_program(&parse(&first).program);
    assert_eq!(first, second, "format must be idempotent");

    let (send, _) = load_label_inserts(&first);
    assert_eq!(send, vec![ep("Ext_Out", 3), ep("Ext_Out", 10)]);
}

// ---------------------------------------------------------------------------
// Backward compatibility — the break that would hit every existing frontend build
// ---------------------------------------------------------------------------

#[test]
fn legacy_bus_json_without_insert_fields_still_deserializes() {
    // `patchlang-wasm`'s add_bus/update_bus deserialize `ast::BusEntry` DIRECTLY from
    // frontend JSON. Without #[serde(default)] on the new fields, serde rejects the
    // whole payload with "missing field `insert_send`" and every existing frontend
    // build breaks the moment this ships.
    let legacy = r#"{
      "name": "Main_LR",
      "label": null,
      "inputs": [],
      "outputs": [],
      "span": {"start": 0, "end": 0}
    }"#;
    let bus: patchlang::ast::BusEntry =
        serde_json::from_str(legacy).expect("legacy add_bus payload must still deserialize");
    assert!(bus.insert_send.is_empty() && bus.insert_return.is_empty());
}

#[test]
fn legacy_emit_input_json_without_insert_fields_still_deserializes() {
    use patchlang::builder::canvas_input::{BusEmitInput, ChannelLabelEmitInput};

    let label: ChannelLabelEmitInput = serde_json::from_str(
        r#"{"channel_index":1,"label":"Kick","phantom":false,"propagated":false,
            "source_type":null,"capsule":null,"rf_band":null}"#,
    )
    .expect("legacy label payload must still deserialize");
    assert!(label.insert_send.is_empty() && label.properties.is_empty());

    let bus: BusEmitInput = serde_json::from_str(
        r#"{"label":"Main","display_name":null,"input_interface":"Mic_In",
            "input_channels":[1],"output_interface":"A","output_channels":[1],
            "named_outputs":[]}"#,
    )
    .expect("legacy bus payload must still deserialize");
    assert!(bus.insert_send.is_empty() && bus.insert_return.is_empty());
}

// ---------------------------------------------------------------------------
// Bus legs must name exactly one channel — rejected loudly, never dropped quietly
// ---------------------------------------------------------------------------

#[test]
fn bus_insert_leg_with_a_range_index_is_a_parse_error() {
    // The DTO carries one entry per leg. A range would have to be expanded (doubling
    // the claimed width) or dropped (losing it) at that boundary — so it is refused
    // here instead, where the user can see it.
    let result = patchlang::parse(&bus_patch("    insert_send: Ext_Out[1..2]"));
    assert!(
        result.errors.iter().any(|e| e.message.contains("exactly one channel")),
        "expected a single-channel diagnostic, got: {:?}",
        result.errors
    );
}

#[test]
fn bus_insert_leg_without_an_index_is_a_parse_error() {
    let result = patchlang::parse(&bus_patch("    insert_send: Ext_Out"));
    assert!(
        result.errors.iter().any(|e| e.message.contains("exactly one channel")),
        "a leg with no channel must not be silently dropped, got: {:?}",
        result.errors
    );
}

#[test]
fn valid_bus_insert_legs_produce_no_diagnostics() {
    let result = patchlang::parse(&bus_patch(
        "    insert_send: Ext_Out[3], Ext_Out[10]\n    insert_return: Ext_In[4]",
    ));
    assert!(result.errors.is_empty(), "unexpected diagnostics: {:?}", result.errors);
}

// ---------------------------------------------------------------------------
// The seam that actually retires the sidecar: load → re-emit
// ---------------------------------------------------------------------------

/// Loading `.patch` text and emitting it again must preserve both the insert legs and
/// the unknown properties riding the verbatim bag.
///
/// This is the cycle that matters in production — the earlier emit tests start from a
/// hand-built `ChannelLabelEmitInput`, which cannot catch a loss on the load side.
#[test]
fn label_inserts_and_unknown_properties_survive_load_then_reemit() {
    use patchlang::builder::canvas_input::ChannelLabelEmitInput;
    use patchlang::builder::insert_endpoints::format_insert_list;

    let src = label_patch(
        "    stand: \"tall boom\"\n    gain: \"+12\"\n    insert_send: \"Ext_Out[3], Ext_Out[10]\"",
    );
    let out = load_from_patch(&src, "").expect("load");
    let inst = out.instances.iter().find(|i| i.name == "FOH").unwrap();
    let loaded = &inst.channel_labels.get("Mic_In").unwrap()[0];

    // Rebuild the emit input from exactly what the load handed back — the conversion
    // the frontend performs. Every field must survive the hand-off.
    let round_tripped = ChannelLabelEmitInput {
        channel_index: loaded.channel_index,
        label: loaded.label.clone(),
        phantom: loaded.phantom,
        propagated: loaded.propagated,
        source_type: loaded.source_type.clone(),
        capsule: loaded.capsule.clone(),
        rf_band: loaded.rf_band.clone(),
        insert_send: loaded.insert_send.clone(),
        insert_return: loaded.insert_return.clone(),
        properties: loaded.properties.clone(),
    };

    assert_eq!(
        format_insert_list(&round_tripped.insert_send),
        "Ext_Out[3], Ext_Out[10]",
        "insert legs must survive the load → emit-input hand-off"
    );
    assert_eq!(round_tripped.properties.get("stand").map(String::as_str), Some("tall boom"));
    assert_eq!(round_tripped.properties.get("gain").map(String::as_str), Some("+12"));
    assert!(
        !round_tripped.properties.contains_key("insert_send"),
        "insert_send is owned by the typed field; duplicating it would double-emit"
    );
}
