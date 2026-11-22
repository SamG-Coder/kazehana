//! The duel playmat (docs/BATTLE_DESIGN.md): a 640 x 500 mm neoprene mat with each player's
//! three monster zones, two trap zones, Leader, deck and discard, and a type wheel as a reminder.
//! The near player is at the bottom of the image; the far half is the same layout turned 180°.
//!
//! Zone centres (mm from the mat centre, +y towards the near player) are also written to
//! playmat.json for tools/duel.

use crate::canvas::{write_rgb, Mask};
use crate::math::*;
use crate::sdf;
use crate::text::{self, Fonts};
use rayon::prelude::*;
use std::path::Path;
use tcg::Attribute;

pub const W_MM: f32 = 640.0;
pub const H_MM: f32 = 500.0;
const PX: f32 = 6.0;
/// Zone frame (a card is 63 x 88).
pub const ZONE: (f32, f32) = (74.0, 100.0);
pub const MONSTER_X: [f32; 3] = [-92.0, 0.0, 92.0];
pub const MONSTER_Y: f32 = 66.0;
pub const BACK_Y: f32 = 184.0;
/// Back row: discard, trap 1, Leader, trap 2, deck.
pub const BACK_X: [f32; 5] = [-232.0, -116.0, 0.0, 116.0, 232.0];

struct Label {
    m: Mask,
}

fn label(fonts: &Fonts, font: &fontdue::Font, size_mm: f32, s: &str, track: f32, cx: f32, base_y: f32) -> Label {
    let size = size_mm * PX;
    let w = text::measure(font, size, s, track * PX);
    let _ = fonts;
    // masks live in "half" pixel space: origin at the mat centre, +y towards the near edge
    Label { m: text::line(font, size, s, track * PX, cx * PX - w * 0.5, base_y * PX, 1.0) }
}

pub fn render(dir: &Path, fonts: &Fonts, game: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let (w, h) = ((W_MM * PX) as usize, (H_MM * PX) as usize);
    let zones: Vec<(f32, f32, &str)> = MONSTER_X
        .iter()
        .map(|&x| (x, MONSTER_Y, "MONSTER"))
        .chain(BACK_X.iter().zip(["DISCARD", "TRAP", "LEADER", "TRAP", "DECK"]).map(|(&x, n)| (x, BACK_Y, n)))
        .collect();
    let mut labels: Vec<Label> = zones.iter().map(|(x, y, n)| label(fonts, &fonts.label, 4.2, n, 1.2, *x, *y + ZONE.1 * 0.5 - 6.0)).collect();
    labels.push(label(fonts, &fonts.logo, 11.0, &game.to_uppercase(), 3.0, 0.0, 129.0));
    // type wheel captions
    let wheel = (-236.0f32, 70.0f32);
    labels.push(label(fonts, &fonts.label, 3.6, "STRONG AGAINST  +2", 0.6, wheel.0, wheel.1 + 52.0));
    let wl = labels.len();
    let ring = [Attribute::Tide, Attribute::Flame, Attribute::Terra, Attribute::Gale];
    let ring_pos: Vec<(f32, f32)> = (0..4)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::FRAC_PI_2;
            (wheel.0 + a.cos() * 26.0, wheel.1 - 8.0 + a.sin() * 26.0)
        })
        .collect();
    for (i, at) in ring.iter().enumerate() {
        let (x, y) = ring_pos[i];
        labels.push(label(fonts, &fonts.label, 2.6, &at.to_string().to_uppercase(), 0.3, x, y + 1.0));
    }
    let pair = [(wheel.0 - 15.0, wheel.1 + 36.0, Attribute::Radiant), (wheel.0 + 15.0, wheel.1 + 36.0, Attribute::Umbral)];
    for (x, y, at) in pair {
        labels.push(label(fonts, &fonts.label, 2.4, &at.to_string().to_uppercase(), 0.2, x, y + 1.0));
    }
    let _ = wl;
    let col = |a: Attribute| srgb8(a.color());

    let px: Vec<(V3, f32, f32)> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (ix, iy) = (i % w, i / w);
            let gx = (ix as f32 + 0.5) / PX - W_MM * 0.5;
            let gy = (iy as f32 + 0.5) / PX - H_MM * 0.5;
            // fold the far half onto the near one (turned 180°)
            let (x, y, far) = if gy >= 0.0 { (gx, gy, false) } else { (-gx, -gy, true) };
            let p = (x, y);
            // neoprene: deep slate with a soft vignette and a faint wave pattern
            let r = ((gx / (W_MM * 0.5)).powi(2) + (gy / (H_MM * 0.5)).powi(2)).sqrt();
            let mut c = hex(0x1b2233).lerp(hex(0x0c1019), smoothstep(0.2, 1.2, r));
            let wave = ((x * 0.18 + (y * 0.05).sin() * 4.0).sin() * 0.5 + 0.5).powf(18.0);
            c = c.lerp(hex(0x2a3550), wave * 0.12);
            let grain = (vnoise(gx * 2.3, gy * 2.3, 3) - 0.5) * 0.05;
            c = c * (1.0 + grain);
            let mut ink = 0.0f32;
            let gold = V3(0.78, 0.62, 0.36);
            let side_tint = if far { hex(0xb85a4a) } else { hex(0x4a8ab8) };
            // centre line and the half tint
            c = c.lerp(side_tint * 0.35, 0.10 * (1.0 - smoothstep(0.0, 160.0, y)));
            let cl = sdf::cov_soft(gy.abs() - 0.5, 0.4);
            c = c.lerp(gold, cl * 0.55);
            ink = ink.max(cl);
            // zones
            for (zx, zy, _) in &zones {
                let d = sdf::rrect(p, zx - ZONE.0 * 0.5, zy - ZONE.1 * 0.5, zx + ZONE.0 * 0.5, zy + ZONE.1 * 0.5, 4.0);
                let fill = sdf::cov_soft(d, 0.4);
                c = c.lerp(hex(0x0a0d15), fill * 0.45);
                let frame = sdf::cov_soft(d.abs() - 0.6, 0.35);
                let inner = sdf::cov_soft((d + 3.0).abs() - 0.25, 0.3);
                c = c.lerp(side_tint.lerp(gold, 0.55), frame * 0.85);
                c = c.lerp(gold * 0.8, inner * 0.35);
                ink = ink.max(frame);
            }
            // type wheel (near half only shows it once per player; drawn in both halves)
            let wd = sdf::circle(p, (wheel.0, wheel.1 - 8.0), 26.0);
            c = c.lerp(gold * 0.6, sdf::cov_soft(wd.abs() - 0.35, 0.3) * 0.6);
            for i in 0..4 {
                let (ax, ay) = ring_pos[i];
                let (bx, by) = ring_pos[(i + 1) % 4];
                // arrowhead near b
                let dir = ((bx - ax), (by - ay));
                let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
                let (ux, uy) = (dir.0 / len, dir.1 / len);
                let tip = (bx - ux * 9.5, by - uy * 9.5);
                let tri = [tip, (tip.0 - ux * 4.0 - uy * 2.4, tip.1 - uy * 4.0 + ux * 2.4), (tip.0 - ux * 4.0 + uy * 2.4, tip.1 - uy * 4.0 - ux * 2.4)];
                c = c.lerp(gold, sdf::cov_soft(sdf::poly(p, &tri), 0.3));
            }
            for (i, at) in ring.iter().enumerate() {
                let (x0, y0) = ring_pos[i];
                let d = sdf::circle(p, (x0, y0 - 1.0), 8.0);
                c = c.lerp(col(*at) * 0.85, sdf::cov_soft(d, 0.3));
                c = c.lerp(gold, sdf::cov_soft(d.abs() - 0.35, 0.3));
            }
            for (x0, y0, at) in pair {
                let d = sdf::circle(p, (x0, y0 - 1.0), 7.0);
                c = c.lerp(col(at) * 0.85, sdf::cov_soft(d, 0.3));
                c = c.lerp(gold, sdf::cov_soft(d.abs() - 0.35, 0.3));
            }
            let dd = sdf::seg(p, (pair[0].0 + 8.0, pair[0].1 - 1.0), (pair[1].0 - 8.0, pair[1].1 - 1.0));
            c = c.lerp(gold, sdf::cov_soft(dd - 0.4, 0.3));
            // text
            let (hx, hy) = (x * PX, y * PX);
            for (k, l) in labels.iter().enumerate() {
                let v = l.m.bilinear(hx, hy);
                if v > 0.0 {
                    let tc = if k < zones.len() { gold * 0.9 } else if k == zones.len() { gold } else { V3::splat(0.92) };
                    c = c.lerp(tc, v * if k == zones.len() { 0.6 } else { 0.9 });
                    ink = ink.max(v);
                }
            }
            let rough = mix(0.88, 0.62, ink);
            (c, rough, ink)
        })
        .collect();
    let base: Vec<u8> = px.iter().flat_map(|p| [(to_srgb(p.0 .0.clamp(0.0, 1.0)) * 255.0) as u8, (to_srgb(p.0 .1.clamp(0.0, 1.0)) * 255.0) as u8, (to_srgb(p.0 .2.clamp(0.0, 1.0)) * 255.0) as u8]).collect();
    write_rgb(&dir.join("base.png"), w, h, &base);
    // neoprene micro texture + very slightly raised ink
    let height: Vec<f32> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = ((i % w) as f32 / PX, (i / w) as f32 / PX);
            (vnoise(x * 3.0, y * 3.0, 17) - 0.5) * 0.02 + px[i].2 * 0.01
        })
        .collect();
    let hh = |x: isize, y: isize| height[(y.clamp(0, h as isize - 1) as usize) * w + x.clamp(0, w as isize - 1) as usize];
    let enc = |v: f32| ((v * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let nrm: Vec<u8> = (0..w * h)
        .into_par_iter()
        .flat_map_iter(|i| {
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            let dx = (hh(x + 1, y) - hh(x - 1, y)) * PX * 0.5;
            let dy = (hh(x, y + 1) - hh(x, y - 1)) * PX * 0.5;
            let n = V3(-dx, -dy, 1.0).norm();
            [enc(n.0), enc(n.1), enc(n.2)]
        })
        .collect();
    write_rgb(&dir.join("normal.png"), w, h, &nrm);
    let orm: Vec<u8> = px.iter().flat_map(|p| [255u8, (p.1 * 255.0) as u8, 0u8]).collect();
    write_rgb(&dir.join("orm.png"), w, h, &orm);
    let json = format!(
        "{{\n  \"size_mm\": [{W_MM}, {H_MM}],\n  \"zone_mm\": [{}, {}],\n  \"monster_x_mm\": {:?},\n  \"monster_y_mm\": {MONSTER_Y},\n  \"back_x_mm\": {:?},\n  \"back_y_mm\": {BACK_Y},\n  \"back_order\": [\"discard\", \"trap\", \"leader\", \"trap\", \"deck\"],\n  \"note\": \"+y is towards the near player; the far player's zones are the same turned 180 degrees\"\n}}\n",
        ZONE.0, ZONE.1, MONSTER_X, BACK_X
    );
    std::fs::write(dir.join("playmat.json"), json).unwrap();
    println!("wrote {}", dir.display());
}
