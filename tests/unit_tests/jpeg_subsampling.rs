//! Tests for JPEG subsampling parser (SOF parsing)

use zahirscan::parsers::image::metadata::{FormatMetadata, ImageFormat};

/// Helper to create a minimal valid JPEG header
fn create_jpeg_header() -> Vec<u8> {
    vec![0xFF, 0xD8] // SOI marker
}

/// Helper to create a SOF segment
fn create_sof_segment(
    marker: u8,                  // SOF marker (0xC0-0xCF)
    precision: u8,               // Precision (usually 8)
    height: u16,                 // Image height
    width: u16,                  // Image width
    nf: u8,                      // Number of components
    components: &[(u8, u8, u8)], // (component_id, H, V) for each component
) -> Vec<u8> {
    let mut seg = vec![0xFF, marker];
    let seg_len = (8 + nf as usize * 3) as u16;
    seg.extend_from_slice(&seg_len.to_be_bytes());
    seg.push(precision);
    seg.extend_from_slice(&height.to_be_bytes());
    seg.extend_from_slice(&width.to_be_bytes());
    seg.push(nf);
    for (cid, h, v) in components {
        seg.push(*cid);
        seg.push((h << 4) | v); // H in high 4 bits, V in low 4 bits
        seg.push(0); // Tqi (quantization table selector, not used)
    }
    seg
}

#[test]
fn test_jpeg_4_4_4_subsampling() {
    // 4:4:4 - Y 1x1, Cb 1x1, Cr 1x1
    let mut jpeg = create_jpeg_header();
    jpeg.extend_from_slice(&create_sof_segment(
        0xC0, // SOF0 (baseline)
        8,
        100,
        100,
        3,
        &[(1, 1, 1), (2, 1, 1), (3, 1, 1)], // Y, Cb, Cr all 1x1
    ));
    jpeg.push(0xFF);
    jpeg.push(0xD9); // EOI

    let format = ImageFormat::Jpeg;
    let result = format.extract_chroma_subsampling(&jpeg);
    assert_eq!(result, Some("4:4:4".to_string()));
}

#[test]
fn test_jpeg_4_2_2_subsampling() {
    // 4:2:2 - Y 2x1, Cb 1x1, Cr 1x1
    let mut jpeg = create_jpeg_header();
    jpeg.extend_from_slice(&create_sof_segment(
        0xC0,
        8,
        100,
        100,
        3,
        &[(1, 2, 1), (2, 1, 1), (3, 1, 1)], // Y 2x1, Cb/Cr 1x1
    ));
    jpeg.push(0xFF);
    jpeg.push(0xD9);

    let format = ImageFormat::Jpeg;
    let result = format.extract_chroma_subsampling(&jpeg);
    assert_eq!(result, Some("4:2:2".to_string()));
}

#[test]
fn test_jpeg_4_2_0_subsampling() {
    // 4:2:0 - Y 2x2, Cb 1x1, Cr 1x1
    let mut jpeg = create_jpeg_header();
    jpeg.extend_from_slice(&create_sof_segment(
        0xC0,
        8,
        100,
        100,
        3,
        &[(1, 2, 2), (2, 1, 1), (3, 1, 1)], // Y 2x2, Cb/Cr 1x1
    ));
    jpeg.push(0xFF);
    jpeg.push(0xD9);

    let format = ImageFormat::Jpeg;
    let result = format.extract_chroma_subsampling(&jpeg);
    assert_eq!(result, Some("4:2:0".to_string()));
}

#[test]
fn test_jpeg_4_1_1_subsampling() {
    // 4:1:1 - Y 4x1, Cb 1x1, Cr 1x1
    let mut jpeg = create_jpeg_header();
    jpeg.extend_from_slice(&create_sof_segment(
        0xC0,
        8,
        100,
        100,
        3,
        &[(1, 4, 1), (2, 1, 1), (3, 1, 1)], // Y 4x1, Cb/Cr 1x1
    ));
    jpeg.push(0xFF);
    jpeg.push(0xD9);

    let format = ImageFormat::Jpeg;
    let result = format.extract_chroma_subsampling(&jpeg);
    assert_eq!(result, Some("4:1:1".to_string()));
}

#[test]
fn test_jpeg_grayscale() {
    // Grayscale - single component
    let mut jpeg = create_jpeg_header();
    jpeg.extend_from_slice(&create_sof_segment(
        0xC0,
        8,
        100,
        100,
        1,
        &[(1, 1, 1)], // Only Y component
    ));
    jpeg.push(0xFF);
    jpeg.push(0xD9);

    let format = ImageFormat::Jpeg;
    let result = format.extract_chroma_subsampling(&jpeg);
    assert_eq!(result, Some("Grayscale".to_string()));
}

#[test]
fn test_jpeg_invalid_header() {
    // Invalid JPEG (no SOI marker)
    let jpeg = vec![0xFF, 0xD9]; // Just EOI, no SOI

    let format = ImageFormat::Jpeg;
    let result = format.extract_chroma_subsampling(&jpeg);
    assert_eq!(result, None);
}

#[test]
fn test_jpeg_too_short() {
    // Too short to be valid
    let jpeg = vec![0xFF];

    let format = ImageFormat::Jpeg;
    let result = format.extract_chroma_subsampling(&jpeg);
    assert_eq!(result, None);
}

#[test]
fn test_jpeg_no_sof_marker() {
    // Valid JPEG header but no SOF marker
    let mut jpeg = create_jpeg_header();
    jpeg.push(0xFF);
    jpeg.push(0xE0); // APP0 marker (not SOF)
    jpeg.extend_from_slice(&[0x00, 0x10]); // Length
    jpeg.push(0xFF);
    jpeg.push(0xD9); // EOI

    let format = ImageFormat::Jpeg;
    let result = format.extract_chroma_subsampling(&jpeg);
    assert_eq!(result, None);
}

#[test]
fn test_jpeg_progressive_sof() {
    // Progressive JPEG (SOF2)
    let mut jpeg = create_jpeg_header();
    jpeg.extend_from_slice(&create_sof_segment(
        0xC2, // SOF2 (progressive)
        8,
        100,
        100,
        3,
        &[(1, 2, 2), (2, 1, 1), (3, 1, 1)], // 4:2:0
    ));
    jpeg.push(0xFF);
    jpeg.push(0xD9);

    let format = ImageFormat::Jpeg;
    let result = format.extract_chroma_subsampling(&jpeg);
    assert_eq!(result, Some("4:2:0".to_string()));
}

#[test]
fn test_jpeg_with_fill_bytes() {
    // JPEG with fill bytes (0xFF 0xFF) before marker
    let mut jpeg = create_jpeg_header();
    jpeg.push(0xFF);
    jpeg.push(0xFF); // Fill bytes
    jpeg.extend_from_slice(&create_sof_segment(
        0xC0,
        8,
        100,
        100,
        3,
        &[(1, 1, 1), (2, 1, 1), (3, 1, 1)], // 4:4:4
    ));
    jpeg.push(0xFF);
    jpeg.push(0xD9);

    let format = ImageFormat::Jpeg;
    let result = format.extract_chroma_subsampling(&jpeg);
    assert_eq!(result, Some("4:4:4".to_string()));
}
