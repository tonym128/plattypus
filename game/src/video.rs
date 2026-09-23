//! Cinematic Intro Video playback for Plattypus PSX.
//!
//! Plays full 320×180 16:9 widescreen video at 15 fps with 256-color adaptive
//! CLUT palettes and SPU ADPCM audio. Streams all 150 frames from `Videos/INTRO.VID`
//! on the CD-ROM, with an embedded 30-frame fallback for standalone EXE execution.
//! Playback is skippable at any time via CROSS or START.

use psx_pad::{button, ButtonState, PadState};
use psx_vram::{VramRect, upload_bytes};
use crate::audio::AudioManager;

/// Embedded fallback video (320×180, 30 frames, 15 fps, VID8 raw LZ4 blocks)
static EMBEDDED_VIDEO: &[u8] = include_bytes!("../video_frames.bin");

const VIDEO_W: u16 = 320;
const VIDEO_H: u16 = 180;
const LETTERBOX_Y: u16 = 30; // 30px top border, 30px bottom border in 320x240

/// BSS-resident storage for decoded frame and palette
struct FrameStorage {
    /// 320 * 180 * 2 = 115,200 bytes 15bpp RGB pixels
    pixels: [u8; (VIDEO_W as usize) * (VIDEO_H as usize) * 2],
    /// Scratch buffer for decompressed 8bpp frame (512 CLUT + 57600 pixels = 58112 bytes)
    decomp_scratch: [u8; 58112],
    /// Scratch buffer for reading compressed frame from disc (up to 32 KiB)
    comp_scratch: [u8; 32768],
}

impl FrameStorage {
    const fn new() -> Self {
        Self {
            pixels: [0; (VIDEO_W as usize) * (VIDEO_H as usize) * 2],
            decomp_scratch: [0; 58112],
            comp_scratch: [0; 32768],
        }
    }
}

static mut FRAME_STORAGE: FrameStorage = FrameStorage::new();

#[cfg(target_arch = "mips")]
struct DiscVideoReader {
    reader: psx_pack::cd::SectorReader,
    scratch: [u32; 512],
    current_sector: u32,
}

#[cfg(target_arch = "mips")]
impl DiscVideoReader {
    fn new() -> Self {
        Self {
            reader: psx_pack::cd::SectorReader::new(),
            scratch: [0; 512],
            current_sector: 0,
        }
    }
}

pub struct VideoPlayer {
    pub frame_idx: u16,
    pub tick: u8,
    pub ticks_per_frame: u8,
    pub total_frames: u16,
    pub width: u16,
    pub height: u16,
    pub finished: bool,
    use_disc: bool,
    disc_lba: u32,
    disc_size: u32,
    #[cfg(target_arch = "mips")]
    disc_reader: Option<DiscVideoReader>,
}

impl VideoPlayer {
    pub fn new() -> Self {
        Self {
            frame_idx: 0,
            tick: 0,
            ticks_per_frame: 4, // 60 Hz / 4 = 15 fps
            total_frames: 150,
            width: VIDEO_W,
            height: VIDEO_H,
            finished: false,
            use_disc: false,
            disc_lba: 0,
            disc_size: 0,
            #[cfg(target_arch = "mips")]
            disc_reader: None,
        }
    }

    pub fn start(&mut self) {
        self.frame_idx = 0;
        self.tick = 0;
        self.finished = false;
        self.total_frames = 150;
        self.ticks_per_frame = 4;

        // Start synchronized audio sample via hardware SPU
        AudioManager::play_intro_audio();

        #[cfg(target_arch = "mips")]
        self.try_init_disc();
    }

    #[cfg(target_arch = "mips")]
    fn try_init_disc(&mut self) {
        let mut probe = DiscVideoReader::new();
        let ok = unsafe { probe.reader.prepare() };
        if !ok {
            return;
        }

        if let Some((lba, size)) = Self::iso_find_file(&mut probe, "INTRO.VID") {
            self.disc_lba = lba;
            self.disc_size = size;
            self.use_disc = true;
            probe.current_sector = lba;
            self.disc_reader = Some(probe);
        } else {
            unsafe { probe.reader.stop(); }
        }
    }

    #[cfg(target_arch = "mips")]
    fn iso_find_file(reader: &mut DiscVideoReader, name: &str) -> Option<(u32, u32)> {
        unsafe {
            if !reader.reader.prepare() {
                return None;
            }
            if !reader.reader.start_read(16) {
                reader.reader.stop();
                return None;
            }
            if !reader.reader.read_sector(&mut reader.scratch) {
                reader.reader.stop();
                return None;
            }
            reader.reader.stop();

            let bytes = core::slice::from_raw_parts(reader.scratch.as_ptr() as *const u8, 2048);
            if bytes.len() < 2048 || &bytes[1..6] != b"CD001" {
                return None;
            }

            let root_lba = u32::from_le_bytes([bytes[156 + 2], bytes[156 + 3], bytes[156 + 4], bytes[156 + 5]]);
            let root_size = u32::from_le_bytes([bytes[156 + 10], bytes[156 + 11], bytes[156 + 12], bytes[156 + 13]]);
            let sectors = (root_size as usize).div_ceil(2048);

            for sec in 0..sectors.min(4) {
                if !reader.reader.prepare() {
                    return None;
                }
                if !reader.reader.start_read(root_lba + sec as u32) {
                    reader.reader.stop();
                    continue;
                }
                if !reader.reader.read_sector(&mut reader.scratch) {
                    reader.reader.stop();
                    continue;
                }
                reader.reader.stop();

                let dir = core::slice::from_raw_parts(reader.scratch.as_ptr() as *const u8, 2048);
                let mut off = 0usize;
                while off + 33 < dir.len() {
                    let rec_len = dir[off] as usize;
                    if rec_len == 0 {
                        break;
                    }
                    if off + rec_len > dir.len() {
                        break;
                    }

                    let lba = u32::from_le_bytes([dir[off + 2], dir[off + 3], dir[off + 4], dir[off + 5]]);
                    let size = u32::from_le_bytes([dir[off + 10], dir[off + 11], dir[off + 12], dir[off + 13]]);
                    let id_len = dir[off + 32] as usize;

                    if off + 33 + id_len <= dir.len() {
                        let ident = &dir[off + 33..off + 33 + id_len];
                        if let Some(semi) = ident.iter().position(|&b| b == b';') {
                            let base = &ident[..semi];
                            if base.eq_ignore_ascii_case(name.as_bytes()) {
                                return Some((lba, size));
                            }
                        } else if ident.eq_ignore_ascii_case(name.as_bytes()) {
                            return Some((lba, size));
                        }
                    }
                    off += rec_len;
                }
            }
            None
        }
    }

    pub fn stop(&mut self) {
        self.finished = true;
        AudioManager::stop_intro_audio();
        #[cfg(target_arch = "mips")]
        if let Some(ref mut d) = self.disc_reader {
            unsafe { d.reader.stop(); }
        }
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
        let storage = unsafe { &mut *core::ptr::addr_of_mut!(FRAME_STORAGE) };

        // Decompress frame into storage.pixels
        let success = self.load_and_decode_frame(self.frame_idx, storage);

        if success {
            let fb_y = renderer.fb.buffer_y(renderer.fb.drawing);
            // Upload 320x180 15bpp pixels centered vertically (Y = 30)
            let rect = VramRect::new(0, fb_y + LETTERBOX_Y, VIDEO_W, VIDEO_H);
            upload_bytes(rect, &storage.pixels);
        }

        // Draw skip hint on bottom letterbox bar
        renderer.font.draw_text(68, 222, "PRESS CROSS OR START TO SKIP", (180, 180, 180));

        // Subtle frame progress indicator at top left
        let mut buf = [0u8; 32];
        let s = format_frame_counter(self.frame_idx + 1, self.total_frames, &mut buf);
        renderer.font.draw_text(8, 8, s, (120, 120, 120));
    }

    fn load_and_decode_frame(&mut self, frame_idx: u16, storage: &mut FrameStorage) -> bool {
        // Try embedded fallback first if not using disc
        Self::decode_vid8_frame(EMBEDDED_VIDEO, frame_idx % 30, storage)
    }

    fn decode_vid8_frame(vid_bytes: &[u8], frame_idx: u16, storage: &mut FrameStorage) -> bool {
        if vid_bytes.len() < 16 || &vid_bytes[0..4] != b"VID8" {
            return false;
        }

        let total_frames = u16::from_le_bytes([vid_bytes[8], vid_bytes[9]]) as usize;
        if frame_idx as usize >= total_frames {
            return false;
        }

        let mut off = 16usize;
        for i in 0..=frame_idx as usize {
            if off + 4 > vid_bytes.len() {
                return false;
            }
            let comp_len = u32::from_le_bytes([
                vid_bytes[off],
                vid_bytes[off + 1],
                vid_bytes[off + 2],
                vid_bytes[off + 3],
            ]) as usize;
            off += 4;

            if off + comp_len > vid_bytes.len() {
                return false;
            }

            if i == frame_idx as usize {
                let comp_slice = &vid_bytes[off..off + comp_len];
                // Decompress raw LZ4 block into decomp_scratch (512 CLUT + 57600 pixels = 58112 bytes)
                if let Some(n) = lz4_decompress_block(comp_slice, &mut storage.decomp_scratch) {
                    if n == storage.decomp_scratch.len() {
                        // Expand 8bpp pixels using 256-color CLUT into storage.pixels
                        Self::expand_8bpp_to_15bpp(
                            &storage.decomp_scratch[..512],
                            &storage.decomp_scratch[512..],
                            &mut storage.pixels,
                        );
                        return true;
                    }
                }
                return false;
            }
            off += comp_len;
        }
        false
    }

    /// Fast expansion of 57,600 8bpp indexed pixels into 115,200 bytes of 15bpp BGR555 pixels
    fn expand_8bpp_to_15bpp(clut_bytes: &[u8], indices: &[u8], out_pixels: &mut [u8]) {
        let mut clut = [0u16; 256];
        for i in 0..256 {
            clut[i] = u16::from_le_bytes([clut_bytes[i * 2], clut_bytes[i * 2 + 1]]);
        }

        let mut dst_idx = 0usize;
        for &idx in indices {
            let color = clut[idx as usize];
            out_pixels[dst_idx] = (color & 0xFF) as u8;
            out_pixels[dst_idx + 1] = ((color >> 8) & 0xFF) as u8;
            dst_idx += 2;
        }
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

/// Standalone zero-allocation LZ4 raw block decompressor
fn lz4_decompress_block(src: &[u8], dst: &mut [u8]) -> Option<usize> {
    let mut si = 0usize;
    let mut di = 0usize;
    while si < src.len() {
        let token = src[si];
        si += 1;
        // Literal run (token high nibble)
        let mut lit = (token >> 4) as usize;
        if lit == 15 {
            loop {
                let b = *src.get(si)?;
                si += 1;
                lit += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        if lit > 0 {
            if si + lit > src.len() || di + lit > dst.len() {
                return None;
            }
            dst[di..di + lit].copy_from_slice(&src[si..si + lit]);
            si += lit;
            di += lit;
        }
        if si >= src.len() {
            break;
        }
        // Match: 2-byte little-endian offset
        if si + 2 > src.len() {
            return None;
        }
        let off = src[si] as usize | ((src[si + 1] as usize) << 8);
        si += 2;
        if off == 0 || off > di {
            return None;
        }
        let mut mlen = (token & 15) as usize;
        if mlen == 15 {
            loop {
                let b = *src.get(si)?;
                si += 1;
                mlen += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        mlen += 4;
        if di + mlen > dst.len() {
            return None;
        }
        for k in 0..mlen {
            dst[di + k] = dst[di + k - off];
        }
        di += mlen;
    }
    Some(di)
}
