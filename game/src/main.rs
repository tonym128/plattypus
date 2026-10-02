//! Plattypus - A PSX Platformer built with PSoXide.
//!
//! Help Platty the Platypus escape Healesville Sanctuary, journey across the
//! bushland and city, and arrive at the ocean estuary to meet his new baby sibling!

#![no_std]
#![no_main]

extern crate psx_rt;

mod audio;
mod codec;
pub mod dualshock;
mod entities;
mod game;
mod level;
mod platypus;
mod renderer;
pub mod save;
pub mod texture;
mod title_bg;
mod title_bg_data;
pub mod video;

use audio::AudioManager;
use game::Game;

/// The game state lives in `.bss`, not on the stack.
///
/// `Game` embeds the level grid, the entity arrays and the CD sector buffers
/// (several KiB on their own), and `Game::new` additionally builds a 16 KiB
/// font-atlas staging buffer on the stack. The linker reserves only
/// `STACK_RESERVE` (32 KiB) below the stack top, so a stack-resident `Game`
/// plus that buffer left very little headroom for the rest of the frame. A
/// static costs nothing extra: the space is already reserved in `.bss`.
static mut GAME: Option<Game> = None;

#[no_mangle]
fn main() -> ! {
    // Initialize audio SPU and ADPCM voice channels
    AudioManager::init();

    // SAFETY: `main` is the only entry point, the game is single-threaded, and
    // this runs exactly once, so nothing else can observe the static while it
    // is initialised or in use. The raw pointer form avoids the
    // `static_mut_refs` lint, which assumes the possibility of aliasing that
    // cannot arise here.
    unsafe {
        let slot = core::ptr::addr_of_mut!(GAME);
        *slot = Some(Game::new());
        match (*slot).as_mut() {
            Some(game) => game.run(),
            None => loop {},
        }
    }
}
