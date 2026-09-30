//! Metric definitions and TTY emission.
//!
//! # What counts as a stutter
//!
//! The video is 15 fps on a 60 Hz display, so an ideal present interval
//! is exactly [`VBLANKS_PER_VIDEO_FRAME`] display periods. The present
//! happens in `FrameBuffer::swap()`, which `begin_frame` calls *before*
//! `draw()` writes the next frame into the newly-current back buffer. A
//! frame is therefore only ever shown for as long as the interval between
//! two swaps, and `draw()` has to finish inside that same interval.
//!
//! `draw()` starts immediately after a swap, so it has one display period
//! of uninterrupted time before the next swap's `wait_vblank()`. Overrun
//! it by even a fraction and the following tick's `wait_vblank()` has
//! already missed its edge, pushing that swap out by a whole display
//! period. So:
//!
//! * `interval != 4` on a present means the viewer saw a repeat or a
//!   hold. That is the stutter count.
//! * `work >= 1` display period means `draw()` overran, which is the
//!   cause. A frame with `work == 0` and `interval == 5` was instead a
//!   scheduling hiccup elsewhere.
//!
//! Both are reported, and separately, so a fix can be aimed at the right
//! thing.

use psx_rt::tty;

use crate::stages::{Burst, Presents};
use crate::timing::{Clock, ClockKind, CounterProbe, Span, VBLANKS_PER_VIDEO_FRAME};

const FRAMES: usize = crate::stages::FRAMES;
/// `FRAMES` as the `u32` the timing helpers take.
const FRAMES_U32: u32 = FRAMES as u32;

/// Everything the host harness reads, already reduced to scalars.
pub struct Summary {
    pub clock: ClockKind,
    pub rcnt_live: bool,
    pub overflow_live: bool,

    pub using_cd: bool,
    pub video_lba: u32,
    pub located: bool,

    pub presented: u32,
    pub fps_x1000: u32,
    pub interval_min: u32,
    pub interval_mean: u32,
    pub interval_p95: u32,
    pub interval_max: u32,
    pub stutters: u32,
    pub avg_frame_time_us: u32,

    pub work_vb_mean_x100: u32,
    pub work_vb_max: u32,
    pub work_overruns: u32,

    pub read_us: u32,
    pub copy_us: u32,
    pub decode_us: u32,
    pub upload_us: u32,
    /// Same payload over DMA channel 2, so the size of the unused
    /// opportunity is a measurement rather than an estimate.
    pub upload_dma_us: u32,
    /// Times the channel wedged and the shipped fallback took over.
    pub vram_dma_fallbacks: u32,
    pub pipeline_us: u32,
    pub budget_pct: u32,
    pub read_pct: u32,
    pub decode_pct: u32,
    pub upload_pct: u32,
    pub copy_pct: u32,

    pub real_us: u32,
    pub closure_gap_pct: i32,

    pub integ_ok: bool,
    pub integ_checksum: u32,
    pub integ_nonzero: u32,

    /// Raw per-present records, for the host's distribution analysis.
    pub per_presents: Presents,
    /// Burst-phase CD read cost attributed to each frame index.
    pub read_per_frame: [u32; FRAMES],

    /// Sectors streamed in one `Setloc`+`ReadN` and the display periods
    /// that took. Raw counts, not a rate: a rate rounded to whole sectors
    /// per period loses the difference between 1x and 2x drives.
    pub cd_contiguous_sectors: u32,
    pub cd_contiguous_vblanks: u32,
    /// Same, in the shipped per-batch re-seek-and-pause pattern.
    pub cd_batched_sectors: u32,
    pub cd_batched_vblanks: u32,
}

/// Reduce a [`Burst`] and a [`Presents`] run to the reported scalars.
#[allow(clippy::too_many_arguments)]
pub fn build_report(
    probe: &CounterProbe,
    presents: &Presents,
    using_cd: bool,
    burst: &Burst,
    real: Span,
    integ: (u32, u32, bool),
    cd_rate: &crate::stages::CdRate,
) -> Summary {
    let clock_kind = probe.clock;
    let us_per_vblank = crate::timing::US_PER_VBLANK;

    // ---- paced phase -------------------------------------------------
    // A present whose interval is 0 is the first one: it has no
    // predecessor, so there is no interval to judge. It still counts
    // towards `presented`, and its `work` still counts, but it is not a
    // stutter and not part of the interval distribution.
    let items = presents.as_slice();
    let presented = items.len() as u32;
    let mut measured = 0u32;
    let mut iv_sum = 0u64;
    let mut iv_min = u32::MAX;
    let mut iv_max = 0u32;
    let mut stutters = 0u32;
    for p in items {
        if p.interval == 0 {
            continue;
        }
        measured += 1;
        iv_sum += p.interval as u64;
        if p.interval < iv_min {
            iv_min = p.interval;
        }
        if p.interval > iv_max {
            iv_max = p.interval;
        }
        if p.interval != VBLANKS_PER_VIDEO_FRAME {
            stutters += 1;
        }
    }
    if measured == 0 {
        iv_min = 0;
    }
    let iv_mean = if measured > 0 {
        (iv_sum / measured as u64) as u32
    } else {
        VBLANKS_PER_VIDEO_FRAME
    };

    // Percentile over the intervals actually seen. Counting sort: the
    // value domain is tiny.
    let mut iv_hist = [0u32; 64];
    for p in items {
        if p.interval != 0 {
            iv_hist[core::cmp::min(p.interval as usize, 63)] += 1;
        }
    }
    let p95 = percentile_from_hist(&iv_hist, measured, 95);

    // Presented frame rate: 60 Hz display / mean interval.
    let fps_x1000 = if iv_mean > 0 {
        60_000 / iv_mean
    } else {
        0
    };
    let avg_frame_time_us = iv_mean * us_per_vblank;

    // Work overrun: `draw()` consuming a whole display period or more is
    // what pushes the following swap late.
    let mut work_sum = 0u64;
    let mut work_max = 0u32;
    let mut work_overruns = 0u32;
    for p in items {
        work_sum += p.work as u64;
        if p.work > work_max {
            work_max = p.work;
        }
        if p.work >= 1 {
            work_overruns += 1;
        }
    }
    // Resolution note: `work` is whole display periods, so a mean over
    // the run resolves to 1/presented of a period. Report it scaled by
    // 100 so the host can show a percentage of the one-period budget.
    let work_vb_mean_x100 = if presented > 0 {
        (work_sum * 100 / presented as u64) as u32
    } else {
        0
    };

    // ---- burst phase -------------------------------------------------
    // The clock used for the burst is the one `detect()` chose; re-derive
    // it here so the burst's Span values are read consistently.
    let burst_clock = Clock::of(clock_kind);
    let read_us = burst_clock.us_per_sample(burst.read, FRAMES_U32);
    let copy_us = burst_clock.us_per_sample(burst.copy, FRAMES_U32);
    let decode_us = burst_clock.us_per_sample(burst.decode, FRAMES_U32);
    let upload_us = burst_clock.us_per_sample(burst.upload, FRAMES_U32);
    // `upload_dma` here is the GP0-only cost, kept as the comparison.
    let upload_dma_us = burst_clock.us_per_sample(burst.upload_dma, FRAMES_U32);
    let pipeline_us = burst_clock.us_per_sample(burst.pipeline, FRAMES_U32);
    let real_us = burst_clock.us_per_sample(real, FRAMES_U32);

    let budget = us_per_vblank;
    let pct = |v: u32| core::cmp::min(v * 100 / budget, 999);

    let closure_gap_pct = if real_us > 0 {
        (pipeline_us as i64 - real_us as i64) * 100 / real_us as i64
    } else {
        0
    } as i32;

    let (integ_checksum, integ_nonzero, integ_ok) = integ;

    Summary {
        clock: clock_kind,
        rcnt_live: probe.rcnt_per_vblank != 0,
        overflow_live: probe.overflow_moved,
        using_cd,
        video_lba: burst.video_lba,
        located: burst.located,
        presented,
        fps_x1000,
        interval_min: iv_min,
        interval_mean: iv_mean,
        interval_p95: p95,
        interval_max: iv_max,
        stutters,
        avg_frame_time_us,
        work_vb_mean_x100,
        work_vb_max: work_max,
        work_overruns,
        read_us,
        copy_us,
        decode_us,
        upload_us,
        upload_dma_us,
        vram_dma_fallbacks: burst.vram_dma_fallbacks,
        pipeline_us,
        budget_pct: pct(pipeline_us),
        read_pct: pct(read_us),
        copy_pct: pct(copy_us),
        decode_pct: pct(decode_us),
        upload_pct: pct(upload_us),
        real_us,
        closure_gap_pct,
        integ_ok,
        integ_checksum,
        integ_nonzero,
        per_presents: presents.clone(),
        read_per_frame: burst.read_per_frame,
        cd_contiguous_sectors: cd_rate.contiguous_sectors,
        cd_contiguous_vblanks: cd_rate.contiguous_vblanks,
        cd_batched_sectors: cd_rate.batched_sectors,
        cd_batched_vblanks: cd_rate.batched_vblanks,
    }
}

fn percentile_from_hist(hist: &[u32; 64], total: u32, percent: u32) -> u32 {
    if total == 0 {
        return 0;
    }
    let target = (total as u64 * percent as u64 / 100).max(1);
    let mut acc = 0u64;
    for (value, &count) in hist.iter().enumerate() {
        acc += count as u64;
        if acc >= target {
            return value as u32;
        }
    }
    0
}

/// Emit the machine-readable report on the TTY.
pub fn emit(s: &Summary) {
    let kv = |tag: &str, name: &str, value: u32| {
        tty::print("@@VB1 ");
        tty::print(tag);
        tty::print(" ");
        tty::print(name);
        tty::print("=");
        tty::print_hex_u32(value);
        tty::print("\n");
    };

    tty::println("@@VB1 BEGIN");

    kv("probe", "clock", s.clock.code());
    kv("probe", "rcnt_live", s.rcnt_live as u32);
    kv("probe", "overflow_live", s.overflow_live as u32);

    kv("env", "located", s.located as u32);
    kv("env", "using_cd", s.using_cd as u32);
    kv("env", "video_lba", s.video_lba);

    kv("paced", "presented", s.presented);
    kv("paced", "fps_x1000", s.fps_x1000);
    kv("paced", "interval_min", s.interval_min);
    kv("paced", "interval_mean", s.interval_mean);
    kv("paced", "interval_p95", s.interval_p95);
    kv("paced", "interval_max", s.interval_max);
    kv("paced", "stutters", s.stutters);
    kv("paced", "avg_frame_time_us", s.avg_frame_time_us);
    kv("paced", "work_vb_mean_x100", s.work_vb_mean_x100);
    kv("paced", "work_vb_max", s.work_vb_max);
    kv("paced", "work_overruns", s.work_overruns);

    kv("burst", "read_us", s.read_us);
    kv("burst", "copy_us", s.copy_us);
    kv("burst", "decode_us", s.decode_us);
    kv("burst", "upload_us", s.upload_us);
    kv("burst", "upload_gp0_us", s.upload_dma_us);
    kv("burst", "vram_dma_fallbacks", s.vram_dma_fallbacks);
    kv("burst", "pipeline_us", s.pipeline_us);
    kv("burst", "budget_pct", s.budget_pct);
    kv("burst", "read_pct", s.read_pct);
    kv("burst", "copy_pct", s.copy_pct);
    kv("burst", "decode_pct", s.decode_pct);
    kv("burst", "upload_pct", s.upload_pct);

    kv("real", "total_us", s.real_us);
    kv("real", "closure_gap_pct", s.closure_gap_pct as u32);

    kv("cdrate", "contiguous_sectors", s.cd_contiguous_sectors);
    kv("cdrate", "contiguous_vblanks", s.cd_contiguous_vblanks);
    kv("cdrate", "batched_sectors", s.cd_batched_sectors);
    kv("cdrate", "batched_vblanks", s.cd_batched_vblanks);

    kv("integ", "ok", s.integ_ok as u32);
    kv("integ", "checksum", s.integ_checksum);
    kv("integ", "nonzero", s.integ_nonzero);

    // Per-present rows: interval, work, and the read cost of that
    // present. Emitted so the host can build a distribution and point at
    // the individual frames that went late, rather than only reporting an
    // average that hides them.
    for (i, p) in s.per_presents.iter().enumerate() {
        tty::print("@@VBP ");
        tty::print_hex_u32(i as u32);
        tty::print(" ");
        tty::print_hex_u32(p.interval);
        tty::print(" ");
        tty::print_hex_u32(p.work);
        tty::print(" ");
        tty::print_hex_u32(s.read_per_frame.get(i).copied().unwrap_or(0));
        tty::print("\n");
    }

    tty::println("@@VB1 END");
}

/// Draw the headline numbers on screen so a human running the disc in an
/// emulator sees the same verdict without capturing stdout.
pub fn draw_on_screen(
    font: &psx_font::FontAtlas,
    fb: &mut psx_gpu::framebuf::FrameBuffer,
    s: &Summary,
) {
    use crate::fmt::Line;

    const GOOD: (u8, u8, u8) = (80, 220, 100);
    const BAD: (u8, u8, u8) = (230, 80, 80);
    const WHITE: (u8, u8, u8) = (210, 210, 220);
    const ACCENT: (u8, u8, u8) = (110, 200, 240);
    const DIM: (u8, u8, u8) = (150, 150, 160);

    fb.apply_draw_target();
    fb.clear(8, 10, 18);

    let mut l = Line::new();
    let mut y: i16 = 8;

    font.draw_text(8, y, "INTRO VIDEO PERF", ACCENT);
    y += 14;

    l.clear().text(b"clock  ").text(s.clock.name().as_bytes());
    font.draw_text(8, y, l.as_str(), DIM);
    y += 12;

    l.clear().text(b"fps    ").fixed3(s.fps_x1000);
    font.draw_text(8, y, l.as_str(), if s.stutters == 0 { GOOD } else { BAD });
    y += 14;

    l.clear().text(b"stutter ").num(s.stutters, 3);
    font.draw_text(8, y, l.as_str(), if s.stutters == 0 { GOOD } else { BAD });
    y += 14;

    l.clear()
        .text(b"interval ")
        .num(s.interval_mean, 1)
        .ch(b' ')
        .text(b"vb / ")
        .num(s.avg_frame_time_us / 1000, 2)
        .ch(b'.')
        .num((s.avg_frame_time_us % 1000) / 10, 1)
        .ch(b'm')
        .text(b"s");
    font.draw_text(8, y, l.as_str(), WHITE);
    y += 14;

    l.clear().text(b"work   ").num(s.work_vb_mean_x100, 3).ch(b'%').text(b" of 1 vb");
    font.draw_text(8, y, l.as_str(), if s.work_overruns == 0 { GOOD } else { BAD });
    y += 16;

    l.clear().text(b"budget ").num(s.budget_pct, 3).ch(b'%');
    font.draw_text(8, y, l.as_str(), if s.budget_pct > 100 { BAD } else { GOOD });
    y += 16;

    font.draw_text(8, y, "per-frame us", DIM);
    y += 12;

    l.clear().text(b"  read   ").num(s.read_us, 5);
    font.draw_text(8, y, l.as_str(), WHITE);
    y += 12;

    l.clear().text(b"  copy   ").num(s.copy_us, 5);
    font.draw_text(8, y, l.as_str(), WHITE);
    y += 12;

    l.clear().text(b"  decode ").num(s.decode_us, 5);
    font.draw_text(8, y, l.as_str(), WHITE);
    y += 12;

    l.clear().text(b"  upload ").num(s.upload_us, 5);
    font.draw_text(8, y, l.as_str(), WHITE);
    y += 16;

    let verdict: &[u8] = if s.stutters == 0 && s.using_cd && s.integ_ok {
        b"RESULT: SMOOTH"
    } else if !s.using_cd {
        b"RESULT: CD FALLBACK"
    } else {
        b"RESULT: STUTTERING"
    };
    let colour = if s.stutters == 0 && s.using_cd && s.integ_ok {
        GOOD
    } else {
        BAD
    };
    l.clear().text(verdict);
    font.draw_text(8, y, l.as_str(), colour);
}

/// Emit just the paced metrics, immediately after the paced phase.
///
/// The paced numbers are the deliverable; the later stage-attribution
/// phases are diagnostics. If one of those wedges (the CD read path has
/// several emulator-timing-sensitive spots), this still gets the frame
/// rate, interval distribution, stutter count and average frame time to
/// the host rather than losing the whole run.
pub fn emit_paced_only(presents: &Presents, using_cd: bool, vram_dma_fallbacks: u32) {
    let items = presents.as_slice();
    let presented = items.len() as u32;
    let mut iv_sum = 0u64;
    let mut iv_min = u32::MAX;
    let mut iv_max = 0u32;
    let mut stutters = 0u32;
    let mut measured = 0u32;
    for p in items {
        if p.interval == 0 { continue; }
        measured += 1;
        iv_sum += p.interval as u64;
        if p.interval < iv_min { iv_min = p.interval; }
        if p.interval > iv_max { iv_max = p.interval; }
        if p.interval != VBLANKS_PER_VIDEO_FRAME { stutters += 1; }
    }
    if measured == 0 { iv_min = 0; }
    let iv_mean = if measured > 0 { (iv_sum / measured as u64) as u32 } else { VBLANKS_PER_VIDEO_FRAME };
    let fps_x1000 = if iv_mean > 0 { 60_000 / iv_mean } else { 0 };
    let avg_us = iv_mean * crate::timing::US_PER_VBLANK;
    let kv = |tag: &str, name: &str, value: u32| {
        tty::print("@@VB1 "); tty::print(tag); tty::print(" ");
        tty::print(name); tty::print("=");
        tty::print_hex_u32(value); tty::print("\n");
    };
    tty::println("@@VB1 PACED_ONLY 1");
    kv("paced_only", "using_cd", using_cd as u32);
    kv("paced_only", "vram_dma_fallbacks", vram_dma_fallbacks);
    kv("paced_only", "presented", presented);
    kv("paced_only", "fps_x1000", fps_x1000);
    kv("paced_only", "interval_min", iv_min);
    kv("paced_only", "interval_mean", iv_mean);
    kv("paced_only", "interval_max", iv_max);
    kv("paced_only", "stutters", stutters);
    kv("paced_only", "avg_frame_time_us", avg_us);
    tty::println("@@VB1 PACED_ONLY 0");
}
