//! Media parsers: audio, image, video.

pub mod audio;
pub mod image;
pub mod video;

pub use audio::{extract_audio_metadata, extract_audio_templates};
pub use image::{extract_image_metadata, extract_image_templates};
pub use video::{extract_video_metadata, extract_video_templates};
