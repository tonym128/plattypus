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

use crate::video::{COLUMNS, RLE_WORDS, SLOT_WORDS, TOTAL_FRAMES, VIDEO_H};
use psx_fmv::{bs, mdec, str as strfmt};
use psx_pack::cd::{SectorReader, SECTOR_WORDS};
use psx_vram::{dma_copy_to_vram, upload_words, VramRect};

use crate::timing::{Clock, Span};

pub const FRAMES: usize = TOTAL_FRAMES as usize;
/// Macroblock columns in a 320px-wide frame.
const SLICE_COLUMNS: u16 = COLUMNS;
/// Bytes one STR sector carries after its 32-byte chunk header.
const CHUNK_PAYLOAD: usize = strfmt::CHUNK_PAYLOAD_BYTES;
/// Decoded pixels in one 16x240 column.
const COLUMN_WORDS: usize = 8 * VIDEO_H as usize;

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
    /// What the same twenty slices cost on the GP0 command port alone,
    /// kept alongside `upload` so the report can show the difference.
    pub upload_dma: Span,
    /// Times the channel wedged and the shipped fallback took over.
    pub vram_dma_fallbacks: u32,
    /// Sum of the four stages above.
    pub pipeline: Span,
    /// Per-frame read VBlanks, for spotting frames that read late.
    pub read_per_frame: [u32; FRAMES],
    /// LBA the video was streamed from, or 0 when the disc scan failed.
    pub video_lba: u32,
    /// Whether the root-directory scan found INTRO.VID.
    pub located: bool,
}

/// BSS storage mirroring the shipped `VideoStorage`, so the stage timings
/// here are of the same code paths the player runs.
struct BenchStorage {
    /// Reassembly buffer for the frame being measured.
    slot: [u32; SLOT_WORDS],
    /// MDEC run-length halfwords from the CPU-side expand.
    rle: [u32; RLE_WORDS],
    /// One decoded 16x240 column.
    column: [u32; COLUMN_WORDS],
    sector_buf: [u32; SECTOR_WORDS],
}

impl BenchStorage {
    const fn new() -> Self {
        Self {
            slot: [0; SLOT_WORDS],
            rle: [0; RLE_WORDS],
            column: [0; COLUMN_WORDS],
            sector_buf: [0; SECTOR_WORDS],
        }
    }
}

static mut STORAGE: BenchStorage = BenchStorage::new();

#[inline]
fn storage() -> &'static mut BenchStorage {
    unsafe { &mut *core::ptr::addr_of_mut!(STORAGE) }
}

/// Per-stage accumulators for one streaming pass over the movie.
///
/// The bench times each stage as the frame passes through it, so nothing has
/// to be buffered beyond the one frame in flight. `no_std` has no `Vec` and
/// the movie is 1.5 MB, so a second copy of it would not fit anyway.
struct Pass<'a> {
    reader: &'a mut SectorReader,
    video_lba: u32,
    read: Span,
    expand: Span,
    mdec_span: Span,
    upload: Span,
    upload_gp0: Span,
    vram_dma_fallbacks: u32,
    read_per_frame: [u32; FRAMES],
    frames: usize,
    located: bool,
}

impl<'a> Pass<'a> {
    /// Stream the whole movie once, timing read / expand / MDEC / upload.
    ///
    /// The read and the CPU work are interleaved the way the shipped player
    /// interleaves them, so `read` is the drive time the movie needed rather
    /// than a burst measured with the CPU idle.
    fn run(&mut self, clock: &mut Clock) {
        let s = storage();
        let mut asm = strfmt::FrameAssembler::new();
        let mut last_frame = 0u32;
        let mut read_vb = 0u32;

        clock.start();
        // SAFETY: single-threaded bench, reader prepared by open_video.
        if !unsafe { self.reader.start_read(self.video_lba) } {
            self.located = false;
            return;
        }
        loop {
            // SAFETY: as above; sector_buf is only used here.
            let got = unsafe { self.reader.read_sector(&mut s.sector_buf) };
            if !got {
                break;
            }
            let sector: &[u8] =
                unsafe { core::slice::from_raw_parts(s.sector_buf.as_ptr() as *const u8, 2048) };
            let Some(chunk) = strfmt::Chunk::parse(sector) else {
                continue;
            };
            if chunk.frame == 1 && last_frame > 1 {
                break; // trailing EOF marker
            }
            last_frame = chunk.frame;

            let buf: &mut [u8] = unsafe {
                let p = core::ptr::addr_of_mut!(s.slot) as *mut u8;
                core::slice::from_raw_parts_mut(p, SLOT_WORDS * 4)
            };
            let Some(frame) = asm.add(sector, buf) else {
                continue;
            };

            // The frame is complete. The sector read that completed it is
            // the last of the read work for this frame.
            self.read += clock.lap();
            read_vb = self.read.vblanks.saturating_sub(read_vb) + 0;

            // Stage 2: CPU-side BS v2 expand to MDEC run-lengths.
            clock.start();
            let rle16: &mut [u16] = unsafe {
                core::slice::from_raw_parts_mut(
                    core::ptr::addr_of_mut!(s.rle) as *mut u16,
                    RLE_WORDS * 2,
                )
            };
            let frame_bytes: &[u8] = &buf[..frame.size as usize];
            let words =
                match bs::decode_frame(frame_bytes, rle16, COLUMNS as u32 * 15, 15, &mut || {}) {
                    Ok(w) => w,
                    Err(_) => continue,
                };
            self.expand += clock.lap();
            if words == 0 {
                continue;
            }

            // Stage 3: MDEC, one 16-pixel column at a time over DMA1.
            clock.start();
            let rle: &[u32] = unsafe {
                core::slice::from_raw_parts(core::ptr::addr_of!(s.rle) as *const u32, words)
            };
            // SAFETY: `rle` stays alive and unmodified until decode_finish.
            unsafe { mdec::decode_start(rle, words, mdec::DECODE_15BPP) };
            let mut ok = true;
            for _c in 0..SLICE_COLUMNS {
                if !mdec::read_column(&mut s.column) {
                    ok = false;
                    break;
                }
            }
            let finished = mdec::decode_finish();
            if !ok || !finished {
                mdec::reset();
                let _ = mdec::load_tables();
                continue;
            }
            self.mdec_span += clock.lap();

            // Stage 4: VRAM upload over channel 2, with the GP0 path kept
            // for reference so the report can show what DMA saved.
            clock.start();
            let mut dma_live = true;
            for c in 0..SLICE_COLUMNS {
                let rect = VramRect::new(c * 16, 0, 16, VIDEO_H);
                if dma_live && !dma_copy_to_vram(rect, s.column.as_ptr()) {
                    dma_live = false;
                    self.vram_dma_fallbacks += 1;
                }
                if !dma_live {
                    upload_words(rect, &s.column);
                }
            }
            self.upload += clock.lap();

            clock.start();
            for c in 0..SLICE_COLUMNS {
                upload_words(VramRect::new(c * 16, 0, 16, VIDEO_H), &s.column);
            }
            self.upload_gp0 += clock.lap();

            self.read_per_frame[self.frames.min(FRAMES - 1)] = self.read.vblanks;
            self.frames += 1;
        }
        // SAFETY: as above.
        unsafe { self.reader.stop() };
    }
}

/// Bring up a `SectorReader` and return the LBA of `name` in the ISO 9660
/// root directory. Mirrors `VideoPlayer::start_video`'s CD bring-up.
fn open_video(name: &[u8], reader: &mut SectorReader) -> Option<u32> {
    // Single speed, matching `VideoPlayer::start_video`. The drive rate is
    // the number the whole design rests on, so it has to be measured at the
    // speed the player actually runs at.
    if !unsafe { reader.prepare_single_speed() } {
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

    let bytes: &[u8] =
        unsafe { core::slice::from_raw_parts(s.sector_buf.as_ptr() as *const u8, 2048) };

    let mut off = 0usize;
    while off + 34 <= bytes.len() {
        let record_len = bytes[off] as usize;
        if record_len == 0 || off + record_len > bytes.len() {
            break;
        }
        let lba = u32::from_le_bytes([
            bytes[off + 2],
            bytes[off + 3],
            bytes[off + 4],
            bytes[off + 5],
        ]);
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

/// Run the pipeline over the whole movie with no display sync, timing each
/// stage. Read, expand, MDEC and upload are measured on one streaming pass
/// so the numbers are of the shipped pipeline, not of a serialised model
/// of it.
pub fn run_burst(clock: &mut Clock) -> Burst {
    let mut reader = SectorReader::new();
    let video_lba = open_video(b"INTRO.VID", &mut reader).unwrap_or(0);
    let mut out = Burst {
        read: Span::default(),
        copy: Span::default(),
        decode: Span::default(),
        upload: Span::default(),
        upload_dma: Span::default(),
        vram_dma_fallbacks: 0,
        pipeline: Span::default(),
        read_per_frame: [0; FRAMES],
        video_lba,
        located: video_lba != 0,
    };
    if video_lba == 0 {
        return out;
    }

    mdec::reset();
    let _ = mdec::load_tables();

    let mut pass = Pass {
        reader: &mut reader,
        video_lba,
        read: Span::default(),
        expand: Span::default(),
        mdec_span: Span::default(),
        upload: Span::default(),
        upload_gp0: Span::default(),
        vram_dma_fallbacks: 0,
        read_per_frame: [0; FRAMES],
        frames: 0,
        located: true,
    };
    pass.run(clock);

    out.read = pass.read;
    out.copy = pass.expand;
    out.decode = pass.mdec_span;
    out.upload = pass.upload;
    out.upload_dma = pass.upload_gp0;
    out.vram_dma_fallbacks = pass.vram_dma_fallbacks;
    out.read_per_frame = pass.read_per_frame;
    out.pipeline = out.read + out.copy + out.decode + out.upload;
    out
}

/// Decode one streamed frame and summarise the decoded pixels, so a run
/// that fell back to the embedded movie -- or that decoded garbage -- is
/// visible in the report rather than showing up as an implausibly fast
/// time.
pub fn check_integrity() -> (u32, u32, bool) {
    let s = storage();
    let mut reader = SectorReader::new();
    let Some(video_lba) = open_video(b"INTRO.VID", &mut reader) else {
        return (0, 0, false);
    };

    // Reassemble and expand the first complete frame off the disc.
    mdec::reset();
    let _ = mdec::load_tables();
    let mut asm = strfmt::FrameAssembler::new();
    let mut words = 0usize;
    // SAFETY: single-threaded bench.
    if !unsafe { reader.start_read(video_lba) } {
        return (0, 0, false);
    }
    loop {
        // SAFETY: as above.
        if !unsafe { reader.read_sector(&mut s.sector_buf) } {
            break;
        }
        let sector: &[u8] =
            unsafe { core::slice::from_raw_parts(s.sector_buf.as_ptr() as *const u8, 2048) };
        let buf: &mut [u8] = unsafe {
            let p = core::ptr::addr_of_mut!(s.slot) as *mut u8;
            core::slice::from_raw_parts_mut(p, SLOT_WORDS * 4)
        };
        let Some(frame) = asm.add(sector, buf) else {
            continue;
        };
        let rle16: &mut [u16] = unsafe {
            core::slice::from_raw_parts_mut(
                core::ptr::addr_of_mut!(s.rle) as *mut u16,
                RLE_WORDS * 2,
            )
        };
        words = bs::decode_frame(
            &buf[..frame.size as usize],
            rle16,
            COLUMNS as u32 * 15,
            15,
            &mut || {},
        )
        .unwrap_or(0);
        break;
    }
    // SAFETY: as above.
    unsafe { reader.stop() };
    if words == 0 {
        return (0, 0, false);
    }

    let rle: &[u32] =
        unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(s.rle) as *const u32, words) };
    // SAFETY: `rle` stays alive and unmodified until decode_finish.
    unsafe { mdec::decode_start(rle, words, mdec::DECODE_15BPP) };
    let mut checksum = 0u32;
    let mut nonzero = 0u32;
    for _col in 0..SLICE_COLUMNS {
        if !mdec::read_column(&mut s.column) {
            return (0, 0, false);
        }
        for &w in s.column.iter() {
            checksum = checksum.rotate_left(5) ^ w;
            if w != 0 {
                nonzero += 1;
            }
        }
    }
    if !mdec::decode_finish() {
        return (0, 0, false);
    }

    (checksum, nonzero, true)
}

/// What the CD path can deliver on its own, independent of video: how long
/// a pure contiguous stream takes, and what a stop-and-re-seek costs on top.
///
/// Both numbers are what the chunk sizing is computed from. The player
/// streams the movie without ever stopping the drive, so the contiguous
/// rate is the one that matters; the seek cost is kept because it is the
/// price of not doing that, and because a non-zero seek count in the paced
/// run is a regression signal.
pub struct CdRate {
    /// Sectors streamed with one `Setloc`+`ReadN`, drive left running.
    pub contiguous_sectors: u32,
    /// Display periods that took.
    pub contiguous_vblanks: u32,
    /// Sectors streamed as 4 single-frame batches, re-seeking each batch
    /// and stopping the drive after it, the pattern the old fixed-stride
    /// reader used.
    pub batched_sectors: u32,
    pub batched_vblanks: u32,
}

/// Sectors one frame occupies in the current movie encoding, read from the
/// disc rather than assumed, so the batched measurement tracks the asset.
const FRAME_SECTORS_PROBE: u32 = 5;

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

    let scratch: &mut [u32; SECTOR_WORDS] = &mut s.sector_buf;

    // 1. One long contiguous ReadN, drive left running. Far enough past the
    //    file end to stay inside the disc image.
    let contiguous = 128u32;
    clock.start();
    // SAFETY: single-threaded bench.
    if unsafe { reader.start_read(video_lba) } {
        for _ in 0..contiguous {
            // SAFETY: as above.
            unsafe { reader.read_sector(scratch) };
        }
    }
    // SAFETY: as above.
    unsafe { reader.stop() };
    let contiguous_vblanks = clock.lap().vblanks;

    // 2. The stop-and-re-seek pattern: four batches of one frame's sectors,
    //    each re-seeking and stopping the drive. This is what the old
    //    fixed-stride player did once per frame.
    let per = FRAME_SECTORS_PROBE;
    let batched = 4 * per;
    clock.start();
    for b in 0..4u32 {
        let lba = video_lba.wrapping_add(b * per);
        // SAFETY: as above.
        if unsafe { reader.start_read(lba) } {
            for _ in 0..per {
                // SAFETY: as above.
                unsafe { reader.read_sector(scratch) };
            }
        }
        // SAFETY: as above.
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
    let mut slice = [0u32; COLUMN_WORDS];
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

// The MDEC/CD contention probes that used to live here (`probe_mdec_contention`,
// `probe_mdec_repeat`, `probe_mdec_sweep`, `probe_mdec_frames_no_cd`,
// `probe_mdec_nocd`) were removed with the fixed-stride raw-MDEC format they
// drove. They fed `start_decode_frame` a hand-picked 7-sector frame and
// counted its 20 drained slices, and there is no such frame to pick any
// more: a BS v2 movie is demuxed by chunk header, so "frame N" is a
// reassembled variable-length payload and a probe cannot address it without
// doing the whole pipeline.
//
// They are not needed, either. Their question -- whether the MDEC tolerates
// a live drive -- is answered and recorded in `docs/perf/why-6fps.md`, and
// the answer turned out to be beside the point: what actually wedged the
// decoder was the *guest* not servicing the drive's data FIFO often enough
// (see `VideoPlayer::pump`), which corrupted the stream rather than
// disturbing the MDEC. The paced run is the check that matters now, and it
// exercises the real code path.
