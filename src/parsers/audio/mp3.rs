//! MP3-specific parsing utilities, including LAME tag reading

use crate::parsers::FileType;
use crate::parsers::media_helpers::BitrateMode;
use crate::tools::get_extensions_for_file_type;

/// Check if a codec string represents MP3
/// Verifies against extension map from tools.rs for consistency
pub fn is_mp3_codec(codec: &str) -> bool {
    let audio_extensions = get_extensions_for_file_type(FileType::Audio);
    audio_extensions.contains(&"mp3") && codec.to_lowercase().contains("mp3")
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

/// Read LAME tag from MP3 file to determine bitrate mode (CBR/VBR/ABR)
///
/// The LAME tag is embedded in the first MP3 frame's Xing/Info header.
/// Returns the bitrate mode if successfully parsed, None otherwise.
///
/// Note: Many MP3 files (especially CBR) don't have a Xing/Info header,
/// so this will return None for those files.
pub fn read_lame_tag_bitrate_mode(file_path: &str) -> Option<BitrateMode> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let mut file = match File::open(file_path) {
        Ok(f) => f,
        Err(_) => return None,
    };

    // Skip ID3v2 tag if present (starts with "ID3")
    let mut header = [0u8; 10];
    if file.read_exact(&mut header).is_ok() {
        if &header[0..3] == b"ID3" {
            // ID3v2 tag present - skip it
            // Size is stored in bytes 6-9 as synchsafe integers
            let size = ((header[6] as u32) << 21)
                | ((header[7] as u32) << 14)
                | ((header[8] as u32) << 7)
                | (header[9] as u32);
            let id3_size = 10 + size as u64;
            if file.seek(SeekFrom::Start(id3_size)).is_err() {
                return None;
            }
        } else {
            // Not ID3v2, rewind to start
            if file.seek(SeekFrom::Start(0)).is_err() {
                return None;
            }
        }
    } else {
        // File too small or read error
        return None;
    }

    // Read first MP3 frame header (4 bytes)
    let mut frame_header = [0u8; 4];
    if file.read_exact(&mut frame_header).is_err() {
        return None;
    }

    // Check MP3 frame sync (first 11 bits should be all 1s)
    if frame_header[0] != mp3_constants::FRAME_SYNC_BYTE
        || (frame_header[1] & mp3_constants::FRAME_SYNC_MASK) != mp3_constants::FRAME_SYNC_MASK
    {
        return None; // Not a valid MP3 frame
    }

    // Check protection bit (bit 1 of byte 1)
    // If protection bit is 0, there's a 2-byte CRC field we need to skip
    let has_crc = (frame_header[1] & mp3_constants::PROTECTION_BIT_MASK) == 0;

    // Parse MPEG version and channel mode to determine side info size
    let mpeg_version = (frame_header[1] >> 3) & mp3_constants::MPEG_VERSION_MASK;
    let channel_mode = (frame_header[3] >> 6) & mp3_constants::CHANNEL_MODE_MASK;

    // Skip CRC if present (2 bytes)
    if has_crc && file.seek(SeekFrom::Current(2)).is_err() {
        return None;
    }

    // Calculate side info offset
    // MPEG-1 Layer III: 17 bytes (mono) or 32 bytes (stereo/dual/joint)
    // MPEG-2/2.5 Layer III: 9 bytes (mono) or 17 bytes (stereo/dual/joint)
    let side_info_size = match mpeg_version {
        mp3_constants::MPEG_VERSION_1 => {
            // MPEG-1 Layer III
            if channel_mode == mp3_constants::CHANNEL_MODE_MONO {
                mp3_constants::side_info_size::MPEG1_MONO
            } else {
                mp3_constants::side_info_size::MPEG1_STEREO
            }
        }
        _ => {
            // MPEG-2 or MPEG-2.5 Layer III
            if channel_mode == mp3_constants::CHANNEL_MODE_MONO {
                mp3_constants::side_info_size::MPEG2_MONO
            } else {
                mp3_constants::side_info_size::MPEG2_STEREO
            }
        }
    };

    // Skip side info to get to Xing/Info header
    if file.seek(SeekFrom::Current(side_info_size as i64)).is_err() {
        return None;
    }

    // Read Xing/Info header identifier (4 bytes)
    let mut xing_header = [0u8; 4];
    if file.read_exact(&mut xing_header).is_err() {
        return None;
    }

    // Check for "Xing" or "Info" header
    let has_xing = &xing_header == b"Xing" || &xing_header == b"Info";
    if !has_xing {
        return None; // No Xing/Info header, can't read LAME tag
    }

    // Read flags (4 bytes) to see what fields are present
    let mut flags = [0u8; 4];
    if file.read_exact(&mut flags).is_err() {
        return None;
    }

    // Skip optional fields based on flags (flags are in big-endian format):
    // - Frame count (4 bytes) if flag bit 0 is set (flags[3] bit 0)
    // - Byte count (4 bytes) if flag bit 1 is set (flags[3] bit 1)
    // - TOC (100 bytes) if flag bit 2 is set (flags[3] bit 2)
    // - Quality (4 bytes) if flag bit 3 is set (flags[3] bit 3)
    let mut skip_bytes = 0u64;
    if (flags[3] & mp3_constants::xing_flags::FRAME_COUNT) != 0 {
        skip_bytes += 4; // Frame count
    }
    if (flags[3] & mp3_constants::xing_flags::BYTE_COUNT) != 0 {
        skip_bytes += 4; // Byte count
    }
    if (flags[3] & mp3_constants::xing_flags::TOC) != 0 {
        skip_bytes += 100; // TOC
    }
    if (flags[3] & mp3_constants::xing_flags::QUALITY) != 0 {
        skip_bytes += 4; // Quality
    }

    if file.seek(SeekFrom::Current(skip_bytes as i64)).is_err() {
        return None;
    }

    // Now we should be at the LAME tag
    // LAME tag structure:
    // - Encoder version string (9 bytes, null-terminated)
    // - Info tag revision + VBR method (1 byte: high 4 bits = revision, low 4 bits = VBR method)
    let mut lame_version = [0u8; 9];
    if file.read_exact(&mut lame_version).is_err() {
        return None;
    }

    // Check if this looks like a LAME tag (starts with "LAME")
    if &lame_version[0..4] != b"LAME" {
        return None;
    }

    // Read the revision + VBR method byte (immediately after the 9-byte version string)
    let mut revision_vbr_byte = [0u8; 1];
    if file.read_exact(&mut revision_vbr_byte).is_err() {
        return None;
    }

    // VBR method is in the low 4 bits of this byte
    // High 4 bits contain the info tag revision
    let vbr_method = revision_vbr_byte[0] & mp3_constants::VBR_METHOD_MASK;

    // Map VBR method to BitrateMode enum
    match vbr_method {
        mp3_constants::vbr_method::CBR => Some(BitrateMode::Cbr),
        mp3_constants::vbr_method::ABR => Some(BitrateMode::Abr),
        mp3_constants::vbr_method::VBR_OLD => Some(BitrateMode::Vbr),
        mp3_constants::vbr_method::VBR_NEW => Some(BitrateMode::Vbr),
        mp3_constants::vbr_method::VBR_MT => Some(BitrateMode::Vbr),
        mp3_constants::vbr_method::VBR_MTRH => Some(BitrateMode::Vbr),
        mp3_constants::vbr_method::VBR_ABR_ALT => Some(BitrateMode::Vbr),
        _ => None, // Unknown or reserved
    }
}
