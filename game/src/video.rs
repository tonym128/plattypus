//! Cinematic Intro Video playback for Plattypus PSX.
//!
//! Decodes 320x240 video at 15 fps using the PS1 hardware MDEC coprocessor
//! (MDEC0 / DMA channel 0 & channel 1) with synchronized SPU ADPCM audio.
//! Reads four adjacent 8-sector frames at a time from CD-ROM via SectorReader,
//! with seamless fallback to embedded ROM frames.
//! Playback is skippable at any time via CROSS or START.

use psx_pad::{button, ButtonState, PadState};
use psx_vram::{VramRect, upload_words};
use psx_pack::cd::{SectorReader, SECTOR_WORDS};
use crate::audio::AudioManager;

/// Embedded fallback video (320x240, 16 frames, 15 fps, raw MDEC hardware bitstream)
static EMBEDDED_VIDEO: &[u8] = include_bytes!("../video_mdec.bin");

pub const VIDEO_W: u16 = 320;
pub const VIDEO_H: u16 = 240;
pub const TOTAL_FRAMES: u16 = 150;
pub const EMBEDDED_FRAME_COUNT: u16 = 16;
pub const SECTORS_PER_FRAME: usize = 8;
pub const WORDS_PER_FRAME: usize = 4096; // 16,384 bytes = 8 sectors
pub const WORDS_PER_SLICE: usize = 1920; // 16 * 240 / 2 = 1,920 u32 words
// Keep several compressed frames in RAM so CD reads can be done as one
// sequential burst instead of seeking once for every frame.
const PREFETCH_FRAMES: usize = 4;

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

/// BSS-resident storage for MDEC bitstream, slice buffer, and CD reading
struct VideoStorage {
    /// 4,096 u32 words (16 KiB) holding the active frame payload
    frame_words: [u32; WORDS_PER_FRAME],
    /// Read-ahead batch: 64 KiB of compressed video, four frames at a time.
    frame_cache: [[u32; WORDS_PER_FRAME]; PREFETCH_FRAMES],
    /// 1,920 u32 words (7.68 KiB) holding one decoded 16x240 slice
    slice_words: [u32; WORDS_PER_SLICE],
    /// Bounce buffer for single-sector reads (e.g. root directory scan)
    sector_buf: [u32; SECTOR_WORDS],
}

impl VideoStorage {
    const fn new() -> Self {
        Self {
            frame_words: [0; WORDS_PER_FRAME],
            frame_cache: [[0; WORDS_PER_FRAME]; PREFETCH_FRAMES],
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
    saved_irq_mask: u32,
    /// Whether `saved_irq_mask` holds a mask that still needs restoring.
    irq_mask_saved: bool,
    /// Failed prefetches since the last successful one.
    cd_read_retries: u8,
    cache_start_frame: u16,
    cached_frames: u8,
    cd_reader: SectorReader,
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
            saved_irq_mask: 0,
            irq_mask_saved: false,
            cd_read_retries: 0,
            cache_start_frame: 0xFFFF,
            cached_frames: 0,
            cd_reader: SectorReader::new(),
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

    /// Read a small run of adjacent frames in one CD command. The drive is
    /// paused at the end of the burst, so decoding cannot let its sector FIFO
    /// overflow while the next cached frames are waiting in RAM.
    fn prefetch_cd_batch(&mut self, first_frame: u16, storage: &mut VideoStorage) -> bool {
        if !self.using_cd || first_frame >= self.total_frames {
            return false;
        }

        let batch_start = (first_frame / PREFETCH_FRAMES as u16) * PREFETCH_FRAMES as u16;
        let frame_count = core::cmp::min(
            PREFETCH_FRAMES,
            (self.total_frames - batch_start) as usize,
        );
        let lba = self.cd_start_lba
            .wrapping_add(batch_start as u32 * SECTORS_PER_FRAME as u32);
        if !unsafe { self.cd_reader.start_read(lba) } {
            psx_rt::tty::println("[VIDEO] CD batch ReadN start failed");
            self.cached_frames = 0;
            return false;
        }

        let mut ok = true;
        'frames: for frame in 0..frame_count {
            for sector in 0..SECTORS_PER_FRAME {
                let offset = sector * SECTOR_WORDS;
                let sector_buf: &mut [u32; SECTOR_WORDS] = unsafe {
                    &mut *(&mut storage.frame_cache[frame][offset..offset + SECTOR_WORDS]
                        as *mut [u32] as *mut [u32; SECTOR_WORDS])
                };
                if !unsafe { self.cd_reader.read_sector(sector_buf) } {
                    psx_rt::tty::print("[VIDEO] CD batch read failed at frame ");
                    psx_rt::tty::print_hex_u32(frame as u32);
                    psx_rt::tty::print(" sector ");
                    psx_rt::tty::print_hex_u32(sector as u32);
                    psx_rt::tty::print("\n");
                    ok = false;
                    break 'frames;
                }
            }
        }

        // Stop ReadN after the batch. This avoids missed sector IRQs during
        // MDEC decoding; the next batch resumes sequentially after one seek.
        unsafe { self.cd_reader.stop(); }

        if ok {
            self.cache_start_frame = batch_start;
            self.cached_frames = frame_count as u8;
        } else {
            self.cached_frames = 0;
        }
        ok
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
        self.ticks_per_frame = 4; // 60 Hz / 4 = 15 fps
        self.last_decoded_frame = 0xFFFF;
        self.using_cd = false;
        self.cache_start_frame = 0xFFFF;
        self.cached_frames = 0;
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
        // Read several adjacent frames per CD command to amortize seek and
        // command overhead, while keeping the drive paused during MDEC work.
        let prepared = unsafe { self.cd_reader.prepare() };
        if prepared {
            psx_rt::tty::println("[VIDEO] CD reader prepare OK");
            if let Some(lba) = unsafe { Self::find_vid_lba(filename, &mut self.cd_reader, storage) } {
                self.cd_start_lba = lba;
                // ReadN bursts fill the four-frame cache; the reader pauses
                // between batches so MDEC work cannot overrun the CD FIFO.
                psx_rt::tty::println("[VIDEO] Target video located; using four-frame read-ahead");
                self.using_cd = true;
                let _ = self.prefetch_cd_batch(0, storage);
            }
        } else {
            psx_rt::tty::println("[VIDEO] CD reader prepare FAILED");
        }

        // Start audio after the initial read-ahead burst so its first frame
        // remains synchronized with the first displayed video frame.
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

        self.tick = self.tick.wrapping_add(1);
        if self.tick >= self.ticks_per_frame {
            self.tick = 0;
            self.frame_idx += 1;
            if self.frame_idx >= self.total_frames {
                self.stop();
                return true;
            }
        }

        false
    }

    pub fn draw(&mut self, renderer: &crate::renderer::Renderer) {
        let storage = unsafe { &mut *core::ptr::addr_of_mut!(STORAGE) };
        let frame_to_show = self.frame_idx;

        // Decode new frame when index changes
        if frame_to_show != self.last_decoded_frame {
            let mut read_ok = false;

            if self.using_cd {
                let cache_end = self.cache_start_frame.saturating_add(self.cached_frames as u16);
                if frame_to_show < self.cache_start_frame || frame_to_show >= cache_end {
                    if !self.prefetch_cd_batch(frame_to_show, storage) {
                        // One dropped sector used to latch streaming off for
                        // the whole cutscene, collapsing a 150-frame intro into
                        // a 16-frame embedded loop with the 10-second voiceover
                        // still playing against it. Re-seek once from the top
                        // before giving up: a transient read error is the
                        // common case, not a broken file.
                        self.cd_read_retries = self.cd_read_retries.saturating_add(1);
                        if self.cd_read_retries <= CD_READ_RETRIES {
                            self.cache_start_frame = 0xFFFF;
                            self.cached_frames = 0;
                            if !self.prefetch_cd_batch(frame_to_show, storage) {
                                self.using_cd = false;
                            }
                        } else {
                            self.using_cd = false;
                        }
                    } else {
                        self.cd_read_retries = 0;
                    }
                }

                if self.using_cd
                    && frame_to_show >= self.cache_start_frame
                    && frame_to_show < self.cache_start_frame.saturating_add(self.cached_frames as u16)
                {
                    let cache_slot = (frame_to_show - self.cache_start_frame) as usize;
                    storage.frame_words.copy_from_slice(&storage.frame_cache[cache_slot]);
                    read_ok = true;
                }
            }

            if !read_ok {
                // Load from embedded fallback frames. Only meaningful if the
                // blob actually matches the decode geometry.
                if !Self::embedded_fallback_is_consistent() {
                    psx_rt::tty::println("[VIDEO] embedded fallback size mismatch; cannot decode");
                    return;
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
                    upload_words(rect, &storage.slice_words);
                }

                if frame_to_show < 3 {
                    psx_rt::tty::println("[VIDEO] Frame upload complete.");
                }

                self.last_decoded_frame = frame_to_show;
            }
        }

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
