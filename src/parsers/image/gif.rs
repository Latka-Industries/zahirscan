//! GIF format metadata extraction

use super::constants::{COLOR_TYPE_INDEXED8, has_gif_signature};

/// Extract compression info from GIF data
pub fn extract_compression(gif_data: &[u8]) -> Option<String> {
    // Check GIF signature (first 6 bytes: "GIF87a" or "GIF89a")
    if has_gif_signature(gif_data) {
        // GIF always uses LZW compression (lossless)
        Some("LZW".to_string())
    } else {
        None
    }
}

/// Extract color type from GIF data
pub fn extract_color_type(gif_data: &[u8]) -> Option<String> {
    // GIF logical screen descriptor starts at byte 6
    // Byte 10 contains packed fields: global color table flag, color resolution, sort flag, global color table size
    if gif_data.len() < 11 {
        return None;
    }

    if !has_gif_signature(gif_data) {
        return None;
    }

    // GIF uses indexed color (palette-based)
    // The actual color depth depends on the global color table size
    // For simplicity, we'll return Indexed8 (GIF is always 8-bit per pixel)
    Some(COLOR_TYPE_INDEXED8.to_string())
}
