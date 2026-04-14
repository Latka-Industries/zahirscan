//! WebP format metadata extraction

use super::constants::{
    COLOR_TYPE_RGB8, COLOR_TYPE_RGBA8, WEBP_ALPHA_FLAG, WEBP_VP8, WEBP_VP8L, WEBP_VP8X,
    has_webp_signature,
};

/// Extract compression info from WebP data
pub fn extract_compression(webp_data_ref: &[u8]) -> Option<String> {
    // Check WebP RIFF signature
    // Bytes 0-3: "RIFF"
    // Bytes 8-11: "WEBP"
    // Bytes 12-15: chunk type ("VP8 " for lossy, "VP8L" for lossless)
    if webp_data_ref.len() >= 16 && has_webp_signature(webp_data_ref) {
        let chunk_type = &webp_data_ref[12..16];
        match chunk_type {
            chunk if chunk == WEBP_VP8 => Some("VP8 (lossy)".to_string()),
            chunk if chunk == WEBP_VP8L => Some("VP8L (lossless)".to_string()),
            chunk if chunk == WEBP_VP8X => Some("VP8X (extended)".to_string()),
            _ => Some("WebP".to_string()), // Unknown WebP variant
        }
    } else {
        None
    }
}

/// Extract color type from WebP data
pub fn extract_color_type(webp_data_ref: &[u8]) -> Option<String> {
    // WebP color type depends on chunk type
    // VP8 (lossy) = RGB
    // VP8L (lossless) = can be RGB or RGBA
    // VP8X (extended) = can have alpha channel
    if webp_data_ref.len() < 16 {
        return None;
    }

    if !has_webp_signature(webp_data_ref) {
        return None;
    }

    let chunk_type = &webp_data_ref[12..16];
    match chunk_type {
        chunk if chunk == WEBP_VP8 => Some(COLOR_TYPE_RGB8.to_string()), // Lossy, typically RGB
        chunk if chunk == WEBP_VP8L => {
            // VP8L can have alpha, check alpha flag at byte 20 (bit 4)
            if webp_data_ref.len() >= 21 {
                let flags = webp_data_ref[20];
                if (flags & WEBP_ALPHA_FLAG) != 0 {
                    Some(COLOR_TYPE_RGBA8.to_string())
                } else {
                    Some(COLOR_TYPE_RGB8.to_string())
                }
            } else {
                Some(COLOR_TYPE_RGB8.to_string()) // Default assumption
            }
        }
        chunk if chunk == WEBP_VP8X => {
            // VP8X extended format, check alpha flag at byte 20 (bit 4)
            if webp_data_ref.len() >= 21 {
                let flags = webp_data_ref[20];
                if (flags & WEBP_ALPHA_FLAG) != 0 {
                    Some(COLOR_TYPE_RGBA8.to_string())
                } else {
                    Some(COLOR_TYPE_RGB8.to_string())
                }
            } else {
                Some(COLOR_TYPE_RGB8.to_string()) // Default assumption
            }
        }
        _ => Some(COLOR_TYPE_RGB8.to_string()), // Default assumption
    }
}
