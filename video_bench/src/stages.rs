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
use psx_hw::mdec::status as mdec_status;
use psx_io::dma;
use psx_rt::tty;

pub const FRAMES: usize = TOTAL_FRAMES as usize;
/// Compressed frames pulled per CD command, matching `VideoPlayer`.
const PREFETCH_FRAMES: u16 = 8;
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

/// Distinct compressed frames, pre-loaded so a probe can vary the decoded
/// data while controlling the drive separately. 8 frames is 112 KiB of the
/// 844 KiB free.
pub const FRAME_BANK_SLOTS: usize = 8;
static mut FRAME_BANK: [[u32; WORDS_PER_FRAME]; FRAME_BANK_SLOTS] =
    [[0; WORDS_PER_FRAME]; FRAME_BANK_SLOTS];

/// Read `n` distinct frames into the bank. Leaves the drive stopped.
fn load_frame_bank(reader: &mut SectorReader, video_lba: u32, n: usize) -> bool {
    if !unsafe { reader.start_read(video_lba) } {
        return false;
    }
    let mut ok = true;
    'outer: for f in 0..n {
        for sector in 0..SECTORS_PER_FRAME {
            let offset = sector * SECTOR_WORDS;
            let bank = unsafe { &mut *(&raw mut FRAME_BANK) };
            let buf: &mut [u32; SECTOR_WORDS] = unsafe {
                &mut *(bank[f][offset..offset + SECTOR_WORDS].as_mut_ptr()
                    as *mut [u32; SECTOR_WORDS])
            };
            if !unsafe { reader.read_sector(buf) } {
                ok = false;
                break 'outer;
            }
        }
    }
    unsafe { reader.stop() };
    ok
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
        vram_dma_fallbacks: 0,
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

        // The shipped path: DMA channel 2, falling back to the GP0
        // command port for the rest of the run if the channel wedges.
        clock.start();
        let mut dma_live = true;
        for col in 0..SLICE_COLUMNS {
            let rect = VramRect::new(col * 16, 0, 16, VIDEO_H);
            if dma_live && !dma_copy_to_vram(rect, s.slice_words.as_ptr()) {
                dma_live = false;
                out.vram_dma_fallbacks += 1;
            }
            if !dma_live {
                upload_words(rect, &s.slice_words);
            }
        }
        out.upload += clock.lap();

        // For reference, what the same payload costs on the GP0 path
        // alone, so the report can show the win even when DMA is in use.
        clock.start();
        for col in 0..SLICE_COLUMNS {
            let rect = VramRect::new(col * 16, 0, 16, VIDEO_H);
            upload_words(rect, &s.slice_words);
        }
        out.upload_dma = clock.lap();
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

// ---------------------------------------------------------------------------
// MDEC / CD contention probe
// ---------------------------------------------------------------------------

/// CD-ROM controller ports, for reproducing `SectorReader`'s private ack
/// helpers from outside the crate.
const CD_STATUS: u32 = 0x1F80_1800;
const CD_PARAM: u32 = 0x1F80_1802;
const CD_IRQ: u32 = 0x1F80_1803;

#[inline]
fn cd_wr_index(i: u8) {
    unsafe { psx_io::write8(CD_STATUS, i & 0x03) };
}

/// `SectorReader::ack_all`, replicated: select the interrupt-enable
/// register, clear all five CD interrupt flags, and release the CPU-level
/// CDROM source.
fn ack_all_cd() {
    cd_wr_index(1);
    unsafe { psx_io::write8(CD_IRQ, 0x5F) };
    psx_io::irq::ack(1 << psx_io::irq::source::CDROM);
    cd_wr_index(0);
}

/// Zero the drive's interrupt-enable mask, leaving the ReadN running.
fn quiesce_cd() {
    cd_wr_index(1);
    unsafe { psx_io::write8(CD_PARAM, 0x00) };
    cd_wr_index(0);
}

/// How the drive is left while the MDEC decodes.
#[derive(Copy, Clone)]
pub enum ContentionCase {
    /// Nothing: a live ReadN, which is the condition that breaks the decode.
    Live,
    /// `pause_read` alone.
    Pause,
    /// `pause_read` then `ack_all`.
    PauseAck,
    /// The full `stop()`: pause, ack, and drop any deferred sector.
    Stop,
    /// Ack and quiesce the drive, but leave the ReadN running.
    AckQuiesce,
    /// Quiesce the drive only, ReadN still running.
    Quiesce,
}

impl ContentionCase {
    const ALL: [(ContentionCase, u8); 6] = [
        (ContentionCase::Live, 0),
        (ContentionCase::Pause, 1),
        (ContentionCase::PauseAck, 2),
        (ContentionCase::Stop, 3),
        (ContentionCase::AckQuiesce, 4),
        (ContentionCase::Quiesce, 5),
    ];

    const fn name(self) -> &'static str {
        match self {
            ContentionCase::Live => "live",
            ContentionCase::Pause => "pause",
            ContentionCase::PauseAck => "pause+ack",
            ContentionCase::Stop => "stop",
            ContentionCase::AckQuiesce => "ack+quiesce",
            ContentionCase::Quiesce => "quiesce",
        }
    }
}

/// Decode one real frame under each way of parking the drive, and report
/// how many of its 20 slices came out.
///
/// The frame is read once and reused: the MDEC does not modify its input,
/// so every case decodes identical data. The drive is re-armed before each
/// case so each one genuinely starts from a streaming state.
pub fn probe_mdec_contention() {
    let s = storage();
    let mut reader = SectorReader::new();
    let Some(video_lba) = open_video(b"INTRO.VID", &mut reader) else {
        tty::println("@@VB1 CONTENTION skipped (no video)");
        return;
    };
    if !unsafe { reader.prepare() } {
        tty::println("@@VB1 CONTENTION skipped (no CD)");
        return;
    }

    // Pull frame 0 into RAM while the drive streams, so the buffer holds
    // real MDEC data and the drive is left running.
    if !unsafe { reader.start_read(video_lba) } {
        tty::println("@@VB1 CONTENTION skipped (no ReadN)");
        return;
    }
    for sector in 0..SECTORS_PER_FRAME {
        let offset = sector * SECTOR_WORDS;
        let buf: &mut [u32; SECTOR_WORDS] = unsafe {
            &mut *(s.frame_words[offset..offset + SECTOR_WORDS].as_mut_ptr()
                as *mut [u32; SECTOR_WORDS])
        };
        unsafe { reader.read_sector(buf) };
    }
    let data_words = (s.frame_words[0] & 0xFFFF) as usize;
    if data_words == 0 || data_words >= WORDS_PER_FRAME {
        tty::println("@@VB1 CONTENTION bad frame cmd");
        unsafe { reader.stop() };
        return;
    }

    tty::print("@@VB1 contention frame_cmd=");
    tty::print_hex_u32(s.frame_words[0]);
    tty::print(" data_words=");
    tty::print_hex_u32(data_words as u32);
    tty::print("\n");

    for (case, id) in ContentionCase::ALL {
        // Re-arm a live stream from the *next* frame, so the drive is
        // genuinely delivering into the FIFO during the decode.
        let lba = video_lba.wrapping_add(SECTORS_PER_FRAME as u32);
        let streaming = unsafe { reader.start_read(lba) };
        // Take one sector so the FIFO is non-empty, exactly as it would be
        // mid-playback.
        if streaming {
            let offset = SECTOR_WORDS;
            let buf: &mut [u32; SECTOR_WORDS] = unsafe {
                &mut *(s.frame_cache[0][offset..offset + SECTOR_WORDS].as_mut_ptr()
                    as *mut [u32; SECTOR_WORDS])
            };
            unsafe { reader.read_sector(buf) };
        }

        match case {
            ContentionCase::Live => {}
            ContentionCase::Pause => {
                unsafe { reader.pause_read() };
            }
            ContentionCase::PauseAck => {
                unsafe { reader.pause_read() };
                ack_all_cd();
            }
            ContentionCase::Stop => {
                unsafe { reader.stop() };
            }
            ContentionCase::AckQuiesce => {
                ack_all_cd();
                quiesce_cd();
            }
            ContentionCase::Quiesce => {
                quiesce_cd();
            }
        }

        mdec::init();
        mdec::init();
        mdec::start_decode_frame(&s.frame_words);
        let mut slices = 0u32;
        for _col in 0..SLICE_COLUMNS {
            if mdec::drain_slice_dma(&mut s.slice_words) {
                slices += 1;
            } else {
                break;
            }
        }
        let stat = mdec::read_stat();
        let in_busy = dma::is_busy(dma::Channel::MdecIn) as u32;

        tty::print("@@VB1 contention id=");
        tty::print_hex_u32(id as u32);
        tty::print(" case=");
        tty::print(case.name());
        tty::print(" slices=");
        tty::print_hex_u32(slices);
        tty::print("/");
        tty::print_hex_u32(SLICE_COLUMNS as u32);
        tty::print(" stat=");
        tty::print_hex_u32(stat);
        tty::print(" inbusy=");
        tty::print_hex_u32(in_busy);
        tty::print("\n");

        // Leave the drive quiet before re-arming for the next case.
        unsafe { reader.stop() };
    }

    tty::println("@@VB1 contention done");
}

/// Repeat "read one frame from a live stream, then decode it" and report
/// the slice count for each repetition.
///
/// A single decode against a live `ReadN` succeeds for every way of
/// parking the drive, so the failure is not per-decode: it accumulates.
/// This walks the repetition count until the decode stops producing and
/// prints the iteration it died on, which is the first hard number for
/// what the accumulation actually is.
pub fn probe_mdec_repeat(iterations: u32) {
    let s = storage();
    let mut reader = SectorReader::new();
    let Some(video_lba) = open_video(b"INTRO.VID", &mut reader) else {
        tty::println("@@VB1 REPEAT skipped (no video)");
        return;
    };
    if !unsafe { reader.prepare() } {
        tty::println("@@VB1 REPEAT skipped (no CD)");
        return;
    }

    tty::println("@@VB1 REPEAT begin");

    // Start at frame 0 and never stop the stream, so every iteration is
    // read-then-decode against a live drive, exactly as playback does.
    if !unsafe { reader.start_read(video_lba) } {
        tty::println("@@VB1 REPEAT skipped (no ReadN)");
        return;
    }

    for iter in 0..iterations {
        // Read the next frame's sectors into slot (iter % PREFETCH_FRAMES).
        let slot = (iter as usize) % PREFETCH_FRAMES as usize;
        let mut read_ok = true;
        for sector in 0..SECTORS_PER_FRAME {
            let offset = sector * SECTOR_WORDS;
            let buf: &mut [u32; SECTOR_WORDS] = unsafe {
                &mut *(s.frame_cache[slot][offset..offset + SECTOR_WORDS].as_mut_ptr()
                    as *mut [u32; SECTOR_WORDS])
            };
            if !unsafe { reader.read_sector(buf) } {
                read_ok = false;
                break;
            }
        }
        if !read_ok {
            tty::print("@@VB1 REPEAT iter=");
            tty::print_hex_u32(iter);
            tty::print(" readfail=1 slices=0/");
            tty::print_hex_u32(SLICE_COLUMNS as u32);
            tty::print("\n");
            break;
        }
        s.frame_words.copy_from_slice(&s.frame_cache[slot]);

        mdec::start_decode_frame(&s.frame_words);
        let mut slices = 0u32;
        for _ in 0..SLICE_COLUMNS {
            if mdec::drain_slice_dma(&mut s.slice_words) {
                slices += 1;
            } else {
                break;
            }
        }
        // 20 slices is the display geometry, not the frame. Keep draining
        // while the MDEC still has output pending, so the frame actually
        // ends instead of being abandoned mid-frame.
        let mut extra = 0u32;
        while mdec::read_stat() & mdec_status::DATA_OUT_REQ != 0 && extra < 8 {
            if !mdec::drain_slice_dma(&mut s.slice_words) {
                break;
            }
            extra += 1;
        }
        let mut settle = 0u32;
        while mdec::is_busy() && settle < 2_000_000 {
            settle += 1;
        }
        let busy_after_settle = mdec::is_busy();

        tty::print("@@VB1 REPEAT iter=");
        tty::print_hex_u32(iter);
        tty::print(" extra=");
        tty::print_hex_u32(extra);
        tty::print(" settle=");
        tty::print_hex_u32(settle);
        tty::print(" busyafter=");
        tty::print_hex_u32(busy_after_settle as u32);
        tty::print(" slices=");
        tty::print_hex_u32(slices);
        tty::print("/");
        tty::print_hex_u32(SLICE_COLUMNS as u32);
        tty::print(" stat=");
        tty::print_hex_u32(mdec::read_stat());
        tty::print(" inbusy=");
        tty::print_hex_u32(dma::is_busy(dma::Channel::MdecIn) as u32);
        tty::print("\n");

        if slices < SLICE_COLUMNS as u32 {
            tty::print("@@VB1 REPEAT died_at=");
            tty::print_hex_u32(iter);
            tty::print("\n");
            break;
        }
    }

    unsafe { reader.stop() };
    tty::println("@@VB1 REPEAT done");
}

/// Decode the same frame repeatedly with the drive never touched.
///
/// This separates "the MDEC cannot decode back-to-back frames" from "the
/// CD causes it". If this wedges too, the CD is irrelevant to the
/// accumulation and the old code only survived because its long read
/// Sweep how much CD activity can precede a decode before the MDEC wedges.
///
/// `probe_mdec_repeat` gave a binary answer: seven sectors of live CD reads
/// before each decode, and iteration 1 never came back. That is enough to
/// know overlap is currently unsafe, but not enough to design around it --
/// chunked overlap only needs to know where the threshold sits. If a small
/// number of sectors survives repeated iterations then a read/decode
/// interleave can go that deep and no further.
///
/// Each step k runs its own continuous stream: prime one frame, then per
/// iteration read k sectors into scratch and decode the primed frame. Only
/// the CD traffic varies; the decode is identical every time, and the frame
/// is deliberately held constant so that a failure means "the drive was
/// busy", never "the data was wrong".
pub fn probe_mdec_sweep(iterations: u32) {
    const SWEEP_K: [u32; 6] = [0, 1, 2, 4, 7, 14];

    let s = storage();
    let mut reader = SectorReader::new();
    let Some(video_lba) = open_video(b"INTRO.VID", &mut reader) else {
        tty::println("@@VB1 SWEEP skipped (no video)");
        return;
    };
    if !unsafe { reader.prepare() } {
        tty::println("@@VB1 SWEEP skipped (no CD)");
        return;
    }
    let n = core::cmp::min(iterations as usize, FRAME_BANK_SLOTS);
    if n == 0 {
        return;
    }
    // Distinct frames, pre-loaded, so every iteration decodes different data.
    // This is the part the first version of this probe got wrong: holding
    // the frame constant also held the decode duration constant, which is
    // what decides whether the drive's FIFO has time to overflow. With the
    // frame fixed, every k "passed" for the uninteresting reason that the
    // failing condition was never produced.
    if !load_frame_bank(&mut reader, video_lba, n) {
        tty::println("@@VB1 SWEEP readfail");
        return;
    }

    tty::print("@@VB1 SWEEP begin iters=");
    tty::print_hex_u32(n as u32);
    tty::print("\n");

    for &k in SWEEP_K.iter() {
        // Fresh stream per step so each run starts from the same place.
        if !unsafe { reader.start_read(video_lba) } {
            tty::println("@@VB1 SWEEP k=? readfail");
            continue;
        }
        let mut ok_iters = 0u32;
        let mut first_bad = 0xFFFF_FFFFu32;
        let mut last_stat = 0u32;
        let mut worst_settle = 0u32;

        for iter in 0..n {
            let iter = iter as u32;
            // This much extra CD traffic while the decode will have to run.
            let mut read_ok = true;
            for _ in 0..k {
                let buf: &mut [u32; SECTOR_WORDS] = unsafe {
                    &mut *(s.frame_cache[2][0..SECTOR_WORDS].as_mut_ptr() as *mut [u32; SECTOR_WORDS])
                };
                if !unsafe { reader.read_sector(buf) } {
                    read_ok = false;
                    break;
                }
            }
            if !read_ok {
                first_bad = iter;
                break;
            }
            unsafe {
                let bank = &*(&raw const FRAME_BANK);
                s.frame_words.copy_from_slice(&bank[iter as usize]);
            }

            mdec::start_decode_frame(&s.frame_words);
            let mut slices = 0u32;
            for _ in 0..SLICE_COLUMNS {
                if mdec::drain_slice_dma(&mut s.slice_words) {
                    slices += 1;
                } else {
                    break;
                }
            }
            let mut extra = 0u32;
            while mdec::read_stat() & mdec_status::DATA_OUT_REQ != 0 && extra < 8 {
                if !mdec::drain_slice_dma(&mut s.slice_words) {
                    break;
                }
                extra += 1;
            }
            let mut settle = 0u32;
            while mdec::is_busy() && settle < 2_000_000 {
                settle += 1;
            }
            let busy_after = mdec::is_busy();
            last_stat = mdec::read_stat();
            if settle > worst_settle {
                worst_settle = settle;
            }
            if slices != SLICE_COLUMNS as u32 || busy_after {
                first_bad = iter;
                break;
            }
            ok_iters += 1;
        }
        unsafe { reader.stop() };

        tty::print("@@VB1 SWEEP k=");
        tty::print_hex_u32(k);
        tty::print(" ok=");
        tty::print_hex_u32(ok_iters);
        tty::print(" firstbad=");
        tty::print_hex_u32(first_bad);
        tty::print(" settlespc=");
        tty::print_hex_u32(worst_settle);
        tty::print(" stat=");
        tty::print_hex_u32(last_stat);
        tty::print("\n");
    }

    tty::println("@@VB1 SWEEP end");
}

/// Decode several *different* frames with the drive stopped.
///
/// This is the control the earlier probes left open. `probe_mdec_nocd`
/// decodes one frame repeatedly and `probe_mdec_repeat` decodes varying
/// frames against a live drive; neither varies the data *and* isolates the
/// drive. Until this runs, "sustained CD activity wedges the MDEC" and "some
/// frames wedge the MDEC" are equally consistent with everything measured so
/// far -- and the sweep says live CD traffic up to 14 sectors before a decode
/// is harmless, which points hard at the data.
///
/// Frames are read up front and the drive is stopped before the first decode,
/// so nothing about the drive changes between iterations.
pub fn probe_mdec_frames_no_cd(frames: u32) {
    let s = storage();
    let mut reader = SectorReader::new();
    let Some(video_lba) = open_video(b"INTRO.VID", &mut reader) else {
        tty::println("@@VB1 FRAMENOCD skipped (no video)");
        return;
    };
    if !unsafe { reader.prepare() } {
        tty::println("@@VB1 FRAMENOCD skipped (no CD)");
        return;
    }
    let n = core::cmp::min(frames as usize, FRAME_BANK_SLOTS);
    if n == 0 {
        return;
    }

    if !load_frame_bank(&mut reader, video_lba, n) {
        tty::println("@@VB1 FRAMENOCD readfail");
        return;
    }

    tty::print("@@VB1 FRAMENOCD begin frames=");
    tty::print_hex_u32(n as u32);
    tty::print("\n");

    for f in 0..n {
        unsafe {
            let bank = &*(&raw const FRAME_BANK);
            s.frame_words.copy_from_slice(&bank[f]);
        }
        mdec::start_decode_frame(&s.frame_words);
        let mut slices = 0u32;
        for _ in 0..SLICE_COLUMNS {
            if mdec::drain_slice_dma(&mut s.slice_words) {
                slices += 1;
            } else {
                break;
            }
        }
        let mut extra = 0u32;
        while mdec::read_stat() & mdec_status::DATA_OUT_REQ != 0 && extra < 8 {
            if !mdec::drain_slice_dma(&mut s.slice_words) {
                break;
            }
            extra += 1;
        }
        let mut settle = 0u32;
        while mdec::is_busy() && settle < 2_000_000 {
            settle += 1;
        }
        let busy_after = mdec::is_busy();
        tty::print("@@VB1 FRAMENOCD frame=");
        tty::print_hex_u32(f as u32);
        tty::print(" slices=");
        tty::print_hex_u32(slices);
        tty::print("/");
        tty::print_hex_u32(SLICE_COLUMNS as u32);
        tty::print(" extra=");
        tty::print_hex_u32(extra);
        tty::print(" settle=");
        tty::print_hex_u32(settle);
        tty::print(" busyafter=");
        tty::print_hex_u32(busy_after as u32);
        tty::print(" stat=");
        tty::print_hex_u32(mdec::read_stat());
        tty::print("\n");
    }
    tty::println("@@VB1 FRAMENOCD done");
}

/// burst left the MDEC time to settle between frames.
pub fn probe_mdec_nocd(iterations: u32) {
    let s = storage();
    let mut reader = SectorReader::new();
    let Some(video_lba) = open_video(b"INTRO.VID", &mut reader) else {
        tty::println("@@VB1 NOCD skipped");
        return;
    };
    if !unsafe { reader.prepare() } {
        tty::println("@@VB1 NOCD skipped");
        return;
    }
    // Read one frame, then leave the drive stopped for the whole probe.
    if !unsafe { reader.start_read(video_lba) } {
        tty::println("@@VB1 NOCD skipped");
        return;
    }
    for sector in 0..SECTORS_PER_FRAME {
        let offset = sector * SECTOR_WORDS;
        let buf: &mut [u32; SECTOR_WORDS] = unsafe {
            &mut *(s.frame_words[offset..offset + SECTOR_WORDS].as_mut_ptr()
                as *mut [u32; SECTOR_WORDS])
        };
        unsafe { reader.read_sector(buf) };
    }
    unsafe { reader.stop() };

    tty::println("@@VB1 NOCD begin");
    for iter in 0..iterations {
        mdec::init();
        mdec::start_decode_frame(&s.frame_words);
        let mut slices = 0u32;
        for _ in 0..SLICE_COLUMNS {
            if mdec::drain_slice_dma(&mut s.slice_words) {
                slices += 1;
            } else {
                break;
            }
        }
        let mut settle = 0u32;
        while mdec::is_busy() && settle < 2_000_000 {
            settle += 1;
        }
        tty::print("@@VB1 NOCD iter=");
        tty::print_hex_u32(iter);
        tty::print(" slices=");
        tty::print_hex_u32(slices);
        tty::print("/");
        tty::print_hex_u32(SLICE_COLUMNS as u32);
        tty::print(" settle=");
        tty::print_hex_u32(settle);
        tty::print(" busyafter=");
        tty::print_hex_u32(mdec::is_busy() as u32);
        tty::print(" stat=");
        tty::print_hex_u32(mdec::read_stat());
        tty::print("\n");
        if slices < SLICE_COLUMNS as u32 {
            tty::print("@@VB1 NOCD died_at=");
            tty::print_hex_u32(iter);
            tty::print("\n");
            break;
        }
    }
    tty::println("@@VB1 NOCD done");
}
