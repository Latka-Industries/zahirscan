//! Result structures for template mining and parsing

pub mod core;
pub mod metadata;
pub mod writing;

pub use core::*;
pub use metadata::*;
pub use writing::*;

// ============================================================================
// Minimal Fallback Trait
// ============================================================================

/// Trait for metadata types that can create a minimal fallback
pub trait MinimalFallback {
    /// Create minimal fallback metadata when extraction fails
    /// Only sets the file size (stream_size), all other fields are None/0
    fn minimal_fallback(file_size_bytes: usize) -> Self;
}

/// Helper function to create minimal fallback metadata
/// This simplifies calls from outside the results module
pub fn create_minimal_fallback<T: MinimalFallback>(file_size_bytes: usize) -> T {
    T::minimal_fallback(file_size_bytes)
}

// ============================================================================
// Macros
// ============================================================================

/// Helper macro to conditionally serialize optional fields
/// Skips serialization if the field is None
#[macro_export]
macro_rules! serialize_optional {
    ($state:expr, $field:expr, $name:literal) => {
        if let Some(ref val) = $field {
            $state.serialize_field($name, val)?;
        }
    };
}

/// Macro to implement MinimalFallback trait for metadata types
///
/// Usage:
/// - `impl_minimal_fallback!(TypeName)` - for types with `stream_size: Option<usize>`
/// - `impl_minimal_fallback!(TypeName, field_name)` - for types with a different field name
/// - `impl_minimal_fallback!(TypeName, _)` - for types that just need `Self::default()`
#[macro_export]
macro_rules! impl_minimal_fallback {
    // Default-only case (ignores file_size_bytes) - must come first
    ($type:ty, _) => {
        impl MinimalFallback for $type {
            fn minimal_fallback(_file_size_bytes: usize) -> Self {
                Self::default()
            }
        }
    };
    // Custom field name case
    ($type:ty, $field:ident) => {
        impl MinimalFallback for $type {
            fn minimal_fallback(file_size_bytes: usize) -> Self {
                Self {
                    $field: Some(file_size_bytes),
                    ..Default::default()
                }
            }
        }
    };
    // Default case: use stream_size field
    ($type:ty) => {
        impl MinimalFallback for $type {
            fn minimal_fallback(file_size_bytes: usize) -> Self {
                Self {
                    stream_size: Some(file_size_bytes),
                    ..Default::default()
                }
            }
        }
    };
}
