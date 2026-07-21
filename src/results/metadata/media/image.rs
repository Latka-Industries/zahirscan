//! Image metadata structures

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::results::MinimalFallback;

/// Image metadata (Mode 2 only, for image files)
///
/// Unknown dimensions stay `0` in-memory and are **omitted from JSON** (not serialized as
/// `0×0`). SVG uses root `width`/`height`/`viewBox` when absolute; percentages stay unknown.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ImageMetadata {
    /// Image width in pixels (`0` = unknown; omitted from JSON)
    pub width: usize,
    /// Image height in pixels (`0` = unknown; omitted from JSON)
    pub height: usize,
    /// Aspect ratio (width/height)
    pub aspect_ratio: Option<f64>,
    /// File size in bytes (stream size)
    pub stream_size: Option<usize>,
    /// Color type (e.g., "Rgb8", "Rgba8", "L8")
    pub color_type: Option<String>,
    /// Image format (e.g., "Jpeg", "Png", "Gif")
    pub format: Option<String>,
    /// Chroma subsampling (JPEG only, e.g., "4:2:0", "4:2:2", "4:4:4")
    pub chroma_subsampling: Option<String>,
    /// Compression info (format-specific, e.g., "JPEG quality: 85")
    pub compression: Option<String>,
    /// Bit depth (bits per sample/channel, e.g., 8, 16, 24, 32)
    pub bit_depth: Option<u32>,
}

impl Serialize for ImageMetadata {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Omit unknown dimensions (0) so SVG / failed header reads do not report 0×0 as real size.
        let mut state = serializer.serialize_struct("ImageMetadata", 9)?;
        if self.width > 0 {
            state.serialize_field("width", &self.width)?;
        }
        if self.height > 0 {
            state.serialize_field("height", &self.height)?;
        }
        crate::serialize_optional!(state, self.aspect_ratio, "aspect_ratio");
        crate::serialize_optional!(state, self.stream_size, "stream_size");
        crate::serialize_optional!(state, self.color_type, "color_type");
        crate::serialize_optional!(state, self.format, "format");
        crate::serialize_optional!(state, self.chroma_subsampling, "chroma_subsampling");
        crate::serialize_optional!(state, self.compression, "compression");
        crate::serialize_optional!(state, self.bit_depth, "bit_depth");
        state.end()
    }
}

crate::impl_minimal_fallback!(ImageMetadata);
