//! Image file metadata extraction

pub mod metadata;

mod bmp;
mod constants;
mod gif;
mod jpeg;
mod png;
mod tiff;
mod webp;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::ImageMetadata;
use anyhow::Result;
use image::ImageReader;
use metadata::{FormatMetadata, format_from_string};
use std::io::Cursor;

/// Extract image metadata
/// Uses into_dimensions() to read only header metadata without decoding the full image
pub fn extract_image_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> Result<ImageMetadata> {
    let reader = ImageReader::new(Cursor::new(content));
    let stream_size = Some(stats.byte_count);

    match reader.with_guessed_format() {
        Ok(reader) => {
            let format = reader.format().map(|f| format!("{:?}", f));

            // Extract format-specific metadata based on detected format
            // This can be done even if into_dimensions() fails, since header-only parsing
            // doesn't require full image decoding
            let (chroma_subsampling, compression, bit_depth, color_type) = format
                .as_deref()
                .and_then(format_from_string)
                .map(|image_format| {
                    (
                        image_format.extract_chroma_subsampling(content),
                        image_format.extract_compression(content),
                        image_format.extract_bit_depth(content),
                        image_format.extract_color_type(content),
                    )
                })
                .unwrap_or((None, None, None, None));

            // Use into_dimensions() instead of decode() - reads only header metadata
            // This is much faster as it doesn't decompress/decode the entire image
            match reader.into_dimensions() {
                Ok((width, height)) => {
                    let width_usize = width as usize;
                    let height_usize = height as usize;
                    // Calculate aspect ratio
                    let aspect_ratio = if height_usize > 0 {
                        Some(width_usize as f64 / height_usize as f64)
                    } else {
                        None
                    };

                    Ok(ImageMetadata {
                        width: width_usize,
                        height: height_usize,
                        aspect_ratio,
                        stream_size,
                        color_type,
                        format,
                        chroma_subsampling,
                        compression,
                        bit_depth,
                    })
                }
                Err(_) => {
                    // Failed to read dimensions, but we can still return format-specific metadata
                    Ok(ImageMetadata {
                        width: 0,
                        height: 0,
                        stream_size,
                        color_type,
                        format,
                        chroma_subsampling,
                        compression,
                        bit_depth,
                        ..Default::default()
                    })
                }
            }
        }
        Err(_) => {
            // Could not determine format, return empty metadata
            Ok(ImageMetadata {
                stream_size,
                ..Default::default()
            })
        }
    }
}

crate::no_template_mining!(
    extract_image_templates,
    "Images don't have templates, return empty result."
);
