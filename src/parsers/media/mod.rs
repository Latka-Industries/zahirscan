//! Media parsers: audio, image, video.

use crate::config::RuntimeConfig;
use crate::parsers::{FileType, ParseResult};
use crate::results::MiningResult;
use anyhow::Result;
use memmap2::Mmap;

pub mod audio;
pub mod image;
pub mod video;

pub use audio::{extract_audio_metadata, extract_audio_templates};
pub use image::{extract_image_metadata, extract_image_templates};
pub use video::{extract_video_metadata, extract_video_templates};

/// Dispatch by file type; fills the appropriate metadata field and returns templates.
///
/// # Errors
///
/// Propagates errors from the image, video, or audio metadata/template extractors for the active [`crate::parsers::FileType`].
pub fn process(
    stats_ref: &mut ParseResult,
    mmap_ref: &Mmap,
    config_ref: &RuntimeConfig,
) -> Result<MiningResult> {
    match stats_ref.file_type {
        FileType::Image => crate::process_with_metadata!(
            stats_ref,
            mmap_ref,
            config_ref,
            image_metadata,
            extract_image_metadata(mmap_ref, stats_ref, config_ref),
            crate::results::ImageMetadata,
            FileType::Image,
            extract_image_templates(mmap_ref, stats_ref, config_ref)
        ),
        FileType::Video => crate::process_with_metadata!(
            stats_ref,
            mmap_ref,
            config_ref,
            video_metadata,
            extract_video_metadata(mmap_ref, stats_ref, config_ref),
            crate::results::VideoMetadata,
            FileType::Video,
            extract_video_templates(mmap_ref, stats_ref, config_ref)
        ),
        FileType::Audio => crate::process_with_metadata!(
            stats_ref,
            mmap_ref,
            config_ref,
            audio_metadata,
            extract_audio_metadata(mmap_ref, stats_ref, config_ref),
            crate::results::AudioMetadata,
            FileType::Audio,
            extract_audio_templates(mmap_ref, stats_ref, config_ref)
        ),
        _ => unreachable!("media::process called with {:?}", stats_ref.file_type),
    }
}
