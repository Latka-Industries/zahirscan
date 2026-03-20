//! PNG format metadata extraction

use super::constants::{
    COLOR_TYPE_INDEXED8, COLOR_TYPE_L8, COLOR_TYPE_L16, COLOR_TYPE_LA8, COLOR_TYPE_LA16,
    COLOR_TYPE_RGB8, COLOR_TYPE_RGB16, COLOR_TYPE_RGBA8, COLOR_TYPE_RGBA16, PNG_IHDR,
    has_png_signature,
};

/// Extract compression info from PNG data
pub fn extract_compression(png_data: &[u8]) -> Option<String> {
    // Check PNG signature (first 8 bytes)
    if has_png_signature(png_data) {
        // PNG compression method is always DEFLATE (method 0 per PNG spec)
        // Compression level is not stored in the file - it's an encoding parameter
        Some("DEFLATE".to_string())
    } else {
        None
    }
}

/// Extract bit depth from PNG data
pub fn extract_bit_depth(png_data: &[u8]) -> Option<u32> {
    // PNG structure:
    // Bytes 0-7: PNG signature
    // Bytes 8-11: Chunk length (4 bytes, big-endian)
    // Bytes 12-15: Chunk type "IHDR" (4 bytes)
    // Bytes 16-19: Width (4 bytes)
    // Bytes 20-23: Height (4 bytes)
    // Byte 24: Bit depth
    // Byte 25: Color type
    if png_data.len() < 25 {
        return None;
    }

    // Check PNG signature
    if !has_png_signature(png_data) {
        return None;
    }

    // Check IHDR chunk signature (bytes 12-15 should be "IHDR")
    if &png_data[12..16] != PNG_IHDR {
        return None;
    }

    // Bit depth is at byte 24 (8th byte of IHDR data)
    let bit_depth = png_data[24];
    Some(bit_depth as u32)
}

/// Extract color type from PNG data
pub fn extract_color_type(png_data: &[u8]) -> Option<String> {
    // PNG structure:
    // Bytes 0-7: PNG signature
    // Bytes 8-11: Chunk length (4 bytes, big-endian)
    // Bytes 12-15: Chunk type "IHDR" (4 bytes)
    // Bytes 16-19: Width (4 bytes)
    // Bytes 20-23: Height (4 bytes)
    // Byte 24: Bit depth
    // Byte 25: Color type
    if png_data.len() < 26 {
        return None;
    }

    // Check PNG signature
    if !has_png_signature(png_data) {
        return None;
    }

    // Check IHDR chunk signature (bytes 12-15 should be "IHDR")
    if &png_data[12..16] != PNG_IHDR {
        return None;
    }

    let bit_depth = png_data[24];
    let color_type = png_data[25];

    // PNG color type values:
    // 0 = Grayscale
    // 2 = RGB
    // 3 = Indexed (palette)
    // 4 = Grayscale with alpha
    // 6 = RGBA
    match (color_type, bit_depth) {
        (0, 8) => Some(COLOR_TYPE_L8.to_string()),
        (0, 16) => Some(COLOR_TYPE_L16.to_string()),
        (2, 8) => Some(COLOR_TYPE_RGB8.to_string()),
        (2, 16) => Some(COLOR_TYPE_RGB16.to_string()),
        (3, 8) => Some(COLOR_TYPE_INDEXED8.to_string()), // Palette-based
        (4, 8) => Some(COLOR_TYPE_LA8.to_string()),
        (4, 16) => Some(COLOR_TYPE_LA16.to_string()),
        (6, 8) => Some(COLOR_TYPE_RGBA8.to_string()),
        (6, 16) => Some(COLOR_TYPE_RGBA16.to_string()),
        _ => Some(format!("Unknown({color_type},{bit_depth})")),
    }
}
