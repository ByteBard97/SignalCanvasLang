---
layout: default
title: Appendix
permalink: /appendix/
---

# Appendix: Design Decision Records

The decisions in this spec were reached through structured Socratic debates with multiple AI agent perspectives, then cross-validated to consensus. This appendix preserves the rationale.

## Cards: Template with Meta vs Keyword

**Decision: No `card` keyword. Cards are templates with `kind: "card"` meta. Confidence: 88%.**

Cards and templates are structurally identical: both have `ports` and `meta`. Adding a keyword is irreversible. If we later discover we need one, we can add `card` as syntactic sugar that desugars to a template (non-breaking). Removing a keyword is breaking.

## Ring Member Syntax

**Decision: Both explicit and implicit forms accepted. Emitter always emits explicit. Confidence: 82%.**

Explicit form (`member Console.OptoCore_A`) is stable: if someone later adds a TWINLANe port to the Console template, the explicit form still resolves correctly. Implicit form (`member Console`) would break in that scenario.

## Slot/Card Compatibility

**Decision: Inverted model. Cards declare `fits`. Slots declare bay shape only. Confidence: 87%.**

If each slot listed compatible cards, every new card type would require editing every template that has that slot type. The inverted model means adding a new card never requires editing existing templates.

## ID Separator

**Decision: Double-colon `::` separator. Confidence: 85%.**

Underscore (`_`) is ambiguous because identifiers contain underscores. `::` cannot appear in PatchLang identifiers, making parsing unambiguous.

## Reserved Keywords

**Decision: Reserve `ring` and `member` only. Un-reserve `card`. Confidence: 92%.**

Reserve a keyword only when a concrete syntax design exists, the keyword name is confirmed, and an implementation plan is committed.

## Project Structure: Page Tree vs Flat Bundle

**Decision: One DB row per canvas level (`ProjectPage` model). Confidence: high.**

The frontend loads one level at a time. Per-page rows match the loading pattern (one query per level), save pattern (one row per save), and enable row-level concurrency (two users editing different rooms don't conflict).

## `project.json`: Full Manifest vs Inferred

**Decision: Thin manifest. Sub-levels inferred from `use` graph. Libraries and dependencies declared explicitly. Confidence: high.**

Listing sub-levels duplicates the `use` graph and creates sync drift. The compiler discovers sub-levels by walking `use` statements. Only things that can't be inferred (project-local libraries, external dependencies) are declared.

## Multi-File Compilation: File Map

**Decision: `compile_project(files: HashMap, entry: &str)`. No concatenation, no incremental compilation. Confidence: high.**

Concatenation loses file provenance (error messages can't report which file). Incremental compilation is YAGNI at this scale (under 1 MB total). The file map gives the compiler everything it needs to resolve `use` statements and report errors with file + line info.

## Flat Namespace

**Decision: All templates share a single namespace. Duplicate names are compile errors. Confidence: high.**

Simple and sufficient. If two files define `Splitter`, the compiler reports the conflict. Users use unique names (`FOH_Splitter`, `Broadcast_Splitter`). Scoped namespaces can be added later if needed.

## Snapshot Strategy

**Decision: Per-page version rows, not monolithic JSON blobs. Confidence: high.**

Monolithic JSON blobs are the problem PatchLang was created to solve. Snapshots use a `PageVersion` table: each page's content in its own row. Diffing compares rows. Restoring updates pages individually.
