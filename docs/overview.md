---
layout: default
title: Overview
permalink: /overview/
---

# PatchLang Specification

**Version:** 0.2.15
**Date:** 2026-06-16
**Status:** Canonical. All frontend, backend, and compiler work must conform to this document.

PatchLang is a domain-specific language for describing signal flow in broadcast and live production environments. It defines device templates, physical instances, cable connections, logical signal mappings, and channel configuration. PatchLang files (`.patch`) are the source of truth: human-readable, git-diffable, and designed for LLM generation.

**Key principle:** PatchLang stores everything about the system. JSON stores only canvas layout. If it affects signal flow, it belongs in PatchLang.

**Audience:** Contributors, integrators, and AI coding assistants working in this repo. The "What the Emitter/Frontend Must Do" sections are implementation specs, not suggestions.

## Design Principles

1. **Human-readable first.** A broadcast engineer should be able to read a `.patch` file and understand the signal chain.
2. **LLM-friendly.** The syntax is simple enough that language models can generate valid `.patch` files from plain English descriptions.
3. **Git-diffable.** Text diffs show meaningful changes. Adding a mic input is one line, not a JSON blob.
4. **No ambiguity.** Every statement starts with a unique keyword. The grammar is LL(1).
5. **Domain-specific.** The language models broadcast concepts directly, not through generic data structures.
6. **No UUIDs in user-facing files.** Instance names are human-readable identifiers.
7. **Single validation layer.** The Rust compiler validates everything. Frontend and backend never reimplement parsing or validation.
8. **No freelancing on data formats.** Don't invent new file formats, storage mechanisms, or identifier systems outside this spec.

## File Conventions

- **Extension:** `.patch`
- **Encoding:** UTF-8
- **Line endings:** LF or CRLF (both accepted)
- **Layout sidecar:** `<name>.layout.json`, same stem as the `.patch` file
- **Project manifest:** `project.json` at the project root

## How This Document Is Organized

This spec is ordered for reading, not for reference. Start here, then:

1. **Examples**: see what PatchLang looks like before learning the rules
2. **Project Structure**: how files, folders, and hierarchy work
3. **Frontend Integration**: how the Vue canvas consumes PatchLang
4. **Compiler**: design decisions and the multi-file compilation API
5. **Backend**: database model and API endpoints
6. **Language Reference**: the full grammar (for lookup, not for reading cover to cover)
7. **Appendix**: why each decision was made (debate records)
