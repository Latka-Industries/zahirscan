//! TIFF format metadata extraction

/// Helper function to read a u16 from a byte slice with proper endianness
/// Returns None if offset is out of bounds
#[inline]
fn read_u16_safe(data_ref: &[u8], offset: usize, is_little_endian: bool) -> Option<u16> {
    if offset + 1 >= data_ref.len() {
        return None;
    }
    let bytes = [data_ref[offset], data_ref[offset + 1]];
    Some(if is_little_endian {
        u16::from_le_bytes(bytes)
    } else {
        u16::from_be_bytes(bytes)
    })
}

/// Helper function to read a u32 from a byte slice with proper endianness
/// Returns None if offset is out of bounds
#[inline]
fn read_u32_safe(data_ref: &[u8], offset: usize, is_little_endian: bool) -> Option<u32> {
    if offset + 3 >= data_ref.len() {
        return None;
    }
    let bytes = [
        data_ref[offset],
        data_ref[offset + 1],
        data_ref[offset + 2],
        data_ref[offset + 3],
    ];
    Some(if is_little_endian {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    })
}

use super::constants::{
    COLOR_TYPE_CMYK8, COLOR_TYPE_INDEXED8, COLOR_TYPE_L8, COLOR_TYPE_RGB8, TIFF_LE_SIGNATURE,
    TIFF_PHOTOMETRIC_BLACK_IS_ZERO, TIFF_PHOTOMETRIC_CIELAB, TIFF_PHOTOMETRIC_CMYK,
    TIFF_PHOTOMETRIC_MASK, TIFF_PHOTOMETRIC_PALETTE, TIFF_PHOTOMETRIC_RGB,
    TIFF_PHOTOMETRIC_WHITE_IS_ZERO, TIFF_PHOTOMETRIC_YCBCR, TIFF_TAG_BITS_PER_SAMPLE,
    TIFF_TAG_PHOTOMETRIC_INTERPRETATION, TIFF_TYPE_LONG, TIFF_TYPE_SHORT, TIFF_VERSION,
    has_tiff_signature,
};

/// Extract compression info from TIFF data
pub fn extract_compression(tiff_data_ref: &[u8]) -> Option<String> {
    if tiff_data_ref.len() < 8 {
        return None;
    }

    // Check TIFF signature
    if !has_tiff_signature(tiff_data_ref) {
        return None;
    }

    let is_little_endian = &tiff_data_ref[0..2] == TIFF_LE_SIGNATURE;

    // Read version (should be 42)
    let version = read_u16_safe(tiff_data_ref, 2, is_little_endian)?;

    if version != TIFF_VERSION {
        return None;
    }

    // TIFF compression is stored in IFD entries (tag TIFF_TAG_COMPRESSION)
    // Parsing IFD requires reading the offset at byte 4, then traversing IFD entries
    // This is complex, so for now we just note it's TIFF
    // Full implementation would require parsing the IFD structure
    Some("TIFF".to_string())
}

/// Extract bit depth from TIFF data
pub fn extract_bit_depth(tiff_data_ref: &[u8]) -> Option<u32> {
    // TIFF bit depth is stored in IFD entries (tag TIFF_TAG_BITS_PER_SAMPLE: BitsPerSample)
    // This requires parsing the IFD structure, which is complex
    // For now, we'll try a simple approach: read the first IFD offset and check common values
    if tiff_data_ref.len() < 8 {
        return None;
    }

    if !has_tiff_signature(tiff_data_ref) {
        return None;
    }

    let is_little_endian = &tiff_data_ref[0..2] == TIFF_LE_SIGNATURE;

    // Read IFD offset (bytes 4-7)
    let ifd_offset = match read_u32_safe(tiff_data_ref, 4, is_little_endian) {
        Some(offset) => offset as usize,
        None => return None,
    };

    // Try to read IFD entry count and look for BitsPerSample tag (258)
    if ifd_offset + 2 > tiff_data_ref.len() {
        return None;
    }

    let entry_count = match read_u16_safe(tiff_data_ref, ifd_offset, is_little_endian) {
        Some(count) => count as usize,
        None => return None,
    };

    // Each IFD entry is 12 bytes: 2 bytes tag, 2 bytes type, 4 bytes count, 4 bytes value/offset
    let entry_start = ifd_offset + 2;
    for i in 0..entry_count.min(20) {
        // Limit search to first 20 entries to avoid excessive parsing
        let entry_offset = entry_start + i * 12;
        if entry_offset + 12 > tiff_data_ref.len() {
            break;
        }

        let Some(tag) = read_u16_safe(tiff_data_ref, entry_offset, is_little_endian) else {
            break;
        };

        // Tag TIFF_TAG_BITS_PER_SAMPLE = BitsPerSample
        if tag == TIFF_TAG_BITS_PER_SAMPLE {
            let Some(value) = read_u32_safe(tiff_data_ref, entry_offset + 8, is_little_endian)
            else {
                break;
            };
            return Some(value);
        }
    }

    // Default: assume 8-bit if we can't find it
    None
}

/// Extract color type from TIFF data
///
/// Note: Some TIFF files converted from JPEG (e.g., via `sips`) may have non-standard
/// IFD structures. This function attempts to parse them but falls back to RGB8 if
/// the `PhotometricInterpretation` tag cannot be found or parsed.
pub fn extract_color_type(tiff_data_ref: &[u8]) -> String {
    // TIFF color type is stored in IFD entries (tag 262/0x0106: PhotometricInterpretation)
    // This requires parsing the IFD structure
    if tiff_data_ref.len() < 8 {
        // File too small to be valid TIFF, return default
        return COLOR_TYPE_RGB8.to_string();
    }

    if !has_tiff_signature(tiff_data_ref) {
        // Not a valid TIFF signature, return default
        return COLOR_TYPE_RGB8.to_string();
    }

    let is_little_endian = &tiff_data_ref[0..2] == TIFF_LE_SIGNATURE;

    // Read IFD offset (bytes 4-7)
    let ifd_offset = match read_u32_safe(tiff_data_ref, 4, is_little_endian) {
        Some(offset) => offset as usize,
        None => return COLOR_TYPE_RGB8.to_string(),
    };

    // Try to read IFD entry count and look for PhotometricInterpretation tag (262)
    // If IFD offset is out of bounds, return default (RGB)
    if ifd_offset + 2 > tiff_data_ref.len() {
        return COLOR_TYPE_RGB8.to_string();
    }

    // Safely read entry count
    let entry_count = match read_u16_safe(tiff_data_ref, ifd_offset, is_little_endian) {
        Some(count) => count as usize,
        None => return COLOR_TYPE_RGB8.to_string(),
    };

    // Each IFD entry is 12 bytes: 2 bytes tag, 2 bytes type, 4 bytes count, 4 bytes value/offset
    let entry_start = ifd_offset + 2;
    // Limit search to first 20 entries to avoid excessive parsing
    let max_entries = entry_count.min(20);
    for i in 0..max_entries {
        let entry_offset = entry_start + i * 12;
        // Bounds check: need at least 12 bytes for a complete IFD entry
        if entry_offset + 12 > tiff_data_ref.len() {
            break;
        }

        let Some(tag) = read_u16_safe(tiff_data_ref, entry_offset, is_little_endian) else {
            break;
        };

        // Tag TIFF_TAG_PHOTOMETRIC_INTERPRETATION = PhotometricInterpretation
        if tag == TIFF_TAG_PHOTOMETRIC_INTERPRETATION {
            // Check type field (bytes 2-3 of IFD entry)
            // Type TIFF_TYPE_SHORT = SHORT (16-bit), Type TIFF_TYPE_LONG = LONG (32-bit)
            let Some(entry_type) = read_u16_safe(tiff_data_ref, entry_offset + 2, is_little_endian)
            else {
                continue;
            };

            // PhotometricInterpretation is typically a SHORT (type TIFF_TYPE_SHORT)
            let value = if entry_type == TIFF_TYPE_SHORT {
                // SHORT: read from bytes 8-9 (little-endian) or 10-11 (big-endian)
                // For big-endian, SHORT values are in the high bytes (10-11) of the 4-byte value field
                let read_offset = if is_little_endian {
                    entry_offset + 8
                } else {
                    entry_offset + 10
                };
                match read_u16_safe(tiff_data_ref, read_offset, is_little_endian) {
                    Some(v) => v as u32,
                    None => continue,
                }
            } else if entry_type == TIFF_TYPE_LONG {
                // LONG: read all 4 bytes
                match read_u32_safe(tiff_data_ref, entry_offset + 8, is_little_endian) {
                    Some(v) => v,
                    None => continue,
                }
            } else {
                // Unknown type for PhotometricInterpretation - try to read as SHORT anyway
                // Some TIFF writers might use non-standard types
                if is_little_endian && entry_offset + 10 <= tiff_data_ref.len() {
                    // Try reading as SHORT from bytes 8-9
                    let value =
                        match read_u16_safe(tiff_data_ref, entry_offset + 8, is_little_endian) {
                            Some(v) => v as u32,
                            None => continue,
                        };
                    return match value {
                        v if v == TIFF_PHOTOMETRIC_WHITE_IS_ZERO
                            || v == TIFF_PHOTOMETRIC_BLACK_IS_ZERO =>
                        {
                            COLOR_TYPE_L8.to_string()
                        }
                        v if v == TIFF_PHOTOMETRIC_RGB => COLOR_TYPE_RGB8.to_string(),
                        v if v == TIFF_PHOTOMETRIC_PALETTE => COLOR_TYPE_INDEXED8.to_string(),
                        v if v == TIFF_PHOTOMETRIC_MASK => COLOR_TYPE_L8.to_string(), // Transparency mask
                        v if v == TIFF_PHOTOMETRIC_CMYK => COLOR_TYPE_CMYK8.to_string(),
                        v if v == TIFF_PHOTOMETRIC_YCBCR => COLOR_TYPE_RGB8.to_string(),
                        v if v == TIFF_PHOTOMETRIC_CIELAB => COLOR_TYPE_RGB8.to_string(), // CIE L*a*b*
                        _ => format!("Unknown({value})"),
                    };
                }
                // If we can't read it, skip this tag and continue searching
                continue;
            };

            // PhotometricInterpretation values
            return match value {
                v if v == TIFF_PHOTOMETRIC_WHITE_IS_ZERO || v == TIFF_PHOTOMETRIC_BLACK_IS_ZERO => {
                    COLOR_TYPE_L8.to_string() // Grayscale
                }
                v if v == TIFF_PHOTOMETRIC_RGB => COLOR_TYPE_RGB8.to_string(), // RGB
                v if v == TIFF_PHOTOMETRIC_PALETTE => COLOR_TYPE_INDEXED8.to_string(), // Palette
                v if v == TIFF_PHOTOMETRIC_MASK => COLOR_TYPE_L8.to_string(), // Transparency mask (grayscale)
                v if v == TIFF_PHOTOMETRIC_CMYK => COLOR_TYPE_CMYK8.to_string(), // CMYK
                v if v == TIFF_PHOTOMETRIC_YCBCR => COLOR_TYPE_RGB8.to_string(), // YCbCr (treated as RGB)
                v if v == TIFF_PHOTOMETRIC_CIELAB => COLOR_TYPE_RGB8.to_string(), // CIE L*a*b* (treated as RGB for display)
                _ => format!("Unknown({value})"),
            };
        }
    }

    // Default: assume RGB if we can't find it
    COLOR_TYPE_RGB8.to_string()
}
