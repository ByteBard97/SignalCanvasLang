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

    /// The test above cannot tell "ignored" from "read as 0" — `0 > 8` is false either
    /// way, so it survives a `.or(Some(0))` on the parse. F04 is where the difference
    /// actually bites: read as 0, an unparseable count against a real selection emits a
    /// bogus "declares 0 channels but its source selects 2".
    #[test]
    fn f04_unparseable_channels_with_a_selection_does_not_warn() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..16]: out(etherCON) [Dante] } }
            instance D is Dev
            stream Odd { source: D.Out[1, 3] channels: "many" protocol: "AES67" }
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("declares")),
            "an unparseable channels count must be ignored, never read as 0: {:?}", diags);
    }

    // --- F02: selection length wins over the declared channel count ---

    #[test]
    fn f02_counts_selection_length_not_declared_channels() {
        // `channels` under-reports; the selection is 9 wide, so F02 must fire.
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..16]: out(etherCON) [Dante] } }
            instance D is Dev
            stream Wide { source: D.Out[1..9] channels: 2 protocol: "AES67" }
        "#);
        assert!(diags.iter().any(|d| {
            d.severity == Severity::Info
                && d.message.contains("8 channels per flow")
                && d.message.contains("9 channels")
        }), "expected F02 to count the 9-channel selection: {:?}", diags);
    }

    #[test]
    fn f02_selection_within_limit_silences_an_overstated_channels() {
        // `channels` over-reports; the real selection is 4 wide, so F02 must not fire.
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..16]: out(etherCON) [Dante] } }
            instance D is Dev
            stream Narrow { source: D.Out[7, 1, 5, 3] channels: 16 protocol: "AES67" }
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("8 channels per flow")),
            "a 4-channel selection should not trip the 8-channel limit: {:?}", diags);
    }

    // --- F04: channels disagrees with the selection length ---

    #[test]
    fn f04_num_channels_mismatch_warns() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..16]: out(etherCON) [Dante] } }
            instance D is Dev
            stream S { source: D.Out[1, 3, 5, 7] channels: 2 protocol: "AES67" }
        "#);
        assert!(diags.iter().any(|d| {
            d.severity == Severity::Warning
                && d.message.contains("declares 2 channels")
                && d.message.contains("selects 4")
        }), "expected F04 warning for 2 vs 4: {:?}", diags);
    }

    #[test]
    fn f04_str_channels_mismatch_warns() {
        // canvas_emit writes `channels` as a string; F04 must read it from day one.
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..16]: out(etherCON) [Dante] } }
            instance D is Dev
            stream S { source: D.Out[1, 3, 5, 7] channels: "2" protocol: "AES67" }
        "#);
        assert!(diags.iter().any(|d| {
            d.severity == Severity::Warning
                && d.message.contains("declares 2 channels")
                && d.message.contains("selects 4")
        }), "expected F04 warning for string-valued channels: {:?}", diags);
    }

    #[test]
    fn f04_consistent_num_channels_no_warning() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..16]: out(etherCON) [Dante] } }
            instance D is Dev
            stream S { source: D.Out[1, 3, 5, 7] channels: 4 protocol: "AES67" }
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("selects")),
            "matching counts should not warn: {:?}", diags);
    }

    #[test]
    fn f04_consistent_str_channels_no_warning() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..16]: out(etherCON) [Dante] } }
            instance D is Dev
            stream S { source: D.Out[1..4] channels: "4" protocol: "AES67" }
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("selects")),
            "matching counts written as a string should not warn: {:?}", diags);
    }

    #[test]
    fn f04_no_selection_no_warning() {
        // Every stream in every existing file looks like this: a channel count and
        // no index. F04 must stay silent, or it would fire on the whole corpus.
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..8]: out(etherCON) [Dante] } }
            instance D is Dev
            stream S { source: D.Out channels: "8" protocol: "AES67" }
        "#);
        assert!(diags.is_empty(), "a plain 8-channel stream should be clean: {:?}", diags);
    }

    // --- F05: a source channel repeated at more than one position ---

    #[test]
    fn f05_repeated_channel_is_info() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..8]: out(etherCON) [Dante] } }
            instance D is Dev
            stream Dupe { source: D.Out[3, 1, 3] channels: 3 protocol: "AES67" }
        "#);
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("more than one position"))
            .collect();
        assert_eq!(hits.len(), 1, "expected exactly one F05 diagnostic: {:?}", diags);
        assert_eq!(
            hits[0].severity,
            Severity::Info,
            "F05 must be Info — position is significant, so a repeat can be deliberate \
             replication rather than a fault: {:?}",
            hits[0]
        );
        assert!(hits[0].message.contains("intended"),
            "F05 should ask whether the repeat is intended, not assert a fault: {:?}", hits[0]);
    }

    #[test]
    fn f05_non_monotonic_unique_selection_no_diagnostic() {
        // Order is user intent and is preserved; only repeats are remarked on.
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..8]: out(etherCON) [Dante] } }
            instance D is Dev
            stream Shuffled { source: D.Out[7, 1, 5, 3] channels: 4 protocol: "AES67" }
        "#);
        assert!(diags.is_empty(),
            "a unique out-of-order selection is legal and should be clean: {:?}", diags);
    }

    #[test]
    fn f05_reports_each_repeated_channel_once() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..8]: out(etherCON) [Dante] } }
            instance D is Dev
            stream Dupe { source: D.Out[2, 2, 2] channels: 3 protocol: "AES67" }
        "#);
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("more than one position"))
            .collect();
        assert_eq!(hits.len(), 1,
            "a channel repeated three times should be reported once: {:?}", diags);
    }

    // --- [auto] in a stream source ---

    #[test]
    fn auto_in_stream_source_is_info() {
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..8]: out(etherCON) [Dante] } }
            instance D is Dev
            stream A { source: D.Out[auto] channels: 8 protocol: "AES67" }
        "#);
        let hits: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("'auto'"))
            .collect();
        assert_eq!(hits.len(), 1, "expected one auto diagnostic: {:?}", diags);
        assert_eq!(hits[0].severity, Severity::Info, "{:?}", hits[0]);
    }

    #[test]
    fn auto_does_not_trip_the_count_rules() {
        // [auto] flattens to nothing, so it must be treated as "no selection"
        // by F02/F04 rather than as a zero-wide one.
        let diags = flow_diags(r#"
            template Dev { ports { Out[1..8]: out(etherCON) [Dante] } }
            instance D is Dev
            stream A { source: D.Out[auto] channels: 8 protocol: "AES67" }
        "#);
        assert!(!diags.iter().any(|d| d.message.contains("selects")),
            "[auto] must not be read as a 0-channel selection: {:?}", diags);
    }

    // --- Blast radius: the real fixtures gain no diagnostics ---

    #[test]
    fn real_fixtures_gain_no_selection_diagnostics() {
        let fixtures: [(&str, &str); 2] = [
            (
                "10-aes67-interop.patch",
                include_str!("../../../../../tests/fixtures/mtg-features/10-aes67-interop.patch"),
            ),
            (
                "hillsong-mtg.patch",
                include_str!("../../../../../tests/fixtures/examples/hillsong-mtg.patch"),
            ),
        ];

        for (name, source) in fixtures {
            let result = parse(source);
            assert!(result.is_valid(), "{name} should parse cleanly: {:?}", result.errors);
            let diags = drc::run_all(&result.program, &LibraryContext::empty());
            let new_rule_hits: Vec<_> = diags
                .iter()
                .filter(|d| {
                    d.layer == DRCLayer::Flow
                        && (d.message.contains("8 channels per flow")
                            || d.message.contains("selects")
                            || d.message.contains("more than one position")
                            || d.message.contains("'auto'"))
                })
                .collect();
            assert!(
                new_rule_hits.is_empty(),
                "{name} should gain no F02/F04/F05/auto diagnostics: {new_rule_hits:#?}"
            );
        }
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
