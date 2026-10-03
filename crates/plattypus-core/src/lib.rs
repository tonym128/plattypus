// SPDX-License-Identifier: GPL-2.0-or-later
//! Core types, constants, level layouts and save formats for Plattypus PSX.
//! Pure #![no_std] crate with zero hardware or SDK MMIO dependencies,
//! shared between the game binary and the host-side verification test suite.

#![no_std]

pub mod level;
pub mod save;

pub use level::{Act, CellType, GRID_D, GRID_W, TILE_SZ};
pub use save::{
    Codename, Language, LoadOutcome, SaveData, CAMPAIGN_ACT_COUNT, SAVE_FILENAME, SAVE_MAGIC,
    SAVE_TITLE, SAVE_VERSION, SCREEN_OFFSET_LIMIT, SERIALIZED_SIZE,
};

/// Maximum simulation sub-steps per frame to prevent spiral of death during stalls.
pub const MAX_SUBSTEPS: u32 = 4;

/// Computes the number of fixed-timestep simulation sub-steps to execute
/// for the current frame given the number of VBlanks elapsed since the last frame.
/// Clamped between 1 and `MAX_SUBSTEPS` (4) to prevent spiral of death during
/// long pauses (such as CD-DA seeks or disk operations) while maintaining
/// 60 Hz simulation pacing during rendering slowdown (e.g. 30 FPS = 2 substeps).
#[inline]
pub const fn compute_substeps(vblanks_per_frame: u32) -> u32 {
    if vblanks_per_frame <= 1 {
        1
    } else if vblanks_per_frame > MAX_SUBSTEPS {
        MAX_SUBSTEPS
    } else {
        vblanks_per_frame
    }
}
