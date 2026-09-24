//! Cinematic Intro Video playback for Plattypus PSX.
//!
//! Decodes 320x240 video at 15 fps using the PS1 hardware MDEC coprocessor
//! (MDEC0 / DMA channel 0 & channel 1) with synchronized SPU ADPCM audio.
//! Streams seek-free 8-sector (16 KiB) frames directly from CD-ROM via
//! SectorReader, with seamless fallback to embedded ROM frames.
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

/// BSS-resident storage for MDEC bitstream, slice buffer, and CD reading
struct VideoStorage {
    /// 4,096 u32 words (16 KiB) holding the active frame payload
    frame_words: [u32; WORDS_PER_FRAME],
    /// 1,920 u32 words (7.68 KiB) holding one decoded 16x240 slice
    slice_words: [u32; WORDS_PER_SLICE],
    /// Bounce buffer for single-sector reads (e.g. root directory scan)
    sector_buf: [u32; SECTOR_WORDS],
}

impl VideoStorage {
    const fn new() -> Self {
        Self {
            frame_words: [0; WORDS_PER_FRAME],
            slice_words: [0; WORDS_PER_SLICE],
            sector_buf: [0; SECTOR_WORDS],
        }
    }
}

static mut STORAGE: VideoStorage = VideoStorage::new();

pub struct VideoPlayer {
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
    last_uploaded_frame: u16,
    last_uploaded_buffer: u8,
    cd_reader: SectorReader,
}

impl VideoPlayer {
    pub fn new() -> Self {
        Self {
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
            last_uploaded_frame: 0xFFFF,
            last_uploaded_buffer: 0xFF,
            cd_reader: SectorReader::new(),
        }
    }

    /// Locate INTRO.VID on disc by scanning the ISO 9660 root directory (Sector 20).
    unsafe fn find_intro_vid_lba(reader: &mut SectorReader, storage: &mut VideoStorage) -> Option<u32> {
        psx_rt::tty::println("[VIDEO] Scanning root directory (Sector 20)...");
        // Sector 20 is the ISO 9660 root directory extent
        if !unsafe { reader.start_read_seek_first(20, 500_000) } {
            psx_rt::tty::println("[VIDEO] start_read_seek_first(20) FAILED");
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
            if off + record_len > bytes.len() {
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
                if name.starts_with(b"INTRO.VID") {
                    psx_rt::tty::print("[VIDEO] Found INTRO.VID at LBA: ");
                    psx_rt::tty::print_hex_u32(lba);
                    psx_rt::tty::print("\n");
                    return Some(lba);
                }
            }
            off += record_len;
        }
        psx_rt::tty::println("[VIDEO] INTRO.VID not found in Sector 20");
        None
    }

    pub fn start(&mut self) {
        psx_rt::tty::println("[VIDEO] start() called");
        self.frame_idx = 0;
        self.tick = 0;
        self.finished = false;
        self.total_frames = TOTAL_FRAMES;
        self.ticks_per_frame = 4;
        self.last_decoded_frame = 0xFFFF;
        self.last_uploaded_frame = 0xFFFF;
        self.last_uploaded_buffer = 0xFF;
        self.using_cd = false;

        // Reset and initialize hardware MDEC coprocessor with standard tables
        psx_rt::tty::println("[VIDEO] Initializing MDEC...");
        psx_io::mdec::init();
        psx_rt::tty::print("[VIDEO] MDEC init done. Stat: ");
        psx_rt::tty::print_hex_u32(psx_io::mdec::read_stat());
        psx_rt::tty::print("\n");

        // Start synchronized ADPCM audio sample via SPU
        AudioManager::play_intro_audio();

        // Initialize CD streaming if possible
        let storage = unsafe { &mut *core::ptr::addr_of_mut!(STORAGE) };
        let prepared = unsafe { self.cd_reader.prepare() };
        if prepared {
            psx_rt::tty::println("[VIDEO] CD reader prepare OK");
            if let Some(lba) = unsafe { Self::find_intro_vid_lba(&mut self.cd_reader, storage) } {
                self.cd_start_lba = lba;
                if unsafe { self.cd_reader.start_read_seek_first(lba, 1_000_000) } {
                    psx_rt::tty::println("[VIDEO] CD start_read_seek_first OK! using_cd = true");
                    self.using_cd = true;
                } else {
                    psx_rt::tty::println("[VIDEO] CD start_read_seek_first FAILED");
                }
            }
        } else {
            psx_rt::tty::println("[VIDEO] CD reader prepare FAILED");
        }
    }

    pub fn stop(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;

        if self.using_cd {
            unsafe {
                self.cd_reader.stop();
                // Restore standard VBlank IRQ mask
                psx_io::irq::set_mask(1 << psx_io::irq::source::VBLANK);
            }
            self.using_cd = false;
        }

        AudioManager::stop_intro_audio();
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
                // Stream 8 sectors (16 KiB) directly from the running ReadN stream
                let mut all_sectors_read = true;
                for sec in 0..SECTORS_PER_FRAME {
                    let off = sec * SECTOR_WORDS;
                    let sec_buf: &mut [u32; SECTOR_WORDS] = unsafe {
                        &mut *(&mut storage.frame_words[off..off + SECTOR_WORDS] as *mut [u32] as *mut [u32; SECTOR_WORDS])
                    };
                    if !unsafe { self.cd_reader.read_sector(sec_buf) } {
                        psx_rt::tty::print("[VIDEO] CD read_sector FAILED at sec ");
                        psx_rt::tty::print_hex_u32(sec as u32);
                        psx_rt::tty::print("\n");
                        all_sectors_read = false;
                        break;
                    }
                }
                if all_sectors_read {
                    read_ok = true;
                } else {
                    // Fall back to embedded ROM frames on CD error
                    psx_rt::tty::println("[VIDEO] Falling back to embedded ROM frames");
                    self.using_cd = false;
                }
            }

            if !read_ok {
                // Load from embedded fallback frames
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
                        // MDEC has no more output instead of burning seconds
                        // spinning on an incomplete frame.
                        let mut output_words = 0usize;
                        for out in storage.slice_words.iter_mut() {
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

        // Draw skip hint at bottom of screen
        renderer.font.draw_text(68, 222, "PRESS CROSS OR START TO SKIP", (180, 180, 180));

        // Frame progress indicator at top left
        let mut buf = [0u8; 32];
        let s = format_frame_counter(self.frame_idx + 1, self.total_frames, &mut buf);
        renderer.font.draw_text(8, 8, s, (120, 120, 120));
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

fn format_frame_counter(frame: u16, total: u16, buf: &mut [u8; 32]) -> &str {
    let mut out = [b' '; 32];
    out[..6].copy_from_slice(b"FRAME ");
    let mut idx = 6;
    let f = frame as u32;
    let t = total as u32;
    out[idx] = b'0' + ((f / 100) % 10) as u8;
    out[idx + 1] = b'0' + ((f / 10) % 10) as u8;
    out[idx + 2] = b'0' + (f % 10) as u8;
    idx += 3;
    out[idx] = b'/';
    idx += 1;
    out[idx] = b'0' + ((t / 100) % 10) as u8;
    out[idx + 1] = b'0' + ((t / 10) % 10) as u8;
    out[idx + 2] = b'0' + (t % 10) as u8;
    idx += 3;

    let len = idx;
    for i in 0..len {
        buf[i] = out[i];
    }
    unsafe { core::str::from_utf8_unchecked(&buf[..len]) }
}
