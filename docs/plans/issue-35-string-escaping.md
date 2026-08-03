# Issue #35 — escape quoted strings in the formatter

## Summary

`formatter_emit.rs` writes quoted strings by bare concatenation. A user-supplied value
containing `"` produces text that still *parses* but carries a different value —
`The "Big" Mix` comes back as `The `. Silent corruption, no diagnostic.

**This is a format change, not a local bug fix.** The lexer regex is `"[^"]*"` with no
escape support anywhere, so escaping on emit alone would make things worse: `\"` would
terminate the string at the escaped quote. Both sides move together.

## Verified before planning

- **No backslash exists in any quoted string** in `tests/fixtures/**` or
  `SignalCanvasFrontend/MTG.patch`. Introducing escape semantics therefore reinterprets
  nothing in the wild — this is the constraint that would have killed the approach.
- **Newlines already round-trip.** `model: "Lead\nVocal"` (a raw newline inside the
  quotes) formats and re-parses with **0 errors** today, because `[^"]*` happily spans
  lines. The issue assumed a newline would be "similarly destructive"; it is not. That
  changes the newline question from correctness to output hygiene.

## Decisions

**D-1: support escapes on both sides.** Lexer regex becomes `"([^"\\]|\\.)*"` with an
unescape step in the token callback; a single `emit_quoted` helper escapes on the way out.

Rejected: sanitizing or rejecting quotes at emit. Mangling the user's text is the bug, not
the fix.

**D-2: escape set is `\` `"` `\n` `\r` `\t`.** Backslash and quote are correctness.
Newline/carriage-return/tab are hygiene: they already survive, but emitting them raw makes
a `.patch` file non-line-oriented, which breaks diffing and any line-based tooling. A file
containing a raw newline in a string still parses (the regex keeps accepting it), and
re-emitting converts it to `\n` — the value is preserved either way, so this is safe.

**D-3: unknown escapes are an error, not a silent pass-through.** `\q` must produce a
diagnostic rather than being read as `q` or as `\q`. Silent reinterpretation is the exact
failure mode this issue is about.

## Every quoted-emit site

All in `crates/patchlang/src/formatter_emit.rs`. The helper must be used at **all eight**;
a helper wired to six of them is worse than none, because it looks fixed.

| Line | Site | Carries user text? |
|---|---|---|
| 566 | `emit_kv_value_inline`, `KvValue::Str` | **Yes** — every `meta` and property value in the language |
| 337 | `emit_config_label` — channel label | **Yes** — free text like `"Lead Vocal"` |
| 479 | `emit_bus_entry` — bus `label:` | **Yes** — the #35 repro |
| 508 | bus `output "..."` label | **Yes** — the D024 / #34 fallback path |
| 203 | connect `mapping:` | Grammar-shaped, but no reason to skip |
| 453 | `slot` card name, via `needs_quoting` | Possible |
| 16 | template `@version("...")` | No, but audit |
| 136 | instance `@version("...")` | No, but audit |

## Phases

### Phase 1 — lexer
- Regex to `"([^"\\]|\\.)*"`; unescape in the callback for the D-2 set.
- Unknown escape ⇒ a parse error naming the offending sequence (D-3).
- A lone trailing backslash before the closing quote must not swallow the quote.

### Phase 2 — emitter
- One `emit_quoted(out, s)` helper, private to `formatter_emit.rs`.
- Wire it into all eight sites above. Grep afterwards to prove no bare
  `push('"')` / `push_str("\"")` pair remains around an unescaped value.

### Phase 3 — tests
- **The proptest is the deliverable**, not a garnish: for arbitrary strings,
  `parse(emit(x))` returns `x` exactly. `builder_tests/property_tests.rs` already exists.
- Targeted fixtures for the asymmetric cases a naive proptest generator may not reach: a
  lone trailing backslash, a literal `\"`, a string that is *only* quotes, an empty
  string, and the issue's own `The "Big" Mix`.
- An end-to-end check through the rebuilt WASM, since that is how the corruption was
  originally demonstrated.
- **Guard against inverse bugs.** A round-trip test passes if escape and unescape are
  inverse *mistakes* just as readily as if they are inverse *correct* functions. Assert
  the emitted **text** for at least one fixture, not only the round-tripped value.
- Mutation-check everything; confirm each test fails against the unfixed tree first.

### Phase 4 — docs
- Record the decision; update SPEC.md and the language reference with the escape rules.
- **Amend D024**, which currently says "this decision does not address it" about #35 and
  notes that a `display_name` with a quote reaches the emitter via the legacy bus-output
  fallback. Confirm that path is covered and correct the note.
- Tell Reid: this is a format change and 0.3.3 is already tagged behind it.

## Explicitly out of scope

Issue #34's parse-invariant test would not catch this bug — malformed output still parses.
These are different tests of the same family and both are needed.
