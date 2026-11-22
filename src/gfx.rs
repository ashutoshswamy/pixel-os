//! Mode 13h (320x200, 256 colors). Draw into a back buffer, copy on vblank.
use crate::input::{inb, outb};
use core::ptr::addr_of_mut;
use font8x8::legacy::BASIC_LEGACY;

pub const W: i32 = 320;
pub const H: i32 = 200;

// Palette indices. One accent (ACCENT), everything else neutral. No pure black/white.
pub const INK: u8 = 0;
pub const BG_A: u8 = 1;
pub const BG_B: u8 = 2;
pub const PAPER: u8 = 3;
pub const PANEL: u8 = 4;
pub const SHADOW: u8 = 5;
pub const ACCENT: u8 = 6;
pub const MUTED: u8 = 7;
pub const DIM: u8 = 8;

const PALETTE: [(u8, u8, u8); 9] = [
    (0x1b, 0x1f, 0x27), // INK
    (0x2b, 0x3a, 0x44), // BG_A  cool slate
    (0x31, 0x42, 0x4d), // BG_B
    (0xee, 0xec, 0xe6), // PAPER
    (0xd5, 0xd3, 0xcc), // PANEL
    (0x14, 0x1a, 0x20), // SHADOW (tinted to bg)
    (0xff, 0x6a, 0x3d), // ACCENT burnt orange
    (0x56, 0x60, 0x69), // MUTED  (5.6:1 on PAPER)
    (0xb4, 0xb2, 0xab), // DIM    inactive title
];

static mut BUF: [u8; 64000] = [0; 64000];

pub struct Gfx {
    buf: &'static mut [u8; 64000],
}

impl Gfx {
    pub fn new() -> Self {
        outb(0x3c8, 0);
        for (r, g, b) in PALETTE {
            outb(0x3c9, r >> 2); // DAC is 6-bit
            outb(0x3c9, g >> 2);
            outb(0x3c9, b >> 2);
        }
        Gfx { buf: unsafe { &mut *addr_of_mut!(BUF) } }
    }

    pub fn px(&mut self, x: i32, y: i32, c: u8) {
        if (0..W).contains(&x) && (0..H).contains(&y) {
            self.buf[(y * W + x) as usize] = c;
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8) {
        for j in y..y + h {
            for i in x..x + w {
                self.px(i, j, c);
            }
        }
    }

    pub fn glyph(&mut self, x: i32, y: i32, ch: u8, c: u8) {
        let g = BASIC_LEGACY[(ch & 0x7f) as usize];
        for (r, row) in g.iter().enumerate() {
            for b in 0..8 {
                if row >> b & 1 == 1 {
                    self.px(x + b, y + r as i32, c);
                }
            }
        }
    }

    pub fn text(&mut self, x: i32, y: i32, s: &str, c: u8) {
        for (i, b) in s.bytes().enumerate() {
            self.glyph(x + 8 * i as i32, y, b, c);
        }
    }

    pub fn present(&self) {
        while inb(0x3da) & 8 != 0 {}
        while inb(0x3da) & 8 == 0 {}
        unsafe { core::ptr::copy_nonoverlapping(self.buf.as_ptr(), 0xa0000 as *mut u8, 64000) };
    }
}

pub fn fill_screen(c: u8) {
    for i in 0..64000 {
        unsafe { (0xa0000 as *mut u8).add(i).write_volatile(c) };
    }
}
