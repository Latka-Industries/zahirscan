//! MP3-specific parsing utilities, including LAME tag reading

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use crate::parsers::FileType;
use crate::parsers::media_helpers::BitrateMode;
use crate::utils::filetypes::is_codec_for_file_type;

/// Check if a codec string represents MP3
/// Verifies against `FILE_EXTENSION_MAP` so we only treat known audio codecs as MP3.
pub fn is_mp3_codec(codec_ref: &str) -> bool {
    is_codec_for_file_type(codec_ref, FileType::Audio) && codec_ref.to_lowercase().contains("mp3")
}

/// MP3 frame header constants
mod mp3_constants {
    /// MPEG-1 version identifier
    pub const MPEG_VERSION_1: u8 = 0x03;

    /// Channel mode: Mono (single channel)
    pub const CHANNEL_MODE_MONO: u8 = 0x03;

    /// Frame sync pattern: first byte must be 0xFF
    pub const FRAME_SYNC_BYTE: u8 = 0xFF;

    /// Frame sync mask: first 3 bits of second byte must be 0xE0 (11100000)
    pub const FRAME_SYNC_MASK: u8 = 0xE0;

    /// Protection bit mask (bit 1 of frame header byte 1)
    pub const PROTECTION_BIT_MASK: u8 = 0x01;

    /// MPEG version mask (bits 3-4 of frame header byte 1)
    pub const MPEG_VERSION_MASK: u8 = 0x03;

    /// Channel mode mask (bits 6-7 of frame header byte 3)
    pub const CHANNEL_MODE_MASK: u8 = 0x03;

    /// VBR method mask (low 4 bits of revision+VBR byte)
    pub const VBR_METHOD_MASK: u8 = 0x0F;

    /// Xing/Info header flags bit masks
    pub mod xing_flags {
        /// Frame count flag (bit 0)
        pub const FRAME_COUNT: u8 = 0x01;
        /// Byte count flag (bit 1)
        pub const BYTE_COUNT: u8 = 0x02;
        /// TOC flag (bit 2)
        pub const TOC: u8 = 0x04;
        /// Quality flag (bit 3)
        pub const QUALITY: u8 = 0x08;
    }

    /// LAME VBR method values
    pub mod vbr_method {
        pub const CBR: u8 = 1;
        pub const ABR: u8 = 2;
        pub const VBR_OLD: u8 = 3;
        pub const VBR_NEW: u8 = 4;
        pub const VBR_MT: u8 = 5;
        pub const VBR_MTRH: u8 = 6;
        pub const VBR_ABR_ALT: u8 = 8;
    }

    /// Side info sizes (in bytes) for different MPEG versions and channel modes
    pub mod side_info_size {
        /// MPEG-1 Layer III mono
        pub const MPEG1_MONO: usize = 17;
        /// MPEG-1 Layer III stereo/dual/joint
        pub const MPEG1_STEREO: usize = 32;
        /// MPEG-2/2.5 Layer III mono
        pub const MPEG2_MONO: usize = 9;
        /// MPEG-2/2.5 Layer III stereo/dual/joint
        pub const MPEG2_STEREO: usize = 17;
    }
}

/// Skip `ID3v2` if present, otherwise rewind to file start. Returns `None` on I/O failure.
fn skip_id3v2_prefix(file_ref: &mut File) -> Option<()> {
    let mut header = [0u8; 10];
    file_ref.read_exact(&mut header).ok()?;
    if &header[0..3] == b"ID3" {
        let size = ((header[6] as u32) << 21)
            | ((header[7] as u32) << 14)
            | ((header[8] as u32) << 7)
            | (header[9] as u32);
        let id3_size = 10 + u64::from(size);
        file_ref.seek(SeekFrom::Start(id3_size)).ok()?;
    } else {
        file_ref.seek(SeekFrom::Start(0)).ok()?;
    }
    Some(())
}

fn mp3_frame_sync_valid(frame_header: [u8; 4]) -> bool {
    frame_header[0] == mp3_constants::FRAME_SYNC_BYTE
        && (frame_header[1] & mp3_constants::FRAME_SYNC_MASK) == mp3_constants::FRAME_SYNC_MASK
}

fn side_info_byte_count(mpeg_version: u8, channel_mode: u8) -> usize {
    match mpeg_version {
        mp3_constants::MPEG_VERSION_1 => {
            if channel_mode == mp3_constants::CHANNEL_MODE_MONO {
                mp3_constants::side_info_size::MPEG1_MONO
            } else {
                mp3_constants::side_info_size::MPEG1_STEREO
            }
        }
        _ => {
            if channel_mode == mp3_constants::CHANNEL_MODE_MONO {
                mp3_constants::side_info_size::MPEG2_MONO
            } else {
                mp3_constants::side_info_size::MPEG2_STEREO
            }
        }
    }
}

/// After the first frame header: skip CRC (if any), side info, then read Xing/LAME VBR byte.
fn bitrate_mode_after_first_frame(
    file_ref: &mut File,
    frame_header: [u8; 4],
) -> Option<BitrateMode> {
    let has_crc = (frame_header[1] & mp3_constants::PROTECTION_BIT_MASK) == 0;
    if has_crc {
        file_ref.seek(SeekFrom::Current(2)).ok()?;
    }
    let mpeg_version = (frame_header[1] >> 3) & mp3_constants::MPEG_VERSION_MASK;
    let channel_mode = (frame_header[3] >> 6) & mp3_constants::CHANNEL_MODE_MASK;
    let side_n = side_info_byte_count(mpeg_version, channel_mode);
    let side_off = i64::try_from(side_n).unwrap_or(i64::MAX);
    file_ref.seek(SeekFrom::Current(side_off)).ok()?;

    let mut xing_header = [0u8; 4];
    file_ref.read_exact(&mut xing_header).ok()?;
    if &xing_header != b"Xing" && &xing_header != b"Info" {
        return None;
    }

    let mut flags = [0u8; 4];
    file_ref.read_exact(&mut flags).ok()?;

    let mut skip_bytes = 0u64;
    if (flags[3] & mp3_constants::xing_flags::FRAME_COUNT) != 0 {
        skip_bytes += 4;
    }
    if (flags[3] & mp3_constants::xing_flags::BYTE_COUNT) != 0 {
        skip_bytes += 4;
    }
    if (flags[3] & mp3_constants::xing_flags::TOC) != 0 {
        skip_bytes += 100;
    }
    if (flags[3] & mp3_constants::xing_flags::QUALITY) != 0 {
        skip_bytes += 4;
    }
    file_ref
        .seek(SeekFrom::Current(skip_bytes.cast_signed()))
        .ok()?;

    let mut lame_version = [0u8; 9];
    file_ref.read_exact(&mut lame_version).ok()?;
    if &lame_version[0..4] != b"LAME" {
        return None;
    }

    let mut revision_vbr_byte = [0u8; 1];
    file_ref.read_exact(&mut revision_vbr_byte).ok()?;
    let vbr_method = revision_vbr_byte[0] & mp3_constants::VBR_METHOD_MASK;

    match vbr_method {
        mp3_constants::vbr_method::CBR => Some(BitrateMode::Cbr),
        mp3_constants::vbr_method::ABR => Some(BitrateMode::Abr),
        mp3_constants::vbr_method::VBR_OLD
        | mp3_constants::vbr_method::VBR_NEW
        | mp3_constants::vbr_method::VBR_MT
        | mp3_constants::vbr_method::VBR_MTRH
        | mp3_constants::vbr_method::VBR_ABR_ALT => Some(BitrateMode::Vbr),
        _ => None,
    }
}

/// Read LAME tag from MP3 file to determine bitrate mode (CBR/VBR/ABR)
///
/// The LAME tag is embedded in the first MP3 frame's Xing/Info header.
/// Returns the bitrate mode if successfully parsed, None otherwise.
///
/// Note: Many MP3 files (especially CBR) don't have a Xing/Info header,
/// so this will return None for those files.
pub fn read_lame_tag_bitrate_mode(file_path_ref: &str) -> Option<BitrateMode> {
    let mut file = File::open(file_path_ref).ok()?;
    skip_id3v2_prefix(&mut file)?;

    let mut frame_header = [0u8; 4];
    file.read_exact(&mut frame_header).ok()?;
    if !mp3_frame_sync_valid(frame_header) {
        return None;
    }
    bitrate_mode_after_first_frame(&mut file, frame_header)
}
