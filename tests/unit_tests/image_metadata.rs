//! Tests for image metadata extraction

use zahirscan::parsers::image::metadata::{FormatMetadata, ImageFormat, format_from_string};

#[test]
fn test_format_from_string_jpeg() {
    assert_eq!(format_from_string("Jpeg"), Some(ImageFormat::Jpeg));
    assert_eq!(format_from_string("jpeg"), Some(ImageFormat::Jpeg));
    assert_eq!(format_from_string("JPEG"), Some(ImageFormat::Jpeg));
    assert_eq!(format_from_string("Jpg"), Some(ImageFormat::Jpeg));
    assert_eq!(format_from_string("jpg"), Some(ImageFormat::Jpeg));
}

#[test]
fn test_format_from_string_png() {
    assert_eq!(format_from_string("Png"), Some(ImageFormat::Png));
    assert_eq!(format_from_string("png"), Some(ImageFormat::Png));
    assert_eq!(format_from_string("PNG"), Some(ImageFormat::Png));
}

#[test]
fn test_format_from_string_tiff() {
    assert_eq!(format_from_string("Tiff"), Some(ImageFormat::Tiff));
    assert_eq!(format_from_string("tiff"), Some(ImageFormat::Tiff));
    assert_eq!(format_from_string("TIFF"), Some(ImageFormat::Tiff));
    assert_eq!(format_from_string("Tif"), Some(ImageFormat::Tiff));
    assert_eq!(format_from_string("tif"), Some(ImageFormat::Tiff));
}

#[test]
fn test_format_from_string_other_formats() {
    assert_eq!(format_from_string("Gif"), Some(ImageFormat::Gif));
    assert_eq!(format_from_string("WebP"), Some(ImageFormat::WebP));
    assert_eq!(format_from_string("Bmp"), Some(ImageFormat::Bmp));
}

#[test]
fn test_format_from_string_invalid() {
    assert_eq!(format_from_string("invalid"), None);
    assert_eq!(format_from_string(""), None);
    assert_eq!(format_from_string("xyz"), None);
}

#[test]
fn test_tiff_color_type_extraction_minimal_rgb() {
    // Minimal valid TIFF header with RGB PhotometricInterpretation (value 2)
    // Little-endian TIFF signature: II (0x4949) + 42 (0x002A)
    // IFD offset: 8 (0x08000000 in little-endian)
    // IFD entry count: 1 (0x0100 in little-endian)
    // IFD entry for PhotometricInterpretation (tag 262 = 0x0601):
    //   Tag: 0x0601 (PhotometricInterpretation)
    //   Type: 3 (SHORT)
    //   Count: 1
    //   Value: 2 (RGB) stored in bytes 8-9
    let tiff_data = vec![
        0x49, 0x49, // Little-endian signature
        0x2A, 0x00, // Version 42
        0x08, 0x00, 0x00, 0x00, // IFD offset at byte 8
        // IFD starts at byte 8:
        0x01, 0x00, // Entry count: 1
        // First IFD entry:
        0x06, 0x01, // Tag: PhotometricInterpretation (262)
        0x03, 0x00, // Type: SHORT (3)
        0x01, 0x00, 0x00, 0x00, // Count: 1
        0x02, 0x00, 0x00, 0x00, // Value: 2 (RGB) - stored in first 2 bytes
        0x00, 0x00, 0x00, 0x00, // Next IFD offset: 0 (end)
    ];

    let result = ImageFormat::Tiff.extract_color_type(&tiff_data);
    assert_eq!(result, Some("Rgb8".to_string()));
}

#[test]
fn test_tiff_color_type_extraction_grayscale() {
    // Minimal valid TIFF header with WhiteIsZero PhotometricInterpretation (value 0)
    let tiff_data = vec![
        0x49, 0x49, // Little-endian signature
        0x2A, 0x00, // Version 42
        0x08, 0x00, 0x00, 0x00, // IFD offset at byte 8
        // IFD starts at byte 8:
        0x01, 0x00, // Entry count: 1
        // First IFD entry:
        0x06, 0x01, // Tag: PhotometricInterpretation (262)
        0x03, 0x00, // Type: SHORT (3)
        0x01, 0x00, 0x00, 0x00, // Count: 1
        0x00, 0x00, 0x00, 0x00, // Value: 0 (WhiteIsZero) - stored in first 2 bytes
        0x00, 0x00, 0x00, 0x00, // Next IFD offset: 0 (end)
    ];

    let result = ImageFormat::Tiff.extract_color_type(&tiff_data);
    assert_eq!(result, Some("L8".to_string()));
}

#[test]
fn test_tiff_color_type_extraction_big_endian() {
    // Big-endian TIFF header with RGB PhotometricInterpretation
    // For big-endian SHORT values, the value is in bytes 10-11 (high bytes)
    let tiff_data = vec![
        0x4D, 0x4D, // Big-endian signature
        0x00, 0x2A, // Version 42
        0x00, 0x00, 0x00, 0x08, // IFD offset at byte 8
        // IFD starts at byte 8:
        0x00, 0x01, // Entry count: 1
        // First IFD entry:
        0x01, 0x06, // Tag: PhotometricInterpretation (262)
        0x00, 0x03, // Type: SHORT (3)
        0x00, 0x00, 0x00, 0x01, // Count: 1
        0x00, 0x00, 0x00,
        0x02, // Value: 2 (RGB) - for big-endian, SHORT value is in bytes 10-11
        0x00, 0x00, 0x00, 0x00, // Next IFD offset: 0 (end)
    ];

    let result = ImageFormat::Tiff.extract_color_type(&tiff_data);
    assert_eq!(result, Some("Rgb8".to_string()));
}

#[test]
fn test_tiff_color_type_extraction_fallback_to_rgb() {
    // TIFF with invalid signature - should fallback to RGB
    let tiff_data = vec![0x00, 0x00, 0x00, 0x00];

    let result = ImageFormat::Tiff.extract_color_type(&tiff_data);
    // Should return Some("Rgb8") as fallback, not None
    assert_eq!(result, Some("Rgb8".to_string()));
}

#[test]
fn test_tiff_color_type_extraction_too_small() {
    // TIFF data too small to be valid
    let tiff_data = vec![0x49, 0x49];

    let result = ImageFormat::Tiff.extract_color_type(&tiff_data);
    // Should return Some("Rgb8") as fallback, not None
    assert_eq!(result, Some("Rgb8".to_string()));
}

#[test]
fn test_tiff_color_type_extraction_mask() {
    // TIFF with MASK PhotometricInterpretation (value 4)
    let tiff_data = vec![
        0x49, 0x49, // Little-endian signature
        0x2A, 0x00, // Version 42
        0x08, 0x00, 0x00, 0x00, // IFD offset at byte 8
        // IFD starts at byte 8:
        0x01, 0x00, // Entry count: 1
        // First IFD entry:
        0x06, 0x01, // Tag: PhotometricInterpretation (262)
        0x03, 0x00, // Type: SHORT (3)
        0x01, 0x00, 0x00, 0x00, // Count: 1
        0x04, 0x00, 0x00, 0x00, // Value: 4 (MASK) - stored in first 2 bytes
        0x00, 0x00, 0x00, 0x00, // Next IFD offset: 0 (end)
    ];

    let result = ImageFormat::Tiff.extract_color_type(&tiff_data);
    assert_eq!(result, Some("L8".to_string()));
}

#[test]
fn test_tiff_color_type_extraction_cielab() {
    // TIFF with CIELAB PhotometricInterpretation (value 8)
    let tiff_data = vec![
        0x49, 0x49, // Little-endian signature
        0x2A, 0x00, // Version 42
        0x08, 0x00, 0x00, 0x00, // IFD offset at byte 8
        // IFD starts at byte 8:
        0x01, 0x00, // Entry count: 1
        // First IFD entry:
        0x06, 0x01, // Tag: PhotometricInterpretation (262)
        0x03, 0x00, // Type: SHORT (3)
        0x01, 0x00, 0x00, 0x00, // Count: 1
        0x08, 0x00, 0x00, 0x00, // Value: 8 (CIELAB) - stored in first 2 bytes
        0x00, 0x00, 0x00, 0x00, // Next IFD offset: 0 (end)
    ];

    let result = ImageFormat::Tiff.extract_color_type(&tiff_data);
    assert_eq!(result, Some("Rgb8".to_string()));
}

#[test]
fn test_tiff_color_type_extraction_always_returns_some() {
    // Test that extract_color_type never returns None for TIFF
    // This is the key fix: even invalid TIFFs should return Some(String), not None

    // Empty data
    let result = ImageFormat::Tiff.extract_color_type(&[]);
    assert!(result.is_some());
    assert_eq!(result, Some("Rgb8".to_string()));

    // Invalid signature
    let result = ImageFormat::Tiff.extract_color_type(&[0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(result.is_some());
    assert_eq!(result, Some("Rgb8".to_string()));

    // Valid signature but invalid IFD offset
    let result =
        ImageFormat::Tiff.extract_color_type(&[0x49, 0x49, 0x2A, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(result.is_some());
    assert_eq!(result, Some("Rgb8".to_string()));
}
