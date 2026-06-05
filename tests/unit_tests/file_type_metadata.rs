//! `FileType::as_metadata_name` / `from_metadata_name` round-trip and edge cases.

use zahirscan::FileType;
use zahirscan::utils::filetypes::detect_file_type;

/// Every `FileType` variant in `repr(u8)` order (must match `parsers/mod.rs`).
fn all_file_types() -> [FileType; 40] {
    [
        FileType::Log,
        FileType::Json,
        FileType::Text,
        FileType::Markdown,
        FileType::Image,
        FileType::Video,
        FileType::Audio,
        FileType::Csv,
        FileType::Pdf,
        FileType::Docx,
        FileType::Xlsx,
        FileType::Sqlite,
        FileType::Toml,
        FileType::Zip,
        FileType::Xml,
        FileType::Html,
        FileType::Yaml,
        FileType::Ini,
        FileType::Pptx,
        FileType::Epub,
        FileType::Archive,
        FileType::Code,
        FileType::Parquet,
        FileType::ArrowIpc,
        FileType::Avro,
        FileType::Orc,
        FileType::Npy,
        FileType::Npz,
        FileType::Hdf5,
        FileType::NetCdf,
        FileType::Mtx,
        FileType::Mat,
        FileType::Onnx,
        FileType::Gguf,
        FileType::Tflite,
        FileType::Safetensors,
        FileType::Zarr,
        FileType::Tetration,
        FileType::Pickle,
        FileType::Unknown,
    ]
}

#[test]
fn from_metadata_name_round_trips_as_metadata_name_for_all_variants() {
    for ft in all_file_types() {
        let name = ft.as_metadata_name();
        assert_eq!(FileType::from_metadata_name(name), Some(ft), "{name}");
        assert_eq!(
            FileType::from_metadata_name(ft.as_metadata_name()),
            Some(ft)
        );
    }
}

#[test]
fn from_metadata_name_returns_none_for_unknown_strings() {
    assert_eq!(FileType::from_metadata_name(""), None);
    assert_eq!(FileType::from_metadata_name("not a type"), None);
    assert_eq!(FileType::from_metadata_name("csv"), None);
    assert_eq!(FileType::from_metadata_name("CSV "), None);
    assert_eq!(FileType::from_metadata_name("JSON "), None);
}

#[test]
fn file_type_discriminant_range_matches_variant_count() {
    assert_eq!(FileType::Parquet as u8, 22);
    assert_eq!(FileType::Orc as u8, 25);
    assert_eq!(FileType::Npy as u8, 26);
    assert_eq!(FileType::Npz as u8, 27);
    assert_eq!(FileType::Hdf5 as u8, 28);
    assert_eq!(FileType::NetCdf as u8, 29);
    assert_eq!(FileType::Mtx as u8, 30);
    assert_eq!(FileType::Mat as u8, 31);
    assert_eq!(FileType::Onnx as u8, 32);
    assert_eq!(FileType::Gguf as u8, 33);
    assert_eq!(FileType::Tflite as u8, 34);
    assert_eq!(FileType::Safetensors as u8, 35);
    assert_eq!(FileType::Zarr as u8, 36);
    assert_eq!(FileType::Tetration as u8, 37);
    assert_eq!(FileType::Pickle as u8, 38);
    assert_eq!(FileType::Unknown as u8, 39);
    assert_eq!(all_file_types().len(), 40);
}

#[test]
fn detect_file_type_columnar_extensions() {
    assert_eq!(detect_file_type("a.parquet"), FileType::Parquet);
    assert_eq!(detect_file_type("b.feather"), FileType::ArrowIpc);
    assert_eq!(detect_file_type("c.arrow"), FileType::ArrowIpc);
    assert_eq!(detect_file_type("d.avro"), FileType::Avro);
    assert_eq!(detect_file_type("e.orc"), FileType::Orc);
    assert_eq!(detect_file_type("f.npy"), FileType::Npy);
    assert_eq!(detect_file_type("g.npz"), FileType::Npz);
    assert_eq!(detect_file_type("h.h5"), FileType::Hdf5);
    assert_eq!(detect_file_type("i.hdf5"), FileType::Hdf5);
    assert_eq!(detect_file_type("j.nc"), FileType::NetCdf);
    assert_eq!(detect_file_type("k.cdf"), FileType::NetCdf);
    assert_eq!(detect_file_type("m.mtx"), FileType::Mtx);
    assert_eq!(detect_file_type("vars.mat"), FileType::Mat);
    assert_eq!(detect_file_type("m.onnx"), FileType::Onnx);
    assert_eq!(detect_file_type("model.gguf"), FileType::Gguf);
    assert_eq!(detect_file_type("m.tflite"), FileType::Tflite);
    assert_eq!(detect_file_type("x.safetensors"), FileType::Safetensors);
    assert_eq!(detect_file_type("ds.zarr"), FileType::Zarr);
    assert_eq!(detect_file_type("volume.tet"), FileType::Tetration);
    assert_eq!(detect_file_type("model.pickle"), FileType::Pickle);
}

/// `.pickle` is always Python pickle; `.pkl` sniffs bytes vs Apple Pkl source.
#[test]
fn detect_file_type_pkl_sniffs_python_pickle_vs_apple_pkl() {
    use std::io::Write;

    let dir = tempfile::tempdir().expect("tempdir");
    let bin_path = dir.path().join("model.pkl");
    let mut bin = std::fs::File::create(&bin_path).expect("create");
    bin.write_all(&[0x80, 0x05, 0x95, 0x0a, 0x00, 0x00, 0x00, 0x00])
        .expect("write");
    drop(bin);

    let src_path = dir.path().join("config.pkl");
    std::fs::write(&src_path, "module example\n\nfoo = 1\n").expect("write");

    assert_eq!(
        detect_file_type(bin_path.to_str().unwrap()),
        FileType::Pickle
    );
    assert_eq!(detect_file_type(src_path.to_str().unwrap()), FileType::Code);
}
