//! Your schoolwork on paper (sim::lessons::paper): the lesson's printed worksheet with your
//! answers in pencil and the teacher's red marks, and your subject notebook with the board copied
//! down. Japanese (kokugo) is written vertically, right to left, as it is in a real class.
//!
//! - [`worksheet`]: an A4 print, 1024 x 1448 (the `worksheet` prop's `kzpage_worksheet`).
//! - [`notebook`]: a B5 notebook's spread, 2048 x 1448 (`notebook_open`'s `kzpage_notebook`).

use crate::canvas::{srgb, Canvas, Font, Rgba};
use sim::lessons::paper::{subject_jp, vertical, NoteKind, Notes, Sheet, SheetItem};

pub const SHEET_W: usize = 1024;
pub const SHEET_H: usize = 1448;
pub const BOOK_W: usize = 2048;
pub const BOOK_H: usize = 1448;

const KANA: [&str; 8] = ["ア", "イ", "ウ", "エ", "オ", "カ", "キ", "ク"];
const KANJI_NUM: [&str; 12] = ["一", "二", "三", "四", "五", "六", "七", "八", "九", "十", "十一", "十二"];

fn printed() -> Rgba {
    srgb(28, 28, 30)
}
fn faint() -> Rgba {
    srgb(150, 150, 155)
}
fn pencil() -> Rgba {
    let mut c = srgb(52, 52, 60);
    c[3] = 0.95;
    c
}
/// Pencil still being written (the answer you're on): fainter.
fn pencil_faint() -> Rgba {
    let mut c = pencil();
    c[3] = 0.45;
    c
}
fn red() -> Rgba {
    srgb(214, 32, 36)
}
fn red_pen() -> Rgba {
    srgb(200, 40, 50)
}

fn seed_of(s: &str) -> u32 {
    s.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
}

fn date_jp(d: sim::Date) -> String {
    format!("{}月{}日（{}）", d.month, d.day, d.weekday().kanji())
}

/// What you wrote for a choice or an order, as you'd write it: the letters.
fn letters_for(item: &SheetItem, given: &str) -> String {
    match item.kind.as_str() {
        "choice" | "reflect" => item.options.iter().position(|o| o == given).map(|i| KANA[i.min(7)].to_string()).unwrap_or_else(|| given.to_string()),
        "order" => given.split(" → ").map(|g| item.options.iter().position(|o| o == g).map(|i| KANA[i.min(7)].to_string()).unwrap_or_default()).collect::<Vec<_>>().join("→"),
        // (a minus sign, not a hyphen, as you'd write it)
        _ => given.replace('-', "−"),
    }
}

/// The right answer, as the teacher writes it in red.
fn letters_solution(item: &SheetItem) -> String {
    match item.kind.as_str() {
        "choice" | "order" => letters_for(item, &item.solution),
        _ => item.solution.replace('-', "−"),
    }
}

/// A red circle drawn by hand over an answer (a touch open, a touch tilted).
fn red_circle(c: &mut Canvas, cx: f32, cy: f32, rx: f32, ry: f32) {
    c.ring(cx, cy, rx, ry, 4.0, red());
    c.ring(cx + 2.0, cy - 1.5, rx * 0.97, ry * 1.02, 2.0, red());
}

/// A red tick (レ) for a wrong answer.
fn red_tick(c: &mut Canvas, x: f32, y: f32, s: f32) {
    c.line(x, y, x + s * 0.35, y + s * 0.5, 5.0, red());
    c.line(x + s * 0.35, y + s * 0.5, x + s, y - s * 0.45, 5.0, red());
}

// ------------------------------------------------------------------------------------- the worksheet

/// A lesson's print (or a test paper), as it stands: printed questions, your pencil answers, the
/// red marks once it's marked. `name`: yours, as you write it at the top.
pub fn worksheet(sheet: &Sheet, name: &str) -> Canvas {
    if vertical(sheet.subject) {
        return worksheet_vertical(sheet, name);
    }
    let mut c = Canvas::new(SHEET_W, SHEET_H, srgb(248, 247, 242));
    let (l, r) = (62.0, SHEET_W as f32 - 62.0);
    let kind = if sheet.exam.is_some() { "テスト" } else { "プリント" };
    // the header: year and subject, the date, the unit and the aim, your class and name
    c.text(&format!("1年 {} {kind}", subject_jp(sheet.subject)), l, 84.0, Font::Bold, 34.0, printed(), 0.0);
    c.text(&date_jp(sheet.date), r, 80.0, Font::Regular, 24.0, printed(), 1.0);
    c.rect(l, 104.0, r - l, 3.0, printed());
    c.text(&sheet.unit, l, 154.0, Font::Bold, 30.0, printed(), 0.0);
    c.text("1年　A組　　番　名前", l, 206.0, Font::Regular, 24.0, printed(), 0.0);
    c.rect(l + 268.0, 212.0, 330.0, 2.0, printed());
    c.hand(name, l + 290.0, 204.0, 34.0, pencil(), seed_of(name));
    // the aim (めあて), boxed
    let aim_top = 236.0;
    c.rect(l, aim_top, r - l, 2.0, printed());
    c.rect(l, aim_top + 58.0, r - l, 2.0, printed());
    c.rect(l, aim_top, 2.0, 60.0, printed());
    c.rect(r - 2.0, aim_top, 2.0, 60.0, printed());
    c.text("めあて", l + 14.0, aim_top + 40.0, Font::Bold, 24.0, printed(), 0.0);
    c.text(&sheet.topic, l + 110.0, aim_top + 40.0, Font::Regular, 24.0, printed(), 0.0);

    // the questions: as many as there are, the type shrinking to fit the page
    let top = aim_top + 96.0;
    let n = sheet.items.len().max(1) as f32;
    let room = SHEET_H as f32 - 90.0 - top;
    let size = (room / n / 4.4).clamp(19.0, 30.0);
    let lead = size * 1.35;
    let box_x = r - 250.0;
    let text_w = box_x - l - 70.0;
    let mut y = top;
    for (k, it) in sheet.items.iter().enumerate() {
        let y0 = y;
        if sheet.current == Some(k) {
            // (where you're up to: a pencil arrow in the margin)
            c.hand("→", l - 52.0, y + size, size * 1.2, pencil(), 7);
        }
        c.text(&format!("({})", k + 1), l, y + size, Font::Bold, size, printed(), 0.0);
        let lines = c.wrap(&it.prompt, Font::Regular, size, text_w);
        for (j, line) in lines.iter().enumerate() {
            c.text(line, l + 56.0, y + size + j as f32 * lead, Font::Regular, size, printed(), 0.0);
        }
        y += lines.len() as f32 * lead;
        // the printed choices
        if matches!(it.kind.as_str(), "choice" | "order" | "reflect" | "sort") && !it.options.is_empty() {
            let mut x = l + 56.0;
            for (j, o) in it.options.iter().enumerate() {
                let label = if it.kind == "sort" { format!("［{o}］") } else { format!("{}　{o}", KANA[j.min(7)]) };
                let w = c.measure(&label, Font::Regular, size * 0.92) + 30.0;
                if x + w > box_x - 10.0 && x > l + 56.0 {
                    x = l + 56.0;
                    y += lead;
                }
                c.text(&label, x, y + size, Font::Regular, size * 0.92, printed(), 0.0);
                if it.kind == "reflect" && it.given.as_deref() == Some(o.as_str()) {
                    // (you circle the one you'd choose)
                    c.ring(x + w * 0.5 - 15.0, y + size * 0.62, w * 0.55, size * 0.85, 2.5, pencil());
                }
                x += w;
            }
            y += lead;
        }
        // the answer box at the right: your answer in pencil
        let bh = (y - y0).max(size * 2.0).min(size * 2.6);
        let by = y0 + 2.0;
        if it.kind != "reflect" {
            c.rect(box_x, by, 230.0, 2.0, printed());
            c.rect(box_x, by + bh, 230.0, 2.0, printed());
            c.rect(box_x, by, 2.0, bh, printed());
            c.rect(box_x + 228.0, by, 2.0, bh, printed());
            if !it.unit.is_empty() {
                c.text(&it.unit, box_x + 222.0, by + bh - 10.0, Font::Regular, size * 0.8, printed(), 1.0);
            }
        }
        // your answer, fitted to the box (the tick has the right end of it)
        let mut tw = 0.0;
        if let (Some(g), true) = (&it.given, it.kind != "reflect") {
            let txt = letters_for(it, g);
            let mut hs = size * 1.35;
            let fit = 160.0;
            if c.hand_width(&txt, hs) > fit {
                hs *= fit / c.hand_width(&txt, hs);
            }
            tw = c.hand(&txt, box_x + 16.0, by + bh * 0.5 + hs * 0.38, hs, if it.drafting { pencil_faint() } else { pencil() }, seed_of(&txt) ^ k as u32);
        }
        // the marks: a circle round a right answer, a tick and the answer in red by a wrong one
        match it.right {
            Some(true) => red_circle(&mut c, box_x + 16.0 + tw * 0.5, by + bh * 0.5, (tw * 0.5 + 26.0).max(46.0), bh * 0.56),
            Some(false) => {
                red_tick(&mut c, box_x + 178.0, by + bh * 0.45, 42.0);
                let sol = letters_solution(it);
                let mut ss = size * 0.95;
                if c.hand_width(&sol, ss) > 225.0 {
                    ss *= 225.0 / c.hand_width(&sol, ss);
                }
                c.hand(&sol, box_x + 4.0, by + bh + size * 1.05, ss, red_pen(), seed_of(&sol));
            }
            None => {}
        }
        y = y.max(y0 + bh + size * 0.6) + size * 0.9;
        if it.right == Some(false) {
            y += size * 0.6;
        }
        // a faint rule between questions
        c.rect(l, y - size * 0.55, r - l, 1.0, faint());
    }
    // the mark at the top
    if sheet.marked {
        let txt = match sheet.mark {
            Some(m) => format!("{m}点"),
            None => {
                let (a, b) = sheet.score();
                format!("{a}/{b}")
            }
        };
        c.hand(&txt, r - 250.0, 196.0, 58.0, red(), seed_of(&txt));
        c.line(r - 250.0, 208.0, r - 40.0, 204.0, 3.5, red());
        c.line(r - 246.0, 216.0, r - 44.0, 213.0, 3.0, red());
    }
    c.grain(seed_of(&sheet.topic), 0.012);
    c
}

/// Kokugo's print: written vertically, the columns running right to left; the header down the
/// right edge; the questions in two bands; your name across the bottom.
fn worksheet_vertical(sheet: &Sheet, name: &str) -> Canvas {
    let mut c = Canvas::new(SHEET_W, SHEET_H, srgb(248, 247, 242));
    let w = SHEET_W as f32;
    let kind = if sheet.exam.is_some() { "テスト" } else { "プリント" };
    // the header down the right: subject, the aim, the date
    c.vertical(&format!("一年　国語{kind}"), w - 64.0, 60.0, 700.0, Font::Bold, 36.0, 44.0, printed(), None);
    c.rect(w - 112.0, 50.0, 2.0, SHEET_H as f32 - 190.0, printed());
    c.vertical(&format!("めあて　{}", sheet.topic), w - 142.0, 60.0, 1220.0, Font::Regular, 24.0, 32.0, printed(), None);
    c.vertical(&date_jp(sheet.date), w - 64.0, 760.0, 1240.0, Font::Regular, 24.0, 30.0, printed(), None);
    c.rect(w - 174.0, 50.0, 2.0, SHEET_H as f32 - 190.0, printed());
    // your class and name across the bottom
    c.rect(56.0, SHEET_H as f32 - 120.0, w - 112.0, 2.0, printed());
    c.text("一年　A組　　番　名前", 70.0, SHEET_H as f32 - 66.0, Font::Regular, 26.0, printed(), 0.0);
    c.hand(name, 360.0, SHEET_H as f32 - 64.0, 38.0, pencil(), seed_of(name));
    // the questions, right to left in two bands
    let bands: [(f32, f32); 2] = [(70.0, 620.0), (680.0, 1240.0)];
    let size: f32 = 26.0;
    let col = 36.0;
    let mut band = 0;
    let mut x = w - 220.0;
    for (k, it) in sheet.items.iter().enumerate() {
        // (how many columns it needs: its number and question, its choices, the answer box)
        let (top, bottom) = bands[band];
        let ans_h = 170.0;
        let q_bottom = bottom - ans_h - 20.0;
        let chars_per_col = ((q_bottom - top - 40.0) / (size * 1.06)).floor().max(4.0);
        let q_cols = (it.prompt.chars().count() as f32 / chars_per_col).ceil().max(1.0);
        let o_cols = if it.options.is_empty() { 0.0 } else { it.options.iter().map(|o| ((o.chars().count() + 2) as f32 / chars_per_col).ceil()).sum::<f32>() };
        let width = (q_cols + o_cols) * col + 70.0;
        if x - width < 50.0 {
            band += 1;
            if band >= bands.len() {
                break;
            }
            x = w - 220.0;
        }
        let (top, bottom) = bands[band];
        let q_bottom = bottom - ans_h - 20.0;
        if sheet.current == Some(k) {
            c.vertical("↓", x, top - 44.0, top, Font::Hand, size, col, pencil(), Some(7));
        }
        c.vertical(KANJI_NUM[k.min(11)], x, top, top + 40.0, Font::Bold, size, col, printed(), None);
        let used = c.vertical(&it.prompt, x, top + 40.0, q_bottom, Font::Regular, size, col, printed(), None);
        let mut cx = x - used as f32 * col;
        for (j, o) in it.options.iter().enumerate() {
            let label = format!("{}　{o}", KANA[j.min(7)]);
            let u = c.vertical(&label, cx, top + 40.0, q_bottom, Font::Regular, size * 0.88, col, printed(), None);
            if it.kind == "reflect" && it.given.as_deref() == Some(o.as_str()) {
                c.ring(cx, top + 120.0, col * 0.6, 100.0, 2.5, pencil());
            }
            cx -= u as f32 * col;
        }
        // the answer box at the foot of the question's first column
        let (bx, by) = (x - 30.0, bottom - ans_h);
        c.rect(bx, by, 60.0, 2.0, printed());
        c.rect(bx, by + ans_h, 60.0, 2.0, printed());
        c.rect(bx, by, 2.0, ans_h, printed());
        c.rect(bx + 58.0, by, 2.0, ans_h, printed());
        if let Some(g) = &it.given {
            let txt = letters_for(it, g);
            c.vertical(&txt, x, by + 10.0, by + ans_h, Font::Hand, 36.0, 40.0, if it.drafting { pencil_faint() } else { pencil() }, Some(seed_of(&txt)));
        }
        match it.right {
            Some(true) => red_circle(&mut c, x, by + ans_h * 0.5, 44.0, ans_h * 0.6),
            Some(false) => {
                red_tick(&mut c, x - 20.0, by + ans_h * 0.4, 44.0);
                let sol = letters_solution(it);
                c.vertical(&sol, x - 48.0, by + 10.0, by + ans_h + 60.0, Font::Hand, 26.0, 30.0, red_pen(), Some(seed_of(&sol)));
            }
            None => {}
        }
        x = cx - 40.0;
        c.rect(x + 18.0, top, 1.0, bottom - top, faint());
    }
    if sheet.marked {
        let txt = match sheet.mark {
            Some(m) => format!("{m}点"),
            None => {
                let (a, b) = sheet.score();
                format!("{a}/{b}")
            }
        };
        c.hand(&txt, 70.0, 120.0, 64.0, red(), seed_of(&txt));
        c.line(66.0, 134.0, 260.0, 130.0, 3.5, red());
    }
    c.grain(seed_of(&sheet.topic), 0.012);
    c
}

// ------------------------------------------------------------------------------------- the notebook

/// A ruled B5 school notebook's spread, `notes` copied into it (the latest that fit, oldest
/// first); kokugo's is ruled vertically and written right to left.
pub fn notebook(subject: sim::school::Subject, notes: &[Notes]) -> Canvas {
    let mut c = Canvas::new(BOOK_W, BOOK_H, srgb(250, 249, 243));
    let rule = srgb(150, 175, 200);
    let half = BOOK_W as f32 / 2.0;
    let vert = vertical(subject);
    for page in 0..2 {
        let x0 = page as f32 * half;
        // the printed head of each page: No. and Date
        let hx = x0 + half - 90.0;
        c.text("No.", hx - 330.0, 64.0, Font::Regular, 22.0, rule, 0.0);
        c.rect(hx - 290.0, 68.0, 110.0, 1.5, rule);
        c.text("Date", hx - 160.0, 64.0, Font::Regular, 22.0, rule, 0.0);
        c.text("・　・", hx - 60.0, 64.0, Font::Regular, 22.0, rule, 0.0);
        c.rect(hx - 100.0, 68.0, 130.0, 1.5, rule);
        if vert {
            let mut x = x0 + half - 70.0;
            while x > x0 + 60.0 {
                c.line(x, 100.0, x, BOOK_H as f32 - 60.0, 1.4, rule);
                x -= 56.0;
            }
        } else {
            let mut y = 140.0;
            let mut n = 0;
            while y < BOOK_H as f32 - 50.0 {
                c.line(x0 + 50.0, y, x0 + half - 50.0, y, 1.4, rule);
                // (the dots along the rules that keep your columns straight)
                if n % 1 == 0 {
                    let mut dx = x0 + 50.0 + 64.0;
                    while dx < x0 + half - 50.0 {
                        c.rect(dx, y - 29.0, 1.6, 1.6, rule);
                        dx += 64.0;
                    }
                }
                y += 58.0;
                n += 1;
            }
        }
    }
    // the gutter's shadow
    for i in 0..50 {
        let a = 0.14 * (1.0 - i as f32 / 50.0);
        c.rect(half - i as f32, 0.0, 1.0, BOOK_H as f32, [0.3, 0.27, 0.22, a]);
        c.rect(half + i as f32, 0.0, 1.0, BOOK_H as f32, [0.3, 0.27, 0.22, a]);
    }
    if vert {
        notes_vertical(&mut c, notes);
    } else {
        notes_horizontal(&mut c, notes);
    }
    c.grain(91, 0.01);
    c
}

/// How many ruled lines a lesson's notes take (horizontal pages).
fn note_lines(c: &Canvas, n: &Notes, width: f32) -> usize {
    1 + n.lines.iter().map(|l| c.wrap(&l.text, Font::Hand, 34.0, width).len().max(1)).sum::<usize>() + 1
}

fn notes_horizontal(c: &mut Canvas, notes: &[Notes]) {
    let half = BOOK_W as f32 / 2.0;
    let width = half - 170.0;
    let per_page = ((BOOK_H as f32 - 50.0 - 140.0) / 58.0) as usize;
    // the latest lessons that fit on the spread
    let mut start = notes.len();
    let mut used = 0;
    while start > 0 {
        let need = note_lines(c, &notes[start - 1], width);
        if used + need > per_page * 2 {
            break;
        }
        used += need;
        start -= 1;
    }
    let (mut page, mut line) = (0usize, 0usize);
    let next = |page: &mut usize, line: &mut usize| {
        *line += 1;
        if *line >= per_page {
            *page += 1;
            *line = 0;
        }
    };
    for n in &notes[start..] {
        let x0 = page as f32 * half + 80.0;
        let y = 140.0 + line as f32 * 58.0 - 12.0;
        c.hand(&format!("{}/{}（{}）", n.date.month, n.date.day, n.date.weekday().kanji()), x0, y, 32.0, pencil(), seed_of(&n.date.to_string()));
        next(&mut page, &mut line);
        for (k, l) in n.lines.iter().enumerate() {
            if page >= 2 {
                return;
            }
            let x0 = page as f32 * half + 80.0;
            let (col, indent, size) = match l.kind {
                NoteKind::Title => (pencil(), 0.0, 38.0),
                NoteKind::Point => (red_pen(), 20.0, 32.0),
                NoteKind::Example => (pencil(), 20.0, 32.0),
                NoteKind::Answer => (pencil(), 90.0, 34.0),
            };
            let text = match l.kind {
                NoteKind::Example => format!("例）{}", l.text),
                NoteKind::Answer => format!("→ {}", l.text),
                _ => l.text.clone(),
            };
            let lines = c.wrap(&text, Font::Hand, size, width - indent);
            for (j, t) in lines.iter().enumerate() {
                if page >= 2 {
                    return;
                }
                let x0 = page as f32 * half + 80.0;
                let y = 140.0 + line as f32 * 58.0 - 12.0;
                let w = c.hand(t, x0 + indent, y, size, col, seed_of(t) ^ (k * 31 + j) as u32);
                match l.kind {
                    // the title boxed, the point underlined in red
                    NoteKind::Title => {
                        c.line(x0 - 10.0, y - 44.0, x0 + w + 12.0, y - 46.0, 2.2, pencil());
                        c.line(x0 - 10.0, y + 12.0, x0 + w + 12.0, y + 11.0, 2.2, pencil());
                        c.line(x0 - 10.0, y - 44.0, x0 - 10.0, y + 12.0, 2.2, pencil());
                        c.line(x0 + w + 12.0, y - 46.0, x0 + w + 12.0, y + 11.0, 2.2, pencil());
                    }
                    NoteKind::Point => c.line(x0 + indent, y + 8.0, x0 + indent + w, y + 6.0, 2.5, red_pen()),
                    _ => {}
                }
                next(&mut page, &mut line);
            }
            let _ = x0;
        }
        next(&mut page, &mut line);
    }
}

fn notes_vertical(c: &mut Canvas, notes: &[Notes]) {
    // right page first (a vertical notebook opens from the right), right to left
    let half = BOOK_W as f32 / 2.0;
    let (top, bottom) = (110.0, BOOK_H as f32 - 70.0);
    let per_col = ((bottom - top) / (34.0 * 1.06)) as usize;
    let cols_of = |n: &Notes| -> usize { 1 + n.lines.iter().map(|l| (l.text.chars().count() + 3) / per_col.max(1) + 1).sum::<usize>() + 1 };
    let cols_per_page = ((half - 140.0) / 56.0) as usize;
    let mut start = notes.len();
    let mut used = 0;
    while start > 0 {
        let need = cols_of(&notes[start - 1]);
        if used + need > cols_per_page * 2 {
            break;
        }
        used += need;
        start -= 1;
    }
    let mut colno = 0usize;
    let x_of = |k: usize| -> f32 {
        let page = k / cols_per_page;
        let base = if page == 0 { BOOK_W as f32 - 98.0 } else { half - 98.0 };
        base - (k % cols_per_page) as f32 * 56.0
    };
    for n in &notes[start..] {
        if colno >= cols_per_page * 2 {
            return;
        }
        c.vertical(&format!("{}月{}日", n.date.month, n.date.day), x_of(colno), top, bottom, Font::Hand, 30.0, 56.0, pencil(), Some(seed_of(&n.date.to_string())));
        colno += 1;
        for l in &n.lines {
            if colno >= cols_per_page * 2 {
                return;
            }
            let (col, size, lead) = match l.kind {
                NoteKind::Title => (pencil(), 36.0, "　"),
                NoteKind::Point => (red_pen(), 32.0, "　"),
                NoteKind::Example => (pencil(), 32.0, "例　"),
                NoteKind::Answer => (pencil(), 34.0, "　　答　"),
            };
            let text = format!("{lead}{}", l.text);
            let x = x_of(colno);
            let used = c.vertical(&text, x, top, bottom, Font::Hand, size, 56.0, col, Some(seed_of(&text)));
            match l.kind {
                NoteKind::Title => {
                    c.line(x - 24.0, top - 6.0, x - 24.0, top + size * 1.06 * text.chars().count() as f32, 2.0, pencil());
                    c.line(x + 24.0, top - 6.0, x + 24.0, top + size * 1.06 * text.chars().count() as f32, 2.0, pencil());
                }
                NoteKind::Point => c.line(x + 22.0, top + 30.0, x + 22.0, (top + size * 1.06 * text.chars().count() as f32).min(bottom), 2.5, red_pen()),
                _ => {}
            }
            colno += used;
        }
        colno += 1;
    }
}
