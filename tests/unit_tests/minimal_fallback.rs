//! Tests for MinimalFallback trait implementations

use zahirscan::results::{AudioMetadata, ImageMetadata, VideoMetadata, create_minimal_fallback};

#[test]
fn test_image_metadata_minimal_fallback() {
    let file_size = 1024;
    let metadata = create_minimal_fallback::<ImageMetadata>(file_size);

    assert_eq!(metadata.width, 0);
    assert_eq!(metadata.height, 0);
    assert_eq!(metadata.stream_size, Some(file_size));
    assert_eq!(metadata.aspect_ratio, None);
    assert_eq!(metadata.color_type, None);
    assert_eq!(metadata.format, None);
}

#[test]
fn test_audio_metadata_minimal_fallback() {
    let file_size = 2048;
    let metadata = create_minimal_fallback::<AudioMetadata>(file_size);

    assert_eq!(metadata.duration_seconds, None);
    assert_eq!(metadata.audio_codec, None);
    assert_eq!(metadata.audio_bitrate, None);
    assert_eq!(metadata.audio_channels, None);
    assert_eq!(metadata.title, None);
    assert_eq!(metadata.artist, None);
    // AudioMetadata no longer has stream_size field
}

#[test]
fn test_video_metadata_minimal_fallback() {
    let file_size = 4096;
    let metadata = create_minimal_fallback::<VideoMetadata>(file_size);

    assert_eq!(metadata.width, 0);
    assert_eq!(metadata.height, 0);
    assert_eq!(metadata.stream_size, Some(file_size));
    assert_eq!(metadata.duration_seconds, None);
    assert_eq!(metadata.video_codec, None);
    assert_eq!(metadata.audio_codec, None);
}

#[test]
fn test_minimal_fallback_zero_size() {
    let metadata = create_minimal_fallback::<ImageMetadata>(0);
    assert_eq!(metadata.stream_size, Some(0));
}

#[test]
fn test_minimal_fallback_large_size() {
    let large_size = 1_000_000_000; // 1 GB
    let metadata = create_minimal_fallback::<ImageMetadata>(large_size);
    assert_eq!(metadata.stream_size, Some(large_size));
}
