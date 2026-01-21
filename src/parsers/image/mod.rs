//! Image file metadata extraction

mod metadata;

use crate::config::Config;
use crate::parsers::ParseResult;
use crate::results::{ImageMetadata, MiningResult};
use anyhow::Result;
use image::ImageReader;
use metadata::{FormatMetadata, format_from_string};
use std::io::Cursor;

/// Extract image metadata
/// Uses into_dimensions() to read only header metadata without decoding the full image
pub fn extract_image_metadata(
    content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<ImageMetadata> {
    let reader = ImageReader::new(Cursor::new(content));
    let stream_size = Some(stats.byte_count);

    match reader.with_guessed_format() {
        Ok(reader) => {
            let format = reader.format().map(|f| format!("{:?}", f));
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

                    // Extract format-specific metadata based on detected format
                    // Format string from Debug representation (e.g., "Jpeg", "Png")
                    // format_from_string already handles case-insensitive matching
                    let (chroma_subsampling, compression) =
                        if let Some(format_str) = format.as_deref() {
                            if let Some(image_format) = format_from_string(format_str) {
                                (
                                    image_format.extract_chroma_subsampling(content),
                                    image_format.extract_compression(content),
                                )
                            } else {
                                // Format not recognized by our parser, but we still have dimensions
                                (None, None)
                            }
                        } else {
                            // Format detection failed, but we still have dimensions
                            (None, None)
                        };

                    Ok(ImageMetadata {
                        width: width_usize,
                        height: height_usize,
                        aspect_ratio,
                        stream_size,
                        color_type: None, // Color type requires full decode, skip for speed
                        format,
                        chroma_subsampling,
                        compression,
                    })
                }
                Err(_) => {
                    // Failed to read dimensions, return basic metadata with format if available
                    Ok(ImageMetadata {
                        width: 0,
                        height: 0,
                        aspect_ratio: None,
                        stream_size,
                        color_type: None,
                        format,
                        chroma_subsampling: None,
                        compression: None,
                    })
                }
            }
        }
        Err(_) => {
            // Could not determine format, return empty metadata
            Ok(ImageMetadata {
                width: 0,
                height: 0,
                aspect_ratio: None,
                stream_size,
                color_type: None,
                format: None,
                chroma_subsampling: None,
                compression: None,
            })
        }
    }
}

/// Extract templates from image files (images don't have templates, return empty result)
pub fn extract_image_templates(
    _content: &[u8],
    stats: &ParseResult,
    _config: &Config,
) -> Result<MiningResult> {
    // Images don't have templates, return empty result
    Ok(crate::parsers::traits::empty_mining_result(stats))
}
