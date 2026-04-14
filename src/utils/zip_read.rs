//! Bounded reads from ZIP member streams (mitigate decompression bombs).

use log::warn;
use std::io::{self, Read};
use zip::read::ZipFile;

/// Read a ZIP entry fully into a UTF-8 string, enforcing a maximum **decompressed** size.
///
/// When `max_uncompressed_bytes == 0`, no limit is applied (previous behavior).
///
/// Compares the entry’s declared uncompressed size when limited, then reads at most
/// `max_uncompressed_bytes + 1` bytes so a mismatching stream cannot grow past the cap.
pub(crate) fn read_zip_entry_to_string_limited<R: Read>(
    mut file: ZipFile<R>,
    max_uncompressed_bytes: usize,
    entry_path: &str,
    container_label: &str,
) -> io::Result<String> {
    if max_uncompressed_bytes == 0 {
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        return Ok(s);
    }

    let max_u64 = max_uncompressed_bytes as u64;
    let declared = file.size();
    if declared > max_u64 {
        warn!(
            "ZIP entry '{entry_path}' in {container_label} declares uncompressed size {declared} bytes (max {max_uncompressed_bytes}); skipping"
        );
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "zip entry uncompressed size exceeds cap",
        ));
    }

    let mut buf = Vec::new();
    file.take(max_u64.saturating_add(1)).read_to_end(&mut buf)?;

    if buf.len() > max_uncompressed_bytes {
        warn!(
            "ZIP entry '{entry_path}' in {container_label} exceeded {max_uncompressed_bytes} bytes while reading; skipping"
        );
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "zip entry read exceeded cap",
        ));
    }

    String::from_utf8(buf).map_err(|e| {
        warn!("ZIP entry '{entry_path}' in {container_label}: invalid UTF-8: {e}");
        io::Error::new(io::ErrorKind::InvalidData, e.utf8_error())
    })
}
