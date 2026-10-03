//! Minimal fixed-width number formatting for the on-screen summary.
//!
//! `no_std` has no `format!` and no allocator, so each screen line is
//! built into a stack buffer by hand. TTY output needs none of this: it
//! goes out as hex and the host parses it as integers.

/// A single line of text under construction, in a fixed buffer.
pub struct Line {
    buf: [u8; 64],
    len: usize,
}

impl Line {
    pub const fn new() -> Self {
        Self {
            buf: [0; 64],
            len: 0,
        }
    }

    pub fn clear(&mut self) -> &mut Self {
        self.len = 0;
        self
    }

    /// Append literal text.
    pub fn text(&mut self, s: &[u8]) -> &mut Self {
        for &b in s {
            if self.len < self.buf.len() {
                self.buf[self.len] = b;
                self.len += 1;
            }
        }
        self
    }

    /// Append a single character.
    pub fn ch(&mut self, c: u8) -> &mut Self {
        if self.len < self.buf.len() {
            self.buf[self.len] = c;
            self.len += 1;
        }
        self
    }

    /// Append `value` in decimal. `width` left-pads with spaces to keep
    /// columns aligned; 0 means "natural width".
    pub fn num(&mut self, value: u32, width: usize) -> &mut Self {
        let mut tmp = [0u8; 10];
        let mut n = 0usize;
        let mut v = value;
        if v == 0 {
            tmp[0] = b'0';
            n = 1;
        }
        while v > 0 {
            tmp[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
        }
        for _ in (n..width).rev() {
            self.ch(b' ');
        }
        while n > 0 {
            n -= 1;
            self.ch(tmp[n]);
        }
        self
    }

    /// Append `millis / 1000` with three fractional digits.
    pub fn fixed3(&mut self, millis: u32) -> &mut Self {
        self.num(millis / 1000, 0);
        self.ch(b'.');
        self.num(millis % 1000, 3)
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("?")
    }
}
