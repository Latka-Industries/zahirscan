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
pub fn process(
    stats: &mut ParseResult,
    mmap: &Mmap,
    config: &RuntimeConfig,
) -> Result<MiningResult> {
    match stats.file_type {
        FileType::Image => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            image_metadata,
            extract_image_metadata(mmap, stats, config),
            crate::results::ImageMetadata,
            FileType::Image,
            extract_image_templates(mmap, stats, config)
        ),
        FileType::Video => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            video_metadata,
            extract_video_metadata(mmap, stats, config),
            crate::results::VideoMetadata,
            FileType::Video,
            extract_video_templates(mmap, stats, config)
        ),
        FileType::Audio => crate::process_with_metadata!(
            stats,
            mmap,
            config,
            audio_metadata,
            extract_audio_metadata(mmap, stats, config),
            crate::results::AudioMetadata,
            FileType::Audio,
            extract_audio_templates(mmap, stats, config)
        ),
        _ => unreachable!("media::process called with {:?}", stats.file_type),
    }
}
