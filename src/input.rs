//! Polled PS/2 keyboard + mouse. No interrupts: the main loop drains the controller each frame.
use core::arch::asm;

pub fn inb(p: u16) -> u8 {
    let v: u8;
    unsafe { asm!("in al, dx", out("al") v, in("dx") p, options(nomem, nostack)) };
    v
}
pub fn outb(p: u16, v: u8) {
    unsafe { asm!("out dx, al", in("dx") p, in("al") v, options(nomem, nostack)) };
}

fn wait_w() {
    for _ in 0..100_000 {
        if inb(0x64) & 2 == 0 {
            return;
        }
    }
}
fn wait_r() {
    for _ in 0..100_000 {
        if inb(0x64) & 1 != 0 {
            return;
        }
    }
}
fn mouse_cmd(c: u8) {
    wait_w();
    outb(0x64, 0xd4);
    wait_w();
    outb(0x60, c);
    wait_r();
    inb(0x60); // ACK
}

pub fn init() {
    wait_w();
    outb(0x64, 0xa8); // enable aux port
    wait_w();
    outb(0x64, 0x20);
    wait_r();
    let cfg = inb(0x60) & !0x20; // clear "aux clock disabled"
    wait_w();
    outb(0x64, 0x60);
    wait_w();
    outb(0x60, cfg);
    mouse_cmd(0xf6); // defaults
    mouse_cmd(0xf4); // start streaming
}

pub enum Ev {
    Mouse { dx: i32, dy: i32, left: bool },
    Key(u8),
}

// scancode set 1, indices 0..58. ponytail: no arrows/F-keys/caps, add when a shell needs them
const LOWER: &[u8; 58] = b"\0\x1b1234567890-=\x08\0qwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";
const UPPER: &[u8; 58] = b"\0\x1b!@#$%^&*()_+\x08\0QWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";

pub struct Input {
    pkt: [u8; 3],
    n: usize,
    shift: bool,
}

impl Input {
    pub const fn new() -> Self {
        Input { pkt: [0; 3], n: 0, shift: false }
    }

    pub fn poll(&mut self) -> Option<Ev> {
        loop {
            let s = inb(0x64);
            if s & 1 == 0 {
                return None;
            }
            let d = inb(0x60);
            if s & 0x20 != 0 {
                if self.n == 0 && d & 8 == 0 {
                    continue; // out of sync, wait for a header byte
                }
                self.pkt[self.n] = d;
                self.n += 1;
                if self.n == 3 {
                    self.n = 0;
                    let b = self.pkt;
                    let h = b[0] as i32;
                    return Some(Ev::Mouse {
                        dx: b[1] as i32 - ((h << 4) & 0x100),
                        dy: b[2] as i32 - ((h << 3) & 0x100),
                        left: h & 1 != 0,
                    });
                }
            } else {
                match d {
                    0x2a | 0x36 => self.shift = true,
                    0xaa | 0xb6 => self.shift = false,
                    1..=57 => {
                        let c = if self.shift { UPPER } else { LOWER }[d as usize];
                        if c != 0 {
                            return Some(Ev::Key(c));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

pub fn inw(p: u16) -> u16 {
    let v: u16;
    unsafe { asm!("in ax, dx", out("ax") v, in("dx") p, options(nomem, nostack)) };
    v
}
pub fn outw(p: u16, v: u16) {
    unsafe { asm!("out dx, ax", in("dx") p, in("ax") v, options(nomem, nostack)) };
}
