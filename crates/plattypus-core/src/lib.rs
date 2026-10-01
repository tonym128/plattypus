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
