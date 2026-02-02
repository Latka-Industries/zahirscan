use toml::Value as TomlValue;
use toml::map::Map;

/// Deep-merge two TOML values. Base is the default; overlay only overrides keys it provides.
/// For keys not present in overlay, the result keeps base's value. Nested tables are merged
/// recursively with the same rule.
pub(crate) fn deep_merge_toml(base: TomlValue, overlay: TomlValue) -> TomlValue {
    match (base, overlay) {
        (TomlValue::Table(base_map), TomlValue::Table(overlay_map)) => {
            let mut out = base_map;
            for (key, overlay_val) in overlay_map {
                let base_val = out.remove(&key).unwrap_or(TomlValue::Table(Map::new()));
                out.insert(key, deep_merge_toml(base_val, overlay_val));
            }
            TomlValue::Table(out)
        }
        (_, overlay) => overlay,
    }
}

// -----------------------------------------------------------------------------
// Helpers (used by core.rs)
// -----------------------------------------------------------------------------

#[inline]
pub(crate) fn u64_to_usize_min(value: u64, min: u64) -> usize {
    value.max(min) as usize
}

#[inline]
pub(crate) fn u64_to_usize(value: u64) -> usize {
    value as usize
}

#[inline]
pub(crate) fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    value.clamp(min, max)
}

/// Validate a f64 is in [0.0, 1.0]; return Err with clear message otherwise.
#[macro_export]
macro_rules! validate_range_01 {
    ($self:expr, $field:ident, $name:expr) => {
        if !(0.0..=1.0).contains(&$self.$field) {
            return Err(anyhow::anyhow!(
                "config: {} must be in [0.0, 1.0], got {}",
                $name,
                $self.$field
            ));
        }
    };
}

/// Validate a numeric field is >= min; return Err with clear message otherwise.
#[macro_export]
macro_rules! validate_min {
    ($self:expr, $field:ident, $min:expr, $name:expr) => {
        if $self.$field < $min {
            return Err(anyhow::anyhow!(
                "config: {} must be >= {}, got {}",
                $name,
                $min,
                $self.$field
            ));
        }
    };
}
