#[cfg(test)]
mod temporal {
    use crate::builder::LibraryContext;
    use crate::drc::{self, DRCLayer, Severity};
    use crate::parser::parse;

    fn check(source: &str) -> Vec<crate::drc::Diagnostic> {
        drc::run_all(&parse(source).program, &LibraryContext::empty())
    }

    const HDR: &str = "
        template T48 { ports { Out: out(etherCON) [Dante, clk_48kHz] In: in(etherCON) [Dante, clk_48kHz] } }
        template T96 { ports { Out: out(etherCON) [Dante, clk_96kHz] In: in(etherCON) [Dante, clk_96kHz] } }
        instance A is T48
        instance B is T96
    ";

    #[test]
    fn t01_48khz_to_96khz_is_warning() {
        let src = format!("{HDR}\nconnect A.Out -> B.In");
        let diags = check(&src);
        assert!(diags.iter().any(|d| {
            d.layer == DRCLayer::Temporal
                && d.severity == Severity::Warning
                && d.message.contains("clk_48kHz")
                && d.message.contains("clk_96kHz")
        }));
    }

    #[test]
    fn same_clock_no_diagnostic() {
        let src = format!("{HDR}\nconnect A.Out -> A.In");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Temporal));
    }

    #[test]
    fn suppress_temporal_skips_check() {
        let src = format!("{HDR}\nconnect A.Out -> B.In {{ @suppress(temporal) }}");
        let diags = check(&src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Temporal));
    }

    #[test]
    fn no_clock_tag_skipped() {
        let src = "template A { ports { Out: out(etherCON) [Dante] } }
                   template B { ports { In: in(etherCON) [Dante] } }
                   instance X is A  instance Y is B  connect X.Out -> Y.In";
        let diags = check(src);
        assert!(!diags.iter().any(|d| d.layer == DRCLayer::Temporal));
    }
}

#[cfg(test)]
mod flow {
    use crate::builder::LibraryContext;
    use crate::drc::{self, DRCLayer, Severity};
    use crate::parser::parse;

    fn check(source: &str) -> Vec<crate::drc::Diagnostic> {
        let result = parse(source);
        drc::run_all(&result.program, &LibraryContext::empty())
    }

    fn flow_diags(source: &str) -> Vec<crate::drc::Diagnostic> {
        check(source)
            .into_iter()
            .filter(|d| d.layer == DRCLayer::Flow)
            .collect()
    }

    // --- F01: Flow slot exhaustion ---

    #[test]
    fn f01_ultimo_3_streams_exceeds_limit() {
        let diags = flow_diags(r#"
            template Dev { meta { dante_chipset: "Ultimo" } ports { Out[1..4]: out(etherCON) [Dante] } }
            instance D is Dev
            stream S1 { source: D.Out channels: 2 protocol: "Dante" }
            stream S2 { source: D.Out channels: 2 protocol: "Dante" }
            stream S3 { source: D.Out channels: 2 protocol: "Dante" }
        "#);
        assert!(diags.iter().any(|d| {
            d.severity == Severity::Warning
                && d.message.contains("3 streams")
                && d.message.contains("Ultimo")
                && d.message.contains("2 flow slots")
        }), "expected F01 warning for Ultimo with 3 streams: {:?}", diags);
    }

    #[test]
    fn f01_brooklyn_3_streams_no_warning() {
        let diags = flow_diags(r#"
            template Dev { meta { dante_chipset: "Brooklyn_II" } ports { Out[1..4]: out(etherCON) [Dante] } }
            instance D is Dev
            stream S1 { source: D.Out channels: 2 protocol: "Dante" }
            stream S2 { source: D.Out channels: 2 protocol: "Dante" }
            stream S3 { source: D.Out channels: 2 protocol: "Dante" }
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("flow slots")),
            "Brooklyn_II with 3 streams should not warn: {:?}", diags);
    }

    #[test]
    fn f01_no_chipset_no_warning() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..4]: out(etherCON) [Dante] } }
            instance D is Dev
            stream S1 { source: D.Out channels: 2 protocol: "Dante" }
            stream S2 { source: D.Out channels: 2 protocol: "Dante" }
            stream S3 { source: D.Out channels: 2 protocol: "Dante" }
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("flow slots")),
            "no chipset should not warn: {:?}", diags);
    }

    // --- F02: AES67 stream channel limit ---

    #[test]
    fn f02_aes67_16_channels_emits_info() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..16]: out(etherCON) [Dante] } }
            instance D is Dev
            stream BigStream { source: D.Out channels: 16 protocol: "AES67" }
        "#);
        assert!(diags.iter().any(|d| {
            d.severity == Severity::Info
                && d.message.contains("8 channels per flow")
                && d.message.contains("16 channels")
        }), "expected F02 info for 16-channel AES67: {:?}", diags);
    }

    #[test]
    fn f02_aes67_8_channels_no_warning() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..8]: out(etherCON) [Dante] } }
            instance D is Dev
            stream NormalStream { source: D.Out channels: 8 protocol: "AES67" }
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("8 channels per flow")),
            "8-channel AES67 should not warn: {:?}", diags);
    }

    #[test]
    fn f02_non_aes67_16_channels_no_warning() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..16]: out(etherCON) [Dante] } }
            instance D is Dev
            stream BigDante { source: D.Out channels: 16 protocol: "Dante" }
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("8 channels per flow")),
            "non-AES67 16-channel should not warn: {:?}", diags);
    }

    #[test]
    fn f02_aes67_string_channels_emits_info() {
        // canvas_emit writes `channels` via kv_str, so the property arrives as a
        // string. F02 read only KvValue::Num and therefore never fired on any
        // canvas-emitted file.
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..16]: out(etherCON) [Dante] } }
            instance D is Dev
            stream BigStream { source: D.Out channels: "16" protocol: "AES67" }
        "#);
        assert!(diags.iter().any(|d| {
            d.severity == Severity::Info
                && d.message.contains("8 channels per flow")
                && d.message.contains("16 channels")
        }), "expected F02 info for string-valued 16-channel AES67: {:?}", diags);
    }

    #[test]
    fn f02_aes67_unparseable_string_channels_is_ignored() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..16]: out(etherCON) [Dante] } }
            instance D is Dev
            stream Odd { source: D.Out channels: "many" protocol: "AES67" }
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("8 channels per flow")),
            "unparseable channels should not trip F02: {:?}", diags);
    }

    // --- F03: Multicast prefix mismatch ---

    #[test]
    fn f03_mismatched_prefix_emits_error() {
        let diags = flow_diags(r#"
            template T { ports { Out: out(etherCON) [Dante] In: in(etherCON) [Dante] } }
            instance A is T { aes67_mode: true multicast_prefix: 71 }
            instance B is T { aes67_mode: true multicast_prefix: 72 }
            connect A.Out -> B.In
        "#);
        assert!(diags.iter().any(|d| {
            d.severity == Severity::Error
                && d.message.contains("Multicast prefix mismatch")
                && d.message.contains("71")
                && d.message.contains("72")
        }), "expected F03 error for mismatched prefixes: {:?}", diags);
    }

    #[test]
    fn f03_matching_prefix_no_error() {
        let diags = flow_diags(r#"
            template T { ports { Out: out(etherCON) [Dante] In: in(etherCON) [Dante] } }
            instance A is T { aes67_mode: true multicast_prefix: 71 }
            instance B is T { aes67_mode: true multicast_prefix: 71 }
            connect A.Out -> B.In
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("Multicast prefix mismatch")),
            "matching prefixes should not error: {:?}", diags);
    }

    #[test]
    fn f03_no_aes67_mode_no_check() {
        let diags = flow_diags(r#"
            template T { ports { Out: out(etherCON) [Dante] In: in(etherCON) [Dante] } }
            instance A is T { multicast_prefix: 71 }
            instance B is T { multicast_prefix: 72 }
            connect A.Out -> B.In
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("Multicast prefix mismatch")),
            "without aes67_mode should not check prefixes: {:?}", diags);
    }
}
