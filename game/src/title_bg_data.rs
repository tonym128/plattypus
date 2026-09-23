pub fn title_bg_pixels() -> &'static [u16] {
    static BYTES: &[u8] = include_bytes!("../title_bg.bin");
    unsafe { core::slice::from_raw_parts(BYTES.as_ptr() as *const u16, 320 * 240) }
}
