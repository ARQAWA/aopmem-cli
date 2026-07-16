//! Output-neutral warnings shared by core commands and best-effort services.

use serde::Serialize;

pub const OBSERVABILITY_WRITE_FAILED: &str = "OBSERVABILITY_WRITE_FAILED";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutputWarning {
    pub code: &'static str,
    pub message: String,
}
