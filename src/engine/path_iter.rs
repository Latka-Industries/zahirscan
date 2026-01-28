//! Path iteration utilities
//!
//! Provides the `ToPathIter` trait for converting various path types into a vector of strings.
//! Allowable input types:
//! - &str
//! - &String
//! - String
//! - Vec<String>
//! - &Vec<String>
//! - &[String]
//! - [&str; N]

/// Helper trait to convert both `&str` and collections into an iterator of strings
pub(crate) trait ToPathIter {
    fn to_path_iter(self) -> Vec<String>;
}

// Macro to generate ToPathIter implementations with different conversion strategies
macro_rules! impl_to_path_iter {
    // Direct pass-through (no conversion needed) - for Vec<String>
    (pass: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                self
            }
        }
    };

    // Wrap single String in Vec
    (wrap: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                vec![self]
            }
        }
    };

    // Clone strategy - for &String
    (clone: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                vec![self.clone()]
            }
        }
    };

    // Clone from reference
    (clone_ref: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                self.clone()
            }
        }
    };

    // Convert to String
    (to_string: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                vec![self.to_string()]
            }
        }
    };

    // Convert slice to Vec
    (to_vec: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                self.to_vec()
            }
        }
    };

    // Map iterator to String
    (map_iter: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                self.iter().map(|s| s.to_string()).collect()
            }
        }
    };

    // Map into_iter to String
    (map_into: $t:ty) => {
        impl ToPathIter for $t {
            fn to_path_iter(self) -> Vec<String> {
                self.into_iter().map(|s| s.to_string()).collect()
            }
        }
    };
}

// Generate implementations
impl_to_path_iter!(to_string: &str);
impl_to_path_iter!(clone: &String);
impl_to_path_iter!(wrap: String);
impl_to_path_iter!(pass: Vec<String>);
impl_to_path_iter!(clone_ref: &Vec<String>);
impl_to_path_iter!(to_vec: &[String]);
impl_to_path_iter!(map_iter: &[&str]);
impl_to_path_iter!(map_into: Vec<&str>);
impl_to_path_iter!(map_iter: &Vec<&str>);

// Array implementation needs const generic
impl<const N: usize> ToPathIter for [&str; N] {
    fn to_path_iter(self) -> Vec<String> {
        self.into_iter().map(|s| s.to_string()).collect()
    }
}
