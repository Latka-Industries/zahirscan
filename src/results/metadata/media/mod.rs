//! Media metadata structures (images, videos, audio)

pub mod audio;
pub mod image;
pub mod video;

pub use audio::AudioMetadata;
pub use image::ImageMetadata;
pub use video::VideoMetadata;
