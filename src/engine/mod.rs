//! Scan engine: orchestration, chunking, path iteration, progress, and file-type/format utilities.

pub mod chunking;
pub mod orchestrator;
mod path_iter;
mod progress;
pub mod tools;

pub(crate) use path_iter::ToPathIter;
