//! A wall calendar's month page, as it hangs in Tsukumodō and in your room: a seasonal picture,
//! the month, and the grid (Sundays and holidays red, Saturdays blue), with the town's events
//! printed small and your plans written in pencil. Today is ringed.

use crate::canvas::{srgb, Canvas, Font};
use sim::calendar::{events_on, holiday, school_break, Date, Event, Weekday};

/// The old names of the months, printed beside the number.
const OLD_NAMES: [&str; 12] = ["睦月", "如月", "弥生", "卯月", "皐月", "水無月", "文月", "葉月", "長月", "神無月", "霜月", "師走"];
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

pub const W: usize = 1024;
pub const H: usize = 1448;

fn days_in(year: i32, month: u32) -> u32 {
    let first = Date::new(year, month, 1);
    let next = if month == 12 { Date::new(year + 1, 1, 1) } else { Date::new(year, month + 1, 1) };
    (next.to_days() - first.to_days()) as u32
}

/// A short label for an event in a calendar square.
fn short(e: &Event) -> Option<String> {
    Some(match e {
        Event::Holiday(_) => return None, // (written as the holiday's name)
        Event::SetRelease(_) => "New cards out".into(),
        Event::ShopTournament => "Shop tournament".into(),
        Event::EntranceCeremony => "Entrance ceremony".into(),
        Event::Exams(_) => "Exams".into(),
        Event::SportsDay => "Sports day".into(),
        Event::SummerFestival => "Summer festival".into(),
        Event::CultureFestival => "Culture festival".into(),
        Event::ClosingCeremony(_) => "Closing ceremony".into(),
    })
}

/// The picture at the top: a sky for the season and something of the month.
fn picture(c: &mut Canvas, month: u32, x: f32, y: f32, w: f32, h: f32) {
    let (top, bottom) = match month {
        12 | 1 | 2 => (srgb(150, 175, 205), srgb(230, 236, 242)),
        3..=5 => (srgb(150, 195, 235), srgb(238, 230, 235)),
        6 => (srgb(150, 165, 180), srgb(215, 220, 225)),
        7 | 8 => (srgb(60, 140, 225), srgb(200, 230, 250)),
        _ => (srgb(120, 165, 215), srgb(245, 225, 200)),
    };
    for j in 0..h as i32 {
        let t = j as f32 / h;
        let col = [0, 1, 2].map(|i| top[i] + (bottom[i] - top[i]) * t);
        c.rect(x, y + j as f32, w, 1.0, [col[0], col[1], col[2], 1.0]);
    }
    // hills and paddies along the bottom
    let hill = match month {
        12 | 1 | 2 => srgb(235, 238, 242),
        3..=5 => srgb(120, 160, 90),
        6..=8 => srgb(70, 130, 60),
        9 | 10 => srgb(200, 160, 70),
        _ => srgb(150, 110, 70),
    };
    for i in 0..w as i32 {
        let fx = i as f32 / w;
        let hh = h * (0.22 + 0.06 * (fx * 7.0).sin() + 0.04 * (fx * 17.0 + 1.0).sin());
        c.rect(x + i as f32, y + h - hh, 1.0, hh, hill);
    }
    let mut seed = month * 7919;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % 10000) as f32 / 10000.0
    };
    match month {
        3 | 4 => {
            // a cherry branch with blossom
            c.line(x + w * 0.02, y + h * 0.1, x + w * 0.55, y + h * 0.32, 10.0, srgb(90, 60, 50));
            c.line(x + w * 0.3, y + h * 0.22, x + w * 0.42, y + h * 0.05, 6.0, srgb(90, 60, 50));
            for _ in 0..90 {
                let (px, py) = (x + w * (0.05 + 0.55 * rnd()), y + h * (0.02 + 0.4 * rnd()));
                let r = 6.0 + 8.0 * rnd();
                c.ring(px, py, r * 0.5, r * 0.5, r, srgb(248, 196, 210));
            }
            for _ in 0..30 {
                let (px, py) = (x + w * rnd(), y + h * (0.3 + 0.6 * rnd()));
                c.ring(px, py, 2.0, 1.5, 3.0, srgb(250, 205, 215));
            }
        }
        5 => {
            // carp streamers on a pole
            c.line(x + w * 0.8, y + h * 0.05, x + w * 0.8, y + h * 0.85, 6.0, srgb(110, 90, 70));
            for (k, col) in [srgb(40, 50, 70), srgb(200, 50, 50), srgb(60, 110, 200)].iter().enumerate() {
                let yy = y + h * (0.12 + 0.13 * k as f32);
                for s in 0..20 {
                    let t = s as f32 / 20.0;
                    c.line(x + w * (0.79 - 0.32 * t), yy + (t * 9.0).sin() * 6.0, x + w * (0.79 - 0.32 * (t + 0.05)), yy + ((t + 0.05) * 9.0).sin() * 6.0, 34.0 * (1.0 - 0.4 * t), *col);
                }
            }
        }
        6 => {
            for _ in 0..140 {
                let (px, py) = (x + w * rnd(), y + h * rnd() * 0.8);
                c.line(px, py, px - 6.0, py + 26.0, 2.0, srgb(235, 240, 245));
            }
            for _ in 0..60 {
                let (px, py) = (x + w * (0.6 + 0.35 * rnd()), y + h * (0.6 + 0.3 * rnd()));
                c.ring(px, py, 5.0, 5.0, 9.0, if rnd() < 0.5 { srgb(120, 120, 210) } else { srgb(170, 120, 200) });
            }
        }
        7 | 8 => {
            // big summer clouds and sunflowers
            for k in 0..5 {
                let (px, py) = (x + w * (0.15 + 0.16 * k as f32), y + h * (0.28 + 0.06 * (k % 2) as f32));
                c.ring(px, py, 26.0, 22.0, 46.0, srgb(250, 250, 250));
            }
            for k in 0..6 {
                let px = x + w * (0.08 + 0.15 * k as f32);
                let py = y + h * (0.62 + 0.05 * (k % 3) as f32);
                c.line(px, py, px, y + h, 6.0, srgb(70, 120, 50));
                c.ring(px, py, 16.0, 16.0, 22.0, srgb(250, 200, 40));
                c.ring(px, py, 5.0, 5.0, 10.0, srgb(110, 70, 40));
            }
        }
        9..=11 => {
            for _ in 0..70 {
                let (px, py) = (x + w * rnd(), y + h * rnd() * 0.85);
                let col = if rnd() < 0.5 { srgb(220, 80, 40) } else { srgb(240, 160, 40) };
                c.ring(px, py, 6.0, 4.0, 9.0, col);
            }
            if month == 11 {
                for k in 0..8 {
                    c.ring(x + w * (0.65 + 0.03 * k as f32), y + h * (0.15 + 0.05 * (k % 3) as f32), 10.0, 10.0, 18.0, srgb(240, 130, 30));
                }
            }
        }
        _ => {
            for _ in 0..180 {
                let (px, py) = (x + w * rnd(), y + h * rnd());
                c.ring(px, py, 2.5, 2.5, 4.0, srgb(255, 255, 255));
            }
        }
    }
}

/// The page for a month. `today`: ringed if it's in this month. `plans`: what you've written
/// in (date, text).
pub fn month(year: i32, month: u32, today: Option<Date>, plans: &[(Date, String)]) -> Canvas {
    let paper = srgb(250, 248, 242);
    let ink = srgb(40, 40, 46);
    let red = srgb(200, 45, 45);
    let blue = srgb(40, 80, 170);
    let pencil = srgb(55, 60, 80);
    let mut c = Canvas::new(W, H, paper);
    // the hole it hangs from
    c.ring(W as f32 / 2.0, 22.0, 10.0, 10.0, 6.0, srgb(180, 180, 175));
    picture(&mut c, month, 40.0, 50.0, W as f32 - 80.0, 520.0);
    // the month
    let mi = (month - 1) as usize;
    c.text(&month.to_string(), 60.0, 720.0, Font::Bold, 150.0, ink, 0.0);
    c.text(MONTHS[mi], 210.0, 650.0, Font::Bold, 46.0, ink, 0.0);
    c.text(&format!("{}  {year}  令和{}年", OLD_NAMES[mi], year - 2018), 212.0, 706.0, Font::Regular, 30.0, ink, 0.0);
    // the grid: Sunday first
    let gx = 40.0;
    let gy = 760.0;
    let cw = (W as f32 - 80.0) / 7.0;
    // (five rows when the month fits in five weeks, six when it needs them)
    let lead0 = (Date::new(year, month, 1).weekday().index() + 1) % 7;
    let rows = (lead0 + days_in(year, month)).div_ceil(7);
    let ch = (H as f32 - gy - 70.0) / rows as f32;
    let heads = [("日", "SUN"), ("月", "MON"), ("火", "TUE"), ("水", "WED"), ("木", "THU"), ("金", "FRI"), ("土", "SAT")];
    for (i, (k, e)) in heads.iter().enumerate() {
        let col = if i == 0 { red } else if i == 6 { blue } else { ink };
        c.text(&format!("{k} {e}"), gx + cw * (i as f32 + 0.5), gy - 16.0, Font::Bold, 24.0, col, 0.5);
    }
    let first = Date::new(year, month, 1);
    let lead = (first.weekday().index() + 1) % 7; // (Monday = 0 in the game; Sunday first here)
    let n = days_in(year, month);
    for row in 0..=rows {
        c.line(gx, gy + ch * row as f32, gx + cw * 7.0, gy + ch * row as f32, 2.0, srgb(190, 190, 185));
    }
    for col in 0..=7 {
        c.line(gx + cw * col as f32, gy, gx + cw * col as f32, gy + ch * rows as f32, 2.0, srgb(190, 190, 185));
    }
    for d in 1..=n {
        let slot = lead + d - 1;
        let (col, row) = ((slot % 7) as f32, (slot / 7) as f32);
        if row >= rows as f32 {
            continue;
        }
        let date = Date::new(year, month, d);
        let (x, y) = (gx + cw * col, gy + ch * row);
        let hol = holiday(date);
        let wd = date.weekday();
        if school_break(date).is_some() {
            c.rect(x + 1.0, y + 1.0, cw - 2.0, ch - 2.0, srgb(238, 236, 226));
        }
        let colr = if hol.is_some() || wd == Weekday::Sun { red } else if wd == Weekday::Sat { blue } else { ink };
        c.text(&d.to_string(), x + 10.0, y + 46.0, Font::Bold, 40.0, colr, 0.0);
        // what's on: printed events, then your plans in pencil, as many lines as the square holds
        let mut lines: Vec<(String, Font, f32, [f32; 4])> = Vec::new();
        if let Some(h) = hol {
            lines.push((h.to_string(), Font::Regular, 15.0, red));
        }
        for e in events_on(date).iter().filter_map(short) {
            lines.push((e, Font::Bold, 15.0, ink));
        }
        for (_, text) in plans.iter().filter(|p| p.0 == date) {
            lines.push((text.clone(), Font::Regular, 17.0, pencil));
        }
        let mut line_y = y + 70.0;
        let bottom = y + ch - 6.0;
        for (k, (text, font, size, colr)) in lines.iter().enumerate() {
            // (wrap into the square's width; stop at its bottom with a "+n")
            let mut rows: Vec<String> = Vec::new();
            let mut cur = String::new();
            for word in text.split_inclusive(' ') {
                let t = format!("{cur}{word}");
                if c.measure(t.trim_end(), *font, *size) > cw - 14.0 && !cur.is_empty() {
                    rows.push(cur.trim_end().into());
                    cur = word.into();
                } else {
                    cur = t;
                }
            }
            rows.push(cur.trim_end().into());
            for r in rows {
                if line_y > bottom {
                    let more = lines.len() - k;
                    c.text(&format!("+{more}"), x + cw - 10.0, y + ch - 8.0, Font::Bold, 15.0, pencil, 1.0);
                    break;
                }
                c.text(&r, x + 8.0, line_y, *font, *size, *colr, 0.0);
                line_y += size + 3.0;
            }
            if line_y > bottom {
                break;
            }
        }
        if today == Some(date) {
            c.ring(x + 34.0, y + 32.0, 30.0, 26.0, 4.0, srgb(210, 60, 60));
        }
    }
    c.grain(year as u32 * 100 + month, 0.012);
    c
}
