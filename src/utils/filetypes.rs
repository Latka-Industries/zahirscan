use linguist;
use std::io::Read;
use std::path::Path;

use crate::parsers::FileType;

/// Bytes read when disambiguating `.pkl` (Python pickle vs Apple Pkl source).
const PKL_HEAD_BYTES: usize = 4096;

/// Expands to the full `[(ext, FileType), ...]` array. Each `Variant: "a", "b"` becomes
/// `("a", FileType::Variant), ("b", FileType::Variant)`. Macros must expand to a complete
/// expression, so everything is in one macro.
macro_rules! file_extension_map {
    (
        $(
            $ft:ident: $($e:literal),+ $(,)?
        );+ $(;)?
    ) => {
        &[
            $( $( ( $e, FileType::$ft ) ),+ ),+
        ]
    };
}

/// File extension to `FileType` mapping. Grouped by `FileType` for easier maintenance.
const FILE_EXTENSION_MAP: &[(&str, FileType)] = file_extension_map! {
    Log: "log";
    Json: "json";
    Text: "txt";
    Markdown: "md", "markdown";
    Image: "jpg", "jpeg", "png", "gif", "bmp", "tiff", "tif", "webp", "ico", "svg";
    Video: "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "3gp", "ogv";
    Audio: "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma", "ape", "dsd", "dsf", "aif", "aiff", "aifc";
    Csv: "csv", "tsv", "tab", "psv";
    Parquet: "parquet";
    ArrowIpc: "arrow", "feather", "ipc";
    Avro: "avro";
    Orc: "orc";
    Npy: "npy";
    Npz: "npz";
    Hdf5: "h5", "hdf5";
    NetCdf: "nc", "cdf";
    Mtx: "mtx";
    Mat: "mat";
    Onnx: "onnx";
    Gguf: "gguf";
    Tflite: "tflite";
    Safetensors: "safetensors";
    Zarr: "zarr";
    Tetration: "tet";
    Pickle: "pickle";
    Html: "html", "htm";
    Docx: "docx";
    Xlsx: "xlsx";
    Pptx: "pptx";
    Sqlite: "db", "sqlite", "sqlite3";
    Toml: "toml", "lock";
    Ini: "ini", "cfg";
    Xml: "xml";
    Yaml: "yaml", "yml";
    Zip: "zip";
    Archive: "tar", "gz", "bz2", "xz", "tgz";
    Epub: "epub";
    Pdf: "pdf";
};

/// Get `FileType` from extension using linear search
/// Returns `FileType::Unknown` if extension is not recognized
///
/// For ~49 extensions, linear search is faster than `HashMap` due to:
/// - No hash computation overhead
/// - No memory allocation
/// - Cache-friendly sequential access
/// - Most common extensions (jpg, mp3, pdf) are near the start
fn get_file_type_from_extension(extension: &str) -> FileType {
    FILE_EXTENSION_MAP
        .iter()
        .find(|(ext, _)| *ext == extension)
        .map_or(FileType::Unknown, |(_, file_type)| *file_type)
}

/// Get all file extensions for a given `FileType`
/// Useful for validation, documentation, or UI display
#[must_use]
pub fn get_extensions_for_file_type(file_type: FileType) -> Vec<&'static str> {
    FILE_EXTENSION_MAP
        .iter()
        .filter_map(|(ext, ft)| if *ft == file_type { Some(*ext) } else { None })
        .collect()
}

/// Check if a codec name (string) matches any extension for a given `FileType`
/// Useful for checking codec names from ffprobe against our known file types
///
/// Example:
/// ```
/// use zahirscan::utils::filetypes::is_codec_for_file_type;
/// use zahirscan::parsers::FileType;
///
/// assert!(is_codec_for_file_type("mp3", FileType::Audio));
/// assert!(is_codec_for_file_type("flac", FileType::Audio));
/// assert!(!is_codec_for_file_type("mp3", FileType::Video));
/// ```
#[must_use]
pub fn is_codec_for_file_type(codec: &str, file_type: FileType) -> bool {
    let codec_lower = codec.to_lowercase();
    FILE_EXTENSION_MAP
        .iter()
        .any(|(ext, ft)| *ft == file_type && codec_lower.contains(ext))
}

/// Detect file type from extension.
/// Compound extensions (e.g. .tar.gz, .tar.bz2, .tgz, .tar.xz) are checked first.
/// If extension is unknown, tries linguist (extension + filename) as fallback for code/script files.
#[must_use]
pub fn detect_file_type(path: &str) -> FileType {
    let lo = path.to_lowercase();
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    if lo.ends_with(".tar.xz")
        || lo.ends_with(".tar.bz2")
        || lo.ends_with(".tar.gz")
        || lo.ends_with(".tgz")
    {
        return FileType::Archive;
    }

    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();

    if extension == "pkl" {
        // `.pkl` is shared by Python pickle bytes and Apple Pkl source text — sniff before routing.
        let p = Path::new(path);
        if p.exists()
            && let Ok(head) = read_pkl_head(p)
            && is_python_pickle_bytes(&head)
        {
            return FileType::Pickle;
        }
        // Apple Pkl (or unknown `.pkl`) falls through to linguist → Code when recognized.
    }

    let file_type = get_file_type_from_extension(&extension);
    if file_type == FileType::Unknown {
        let p = Path::new(path);
        if p.exists() {
            let by_ext = linguist::detect_language_by_extension(p)
                .ok()
                .unwrap_or_default();
            if !by_ext.is_empty() {
                return FileType::Code;
            }
            let by_name = linguist::detect_language_by_filename(p)
                .ok()
                .unwrap_or_default();
            if !by_name.is_empty() {
                return FileType::Code;
            }
        }
    }
    file_type
}

/// Read the first [`PKL_HEAD_BYTES`] of a path for `.pkl` extension sniffing.
fn read_pkl_head(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0_u8; PKL_HEAD_BYTES];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

/// True when bytes look like Python pickle serialization (not Apple Pkl source text).
///
/// Binary pickles: `\x80` + protocol 2–5, or non‑UTF‑8 bytes.
/// Text protocol 0/1: printable stream starting with `(`, `]`, `}`, or `.`.
/// Apple Pkl configs are UTF‑8 source and rejected via [`looks_like_apple_pkl_source`].
fn is_python_pickle_bytes(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    if bytes.len() >= 2 && bytes[0] == 0x80 && (2..=5).contains(&bytes[1]) {
        return true;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return true;
    };
    !looks_like_apple_pkl_source(text)
        && (looks_like_text_pickle(text) || text.bytes().any(|b| b == 0))
}

/// Apple Pkl (https://pkl-lang.org) source keywords at the start of a `.pkl` file.
fn looks_like_apple_pkl_source(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("module")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("amends ")
        || trimmed.starts_with("extends ")
        || trimmed.starts_with("local ")
        || trimmed.starts_with("const ")
        || trimmed.starts_with("fixed ")
        || trimmed.starts_with("abstract ")
        || trimmed.starts_with("open ")
        || trimmed.starts_with("class ")
        || trimmed.starts_with("typealias ")
        || trimmed.starts_with('@')
}

/// Protocol 0/1 pickles are line-oriented ASCII; first opcode is often `(` (MARK) or `.` (STOP).
fn looks_like_text_pickle(text: &str) -> bool {
    matches!(
        text.trim_start().as_bytes().first(),
        Some(b'(' | b']' | b'}' | b'.')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol5_header_is_pickle() {
        assert!(is_python_pickle_bytes(&[0x80, 0x05, 0x95, 0x0a]));
    }

    #[test]
    fn apple_pkl_source_is_not_pickle() {
        let src = "module example\n\nfoo = 1\n";
        assert!(!is_python_pickle_bytes(src.as_bytes()));
    }
}
