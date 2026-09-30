//! Cinematic Intro Video playback for Plattypus PSX.
//!
//! Decodes 320x240 video at 15 fps using the PS1 hardware MDEC coprocessor
//! (MDEC0 / DMA channel 0 & channel 1) with synchronized SPU ADPCM audio.
//! Streams the disc copy through a 32-frame read-ahead ring in 16-frame
//! chunks, draining sectors into the ring between MDEC slices so the CD
//! read overlaps the decode, with seamless fallback to embedded ROM frames.
//! Playback is skippable at any time via CROSS or START.

use psx_pad::{button, ButtonState, PadState};
use psx_vram::{VramRect, dma_copy_to_vram, upload_words};
use psx_pack::cd::{SectorReader, SECTOR_WORDS};
use crate::audio::AudioManager;

/// Embedded fallback video (320x240, 16 frames, 15 fps, raw MDEC hardware bitstream)
static EMBEDDED_VIDEO: &[u8] = include_bytes!("../video_mdec.bin");

pub const VIDEO_W: u16 = 320;
pub const VIDEO_H: u16 = 240;
pub const TOTAL_FRAMES: u16 = 150;
pub const EMBEDDED_FRAME_COUNT: u16 = 16;
/// CD sectors per compressed frame, and so the stride into the .VID file.
///
/// This is the bandwidth dial. The drive has to deliver this many sectors
/// inside one 4-display-period frame window while the MDEC holds the DMA
/// controller, so it decides whether 15 fps is reachable. Reduced from the
/// original 8 sectors after measuring that the worst frame in the video
/// needs only 7; see `docs/perf/why-6fps.md`.
pub const SECTORS_PER_FRAME: usize = 7;
pub const WORDS_PER_FRAME: usize = 3584; // 14,336 bytes = 7 sectors
pub const WORDS_PER_SLICE: usize = 1920; // 16 * 240 / 2 = 1,920 u32 words
/// Display periods each video frame is held for. Four gives 15 fps on a
/// 60 Hz display, which is the rate the assets are encoded at and the
/// target. The pipeline only fits it because the CD read now overlaps the
/// MDEC decode; see the chunk sizing in `docs/perf/why-6fps.md`.
pub const VBLANKS_PER_VIDEO_FRAME: u32 = 4;

/// Frames read per `ReadN` before the drive is stopped and re-seeked.
///
/// A stop plus re-seek costs a measured 4.02 display periods, against a
/// 3.23-period contiguous fetch of one 7-sector frame. Fifteen fps gives
/// four display periods per frame, so the seek has to be amortised twice
/// over: it is a *contiguous* 4.02-period block of blocked CPU, and the
/// per-frame slack is only `4 - 1.69 (decode) = 2.31` periods, so a seek
/// that straddles a present costs that present a whole extra period. Every
/// seek is therefore a chance to drop a frame, and the only lever is to
/// seek less often. The totals leave plenty of room -- 16-frame chunks put
/// the whole cut at 503 of its 600 available display periods -- so the
/// chunk is sized by the RAM the ring costs, not by the drive budget.
const CHUNK_FRAMES: usize = 16;

/// Read-ahead ring depth, in compressed frames.
///
/// Two chunks, so a chunk can be streamed in while the previous one is
/// being decoded: the stream fills the back half as the decode drains the
/// front. This is also what sets how long the drive can run between seeks.
/// A new chunk starts when the ring has room for one whole, i.e. once the
/// decode has drained `CHUNK_FRAMES` frames, so the seek interval is the
/// chunk size and the ring only has to be deep enough to hold the chunk
/// being filled plus the one being decoded.
const RING_FRAMES: usize = 32;

/// Frames to read before the first present.
///
/// Deliberately less than a chunk: filling the whole ring up front would be
/// ~1.7 s of black screen before the intro's first frame, and the scheduler
/// refills the rest during playback out of the 116 display periods of slack
/// the video has. One chunk's worth costs ~0.6 s.
const PRIME_FRAMES: usize = 8;

/// How many times a failed CD prefetch is re-attempted from a fresh seek before
/// the player gives up on streaming. A single transient read error is common on
/// real hardware, and giving up on the first one truncated a 150-frame cutscene
/// to a 16-frame loop for the rest of the session.
const CD_READ_RETRIES: u8 = 3;

/// Spin budget for the whole PIO decode fallback of one slice. The old
/// per-word budget of 10,000 across 1,920 words was several seconds of frozen
/// screen inside a single frame, so the budget is per slice rather than per
/// word.
const PIO_SPIN_BUDGET: u32 = 200_000;

// The frame geometry is only correct for assets encoded at this GOP size.
// `SECTORS_PER_FRAME` doubles as the LBA stride into the .VID file, so
// re-encoding at a different size silently desynced sector addressing. The
// pure-arithmetic invariants are checked at compile time; the embedded
// fallback's size is checked at boot instead, because `include_bytes!` yields
// a reference whose length is not const-evaluable here.
const _: () = assert!(SECTORS_PER_FRAME * 2048 == WORDS_PER_FRAME * 4);
const _: () = assert!(EMBEDDED_FRAME_COUNT <= TOTAL_FRAMES);
// The ring is a FIFO addressed by `frame % RING_FRAMES`, so a chunk has to
// fit in it whole or a stream would overwrite frames still waiting to be
// decoded, and it has to be deeper than one chunk for the read to overlap
// the decode at all. Keeping the ring a whole number of chunks is what makes
// the ping-pong between them land back on a chunk boundary.
const _: () = assert!(RING_FRAMES >= 2 * CHUNK_FRAMES);
const _: () = assert!(RING_FRAMES % CHUNK_FRAMES == 0);

/// BSS-resident storage for MDEC bitstream, slice buffer, and CD reading
struct VideoStorage {
    /// One frame payload, sized for the largest frame in the video.
    frame_words: [u32; WORDS_PER_FRAME],
    /// Read-ahead ring. Slot `i` holds frame `i % RING_FRAMES`, so the ring
    /// is a FIFO and the decode drains it from the front while the drive
    /// fills it from the back.
    frame_cache: [[u32; WORDS_PER_FRAME]; RING_FRAMES],
    /// 1,920 u32 words (7.68 KiB) holding one decoded 16x240 slice
    slice_words: [u32; WORDS_PER_SLICE],
    /// Bounce buffer for single-sector reads (e.g. root directory scan)
    sector_buf: [u32; SECTOR_WORDS],
}

impl VideoStorage {
    const fn new() -> Self {
        Self {
            frame_words: [0; WORDS_PER_FRAME],
            frame_cache: [[0; WORDS_PER_FRAME]; RING_FRAMES],
            slice_words: [0; WORDS_PER_SLICE],
            sector_buf: [0; SECTOR_WORDS],
        }
    }
}

static mut STORAGE: VideoStorage = VideoStorage::new();

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum VideoKind {
    Intro,
    Outro,
}

pub struct VideoPlayer {
    pub kind: VideoKind,
    pub frame_idx: u16,
    pub tick: u8,
    pub ticks_per_frame: u8,
    pub total_frames: u16,
    pub width: u16,
    pub height: u16,
    pub finished: bool,
    pub using_cd: bool,
    pub cd_start_lba: u32,
    last_decoded_frame: u16,
    /// Display-period count at the last `swap`.
    pub last_swap_vblank: u32,
    /// A decoded frame is complete in the back buffer, waiting to be shown.
    frame_unpresented: bool,
    saved_irq_mask: u32,
    /// Whether `saved_irq_mask` holds a mask that still needs restoring.
    irq_mask_saved: bool,
    /// Failed chunk reads since the last successful one.
    cd_read_retries: u8,
    /// Oldest frame held in the ring. The ring is the contiguous frame run
    /// `[ring_start_frame, ring_start_frame + ring_frames)`.
    ring_start_frame: u16,
    ring_frames: u16,
    /// Next frame the running `ReadN` will deliver. Equal to the ring's tail
    /// (`ring_start_frame + ring_frames`) whenever no stream is running.
    stream_frame: u16,
    /// A `ReadN` is live and `stream_frame` is walking towards the end of
    /// its chunk.
    stream_active: bool,
    /// Sector within `stream_frame` that the stream will deliver next.
    stream_sector: u8,
    /// Frame at which the running stream stops, one past its last frame.
    chunk_end_frame: u16,
    cd_reader: SectorReader,
    /// Whether VRAM uploads may still go over DMA channel 2.
    vram_dma_ok: bool,
    /// Times the channel wedged and the GP0 path took over.
    pub vram_dma_fallbacks: u32,
    /// Chunks started, i.e. stop-and-re-seek cycles. Each one is a
    /// contiguous ~4-display-period block of blocked CPU, so this is the
    /// count of chances to push a present a period late.
    pub chunk_starts: u32,
    /// Sectors moved into the ring by `pump_ready_sectors`, i.e. read work
    /// that happened *underneath* a decode rather than in front of one.
    pub overlapped_sectors: u32,
}

impl VideoPlayer {
    pub fn new() -> Self {
        Self {
            kind: VideoKind::Intro,
            frame_idx: 0,
            tick: 0,
            ticks_per_frame: 4, // 60 Hz / 4 = 15 fps
            total_frames: TOTAL_FRAMES,
            width: VIDEO_W,
            height: VIDEO_H,
            finished: false,
            using_cd: false,
            cd_start_lba: 0,
            last_decoded_frame: 0xFFFF,
            last_swap_vblank: psx_rt::interrupts::vblank_count(),
            frame_unpresented: false,
            saved_irq_mask: 0,
            irq_mask_saved: false,
            cd_read_retries: 0,
            ring_start_frame: 0,
            ring_frames: 0,
            stream_frame: 0,
            stream_active: false,
            stream_sector: 0,
            chunk_end_frame: 0,
            cd_reader: SectorReader::new(),
            vram_dma_ok: true,
            vram_dma_fallbacks: 0,
            chunk_starts: 0,
            overlapped_sectors: 0,
        }
    }

    /// Locate specified video file on disc by scanning the ISO 9660 root directory (Sector 20).
    unsafe fn find_vid_lba(filename: &[u8], reader: &mut SectorReader, storage: &mut VideoStorage) -> Option<u32> {
        psx_rt::tty::println("[VIDEO] Scanning root directory (Sector 20)...");
        // Sector 20 is the ISO 9660 root directory extent
        // Use the reader's implicit SetLoc+ReadN path here. The explicit BIOS
        // SeekL bracket fails on some drives/emulators during the first read,
        // which silently forced playback to the 16-frame embedded loop.
        if !unsafe { reader.start_read(20) } {
            psx_rt::tty::println("[VIDEO] start_read(20) FAILED");
            return None;
        }
        let read_ok = unsafe { reader.read_sector(&mut storage.sector_buf) };
        unsafe { reader.stop(); }
        if !read_ok {
            psx_rt::tty::println("[VIDEO] read_sector(20) FAILED");
            return None;
        }

        let bytes: &[u8] = unsafe {
            core::slice::from_raw_parts(storage.sector_buf.as_ptr() as *const u8, 2048)
        };

        let mut off = 0usize;
        while off < bytes.len() {
            let record_len = bytes[off] as usize;
            if record_len == 0 {
                break;
            }
            // A directory record must be at least 33 bytes to carry the LBA at
            // offset 2..6 and the name length at offset 32. A short record
            // used to pass the `off + record_len` check and then read the LBA
            // past the end of the sector.
            if record_len < 33 || off + record_len > bytes.len() {
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
                let name = &bytes[off + 33..off + 33 + name_len];
                // ISO 9660 identifiers carry a `;1` version suffix, so
                // `INTRO.VID` is stored as `INTRO.VID;1`. A bare prefix match
                // also matched a hypothetical `INTRO.VIDX`; require the
                // version separator or the end of the identifier.
                if name.starts_with(filename)
                    && (name.len() == filename.len() || name[filename.len()] == b';')
                {
                    psx_rt::tty::print("[VIDEO] Found video file at LBA: ");
                    psx_rt::tty::print_hex_u32(lba);
                    psx_rt::tty::print("\n");
                    return Some(lba);
                }
            }
            off += record_len;
        }
        psx_rt::tty::println("[VIDEO] Video file not found in Sector 20");
        None
    }

    /// Ring slot holding `frame`. Valid only while the ring holds it.
    #[inline]
    fn ring_slot(frame: u16) -> usize {
        (frame as usize) % RING_FRAMES
    }

    /// True when `frame` is already sitting in the read-ahead ring.
    #[inline]
    fn ring_has(&self, frame: u16) -> bool {
        self.using_cd
            && frame >= self.ring_start_frame
            && frame < self.ring_start_frame + self.ring_frames
    }

    /// Frames the ring can still accept before a slot would be overwritten.
    #[inline]
    fn ring_free(&self) -> u16 {
        RING_FRAMES as u16 - self.ring_frames
    }

    /// Drop the whole ring and make sure no `ReadN` is left running.
    ///
    /// The partially-read frame at the stream's cursor is discarded rather
    /// than kept: its sectors are gone from the drive, so the only way back
    /// is a fresh seek to the frame's first sector.
    fn reset_stream(&mut self) {
        if self.stream_active {
            unsafe { self.cd_reader.stop() };
            self.stream_active = false;
        }
        self.ring_frames = 0;
    }

    /// Start a `ReadN` for the next chunk, if the ring has room for it whole.
    ///
    /// The chunk is deliberately *not* aligned to a multiple of
    /// `CHUNK_FRAMES`: it starts wherever the ring's tail currently is. A
    /// fixed grid would force a seek wherever playback and the grid drift
    /// apart, which is the per-frame seek this design exists to avoid.
    fn start_next_chunk(&mut self) -> bool {
        if !self.using_cd || self.stream_active {
            return false;
        }
        let tail = self.ring_start_frame + self.ring_frames;
        if tail >= self.total_frames {
            return false;
        }
        // A partial tail chunk is fine; a partial *ring* is not, because the
        // stream would overwrite frames the decode has not reached.
        let chunk = core::cmp::min(CHUNK_FRAMES as u16, self.total_frames - tail);
        if chunk > self.ring_free() {
            return false;
        }

        let lba = self.cd_start_lba
            .wrapping_add(tail as u32 * SECTORS_PER_FRAME as u32);
        if !unsafe { self.cd_reader.start_read(lba) } {
            psx_rt::tty::println("[VIDEO] CD chunk ReadN start failed");
            return false;
        }
        self.stream_frame = tail;
        self.stream_sector = 0;
        self.chunk_end_frame = tail + chunk;
        self.stream_active = true;
        self.chunk_starts += 1;
        true
    }

    /// Pop every sector the drive has ready, without ever waiting for one.
    ///
    /// This is the half of the read that runs *during* a decode. The MDEC is
    /// driven by DMA, so the CPU is otherwise idle inside the 20-slice loop
    /// and the time it spends here is time the decode would have spent
    /// spinning. Checking after every slice keeps the drive's data FIFO
    /// drained -- a sector arrives every ~7.7 ms and a slice completes every
    /// ~1.3 ms, so the FIFO never holds more than the one sector in flight.
    ///
    /// Returns the number of sectors moved into the ring.
    fn pump_ready_sectors(&mut self, storage: &mut VideoStorage) -> u16 {
        let mut moved = 0u16;
        while self.stream_active && self.stream_frame < self.chunk_end_frame {
            if !unsafe { self.cd_reader.sector_pending() } {
                break;
            }
            if !self.pop_one_sector(storage) {
                break;
            }
            moved += 1;
        }
        moved
    }

    /// Move the stream's next sector into its ring slot and advance the
    /// cursor. Stops the stream when its chunk is fully delivered.
    fn pop_one_sector(&mut self, storage: &mut VideoStorage) -> bool {
        let slot = Self::ring_slot(self.stream_frame);
        let offset = self.stream_sector as usize * SECTOR_WORDS;
        let sector_buf: &mut [u32; SECTOR_WORDS] = unsafe {
            &mut *(storage.frame_cache[slot][offset..offset + SECTOR_WORDS]
                .as_mut_ptr() as *mut [u32; SECTOR_WORDS])
        };
        if !unsafe { self.cd_reader.read_sector(sector_buf) } {
            psx_rt::tty::print("[VIDEO] CD chunk read failed at frame ");
            psx_rt::tty::print_hex_u32(self.stream_frame as u32);
            psx_rt::tty::print(" sector ");
            psx_rt::tty::print_hex_u32(self.stream_sector as u32);
            psx_rt::tty::print("\n");
            self.abandon_stream();
            return false;
        }

        self.stream_sector += 1;
        if self.stream_sector as usize == SECTORS_PER_FRAME {
            self.stream_sector = 0;
            self.stream_frame += 1;
            // A frame joins the ring only once all of its sectors have
            // landed. The guard is not paranoia about the normal path --
            // exactly one frame completes per call there -- it covers a
            // stream resumed onto a ring whose tail already covers it.
            if self.stream_frame > self.ring_start_frame + self.ring_frames {
                self.ring_frames += 1;
            }
        }
        if self.stream_frame >= self.chunk_end_frame {
            unsafe { self.cd_reader.stop() };
            self.stream_active = false;
        }
        true
    }

    /// Tear down a stream that failed mid-chunk, discarding the ring.
    fn abandon_stream(&mut self) {
        unsafe { self.cd_reader.stop() };
        self.stream_active = false;
        self.ring_frames = 0;
    }

    /// Fill the ring with at least `want` frames of read-ahead, blocking on
    /// the drive. Used for the initial prime and for the rare case where the
    /// decode has caught up with the stream.
    ///
    /// Blocking is safe here precisely because the ring was empty: there is
    /// no decoded frame waiting to be shown, so a wait costs nothing that
    /// was not already lost.
    fn read_ahead_blocking(&mut self, want: u16, storage: &mut VideoStorage) -> bool {
        while (self.ring_start_frame + self.ring_frames) < want {
            if !self.stream_active && !self.start_next_chunk() {
                return false;
            }
            if !self.pop_one_sector(storage) {
                return false;
            }
        }
        true
    }

    /// Wait out the rest of the current display period, keeping a live CD
    /// stream drained, and return the VBlank count once it lands.
    ///
    /// This replaces a bare `wait_vblank` and is not an optimisation. The
    /// drive delivers a sector every ~7.7 ms while a display period is
    /// 16.7 ms, and its data FIFO is only a sector or two deep, so servicing
    /// the stream once per period overruns it: the drive reports "interrupt
    /// not processed in time, missed N sectors", the stream desynchronises,
    /// and the next frame to decode is corrupt -- which is what wedges the
    /// MDEC. Draining on the way to the VBlank is what lets a `ReadN` stay
    /// live across frames at all, and that liveness is the whole overlap.
    fn wait_vblank_serving(&mut self, storage: &mut VideoStorage) -> u32 {
        if !self.using_cd || !self.stream_active {
            psx_rt::interrupts::wait_vblank();
            return psx_rt::interrupts::vblank_count();
        }
        let before = psx_rt::interrupts::vblank_count();
        loop {
            self.overlapped_sectors += self.pump_ready_sectors(storage) as u32;
            let now = psx_rt::interrupts::vblank_count();
            if now != before {
                return now;
            }
            core::hint::spin_loop();
        }
    }

    pub fn start(&mut self) {
        self.start_video(VideoKind::Intro);
    }

    pub fn start_intro(&mut self) {
        self.start_video(VideoKind::Intro);
    }

    pub fn start_outro(&mut self) {
        self.start_video(VideoKind::Outro);
    }

    /// One-time check that the embedded fallback matches the frame geometry it
    /// is decoded with. A mismatch would index past the blob.
    pub fn embedded_fallback_is_consistent() -> bool {
        EMBEDDED_VIDEO.len() == EMBEDDED_FRAME_COUNT as usize * WORDS_PER_FRAME * 4
    }

    pub fn start_video(&mut self, kind: VideoKind) {
        psx_rt::tty::println("[VIDEO] start_video() called");
        self.kind = kind;
        self.frame_idx = 0;
        self.tick = 0;
        self.finished = false;
        self.total_frames = TOTAL_FRAMES;
        self.ticks_per_frame = VBLANKS_PER_VIDEO_FRAME as u8; // 60 Hz / 4 = 15 fps
        self.last_decoded_frame = 0xFFFF;
        self.frame_unpresented = false;
        self.last_swap_vblank = psx_rt::interrupts::vblank_count();
        self.using_cd = false;
        self.ring_start_frame = 0;
        self.ring_frames = 0;
        self.stream_frame = 0;
        self.stream_active = false;
        self.stream_sector = 0;
        self.chunk_end_frame = 0;
        self.vram_dma_ok = true;
        self.vram_dma_fallbacks = 0;
        self.chunk_starts = 0;
        self.overlapped_sectors = 0;
        self.saved_irq_mask = psx_io::irq::mask();
        self.irq_mask_saved = true;
        self.cd_read_retries = 0;

        // Reset and initialize hardware MDEC coprocessor with standard tables
        psx_rt::tty::println("[VIDEO] Initializing MDEC...");
        psx_io::mdec::init();
        psx_rt::tty::print("[VIDEO] MDEC init done. Stat: ");
        psx_rt::tty::print_hex_u32(psx_io::mdec::read_stat());
        psx_rt::tty::print("\n");

        let filename: &[u8] = match kind {
            VideoKind::Intro => b"INTRO.VID",
            VideoKind::Outro => b"OUTRO.VID",
        };

        // Initialize CD streaming if possible
        let storage = unsafe { &mut *core::ptr::addr_of_mut!(STORAGE) };
        let prepared = unsafe { self.cd_reader.prepare() };
        if prepared {
            psx_rt::tty::println("[VIDEO] CD reader prepare OK");
            if let Some(lba) = unsafe { Self::find_vid_lba(filename, &mut self.cd_reader, storage) } {
                self.cd_start_lba = lba;
                psx_rt::tty::print("[VIDEO] Target video located; 7-sector frames, ");
                psx_rt::tty::print_hex_u32(CHUNK_FRAMES as u32);
                psx_rt::tty::print("-frame chunks, ");
                psx_rt::tty::print_hex_u32(RING_FRAMES as u32);
                psx_rt::tty::print("-frame ring, total ");
                psx_rt::tty::print_hex_u32((TOTAL_FRAMES as u32) * SECTORS_PER_FRAME as u32);
                psx_rt::tty::print(" sectors\n");
                self.using_cd = true;
                // Prime one chunk. Filling the whole ring would be ~1.7 s of
                // black screen before the first frame, and the scheduler
                // refills the rest during playback anyway -- the video has
                // 116 display periods of slack to spend on it.
                if !self.read_ahead_blocking(PRIME_FRAMES as u16, storage) {
                    psx_rt::tty::println("[VIDEO] initial chunk read failed");
                    self.using_cd = false;
                }
            }
        } else {
            psx_rt::tty::println("[VIDEO] CD reader prepare FAILED");
        }

        // Start audio after the initial prime so its first frame remains
        // synchronized with the first displayed video frame.
        match kind {
            VideoKind::Intro => AudioManager::play_intro_audio(),
            VideoKind::Outro => AudioManager::play_outro_audio(),
        }
    }

    pub fn stop(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;

        // Restore the IRQ mask unconditionally. `SectorReader::prepare` masks
        // CD and timer interrupts for the duration of a stream, and this guard
        // used to be `if self.using_cd` -- so a stream that gave up early left
        // the SPU and CD handlers masked off for the rest of the process, which
        // the VBlank-driven frame loop depends on.
        if self.irq_mask_saved {
            unsafe {
                self.cd_reader.stop();
                psx_io::irq::set_mask(self.saved_irq_mask);
            }
            self.irq_mask_saved = false;
        }
        self.using_cd = false;

        AudioManager::stop_intro_audio();
        AudioManager::stop_outro_audio();
    }

    /// Check for a skip request. Returns true when the video is over.
    ///
    /// Pacing lives in [`VideoPlayer::present`], not here. This used to
    /// count loop iterations, but an iteration that also decoded was not
    /// one display period long, so the count drifted against real time and
    /// each frame cost `ticks_per_frame + decode_time` instead of
    /// `ticks_per_frame`.
    pub fn update(&mut self, pad: &PadState, prev: &ButtonState) -> bool {
        if self.finished {
            return true;
        }

        // Allow skipping via CROSS or START
        let just_cross = pad.buttons.is_held(button::CROSS) && !prev.is_held(button::CROSS);
        let just_start = pad.buttons.is_held(button::START) && !prev.is_held(button::START);
        if just_cross || just_start {
            self.stop();
            return true;
        }

        false
    }

    /// Run one display period of video playback: decode the next frame if
    /// the back buffer is free, and present a frame once it is finished.
    ///
    /// The present is decoupled from the decode. `FrameBuffer::swap()`
    /// shows the buffer that was being drawn into, so swapping *before* the
    /// decode hands the decode only the window between one swap and the
    /// next, and any overrun pushes the following swap out by a whole
    /// display period. Presenting only a frame that is already complete
    /// gives the decode the full `VBLANKS_PER_VIDEO_FRAME` window.
    ///
    /// The decode starts as soon as the back buffer is free, immediately
    /// after the previous present, rather than when the next frame falls
    /// due. Waiting for the due time would make each period
    /// `target + decode_time` instead of `max(target, decode_time)`.
    ///
    /// The swap is also floored at the target dwell: without that gate the
    /// cadence follows the decode rather than the target, so a fast decode
    /// would be presented every couple of display periods and the slow
    /// frames would lurch. Pacing has to be a floor the decode can be early
    /// for, not just a deadline it can be late against.
    ///
    /// The read and the decode overlap. `decode_and_upload` drains whatever
    /// sectors the drive has ready between MDEC slices, so a chunk is
    /// streamed in while the previous chunk is being decoded instead of
    /// landing as an atomic burst between presents. That is what makes 15 fps
    /// fit: serialised, the frame costs 54.89 ms of drive plus 28 ms of
    /// decode against a 66.67 ms budget, and the drive is the binding term
    /// either way -- overlapping hides the decode inside it.
    pub fn present(&mut self, renderer: &mut crate::renderer::Renderer) {
        if self.finished {
            psx_rt::interrupts::wait_vblank();
            return;
        }

        let storage = unsafe { &mut *core::ptr::addr_of_mut!(STORAGE) };

        let now = self.wait_vblank_serving(storage);

        // 1. Show a frame that is already decoded and has served its dwell
        //    time. This costs one register write and must not be delayed by
        //    anything, least of all a CD read.
        if self.frame_unpresented
            && now != self.last_swap_vblank
            && now.wrapping_sub(self.last_swap_vblank) >= VBLANKS_PER_VIDEO_FRAME
        {
            renderer.fb.swap();
            self.last_swap_vblank = now;
            self.frame_unpresented = false;
        }

        if self.frame_idx >= self.total_frames && !self.frame_unpresented {
            self.stop();
            return;
        }

        // 2. The back buffer is free, so decode the next frame. The read it
        //    needs is already in the ring unless the decode has caught up
        //    with the drive, which is the only case that blocks.
        if !self.frame_unpresented && self.frame_idx < self.total_frames {
            if self.using_cd && !self.ring_has(self.frame_idx) {
                if !self.read_ahead_blocking(self.frame_idx + 1, storage) {
                    // One dropped sector used to latch streaming off for the
                    // whole cutscene, collapsing a 150-frame intro into a
                    // 16-frame embedded loop with the voiceover still playing
                    // against it. Re-seek from a clean state before giving up:
                    // a transient read error is the common case, not a broken
                    // file.
                    self.cd_read_retries = self.cd_read_retries.saturating_add(1);
                    if self.cd_read_retries <= CD_READ_RETRIES {
                        self.reset_stream();
                        if !self.read_ahead_blocking(self.frame_idx + 1, storage) {
                            self.using_cd = false;
                        }
                    } else {
                        psx_rt::tty::println("[VIDEO] CD read retries exhausted; using embedded frames");
                        self.using_cd = false;
                    }
                } else {
                    self.cd_read_retries = 0;
                }
            }
            if self.decode_and_upload(renderer, storage) {
                self.frame_unpresented = true;
            }
            return;
        }

        // 3. A decoded frame is sitting in the back buffer waiting out its
        //    dwell time and there is nothing to decode this period, so the
        //    wait is idle CPU time. Spend it pulling the next chunk in.
        if self.frame_unpresented && self.using_cd {
            self.overlapped_sectors += self.pump_ready_sectors(storage) as u32;
            // Starting a chunk blocks for the seek, so only do it with enough
            // dwell left to hide it: the seek costs ~4 display periods and a
            // decoded frame in hand covers exactly that.
            if !self.stream_active
                && self.ring_free() >= CHUNK_FRAMES as u16
                && self.ring_start_frame + self.ring_frames < self.total_frames
            {
                let _ = self.start_next_chunk();
            }
        }
    }

    /// Decode the next video frame into the back buffer. Returns false when
    /// there was nothing to decode.
    fn decode_and_upload(
        &mut self,
        renderer: &crate::renderer::Renderer,
        storage: &mut VideoStorage,
    ) -> bool {
        let frame_to_show = self.frame_idx;
        {
            let mut read_ok = false;

            if self.using_cd && self.ring_has(frame_to_show) {
                // Copy out of the ring before the decode starts. The slot is
                // addressed by `frame % RING_FRAMES`, so once the decode
                // begins and the pump starts refilling the ring, the source
                // slot is allowed to be overwritten.
                let slot = Self::ring_slot(frame_to_show);
                storage.frame_words.copy_from_slice(&storage.frame_cache[slot]);
                read_ok = true;
            }

            if !read_ok {
                // Load from embedded fallback frames. Only meaningful if the
                // blob actually matches the decode geometry.
                if !Self::embedded_fallback_is_consistent() {
                    psx_rt::tty::println("[VIDEO] embedded fallback size mismatch; cannot decode");
                    // Nothing was decoded, so report it: `present` must not
                    // mark a frame ready that was never written to VRAM.
                    return false;
                }
                let embed_idx = (frame_to_show % EMBEDDED_FRAME_COUNT) as usize;
                let src_offset = embed_idx * WORDS_PER_FRAME * 4;
                if src_offset + WORDS_PER_FRAME * 4 <= EMBEDDED_VIDEO.len() {
                    let src_slice = &EMBEDDED_VIDEO[src_offset..src_offset + WORDS_PER_FRAME * 4];
                    for (i, word) in storage.frame_words.iter_mut().enumerate() {
                        *word = u32::from_le_bytes([
                            src_slice[i * 4],
                            src_slice[i * 4 + 1],
                            src_slice[i * 4 + 2],
                            src_slice[i * 4 + 3],
                        ]);
                    }
                    read_ok = true;
                }
            }

            if read_ok {
                if frame_to_show < 3 {
                    psx_rt::tty::print("[VIDEO] Decoding frame ");
                    psx_rt::tty::print_hex_u32(frame_to_show as u32);
                    psx_rt::tty::print(" cmd=");
                    psx_rt::tty::print_hex_u32(storage.frame_words[0]);
                    psx_rt::tty::print("\n");
                }

                // Feed the frame into the MDEC coprocessor
                psx_io::mdec::start_decode_frame(&storage.frame_words);

                // Drain 20 vertical macroblock columns (16x240 pixels each) and upload to VRAM
                let fb_y = renderer.fb.buffer_y(renderer.fb.drawing);

                for col in 0..20u16 {
                    // Drain one slice (1,920 words) via DMA channel 1 (or PIO fallback)
                    // The frame input is submitted before any output is
                    // requested. Do not wait forever if a malformed/incomplete
                    // bitstream stops producing decoded pixels.
                    let dma_ok = psx_io::mdec::drain_slice_dma(&mut storage.slice_words);
                    if !dma_ok {
                        if frame_to_show < 2 && col == 0 {
                            psx_rt::tty::print("[VIDEO] DMA timed out on slice 0! stat=");
                            psx_rt::tty::print_hex_u32(psx_io::mdec::read_stat());
                            psx_rt::tty::print("\n");
                        }
                        psx_io::dma::abort(psx_io::dma::Channel::MdecOut);
                        // PIO fallback if DMA timed out. Stop as soon as the
                        // MDEC has finished processing and has no more output.
                        // DATA_OUT_EMPTY can be transient between macroblocks.
                        let mut output_words = 0usize;
                        // One budget for the whole slice. A per-word budget let
                        // a partially drained slice walk 1,920 * 10,000 spins,
                        // which is several seconds of frozen screen inside a
                        // single frame.
                        let mut slice_spins = 0u32;
                        for out in storage.slice_words.iter_mut() {
                            while psx_io::mdec::read_stat() & psx_hw::mdec::status::DATA_OUT_EMPTY != 0
                                && (psx_io::mdec::is_busy()
                                    || psx_io::dma::is_busy(psx_io::dma::Channel::MdecIn))
                                && slice_spins < PIO_SPIN_BUDGET
                            {
                                slice_spins += 1;
                                core::hint::spin_loop();
                            }
                            if slice_spins >= PIO_SPIN_BUDGET {
                                psx_rt::tty::println("[VIDEO] MDEC PIO fallback budget exhausted; ending slice");
                                break;
                            }
                            if psx_io::mdec::read_stat() & psx_hw::mdec::status::DATA_OUT_EMPTY != 0 {
                                break;
                            }
                            *out = psx_io::mdec::read_data();
                            output_words += 1;
                        }
                        if output_words == 0 {
                            psx_rt::tty::println("[VIDEO] MDEC produced no more pixels; ending frame decode");
                            break;
                        }
                    } else if frame_to_show < 2 && col == 0 {
                        psx_rt::tty::println("[VIDEO] DMA slice 0 OK!");
                    }

                    // Upload slice to VRAM at destination rectangle
                    let rect = VramRect::new(col * 16, fb_y, 16, VIDEO_H);
                    self.upload_slice(rect, &storage.slice_words);

                    // Overlap: the MDEC runs off DMA, so the CPU has been
                    // spinning on this slice and the drive has been filling
                    // its data FIFO for the ~1.3 ms it took. Take whatever is
                    // ready. This is the whole point of the chunked reader --
                    // it converts the read from an atomic burst between
                    // presents into work done inside the decode.
                    //
                    // A slice that falls into the PIO fallback above `break`s
                    // out of this loop, so the post-loop pump covers it.
                    self.overlapped_sectors += self.pump_ready_sectors(storage) as u32;
                }

                // Whatever the drive finished while the last slice went out.
                self.overlapped_sectors += self.pump_ready_sectors(storage) as u32;

                if frame_to_show < 3 {
                    psx_rt::tty::println("[VIDEO] Frame upload complete.");
                }

                self.last_decoded_frame = frame_to_show;
                self.frame_idx = frame_to_show + 1;
                // Release the frame we just consumed so the stream can refill
                // its slot. The stream only ever writes at the ring's tail,
                // which is `RING_FRAMES` past this once the ring is full.
                if self.using_cd && self.ring_start_frame == frame_to_show && self.ring_frames > 0
                {
                    self.ring_start_frame += 1;
                    self.ring_frames -= 1;
                }
            }
            read_ok
        }
    }

    /// Push one decoded 16x240 slice into VRAM.
    ///
    /// DMA channel 2 first: it moves the same bytes the GP0 command port
    /// would, but without one CPU store per word, and measures 1.44 ms per
    /// frame against 12.55 ms -- by some margin the largest win available.
    ///
    /// `psx-vram` documents that on real silicon channel 2 can latch its
    /// start bit and stay busy forever; `dma_copy_to_vram` bounds that wait
    /// and aborts, returning false with the GP0(A0) header already emitted.
    /// So a failure is recovered: the slice is re-sent the safe way, which
    /// rewrites the header and the whole payload, leaving VRAM correct. DMA
    /// is then off for the rest of the session, so a wedging target pays
    /// the bounded wait once rather than once per frame.
    fn upload_slice(&mut self, rect: VramRect, words: &[u32; WORDS_PER_SLICE]) {
        if self.vram_dma_ok && dma_copy_to_vram(rect, words.as_ptr()) {
            return;
        }
        if self.vram_dma_ok {
            self.vram_dma_ok = false;
            self.vram_dma_fallbacks += 1;
            psx_rt::tty::println("[VIDEO] VRAM DMA wedged; using GP0 path for the rest of this video");
        }
        upload_words(rect, words);
    }

    /// True when a newly decoded video frame is ready to be presented.
    /// The game loop uses this to avoid swapping to the stale back buffer on
    /// intermediate VBlanks between 15 fps video frames.
    pub fn needs_redraw(&self) -> bool {
        self.frame_idx != self.last_decoded_frame
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }
}
