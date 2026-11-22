use crate::fs::{Fs, FILE_MAX, NAME};
use core::fmt::Write;

pub const COLS: usize = 38;
pub const ROWS: usize = 5; // visible rows: ROWS-1 history + prompt
const HIST: usize = 16;

pub struct Term {
    lines: [[u8; COLS]; HIST],
    n: [u8; HIST],
    head: usize,
    pub input: [u8; COLS - 2],
    pub ilen: usize,
}

impl Write for Term {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.bytes().for_each(|b| self.putc(b));
        Ok(())
    }
}

impl Term {
    pub fn new() -> Self {
        Term { lines: [[0; COLS]; HIST], n: [0; HIST], head: 0, input: [0; COLS - 2], ilen: 0 }
    }

    fn nl(&mut self) {
        self.head = (self.head + 1) % HIST;
        self.n[self.head] = 0;
    }

    fn putc(&mut self, c: u8) {
        if c == b'\n' {
            return self.nl();
        }
        if self.n[self.head] as usize == COLS {
            self.nl();
        }
        let c = if (32..127).contains(&c) { c } else { b'?' };
        self.lines[self.head][self.n[self.head] as usize] = c;
        self.n[self.head] += 1;
    }

    /// History row r (0 = oldest shown). Current head line is always empty between commands.
    pub fn hist(&self, r: usize) -> &[u8] {
        let i = (self.head + HIST - (ROWS - 1 - r)) % HIST;
        &self.lines[i][..self.n[i] as usize]
    }

    /// Runs the input line. Returns a filename if the command wants it opened in Notes.
    pub fn run(&mut self, fs: &mut Fs, secs: u32) -> Option<[u8; NAME]> {
        let mut buf = [0u8; COLS - 2];
        let l = self.ilen;
        buf[..l].copy_from_slice(&self.input[..l]);
        self.ilen = 0;
        let line = core::str::from_utf8(&buf[..l]).unwrap_or("").trim();
        let _ = writeln!(self, "> {line}");
        let (cmd, arg) = line.split_once(' ').unwrap_or((line, ""));
        let arg = arg.trim();
        match cmd {
            "" => {}
            "help" => {
                let _ = writeln!(self, "ls  cat F  rm F  echo T\nwrite F T  edit F\nclear  about  up");
            }
            "ls" => {
                if !fs.disk {
                    let _ = writeln!(self, "no disk");
                } else if fs.dir.iter().all(|e| !e.used) {
                    let _ = writeln!(self, "(empty)");
                }
                for e in fs.dir.iter().filter(|e| e.used) {
                    let _ = writeln!(self, "{} {}b", crate::fs::ename(&e.name), e.len);
                }
            }
            "cat" => match fs.find(arg) {
                Some(i) => {
                    let mut d = [0u8; FILE_MAX];
                    let n = fs.read(i, &mut d);
                    d[..n].iter().for_each(|&c| self.putc(c));
                    self.putc(b'\n');
                }
                None => {
                    let _ = writeln!(self, "not found");
                }
            },
            "rm" => {
                let _ = writeln!(self, "{}", if fs.remove(arg) { "removed" } else { "not found" });
            }
            "echo" => {
                let _ = writeln!(self, "{arg}");
            }
            "write" => {
                let (f, t) = arg.split_once(' ').unwrap_or((arg, ""));
                match fs.write(f, t.as_bytes()) {
                    Ok(()) => {
                        let _ = writeln!(self, "saved");
                    }
                    Err(e) => {
                        let _ = writeln!(self, "{e}");
                    }
                }
            }
            "edit" => {
                if fs.find(arg).is_none() {
                    if let Err(e) = fs.write(arg, b"") {
                        let _ = writeln!(self, "{e}");
                        return None;
                    }
                }
                let mut n = [0u8; NAME];
                n[..arg.len()].copy_from_slice(arg.as_bytes());
                return Some(n);
            }
            "clear" => (0..ROWS).for_each(|_| self.nl()),
            "about" => {
                let _ = writeln!(self, "Pixel OS v{}\nRust no_std, VGA 13h", env!("CARGO_PKG_VERSION"));
            }
            "up" => {
                let _ = writeln!(self, "{secs}s");
            }
            _ => {
                let _ = writeln!(self, "unknown: {cmd}");
            }
        }
        None
    }
}
