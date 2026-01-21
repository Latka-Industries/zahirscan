//! Format-specific image metadata extraction

use log::warn;

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
}

impl FormatMetadata for ImageFormat {
    fn extract_compression(&self, data: &[u8]) -> Option<String> {
        match self {
            ImageFormat::Jpeg => extract_jpeg_compression(data),
            ImageFormat::Png => extract_png_compression(data),
            ImageFormat::Gif => extract_gif_compression(data),
            ImageFormat::WebP => extract_webp_compression(data),
            ImageFormat::Bmp => extract_bmp_compression(data),
            ImageFormat::Tiff => extract_tiff_compression(data),
        }
    }

    fn extract_chroma_subsampling(&self, data: &[u8]) -> Option<String> {
        match self {
            ImageFormat::Jpeg => extract_jpeg_subsampling(data),
            _ => None, // Only JPEG uses chroma subsampling
        }
    }
}

// JPEG metadata extraction
fn extract_jpeg_subsampling(jpeg_data: &[u8]) -> Option<String> {
    jpeg_chroma_subsampling(jpeg_data)
}

fn extract_jpeg_compression(_jpeg_data: &[u8]) -> Option<String> {
    // JPEG quality is not stored in the file - it's an encoding parameter
    // We can only note that it's JPEG compression
    Some("JPEG".to_string())
}

// PNG metadata extraction
fn extract_png_compression(png_data: &[u8]) -> Option<String> {
    // Check PNG signature (first 8 bytes)
    if png_data.len() >= 8 && png_data[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        // PNG compression method is always DEFLATE (method 0 per PNG spec)
        // Compression level is not stored in the file - it's an encoding parameter
        Some("DEFLATE".to_string())
    } else {
        None
    }
}

// GIF metadata extraction
fn extract_gif_compression(gif_data: &[u8]) -> Option<String> {
    // Check GIF signature (first 6 bytes: "GIF87a" or "GIF89a")
    if gif_data.len() >= 6 {
        let signature = &gif_data[0..6];
        if signature == b"GIF87a" || signature == b"GIF89a" {
            // GIF always uses LZW compression (lossless)
            Some("LZW".to_string())
        } else {
            None
        }
    } else {
        None
    }
}

// WebP metadata extraction
fn extract_webp_compression(webp_data: &[u8]) -> Option<String> {
    // Check WebP RIFF signature
    // Bytes 0-3: "RIFF"
    // Bytes 8-11: "WEBP"
    // Bytes 12-15: chunk type ("VP8 " for lossy, "VP8L" for lossless)
    if webp_data.len() >= 16 {
        if &webp_data[0..4] == b"RIFF" && &webp_data[8..12] == b"WEBP" {
            let chunk_type = &webp_data[12..16];
            match chunk_type {
                b"VP8 " => Some("VP8 (lossy)".to_string()),
                b"VP8L" => Some("VP8L (lossless)".to_string()),
                b"VP8X" => Some("VP8X (extended)".to_string()),
                _ => Some("WebP".to_string()), // Unknown WebP variant
            }
        } else {
            None
        }
    } else {
        None
    }
}

// BMP metadata extraction
fn extract_bmp_compression(bmp_data: &[u8]) -> Option<String> {
    // Check BMP signature (first 2 bytes: "BM")
    if bmp_data.len() < 54 {
        return None;
    }

    if &bmp_data[0..2] != b"BM" {
        return None;
    }

    // Read compression field at offset 30 (4 bytes, little-endian)
    let compression = u32::from_le_bytes([bmp_data[30], bmp_data[31], bmp_data[32], bmp_data[33]]);

    // BMP compression constants
    // 0 = BI_RGB (no compression)
    // 1 = BI_RLE8 (8-bit RLE)
    // 2 = BI_RLE4 (4-bit RLE)
    // 3 = BI_BITFIELDS (bit fields)
    // 4 = BI_JPEG (JPEG compression)
    // 5 = BI_PNG (PNG compression)
    match compression {
        0 => Some("None (BI_RGB)".to_string()),
        1 => Some("RLE8".to_string()),
        2 => Some("RLE4".to_string()),
        3 => Some("Bitfields".to_string()),
        4 => Some("JPEG".to_string()),
        5 => Some("PNG".to_string()),
        _ => Some(format!("Unknown ({})", compression)),
    }
}

// TIFF metadata extraction
fn extract_tiff_compression(tiff_data: &[u8]) -> Option<String> {
    if tiff_data.len() < 8 {
        return None;
    }

    // Check TIFF signature
    // Little-endian: "II" (0x4949) + version 42 (0x002A)
    // Big-endian: "MM" (0x4D4D) + version 42 (0x002A)
    let is_little_endian = &tiff_data[0..2] == b"II";
    let is_big_endian = &tiff_data[0..2] == b"MM";

    if !is_little_endian && !is_big_endian {
        return None;
    }

    // Read version (should be 42)
    let version = if is_little_endian {
        u16::from_le_bytes([tiff_data[2], tiff_data[3]])
    } else {
        u16::from_be_bytes([tiff_data[2], tiff_data[3]])
    };

    if version != 42 {
        return None;
    }

    // TIFF compression is stored in IFD entries (tag 259/0x0103)
    // Parsing IFD requires reading the offset at byte 4, then traversing IFD entries
    // This is complex, so for now we just note it's TIFF
    // Full implementation would require parsing the IFD structure
    Some("TIFF".to_string())
}

/// Parse JPEG SOF marker and infer chroma subsampling from sampling factors.
///
/// We look for a Start Of Frame marker (SOF0/SOF1/SOF2/etc), then read the
/// per-component sampling factors. This avoids requiring libjpeg-turbo / NASM
/// in CI (pure Rust, header-only parsing).
fn jpeg_chroma_subsampling(jpeg_data: &[u8]) -> Option<String> {
    // Must start with SOI (FF D8)
    if jpeg_data.len() < 4 || jpeg_data[0] != 0xFF || jpeg_data[1] != 0xD8 {
        return None;
    }

    let mut i = 2usize;
    while i + 4 <= jpeg_data.len() {
        // Find next marker (0xFF ...)
        if jpeg_data[i] != 0xFF {
            i += 1;
            continue;
        }

        // Skip fill bytes (FF FF...)
        while i < jpeg_data.len() && jpeg_data[i] == 0xFF {
            i += 1;
        }
        if i >= jpeg_data.len() {
            break;
        }

        let marker = jpeg_data[i];
        i += 1;

        // Markers without length field (standalone)
        match marker {
            0xD9 => break,           // EOI
            0xD0..=0xD7 => continue, // RSTn
            0x01 => continue,        // TEM
            _ => {}
        }

        if i + 2 > jpeg_data.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([jpeg_data[i], jpeg_data[i + 1]]) as usize;
        i += 2;
        if seg_len < 2 || i + (seg_len - 2) > jpeg_data.len() {
            break;
        }

        // SOF markers (baseline/progressive/etc). Exclude DHT/DAC/DRI/etc.
        let is_sof = matches!(
            marker,
            0xC0 | 0xC1
                | 0xC2
                | 0xC3
                | 0xC5
                | 0xC6
                | 0xC7
                | 0xC9
                | 0xCA
                | 0xCB
                | 0xCD
                | 0xCE
                | 0xCF
        );
        if is_sof {
            // SOF segment layout:
            // [P][Yhi][Ylo][Xhi][Xlo][Nf] then Nf * ([Ci][HiVi][Tqi])
            let seg = &jpeg_data[i..i + (seg_len - 2)];
            if seg.len() < 6 {
                return None;
            }
            let nf = seg[5] as usize;
            if nf == 0 {
                return None;
            }
            if nf == 1 {
                return Some("Grayscale".to_string());
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
                (1, 1, 1, 1) => "4:4:4",
                (2, 1, 1, 1) => "4:2:2",
                (2, 2, 1, 1) => "4:2:0",
                (4, 1, 1, 1) => "4:1:1",
                // Less common / ambiguous
                _ => {
                    warn!(
                        "Unknown JPEG sampling factors: Y={}x{}, C={}x{}",
                        yh, yv, ch, cv
                    );
                    return Some("Unknown".to_string());
                }
            };
            return Some(out.to_string());
        }

        // Skip segment payload
        i += seg_len - 2;
    }

    None
}

/// Convert format string to ImageFormat enum
pub fn format_from_string(format_str: &str) -> Option<ImageFormat> {
    match format_str.to_lowercase().as_str() {
        "jpeg" | "jpg" => Some(ImageFormat::Jpeg),
        "png" => Some(ImageFormat::Png),
        "gif" => Some(ImageFormat::Gif),
        "webp" => Some(ImageFormat::WebP),
        "bmp" => Some(ImageFormat::Bmp),
        "tiff" | "tif" => Some(ImageFormat::Tiff),
        _ => None,
    }
}
