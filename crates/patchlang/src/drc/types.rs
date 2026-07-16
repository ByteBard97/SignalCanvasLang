use serde::Serialize;
use ts_rs::TS;

use crate::compat_types::{TsParseError, TsProgram};

/// Which validation layer produced this diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum DRCLayer {
    Structural,
    Direction,
    Mechanical,
    Electrical,
    Logical,
    Temporal,
    Ring,
    Network,
    Flow,
    Convention,
    Trace,
}

/// How severe the diagnostic is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A single design rule violation or advisory.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Diagnostic {
    pub severity: Severity,
    pub layer: DRCLayer,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub span: Option<crate::error::Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fix: Option<String>,
}

/// Output of `patchlang::check()` — parse result plus semantic diagnostics.
#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub program: TsProgram,
    pub errors: Vec<TsParseError>,
    pub diagnostics: Vec<Diagnostic>,
}
