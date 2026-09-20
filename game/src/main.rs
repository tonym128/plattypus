//! Plattypus - A PSX Platformer built with PSoXide.
//!
//! Help Platty the Platypus escape Healesville Sanctuary, journey across the
//! bushland and city, and arrive at the ocean estuary to meet his new baby sibling!

#![no_std]
#![no_main]
#![allow(dead_code)]

extern crate psx_rt;

mod audio;
mod codec;
mod entities;
mod fixed;
mod game;
mod level;
mod platypus;
mod renderer;
pub mod save;

use audio::AudioManager;
use game::Game;

#[no_mangle]
fn main() -> ! {
    // Initialize audio SPU and ADPCM voice channels
    AudioManager::init();

    // Create and run game loop
    let mut game = Game::new();
    game.run();
}
