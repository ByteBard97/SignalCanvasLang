//! Issue #35 — quoted strings must survive the emit → parse round trip.
//!
//! The formatter used to write quoted strings by bare concatenation, so a value
//! containing a quote emitted text that still *parsed* but carried a different
//! value (`The "Big" Mix` came back as `The `). These tests pin both halves: the
//! emitter escapes, the lexer unescapes, and the composition is the identity.

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::ast::{KvValue, ParamValue, Statement};
    use crate::formatter::format_program;
    use crate::parser::parse;

    /// Source exercising all nine quoted-emission sites in `formatter_emit`: template
    /// `@version` and string param default, `meta` value, `config` label, slot card
    /// name, bus label, bus output label, instance `@version`, and connect `mapping`.
    /// Each value is a placeholder the helper below overwrites with the string under test.
    const CARRIER: &str = r#"template Desk(name: "placeholder") @version("1.0") {
  meta {
    model: "placeholder"
  }
}
config Labels {
  label Desk.In[1]: "placeholder"
}
instance Mixer is Desk @version(">=1.0") {
  slot Card[1]: "placeholder"
  bus PQMM {
    label: "placeholder"
    output "placeholder"
  }
}
connect Mixer.Out[1] -> Mixer.In[1] {
  mapping: "placeholder"
}"#;

    /// The language rejects an empty bus output label, so that one site carries the
    /// value behind a fixed prefix. Escaping is still exercised; only emptiness isn't.
    const OUTPUT_PREFIX: &str = "O";

    /// Set every string-bearing site in `CARRIER` to `value`, format, re-parse, and
    /// return `(formatted_text, values_read_back)`.
    ///
    /// One helper for all sites so that a site wired to the escaping helper but not
    /// covered by a test cannot hide.
    fn roundtrip(value: &str) -> (String, Vec<String>) {
        let mut program = parse(CARRIER).program;
        assert!(
            !program.statements.is_empty(),
            "carrier source must parse into statements"
        );

        for stmt in &mut program.statements {
            match stmt {
                Statement::Template(t) => {
                    t.meta[0].value = KvValue::Str {
                        value: value.to_string(),
                    };
                    t.params[0].default_value = ParamValue::Str {
                        value: value.to_string(),
                    };
                    t.version = Some(value.to_string());
                }
                Statement::Config(c) => c.labels[0].label = value.to_string(),
                Statement::Instance(i) => {
                    i.buses[0].label = Some(value.to_string());
                    i.buses[0].outputs[0].label = format!("{OUTPUT_PREFIX}{value}");
                    i.slot_assignments[0].card_name = value.to_string();
                    i.version_constraint = Some(value.to_string());
                }
                Statement::Connect(c) => c.mapping = Some(value.to_string()),
                _ => panic!("unexpected statement in carrier source"),
            }
        }

        let formatted = format_program(&program);
        let result = parse(&formatted);
        assert!(
            result.is_valid(),
            "formatted output must parse cleanly for {value:?}: {:?}\n\nFormatted:\n{formatted}",
            result.errors
        );

        let mut read_back = Vec::new();
        for stmt in &result.program.statements {
            match stmt {
                Statement::Template(t) => {
                    match &t.meta[0].value {
                        KvValue::Str { value } => read_back.push(value.clone()),
                        other => panic!("meta value came back as {other:?}, not a string"),
                    }
                    match &t.params[0].default_value {
                        ParamValue::Str { value } => read_back.push(value.clone()),
                        other => panic!("param default came back as {other:?}, not a string"),
                    }
                    read_back.push(t.version.clone().unwrap_or_default());
                }
                Statement::Config(c) => read_back.push(c.labels[0].label.clone()),
                Statement::Connect(c) => {
                    read_back.push(c.mapping.clone().unwrap_or_default());
                }
                Statement::Instance(i) => {
                    read_back.push(i.buses[0].label.clone().unwrap_or_default());
                    let output_label = &i.buses[0].outputs[0].label;
                    read_back.push(
                        output_label
                            .strip_prefix(OUTPUT_PREFIX)
                            .unwrap_or(output_label)
                            .to_string(),
                    );
                    read_back.push(i.slot_assignments[0].card_name.clone());
                    read_back.push(i.version_constraint.clone().unwrap_or_default());
                }
                _ => panic!("unexpected statement in formatted output"),
            }
        }
        (formatted, read_back)
    }

    /// Assert `value` survives every site unchanged.
    fn assert_survives(value: &str) {
        let (formatted, read_back) = roundtrip(value);
        assert_eq!(read_back.len(), 9, "expected every string site to report back");
        for got in &read_back {
            assert_eq!(
                got, value,
                "value did not survive the round trip\n\nFormatted:\n{formatted}"
            );
        }
    }

    // ---- Targeted fixtures -------------------------------------------------
    // The asymmetric cases a random generator is unlikely to reach.

    #[test]
    fn issue_35_repro_survives() {
        assert_survives(r#"The "Big" Mix"#);
    }

    /// Round-tripping alone would pass just as happily if escape and unescape were
    /// inverse *mistakes*, so pin the emitted text itself for the issue's own case.
    #[test]
    fn issue_35_repro_emits_escaped_text() {
        let (formatted, _) = roundtrip(r#"The "Big" Mix"#);
        assert!(
            formatted.contains(r#"label: "The \"Big\" Mix""#),
            "bus label must be emitted with escaped quotes:\n{formatted}"
        );
        assert!(
            formatted.contains(r#"model: "The \"Big\" Mix""#),
            "meta value must be emitted with escaped quotes:\n{formatted}"
        );
        assert!(
            formatted.contains(r#"output "OThe \"Big\" Mix""#),
            "bus output label must be emitted with escaped quotes:\n{formatted}"
        );
        assert!(
            formatted.contains(r#"Desk.In[1]: "The \"Big\" Mix""#),
            "config label must be emitted with escaped quotes:\n{formatted}"
        );
    }

    #[test]
    fn trailing_backslash_does_not_swallow_the_closing_quote() {
        assert_survives(r"ends with a backslash\");
    }

    #[test]
    fn literal_backslash_quote_survives() {
        assert_survives(r#"\""#);
    }

    #[test]
    fn only_quotes_survives() {
        assert_survives(r#"""""#);
    }

    #[test]
    fn empty_string_survives() {
        assert_survives("");
    }

    #[test]
    fn control_characters_survive() {
        assert_survives("tab\there\nnewline\rreturn");
    }

    #[test]
    fn newline_is_emitted_as_an_escape_not_raw() {
        let (formatted, _) = roundtrip("two\nlines");
        assert!(
            formatted.contains(r#"label: "two\nlines""#),
            "a newline must be emitted as \\n so the file stays line-oriented:\n{formatted}"
        );
    }

    // ---- Lexer-side behaviour ---------------------------------------------

    /// Files written before escaping existed may hold a raw newline inside a
    /// string; those must keep parsing (they are merely re-emitted as `\n`).
    #[test]
    fn raw_newline_in_source_still_parses() {
        let src = "template Desk {\n  meta {\n    model: \"Lead\nVocal\"\n  }\n}";
        let result = parse(src);
        assert!(result.is_valid(), "raw newline must still parse: {:?}", result.errors);
        match &result.program.statements[0] {
            Statement::Template(t) => match &t.meta[0].value {
                KvValue::Str { value } => assert_eq!(value, "Lead\nVocal"),
                other => panic!("expected a string, got {other:?}"),
            },
            other => panic!("expected a template, got {other:?}"),
        }
    }

    #[test]
    fn unknown_escape_is_a_named_error() {
        let result = parse(r#"template Desk { meta { model: "a\qb" } }"#);
        assert!(!result.is_valid(), "an unknown escape must not parse silently");
        assert!(
            result.errors.iter().any(|e| e.message.contains(r"\q")),
            "the error must name the offending sequence: {:?}",
            result.errors
        );
    }

    /// An unterminated literal must say so, and blame only the opening quote.
    ///
    /// logos consumes from the opening `"` to end-of-input hunting for a close, so the
    /// error span covers the whole remainder of the file. Reported as "unexpected
    /// character" that dumped the rest of the source into the message and highlighted
    /// all of it — useless on a real file. The reachable way to hit this is a trailing
    /// backslash, which escapes the closing quote (D026's known behaviour change).
    #[test]
    fn unterminated_string_is_named_and_blames_only_the_quote() {
        let src = "template Desk { meta { model: \"trailing\\\" } }";
        let result = parse(src);
        assert!(!result.is_valid(), "an unterminated literal must not parse");
        let err = result
            .errors
            .iter()
            .find(|e| e.message.contains("unterminated string literal"))
            .unwrap_or_else(|| panic!("expected an unterminated-string error: {:?}", result.errors));
        assert_eq!(
            err.span.end - err.span.start,
            1,
            "the span must cover only the opening quote, not the rest of the file: {err:?}"
        );
        assert_eq!(&src[err.span.start..err.span.end], "\"");
        assert!(
            !err.message.contains("trailing"),
            "the message must not quote the source back: {}",
            err.message
        );
    }

    #[test]
    fn every_supported_escape_decodes() {
        let result = parse(r#"template Desk { meta { model: "\\ \" \n \r \t" } }"#);
        assert!(result.is_valid(), "supported escapes must parse: {:?}", result.errors);
        match &result.program.statements[0] {
            Statement::Template(t) => match &t.meta[0].value {
                KvValue::Str { value } => assert_eq!(value, "\\ \" \n \r \t"),
                other => panic!("expected a string, got {other:?}"),
            },
            other => panic!("expected a template, got {other:?}"),
        }
    }

    // ---- Property test -----------------------------------------------------

    /// Alphabet biased towards the characters that break naive emission, so every
    /// generated case is a hostile one. `any::<String>()` does reach `"` and `\`
    /// eventually, but only probabilistically and diluted across the whole char
    /// range; this alphabet makes the failure immediate and deterministic.
    fn escape_hostile_char() -> impl Strategy<Value = char> {
        prop_oneof![
            Just('"'),
            Just('\\'),
            Just('n'),
            Just('\n'),
            Just('\r'),
            Just('\t'),
            Just('a'),
            Just(' '),
        ]
    }

    fn escape_hostile_string() -> impl Strategy<Value = String> {
        proptest::collection::vec(escape_hostile_char(), 0..12)
            .prop_map(|chars| chars.into_iter().collect())
    }

    proptest! {
        /// For an arbitrary string, `parse(emit(x))` yields exactly `x`.
        #[test]
        fn arbitrary_strings_survive_the_round_trip(value in escape_hostile_string()) {
            let (formatted, read_back) = roundtrip(&value);
            for got in read_back {
                prop_assert_eq!(
                    &got,
                    &value,
                    "value did not survive the round trip\n\nFormatted:\n{}",
                    formatted
                );
            }
        }
    }
}
