//! Format-specific image metadata extraction

use crate::parsers::FileType;
use crate::utils::filetypes::get_extensions_for_file_type;

use super::{bmp, gif, jpeg, png, tiff, webp};

/// Image format types for metadata extraction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    WebP,
    Bmp,
    Tiff,
}

/// Trait for extracting format-specific metadata
pub trait FormatMetadata {
    /// Extract compression information from image data
    fn extract_compression(&self, data: &[u8]) -> Option<String>;

    /// Extract chroma subsampling information (JPEG only)
    fn extract_chroma_subsampling(&self, _data: &[u8]) -> Option<String> {
        None // Most formats don't use chroma subsampling
    }

    /// Extract bit depth (bits per sample/channel)
    fn extract_bit_depth(&self, _data: &[u8]) -> Option<u32> {
        None // Default: no bit depth extraction
    }

    /// Extract color type (pixel format, e.g., "Rgb8", "Rgba8", "L8")
    fn extract_color_type(&self, _data: &[u8]) -> Option<String> {
        None // Default: no color type extraction
    }
}

impl FormatMetadata for ImageFormat {
    fn extract_compression(&self, data: &[u8]) -> Option<String> {
        match self {
            ImageFormat::Jpeg => Some(jpeg::extract_compression(data)),
            ImageFormat::Png => png::extract_compression(data),
            ImageFormat::Gif => gif::extract_compression(data),
            ImageFormat::WebP => webp::extract_compression(data),
            ImageFormat::Bmp => bmp::extract_compression(data),
            ImageFormat::Tiff => tiff::extract_compression(data),
        }
    }

    fn extract_chroma_subsampling(&self, data: &[u8]) -> Option<String> {
        match self {
            ImageFormat::Jpeg => jpeg::extract_subsampling(data),
            _ => None, // Only JPEG uses chroma subsampling
        }
    }

    fn extract_bit_depth(&self, data: &[u8]) -> Option<u32> {
        match self {
            ImageFormat::Jpeg => jpeg::extract_bit_depth(data),
            ImageFormat::Png => png::extract_bit_depth(data),
            ImageFormat::Bmp => bmp::extract_bit_depth(data),
            ImageFormat::Tiff => tiff::extract_bit_depth(data),
            // GIF and WebP are 8-bit (WebP is typically 8-bit)
            ImageFormat::Gif | ImageFormat::WebP => Some(8),
        }
    }

    fn extract_color_type(&self, data: &[u8]) -> Option<String> {
        match self {
            ImageFormat::Jpeg => jpeg::extract_color_type(data),
            ImageFormat::Png => png::extract_color_type(data),
            ImageFormat::Gif => gif::extract_color_type(data),
            ImageFormat::Bmp => bmp::extract_color_type(data),
            ImageFormat::WebP => webp::extract_color_type(data),
            ImageFormat::Tiff => Some(tiff::extract_color_type(data)),
        }
    }
}

/// Extension to `ImageFormat` mapping
/// Only includes formats that have `ImageFormat` enum variants
const IMAGE_FORMAT_MAP: &[(&str, ImageFormat)] = &[
    ("jpeg", ImageFormat::Jpeg),
    ("jpg", ImageFormat::Jpeg),
    ("png", ImageFormat::Png),
    ("gif", ImageFormat::Gif),
    ("webp", ImageFormat::WebP),
    ("bmp", ImageFormat::Bmp),
    ("tiff", ImageFormat::Tiff),
    ("tif", ImageFormat::Tiff),
];

/// Convert format string to `ImageFormat` enum
/// Verifies format against extension map from tools.rs for consistency
#[must_use]
pub fn format_from_string(format_str: &str) -> Option<ImageFormat> {
    let format_lower = format_str.to_lowercase();
    let image_extensions = get_extensions_for_file_type(FileType::Image);

    // Find matching format in our map, but only if it's in the extension map
    IMAGE_FORMAT_MAP
        .iter()
        .find(|(ext, _)| format_lower.contains(ext) && image_extensions.contains(ext))
        .map(|(_, format)| *format)
}
