//! Shared helper for compat-layer conversion tests.

use crate::error::Span;

pub(super) fn span() -> Span {
    Span { start: 0, end: 0, file: None }
}

// ── KeyValue → Record tests ────────────────────────────────────────
