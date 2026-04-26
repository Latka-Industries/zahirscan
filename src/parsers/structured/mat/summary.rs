//! Global (entry-wide) stats for non-scalar arrays.

use matrw::{MatVariable, MatlabType};

use crate::parsers::column_stats::compute_numeric_stats_from_values;
use crate::results::{ArrayLayoutSummary, BooleanStats, MatGlobalSummary};

use super::common::{
    MaxMatVars, bounded_unique_chars, count_nan_inf_in_numeric_var, count_nan_inf_in_sparse_var,
    is_char_vector_shape, is_mat_scalar_1x1, peel_compressed,
};

fn sample_step(len: usize, cap: usize) -> usize {
    if len <= cap { 1 } else { len.div_ceil(cap) }
}

fn numeric_values_for_global(value: &MatlabType) -> Vec<f64> {
    let cap = MaxMatVars::NUMERIC_STATS_VALUES;
    match value {
        MatlabType::F64(v) => {
            let step = sample_step(v.len(), cap);
            v.iter()
                .step_by(step)
                .copied()
                .filter(|x| x.is_finite())
                .collect()
        }
        MatlabType::F32(v) => {
            let step = sample_step(v.len(), cap);
            v.iter()
                .step_by(step)
                .copied()
                .map(f64::from)
                .filter(|x| x.is_finite())
                .collect()
        }
        MatlabType::I64(v) => {
            let step = sample_step(v.len(), cap);
            v.iter().step_by(step).copied().map(|x| x as f64).collect()
        }
        MatlabType::I32(v) => {
            let step = sample_step(v.len(), cap);
            v.iter().step_by(step).copied().map(f64::from).collect()
        }
        MatlabType::I16(v) => {
            let step = sample_step(v.len(), cap);
            v.iter().step_by(step).copied().map(f64::from).collect()
        }
        MatlabType::I8(v) => {
            let step = sample_step(v.len(), cap);
            v.iter().step_by(step).copied().map(f64::from).collect()
        }
        MatlabType::U64(v) => {
            let step = sample_step(v.len(), cap);
            v.iter().step_by(step).copied().map(|x| x as f64).collect()
        }
        MatlabType::U32(v) => {
            let step = sample_step(v.len(), cap);
            v.iter().step_by(step).copied().map(f64::from).collect()
        }
        MatlabType::U16(v) => {
            let step = sample_step(v.len(), cap);
            v.iter().step_by(step).copied().map(f64::from).collect()
        }
        MatlabType::U8(v) => {
            let step = sample_step(v.len(), cap);
            v.iter().step_by(step).copied().map(f64::from).collect()
        }
        MatlabType::BOOL(v) => {
            let step = sample_step(v.len(), cap);
            v.iter()
                .step_by(step)
                .copied()
                .map(|x| if x { 1.0 } else { 0.0 })
                .collect()
        }
        MatlabType::UTF8(_) | MatlabType::UTF16(_) => Vec::new(),
    }
}

fn matlab_value_count(value: &MatlabType) -> usize {
    match value {
        MatlabType::U8(v) => v.len(),
        MatlabType::I8(v) => v.len(),
        MatlabType::U16(v) => v.len(),
        MatlabType::I16(v) => v.len(),
        MatlabType::U32(v) => v.len(),
        MatlabType::I32(v) => v.len(),
        MatlabType::U64(v) => v.len(),
        MatlabType::I64(v) => v.len(),
        MatlabType::F32(v) => v.len(),
        MatlabType::F64(v) => v.len(),
        MatlabType::UTF8(v) | MatlabType::UTF16(v) => v.len(),
        MatlabType::BOOL(v) => v.len(),
    }
}

fn global_summary_for_numeric_array(var: &MatVariable, value: &MatlabType) -> MatGlobalSummary {
    match value {
        MatlabType::BOOL(v) => {
            let total = v.len();
            let true_count = v.iter().filter(|x| **x).count();
            let true_percentage = if total == 0 {
                None
            } else {
                Some((true_count as f64 / total as f64) * 100.0)
            };
            MatGlobalSummary {
                t: "boolean".to_string(),
                count: total,
                uniq: None,
                num: None,
                date: None,
                bool_stats: Some(BooleanStats { true_percentage }),
                n_nan: None,
                n_inf: None,
            }
        }
        MatlabType::UTF8(v) | MatlabType::UTF16(v) => {
            let uniq = bounded_unique_chars(v, MaxMatVars::CHAR_UNIQ_VALUES);
            MatGlobalSummary {
                t: "char".to_string(),
                count: v.len(),
                uniq: Some(uniq),
                num: None,
                date: None,
                bool_stats: None,
                n_nan: None,
                n_inf: None,
            }
        }
        _ => {
            let values = numeric_values_for_global(value);
            let mut budget = MaxMatVars::NAN_INF_SCAN_FLOATS;
            let mut n_nan = 0u64;
            let mut n_inf = 0u64;
            count_nan_inf_in_numeric_var(var, &mut budget, &mut n_nan, &mut n_inf);
            MatGlobalSummary {
                t: "number".to_string(),
                count: matlab_value_count(value),
                uniq: None,
                num: compute_numeric_stats_from_values(&values),
                date: None,
                bool_stats: None,
                n_nan: Some(n_nan),
                n_inf: Some(n_inf),
            }
        }
    }
}

pub(crate) fn mat_global_summary_for_entry(
    var: &MatVariable,
    layout: &ArrayLayoutSummary,
) -> Option<MatGlobalSummary> {
    if is_mat_scalar_1x1(layout) {
        return None;
    }
    match peel_compressed(var) {
        MatVariable::NumericArray(n) => match &n.value {
            MatlabType::UTF8(_) | MatlabType::UTF16(_) if is_char_vector_shape(layout) => None,
            _ => Some(global_summary_for_numeric_array(var, &n.value)),
        },
        MatVariable::SparseArray(s) => {
            let values = numeric_values_for_global(&s.value);
            let mut budget = MaxMatVars::NAN_INF_SCAN_FLOATS;
            let mut n_nan = 0u64;
            let mut n_inf = 0u64;
            count_nan_inf_in_sparse_var(s, &mut budget, &mut n_nan, &mut n_inf);
            Some(MatGlobalSummary {
                t: "number".to_string(),
                count: matlab_value_count(&s.value),
                uniq: None,
                num: compute_numeric_stats_from_values(&values),
                date: None,
                bool_stats: None,
                n_nan: Some(n_nan),
                n_inf: Some(n_inf),
            })
        }
        _ => None,
    }
}
