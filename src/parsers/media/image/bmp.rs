//! BMP format metadata extraction

use super::constants::{
    BMP_COMPRESSION_BITFIELDS, BMP_COMPRESSION_JPEG, BMP_COMPRESSION_PNG, BMP_COMPRESSION_RGB,
    BMP_COMPRESSION_RLE4, BMP_COMPRESSION_RLE8, COLOR_TYPE_INDEXED1, COLOR_TYPE_INDEXED4,
    COLOR_TYPE_INDEXED8, COLOR_TYPE_RGB8, COLOR_TYPE_RGB16, COLOR_TYPE_RGBA8, has_bmp_signature,
};

/// Extract compression info from BMP data
pub fn extract_compression(bmp_data: &[u8]) -> Option<String> {
    // Check BMP signature (first 2 bytes: "BM")
    if bmp_data.len() < 54 {
        return None;
    }

    if !has_bmp_signature(bmp_data) {
        return None;
    }

    // Read compression field at offset 30 (4 bytes, little-endian)
    let compression = u32::from_le_bytes([bmp_data[30], bmp_data[31], bmp_data[32], bmp_data[33]]);

    // BMP compression constants
    match compression {
        c if c == BMP_COMPRESSION_RGB => Some("None (BI_RGB)".to_string()),
        c if c == BMP_COMPRESSION_RLE8 => Some("RLE8".to_string()),
        c if c == BMP_COMPRESSION_RLE4 => Some("RLE4".to_string()),
        c if c == BMP_COMPRESSION_BITFIELDS => Some("Bitfields".to_string()),
        c if c == BMP_COMPRESSION_JPEG => Some("JPEG".to_string()),
        c if c == BMP_COMPRESSION_PNG => Some("PNG".to_string()),
        _ => Some(format!("Unknown ({})", compression)),
    }
}

/// Extract bit depth from BMP data
pub fn extract_bit_depth(bmp_data: &[u8]) -> Option<u32> {
    // BMP bit depth is at offset 28 (2 bytes, little-endian)
    if bmp_data.len() < 30 {
        return None;
    }

    if !has_bmp_signature(bmp_data) {
        return None;
    }

    let bit_depth = u16::from_le_bytes([bmp_data[28], bmp_data[29]]);
    Some(bit_depth as u32)
}

/// Extract color type from BMP data
pub fn extract_color_type(bmp_data: &[u8]) -> Option<String> {
    // BMP color type depends on bit depth
    // Bit depth is at offset 28 (2 bytes, little-endian)
    if bmp_data.len() < 30 {
        return None;
    }

    if !has_bmp_signature(bmp_data) {
        return None;
    }

    let bit_depth = u16::from_le_bytes([bmp_data[28], bmp_data[29]]);

    // BMP color type mapping based on bit depth
    match bit_depth {
        1 => Some(COLOR_TYPE_INDEXED1.to_string()),
        4 => Some(COLOR_TYPE_INDEXED4.to_string()),
        8 => Some(COLOR_TYPE_INDEXED8.to_string()),
        16 => Some(COLOR_TYPE_RGB16.to_string()), // Usually 5-6-5 format
        24 => Some(COLOR_TYPE_RGB8.to_string()),  // 24-bit RGB
        32 => Some(COLOR_TYPE_RGBA8.to_string()), // 32-bit RGBA
        _ => Some(format!("Unknown({})", bit_depth)),
    }
}
