//! Image format constants: magic numbers, headers, markers, and tags
//!
//! Centralizes all format-specific constants used across image parsers
//! for maintainability and consistency.

// ============================================================================
// Format Signatures / Magic Numbers
// ============================================================================

/// JPEG Start of Image (SOI) marker
pub const JPEG_SOI: &[u8] = &[0xFF, 0xD8];

/// PNG signature (first 8 bytes)
pub const PNG_SIGNATURE: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// `GIF87a` signature
pub const GIF87A_SIGNATURE: &[u8] = b"GIF87a";

/// `GIF89a` signature
pub const GIF89A_SIGNATURE: &[u8] = b"GIF89a";

/// WebP RIFF signature (bytes 0-3)
pub const WEBP_RIFF: &[u8] = b"RIFF";

/// WebP format identifier (bytes 8-11)
pub const WEBP_FORMAT: &[u8] = b"WEBP";

/// BMP signature (bytes 0-1)
pub const BMP_SIGNATURE: &[u8] = b"BM";

/// TIFF little-endian signature (bytes 0-1)
pub const TIFF_LE_SIGNATURE: &[u8] = b"II";

/// TIFF big-endian signature (bytes 0-1)
pub const TIFF_BE_SIGNATURE: &[u8] = b"MM";

/// TIFF version number (should be 42, bytes 2-3)
pub const TIFF_VERSION: u16 = 42;

// ============================================================================
// JPEG Markers
// ============================================================================

/// JPEG End of Image (EOI) marker
pub const JPEG_EOI: u8 = 0xD9;

/// JPEG Restart markers (RST0-RST7)
pub const JPEG_RST_START: u8 = 0xD0;
pub const JPEG_RST_END: u8 = 0xD7;

/// JPEG Temporary marker
pub const JPEG_TEM: u8 = 0x01;

/// JPEG Start of Frame (SOF) markers
/// These indicate the start of frame data and contain image dimensions
pub const JPEG_SOF_MARKERS: &[u8] = &[
    0xC0, // SOF0 (Baseline DCT)
    0xC1, // SOF1 (Extended sequential DCT)
    0xC2, // SOF2 (Progressive DCT)
    0xC3, // SOF3 (Lossless sequential)
    0xC5, // SOF5 (Differential sequential DCT)
    0xC6, // SOF6 (Differential progressive DCT)
    0xC7, // SOF7 (Differential lossless)
    0xC9, // SOF9 (Extended sequential arithmetic)
    0xCA, // SOF10 (Progressive arithmetic)
    0xCB, // SOF11 (Lossless arithmetic)
    0xCD, // SOF13 (Differential sequential arithmetic)
    0xCE, // SOF14 (Differential progressive arithmetic)
    0xCF, // SOF15 (Differential lossless arithmetic)
];

// ============================================================================
// PNG Chunk Types
// ============================================================================

/// PNG IHDR chunk type (Image Header)
pub const PNG_IHDR: &[u8] = b"IHDR";

// ============================================================================
// WebP Chunk Types
// ============================================================================

/// WebP VP8 chunk type (lossy)
pub const WEBP_VP8: &[u8] = b"VP8 ";

/// WebP VP8L chunk type (lossless)
pub const WEBP_VP8L: &[u8] = b"VP8L";

/// WebP VP8X chunk type (extended)
pub const WEBP_VP8X: &[u8] = b"VP8X";

/// WebP alpha channel flag bit mask (bit 4 of flags byte)
pub const WEBP_ALPHA_FLAG: u8 = 0x10;

// ============================================================================
// TIFF Tags
// ============================================================================

/// TIFF tag: `BitsPerSample` (258 / 0x0102)
pub const TIFF_TAG_BITS_PER_SAMPLE: u16 = 258;

/// TIFF tag: Compression (259 / 0x0103)
/// Note: Currently not used - compression extraction returns "TIFF" as placeholder
#[allow(dead_code)]
pub const TIFF_TAG_COMPRESSION: u16 = 259;

/// TIFF tag: `PhotometricInterpretation` (262 / 0x0106)
pub const TIFF_TAG_PHOTOMETRIC_INTERPRETATION: u16 = 262;

/// TIFF IFD entry type: SHORT (16-bit)
pub const TIFF_TYPE_SHORT: u16 = 3;

/// TIFF IFD entry type: LONG (32-bit)
pub const TIFF_TYPE_LONG: u16 = 4;

// ============================================================================
// TIFF PhotometricInterpretation Values
// ============================================================================

/// TIFF `PhotometricInterpretation`: `WhiteIsZero` (grayscale, inverted)
pub const TIFF_PHOTOMETRIC_WHITE_IS_ZERO: u32 = 0;

/// TIFF `PhotometricInterpretation`: `BlackIsZero` (grayscale)
pub const TIFF_PHOTOMETRIC_BLACK_IS_ZERO: u32 = 1;

/// TIFF `PhotometricInterpretation`: RGB
pub const TIFF_PHOTOMETRIC_RGB: u32 = 2;

/// TIFF `PhotometricInterpretation`: Palette color (indexed)
pub const TIFF_PHOTOMETRIC_PALETTE: u32 = 3;

/// TIFF `PhotometricInterpretation`: Transparency mask
pub const TIFF_PHOTOMETRIC_MASK: u32 = 4;

/// TIFF `PhotometricInterpretation`: CMYK
pub const TIFF_PHOTOMETRIC_CMYK: u32 = 5;

/// TIFF `PhotometricInterpretation`: YCbCr (typically converted to RGB for display)
pub const TIFF_PHOTOMETRIC_YCBCR: u32 = 6;

/// TIFF `PhotometricInterpretation`: CIE L*a*b*
pub const TIFF_PHOTOMETRIC_CIELAB: u32 = 8;

// ============================================================================
// BMP Compression Constants
// ============================================================================

/// BMP compression: `BI_RGB` (no compression)
pub const BMP_COMPRESSION_RGB: u32 = 0;

/// BMP compression: `BI_RLE8` (8-bit RLE)
pub const BMP_COMPRESSION_RLE8: u32 = 1;

/// BMP compression: `BI_RLE4` (4-bit RLE)
pub const BMP_COMPRESSION_RLE4: u32 = 2;

/// BMP compression: `BI_BITFIELDS` (bit fields)
pub const BMP_COMPRESSION_BITFIELDS: u32 = 3;

/// BMP compression: `BI_JPEG` (JPEG compression)
pub const BMP_COMPRESSION_JPEG: u32 = 4;

/// BMP compression: `BI_PNG` (PNG compression)
pub const BMP_COMPRESSION_PNG: u32 = 5;

// ============================================================================
// Color Type Names
// ============================================================================

/// Color type: 8-bit RGB (3 channels, 24-bit total)
pub const COLOR_TYPE_RGB8: &str = "Rgb8";

/// Color type: 8-bit RGBA (4 channels, 32-bit total)
pub const COLOR_TYPE_RGBA8: &str = "Rgba8";

/// Color type: 8-bit grayscale (1 channel, 8-bit total)
pub const COLOR_TYPE_L8: &str = "L8";

/// Color type: 8-bit grayscale with alpha (2 channels, 16-bit total)
pub const COLOR_TYPE_LA8: &str = "La8";

/// Color type: 8-bit CMYK (4 channels, 32-bit total)
pub const COLOR_TYPE_CMYK8: &str = "Cmyk8";

/// Color type: 8-bit indexed/palette color
pub const COLOR_TYPE_INDEXED8: &str = "Indexed8";

/// Color type: 4-bit indexed/palette color
pub const COLOR_TYPE_INDEXED4: &str = "Indexed4";

/// Color type: 1-bit indexed/palette color
pub const COLOR_TYPE_INDEXED1: &str = "Indexed1";

/// Color type: 16-bit RGB (typically 5-6-5 format)
pub const COLOR_TYPE_RGB16: &str = "Rgb16";

/// Color type: 16-bit grayscale
pub const COLOR_TYPE_L16: &str = "L16";

/// Color type: 16-bit grayscale with alpha
pub const COLOR_TYPE_LA16: &str = "La16";

/// Color type: 16-bit RGBA
pub const COLOR_TYPE_RGBA16: &str = "Rgba16";

/// Color type: Grayscale (generic, used in JPEG)
pub const COLOR_TYPE_GRAYSCALE: &str = "Grayscale";

// ============================================================================
// Chroma Subsampling Names (JPEG)
// ============================================================================

/// Chroma subsampling: 4:4:4 (no subsampling, full chroma resolution)
/// Y: 1x1, Cb: 1x1, Cr: 1x1
pub const SUBSAMPLING_444: &str = "4:4:4";

/// Chroma subsampling: 4:2:2 (horizontal subsampling)
/// Y: 2x1, Cb: 1x1, Cr: 1x1
pub const SUBSAMPLING_422: &str = "4:2:2";

/// Chroma subsampling: 4:2:0 (horizontal and vertical subsampling)
/// Y: 2x2, Cb: 1x1, Cr: 1x1
pub const SUBSAMPLING_420: &str = "4:2:0";

/// Chroma subsampling: 4:1:1 (heavy horizontal subsampling)
/// Y: 4x1, Cb: 1x1, Cr: 1x1
pub const SUBSAMPLING_411: &str = "4:1:1";

/// Chroma subsampling: Unknown (for unrecognized patterns)
pub const SUBSAMPLING_UNKNOWN: &str = "Unknown";

// ============================================================================
// Helper Functions
// ============================================================================

/// Check if a byte is a JPEG SOF marker
#[inline]
pub fn is_jpeg_sof_marker(marker: u8) -> bool {
    JPEG_SOF_MARKERS.contains(&marker)
}

/// Check if a byte is a JPEG restart marker (RST0-RST7)
#[inline]
pub fn is_jpeg_rst_marker(marker: u8) -> bool {
    (JPEG_RST_START..=JPEG_RST_END).contains(&marker)
}

/// Check if data starts with JPEG SOI marker
#[inline]
pub fn has_jpeg_signature(data_ref: &[u8]) -> bool {
    data_ref.len() >= 2 && data_ref[0] == JPEG_SOI[0] && data_ref[1] == JPEG_SOI[1]
}

/// Check if data starts with PNG signature
#[inline]
pub fn has_png_signature(data_ref: &[u8]) -> bool {
    data_ref.len() >= 8 && &data_ref[0..8] == PNG_SIGNATURE
}

/// Check if data starts with GIF signature (`GIF87a` or `GIF89a`)
#[inline]
pub fn has_gif_signature(data_ref: &[u8]) -> bool {
    data_ref.len() >= 6
        && (&data_ref[0..6] == GIF87A_SIGNATURE || &data_ref[0..6] == GIF89A_SIGNATURE)
}

/// Check if data starts with WebP signature
#[inline]
pub fn has_webp_signature(data_ref: &[u8]) -> bool {
    data_ref.len() >= 12 && &data_ref[0..4] == WEBP_RIFF && &data_ref[8..12] == WEBP_FORMAT
}

/// Check if data starts with BMP signature
#[inline]
pub fn has_bmp_signature(data_ref: &[u8]) -> bool {
    data_ref.len() >= 2 && &data_ref[0..2] == BMP_SIGNATURE
}

/// Check if data starts with TIFF signature (little-endian or big-endian)
#[inline]
pub fn has_tiff_signature(data_ref: &[u8]) -> bool {
    data_ref.len() >= 2
        && (&data_ref[0..2] == TIFF_LE_SIGNATURE || &data_ref[0..2] == TIFF_BE_SIGNATURE)
}
