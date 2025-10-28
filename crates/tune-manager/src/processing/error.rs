//! Processing error trait for handling stage skip behavior.
//!
//! This module provides the `ProcessingError` trait which allows errors to indicate
//! whether they should cause a stage to be skipped (e.g., when a stage is not configured)
//! or result in a failure.

/// Trait for processing errors that can indicate if a stage should be skipped.
///
/// Errors implementing this trait can specify whether they represent a condition
/// that should cause the stage to be skipped (like not being configured) rather
/// than being treated as a failure.
///
/// When a stage is skipped, the error's Display implementation (from the `#[error]`
/// attribute for instance) will be used as the skip reason.
pub trait ProcessingError: std::fmt::Display {
    /// Returns true if this error should cause the stage to be skipped.
    ///
    /// # Returns
    ///
    /// `true` if the stage should be marked as skipped rather than failed,
    /// `false` if it should be treated as a normal error.
    fn causes_skip(&self) -> bool;
}
