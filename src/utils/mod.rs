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
