//! Per-frame records and the instrumented pipeline used for stage
//! attribution.
//!
//! `run_burst` re-runs the shipped playback sequence with a stopwatch
//! around each stage. It exists because the shipped `VideoPlayer` is a
//! single opaque `draw()`: without a second copy there is no way to say
//! *which* stage a slow frame spent its time in, and that attribution is
//! what the optimisation work needs. The copy is kept honest by the
//! closure check in main.rs -- the `real` phase times the shipped
//! `draw()` under the same conditions, and the report prints the gap
//! between it and this module's `pipeline` total.

use psx_io::mdec;
use psx_pack::cd::{SectorReader, SECTOR_WORDS};
use psx_vram::{dma_copy_to_vram, upload_words, VramRect};
use crate::video::{
    SECTORS_PER_FRAME, TOTAL_FRAMES, VIDEO_H, WORDS_PER_FRAME, WORDS_PER_SLICE,
};

use crate::timing::{Clock, Span};

pub const FRAMES: usize = TOTAL_FRAMES as usize;
/// Compressed frames pulled per CD command, matching `VideoPlayer`.
const PREFETCH_FRAMES: u16 = 4;
/// Macroblock columns in a 320px-wide frame.
const SLICE_COLUMNS: u16 = 20;

/// One presented video frame, as observed from the VBlank counter.
#[derive(Copy, Clone, Default)]
pub struct Present {
    /// Display periods between this present and the previous one. The
    /// first present has no predecessor, so it records 0; callers treat
    /// 0 as "no interval" rather than as a stutter.
    pub interval: u32,
    /// Display periods the decode + VRAM upload consumed.
    pub work: u32,
}

/// Fixed-capacity array of [`Present`] (`no_std` has no `Vec`).
#[derive(Clone)]
pub struct Presents {
    pub items: [Present; FRAMES],
    pub count: usize,
}

impl Presents {
    pub const fn new() -> Self {
        Self {
            items: [Present {
                interval: 0,
                work: 0,
            }; FRAMES],
            count: 0,
        }
    }

    pub fn push(&mut self, p: Present) {
        if self.count < FRAMES {
            self.items[self.count] = p;
            self.count += 1;
        }
    }

    pub fn as_slice(&self) -> &[Present] {
        &self.items[..self.count]
    }

    pub fn iter(&self) -> core::slice::Iter<'_, Present> {
        self.as_slice().iter()
    }

    pub fn len(&self) -> usize {
        self.count
    }
}

/// Totals for the instrumented pipeline, one [`Span`] per stage.
pub struct Burst {
    /// Reading four frames' worth of sectors from the drive.
    pub read: Span,
    /// `frame_cache[slot] -> frame_words` (16 KiB memcpy).
    pub copy: Span,
    /// `start_decode_frame` plus twenty `drain_slice_dma`.
    pub decode: Span,
    /// Twenty `upload_words` into VRAM, the shipped GP0-FIFO path.
    pub upload: Span,
    /// The same twenty slices over DMA channel 2, for comparison.
    pub upload_dma: Span,
    /// Sum of the four stages above.
    pub pipeline: Span,
    /// Per-frame read VBlanks, for spotting frames that read late.
    pub read_per_frame: [u32; FRAMES],
    /// LBA the video was streamed from, or 0 when the disc scan failed.
    pub video_lba: u32,
    /// Whether the root-directory scan found INTRO.VID.
    pub located: bool,
}

/// BSS storage mirroring the shipped `VideoStorage`.
struct BenchStorage {
    frame_words: [u32; WORDS_PER_FRAME],
    frame_cache: [[u32; WORDS_PER_FRAME]; PREFETCH_FRAMES as usize],
    slice_words: [u32; WORDS_PER_SLICE],
    sector_buf: [u32; SECTOR_WORDS],
}

impl BenchStorage {
    const fn new() -> Self {
        Self {
            frame_words: [0; WORDS_PER_FRAME],
            frame_cache: [[0; WORDS_PER_FRAME]; PREFETCH_FRAMES as usize],
            slice_words: [0; WORDS_PER_SLICE],
            sector_buf: [0; SECTOR_WORDS],
        }
    }
}

static mut STORAGE: BenchStorage = BenchStorage::new();

#[inline]
fn storage() -> &'static mut BenchStorage {
    unsafe { &mut *core::ptr::addr_of_mut!(STORAGE) }
}

/// Bring up a `SectorReader` and return the LBA of `name` in the ISO 9660
/// root directory. Mirrors `VideoPlayer::start_video`'s CD bring-up.
fn open_video(name: &[u8], reader: &mut SectorReader) -> Option<u32> {
    if !unsafe { reader.prepare() } {
        return None;
    }
    let s = storage();
    if !unsafe { reader.start_read(20) } {
        return None;
    }
    let ok = unsafe { reader.read_sector(&mut s.sector_buf) };
    unsafe { reader.stop() };
    if !ok {
        return None;
    }

    let bytes: &[u8] = unsafe {
        core::slice::from_raw_parts(s.sector_buf.as_ptr() as *const u8, 2048)
    };

    let mut off = 0usize;
    while off + 34 <= bytes.len() {
        let record_len = bytes[off] as usize;
        if record_len == 0 || off + record_len > bytes.len() {
            break;
        }
        let lba = u32::from_le_bytes([bytes[off + 2], bytes[off + 3], bytes[off + 4], bytes[off + 5]]);
        let name_len = bytes[off + 32] as usize;
        if off + 33 + name_len <= bytes.len() {
            let entry = &bytes[off + 33..off + 33 + name_len];
            if entry.starts_with(name) {
                return Some(lba);
            }
        }
        off += record_len;
    }
    None
}

/// Read `count` frames' worth of sectors starting at `first_frame` into
/// the cache, timing the whole burst.
fn read_batch(
    reader: &mut SectorReader,
    video_lba: u32,
    first_frame: u16,
    count: u16,
    clock: &mut Clock,
) -> Span {
    let s = storage();
    clock.start();
    let lba = video_lba.wrapping_add(first_frame as u32 * SECTORS_PER_FRAME as u32);
    if unsafe { reader.start_read(lba) } {
        for f in 0..count as usize {
            for sector in 0..SECTORS_PER_FRAME {
                let offset = sector * SECTOR_WORDS;
                let buf: &mut [u32; SECTOR_WORDS] = unsafe {
                    &mut *(s.frame_cache[f][offset..offset + SECTOR_WORDS].as_mut_ptr()
                        as *mut [u32; SECTOR_WORDS])
                };
                unsafe { reader.read_sector(buf) };
            }
        }
    }
    // The drive is paused at the end of the burst so MDEC work cannot
    // overrun the sector FIFO, exactly as the shipped player does.
    unsafe { reader.stop() };
    clock.lap()
}

/// Run the pipeline over all 150 frames with no display sync, timing each
/// stage.
pub fn run_burst(clock: &mut Clock) -> Burst {
    let s = storage();
    let mut reader = SectorReader::new();
    let video_lba = open_video(b"INTRO.VID", &mut reader).unwrap_or(0);
    let located = video_lba != 0;

    let mut out = Burst {
        read: Span::default(),
        copy: Span::default(),
        decode: Span::default(),
        upload: Span::default(),
        upload_dma: Span::default(),
        pipeline: Span::default(),
        read_per_frame: [0; FRAMES],
        video_lba,
        located,
    };
    if !located {
        return out;
    }

    let mut cached_start = 0u16;
    let mut cached_count = 0u16;

    for frame in 0..TOTAL_FRAMES {
        let batch_start = (frame / PREFETCH_FRAMES) * PREFETCH_FRAMES;
        let batch_end = core::cmp::min(batch_start + PREFETCH_FRAMES, TOTAL_FRAMES);

        if frame < cached_start || frame >= cached_start + cached_count {
            let read = read_batch(&mut reader, video_lba, batch_start, batch_end - batch_start, clock);
            out.read += read;
            out.read_per_frame[frame as usize] = read.vblanks;
            cached_start = batch_start;
            cached_count = batch_end - batch_start;
        }

        let slot = (frame - cached_start) as usize;

        clock.start();
        s.frame_words.copy_from_slice(&s.frame_cache[slot]);
        out.copy += clock.lap();

        clock.start();
        mdec::start_decode_frame(&s.frame_words);
        for _ in 0..SLICE_COLUMNS {
            mdec::drain_slice_dma(&mut s.slice_words);
        }
        out.decode += clock.lap();

        clock.start();
        for col in 0..SLICE_COLUMNS {
            let rect = VramRect::new(col * 16, 0, 16, VIDEO_H);
            upload_words(rect, &s.slice_words);
        }
        out.upload += clock.lap();

        // Same payload, same destination geometry, but pushed over DMA
        // channel 2 instead of the GP0 command port. This is the
        // opt-in path `psx-vram` ships but nothing calls.
        clock.start();
        let mut dma_ok = true;
        for col in 0..SLICE_COLUMNS {
            let rect = VramRect::new(col * 16, 0, 16, VIDEO_H);
            if !dma_copy_to_vram(rect, s.slice_words.as_ptr()) {
                dma_ok = false;
                break;
            }
        }
        out.upload_dma = clock.lap();
        if !dma_ok {
            out.upload_dma = Span::default();
        }
    }

    out.pipeline = out.read + out.copy + out.decode + out.upload;
    out
}

/// Decode one streamed frame and summarise the decoded pixels, so a run
/// that skipped the CD and fell back to the embedded frames -- or that
/// decoded garbage -- is visible in the report rather than showing up as
/// an implausibly fast time.
pub fn check_integrity() -> (u32, u32, bool) {
    let s = storage();
    let mut reader = SectorReader::new();
    let Some(video_lba) = open_video(b"INTRO.VID", &mut reader) else {
        return (0, 0, false);
    };

    // Pull the first frame's eight sectors straight into frame_words.
    clockless_reset();
    if !unsafe { reader.start_read(video_lba) } {
        return (0, 0, false);
    }
    let mut ok = true;
    for sector in 0..SECTORS_PER_FRAME {
        let offset = sector * SECTOR_WORDS;
        let buf: &mut [u32; SECTOR_WORDS] = unsafe {
            &mut *(s.frame_words[offset..offset + SECTOR_WORDS].as_mut_ptr() as *mut [u32; SECTOR_WORDS])
        };
        if !unsafe { reader.read_sector(buf) } {
            ok = false;
            break;
        }
    }
    unsafe { reader.stop() };
    if !ok {
        return (0, 0, false);
    }

    mdec::start_decode_frame(&s.frame_words);
    let mut checksum = 0u32;
    let mut nonzero = 0u32;
    for _col in 0..SLICE_COLUMNS {
        mdec::drain_slice_dma(&mut s.slice_words);
        for &w in s.slice_words.iter() {
            checksum = checksum.rotate_left(5) ^ w;
            if w != 0 {
                nonzero += 1;
            }
        }
    }

    (checksum, nonzero, true)
}

/// Drop any MDEC state left by an earlier phase.
fn clockless_reset() {
    mdec::init();
}

/// Measures of what the CD path can actually deliver, independent of
/// video: how long a pure contiguous stream takes, and how much of that
/// a re-seek per batch costs.
///
/// This separates the two candidate causes of a read-bound pipeline. If
/// the contiguous rate is near the drive's rated throughput, the drive is
/// the limit and the only fixes are to read less (smaller bitrate) or to
/// overlap reading with playback. If the contiguous rate is much higher
/// than the per-batch rate, the per-batch `Setloc` is what costs, and
/// overlapping the seek -- or streaming continuously and caching without
/// ever stopping the drive -- is the fix.
pub struct CdRate {
    /// Sectors streamed with one `Setloc`+`ReadN`, drive left running.
    pub contiguous_sectors: u32,
    /// Display periods that took.
    pub contiguous_vblanks: u32,
    /// Sectors streamed as `4` batches of `8` sectors, re-seeking each
    /// batch and pausing the drive after it, exactly as the game does.
    pub batched_sectors: u32,
    pub batched_vblanks: u32,
}

pub fn measure_cd_rate(clock: &mut Clock) -> CdRate {
    let s = storage();
    let mut reader = SectorReader::new();
    let Some(video_lba) = open_video(b"INTRO.VID", &mut reader) else {
        return CdRate {
            contiguous_sectors: 0,
            contiguous_vblanks: 0,
            batched_sectors: 0,
            batched_vblanks: 0,
        };
    };

    // 1. One long contiguous ReadN: 128 sectors, one seek, no pause.
    //    Far enough past the file end that it stays inside the disc image.
    let contiguous = 128u32;
    clock.start();
    if unsafe { reader.start_read(video_lba) } {
        for i in 0..contiguous {
            let offset = (i as usize % 4) * SECTOR_WORDS;
            let slot = (i as usize / 4) % PREFETCH_FRAMES as usize;
            let buf: &mut [u32; SECTOR_WORDS] = unsafe {
                &mut *(s.frame_cache[slot][offset..offset + SECTOR_WORDS].as_mut_ptr()
                    as *mut [u32; SECTOR_WORDS])
            };
            unsafe { reader.read_sector(buf) };
        }
    }
    unsafe { reader.stop() };
    let contiguous_vblanks = clock.lap().vblanks;

    // 2. The shipped pattern: four batches of eight sectors, each
    //    re-seeking and pausing the drive afterwards.
    let batched = 32u32;
    clock.start();
    for b in 0..4u16 {
        let lba = video_lba.wrapping_add(b as u32 * SECTORS_PER_FRAME as u32);
        if unsafe { reader.start_read(lba) } {
            for i in 0..SECTORS_PER_FRAME {
                let offset = (i % SECTORS_PER_FRAME) * SECTOR_WORDS;
                let buf: &mut [u32; SECTOR_WORDS] = unsafe {
                    &mut *(s.frame_cache[i % PREFETCH_FRAMES as usize][offset..offset + SECTOR_WORDS]
                        .as_mut_ptr() as *mut [u32; SECTOR_WORDS])
                };
                unsafe { reader.read_sector(buf) };
            }
        }
        unsafe { reader.stop() };
    }
    let batched_vblanks = clock.lap().vblanks;

    CdRate {
        contiguous_sectors: contiguous,
        contiguous_vblanks,
        batched_sectors: batched,
        batched_vblanks,
    }
}

/// Time the two ways of getting one decoded frame into VRAM, with no CD
/// read and no decode in the way.
///
/// `psx-vram` ships an opt-in DMA channel 2 path (`dma_copy_to_vram`) that
/// nothing in the project calls, and the shipped path pushes the same
/// bytes through the GP0 command port one word at a time. This puts a
/// number on the difference instead of leaving it as an estimate, and
/// deliberately does not depend on the rest of the pipeline so a wedge
/// elsewhere cannot hide it.
pub fn measure_upload_paths(clock: &mut Clock) -> (u32, u32, bool) {
    const REPS: u32 = 150;

    // One representative decoded slice. Contents do not matter for a
    // throughput measurement; only the size and destination do.
    let mut slice = [0u32; WORDS_PER_SLICE];
    for (i, w) in slice.iter_mut().enumerate() {
        *w = 0x1234_5678 ^ (i as u32);
    }

    clock.start();
    for _ in 0..REPS {
        for col in 0..SLICE_COLUMNS {
            upload_words(VramRect::new(col * 16, 0, 16, VIDEO_H), &slice);
        }
    }
    let fifo = clock.lap();

    clock.start();
    let mut ok = true;
    for _ in 0..REPS {
        for col in 0..SLICE_COLUMNS {
            if !dma_copy_to_vram(VramRect::new(col * 16, 0, 16, VIDEO_H), slice.as_ptr()) {
                ok = false;
                break;
            }
        }
        if !ok {
            break;
        }
    }
    let dma = clock.lap();

    let per_frame = |span: Span| -> u32 {
        (span.vblanks as u64 * crate::timing::US_PER_VBLANK as u64 / REPS as u64) as u32
    };
    (per_frame(fifo), per_frame(dma), ok)
}
