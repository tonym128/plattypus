use psx_vram::{upload_16bpp, Tpage, TexDepth, VramRect};

pub const TITLE_BG_VRAM_X: u16 = 640;
pub const TITLE_BG_VRAM_Y: u16 = 0;
pub const TITLE_BG_WIDTH: u16 = 320;
pub const TITLE_BG_HEIGHT: u16 = 240;

pub unsafe fn upload_title_bg() {
    let pixels = crate::title_bg_data::title_bg_pixels();
    
    let rect = VramRect::new(TITLE_BG_VRAM_X, TITLE_BG_VRAM_Y, TITLE_BG_WIDTH, TITLE_BG_HEIGHT);
    upload_16bpp(rect, pixels);
}

pub fn title_bg_tpage() -> u16 {
    Tpage::new(TITLE_BG_VRAM_X, TITLE_BG_VRAM_Y, TexDepth::Bit15).uv_tpage_word(0)
}