//! Cinematic Intro/Outro Video playback for Plattypus PSX.
//!
//! Plays a 320x240 movie at 15 fps from the disc, with its audio carried in
//! the same stream as XA-ADPCM and decoded by the drive.
//!
//! # Pipeline
//!
//! A movie is a `.VID` file in STR form: 2048-byte sectors, each a 32-byte
//! chunk header plus 2016 bytes of one frame's MDEC BS v2 bitstream. A frame
//! spans a *variable* number of sectors, so the stream is demuxed by chunk
//! header rather than at a fixed stride.
//!
//! Per frame, in [`VideoPlayer::present`]:
//!
//! 1. **Pump.** [`VideoPlayer::pump`] drains every sector the drive has
//!    ready, never waiting for one, and feeds each to
//!    [`strfmt::FrameAssembler`]. It runs between every other unit of work,
//!    *including inside every wait*, because a sector arrives every ~13 ms
//!    while a display period is 16.7 ms and the drive's data FIFO is only a
//!    sector or two deep.
//! 2. **Expand.** [`bs::decode_frame`] turns the bitstream into the MDEC's
//!    run-length halfwords on the CPU, pumping the drive once per macroblock
//!    column.
//! 3. **Decode.** [`mdec::decode_start`] feeds those over DMA0;
//!    [`mdec::read_column`] pulls each 16-pixel column of decoded macroblocks
//!    back over DMA1.
//! 4. **Upload.** [`dma_copy_to_vram`] puts each column into the back buffer
//!    over channel 2 in block mode. A 16-pixel column is 8 words per row:
//!    word-aligned, and inside the 16-word block limit the GPU's DMA FIFO
//!    accepts, so the write is always DMA and never a PIO loop.
//! 5. **Flip.** Double-buffered, display start set at a VBlank, paced to
//!    every 4th VBlank (15 fps on 60 Hz NTSC).
//!
//! # Why BS v2
//!
//! The previous format stored the MDEC's macroblock run-lengths directly at
//! a fixed 7 sectors per frame: 1050 sectors for a 150-frame cut, 2100 KiB,
//! and 1.75 sectors of drive time per 4-VBlank frame against a 2x drive that
//! sustains 2.17. That left ~116 display periods of slack for the whole
//! movie, which a fixed-stride reader spends on stop-and-re-seek cycles --
//! a measured 4.02 display periods each. The result was 15.00 fps with 9
//! presents one period late, exactly one per seek.
//!
//! BS v2 is DCT-based and reaches 5.00 sectors per frame for the same
//! picture: 740 sectors, 1.40x less data. It is encoded with
//! `psxavenc -x 1`, which sizes the file to exactly fill a 1x drive -- the
//! speed the SDK documents as reliable, after sustained 2x reads were found
//! to corrupt payload silently on real silicon.
//!
//! See `docs/perf/why-6fps.md` for the measurements behind both numbers.

use core::ptr::addr_of_mut;

use psx_fmv::{bs, mdec, str as strfmt};
use psx_io::dma;
use psx_pad::{button, ButtonState, PadState};
use psx_pack::cd::{SectorReader, SECTOR_WORDS};

use crate::audio::AudioManager;
use psx_vram::{VramRect, dma_copy_to_vram, upload_words};



pub const VIDEO_W: u16 = 320;
pub const VIDEO_H: u16 = 240;
/// Macroblock columns (320 / 16).
pub const COLUMNS: u16 = VIDEO_W / 16;
/// Macroblock rows (240 / 16).
pub const ROWS: u32 = VIDEO_H as u32 / 16;
/// Display periods each video frame is held for. Four gives 15 fps on a
/// 60 Hz NTSC display, which is the rate the assets are encoded at.
pub const VBLANKS_PER_VIDEO_FRAME: u32 = 4;

/// Frames a movie carries. The real end-of-movie signal is the trailing EOF
/// marker (see [`VideoPlayer::pump`]); this only sizes the bench's report.
pub const TOTAL_FRAMES: u16 = 148;
/// Sectors one frame may span. A 1x BS v2 frame is 5; 16 leaves headroom for
/// a busier movie and bounds the reassembly buffer regardless of what the
/// drive turns out to deliver.
pub const MAX_CHUNKS: u16 = 16;
/// Reassembly buffer per slot, in u32 words.
pub const SLOT_WORDS: usize = MAX_CHUNKS as usize * strfmt::CHUNK_PAYLOAD_BYTES / 4;
/// Frame slots: one decoding, one ready, the rest filling.
pub const SLOTS: usize = 6;
/// MDEC run-length buffer in u32 words. A 320x240 frame is 300 macroblocks
/// of 6 blocks, and BS v2 emits at most 64 halfwords per block, so 115 200
/// halfwords is the true worst case. 16K words (32 768 halfwords) measures
/// ~7 200 in practice and is the bound the reference player uses.
pub const RLE_WORDS: usize = 16 * 1024;
/// Decoded pixels in one 16x240 column: 8 words per row.
pub const COLUMN_WORDS: usize = 8 * VIDEO_H as usize;

/// Display periods with no sector and nothing to show before the stream is
/// declared finished. A movie's EOF marker normally ends it long before
/// this; the watchdog covers a truncated file.
const STALL_VBLANKS: u32 = 120;

/// What a slot is doing. A slot is only refilled once it is `Free`, which is
/// what stops a queued frame being overwritten mid-decode.
#[derive(Copy, Clone, PartialEq, Eq)]
enum SlotState {
    /// Being filled by the assembler.
    Filling,
    /// Complete, waiting to be decoded.
    Ready,
    /// Being decoded into VRAM right now.
    Decoding,
    /// Shown or discarded; available to the assembler again.
    Free,
}

/// BSS-resident streaming and decode storage.
struct VideoStorage {
    /// Frame reassembly slots. Disjoint statics, so a `&mut` to one never
    /// aliases another.
    slots: [[u32; SLOT_WORDS]; SLOTS],
    /// MDEC run-length halfwords, filled by the CPU-side BS v2 expand.
    rle: [u32; RLE_WORDS],
    /// One decoded 16x240 column.
    column: [u32; COLUMN_WORDS],
    /// Bounce buffer for the root-directory scan and the per-sector pump.
    sector: [u32; SECTOR_WORDS],
}

impl VideoStorage {
    const fn new() -> Self {
        Self {
            slots: [[0; SLOT_WORDS]; SLOTS],
            rle: [0; RLE_WORDS],
            column: [0; COLUMN_WORDS],
            sector: [0; SECTOR_WORDS],
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
    pub finished: bool,
    pub using_cd: bool,
    pub cd_start_lba: u32,
    /// VBlank count of the most recent display flip.
    pub last_swap_vblank: u32,
    /// Times the channel-2 VRAM upload wedged and the GP0 path took over.
    pub vram_dma_fallbacks: u32,
    /// Columns uploaded over DMA channel 2.
    pub vram_dma_columns: u32,
    /// Frames flipped.
    pub frames_shown: u16,
    /// Frames whose bitstream failed to expand or whose MDEC decode failed.
    pub decode_errors: u16,
    /// Frames abandoned because a chunk never arrived.
    pub dropped_frames: u16,
    /// Drive errors reported by the reader.
    pub cd_errors: u16,
    /// Times the stream was stopped and re-seeked to throttle the read rate
    /// to the decode rate. Expected to be non-zero; a zero would mean the
    /// drive is slower than the decode, which is the other failure.
    pub restarts: u32,
    /// The stream ended on the stall watchdog rather than its EOF marker.
    pub stalled: bool,
    /// Sectors moved into a slot by the pump, i.e. read work that happened
    /// while a frame was being decoded rather than in front of one.
    pub pumped_sectors: u32,
    /// Sectors moved *during* a decode specifically.
    pub overlapped_sectors: u32,

    /// VBlank the next flip is due at.
    next_flip_vblank: u32,
    saved_irq_mask: u32,
    irq_mask_saved: bool,

    /// Chunk reassembly.
    asm: strfmt::FrameAssembler,
    /// Slot the assembler is currently filling.
    fill_slot: usize,
    /// Highest frame number seen, to recognise the trailing EOF marker.
    last_frame_seen: u32,
    slot_state: [SlotState; SLOTS],
    /// FIFO queue of slots holding complete frames ready for presentation.
    ready_queue: [usize; SLOTS],
    ready_len: usize,
    cd_reader: SectorReader,
    /// Whether VRAM uploads may still go over DMA channel 2.
    vram_dma_ok: bool,
    /// LBA of the next sector the stream will deliver. Tracked directly
    /// rather than derived from a frame index, because a BS v2 frame spans a
    /// variable number of sectors and the STR file is just a sector
    /// sequence.
    next_lba: u32,
    /// A `ReadN` is running.
    stream_live: bool,
    /// VBlank of the last sector delivered, for the stall watchdog.
    last_sector_vblank: u32,
    /// VBlank the current wait-for-a-frame started at.
    wait_started_vblank: u32,
    /// Set by the EOF marker; ends the movie.
    pub eof: bool,

}

impl VideoPlayer {
    pub fn new() -> Self {
        Self {
            kind: VideoKind::Intro,
            finished: false,
            using_cd: false,
            cd_start_lba: 0,
            last_swap_vblank: psx_rt::interrupts::vblank_count(),
            vram_dma_fallbacks: 0,
            vram_dma_columns: 0,
            frames_shown: 0,
            decode_errors: 0,
            dropped_frames: 0,
            cd_errors: 0,
            restarts: 0,
            stalled: false,
            pumped_sectors: 0,
            overlapped_sectors: 0,
            next_flip_vblank: 0,
            saved_irq_mask: 0,
            irq_mask_saved: false,
            asm: strfmt::FrameAssembler::new(),
            fill_slot: 0,
            last_frame_seen: 0,
            slot_state: [SlotState::Free; SLOTS],
            ready_queue: [0; SLOTS],
            ready_len: 0,
            cd_reader: SectorReader::new(),
            vram_dma_ok: true,
            next_lba: 0,
            stream_live: false,
            last_sector_vblank: 0,
            wait_started_vblank: 0,
            eof: false,
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

    pub fn start_video(&mut self, kind: VideoKind) {
        crate::audio::AudioManager::stop_all();
        psx_rt::tty::println("[VIDEO] start_video() called");
        self.kind = kind;
        self.reset_for_playback();

        psx_rt::tty::println("[VIDEO] Initializing MDEC...");
        // SAFETY: single-threaded, and no other MDEC user exists.
        let tables_ok = {
            mdec::reset();
            mdec::load_tables()
        };
        if !tables_ok {
            psx_rt::tty::println("[VIDEO] MDEC table load FAILED");
        }
        psx_rt::tty::print("[VIDEO] MDEC ready. Stat: ");
        psx_rt::tty::print_hex_u32(psx_io::mdec::read_stat());
        psx_rt::tty::print("\n");

        let filename: &[u8] = match kind {
            VideoKind::Intro => b"INTRO.STR",
            VideoKind::Outro => b"OUTRO.STR",
        };

        // SAFETY: single static, only this player touches it.
        let storage = unsafe { &mut *addr_of_mut!(STORAGE) };

        // 1x speed with XA-ADPCM audio decode and file 1 / channel 0 filter.
        // Interleaved audio sectors are routed by the drive directly to the
        // SPU at 37800 Hz stereo, costing zero CPU time.
        //
        // SAFETY: MMIO, single-threaded; this call takes interrupt policy.
        let prepared = unsafe {
            self.cd_reader.demute()
                && self.cd_reader.prepare_mode(0x40 | 0x08)
                && self.cd_reader.set_filter(1, 0)
        };
        psx_io::cdrom::set_audio_mixer(0x80, 0, 0x80, 0);

        if prepared {
            psx_rt::tty::println("[VIDEO] CD reader prepare OK (1x XA)");
            // SAFETY: as above.
            if let Some(lba) = unsafe { Self::find_movie_lba(filename, &mut self.cd_reader, storage) }
            {
                self.cd_start_lba = lba;
                self.next_lba = lba;
                self.using_cd = true;
                self.fill_slot = 0;
                self.slot_state[0] = SlotState::Filling;
                if unsafe { self.cd_reader.start_read(lba) } {
                    self.stream_live = true;
                }
                psx_rt::tty::println("[VIDEO] streaming BS v2 movie at 1x with XA audio");
            }
        } else {
            psx_rt::tty::println("[VIDEO] CD reader prepare FAILED");
        }

        if !self.using_cd {
            psx_rt::tty::println("[VIDEO] no movie on disc; skipping");
        }
    }

    fn reset_for_playback(&mut self) {
        let now = psx_rt::interrupts::vblank_count();
        self.finished = false;
        self.using_cd = false;
        self.cd_start_lba = 0;
        self.vram_dma_fallbacks = 0;
        self.vram_dma_columns = 0;
        self.frames_shown = 0;
        self.decode_errors = 0;
        self.dropped_frames = 0;
        self.cd_errors = 0;
        self.restarts = 0;
        self.stalled = false;
        self.pumped_sectors = 0;
        self.overlapped_sectors = 0;
        self.next_flip_vblank = now;
        self.last_swap_vblank = now;
        self.asm = strfmt::FrameAssembler::new();
        self.fill_slot = 0;
        self.last_frame_seen = 0;
        self.slot_state = [SlotState::Free; SLOTS];
        self.ready_queue = [0; SLOTS];
        self.ready_len = 0;
        self.vram_dma_ok = true;
        self.next_lba = self.cd_start_lba;
        self.stream_live = false;
        self.last_sector_vblank = now;
        self.wait_started_vblank = now;
        self.eof = false;
        self.saved_irq_mask = psx_io::irq::mask();
        self.irq_mask_saved = true;
    }

    /// Pop the oldest ready frame from the FIFO queue.
    fn take_ready_slot(&mut self) -> Option<usize> {
        if self.ready_len == 0 {
            return None;
        }
        let slot = self.ready_queue[0];
        self.ready_queue.copy_within(1..self.ready_len, 0);
        self.ready_len -= 1;
        Some(slot)
    }


    /// Locate a movie in the ISO 9660 root directory (extent 20).
    ///
    /// # Safety
    /// MMIO, single-threaded; `reader` must not have a stream running.
    unsafe fn find_movie_lba(
        filename: &[u8],
        reader: &mut SectorReader,
        storage: &mut VideoStorage,
    ) -> Option<u32> {
        psx_rt::tty::println("[VIDEO] Scanning root directory (Sector 20)...");
        // SAFETY: caller owns the reader; this is the documented
        // prepare -> start_read -> read_sector -> stop sequence.
        unsafe {
            if !reader.start_read(20) {
                psx_rt::tty::println("[VIDEO] start_read(20) FAILED");
                return None;
            }
            let ok = reader.read_sector(&mut storage.sector);
            reader.stop();
            if !ok {
                psx_rt::tty::println("[VIDEO] read_sector(20) FAILED");
                return None;
            }
        }

        let bytes: &[u8] = unsafe {
            core::slice::from_raw_parts(storage.sector.as_ptr() as *const u8, 2048)
        };

        let mut off = 0usize;
        while off < bytes.len() {
            let record_len = bytes[off] as usize;
            if record_len == 0 {
                break;
            }
            // A directory record must be at least 33 bytes to carry the LBA
            // at offset 2..6 and the name length at offset 32. A short record
            // would pass the `off + record_len` check and then read the LBA
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
            if name_len != 0 && off + 33 + name_len <= bytes.len() {
                let name = &bytes[off + 33..off + 33 + name_len];
                // ISO 9660 identifiers carry a `;1` version suffix, so
                // `INTRO.VID` is stored as `INTRO.VID;1`. A bare prefix match
                // would also match a hypothetical `INTRO.VIDX`, so require the
                // version separator or the end of the identifier.
                let matches = name.starts_with(filename)
                    && (name.len() == filename.len() || name[filename.len()] == b';');
                if matches {
                    psx_rt::tty::print("[VIDEO] Found movie at LBA: ");
                    psx_rt::tty::print_hex_u32(lba);
                    psx_rt::tty::print("\n");
                    return Some(lba);
                }
            }
            off += record_len;
        }
        psx_rt::tty::println("[VIDEO] Movie not found in Sector 20");
        None
    }

    /// Drain every sector the drive has ready, without waiting for one, and
    /// feed each to the chunk reassembler.
    ///
    /// This runs between every other unit of work, including inside waits,
    /// and that is load-bearing rather than an optimisation. A sector
    /// arrives every ~13 ms at 1x while a display period is 16.7 ms, and the
    /// drive's data FIFO is only a sector or two deep, so servicing the
    /// stream once per display period overruns it: the drive reports
    /// "interrupt not processed in time, missed N sectors", the stream
    /// desynchronises, and the next frame to decode is corrupt -- which
    /// presents as an MDEC stall. Getting this wrong is what wedged the
    /// previous, fixed-stride player.
    fn pump(&mut self, storage: &mut VideoStorage) {
        while self.using_cd && !self.eof {
            // Throttle: only read while a slot is free to receive the frame.
            //
            // The drive at 2x delivers ~1.7 frames per frame period against a
            // consumption of one, so an unthrottled stream runs the assembler
            // away from the decoder and every extra frame *replaces* one that
            // was decoded and never shown -- which is how a 148-frame movie
            // ends up displaying 77. Stopping the drive instead keeps the
            // read rate equal to the decode rate; the ring is several frames
            // deep, so the stop/restart is hidden behind frames already
            // decoded and costs no presents.
            //
            // Restarting is a seek, and a seek costs ~4.02 display periods
            // measured -- but there are already decoded frames to cover it,
            // which is what the ring is for.
            // Do not read past what the ring can hold. At 1x the drive rate
            // and the decode rate are both 5.00 sectors per frame, so this
            // should not bind; it is a guard against the assembler being
            // starved of slots, not a throttle.
            // Claim a fill target only when we do not already have one. The
            // assembler holds a single partly-filled slot and fills it chunk
            // by chunk, so claiming per *sector* would spend a slot every
            // sector and exhaust the ring before the first frame completed.
            if self.slot_state[self.fill_slot] != SlotState::Filling && !self.acquire_slot() {
                if self.stream_live {
                    // SAFETY: single-threaded, one reader.
                    unsafe { self.cd_reader.stop() };
                    self.stream_live = false;
                }
                return;
            }
            if !self.stream_live {
                // SAFETY: no ReadN is running right now.
                if !unsafe { self.cd_reader.start_read(self.next_lba) } {
                    psx_rt::tty::println("[VIDEO] restart ReadN failed");
                    self.using_cd = false;
                    return;
                }
                self.stream_live = true;
                self.restarts += 1;
            }

            // Non-blocking: take a sector only if one is already there.
            //
            // SAFETY: single-threaded; a ReadN is running while stream_live,
            // and `sector` is only used here.
            match unsafe { self.cd_reader.try_read_sector(&mut storage.sector) } {
                // Nothing yet: the caller is mid-work, and will pump again.
                Ok(false) => return,
                Ok(true) => {}
                Err(()) => {
                    psx_rt::tty::print("[VIDEO] CD read error, diag=");
                    psx_rt::tty::print_hex_u32(self.cd_reader.diag());
                    psx_rt::tty::print("\n");
                    self.cd_errors += 1;
                    self.using_cd = false;
                    return;
                }
            }
            self.pumped_sectors += 1;
            self.next_lba = self.next_lba.wrapping_add(1);
            self.last_sector_vblank = psx_rt::interrupts::vblank_count();
            // SAFETY: `sector` holds 2048 valid bytes; read-only view.
            let sector: &[u8] =
                unsafe { core::slice::from_raw_parts(storage.sector.as_ptr() as *const u8, 2048) };

            // End of file. A `ReadN` stream does not stop at a file boundary
            // -- the drive keeps delivering whatever follows -- so a movie
            // carries a trailing marker sector whose chunk header restarts at
            // frame 1. Frame 1 after a later frame is that boundary, and it is
            // the only unambiguous end signal: without it the last movie on
            // the disc runs off the end of the image.
            let Some(chunk) = strfmt::Chunk::parse(sector) else {
                continue;
            };
            if chunk.frame == 1 && self.last_frame_seen > 1 {
                psx_rt::tty::print("[VIDEO] EOF marker after frame ");
                psx_rt::tty::print_hex_u32(self.last_frame_seen - 1);
                psx_rt::tty::print("\n");
                self.eof = true;
                // Stop the drive. A `ReadN` does not end at a file boundary,
                // and once `eof` is set the pump returns immediately on every
                // later call without ever draining -- so a live stream would
                // overrun its FIFO for the rest of the session.
                if self.stream_live {
                    // SAFETY: single-threaded, one reader.
                    unsafe { self.cd_reader.stop() };
                    self.stream_live = false;
                }
                return;
            }
            self.last_frame_seen = chunk.frame;

            // Reassemble into the filling slot. SAFETY: slots are disjoint
            // statics and the assembler only touches the one passed in.
            let dropped_before = self.asm.dropped;
            let buf: &mut [u8] = unsafe {
                let p = addr_of_mut!(storage.slots[self.fill_slot]) as *mut u8;
                core::slice::from_raw_parts_mut(p, SLOT_WORDS * 4)
            };
            if self.asm.add(sector, buf).is_some() {
                self.slot_state[self.fill_slot] = SlotState::Ready;
                if self.ready_len < SLOTS {
                    self.ready_queue[self.ready_len] = self.fill_slot;
                    self.ready_len += 1;
                }
                self.dropped_frames += (self.asm.dropped - dropped_before) as u16;
                // Claim the next slot now if one is free; if not, reclaim the oldest
                // ready frame so the drive never has to stop during 1x streaming.
                self.acquire_slot();
            }
        }
    }

    /// Point the assembler at a slot it may write.
    ///
    /// If all slots are occupied, we drop the oldest ready frame from the FIFO queue
    /// rather than stopping the CD drive. In 1x streaming mode with CD-XA audio,
    /// stopping the drive breaks continuous 37.8 kHz audio playback and incurs
    /// a 60-120 ms seek penalty to restart, which starves the pipeline and causes stutter.
    fn acquire_slot(&mut self) -> bool {
        if let Some(s) = (0..SLOTS).find(|&s| self.slot_state[s] == SlotState::Free) {
            self.slot_state[s] = SlotState::Filling;
            self.fill_slot = s;
            true
        } else if let Some(oldest) = self.take_ready_slot() {
            // Ring buffer full: reclaim the oldest ready frame to keep video in lock-step
            // with uninterrupted CD-XA audio.
            self.dropped_frames += 1;
            self.slot_state[oldest] = SlotState::Filling;
            self.fill_slot = oldest;
            true
        } else {
            false
        }
    }

    /// Check for a skip request. Returns true when the movie is over.
    pub fn update(&mut self, pad: &PadState, prev: &ButtonState) -> bool {
        if self.finished {
            return true;
        }
        let just_cross = pad.buttons.is_held(button::CROSS) && !prev.is_held(button::CROSS);
        let just_start = pad.buttons.is_held(button::START) && !prev.is_held(button::START);
        if just_cross || just_start {
            self.stop();
            return true;
        }
        false
    }

    /// Decode and show one frame, paced to every 4th VBlank.
    ///
    /// This owns the whole display period. It pumps the drive while waiting,
    /// so the stream is serviced whenever the CPU would otherwise be idle,
    /// and the flip is timed from the VBlank IRQ rather than from however
    /// long the decode happened to take. Pacing is a floor the decode can be
    /// early for, not a deadline it can be late against: a decode that
    /// overruns pushes the flip, but a fast one never pulls it forward.
    pub fn present(&mut self, renderer: &mut crate::renderer::Renderer) {
        if self.finished {
            psx_rt::interrupts::wait_vblank();
            return;
        }
        // SAFETY: single static, only this player touches it.
        let storage = unsafe { &mut *addr_of_mut!(STORAGE) };

        if !self.using_cd && !self.eof {
            self.stop();
            return;
        }

        // 1. Ensure at least one frame is ready in the FIFO queue before presenting.
        //    On initial startup, wait for 2 frames so the ring buffer holds a 1-frame jitter buffer.
        let min_buffered = if self.frames_shown == 0 { 2 } else { 1 };
        if self.ready_len < min_buffered && !self.eof && self.using_cd {
            self.wait_started_vblank = psx_rt::interrupts::vblank_count();
            loop {
                self.pump(storage);
                if self.ready_len >= min_buffered || self.eof || !self.using_cd {
                    break;
                }
                let idle =
                    psx_rt::interrupts::vblank_count().wrapping_sub(self.wait_started_vblank);
                if idle > STALL_VBLANKS {
                    self.stalled = true;
                    break;
                }
                core::hint::spin_loop();
            }
        }

        // 2. Pop the oldest complete frame in strict FIFO sequence, decode, and upload to back buffer.
        let popped_slot = self.take_ready_slot();
        if let Some(slot) = popped_slot {
            self.slot_state[slot] = SlotState::Decoding;
            self.decode_and_upload(slot, renderer, storage);
            self.slot_state[slot] = SlotState::Free;
        }

        // 3. Pace to the flip target (15 fps = every 4 VBlanks), pumping the drive continuously.
        while (psx_rt::interrupts::vblank_count().wrapping_sub(self.next_flip_vblank) as i32) < 0 {
            self.pump(storage);
        }

        // 4. Flip on the VBlank boundary.
        let v0 = psx_rt::interrupts::vblank_count();
        while psx_rt::interrupts::vblank_count() == v0 {
            self.pump(storage);
        }

        if popped_slot.is_some() {
            renderer.fb.swap();
            self.last_swap_vblank = psx_rt::interrupts::vblank_count();
            self.frames_shown += 1;
        }
        // Schedule the next flip one period *minus one* before the target.
        // Waiting for the VBlank counter to increment consumed 1 VBlank, so
        // adding VBLANKS_PER_VIDEO_FRAME - 1 (3) flips exactly 4 VBlanks after this one.
        self.next_flip_vblank = psx_rt::interrupts::vblank_count().wrapping_add(
            VBLANKS_PER_VIDEO_FRAME - 1,
        );

        // 5. End of movie check: stream marked EOF (or stalled) and all queued frames presented.
        let drained = self.ready_len == 0 && popped_slot.is_none();
        if self.frames_shown > 0 && ((self.eof || self.stalled) && drained) {
            self.stop();
        }
    }

    /// Expand one frame's bitstream on the CPU, drive it through the MDEC,
    /// and upload each decoded column into the back buffer.
    fn decode_and_upload(
        &mut self,
        slot: usize,
        renderer: &crate::renderer::Renderer,
        storage: &mut VideoStorage,
    ) {
        // SAFETY: `slots[slot]` is a disjoint static holding SLOT_WORDS
        // words; the assembler last wrote it and nothing else aliases it.
        let frame: &[u8] = unsafe {
            let p = addr_of_mut!(storage.slots[slot]) as *const u8;
            core::slice::from_raw_parts(p, SLOT_WORDS * 4)
        };
        // SAFETY: `rle` is a disjoint static used only here.
        let rle16 = unsafe {
            core::slice::from_raw_parts_mut(addr_of_mut!(storage.rle) as *mut u16, RLE_WORDS * 2)
        };

        // Expand the bitstream into MDEC run-lengths, pumping the drive once
        // per macroblock column. `self` and `storage` are both borrowed
        // mutably by the callback, so go through raw pointers: the slot we
        // are reading from is disjoint from `sector`, which the pump writes.
        let selfp: *mut VideoPlayer = self;
        let storp: *mut VideoStorage = storage;
        let words = bs::decode_frame(
            frame,
            rle16,
            COLUMNS as u32 * ROWS,
            ROWS,
            &mut || {
                // SAFETY: single-threaded, and the two borrows are of
                // disjoint fields reached through raw pointers.
                unsafe {
                    let before = (*selfp).pumped_sectors;
                    (*selfp).pump(&mut *storp);
                    (*selfp).overlapped_sectors += (*selfp).pumped_sectors - before;
                }
            },
        );

        let words = match words {
            Ok(w) if w > 0 => w,
            _ => {
                self.decode_errors += 1;
                return;
            }
        };

        // SAFETY: `rle` stays alive and unmodified until decode_finish.
        unsafe {
            mdec::decode_start(
                core::slice::from_raw_parts(addr_of_mut!(storage.rle) as *const u32, words),
                words,
                mdec::DECODE_15BPP,
            );
        }

        let fb_y = renderer.fb.buffer_y(renderer.fb.drawing);
        let mut ok = true;
        for c in 0..COLUMNS {
            if !mdec::read_column(&mut storage.column) {
                ok = false;
                break;
            }
            // Channel 2, block mode: 8 words per row x 240 rows. A 16-pixel
            // column is word-aligned and within the GPU's 16-word block
            // limit, so this always takes the DMA path; the FIFO loop is a
            // safety net for hardware that will not.
            let rect = VramRect::new(c * 16, fb_y, 16, VIDEO_H);
            if self.vram_dma_ok && dma_copy_to_vram(rect, storage.column.as_ptr()) {
                self.vram_dma_columns += 1;
            } else {
                if self.vram_dma_ok {
                    self.vram_dma_ok = false;
                    self.vram_dma_fallbacks += 1;
                    psx_rt::tty::println("[VIDEO] VRAM DMA wedged; GP0 for the rest of this movie");
                }
                upload_words(rect, &storage.column);
            }
            // Pump between columns: the MDEC is DMA-driven, so the CPU is
            // idle here and the drive has been filling its FIFO.
            let before = self.pumped_sectors;
            self.pump(storage);
            self.overlapped_sectors += self.pumped_sectors - before;
        }

        // SAFETY: `rle` untouched since decode_start.
        let finished = mdec::decode_finish();
        if !finished || !ok {
            self.decode_errors += 1;
            // Reset so a bad frame cannot wedge the next one.
            mdec::reset();
            let _ = mdec::load_tables();
            dma::abort(dma::Channel::MdecIn);
            dma::abort(dma::Channel::MdecOut);
        }
    }

    pub fn stop(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;

        // Restore the IRQ mask unconditionally. `prepare_single_speed` masks
        // CD and timer interrupts for the duration of a stream, and this
        // guard used to be `if self.using_cd` -- so a stream that gave up
        // early left the SPU and CD handlers masked off for the rest of the
        // process, which the VBlank-driven frame loop depends on.
        if self.irq_mask_saved {
            // SAFETY: single-threaded, one reader.
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

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Whether a newly decoded frame is ready to be presented.
    pub fn needs_redraw(&self) -> bool {
        self.ready_len != 0
    }
}
