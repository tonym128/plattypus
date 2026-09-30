//! Title-screen background upload.
//!
//! The picture goes straight into the framebuffer instead of being sampled as
//! a texture page, because a 15bpp page cannot hold a 320-texel-wide row at
//! all. A 15bpp texel is one 16-bit VRAM halfword (bit 15 is the mask bit --
//! the same layout `video.rs` writes its 16x240 MDEC slices in), so a page is
//! 256 texels of data per 256-halfword row stride: a contiguous 256x256 block.
//! A 320-texel image needs 320 halfwords per row, which is *wider than the
//! stride*, so consecutive image rows would alias each other, and a second
//! page 256 halfwords along reads the wrong row rather than the next 64
//! columns. No arrangement of 15bpp pages can hold this picture.
//!
//! The framebuffer has the image's exact shape: 320 pixels wide, read by the
//! display unit as 320 halfwords per row, and the art is already one 15-bit
//! halfword per texel, so it uploads as a single image transfer and the menu
//! is drawn on top of it.

use crate::title_bg_data::{TITLE_BG_HEIGHT, TITLE_BG_WIDTH};
use psx_vram::{upload_16bpp, VramRect};

/// VRAM X of both framebuffer halves (`FrameBuffer::new(320, 240)`).
const FB_X: u16 = 0;
/// VRAM Y of each framebuffer half; the stride is the buffer height.
const FB_Y: [u16; 2] = [0, TITLE_BG_HEIGHT];

/// The picture is exactly one 15-bit halfword per texel, so it covers a
/// framebuffer half texel for texel -- no packing, no padding.
const _: () = assert!(TITLE_BG_WIDTH == 320 && TITLE_BG_HEIGHT == 240);

/// Write the title background into one framebuffer half, whose first VRAM row
/// is `fb_y`.
pub unsafe fn upload_title_bg_to(fb_y: u16) {
    upload_16bpp(
        VramRect::new(FB_X, fb_y, TITLE_BG_WIDTH, TITLE_BG_HEIGHT),
        crate::title_bg_data::title_bg_pixels(),
    );
}

/// Boot-time fill of both framebuffer halves, so the very first title frame
/// already has the picture behind the menu.
pub unsafe fn upload_title_bg() {
    upload_title_bg_to(FB_Y[0]);
    upload_title_bg_to(FB_Y[1]);
}
