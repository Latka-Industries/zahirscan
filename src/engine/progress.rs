//! Progress bar utilities for displaying processing status

use kdam::{Animation, Bar, BarExt};
use std::sync::{Arc, Mutex};

/// Configuration for creating a progress bar
pub struct ProgressBarConfig {
    pub total: usize,
    pub desc: &'static str,
    pub animation: Animation,
}

impl ProgressBarConfig {
    /// Create a new progress bar configuration
    pub fn new(total: usize, desc: &'static str, animation: Animation) -> Self {
        Self {
            total,
            desc,
            animation,
        }
    }
}

/// Create a progress bar with the given configuration
pub fn create_progress_bar(config: ProgressBarConfig) -> Arc<Mutex<Bar>> {
    Arc::new(Mutex::new(kdam::tqdm!(
        total = config.total,
        desc = config.desc,
        animation = config.animation
    )))
}

/// Update progress bar if available
/// Uses try_lock to avoid blocking if mutex is contended (non-blocking)
pub fn update_progress_bar(pb: &Option<Arc<Mutex<Bar>>>) {
    if let Some(pb) = &pb {
        // Use try_lock to avoid blocking parallel workers
        // If lock is contended, skip update (progress bar will catch up on next update)
        if let Ok(mut pb) = pb.try_lock() {
            let _ = pb.update(1);
        }
    }
}

/// Macro to execute a function and update progress bar
/// Usage: `with_progress!(pb, function_call(...))`
/// Optimized: only calls update_progress_bar if pb is Some
#[macro_export]
macro_rules! with_progress {
    ($pb:expr, $func:expr) => {{
        let result = $func;
        if $pb.is_some() {
            $crate::engine::progress::update_progress_bar($pb);
        }
        result
    }};
}
