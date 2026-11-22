//! Your journal's spreads (two facing pages of a ruled notebook), written in your hand from the
//! game state:
//!
//! - [`diary`]: the days, newest last, written up from what you did (`sim::life::Journal`).
//! - [`people`]: someone per section: what you know about them, by how well you know them.
//! - [`asks_and_plans`]: what people asked of you, and your plans.

use crate::canvas::{srgb, Canvas, Font};
use sim::people::Fact;
use sim::GameState;

pub const W: usize = 2048;
pub const H: usize = 1448;
const MARGIN: f32 = 110.0;
const LINE: f32 = 70.0;

/// A blank spread: cream paper, blue rules, a red margin line, the gutter's shadow.
fn spread() -> Canvas {
    let mut c = Canvas::new(W, H, srgb(250, 246, 232));
    let rule = srgb(170, 190, 220);
    for page in 0..2 {
        let x0 = page as f32 * W as f32 / 2.0;
        let mut y = 150.0;
        while y < H as f32 - 60.0 {
            c.line(x0 + 40.0, y, x0 + W as f32 / 2.0 - 40.0, y, 1.6, rule);
            y += LINE;
        }
        c.line(x0 + MARGIN - 18.0, 60.0, x0 + MARGIN - 18.0, H as f32 - 40.0, 1.6, srgb(225, 150, 150));
    }
    for i in 0..60 {
        let a = 0.16 * (1.0 - i as f32 / 60.0);
        c.rect(W as f32 / 2.0 - i as f32, 0.0, 1.0, H as f32, [0.3, 0.25, 0.2, a]);
        c.rect(W as f32 / 2.0 + i as f32, 0.0, 1.0, H as f32, [0.3, 0.25, 0.2, a]);
    }
    c
}

/// Writes lines down the pages, left then right, a ruled line at a time.
struct Pen {
    page: usize,
    line: usize,
    lines_per_page: usize,
}

impl Pen {
    fn new() -> Self {
        Pen { page: 0, line: 0, lines_per_page: ((H as f32 - 60.0 - 150.0) / LINE) as usize }
    }
    fn full(&self) -> bool {
        self.page >= 2
    }
    /// Write (wrapped across ruled lines); false once both pages are full.
    fn write(&mut self, c: &mut Canvas, text: &str, font: Font, size: f32, col: [f32; 4], indent: f32) -> bool {
        if self.full() {
            return false;
        }
        let x = self.page as f32 * W as f32 / 2.0 + MARGIN + indent;
        let width = W as f32 / 2.0 - MARGIN - 70.0 - indent;
        let y = 150.0 + self.line as f32 * LINE - 10.0;
        // (wrap by hand to count lines)
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        for word in text.split_inclusive(' ') {
            let t = format!("{cur}{word}");
            if c.measure(t.trim_end(), font, size) > width && !cur.is_empty() {
                lines.push(cur.trim_end().into());
                cur = word.into();
            } else {
                cur = t;
            }
        }
        if !cur.trim().is_empty() {
            lines.push(cur.trim_end().into());
        }
        for (k, l) in lines.iter().enumerate() {
            if self.line >= self.lines_per_page {
                self.page += 1;
                self.line = 0;
                if self.full() {
                    return false;
                }
            }
            let x = if k == 0 { x } else { self.page as f32 * W as f32 / 2.0 + MARGIN + indent };
            let y = if k == 0 { y } else { 150.0 + self.line as f32 * LINE - 10.0 };
            c.text(l, x, y, font, size, col, 0.0);
            self.line += 1;
        }
        true
    }
    fn gap(&mut self) {
        self.line += 1;
        if self.line >= self.lines_per_page {
            self.page += 1;
            self.line = 0;
        }
    }
}

fn ink() -> [f32; 4] {
    srgb(35, 45, 90)
}

/// The diary: the last days that fit on a spread, newest at the end.
pub fn diary(g: &GameState) -> Canvas {
    let mut c = spread();
    let mut pen = Pen::new();
    let days = &g.life.journal.diary;
    // as many recent days as fit: count lines roughly (one per entry plus a heading and gap)
    let budget = pen.lines_per_page * 2;
    let mut used = 0;
    let mut start = days.len();
    while start > 0 {
        let need = days[start - 1].lines.len() + 2;
        if used + need > budget {
            break;
        }
        used += need;
        start -= 1;
    }
    for d in &days[start..] {
        let head = format!("{} {} {} ({})", d.date.day, d.date.month_name(), d.date.year, d.date.weekday().kanji());
        pen.write(&mut c, &head, Font::Bold, 48.0, ink(), 0.0);
        for l in &d.lines {
            if !pen.write(&mut c, &format!("· {l}"), Font::Hand, 46.0, ink(), 16.0) {
                break;
            }
        }
        pen.gap();
    }
    if days.is_empty() {
        pen.write(&mut c, "(Nothing written yet.)", Font::Hand, 46.0, ink(), 0.0);
    }
    c.grain(7, 0.01);
    c
}

/// People you know, a section each: what you've learned, by how well you know them.
pub fn people(g: &GameState) -> Canvas {
    let mut c = spread();
    let mut pen = Pen::new();
    pen.write(&mut c, "People", Font::Bold, 56.0, ink(), 0.0);
    let uncle = format!("Uncle Kenji: runs Tsukumodō. {} (rank {}).", g.uncle.title(), g.uncle.rank());
    pen.write(&mut c, &uncle, Font::Hand, 46.0, ink(), 0.0);
    pen.gap();
    let mut known: Vec<usize> = (0..g.students.len()).filter(|&i| g.bonds[i].knows(Fact::Name)).collect();
    known.sort_by_key(|&i| std::cmp::Reverse(g.bonds[i].points));
    for i in known {
        let s = &g.students[i];
        let b = &g.bonds[i];
        pen.write(&mut c, &format!("{} ({})", s.full_name(), s.class_name()), Font::Bold, 46.0, ink(), 0.0);
        let mut facts = vec![format!("{} (rank {})", b.title(), b.rank())];
        if b.knows(Fact::Hobby) {
            facts.push(format!("likes {}", s.hobby));
        }
        if b.knows(Fact::Club) {
            facts.push(s.club.map(|c| format!("{} club", spaced(&format!("{c:?}")).to_lowercase())).unwrap_or("no club".into()));
        }
        if b.knows(Fact::Favorites) {
            facts.push(format!("favourite cards: {}", s.favorites.iter().map(|a| format!("{a:?}")).collect::<Vec<_>>().join(", ")));
        }
        if b.knows(Fact::Wallet) {
            facts.push(format!("pocket money ¥{} a week", s.pocket_money_weekly));
        }
        if !pen.write(&mut c, &facts.join("; "), Font::Hand, 44.0, ink(), 16.0) {
            break;
        }
        pen.gap();
    }
    c.grain(11, 0.01);
    c
}

/// What people asked of you, and your plans.
pub fn asks_and_plans(g: &GameState) -> Canvas {
    let mut c = spread();
    let mut pen = Pen::new();
    let j = &g.life.journal;
    pen.write(&mut c, "Asked of me", Font::Bold, 56.0, ink(), 0.0);
    if j.asks.is_empty() {
        pen.write(&mut c, "(nothing yet)", Font::Hand, 44.0, ink(), 16.0);
    }
    for a in &j.asks {
        let due = a.due.map(|d| format!(" (by {} {})", d.day, d.month_name())).unwrap_or_default();
        let mark = if a.done { "☑" } else { "☐" };
        pen.write(&mut c, &format!("{mark} {}: {}{due}", a.from, a.text), Font::Hand, 44.0, ink(), 16.0);
    }
    pen.gap();
    pen.write(&mut c, "Plans", Font::Bold, 56.0, ink(), 0.0);
    let today = g.time.date;
    let coming: Vec<_> = j.plans.iter().filter(|p| p.date >= today).collect();
    if coming.is_empty() {
        pen.write(&mut c, "(nothing planned)", Font::Hand, 44.0, ink(), 16.0);
    }
    for p in coming {
        pen.write(&mut c, &format!("{} {} ({}): {}", p.date.day, p.date.month_name(), p.date.weekday().kanji(), p.text), Font::Hand, 44.0, ink(), 16.0);
    }
    c.grain(13, 0.01);
    c
}

/// "BrassBand" -> "Brass Band".
fn spaced(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if i > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}
