use crate::builder::LibraryContext;
use crate::drc::{self, DRCLayer, Severity};
use crate::parser::parse;

fn check(source: &str) -> Vec<crate::drc::Diagnostic> {
    let result = parse(source);
    drc::run_all(&result.program, &LibraryContext::empty())
}

#[test]
fn s01_instance_references_unknown_template() {
    let diags = check("instance Bad is GhostTemplate");
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Error
            && d.message.contains("GhostTemplate")
    }));
}

#[test]
fn s01_valid_instance_no_diagnostic() {
    let diags = check("template T { ports { X: out } }\ninstance Good is T");
    assert!(diags
        .iter()
        .all(|d| d.layer != DRCLayer::Structural || d.severity != Severity::Error));
}

#[test]
fn s02_slot_assignment_references_unknown_card() {
    let diags = check(
        "template T { ports { X: out } slot Bay: MyCard }\ninstance D is T { slot Bay: \"GhostCard\" }",
    );
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural && d.message.contains("GhostCard")
    }));
}

#[test]
fn s03_connect_references_unknown_port() {
    let diags = check(
        "template T { ports { A: out } }\ninstance X is T\ninstance Y is T\nconnect X.GhostPort -> Y.A",
    );
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural && d.message.contains("GhostPort")
    }));
}

#[test]
fn s03_valid_connect_no_diagnostic() {
    let diags = check(
        "template T { ports { A: out B: in } }\ninstance X is T\ninstance Y is T\nconnect X.A -> Y.B",
    );
    assert!(!diags
        .iter()
        .any(|d| d.layer == DRCLayer::Structural && d.severity == Severity::Error));
}

#[test]
fn s06_channel_index_out_of_range() {
    let diags = check(
        "template T { ports { Ch[1..4]: out } }\ninstance A is T\ninstance B is T\nconnect A.Ch[9] -> B.Ch[1]",
    );
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural && d.message.contains("[9]")
    }));
}

#[test]
fn s06_channel_in_range_no_diagnostic() {
    let diags = check(
        "template T { ports { Ch[1..4]: out In[1..4]: in } }\ninstance A is T\ninstance B is T\nconnect A.Ch[2] -> B.In[2]",
    );
    assert!(!diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Error
            && d.message.contains("out of range")
    }));
}

#[test]
fn s07_config_references_unknown_instance() {
    let diags = check("config Ghost { label Ch[1]: \"Test\" }");
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural && d.message.contains("Ghost")
    }));
}

#[test]
fn s07_config_valid_instance_no_diagnostic() {
    let diags = check(
        "template T { ports { Ch[1..4]: out } }\ninstance MyDev is T\nconfig MyDev { label Ch[1]: \"Test\" }",
    );
    assert!(!diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Error
            && d.message.contains("Config")
    }));
}

#[test]
fn s08_signal_origin_references_unknown_instance() {
    let diags = check("signal MySig { origin: GhostBox.Port }");
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural && d.message.contains("GhostBox")
    }));
}

#[test]
fn s09_signal_origin_references_unknown_port() {
    let diags = check(
        "template T { ports { A: out } }\ninstance Dev is T\nsignal MySig { origin: Dev.GhostPort }",
    );
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural && d.message.contains("GhostPort")
    }));
}

#[test]
fn s10_duplicate_instance_names() {
    let diags = check(
        "template T { ports { X: out } }\ninstance A is T\ninstance A is T",
    );
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Error
            && d.message.contains("Duplicate instance")
            && d.message.contains("'A'")
    }));
}

#[test]
fn s10_unique_instance_names_no_diagnostic() {
    let diags = check(
        "template T { ports { X: out } }\ninstance A is T\ninstance B is T",
    );
    assert!(!diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Error
            && d.message.contains("Duplicate instance")
    }));
}

#[test]
fn s11_duplicate_signal_names() {
    let diags = check("signal Foo { }\nsignal Foo { }");
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural && d.message.contains("Duplicate signal")
    }));
}

#[test]
fn s11_unique_signal_names_no_diagnostic() {
    let diags = check("signal Foo { }\nsignal Bar { }");
    assert!(!diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Error
            && d.message.contains("Duplicate signal")
    }));
}

#[test]
fn s14_vector_port_without_index_warns() {
    let diags = check(
        "template T { ports { Out[1..4]: out In[1..4]: in } }\ninstance A is T\ninstance B is T\nconnect A.Out -> B.In[1..2]",
    );
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Warning
            && d.message.contains("vector port")
            && d.message.contains("Out")
    }));
}

#[test]
fn s14_vector_port_with_index_no_warning() {
    let diags = check(
        "template T { ports { Out[1..4]: out In[1..4]: in } }\ninstance A is T\ninstance B is T\nconnect A.Out[1..2] -> B.In[1..2]",
    );
    assert!(!diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Warning
            && d.message.contains("vector port")
    }));
}

#[test]
fn s14_scalar_port_without_index_no_warning() {
    let diags = check(
        "template T { ports { Out: out In: in } }\ninstance A is T\ninstance B is T\nconnect A.Out -> B.In",
    );
    assert!(!diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Warning
            && d.message.contains("vector port")
    }));
}

#[test]
fn s14_auto_index_no_warning() {
    let diags = check(
        "template T { ports { Out[1..4]: out In[1..4]: in } }\ninstance A is T\ninstance B is T\nconnect A.Out[auto] -> B.In[1..2]",
    );
    assert!(!diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Warning
            && d.message.contains("vector port")
    }));
}

#[test]
fn s14_both_sides_warned_independently() {
    let diags = check(
        "template T { ports { Out[1..4]: out In[1..4]: in } }\ninstance A is T\ninstance B is T\nconnect A.Out -> B.In",
    );
    let s14_warnings: Vec<_> = diags.iter().filter(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Warning
            && d.message.contains("vector port")
    }).collect();
    assert_eq!(s14_warnings.len(), 2, "should warn on both source and target");
}

#[test]
fn s14_suppress_structural_silences() {
    let diags = check(
        "template T { ports { Out[1..4]: out In[1..4]: in } }\ninstance A is T\ninstance B is T\nconnect A.Out -> B.In { @suppress(structural) }",
    );
    assert!(!diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Warning
            && d.message.contains("vector port")
    }));
}

#[test]
fn s14_link_group_connection_warned() {
    let diags = check(
        "template T { ports { Out[1..4]: out In[1..4]: in } }\ninstance A is T\ninstance B is T\nlink_group G { connect A.Out -> B.In[1..2] }",
    );
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Warning
            && d.message.contains("vector port")
    }));
}

// S15 — Range size mismatch in connect

#[test]
fn s15_range_mismatch_is_error() {
    let src = "
        template T { ports { Out[1..16]: out(XLR) [Analogue] In[1..8]: in(XLR) [Analogue] } }
        instance A is T
        instance B is T
        connect A.Out[1..16] -> B.In[1..8]
    ";
    let diags = check(src);
    assert!(diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Error
            && d.message.contains("16")
            && d.message.contains("8")
    }));
}

#[test]
fn s15_matching_ranges_no_error() {
    let src = "
        template T { ports { Out[1..8]: out(XLR) [Analogue] In[1..8]: in(XLR) [Analogue] } }
        instance A is T
        instance B is T
        connect A.Out[1..8] -> B.In[1..8]
    ";
    let diags = check(src);
    assert!(!diags.iter().any(|d| {
        d.layer == DRCLayer::Structural
            && d.severity == Severity::Error
            && d.message.contains("mismatch")
    }));
}

#[test]
fn s15_suppressed_range_mismatch_no_error() {
    let src = "
        template T { ports { Out[1..32]: out(etherCON) [Dante] In[1..64]: in(etherCON) [Dante] } }
        instance A is T
        instance B is T
        connect A.Out[1..32] -> B.In[1..32] { @suppress(structural) }
    ";
    let diags = check(src);
    assert!(!diags.iter().any(|d| {
        d.layer == DRCLayer::Structural && d.message.contains("mismatch")
    }));
}

#[test]
fn s15_auto_on_one_side_no_error() {
    let src = "
        template T { ports { Out[1..16]: out(etherCON) [Dante] In[1..32]: in(etherCON) [Dante] } }
        instance A is T
        instance B is T
        connect A.Out[auto] -> B.In[1..16]
    ";
    let diags = check(src);
    assert!(!diags.iter().any(|d| {
        d.layer == DRCLayer::Structural && d.message.contains("mismatch")
    }));
}
