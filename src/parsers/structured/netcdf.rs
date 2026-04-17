//! NetCDF (`.nc`, `.cdf`) — global and variable metadata via [`netcdf`] (no full array decode).

use anyhow::Context;
use log::debug;
use memmap2::Mmap;
use netcdf::AttributeValue;
use netcdf::Group;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::{NetCdfAttributeEntry, NetCdfMetadata, NetCdfVariableSummary};
use crate::utils::netcdf_handler::check_netcdf_available;

const MAX_GLOBAL_ATTRS: usize = 512;
const MAX_VARS_TOTAL: usize = 10_000;
const MAX_ATTRS_PER_VAR: usize = 256;
const MAX_DIMS_LISTED: usize = 512;
const MAX_GROUP_DEPTH: usize = 128;

fn attr_value_string(v: AttributeValue) -> String {
    match v {
        AttributeValue::Str(s) => s,
        AttributeValue::Strs(ss) => ss.join("\n"),
        AttributeValue::Uchar(x) => x.to_string(),
        AttributeValue::Uchars(v) => format!("{v:?}"),
        AttributeValue::Schar(x) => x.to_string(),
        AttributeValue::Schars(v) => format!("{v:?}"),
        AttributeValue::Ushort(x) => x.to_string(),
        AttributeValue::Ushorts(v) => format!("{v:?}"),
        AttributeValue::Short(x) => x.to_string(),
        AttributeValue::Shorts(v) => format!("{v:?}"),
        AttributeValue::Uint(x) => x.to_string(),
        AttributeValue::Uints(v) => format!("{v:?}"),
        AttributeValue::Int(x) => x.to_string(),
        AttributeValue::Ints(v) => format!("{v:?}"),
        AttributeValue::Ulonglong(x) => x.to_string(),
        AttributeValue::Ulonglongs(v) => format!("{v:?}"),
        AttributeValue::Longlong(x) => x.to_string(),
        AttributeValue::Longlongs(v) => format!("{v:?}"),
        AttributeValue::Float(x) => x.to_string(),
        AttributeValue::Floats(v) => format!("{v:?}"),
        AttributeValue::Double(x) => x.to_string(),
        AttributeValue::Doubles(v) => format!("{v:?}"),
    }
}

fn collect_global_attrs(file: &netcdf::File, out: &mut Vec<NetCdfAttributeEntry>) {
    for a in file.attributes().take(MAX_GLOBAL_ATTRS) {
        let name = a.name().to_string();
        match a.value() {
            Ok(v) => out.push(NetCdfAttributeEntry {
                name,
                value: attr_value_string(v),
            }),
            Err(e) => out.push(NetCdfAttributeEntry {
                name,
                value: format!("<read error: {e}>"),
            }),
        }
    }
}

fn push_variable(
    var: netcdf::Variable<'_>,
    path_prefix: &str,
    out: &mut Vec<NetCdfVariableSummary>,
    truncated: &mut bool,
) {
    if out.len() >= MAX_VARS_TOTAL {
        *truncated = true;
        return;
    }

    let base = var.name();
    let name = if path_prefix.is_empty() {
        base
    } else {
        format!("{path_prefix}/{base}")
    };

    let dims = var.dimensions();
    let shape: Vec<usize> = dims.iter().map(|d| d.len()).collect();
    let dimension_names: Vec<String> = dims.iter().map(|d| d.name()).collect();
    let vartype = format!("{:?}", var.vartype());

    let mut attrs = Vec::new();
    for a in var.attributes().take(MAX_ATTRS_PER_VAR) {
        let aname = a.name().to_string();
        match a.value() {
            Ok(v) => attrs.push(NetCdfAttributeEntry {
                name: aname,
                value: attr_value_string(v),
            }),
            Err(e) => attrs.push(NetCdfAttributeEntry {
                name: aname,
                value: format!("<read error: {e}>"),
            }),
        }
    }

    out.push(NetCdfVariableSummary {
        name,
        shape: Some(shape),
        dimension_names: Some(dimension_names),
        vartype: Some(vartype),
        attributes: Some(attrs),
    });
}

fn walk_group(
    g: &Group<'_>,
    prefix: &str,
    out: &mut Vec<NetCdfVariableSummary>,
    truncated: &mut bool,
    depth: usize,
) -> anyhow::Result<()> {
    if depth > MAX_GROUP_DEPTH {
        *truncated = true;
        return Ok(());
    }

    for v in g.variables() {
        if out.len() >= MAX_VARS_TOTAL {
            *truncated = true;
            return Ok(());
        }
        push_variable(v, prefix, out, truncated);
    }

    for child in g.groups() {
        let stem = child.name();
        let next = if prefix.is_empty() {
            stem
        } else {
            format!("{prefix}/{stem}")
        };
        walk_group(&child, &next, out, truncated, depth + 1)?;
    }
    Ok(())
}

/// Extract NetCDF metadata (attributes, variable names, shapes, types). Uses [`ParseResult::file_path`];
/// `mmap` is unused because `netcdf` opens by path.
///
/// # Errors
///
/// Returns an error when NetCDF helpers are missing, the file cannot be opened, or group walks fail.
pub fn extract_netcdf_metadata(
    _mmap: &Mmap,
    stats: &ParseResult,
    _config: &RuntimeConfig,
) -> anyhow::Result<NetCdfMetadata> {
    check_netcdf_available()?;

    let path = stats.file_path.as_str();
    let file = netcdf::open(path).map_err(|e| {
        debug!("NetCDF open failed for '{}': {}", path, e);
        anyhow::anyhow!("{}", e)
    })?;

    let netcdf4_model = Some(file.root().is_some());

    let mut global_attributes = Vec::new();
    collect_global_attrs(&file, &mut global_attributes);

    let dims: Vec<_> = file.dimensions().take(MAX_DIMS_LISTED).collect();
    let dimension_count = Some(dims.len());
    let dimensions_sample: Vec<(String, usize)> =
        dims.iter().map(|d| (d.name(), d.len())).collect();

    let mut variables = Vec::new();
    let mut metadata_truncated = false;

    for v in file.variables() {
        if variables.len() >= MAX_VARS_TOTAL {
            metadata_truncated = true;
            break;
        }
        push_variable(v, "", &mut variables, &mut metadata_truncated);
    }

    // NetCDF-4 only: nested groups (root variables are already in `file.variables()`).
    if let Some(root) = file.root() {
        for child in root.groups() {
            if variables.len() >= MAX_VARS_TOTAL {
                metadata_truncated = true;
                break;
            }
            let stem = child.name();
            walk_group(&child, &stem, &mut variables, &mut metadata_truncated, 0)
                .with_context(|| "walk NetCDF-4 groups")?;
        }
    }

    let variable_count = Some(variables.len());

    Ok(NetCdfMetadata {
        byte_count: stats.byte_count,
        netcdf4_model,
        global_attributes: Some(global_attributes),
        dimension_count,
        dimensions_sample: Some(dimensions_sample),
        variable_count,
        variables: Some(variables),
        metadata_truncated: Some(metadata_truncated),
        parse_error: None,
    })
}

crate::no_template_mining!(
    extract_netcdf_templates,
    "NetCDF is binary scientific data; no text template mining."
);
