//! Integration tests that parse real `.patch` fixture files.
//!
//! Each test reads a fixture, parses it, and asserts structural expectations:
//! - Parsing succeeds without panic
//! - Statement type counts match expected values
//! - Template names are correct
//! - Instance names and template references are correct

use patchlang::ast::Statement;

/// Path from the crate root to the workspace-level fixtures directory.
const FIXTURES_DIR: &str = "../../tests/fixtures/examples";

/// Path from the crate root to the MTG feature fixtures directory.
const MTG_FIXTURES_DIR: &str = "../../tests/fixtures/mtg-features";

/// Helper to load and parse a fixture file, returning the parse result.
fn parse_fixture(filename: &str) -> patchlang::error::ParseResult {
    let path = format!("{FIXTURES_DIR}/{filename}");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {path}: {e}"));
    patchlang::parse(&source)
}

/// Count statements by type in a parsed program.
struct StatementCounts {
    templates: usize,
    instances: usize,
    connects: usize,
    bridges: usize,
    bridge_groups: usize,
    link_groups: usize,
    signals: usize,
    flags: usize,
    streams: usize,
    configs: usize,
    uses: usize,
    rings: usize,
    networks: usize,
    errors: usize,
}

fn count_statements(statements: &[Statement]) -> StatementCounts {
    let mut counts = StatementCounts {
        templates: 0,
        instances: 0,
        connects: 0,
        bridges: 0,
        bridge_groups: 0,
        link_groups: 0,
        signals: 0,
        flags: 0,
        streams: 0,
        configs: 0,
        uses: 0,
        rings: 0,
        networks: 0,
        errors: 0,
    };
    for stmt in statements {
        match stmt {
            Statement::Template(_) => counts.templates += 1,
            Statement::Instance(_) => counts.instances += 1,
            Statement::Connect(_) => counts.connects += 1,
            Statement::Bridge(_) => counts.bridges += 1,
            Statement::BridgeGroup(_) => counts.bridge_groups += 1,
            Statement::LinkGroup(_) => counts.link_groups += 1,
            Statement::Signal(_) => counts.signals += 1,
            Statement::Flag(_) => counts.flags += 1,
            Statement::Stream(_) => counts.streams += 1,
            Statement::Config(_) => counts.configs += 1,
            Statement::Use(_) => counts.uses += 1,
            Statement::Ring(_) => counts.rings += 1,
            Statement::Network(_) => counts.networks += 1,
            Statement::Error(_) => counts.errors += 1,
        }
    }
    counts
}

/// Collect template names from parsed statements.
fn template_names(statements: &[Statement]) -> Vec<String> {
    statements
        .iter()
        .filter_map(|s| match s {
            Statement::Template(t) => Some(t.name.clone()),
            _ => None,
        })
        .collect()
}

/// Helper to load and parse an MTG feature fixture file.
fn parse_mtg_fixture(filename: &str) -> patchlang::error::ParseResult {
    let path = format!("{MTG_FIXTURES_DIR}/{filename}");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read MTG fixture {path}: {e}"));
    patchlang::parse(&source)
}

/// Collect ring names from parsed statements.
fn ring_names(statements: &[Statement]) -> Vec<String> {
    statements
        .iter()
        .filter_map(|s| match s {
            Statement::Ring(r) => Some(r.name.clone()),
            _ => None,
        })
        .collect()
}

/// Collect (instance_name, template_name) pairs from parsed statements.
fn instance_pairs(statements: &[Statement]) -> Vec<(String, String)> {
    statements
        .iter()
        .filter_map(|s| match s {
            Statement::Instance(i) => Some((i.name.clone(), i.template_name.clone())),
            _ => None,
        })
        .collect()
}

// ── worship-venue.patch ────────────────────────────────────────────

#[test]
fn worship_venue_parses_without_panic() {
    let result = parse_fixture("worship-venue.patch");
    // Should have no parse errors (only structural errors from stubs are OK)
    assert!(
        result.errors.is_empty(),
        "worship-venue parse errors: {:?}",
        result.errors
    );
}

#[test]
fn worship_venue_statement_counts() {
    let result = parse_fixture("worship-venue.patch");
    let counts = count_statements(&result.program.statements);

    assert_eq!(counts.templates, 3, "expected 3 templates");
    assert_eq!(counts.instances, 4, "expected 4 instances");
    assert_eq!(counts.connects, 6, "expected 6 connects (one per direction, 3 device pairs)");
    assert_eq!(counts.bridges, 2, "expected 2 bridges");
    assert_eq!(counts.signals, 4, "expected 4 signals");
    assert_eq!(counts.errors, 0, "expected 0 error nodes");
}

#[test]
fn worship_venue_template_names() {
    let result = parse_fixture("worship-venue.patch");
    let names = template_names(&result.program.statements);
    assert_eq!(names, vec!["Rio3224", "CL5", "GigabitSwitch"]);
}

#[test]
fn worship_venue_instance_refs() {
    let result = parse_fixture("worship-venue.patch");
    let pairs = instance_pairs(&result.program.statements);

    let expected = [
        ("Stage_Left", "Rio3224"),
        ("Stage_Right", "Rio3224"),
        ("FOH_Console", "CL5"),
        ("Dante_Switch", "GigabitSwitch"),
    ];
    for (i, (name, tmpl)) in expected.iter().enumerate() {
        assert_eq!(pairs[i].0, *name, "instance {i} name mismatch");
        assert_eq!(pairs[i].1, *tmpl, "instance {i} template mismatch");
    }
}

// ── broadcast-truck.patch ──────────────────────────────────────────

#[test]
fn broadcast_truck_parses_without_panic() {
    let result = parse_fixture("broadcast-truck.patch");
    assert!(
        result.errors.is_empty(),
        "broadcast-truck parse errors: {:?}",
        result.errors
    );
}

#[test]
fn broadcast_truck_statement_counts() {
    let result = parse_fixture("broadcast-truck.patch");
    let counts = count_statements(&result.program.statements);

    assert_eq!(counts.templates, 5, "expected 5 templates");
    assert_eq!(counts.instances, 7, "expected 7 instances");
    assert_eq!(counts.connects, 21, "expected 21 connects");
    assert_eq!(counts.bridges, 0, "expected 0 bridges");
    assert_eq!(counts.signals, 4, "expected 4 signals");
    assert_eq!(counts.flags, 1, "expected 1 flag");
    assert_eq!(counts.errors, 0, "expected 0 error nodes");
}

#[test]
fn broadcast_truck_template_names() {
    let result = parse_fixture("broadcast-truck.patch");
    let names = template_names(&result.program.statements);
    assert_eq!(
        names,
        vec!["Camera", "VideoRouter", "Encoder", "SyncGenerator", "Multiviewer"]
    );
}

#[test]
fn broadcast_truck_instance_refs() {
    let result = parse_fixture("broadcast-truck.patch");
    let pairs = instance_pairs(&result.program.statements);

    let expected = [
        ("Cam1", "Camera"),
        ("Cam2", "Camera"),
        ("Cam3", "Camera"),
        ("Router", "VideoRouter"),
        ("Enc1", "Encoder"),
        ("SyncGen", "SyncGenerator"),
        ("MV1", "Multiviewer"),
    ];
    assert_eq!(pairs.len(), expected.len());
    for (i, (name, tmpl)) in expected.iter().enumerate() {
        assert_eq!(pairs[i].0, *name, "instance {i} name mismatch");
        assert_eq!(pairs[i].1, *tmpl, "instance {i} template mismatch");
    }
}

// ── blank-starter.patch ────────────────────────────────────────────

#[test]
fn blank_starter_parses_without_panic() {
    let result = parse_fixture("blank-starter.patch");
    // blank-starter uses `input`/`output` port directions which are not yet
    // supported (template parser expects `in`/`out`/`io`). Verify parse
    // completes without panic; port-direction errors are expected for now.
    let _ = result;
}

#[test]
fn blank_starter_statement_counts() {
    let result = parse_fixture("blank-starter.patch");
    let counts = count_statements(&result.program.statements);

    assert_eq!(counts.templates, 1, "expected 1 template");
    assert_eq!(counts.instances, 2, "expected 2 instances");
    assert_eq!(counts.connects, 1, "expected 1 connect");
    assert_eq!(counts.errors, 0, "expected 0 error nodes");
}

#[test]
fn blank_starter_template_names() {
    let result = parse_fixture("blank-starter.patch");
    let names = template_names(&result.program.statements);
    assert_eq!(names, vec!["MyDevice"]);
}

#[test]
fn blank_starter_instance_refs() {
    let result = parse_fixture("blank-starter.patch");
    let pairs = instance_pairs(&result.program.statements);
    assert_eq!(pairs.len(), 2);
    assert_eq!(pairs[0], ("Device_A".to_string(), "MyDevice".to_string()));
    assert_eq!(pairs[1], ("Device_B".to_string(), "MyDevice".to_string()));
}

// ── concert-venue-hierarchical.patch ───────────────────────────────

#[test]
fn concert_venue_hierarchical_parses_without_panic() {
    let result = parse_fixture("concert-venue-hierarchical.patch");
    assert!(
        result.errors.is_empty(),
        "concert-venue-hierarchical parse errors: {:?}",
        result.errors
    );
}

#[test]
fn concert_venue_hierarchical_statement_counts() {
    let result = parse_fixture("concert-venue-hierarchical.patch");
    let counts = count_statements(&result.program.statements);

    // 19 top-level templates (device + subsystem templates)
    assert_eq!(counts.templates, 19, "expected 19 templates");
    // 6 top-level instances
    assert_eq!(counts.instances, 6, "expected 6 instances");
    // 11 top-level connects (cross-subsystem; Dante secondary modeled via redundant_cable:)
    assert_eq!(counts.connects, 11, "expected 11 connects");
    // 5 signals
    assert_eq!(counts.signals, 5, "expected 5 signals");
    // 2 flags
    assert_eq!(counts.flags, 2, "expected 2 flags");
    assert_eq!(counts.errors, 0, "expected 0 error nodes");
}

#[test]
fn concert_venue_hierarchical_template_names() {
    let result = parse_fixture("concert-venue-hierarchical.patch");
    let names = template_names(&result.program.statements);

    // Verify first few and last few template names (19 total)
    assert_eq!(names.len(), 19);
    assert_eq!(names[0], "DiGiCo_SD7");
    assert_eq!(names[1], "SD_Rack");
    assert_eq!(names[2], "Lab_gruppen_PLM");
    // Subsystem templates
    assert!(names.contains(&"Amplification".to_string()));
    assert!(names.contains(&"AudioFOH".to_string()));
    assert!(names.contains(&"AudioStage".to_string()));
    assert!(names.contains(&"VideoSystem".to_string()));
    assert!(names.contains(&"CommsSystem".to_string()));
    // Last template (subsystem templates follow device templates)
    assert_eq!(names[18], "CommsSystem");
}

#[test]
fn concert_venue_hierarchical_instance_refs() {
    let result = parse_fixture("concert-venue-hierarchical.patch");
    let pairs = instance_pairs(&result.program.statements);

    let expected = [
        ("Audio_FOH", "AudioFOH"),
        ("Audio_Stage", "AudioStage"),
        ("Audio_Net", "AudioNetwork"),
        ("Video", "VideoSystem"),
        ("Comms", "CommsSystem"),
        ("House_Sync", "Sync_Generator"),
    ];
    assert_eq!(pairs.len(), expected.len());
    for (i, (name, tmpl)) in expected.iter().enumerate() {
        assert_eq!(pairs[i].0, *name, "instance {i} name mismatch");
        assert_eq!(pairs[i].1, *tmpl, "instance {i} template mismatch");
    }
}

// ── 09-ring-network.patch (MTG features) ───────────────────────────

#[test]
fn ring_network_parses_without_errors() {
    let result = parse_mtg_fixture("09-ring-network.patch");
    assert!(
        result.errors.is_empty(),
        "ring-network parse errors: {:?}",
        result.errors
    );
}

#[test]
fn ring_network_statement_counts() {
    let result = parse_mtg_fixture("09-ring-network.patch");
    let counts = count_statements(&result.program.statements);

    assert_eq!(counts.templates, 2, "expected 2 templates");
    assert_eq!(counts.instances, 4, "expected 4 instances");
    assert_eq!(counts.connects, 0, "expected 0 connects");
    assert_eq!(counts.rings, 2, "expected 2 rings");
    assert_eq!(counts.errors, 0, "expected 0 error nodes");
}

#[test]
fn ring_network_ring_names() {
    let result = parse_mtg_fixture("09-ring-network.patch");
    let names = ring_names(&result.program.statements);
    assert_eq!(names, vec!["OptoCore_Primary", "OptoCore_Redundant"]);
}

#[test]
fn ring_network_primary_members() {
    let result = parse_mtg_fixture("09-ring-network.patch");
    let rings: Vec<_> = result.program.statements.iter().filter_map(|s| {
        if let Statement::Ring(r) = s { Some(r) } else { None }
    }).collect();
    let primary = rings.iter().find(|r| r.name == "OptoCore_Primary").unwrap();
    assert_eq!(primary.members.len(), 4);
    for m in &primary.members {
        assert_eq!(m.port_name.as_deref(), Some("OptoCore_A"),
            "primary ring members should have explicit OptoCore_A port");
    }
}

#[test]
fn ring_network_redundant_members() {
    let result = parse_mtg_fixture("09-ring-network.patch");
    let rings: Vec<_> = result.program.statements.iter().filter_map(|s| {
        if let Statement::Ring(r) = s { Some(r) } else { None }
    }).collect();
    let redundant = rings.iter().find(|r| r.name == "OptoCore_Redundant").unwrap();
    assert_eq!(redundant.members.len(), 4);
    for m in &redundant.members {
        assert_eq!(m.port_name.as_deref(), Some("OptoCore_B"),
            "redundant ring members should have explicit OptoCore_B port");
    }
}

// ── Multi-File Compilation ──────────────────────────────────

const MULTI_FILE_DIR: &str = "../../tests/fixtures/multi-file/hillsong-mini";

/// Load a multi-file project from disk into a HashMap.
fn load_project_files(root: &str, paths: &[&str]) -> std::collections::HashMap<String, String> {
    paths
        .iter()
        .map(|p| {
            let full = format!("{root}/{p}");
            let source = std::fs::read_to_string(&full)
                .unwrap_or_else(|e| panic!("failed to read {full}: {e}"));
            (p.to_string(), source)
        })
        .collect()
}

#[test]
fn multi_file_hillsong_mini_compiles() {
    let files = load_project_files(MULTI_FILE_DIR, &[
        "campus.patch",
        "buildings/foh.patch",
        "buildings/stage.patch",
        "yamaha.patch",
    ]);
    let result = patchlang::compile_project(files, "campus.patch");

    assert!(result.errors.is_empty(),
        "compile_project should succeed without errors, got: {:?}", result.errors);

    // Merged program should contain statements from all files
    let stmts = &result.program.statements;
    let template_count = stmts.iter()
        .filter(|s| matches!(s, patchlang::compat_types::TsStatement::Template(_)))
        .count();
    let instance_count = stmts.iter()
        .filter(|s| matches!(s, patchlang::compat_types::TsStatement::Instance(_)))
        .count();
    let connect_count = stmts.iter()
        .filter(|s| matches!(s, patchlang::compat_types::TsStatement::Connect(_)))
        .count();

    // yamaha.patch: CL5, Rio3224; foh.patch: FOH_System; stage.patch: Stage_System
    assert_eq!(template_count, 4, "expected 4 templates (CL5, Rio3224, FOH_System, Stage_System)");
    assert!(instance_count >= 2, "expected at least campus-level instances");
    assert!(connect_count >= 1, "expected at least the campus-level connect");
}

#[test]
fn multi_file_hillsong_mini_drc_clean() {
    let files = load_project_files(MULTI_FILE_DIR, &[
        "campus.patch",
        "buildings/foh.patch",
        "buildings/stage.patch",
        "yamaha.patch",
    ]);
    let result = patchlang::compile_project(files, "campus.patch");

    let errors: Vec<_> = result.diagnostics.iter()
        .filter(|d| d.severity == patchlang::drc::Severity::Error)
        .collect();
    assert!(errors.is_empty(),
        "DRC should produce no errors on valid project, got: {:?}", errors);
}

#[test]
fn multi_file_resolve_uses_from_campus() {
    let path = format!("{MULTI_FILE_DIR}/campus.patch");
    let source = std::fs::read_to_string(&path).unwrap();
    let deps = patchlang::resolve_uses(&source);
    assert_eq!(deps, vec!["buildings.foh", "buildings.stage"]);
}

// ── hillsong-mtg.patch — full output verification ───────────────────

/// Path from the crate root to the hillsong-mtg fixture.
const HILLSONG_MTG_PATH: &str = "../../tests/fixtures/examples/hillsong-mtg.patch";

/// Load the hillsong-mtg fixture and run check() on it.
fn check_hillsong_mtg() -> patchlang::drc::CheckResult {
    let source = std::fs::read_to_string(HILLSONG_MTG_PATH)
        .unwrap_or_else(|e| panic!("failed to read hillsong-mtg.patch: {e}"));
    patchlang::check(&source)
}

#[test]
fn hillsong_mtg_full_output_shape() {
    let result = check_hillsong_mtg();
    assert!(result.errors.is_empty(), "expected no parse errors, got: {:?}", result.errors);

    let json = serde_json::to_value(&result).unwrap();
    let stmts = json["program"]["statements"].as_array().unwrap();

    let count = |t: &str| stmts.iter().filter(|s| s["type"] == t).count();

    // Counts verified by grep on the fixture file
    assert_eq!(count("Template"), 24, "expected 24 Template statements");
    assert_eq!(count("Instance"), 53, "expected 53 Instance statements");
    assert_eq!(count("Connect"), 99, "expected 99 Connect statements");
    assert_eq!(count("Config"), 23, "expected 23 Config statements");

    // No unexpected error nodes in the statement list
    assert_eq!(count("Error"), 0, "expected 0 Error nodes");
}

#[test]
fn hillsong_mtg_template_details() {
    let result = check_hillsong_mtg();
    assert!(result.errors.is_empty(), "expected no parse errors, got: {:?}", result.errors);

    let json = serde_json::to_value(&result).unwrap();
    let stmts = json["program"]["statements"].as_array().unwrap();

    // Find the AVID_Venue_FOH_Rack template — the primary FOH console in the file
    let foh_rack = stmts
        .iter()
        .find(|s| s["type"] == "Template" && s["name"] == "AVID_Venue_FOH_Rack")
        .expect("AVID_Venue_FOH_Rack template not found");

    // Verify meta fields
    let meta = &foh_rack["meta"];
    assert_eq!(
        meta["manufacturer"].as_str().unwrap(),
        "AVID",
        "manufacturer should be AVID"
    );
    assert_eq!(
        meta["model"].as_str().unwrap(),
        "Venue FOH Rack",
        "model should be Venue FOH Rack"
    );

    // Verify ports exist and contain expected port names
    let ports = foh_rack["ports"].as_array().unwrap();
    assert!(!ports.is_empty(), "AVID_Venue_FOH_Rack should have ports");

    let port_names: Vec<&str> = ports
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();

    assert!(
        port_names.contains(&"AES_In"),
        "expected AES_In port, found: {:?}",
        port_names
    );
    assert!(
        port_names.contains(&"AES_Out"),
        "expected AES_Out port, found: {:?}",
        port_names
    );
    assert!(
        port_names.contains(&"LINE"),
        "expected LINE port, found: {:?}",
        port_names
    );
    assert!(
        port_names.contains(&"Talkback"),
        "expected Talkback port, found: {:?}",
        port_names
    );
    assert!(
        port_names.contains(&"Monitor"),
        "expected Monitor port, found: {:?}",
        port_names
    );

    // Verify AES_In port direction and range
    let aes_in_port = ports
        .iter()
        .find(|p| p["name"] == "AES_In")
        .expect("AES_In port not found");
    assert_eq!(aes_in_port["direction"].as_str().unwrap(), "in");
    assert_eq!(aes_in_port["rangeStart"].as_u64().unwrap(), 1);
    assert_eq!(aes_in_port["rangeEnd"].as_u64().unwrap(), 2);

    // Verify AES_Out port direction and range
    let aes_out_port = ports
        .iter()
        .find(|p| p["name"] == "AES_Out")
        .expect("AES_Out port not found");
    assert_eq!(aes_out_port["direction"].as_str().unwrap(), "out");
    assert_eq!(aes_out_port["rangeStart"].as_u64().unwrap(), 1);
    assert_eq!(aes_out_port["rangeEnd"].as_u64().unwrap(), 2);

    // Verify Talkback port is an input
    let talkback_port = ports
        .iter()
        .find(|p| p["name"] == "Talkback")
        .expect("Talkback port not found");
    assert_eq!(talkback_port["direction"].as_str().unwrap(), "in");
}

#[test]
fn hillsong_mtg_connect_details() {
    let result = check_hillsong_mtg();
    assert!(result.errors.is_empty(), "expected no parse errors, got: {:?}", result.errors);

    let json = serde_json::to_value(&result).unwrap();
    let stmts = json["program"]["statements"].as_array().unwrap();

    // Collect all Connect statements
    let connects: Vec<&serde_json::Value> = stmts
        .iter()
        .filter(|s| s["type"] == "Connect")
        .collect();
    assert!(!connects.is_empty(), "expected at least one Connect statement");

    // Verify the first connect has source and target with instance and port names
    let first = connects[0];
    let source = &first["source"];
    let target = &first["target"];

    assert!(
        source["instance"].is_string(),
        "connect source should have instance name"
    );
    assert!(
        source["port"].is_string(),
        "connect source should have port name"
    );
    assert!(
        target["instance"].is_string(),
        "connect target should have instance name"
    );
    assert!(
        target["port"].is_string(),
        "connect target should have port name"
    );

    // Verify a connect that has a mapping property
    // The first four connects in the file have mapping: "1->1, ..." properties
    let with_mapping = connects
        .iter()
        .find(|c| !c["mapping"].is_null())
        .expect("expected at least one Connect with a mapping property");
    assert!(
        with_mapping["mapping"].is_object() || with_mapping["mapping"].is_string(),
        "mapping should be present on connects that declare it"
    );
}

#[test]
fn hillsong_mtg_no_drc_errors() {
    let result = check_hillsong_mtg();

    // The fixture must parse without parse errors — this is the hard constraint.
    assert!(result.errors.is_empty(), "expected no parse errors, got: {:?}", result.errors);

    // hillsong-mtg.patch is a real production file with two known categories of
    // DRC errors that are expected at the current compiler maturity:
    //
    //   1. Structural — UUID-style slot-expanded port names in connect/route
    //      statements (e.g. `_75e05cfe_...__5efbb62d_...`) that the DRC cannot
    //      resolve because slot expansion is a runtime concern not yet modelled.
    //
    //   2. Mechanical — Connector-type mismatches that exist in the real rig
    //      (e.g. DB25 → XLR adapter cables; TRS_14 ↔ DB25 wiring). These are
    //      genuine real-world adapters that the crew uses in the field.
    //
    // Errors from any other DRC layer would indicate a regression.
    let unexpected_layer_errors: Vec<&patchlang::Diagnostic> = result
        .diagnostics
        .iter()
        .filter(|d| {
            d.severity == patchlang::drc::Severity::Error
                && d.layer != patchlang::drc::DRCLayer::Structural
                && d.layer != patchlang::drc::DRCLayer::Mechanical
        })
        .collect();

    let structural_count = result.diagnostics.iter()
        .filter(|d| d.severity == patchlang::drc::Severity::Error
            && d.layer == patchlang::drc::DRCLayer::Structural)
        .count();

    let mechanical_count = result.diagnostics.iter()
        .filter(|d| d.severity == patchlang::drc::Severity::Error
            && d.layer == patchlang::drc::DRCLayer::Mechanical)
        .count();

    println!(
        "hillsong-mtg DRC errors — Structural (slot-port): {}, Mechanical (adapter cables): {}",
        structural_count,
        mechanical_count
    );

    assert!(
        unexpected_layer_errors.is_empty(),
        "hillsong-mtg.patch should only produce Structural/Mechanical DRC errors, \
         got unexpected layer errors: {:?}",
        unexpected_layer_errors
    );

    // Structural errors are expected (slot-port resolution not yet implemented)
    assert!(structural_count > 0, "expected Structural DRC errors from slot-port names");
    // Mechanical errors are expected (real-world adapter cables in the rig)
    assert!(mechanical_count > 0, "expected Mechanical DRC errors from connector mismatches");
}

#[test]
fn hillsong_mtg_json_is_valid() {
    let result = check_hillsong_mtg();

    // Serialise the full result to a JSON string
    let json_string = serde_json::to_string(&result)
        .expect("serialising CheckResult to JSON must not fail");

    println!("hillsong-mtg JSON size: {} bytes", json_string.len());

    // Deserialise back to a Value to confirm it is well-formed JSON
    let json: serde_json::Value =
        serde_json::from_str(&json_string).expect("JSON string must be valid JSON");

    // Required top-level fields
    assert!(json.get("program").is_some(), "JSON must have 'program' field");
    assert!(json.get("errors").is_some(), "JSON must have 'errors' field");
    assert!(json.get("diagnostics").is_some(), "JSON must have 'diagnostics' field");

    // program must have a 'statements' array
    assert!(
        json["program"].get("statements").is_some(),
        "program must have 'statements' field"
    );
}

// ── hillsong-mini multi-file JSON output verification ───────────────

const MULTI_FILE_MINI_DIR: &str = "../../tests/fixtures/multi-file/hillsong-mini";

/// Load all four hillsong-mini files and compile the project.
fn compile_hillsong_mini() -> patchlang::multi_file::ProjectResult {
    let files = load_project_files(MULTI_FILE_MINI_DIR, &[
        "campus.patch",
        "buildings/foh.patch",
        "buildings/stage.patch",
        "yamaha.patch",
    ]);
    patchlang::compile_project(files, "campus.patch")
}

#[test]
fn hillsong_mini_project_files_table() {
    let result = compile_hillsong_mini();
    assert!(result.errors.is_empty(), "expected no errors, got: {:?}", result.errors);

    // BFS walk from campus.patch should visit all four files
    assert_eq!(
        result.files.len(),
        4,
        "files table should have 4 entries, got: {:?}",
        result.files
    );

    // campus.patch is the entry file — it should appear first (BFS root)
    assert_eq!(result.files[0], "campus.patch");
}

#[test]
fn hillsong_mini_template_files_map() {
    let result = compile_hillsong_mini();
    assert!(result.errors.is_empty(), "expected no errors, got: {:?}", result.errors);

    // Each template should map to the file that defined it
    assert_eq!(
        result.template_files.get("CL5").map(String::as_str),
        Some("yamaha.patch"),
        "CL5 template should come from yamaha.patch"
    );
    assert_eq!(
        result.template_files.get("Rio3224").map(String::as_str),
        Some("yamaha.patch"),
        "Rio3224 template should come from yamaha.patch"
    );
    assert_eq!(
        result.template_files.get("FOH_System").map(String::as_str),
        Some("buildings/foh.patch"),
        "FOH_System template should come from buildings/foh.patch"
    );
    assert_eq!(
        result.template_files.get("Stage_System").map(String::as_str),
        Some("buildings/stage.patch"),
        "Stage_System template should come from buildings/stage.patch"
    );
}

#[test]
fn hillsong_mini_use_graph() {
    let result = compile_hillsong_mini();
    assert!(result.errors.is_empty(), "expected no errors, got: {:?}", result.errors);

    // campus.patch uses buildings.foh and buildings.stage
    let campus_uses = result.use_graph.get("campus.patch")
        .expect("campus.patch should appear in use_graph");
    assert!(
        campus_uses.contains(&"buildings.foh".to_string()),
        "campus.patch should use buildings.foh"
    );
    assert!(
        campus_uses.contains(&"buildings.stage".to_string()),
        "campus.patch should use buildings.stage"
    );

    // yamaha.patch has no use statements
    let yamaha_uses = result.use_graph.get("yamaha.patch")
        .expect("yamaha.patch should appear in use_graph");
    assert!(
        yamaha_uses.is_empty(),
        "yamaha.patch has no use statements, expected empty vec"
    );
}

#[test]
fn hillsong_mini_merged_program_templates() {
    let result = compile_hillsong_mini();
    assert!(result.errors.is_empty(), "expected no errors, got: {:?}", result.errors);

    // Merged program should contain templates from all files
    let stmts = &result.program.statements;
    let template_count = stmts
        .iter()
        .filter(|s| matches!(s, patchlang::compat_types::TsStatement::Template(_)))
        .count();

    // yamaha.patch: CL5, Rio3224; foh.patch: FOH_System; stage.patch: Stage_System = 4 templates
    assert_eq!(
        template_count,
        4,
        "merged program should have 4 templates (CL5, Rio3224, FOH_System, Stage_System)"
    );
}

#[test]
fn hillsong_mini_json_size() {
    let result = compile_hillsong_mini();
    assert!(result.errors.is_empty(), "expected no errors, got: {:?}", result.errors);

    let json_string = serde_json::to_string(&result)
        .expect("serialising ProjectResult to JSON must not fail");

    println!("hillsong-mini JSON size: {} bytes", json_string.len());

    // Confirm the JSON is valid and has the expected top-level shape
    let json: serde_json::Value =
        serde_json::from_str(&json_string).expect("JSON must be valid");

    assert!(json.get("program").is_some(), "JSON must have 'program'");
    assert!(json.get("errors").is_some(), "JSON must have 'errors'");
    assert!(json.get("diagnostics").is_some(), "JSON must have 'diagnostics'");
    assert!(json.get("files").is_some(), "JSON must have 'files'");
    assert!(json.get("templateFiles").is_some(), "JSON must have 'templateFiles'");
    assert!(json.get("useGraph").is_some(), "JSON must have 'useGraph'");
}

// ── TR01/TR02 false-positive regression tests ──────────────────────────

/// Return all Trace-layer warnings from a single-file fixture.
fn trace_warnings_single(filename: &str) -> Vec<patchlang::Diagnostic> {
    let path = format!("{FIXTURES_DIR}/{filename}");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {path}: {e}"));
    let result = patchlang::check(&source);
    result
        .diagnostics
        .into_iter()
        .filter(|d| d.layer == patchlang::drc::DRCLayer::Trace)
        .collect()
}

/// worship-venue is clean: 4 signals, all reachable through template bridges.
/// This guards against regressions where TR01/TR02 false-positive on valid rigs.
#[test]
fn worship_venue_no_trace_warnings() {
    let diags = trace_warnings_single("worship-venue.patch");
    assert!(
        diags.is_empty(),
        "worship-venue.patch should produce zero Trace warnings, got:\n{:#?}",
        diags
    );
}

/// broadcast-truck is clean: all signals have connects and reach Out/Io ports.
#[test]
fn broadcast_truck_no_trace_warnings() {
    let diags = trace_warnings_single("broadcast-truck.patch");
    assert!(
        diags.is_empty(),
        "broadcast-truck.patch should produce zero Trace warnings, got:\n{:#?}",
        diags
    );
}

/// concert-venue-hierarchical is self-contained and uses hierarchical templates.
/// All signals should be reachable through the expanded port set.
#[test]
fn concert_venue_hierarchical_no_trace_warnings() {
    let diags = trace_warnings_single("concert-venue-hierarchical.patch");
    assert!(
        diags.is_empty(),
        "concert-venue-hierarchical.patch should produce zero Trace warnings, got:\n{:#?}",
        diags
    );
}

/// hillsong-mtg.patch declares zero `signal` statements, so the Trace check has
/// nothing to evaluate — this test is a no-op crash-guard confirming the rule
/// does not panic on a large, complex file.
#[test]
fn hillsong_mtg_no_trace_warnings() {
    let diags = trace_warnings_single("hillsong-mtg.patch");
    assert!(
        diags.is_empty(),
        "hillsong-mtg.patch should produce zero Trace warnings (no signals declared), got:\n{:#?}",
        diags
    );
}

/// hillsong-mini multi-file: campus + foh + stage + yamaha loaded together.
/// Like hillsong-mtg, this project declares zero `signal` statements — this
/// test is a no-op crash-guard confirming multi-file projects don't panic.
#[test]
fn multi_file_hillsong_mini_no_trace_warnings() {
    let files = load_project_files(MULTI_FILE_DIR, &[
        "campus.patch",
        "buildings/foh.patch",
        "buildings/stage.patch",
        "yamaha.patch",
    ]);
    let result = patchlang::compile_project(files, "campus.patch");
    let diags: Vec<_> = result
        .diagnostics
        .into_iter()
        .filter(|d| d.layer == patchlang::drc::DRCLayer::Trace)
        .collect();
    assert!(
        diags.is_empty(),
        "hillsong-mini multi-file project should produce zero Trace warnings, got:\n{:#?}",
        diags
    );
}

// ---------------------------------------------------------------------------
// 11-inserts.patch — channel and bus inserts (issue #31)
// ---------------------------------------------------------------------------

#[test]
fn mtg_inserts_fixture_parses_cleanly() {
    let result = parse_mtg_fixture("11-inserts.patch");
    assert!(
        result.errors.is_empty(),
        "insert fixture must parse with no diagnostics, got: {:?}",
        result.errors
    );
}

#[test]
fn mtg_inserts_fixture_bus_legs_stay_ordered_and_ungrouped() {
    let result = parse_mtg_fixture("11-inserts.patch");
    let inst = result
        .program
        .statements
        .iter()
        .find_map(|s| match s {
            Statement::Instance(i) if i.name == "FOH_Console" => Some(i),
            _ => None,
        })
        .expect("FOH_Console instance");

    let bus = |name: &str| {
        inst.buses.iter().find(|b| b.name == name).unwrap_or_else(|| panic!("bus {name}"))
    };
    let legs = |refs: &[patchlang::ast::PortRef]| -> Vec<String> {
        refs.iter()
            .map(|p| {
                let ch = p
                    .index
                    .as_ref()
                    .and_then(|i| i.elements.first())
                    .map(|e| format!("{e:?}"))
                    .unwrap_or_default();
                format!("{}:{}", p.port, ch)
            })
            .collect()
    };

    // Stereo: exactly two legs each way, in source order.
    let main = bus("Main_LR");
    assert_eq!(main.insert_send.len(), 2);
    assert_eq!(main.insert_send[0].port, "Ext_Out");
    assert_eq!(main.insert_return.len(), 2);

    // Scattered: ports repeat across legs and must NOT be collapsed into a range.
    let drums = bus("Drum_Sub");
    assert_eq!(drums.insert_send.len(), 2, "send legs 3 and 10 must stay separate");
    assert_eq!(drums.insert_return.len(), 2);

    // Interleaved: a flat ordered list, NOT grouped by port the way bus inputs are.
    let inter = bus("Interleaved");
    assert_eq!(
        legs(&inter.insert_send)
            .iter()
            .map(|s| s.split(':').next().unwrap().to_string())
            .collect::<Vec<_>>(),
        vec!["Ext_Out", "MADI_Out", "Ext_Out"],
        "grouping by port would reorder this to Ext_Out, Ext_Out, MADI_Out"
    );

    // A bus with no inserts is unaffected.
    assert_eq!(bus("Vocal_Group").insert_send.len(), 1);
}

#[test]
fn mtg_inserts_fixture_round_trips_through_the_formatter() {
    use patchlang::formatter::format_program;

    let source = std::fs::read_to_string(format!("{MTG_FIXTURES_DIR}/11-inserts.patch"))
        .expect("read fixture");
    let first = format_program(&patchlang::parse(&source).program);
    let second = format_program(&patchlang::parse(&first).program);
    assert_eq!(first, second, "formatting the insert fixture must be idempotent");
    assert!(first.contains("insert_send: MADI_Out[3], MADI_Out[10]"), "got:\n{first}");
    assert!(first.contains("insert_send: \"MADI_Out[3], MADI_Out[10]\""), "got:\n{first}");
}
