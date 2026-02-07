/// Macro to create a lazily-initialized static value using OnceLock
///
/// Usage:
/// ```
/// use zahirscan::cached_static;
/// use regex::Regex;
///
/// let pattern = cached_static!(PATTERN: Regex = Regex::new(r"\d+").unwrap());
/// assert!(pattern.is_match("123"));
/// ```
///
/// This expands to:
/// ```
/// use std::sync::OnceLock;
/// use regex::Regex;
///
/// static PATTERN: OnceLock<Regex> = OnceLock::new();
/// let pattern = PATTERN.get_or_init(|| Regex::new(r"\d+").unwrap());
/// ```
///
#[macro_export]
macro_rules! cached_static {
    ($name:ident: $ty:ty = $init:expr) => {{
        use std::sync::OnceLock;
        static $name: OnceLock<$ty> = OnceLock::new();
        $name.get_or_init(|| $init)
    }};
}

pub mod ffprobe_handler;
pub mod filetypes;
pub mod path_string_helper;
pub mod typecheck;

use anyhow::anyhow;

/// When reporting "no valid files" in batched mode, list at most this many failed paths; rest shown as "(and N more)".
pub const BATCHED_NO_VALID_FILES_DETAIL_CAP: usize = 5;

/// Builds an error when no valid files were found. `detail_cap` limits how many failed paths
/// are listed (e.g. 5); excess is summarized as "(and N more)". Use `None` to list all.
pub fn no_valid_files_error(
    failed: &[(String, String)],
    detail_cap: Option<usize>,
) -> anyhow::Error {
    let msg = if failed.is_empty() {
        "No valid files found. All provided paths failed to scan or do not exist".to_string()
    } else {
        let details: Vec<String> = failed
            .iter()
            .take(detail_cap.unwrap_or(failed.len()))
            .map(|(p, e)| format!("{}: {}", p, e))
            .collect();
        let more = detail_cap
            .map(|cap| failed.len().saturating_sub(cap))
            .unwrap_or(0);
        let suffix = if more > 0 {
            format!(" (and {} more)", more)
        } else {
            String::new()
        };
        format!(
            "No valid files found. {} path(s) failed: {}{}",
            failed.len(),
            details.join("; "),
            suffix
        )
    };
    anyhow!("{}", msg)
}
