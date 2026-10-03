//! 3D Tactical Stealth Environments for Plattypus MGS.
//! Full 3D sectors with cargo crates, security walls, air ducts, water canals,
//! searchlight posts, and infiltration exit burrows.
//!
//! The level model lives in `plattypus-core` so the host-side logic suite can
//! exercise the real generators instead of a mirror of them.

pub use plattypus_core::level::*;
