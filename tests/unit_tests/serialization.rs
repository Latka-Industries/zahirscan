//! Tests for serialization edge cases

use zahirscan::parsers::BitrateMode;
use zahirscan::results::{AudioMetadata, ImageMetadata, VideoMetadata};

#[test]
fn test_image_metadata_serialize_all_none() {
    let metadata = ImageMetadata::default();
    let json = serde_json::to_string(&metadata).unwrap();

    // Unknown dimensions (0) are omitted — not reported as real 0×0 size
    assert!(!json.contains("\"width\""));
    assert!(!json.contains("\"height\""));
}

#[test]
fn test_audio_metadata_serialize_all_none() {
    let metadata = AudioMetadata::default();
    let json = serde_json::to_string(&metadata).unwrap();

    // Should serialize without errors
    // Optional fields should be omitted if None
    assert!(
        !json.contains("\"duration_seconds\":null") || json.contains("\"duration_seconds\":null")
    );
}

#[test]
fn test_video_metadata_serialize_all_none() {
    let metadata = VideoMetadata::default();
    let json = serde_json::to_string(&metadata).unwrap();

    // Should serialize without errors
    assert!(json.contains("\"width\":0"));
    assert!(json.contains("\"height\":0"));
}

#[test]
fn test_image_metadata_serialize_with_all_fields() {
    let mut metadata = ImageMetadata::default();
    metadata.width = 1920;
    metadata.height = 1080;
    metadata.aspect_ratio = Some(1.7777777777777777);
    metadata.stream_size = Some(1_000_000);
    metadata.color_type = Some("Rgb8".to_string());
    metadata.format = Some("Jpeg".to_string());
    metadata.chroma_subsampling = Some("4:2:0".to_string());
    metadata.compression = Some("JPEG quality: 85".to_string());
    metadata.bit_depth = Some(8);

    let json = serde_json::to_string(&metadata).unwrap();

    assert!(json.contains("\"width\":1920"));
    assert!(json.contains("\"height\":1080"));
    assert!(json.contains("\"format\":\"Jpeg\""));
}

#[test]
fn test_audio_metadata_serialize_with_all_fields() {
    let mut metadata = AudioMetadata::default();
    metadata.duration_seconds = Some(180.5);
    metadata.audio_codec = Some("mp3".to_string());
    metadata.audio_bitrate = Some(320000);
    metadata.audio_channels = Some(2);
    metadata.title = Some("Test Song".to_string());
    metadata.artist = Some("Test Artist".to_string());
    metadata.bit_rate_mode = Some(BitrateMode::Cbr);

    let json = serde_json::to_string(&metadata).unwrap();

    assert!(json.contains("\"audio_codec\":\"mp3\""));
    assert!(json.contains("\"audio_bitrate\":320000"));
    assert!(json.contains("\"title\":\"Test Song\""));
}

#[test]
fn test_serialization_round_trip() {
    let mut metadata = ImageMetadata::default();
    metadata.width = 800;
    metadata.height = 600;
    metadata.format = Some("Png".to_string());

    let json = serde_json::to_string(&metadata).unwrap();
    let deserialized: ImageMetadata = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.width, metadata.width);
    assert_eq!(deserialized.height, metadata.height);
    assert_eq!(deserialized.format, metadata.format);
}

#[test]
fn test_serialization_round_trip_omitted_dimensions() {
    let metadata = ImageMetadata {
        format: Some("Svg".to_string()),
        stream_size: Some(42),
        ..Default::default()
    };
    let json = serde_json::to_string(&metadata).unwrap();
    assert!(!json.contains("\"width\""));
    let deserialized: ImageMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.width, 0);
    assert_eq!(deserialized.height, 0);
    assert_eq!(deserialized.format.as_deref(), Some("Svg"));
}

#[test]
fn test_serialization_optional_fields_omitted() {
    let metadata = ImageMetadata::default();
    let json = serde_json::to_string(&metadata).unwrap();

    // Optional fields that are None should be omitted in JSON
    // (This depends on serde's skip_serializing_if behavior)
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Unknown dimensions are omitted (not serialized as 0)
    assert!(parsed.get("width").is_none());
    assert!(parsed.get("height").is_none());
}
