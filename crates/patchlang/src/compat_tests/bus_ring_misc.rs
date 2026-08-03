//! Compat tests: statement tags, buses, spans, rings, stringify.

use crate::ast::*;
use crate::compat::*;
use crate::error::Span;
use super::helpers::*;

#[test]
fn statement_type_tags_correct() {
    let template_stmt = TsStatement::Template(TsTemplateDecl {
        type_tag: "Template",
        name: "Test".into(),
        params: vec![],
        meta: Default::default(),
        ports: vec![],
        bridges: vec![],
        instances: vec![],
        connects: vec![],
        slots: vec![],
        version: None,
    });
    let json = serde_json::to_value(&template_stmt).unwrap();
    assert_eq!(json["type"], "Template");

    let use_stmt = TsStatement::Use(TsUseDecl {
        type_tag: "Use",
        namespace: "yamaha".into(),
        templates: vec!["CL5".into()],
        wildcard: false,
    });
    let json = serde_json::to_value(&use_stmt).unwrap();
    assert_eq!(json["type"], "Use");
}

/// The `type` key must appear EXACTLY ONCE in the raw serialized text.
///
/// Every inner struct carries its own `type_tag` (they are serialized standalone when
/// nested inside a template), so putting serde's `tag = "type"` on `TsStatement` as well
/// emitted the key twice — invalid JSON that strict parsers may reject.
///
/// This must assert against the raw STRING. `serde_json::to_value` builds a Map, which
/// silently collapses duplicate keys, so every existing test here passed while the real
/// output was malformed. That is exactly why this went unnoticed.
#[test]
fn statement_serializes_the_type_key_exactly_once() {
    let stmt = TsStatement::Stream(TsStreamDecl {
        type_tag: "Stream",
        name: "Drums".into(),
        properties: Default::default(),
        source: None,
    });
    let raw = serde_json::to_string(&stmt).unwrap();
    assert_eq!(
        raw.matches("\"type\"").count(),
        1,
        "`type` must appear once, not duplicated by both the enum tag and the inner field: {raw}"
    );
    // and it must still be present and correct
    assert_eq!(serde_json::to_value(&stmt).unwrap()["type"], "Stream");
}

// ── Bus label ──────────────────────────────────────────────────────

#[test]
fn bus_label_roundtrips_through_parser() {
    let src = r#"
        instance FOH is CL5 {
          bus Main_LR {
            label: "SPOTIFY>FOH"
            in: Fader[1]
            output "Mix": Matrix_Out[1]
          }
        }
    "#;
    let result = crate::parse(src);
    assert!(result.errors.is_empty(), "parse errors: {:?}", result.errors);
    let instance = match &result.program.statements[0] {
        crate::ast::Statement::Instance(i) => i,
        _ => panic!("expected instance"),
    };
    assert_eq!(instance.buses[0].label.as_deref(), Some("SPOTIFY>FOH"));
}

#[test]
fn bus_label_serialises_to_json() {
    let src = r#"
        instance FOH is CL5 {
          bus Main_LR {
            label: "SPOTIFY>FOH"
            in: Fader[1]
            output "Mix": Matrix_Out[1]
          }
        }
    "#;
    let result = crate::parse(src);
    assert!(result.errors.is_empty(), "parse errors: {:?}", result.errors);
    let json = serde_json::to_value(crate::to_ts_result(&result)).unwrap();
    let bus = &json["program"]["statements"][0]["buses"][0];
    assert_eq!(bus["label"], "SPOTIFY>FOH");
}

#[test]
fn bus_without_label_omits_label_field_from_json() {
    let src = r#"
        instance FOH is CL5 {
          bus Main_LR {
            in: Fader[1]
            output "Mix": Matrix_Out[1]
          }
        }
    "#;
    let result = crate::parse(src);
    assert!(result.errors.is_empty(), "parse errors: {:?}", result.errors);
    let json = serde_json::to_value(crate::to_ts_result(&result)).unwrap();
    let bus = &json["program"]["statements"][0]["buses"][0];
    assert!(bus.get("label").is_none(), "label should be absent when not set");
}

#[test]
fn bus_label_preserves_special_characters() {
    let src = r#"
        instance FOH is CL5 {
          bus Ch_Strip {
            label: "IEM>WORSHIP-LEAD"
            in: Fader[2]
            output "IEM": IEM_Out[1]
          }
        }
    "#;
    let result = crate::parse(src);
    assert!(result.errors.is_empty(), "parse errors: {:?}", result.errors);
    let instance = match &result.program.statements[0] {
        crate::ast::Statement::Instance(i) => i,
        _ => panic!("expected instance"),
    };
    assert_eq!(instance.buses[0].label.as_deref(), Some("IEM>WORSHIP-LEAD"));
}

// ── TsBusOutput JSON shape ─────────────────────────────────────────

#[test]
fn compat_bus_output_json_shape() {
    let src = r#"
        instance Mixer is CL5 {
          bus Main_LR {
            input: Fader[1]
            output "Mix L": Matrix_Out[1]
            output "Unrouted"
          }
        }
    "#;
    let result = crate::parse(src);
    assert!(result.errors.is_empty(), "parse errors: {:?}", result.errors);
    let json = serde_json::to_value(crate::to_ts_result(&result)).unwrap();
    let outputs = &json["program"]["statements"][0]["buses"][0]["outputs"];
    let arr = outputs.as_array().expect("outputs should be an array");
    assert_eq!(arr.len(), 2);

    // First output: routed — has label and non-empty destinations
    assert_eq!(arr[0]["label"], "Mix L");
    let dests0 = arr[0]["destinations"].as_array().expect("destinations should be an array");
    assert_eq!(dests0.len(), 1);
    assert_eq!(dests0[0]["port"], "Matrix_Out");

    // Second output: unrouted — has label and empty destinations
    assert_eq!(arr[1]["label"], "Unrouted");
    let dests1 = arr[1]["destinations"].as_array().expect("destinations should be an array");
    assert!(dests1.is_empty(), "unrouted output should have empty destinations");
}

#[test]
fn compat_bus_display_label_in_json() {
    let src = r#"
        instance Mixer is CL5 {
          bus PQMM {
            label: "PQ>MM"
            input: Fader[1]
          }
        }
    "#;
    let result = crate::parse(src);
    assert!(result.errors.is_empty(), "parse errors: {:?}", result.errors);
    let json = serde_json::to_value(crate::to_ts_result(&result)).unwrap();
    let bus = &json["program"]["statements"][0]["buses"][0];
    assert_eq!(bus["label"], "PQ>MM");
}

// ── Span stripping ─────────────────────────────────────────────────

#[test]
fn spans_are_stripped_from_output() {
    let instance = InstanceDecl {
        name: "FOH".into(),
        template_name: "CL5".into(),
        args: vec![],
        version_constraint: None,
        properties: vec![],
        routes: vec![],
        buses: vec![],
        slot_assignments: vec![],
        span: Span {
            start: 10,
            end: 50,
            file: None,
        },
    };
    let ts = convert_instance(&instance);
    let json = serde_json::to_value(&ts).unwrap();
    assert!(json.get("span").is_none());
}

// ── Full fixture: worship-venue.patch ──────────────────────────────

#[test]
fn worship_venue_fixture_roundtrip() {
    let source = std::fs::read_to_string(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/examples/worship-venue.patch"
        ),
    )
    .expect("fixture file should exist");

    let result = crate::parser::parse(&source);
    assert!(result.is_valid(), "fixture should parse without errors");

    let ts_result = to_ts_result(&result);
    let json = serde_json::to_value(&ts_result).unwrap();

    // Program has type field
    assert_eq!(json["program"]["type"], "Program");

    // Should have statements
    let stmts = json["program"]["statements"].as_array().unwrap();
    assert!(!stmts.is_empty());

    // Check first template has camelCase meta as object
    let first = &stmts[0];
    assert_eq!(first["type"], "Template");
    assert!(first["meta"].is_object());
    assert_eq!(first["meta"]["manufacturer"], "Yamaha");

    // Ports have lowercase direction and flat range
    let ports = first["ports"].as_array().unwrap();
    let mic_port = ports.iter().find(|p| p["name"] == "Mic_In").unwrap();
    assert_eq!(mic_port["direction"], "in");
    assert_eq!(mic_port["rangeStart"], 1);
    assert_eq!(mic_port["rangeEnd"], 32);
    assert!(mic_port.get("range").is_none()); // flattened, not nested

    // No span fields anywhere
    assert!(first.get("span").is_none());

    // Instance has camelCase fields
    let instance = stmts
        .iter()
        .find(|s| s["type"] == "Instance" && s["name"] == "Stage_Left")
        .unwrap();
    assert_eq!(instance["templateName"], "Rio3224");
    assert!(instance["properties"].is_object());
    assert_eq!(instance["properties"]["location"], "Stage Left Wing");

    // Connect has properties as object
    let connect = stmts.iter().find(|s| s["type"] == "Connect").unwrap();
    assert!(connect["properties"].is_object());
    assert!(connect["source"]["instance"].is_string());

    // Bridge has no span
    let bridge = stmts.iter().find(|s| s["type"] == "Bridge").unwrap();
    assert!(bridge.get("span").is_none());
    assert!(bridge["source"]["instance"].is_string());

    // Signal has origin as PortRef with instance string
    let signal = stmts
        .iter()
        .find(|s| s["type"] == "Signal" && s["name"] == "Lead_Vocal")
        .unwrap();
    assert_eq!(signal["origin"]["instance"], "Stage_Left");
    assert_eq!(signal["origin"]["port"], "Mic_In");
    // indexSpec should be present for Signal origins with index
    assert!(signal["origin"]["indexSpec"].is_array());

    // Errors array should be empty
    assert!(json["errors"].as_array().unwrap().is_empty());
}

// ── Error node filtering ───────────────────────────────────────────

#[test]
fn error_statements_are_filtered_out() {
    let program = PatchProgram {
        statements: vec![
            Statement::Error(Span { start: 0, end: 5, file: None }),
            Statement::Flag(FlagDecl {
                name: "test".into(),
                properties: vec![],
                span: span(),
            }),
        ],
    };
    let ts = to_ts_program(&program);
    assert_eq!(ts.statements.len(), 1);
    match &ts.statements[0] {
        TsStatement::Flag(f) => assert_eq!(f.name, "test"),
        other => panic!("expected Flag, got {other:?}"),
    }
}

// ── Ring compat ─────────────────────────────────────────────────────

#[test]
fn ring_decl_serializes_type_tag() {
    let ring = RingDecl {
        name: "Primary".into(),
        properties: vec![KeyValue {
            key: "protocol".into(),
            value: KvValue::Str { value: "OptoCore".into() },
        }],
        members: vec![RingMember {
            instance_name: "Console".into(),
            port_name: None,
            span: span(),
        }],
        span: span(),
    };
    let ts = convert_ring(&ring);
    let json = serde_json::to_value(&ts).unwrap();
    assert_eq!(json["type"], "Ring");
    assert_eq!(json["name"], "Primary");
}

#[test]
fn ring_member_implicit_no_port_name_field() {
    let member = RingMember {
        instance_name: "Console".into(),
        port_name: None,
        span: span(),
    };
    let ts = convert_ring_member(&member);
    let json = serde_json::to_value(&ts).unwrap();
    assert_eq!(json["instanceName"], "Console");
    assert!(json.get("portName").is_none(), "portName should be absent when None");
}

#[test]
fn ring_member_explicit_has_port_name() {
    let member = RingMember {
        instance_name: "Console".into(),
        port_name: Some("OptoCore_B".into()),
        span: span(),
    };
    let ts = convert_ring_member(&member);
    let json = serde_json::to_value(&ts).unwrap();
    assert_eq!(json["instanceName"], "Console");
    assert_eq!(json["portName"], "OptoCore_B");
}

#[test]
fn ring_properties_in_camel_case() {
    let ring = RingDecl {
        name: "Test".into(),
        properties: vec![
            KeyValue { key: "protocol".into(), value: KvValue::Str { value: "OptoCore".into() } },
            KeyValue { key: "label".into(), value: KvValue::Str { value: "Main ring".into() } },
        ],
        members: vec![],
        span: span(),
    };
    let ts = convert_ring(&ring);
    assert_eq!(ts.properties.get("protocol").unwrap(), "OptoCore");
    assert_eq!(ts.properties.get("label").unwrap(), "Main ring");
}

// ── Ring roundtrip through to_ts_result ─────────────────────────────

#[test]
fn ring_roundtrip_through_to_ts_result() {
    let source = r#"ring Primary {
        protocol: "OptoCore"
        member Console
        member StageBox.OptoCore_A
    }"#;
    let result = crate::parser::parse(source);
    assert!(result.is_valid(), "ring source should parse cleanly: {:?}", result.errors);

    let ts_result = to_ts_result(&result);
    let json = serde_json::to_value(&ts_result).unwrap();

    let stmts = json["program"]["statements"].as_array().unwrap();
    assert_eq!(stmts.len(), 1, "should have exactly one statement");
    assert_eq!(stmts[0]["type"], "Ring");
    assert_eq!(stmts[0]["name"], "Primary");
    assert_eq!(stmts[0]["properties"]["protocol"], "OptoCore");

    let members = stmts[0]["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0]["instanceName"], "Console");
    assert_eq!(members[1]["instanceName"], "StageBox");
    assert_eq!(members[1]["portName"], "OptoCore_A");
}

// ── Mapping spec returns None for unrecognized input ───────────────

#[test]
fn mapping_unrecognized_returns_none() {
    assert!(
        parse_mapping_spec("banana").is_none(),
        "unrecognized mapping spec should return None"
    );
    assert!(
        parse_mapping_spec("").is_none(),
        "empty mapping spec should return None"
    );
    assert!(
        parse_mapping_spec("offset abc").is_none(),
        "offset with non-numeric value should return None"
    );
}

// ── PortRef stringify edge cases ───────────────────────────────────

#[test]
fn stringify_port_ref_local_no_index() {
    let pr = PortRef {
        instance: None,
        port: "Out".into(),
        index: None,
    };
    assert_eq!(stringify_port_ref(&pr), "Out");
}

#[test]
fn stringify_port_ref_with_range_index() {
    let pr = PortRef {
        instance: Some("SB".into()),
        port: "Ch".into(),
        index: Some(IndexSpec {
            elements: vec![
                IndexElement::Single { value: 1 },
                IndexElement::Range { start: 3, end: 5 },
            ],
        }),
    };
    assert_eq!(stringify_port_ref(&pr), "SB.Ch[1,3..5]");
}
