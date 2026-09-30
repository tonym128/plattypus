//! `video-bench` -- performance measurement for Plattypus intro-video playback.
//!
//! Measures the shipped playback path, not a reimplementation of it: the
//! crate textually includes `game/src/video.rs` and `game/src/audio.rs`
//! and drives them with the same tick sequence `game.rs` uses for
//! `GameState::IntroVideo`. The only stand-in for the game is a
//! `renderer::Renderer` shim; `video.rs` reaches for exactly one thing on
//! it -- `fb.buffer_y(fb.drawing)` -- plus `fb.swap()` from `begin_frame`,
//! both of which are reproduced here.
//!
//! # Phases
//!
//! * **P0 `probe`** -- establish which clock exists on this target.
//! * **P1 `paced`** -- the headline numbers. Plays all 150 frames with
//!   the game's own pacing and records, per presented frame, the number
//!   of display periods between presents and the number the decode+upload
//!   work consumed. Everything the viewer would call a stutter is
//!   derived from this phase; it is driven only by the VBlank interrupt,
//!   so it is valid on any target.
//! * **P2 `burst`** -- stage attribution. The same pipeline run without
//!   display sync and with a stopwatch around each stage, so a stage's
//!   cost is averaged over 150 frames rather than rounded to whole display
//!   periods.
//! * **P3 `real`** -- closure check: the shipped `VideoPlayer::draw()`
//!   timed in burst mode, compared against the sum of the instrumented
//!   stages. A large gap means the instrumented copy has drifted from the
//!   game and the stage split should not be trusted.
//! * **P4 `integ`** -- decode integrity, so a run that "finished fast"
//!   because nothing was decoded cannot be mistaken for a good run.
//!
//! Output is a stream of `@@VB1` / `@@VBF` / `@@VBS` lines on the BIOS
//! TTY, parsed by `tools/video_bench/host_bench.py` into `report.json`
//! plus a text summary.

#![no_std]
#![no_main]
#![allow(dead_code)]

extern crate psx_rt;

mod fmt;
mod report;
mod stages;
mod timing;

#[path = "../../game/src/video.rs"]
mod video;

#[path = "../../game/src/audio.rs"]
mod audio;

/// Stand-in for `game::renderer::Renderer`.
///
/// `video.rs` uses the renderer only to learn the VRAM Y of the buffer
/// being drawn into. `Renderer::begin_frame` is `wait_vblank()` then
/// `fb.swap()`, reproduced verbatim.
mod renderer {
    use psx_gpu::framebuf::FrameBuffer;

    pub struct Renderer {
        pub fb: FrameBuffer,
    }

    impl Renderer {
        pub const fn new(fb: FrameBuffer) -> Self {
            Self { fb }
        }

        /// Same as `game::renderer::Renderer::begin_frame`.
        #[inline]
        pub fn begin_frame(&mut self) {
            psx_rt::interrupts::wait_vblank();
            self.fb.swap();
        }
    }
}

use psx_font::{fonts::BASIC, FontAtlas};
use psx_gpu::{self as gpu, Resolution, VideoMode};
use psx_pad::{ButtonState, PadState};
use psx_rt::{interrupts, tty};
use psx_vram::{Clut, TexDepth, Tpage};
use video::{VideoKind, VideoPlayer, TOTAL_FRAMES};

const FONT_TPAGE: Tpage = Tpage::new(320, 0, TexDepth::Bit4);
const FONT_CLUT: Clut = Clut::new(320, 256);
const FRAMES: usize = TOTAL_FRAMES as usize;

#[no_mangle]
fn main() -> ! {
    // `vblank_count()` must be live before anything is timed.
    interrupts::install_vblank_counter();

    let mut clock = timing::Clock::detect();
    let probe = timing::probe_counters();

    gpu::init(VideoMode::Ntsc, Resolution::R320X240);
    let mut renderer = renderer::Renderer::new(psx_gpu::framebuf::FrameBuffer::new(320, 240));
    renderer.fb.apply_draw_target();

    let font = FontAtlas::upload(&BASIC, FONT_TPAGE, FONT_CLUT);
    audio::AudioManager::init();

    // P0d: how much would DMA channel 2 save on the VRAM upload? Measured
    // first and in isolation, because it is the largest unused DMA
    // opportunity in the codebase and it does not depend on anything else.
    let (fifo_us, dma_us, dma_ok) = stages::measure_upload_paths(&mut clock);
    tty::println("@@VB1 UPLOAD_PATHS 1");
    tty::print("@@VB1 upload fifo_us=");
    tty::print_hex_u32(fifo_us);
    tty::print(" dma_us=");
    tty::print_hex_u32(dma_us);
    tty::print(" dma_ok=");
    tty::print_hex_u32(dma_ok as u32);
    tty::print("\n@@VB1 UPLOAD_PATHS 0");

    // P1: paced run -- the numbers that matter.
    let (presents, using_cd) = run_paced(&mut renderer);
    // Emit the headline numbers now, before the diagnostic phases, so a
    // wedge in one of those cannot cost us the deliverable.
    report::emit_paced_only(&presents, using_cd);

    // P2: burst stage attribution.
    let burst = stages::run_burst(&mut clock);

    // P2c: what the CD path can deliver on its own -- separates a
    // drive-throughput limit from a per-batch-seek cost.
    let cd_rate = stages::measure_cd_rate(&mut clock);

    // P3: closure check against the shipped draw().
    let real = run_real_player_burst(&renderer, &mut clock);

    // P4: decode integrity.
    let integ = stages::check_integrity();

    let summary = report::build_report(&probe, &presents, using_cd, &burst, real, integ, &cd_rate);
    report::emit(&summary);
    report::draw_on_screen(&font, &mut renderer.fb, &summary);

    loop {
        interrupts::wait_vblank();
    }
}

/// Replays the intro exactly as `GameState::IntroVideo` does.
///
/// The tick body below is `game.rs`'s `GameState::IntroVideo` arm with
/// the state transition replaced by "keep playing": the bench drives a
/// disconnected pad, so no skip button is ever held and the video runs to
/// its last frame on its own. `using_cd` is sampled before `stop()`, which
/// clears the flag.
fn run_paced(r: &mut renderer::Renderer) -> (stages::Presents, bool) {
    let mut player = VideoPlayer::new();
    player.start_video(VideoKind::Intro);

    let pad = PadState::NONE;
    let prev = ButtonState::NONE;
    let mut presents = stages::Presents::new();
    let mut prev_swap = u32::MAX;
    let mut using_cd = false;

    loop {
        audio::AudioManager::update();
        if player.update(&pad, &prev) {
            player.stop();
            break;
        }

        if player.needs_redraw() {
            r.begin_frame();
            let swap_vblank = interrupts::vblank_count();
            using_cd |= player.using_cd;

            let before = interrupts::vblank_count();
            player.draw(r);
            let work = interrupts::vblank_count().wrapping_sub(before);

            presents.push(stages::Present {
                // The first present has no predecessor to measure against.
                interval: if prev_swap == u32::MAX { 0 } else { swap_vblank.wrapping_sub(prev_swap) },
                work,
            });
            prev_swap = swap_vblank;
        } else {
            interrupts::wait_vblank();
        }
    }

    (presents, using_cd)
}

/// Times the shipped `VideoPlayer::draw()` with no display sync.
fn run_real_player_burst(r: &renderer::Renderer, clock: &mut timing::Clock) -> timing::Span {
    let mut player = VideoPlayer::new();
    player.start_video(VideoKind::Intro);

    let mut total = timing::Span::default();
    for frame in 0..TOTAL_FRAMES {
        // `draw()` decodes when `frame_idx` differs from the last decoded
        // frame, so advancing the public index drives it; no private state
        // is touched.
        player.frame_idx = frame;
        clock.start();
        player.draw(r);
        total += clock.lap();
    }
    player.stop();
    total
}
