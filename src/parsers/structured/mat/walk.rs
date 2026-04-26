//! `struct`/`cell` subtree walk, per-entry build, and file extract.

use std::collections::BTreeSet;

use anyhow::Result;
use matrw::interface::types::array::ArrayType;
use matrw::{MatVariable, MatrwError, load_matfile_from_u8};
use memmap2::Mmap;
use rayon::prelude::*;

use crate::config::RuntimeConfig;
use crate::parsers::ParseResult;
use crate::results::{MatArrayEntrySummary, MatMetadata, MatStructWalkSummary};

use super::common::{
    HDF5_FILE_SIGNATURE, MaxMatVars, array_layout_for_mat_variable, count_nan_inf_in_numeric_var,
    count_nan_inf_in_sparse_var, mat_metadata_v73_hdf5_unsupported,
    normalize_scipy_mat5_header_for_matrw, peel_compressed, scalar_value_for_entry,
};
use super::summary::mat_global_summary_for_entry;

struct MatStructSubWalk {
    layer_key_sets: Vec<BTreeSet<String>>,
    leaf_dtypes: BTreeSet<String>,
    n_nan: u64,
    n_inf: u64,
    float_budget: usize,
}

impl MatStructSubWalk {
    fn new() -> Self {
        Self {
            layer_key_sets: Vec::new(),
            leaf_dtypes: BTreeSet::new(),
            n_nan: 0,
            n_inf: 0,
            float_budget: MaxMatVars::NAN_INF_SCAN_FLOATS,
        }
    }

    fn ensure_layer(&mut self, d: usize) {
        while self.layer_key_sets.len() <= d {
            self.layer_key_sets.push(BTreeSet::new());
        }
    }

    fn add_leaf(&mut self, v: &MatVariable) {
        if let Some(dt) = array_layout_for_mat_variable(v).dtype {
            self.leaf_dtypes.insert(dt);
        }
    }

    fn collect_floats_no_field_layers(&mut self, var: &MatVariable) {
        let v = peel_compressed(var);
        match v {
            MatVariable::Structure(s) => {
                for c in s.value.values() {
                    self.collect_floats_no_field_layers(c);
                }
            }
            MatVariable::StructureArray(sa) => {
                let n: usize = sa.dim.iter().product();
                for k in 0..n.min(MaxMatVars::STRUCT_ARRAY_ELEMS) {
                    if let Some(el) = sa.get_ref_colmaj(k) {
                        self.collect_floats_no_field_layers(el);
                    }
                }
            }
            MatVariable::CellArray(ca) => {
                let n: usize = ca.dim.iter().product();
                for k in 0..n.min(MaxMatVars::CELLS_LINEAR) {
                    if let Some(el) = ca.get_ref_colmaj(k) {
                        self.collect_floats_no_field_layers(el);
                    }
                }
            }
            MatVariable::NumericArray(_) => {
                count_nan_inf_in_numeric_var(
                    var,
                    &mut self.float_budget,
                    &mut self.n_nan,
                    &mut self.n_inf,
                );
            }
            MatVariable::SparseArray(s) => {
                count_nan_inf_in_sparse_var(
                    s,
                    &mut self.float_budget,
                    &mut self.n_nan,
                    &mut self.n_inf,
                );
            }
            _ => {}
        }
    }

    fn walk(&mut self, var: &MatVariable, d: usize) {
        if d > MaxMatVars::STRUCT_NEST {
            self.collect_floats_no_field_layers(var);
            return;
        }
        let v = peel_compressed(var);
        match v {
            MatVariable::Structure(s) => {
                self.ensure_layer(d);
                for (i, (k, _)) in s.value.iter().enumerate() {
                    if i >= MaxMatVars::FIELD_NAMES {
                        break;
                    }
                    if self.layer_key_sets[d].len() >= MaxMatVars::FIELD_NAMES {
                        break;
                    }
                    self.layer_key_sets[d].insert(k.clone());
                }
                for c in s.value.values() {
                    self.walk(c, d.saturating_add(1));
                }
            }
            MatVariable::StructureArray(sa) => {
                let n: usize = sa.dim.iter().product();
                for k in 0..n.min(MaxMatVars::STRUCT_ARRAY_ELEMS) {
                    if let Some(el) = sa.get_ref_colmaj(k) {
                        self.walk(el, d);
                    }
                }
            }
            MatVariable::CellArray(ca) => {
                let n: usize = ca.dim.iter().product();
                for k in 0..n.min(MaxMatVars::CELLS_LINEAR) {
                    if let Some(el) = ca.get_ref_colmaj(k) {
                        self.walk(el, d);
                    }
                }
            }
            MatVariable::NumericArray(_) => {
                self.add_leaf(var);
                count_nan_inf_in_numeric_var(
                    var,
                    &mut self.float_budget,
                    &mut self.n_nan,
                    &mut self.n_inf,
                );
            }
            MatVariable::SparseArray(s) => {
                self.add_leaf(var);
                count_nan_inf_in_sparse_var(
                    s,
                    &mut self.float_budget,
                    &mut self.n_nan,
                    &mut self.n_inf,
                );
            }
            _ => {
                self.add_leaf(var);
            }
        }
    }

    fn into_summary(self) -> MatStructWalkSummary {
        let n_field_layers = self.layer_key_sets.len();
        let field_layers = if n_field_layers == 0 {
            None
        } else {
            Some(
                self.layer_key_sets
                    .into_iter()
                    .map(|b| b.into_iter().collect())
                    .collect(),
            )
        };
        let leaf_dtypes = self.leaf_dtypes.into_iter().collect::<Vec<_>>().join(", ");
        MatStructWalkSummary {
            n_field_layers,
            field_layers,
            leaf_dtypes,
            n_nan: self.n_nan,
            n_inf: self.n_inf,
        }
    }
}

// `struct` / `cell` subtree walk
fn mat_struct_subtree_scan(var: &MatVariable) -> MatStructWalkSummary {
    let mut w = MatStructSubWalk::new();
    w.walk(var, 0);
    w.into_summary()
}

fn top_level_wants_struct_subtree(v: &MatVariable) -> bool {
    matches!(
        peel_compressed(v),
        MatVariable::Structure(_) | MatVariable::StructureArray(_) | MatVariable::CellArray(_)
    )
}

// One workspace variable -> entry
fn build_top_level_entry(
    name: &str,
    var: &MatVariable,
    _file_len_bytes: u64,
    _config: &RuntimeConfig,
) -> MatArrayEntrySummary {
    let mut layout = array_layout_for_mat_variable(var);
    let is_struct = top_level_wants_struct_subtree(var);
    if is_struct {
        layout.shape = None;
    }
    let value = if is_struct {
        None
    } else {
        scalar_value_for_entry(var, &layout)
    };
    let struct_subtree = if is_struct {
        Some(mat_struct_subtree_scan(var))
    } else {
        None
    };
    let global = if is_struct {
        None
    } else {
        mat_global_summary_for_entry(var, &layout)
    };
    MatArrayEntrySummary {
        name: name.to_string(),
        layout,
        value,
        struct_subtree,
        global,
        entry_parse_error: None,
    }
}

/// Extract `.mat` metadata: classic v7 variables via `matrw`; v7.3 HDF5 and other load failures return [`MatMetadata::file_parse_error`] instead of failing the extractor.
///
/// # Errors
///
/// Currently does not fail the extractor; load problems are reported in `file_parse_error`.
pub fn extract_mat_metadata(
    mmap: &Mmap,
    stats: &ParseResult,
    config: &RuntimeConfig,
) -> Result<MatMetadata> {
    let bytes: &[u8] = mmap.as_ref();
    if bytes.starts_with(HDF5_FILE_SIGNATURE) {
        return Ok(mat_metadata_v73_hdf5_unsupported(stats));
    }
    let normalized = normalize_scipy_mat5_header_for_matrw(bytes);

    let mat = match load_matfile_from_u8(normalized.as_ref()) {
        Ok(m) => m,
        Err(MatrwError::MatFile73Error) => {
            return Ok(mat_metadata_v73_hdf5_unsupported(stats));
        }
        Err(e) => {
            return Ok(MatMetadata {
                byte_count: stats.byte_count,
                mat_format: None,
                variable_count: None,
                variables_scanned: None,
                file_parse_error: Some(format!("{e:#}")),
                entries: None,
            });
        }
    };

    let variable_count = mat.iter().count();
    let mut variables_scanned = 0usize;
    let mut top_level_vars = Vec::new();
    for (idx, (name, var)) in mat.iter().enumerate() {
        if top_level_vars.len() >= MaxMatVars::TOP_LEVEL_VARIABLES {
            break;
        }
        variables_scanned = variables_scanned.saturating_add(1);
        top_level_vars.push((idx, name, var));
    }
    let entries = if top_level_vars.len() >= MaxMatVars::PARALLEL_TOP_LEVEL_MIN {
        let mut out: Vec<_> = top_level_vars
            .into_par_iter()
            .map(|(idx, name, var)| {
                (
                    idx,
                    build_top_level_entry(name, var, stats.byte_count as u64, config),
                )
            })
            .collect();
        out.sort_by_key(|(idx, _)| *idx);
        out.into_iter().map(|(_, entry)| entry).collect()
    } else {
        top_level_vars
            .into_iter()
            .map(|(_, name, var)| build_top_level_entry(name, var, stats.byte_count as u64, config))
            .collect()
    };

    Ok(MatMetadata {
        byte_count: stats.byte_count,
        mat_format: Some("v7".to_string()),
        variable_count: Some(variable_count),
        variables_scanned: Some(variables_scanned),
        file_parse_error: None,
        entries: Some(entries),
    })
}
