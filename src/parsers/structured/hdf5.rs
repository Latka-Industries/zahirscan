//! HDF5 (`.h5`, `.hdf5`) metadata via [`hdf5_pure_rust`] (pure Rust; opens by path, not mmap).

use anyhow::Context;
use hdf5_pure_rust::File;
use hdf5_pure_rust::Group;
use hdf5_pure_rust::hl::file::ObjectType;
use log::debug;
use memmap2::Mmap;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::{Hdf5DatasetSummary, Hdf5Metadata};

const MAX_DEPTH: usize = 128;
const MAX_DATASETS_LISTED: usize = 10_000;

fn count_root_datasets(root: &Group) -> anyhow::Result<usize> {
    let mut n = 0usize;
    for name in root.member_names()? {
        if root.member_type(&name)? == ObjectType::Dataset {
            n += 1;
        }
    }
    Ok(n)
}

fn walk_group(
    group: &Group,
    out: &mut Vec<Hdf5DatasetSummary>,
    groups_visited: &mut usize,
    datasets_seen: &mut usize,
    truncated: &mut bool,
    depth: usize,
) -> anyhow::Result<()> {
    if depth > MAX_DEPTH {
        *truncated = true;
        return Ok(());
    }
    *groups_visited += 1;

    let members = group
        .members()
        .map_err(|e| anyhow::anyhow!("{}", e))
        .with_context(|| format!("list members of group {}", group.name()))?;

    for (name, _) in members {
        if out.len() >= MAX_DATASETS_LISTED {
            *truncated = true;
            return Ok(());
        }

        let obj_type = group
            .member_type(&name)
            .map_err(|e| anyhow::anyhow!("{}", e))
            .with_context(|| format!("member type for {}/{}", group.name(), name))?;

        match obj_type {
            ObjectType::Group => {
                let child = group
                    .open_group(&name)
                    .map_err(|e| anyhow::anyhow!("{}", e))
                    .with_context(|| format!("open group {name}"))?;
                walk_group(
                    &child,
                    out,
                    groups_visited,
                    datasets_seen,
                    truncated,
                    depth + 1,
                )?;
            }
            ObjectType::Dataset => {
                *datasets_seen += 1;
                let ds = group
                    .open_dataset(&name)
                    .map_err(|e| anyhow::anyhow!("{}", e))
                    .with_context(|| format!("open dataset {name}"))?;

                let path = ds.name().to_string();
                let mut inspect_error = None;
                let shape = match ds.shape() {
                    Ok(s) => Some(s),
                    Err(e) => {
                        inspect_error = Some(e.to_string());
                        None
                    }
                };
                let datatype_class = ds
                    .info()
                    .ok()
                    .map(|info| format!("{:?}", info.datatype.class));

                out.push(Hdf5DatasetSummary {
                    path,
                    shape,
                    datatype_class,
                    inspect_error,
                });
            }
            ObjectType::NamedDatatype | ObjectType::Unknown => {}
        }
    }
    Ok(())
}

/// Extract HDF5 hierarchy and dataset shapes (bounded walk). Uses [`ParseResult::file_path`];
/// `mmap` is unused because `hdf5-pure-rust` reads via `File::open`.
///
/// # Errors
///
/// Returns an error when the file cannot be opened as HDF5 or the root group is unreadable.
pub fn extract_hdf5_metadata(
    _mmap: &Mmap,
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> anyhow::Result<Hdf5Metadata> {
    let path = stats.file_path.as_str();
    let file = File::open(path).map_err(|e| {
        debug!("HDF5 open failed for '{}': {}", path, e);
        anyhow::anyhow!("{}", e)
    })?;

    let superblock_version = Some(file.superblock().version);

    let root = file
        .root_group()
        .map_err(|e| anyhow::anyhow!("{}", e))
        .context("read HDF5 root group")?;

    let root_member_count = root.len().ok();
    let root_dataset_count = count_root_datasets(&root).ok();
    let root_attribute_count = file.attr_names().ok().map(|v| v.len());

    let mut datasets = Vec::new();
    let mut groups_visited = 0usize;
    let mut datasets_seen = 0usize;
    let mut walk_truncated = false;

    if let Err(e) = walk_group(
        &root,
        &mut datasets,
        &mut groups_visited,
        &mut datasets_seen,
        &mut walk_truncated,
        0,
    ) {
        debug!("HDF5 walk error for '{}': {:#}", path, e);
        return Ok(Hdf5Metadata {
            byte_count: stats.byte_count,
            superblock_version,
            root_member_count,
            root_dataset_count,
            root_attribute_count,
            groups_visited: Some(groups_visited),
            datasets_visited: Some(datasets_seen),
            datasets: Some(datasets),
            walk_truncated: Some(walk_truncated),
            parse_error: Some(e.to_string()),
        });
    }

    Ok(Hdf5Metadata {
        byte_count: stats.byte_count,
        superblock_version,
        root_member_count,
        root_dataset_count,
        root_attribute_count,
        groups_visited: Some(groups_visited),
        datasets_visited: Some(datasets_seen),
        datasets: Some(datasets),
        walk_truncated: Some(walk_truncated),
        parse_error: None,
    })
}

crate::no_template_mining!(
    extract_hdf5_templates,
    "HDF5 is binary scientific data; no text template mining."
);
