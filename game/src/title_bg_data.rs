//! Title-screen artwork: a raw 320x240 15-bit direct-colour picture.
//!
//! `title_bg.bin` holds one BGR555 halfword per texel (mask bit clear) in
//! row-major order -- 320 * 240 texels, two bytes each, which is exactly the
//! layout a 15bpp framebuffer half is read with, so the file uploads as-is.

/// Picture width in texels.
pub const TITLE_BG_WIDTH: u16 = 320;
/// Picture height in texels.
pub const TITLE_BG_HEIGHT: u16 = 240;

const TITLE_BG_BYTES: usize = include_bytes!("../title_bg.bin").len();

const _: () = assert!(
    TITLE_BG_BYTES == (TITLE_BG_WIDTH as usize) * (TITLE_BG_HEIGHT as usize) * 2,
    "title_bg.bin must hold exactly 320x240 15-bit texels (2 bytes each)"
);

pub fn title_bg_pixels() -> &'static [u16] {
    static BYTES: &[u8] = include_bytes!("../title_bg.bin");
    // SAFETY: `include_bytes!` gives an aligned, `'static` byte slice, and the
    // const assertion above pins its length to `TITLE_BG_WIDTH * HEIGHT`
    // texels, so the u16 view stays inside the allocation and is in bounds.
    unsafe { core::slice::from_raw_parts(BYTES.as_ptr() as *const u16, TITLE_BG_BYTES / 2) }
}
