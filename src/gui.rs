use crate::fs::{ename, Fs, FILE_MAX, MAX_FILES, NAME};
use crate::gfx::*;
use crate::input::Ev;
use crate::term::{Term, COLS, ROWS};
use core::fmt::Write;

const MENU_H: i32 = 12;
const DOCK_Y: i32 = 184;
const CPL: usize = 24; // notes chars per line
const NROWS: usize = 6; // notes visible rows
const FILES: usize = 0;
const NOTES: usize = 1;
const TERM: usize = 2;
const DOCK: [&str; 3] = ["Files", "Notes", "Term"];

struct Win {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    open: bool,
}

struct Line {
    b: [u8; 32],
    n: usize,
}
impl Line {
    fn new() -> Self {
        Line { b: [0; 32], n: 0 }
    }
    fn s(&self) -> &str {
        core::str::from_utf8(&self.b[..self.n]).unwrap_or("")
    }
}
impl Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &c in s.as_bytes() {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
        Ok(())
    }
}

struct Notes {
    buf: [u8; FILE_MAX],
    len: usize,
    name: [u8; NAME], // all zero = unsaved
    dirty: bool,
}

const CURSOR: [&str; 10] = [
    "#         ",
    "##        ",
    "#.#       ",
    "#..#      ",
    "#...#     ",
    "#....#    ",
    "#.....#   ",
    "#......#  ",
    "#...####  ",
    "#.#.#     ",
];

/// Lays text out in CPL columns. Calls f(row, col, byte) per glyph, returns caret (row, col).
fn walk(buf: &[u8], mut f: impl FnMut(usize, usize, u8)) -> (usize, usize) {
    let (mut r, mut c) = (0, 0);
    for &b in buf {
        if b == b'\n' {
            r += 1;
            c = 0;
            continue;
        }
        if c == CPL {
            r += 1;
            c = 0;
        }
        f(r, c, b);
        c += 1;
    }
    if c == CPL {
        r += 1;
        c = 0;
    }
    (r, c)
}

fn bytes(g: &mut Gfx, x: i32, y: i32, b: &[u8], c: u8) {
    for (i, &ch) in b.iter().enumerate() {
        g.glyph(x + 8 * i as i32, y, ch, c);
    }
}

pub struct Desktop {
    wins: [Win; 3],
    z: [usize; 3], // back to front
    mx: i32,
    my: i32,
    left: bool,
    drag: Option<(usize, i32, i32)>,
    fs: Fs,
    notes: Notes,
    term: Term,
    sel: Option<usize>,
    msg: &'static str,
    msg_until: u32,
    frame: u32,
}

impl Desktop {
    pub fn new() -> Self {
        let fs = Fs::new();
        let mut term = Term::new();
        let _ = writeln!(term, "Pixel OS. Type help.");
        if !fs.disk {
            let _ = writeln!(term, "No disk: saving is off.");
        }
        Desktop {
            wins: [
                Win { x: 4, y: 16, w: 104, h: 90, open: true },
                Win { x: 112, y: 16, w: 204, h: 90, open: true },
                Win { x: 4, y: 108, w: 312, h: 74, open: true },
            ],
            z: [FILES, NOTES, TERM],
            mx: 160,
            my: 100,
            left: false,
            drag: None,
            fs,
            notes: Notes { buf: [0; FILE_MAX], len: 0, name: [0; NAME], dirty: false },
            term,
            sel: None,
            msg: "",
            msg_until: 0,
            frame: 0,
        }
    }

    fn hit(&self, x: i32, y: i32, w: i32, h: i32) -> bool {
        (x..x + w).contains(&self.mx) && (y..y + h).contains(&self.my)
    }

    fn say(&mut self, m: &'static str) {
        self.msg = m;
        self.msg_until = self.frame + 150;
    }

    fn raise(&mut self, i: usize) {
        let p = self.z.iter().position(|&k| k == i).unwrap_or(2);
        for k in p..2 {
            self.z[k] = self.z[k + 1];
        }
        self.z[2] = i;
    }

    fn active(&self) -> Option<usize> {
        self.z.iter().rev().copied().find(|&i| self.wins[i].open)
    }

    // ---- file actions ----
    fn open_slot(&mut self, slot: usize) {
        let n = self.fs.read(slot, &mut self.notes.buf);
        self.notes.len = n;
        self.notes.name = self.fs.dir[slot].name;
        self.notes.dirty = false;
        self.sel = Some(slot);
        self.wins[NOTES].open = true;
        self.raise(NOTES);
    }

    fn new_name(&self) -> [u8; NAME] {
        for n in 1..100u32 {
            let mut l = Line::new();
            let _ = write!(l, "note{n}");
            if self.fs.find(l.s()).is_none() {
                let mut o = [0; NAME];
                o[..l.n].copy_from_slice(&l.b[..l.n]);
                return o;
            }
        }
        [0; NAME]
    }

    fn save(&mut self) {
        if self.notes.name[0] == 0 {
            self.notes.name = self.new_name();
        }
        let name = self.notes.name;
        match self.fs.write(ename(&name), &self.notes.buf[..self.notes.len]) {
            Ok(()) => {
                self.notes.dirty = false;
                self.sel = self.fs.find(ename(&name));
                self.say("Saved");
            }
            Err(e) => self.say(e),
        }
    }

    fn new_file(&mut self) {
        let name = self.new_name();
        match self.fs.write(ename(&name), b"") {
            Ok(()) => {
                if let Some(s) = self.fs.find(ename(&name)) {
                    self.open_slot(s);
                }
            }
            Err(e) => self.say(e),
        }
    }

    fn del_file(&mut self) {
        let Some(s) = self.sel else { return self.say("Select a file") };
        let nm = self.fs.dir[s].name;
        if self.fs.remove(ename(&nm)) {
            if self.notes.name == nm {
                self.notes = Notes { buf: [0; FILE_MAX], len: 0, name: [0; NAME], dirty: false };
            }
            self.sel = None;
            self.say("Deleted");
        }
    }

    // ---- input ----
    fn click(&mut self, i: usize) {
        let (wx, wy) = (self.wins[i].x, self.wins[i].y);
        match i {
            FILES => {
                if self.hit(wx + 4, wy + 14, 32, 11) {
                    self.new_file();
                } else if self.hit(wx + 40, wy + 14, 28, 11) {
                    self.del_file();
                } else {
                    for r in 0..MAX_FILES {
                        if self.fs.dir[r].used && self.hit(wx + 4, wy + 28 + 10 * r as i32, 96, 10) {
                            self.open_slot(r);
                        }
                    }
                }
            }
            NOTES => {
                if self.hit(wx + 4, wy + 14, 44, 11) {
                    self.save();
                }
            }
            _ => {}
        }
    }

    fn press(&mut self) {
        if self.my >= DOCK_Y {
            for i in 0..3 {
                if self.hit(4 + 60 * i as i32, DOCK_Y + 3, 56, 12) {
                    self.wins[i].open = true;
                    self.raise(i);
                }
            }
            return;
        }
        for k in (0..3).rev() {
            let i = self.z[k];
            let w = &self.wins[i];
            if !w.open || !self.hit(w.x, w.y, w.w, w.h) {
                continue;
            }
            let (wx, wy, ww) = (w.x, w.y, w.w);
            if self.my < wy + 12 {
                if self.mx >= wx + ww - 12 {
                    self.wins[i].open = false;
                    return;
                }
                self.drag = Some((i, self.mx - wx, self.my - wy));
                self.raise(i);
            } else {
                self.raise(i);
                self.click(i);
            }
            return;
        }
    }

    pub fn event(&mut self, e: Ev) {
        match e {
            Ev::Mouse { dx, dy, left } => {
                self.mx = (self.mx + dx).clamp(0, W - 1);
                self.my = (self.my - dy).clamp(0, H - 1);
                if left && !self.left {
                    self.press();
                }
                if !left {
                    self.drag = None;
                }
                self.left = left;
                if let Some((i, ox, oy)) = self.drag {
                    let w = &mut self.wins[i];
                    w.x = (self.mx - ox).clamp(0, W - w.w);
                    w.y = (self.my - oy).clamp(MENU_H, DOCK_Y - w.h);
                }
            }
            Ev::Key(c) => match self.active() {
                Some(NOTES) => match c {
                    8 => {
                        if self.notes.len > 0 {
                            self.notes.len -= 1;
                            self.notes.dirty = true;
                        }
                    }
                    _ if self.notes.len < FILE_MAX => {
                        self.notes.buf[self.notes.len] = c;
                        self.notes.len += 1;
                        self.notes.dirty = true;
                    }
                    _ => {}
                },
                Some(TERM) => match c {
                    8 => self.term.ilen = self.term.ilen.saturating_sub(1),
                    b'\n' => {
                        let secs = self.frame / 60;
                        if let Some(n) = self.term.run(&mut self.fs, secs) {
                            if let Some(s) = self.fs.find(ename(&n)) {
                                self.open_slot(s);
                            }
                        }
                    }
                    32..=126 if self.term.ilen < COLS - 2 => {
                        self.term.input[self.term.ilen] = c;
                        self.term.ilen += 1;
                    }
                    _ => {}
                },
                _ => {}
            },
        }
    }

    // ---- drawing ----
    pub fn draw(&mut self, g: &mut Gfx) {
        self.frame += 1;
        for y in 0..H {
            for x in 0..W {
                g.px(x, y, if (x + y) & 1 == 0 { BG_A } else { BG_B });
            }
        }
        self.menubar(g);
        let act = self.active();
        for k in 0..3 {
            let i = self.z[k];
            if self.wins[i].open {
                self.window(g, i, act == Some(i));
            }
        }
        self.dock(g);
        for (j, row) in CURSOR.iter().enumerate() {
            for (i, ch) in row.bytes().enumerate() {
                match ch {
                    b'#' => g.px(self.mx + i as i32, self.my + j as i32, INK),
                    b'.' => g.px(self.mx + i as i32, self.my + j as i32, PAPER),
                    _ => {}
                }
            }
        }
    }

    fn menubar(&self, g: &mut Gfx) {
        g.rect(0, 0, W, MENU_H, PANEL);
        g.rect(0, MENU_H - 1, W, 1, INK);
        g.text(6, 2, "Pixel OS", INK);
        g.text(7, 2, "Pixel OS", INK); // faux bold
        if self.frame < self.msg_until {
            g.text(110, 2, self.msg, INK);
        }
        let mut l = Line::new();
        let _ = write!(l, "up {}s", self.frame / 60);
        g.text(W - 6 - 8 * l.n as i32, 2, l.s(), MUTED);
    }

    fn button(&self, g: &mut Gfx, x: i32, y: i32, w: i32, label: &str) {
        g.rect(x, y, w, 11, INK);
        g.rect(x + 1, y + 1, w - 2, 9, if self.hit(x, y, w, 11) { PAPER } else { PANEL });
        g.text(x + (w - 8 * label.len() as i32) / 2, y + 2, label, INK);
    }

    fn window(&self, g: &mut Gfx, i: usize, active: bool) {
        let w = &self.wins[i];
        g.rect(w.x + 3, w.y + 3, w.w, w.h, SHADOW);
        g.rect(w.x, w.y, w.w, w.h, INK);
        g.rect(w.x + 1, w.y + 1, w.w - 2, w.h - 2, PAPER);
        g.rect(w.x + 1, w.y + 1, w.w - 2, 10, if active { ACCENT } else { DIM });
        g.rect(w.x + 1, w.y + 11, w.w - 2, 1, INK);
        let mut t = Line::new();
        if i == NOTES && self.notes.name[0] != 0 {
            let _ = write!(t, "Notes: {}{}", ename(&self.notes.name), if self.notes.dirty { "*" } else { "" });
        } else {
            let _ = write!(t, "{}", ["Files", "Notes", "Terminal"][i]);
        }
        g.text(w.x + 5, w.y + 2, t.s(), INK);
        g.glyph(w.x + w.w - 11, w.y + 2, b'x', INK);
        match i {
            FILES => self.files(g, w),
            NOTES => self.notes_view(g, w, active),
            _ => self.terminal(g, w, active),
        }
    }

    fn files(&self, g: &mut Gfx, w: &Win) {
        self.button(g, w.x + 4, w.y + 14, 32, "New");
        self.button(g, w.x + 40, w.y + 14, 28, "Del");
        let mut c = Line::new();
        let _ = write!(c, "{}/{}", self.fs.dir.iter().filter(|e| e.used).count(), MAX_FILES);
        g.text(w.x + w.w - 4 - 8 * c.n as i32, w.y + 16, c.s(), MUTED);
        if !self.fs.disk {
            g.text(w.x + 8, w.y + 29, "No disk", MUTED);
        } else if self.fs.dir.iter().all(|e| !e.used) {
            g.text(w.x + 8, w.y + 29, "No files yet", MUTED);
        }
        for (r, e) in self.fs.dir.iter().enumerate().filter(|(_, e)| e.used) {
            let y = w.y + 28 + 10 * r as i32;
            if self.sel == Some(r) {
                g.rect(w.x + 4, y, w.w - 8, 10, PANEL);
                g.rect(w.x + 4, y, 2, 10, ACCENT);
            }
            g.text(w.x + 8, y + 1, ename(&e.name), INK);
        }
    }

    fn notes_view(&self, g: &mut Gfx, w: &Win, active: bool) {
        self.button(g, w.x + 4, w.y + 14, 44, "Save");
        let b = &self.notes.buf[..self.notes.len];
        if b.is_empty() {
            g.text(w.x + 4, w.y + 29, "Type here...", MUTED);
        }
        let (cr, _) = walk(b, |_, _, _| {});
        let start = cr.saturating_sub(NROWS - 1);
        let (x0, y0) = (w.x + 4, w.y + 28);
        let (r, c) = walk(b, |r, c, ch| {
            if r >= start && r - start < NROWS {
                g.glyph(x0 + 8 * c as i32, y0 + 10 * (r - start) as i32, ch, INK);
            }
        });
        if active && self.frame & 32 == 0 && r >= start && r - start < NROWS {
            g.rect(x0 + 8 * c as i32, y0 + 10 * (r - start) as i32, 7, 9, ACCENT);
        }
    }

    fn terminal(&self, g: &mut Gfx, w: &Win, active: bool) {
        let (x, y) = (w.x + 4, w.y + 16);
        for r in 0..ROWS - 1 {
            bytes(g, x, y + 10 * r as i32, self.term.hist(r), INK);
        }
        let py = y + 10 * (ROWS - 1) as i32;
        g.text(x, py, "> ", INK);
        bytes(g, x + 16, py, &self.term.input[..self.term.ilen], INK);
        if active && self.frame & 32 == 0 {
            g.rect(x + 16 + 8 * self.term.ilen as i32, py, 7, 9, ACCENT);
        }
    }

    fn dock(&self, g: &mut Gfx) {
        g.rect(0, DOCK_Y, W, H - DOCK_Y, PANEL);
        g.rect(0, DOCK_Y, W, 1, INK);
        for i in 0..3 {
            let bx = 4 + 60 * i as i32;
            let open = self.wins[i].open;
            g.rect(bx, DOCK_Y + 3, 56, 12, INK);
            g.rect(bx + 1, DOCK_Y + 4, 54, 10, if open || self.hit(bx, DOCK_Y + 3, 56, 12) { PAPER } else { PANEL });
            if open {
                g.rect(bx + 1, DOCK_Y + 12, 54, 2, ACCENT);
            }
            g.text(bx + 28 - 4 * DOCK[i].len() as i32, DOCK_Y + 4, DOCK[i], INK);
        }
    }
}
