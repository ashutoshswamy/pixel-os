//! Flat filesystem on the 2nd IDE disk (ATA PIO, LBA28).
//! Sector 0 = directory (16 bytes/entry: name[12], len u16, used u8). Sector 1+i = file i.
//! ponytail: fixed slots, 512 B max per file; swap for FAT/extents if files must grow.
use crate::input::{inb, inw, outb, outw};

pub const MAX_FILES: usize = 6;
pub const NAME: usize = 12;
pub const FILE_MAX: usize = 512;

fn ready() -> bool {
    (0..200_000).any(|_| inb(0x1f7) & 0xc0 == 0x40) // DRDY set, BSY clear. 0xff (no disk) fails.
}
fn drq() -> bool {
    for _ in 0..200_000 {
        let s = inb(0x1f7);
        if s & 1 != 0 {
            return false;
        }
        if s & 8 != 0 {
            return true;
        }
    }
    false
}
fn start(lba: u32, cmd: u8) -> bool {
    outb(0x1f6, 0xf0 | ((lba >> 24) & 0xf) as u8); // LBA + slave
    if !ready() {
        return false;
    }
    outb(0x1f2, 1);
    outb(0x1f3, lba as u8);
    outb(0x1f4, (lba >> 8) as u8);
    outb(0x1f5, (lba >> 16) as u8);
    outb(0x1f7, cmd);
    drq()
}
fn read_sector(lba: u32, buf: &mut [u8; 512]) -> bool {
    if !start(lba, 0x20) {
        return false;
    }
    for i in 0..256 {
        let w = inw(0x1f0);
        buf[2 * i] = w as u8;
        buf[2 * i + 1] = (w >> 8) as u8;
    }
    true
}
fn write_sector(lba: u32, buf: &[u8; 512]) -> bool {
    if !start(lba, 0x30) {
        return false;
    }
    for i in 0..256 {
        outw(0x1f0, buf[2 * i] as u16 | (buf[2 * i + 1] as u16) << 8);
    }
    outb(0x1f7, 0xe7); // flush cache
    ready()
}

#[derive(Clone, Copy)]
pub struct Ent {
    pub name: [u8; NAME],
    pub len: u16,
    pub used: bool,
}

pub fn ename(n: &[u8; NAME]) -> &str {
    let l = n.iter().position(|&c| c == 0).unwrap_or(NAME);
    core::str::from_utf8(&n[..l]).unwrap_or("?")
}

pub struct Fs {
    pub dir: [Ent; MAX_FILES],
    pub disk: bool,
}

impl Fs {
    pub fn new() -> Fs {
        let mut fs = Fs { dir: [Ent { name: [0; NAME], len: 0, used: false }; MAX_FILES], disk: false };
        let mut s = [0u8; 512];
        if read_sector(0, &mut s) {
            fs.disk = true;
            for i in 0..MAX_FILES {
                let e = &s[i * 16..i * 16 + 16];
                fs.dir[i] = Ent {
                    name: <[u8; NAME]>::try_from(&e[..NAME]).unwrap(),
                    len: u16::from_le_bytes([e[12], e[13]]),
                    used: e[14] == 1,
                };
            }
        }
        fs
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.dir.iter().position(|e| e.used && ename(&e.name) == name)
    }

    fn flush(&self) -> bool {
        let mut s = [0u8; 512];
        for (i, e) in self.dir.iter().enumerate() {
            s[i * 16..i * 16 + NAME].copy_from_slice(&e.name);
            s[i * 16 + 12..i * 16 + 14].copy_from_slice(&e.len.to_le_bytes());
            s[i * 16 + 14] = e.used as u8;
        }
        write_sector(0, &s)
    }

    pub fn read(&self, i: usize, out: &mut [u8; FILE_MAX]) -> usize {
        if read_sector(1 + i as u32, out) { (self.dir[i].len as usize).min(FILE_MAX) } else { 0 }
    }

    pub fn write(&mut self, name: &str, data: &[u8]) -> Result<(), &'static str> {
        if !self.disk {
            return Err("No disk");
        }
        if name.is_empty() || name.len() >= NAME || !name.bytes().all(|c| (0x21..0x7f).contains(&c)) {
            return Err("Bad name");
        }
        if data.len() > FILE_MAX {
            return Err("Too big");
        }
        let i = match self.find(name) {
            Some(i) => i,
            None => self.dir.iter().position(|e| !e.used).ok_or("Disk full")?,
        };
        let mut s = [0u8; 512];
        s[..data.len()].copy_from_slice(data);
        if !write_sector(1 + i as u32, &s) {
            return Err("Disk error");
        }
        let mut n = [0u8; NAME];
        n[..name.len()].copy_from_slice(name.as_bytes());
        self.dir[i] = Ent { name: n, len: data.len() as u16, used: true };
        if self.flush() { Ok(()) } else { Err("Disk error") }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        match self.find(name) {
            Some(i) => {
                self.dir[i].used = false;
                self.flush()
            }
            None => false,
        }
    }
}
