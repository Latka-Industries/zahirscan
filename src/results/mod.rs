//! Result structures for template mining and parsing

pub mod core;
pub mod metadata;
pub mod writing;

/// Trait for metadata types that can create a minimal fallback
pub trait MinimalFallback {
    /// Create minimal fallback metadata when extraction fails
    /// Only sets the file size (stream_size), all other fields are None/0
    fn minimal_fallback(file_size_bytes: usize) -> Self;
}

// Re-export all public types for convenience
pub use core::{FileMetadata, MiningResult, Output, OutputMode, Template};
pub use metadata::{AudioMetadata, ImageMetadata, VideoMetadata};
pub use writing::{CompressionStats, PunctuationMetrics, SVOAnalysis, WritingFootprint};

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
/// Assumes the type has a `stream_size: Option<usize>` field and implements Default
#[macro_export]
macro_rules! impl_minimal_fallback {
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
