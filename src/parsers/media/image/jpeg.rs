//! JPEG format metadata extraction

use super::constants::{
    COLOR_TYPE_CMYK8, COLOR_TYPE_GRAYSCALE, COLOR_TYPE_L8, COLOR_TYPE_RGB8, JPEG_EOI, JPEG_TEM,
    SUBSAMPLING_411, SUBSAMPLING_420, SUBSAMPLING_422, SUBSAMPLING_444, SUBSAMPLING_UNKNOWN,
    has_jpeg_signature, is_jpeg_rst_marker, is_jpeg_sof_marker,
};
use log::warn;

/// Extract chroma subsampling from JPEG data
pub fn extract_subsampling(jpeg_data_ref: &[u8]) -> Option<String> {
    jpeg_chroma_subsampling(jpeg_data_ref)
}

/// Extract compression info from JPEG data
pub fn extract_compression(_jpeg_data_ref: &[u8]) -> String {
    // JPEG quality is not stored in the file - it's an encoding parameter
    // We can only note that it's JPEG compression
    "JPEG".to_string()
}

/// Extract bit depth from JPEG data
pub fn extract_bit_depth(jpeg_data_ref: &[u8]) -> Option<u32> {
    // JPEG is typically 8-bit per channel, but can be 12-bit for some variants
    // Check SOF marker precision field (byte after marker in SOF segment)
    if !has_jpeg_signature(jpeg_data_ref) {
        return None;
    }

    let mut i = 2usize;
    while i + 4 <= jpeg_data_ref.len() {
        if jpeg_data_ref[i] != 0xFF {
            i += 1;
            continue;
        }

        while i < jpeg_data_ref.len() && jpeg_data_ref[i] == 0xFF {
            i += 1;
        }
        if i >= jpeg_data_ref.len() {
            break;
        }

        let marker = jpeg_data_ref[i];
        i += 1;

        match marker {
            marker if marker == JPEG_EOI => break,
            marker if is_jpeg_rst_marker(marker) => continue,
            marker if marker == JPEG_TEM => continue,
            _ => {}
        }

        if i + 2 > jpeg_data_ref.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([jpeg_data_ref[i], jpeg_data_ref[i + 1]]) as usize;
        i += 2;

        // SOF markers
        let is_sof = is_jpeg_sof_marker(marker);
        if is_sof && seg_len >= 3 && i < jpeg_data_ref.len() {
            // Precision is the first byte of the SOF segment
            let precision = jpeg_data_ref[i];
            return Some(u32::from(precision));
        }

        if seg_len < 2 || i + (seg_len - 2) > jpeg_data_ref.len() {
            break;
        }
        i += seg_len - 2;
    }

    // Default: JPEG is usually 8-bit
    Some(8)
}

/// Extract color type from JPEG data
pub fn extract_color_type(jpeg_data_ref: &[u8]) -> Option<String> {
    // JPEG color type depends on number of components in SOF marker
    // 1 component = grayscale (L8)
    // 3 components = RGB (Rgb8)
    // 4 components = CMYK (not standard, but possible)
    if !has_jpeg_signature(jpeg_data_ref) {
        return None;
    }

    let mut i = 2usize;
    while i + 4 <= jpeg_data_ref.len() {
        if jpeg_data_ref[i] != 0xFF {
            i += 1;
            continue;
        }

        while i < jpeg_data_ref.len() && jpeg_data_ref[i] == 0xFF {
            i += 1;
        }
        if i >= jpeg_data_ref.len() {
            break;
        }

        let marker = jpeg_data_ref[i];
        i += 1;

        match marker {
            marker if marker == JPEG_EOI => break,
            marker if is_jpeg_rst_marker(marker) => continue,
            marker if marker == JPEG_TEM => continue,
            _ => {}
        }

        if i + 2 > jpeg_data_ref.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([jpeg_data_ref[i], jpeg_data_ref[i + 1]]) as usize;
        i += 2;

        // SOF markers
        let is_sof = is_jpeg_sof_marker(marker);
        if is_sof && seg_len >= 6 && i + 5 <= jpeg_data_ref.len() {
            // SOF segment layout: [P][Yhi][Ylo][Xhi][Xlo][Nf]...
            // P = precision (byte 0), Y = height (bytes 1-2), X = width (bytes 3-4), Nf = components (byte 5)
            let num_components = jpeg_data_ref[i + 5];
            return match num_components {
                1 => Some(COLOR_TYPE_L8.to_string()),    // Grayscale
                3 => Some(COLOR_TYPE_RGB8.to_string()),  // RGB
                4 => Some(COLOR_TYPE_CMYK8.to_string()), // CMYK (less common)
                _ => Some(format!("Unknown({num_components})")),
            };
        }

        if seg_len < 2 || i + (seg_len - 2) > jpeg_data_ref.len() {
            break;
        }
        i += seg_len - 2;
    }

    // Default: assume RGB (most common)
    Some(COLOR_TYPE_RGB8.to_string())
}

/// Parse JPEG SOF marker and infer chroma subsampling from sampling factors.
///
/// We look for a Start Of Frame marker (SOF0/SOF1/SOF2/etc), then read the
/// per-component sampling factors. This avoids requiring libjpeg-turbo / NASM
/// in CI (pure Rust, header-only parsing).
fn jpeg_chroma_subsampling(jpeg_data_ref: &[u8]) -> Option<String> {
    // Must start with SOI (FF D8)
    if jpeg_data_ref.len() < 4 || jpeg_data_ref[0] != 0xFF || jpeg_data_ref[1] != 0xD8 {
        return None;
    }

    let mut i = 2usize;
    while i + 4 <= jpeg_data_ref.len() {
        // Find next marker (0xFF ...)
        if jpeg_data_ref[i] != 0xFF {
            i += 1;
            continue;
        }

        // Skip fill bytes (FF FF...)
        while i < jpeg_data_ref.len() && jpeg_data_ref[i] == 0xFF {
            i += 1;
        }
        if i >= jpeg_data_ref.len() {
            break;
        }

        let marker = jpeg_data_ref[i];
        i += 1;

        // Markers without length field (standalone)
        match marker {
            marker if marker == JPEG_EOI => break,
            marker if is_jpeg_rst_marker(marker) => continue,
            marker if marker == JPEG_TEM => continue,
            _ => {}
        }

        if i + 2 > jpeg_data_ref.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([jpeg_data_ref[i], jpeg_data_ref[i + 1]]) as usize;
        i += 2;
        if seg_len < 2 || i + (seg_len - 2) > jpeg_data_ref.len() {
            break;
        }

        // SOF markers (baseline/progressive/etc). Exclude DHT/DAC/DRI/etc.
        let is_sof = is_jpeg_sof_marker(marker);
        if is_sof {
            // SOF segment layout:
            // [P][Yhi][Ylo][Xhi][Xlo][Nf] then Nf * ([Ci][HiVi][Tqi])
            let seg = &jpeg_data_ref[i..i + (seg_len - 2)];
            if seg.len() < 6 {
                return None;
            }
            let nf = seg[5] as usize;
            if nf == 0 {
                return None;
            }
            if nf == 1 {
                return Some(COLOR_TYPE_GRAYSCALE.to_string());
            }
            if seg.len() < 6 + nf * 3 {
                return None;
            }

            // Extract sampling factors for Y(1), Cb(2), Cr(3) when present.
            let mut y: Option<(u8, u8)> = None;
            let mut cb: Option<(u8, u8)> = None;
            let mut cr: Option<(u8, u8)> = None;

            for c in 0..nf {
                let base = 6 + c * 3;
                let cid = seg[base];
                let hv = seg[base + 1];
                let h = hv >> 4;
                let v = hv & 0x0F;
                match cid {
                    1 => y = Some((h, v)),
                    2 => cb = Some((h, v)),
                    3 => cr = Some((h, v)),
                    _ => {}
                }
            }

            let (yh, yv) = y?;
            // If we don't have chroma components, best effort.
            let cb = cb.or(cr);
            let (ch, cv) = cb?;

            // Common subsampling patterns:
            // 4:4:4 => Y 1x1, C 1x1
            // 4:2:2 => Y 2x1, C 1x1
            // 4:2:0 => Y 2x2, C 1x1
            // 4:1:1 => Y 4x1, C 1x1
            let out = match (yh, yv, ch, cv) {
                (1, 1, 1, 1) => SUBSAMPLING_444,
                (2, 1, 1, 1) => SUBSAMPLING_422,
                (2, 2, 1, 1) => SUBSAMPLING_420,
                (4, 1, 1, 1) => SUBSAMPLING_411,
                // Less common / ambiguous
                _ => {
                    warn!("Unknown JPEG sampling factors: Y={yh}x{yv}, C={ch}x{cv}");
                    return Some(SUBSAMPLING_UNKNOWN.to_string());
                }
            };
            return Some(out.to_string());
        }

        // Skip segment payload
        i += seg_len - 2;
    }

    None
}
