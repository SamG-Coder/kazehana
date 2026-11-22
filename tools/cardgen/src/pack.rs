//! Booster pack wrappers: generic, layered flow-wrap print files for every pack format.
//!
//! A booster pack is a flow-wrapped pouch of metallised film: one sheet folded around the cards,
//! sealed by a fin seal down the middle of the back and by serrated crimp seals at the top and
//! bottom. Its print file (the "net") is therefore laid out, left to right, as
//!
//!   fin flap A | back, right half | FRONT | back, left half | fin flap E
//!
//! with the crimps across the top and bottom. The tear notch is cut into both side folds just
//! under the top crimp; tearing across there takes the top strip off and opens the pack.
//!
//! Every format gets the same layer stack (docs/CARD_TEMPLATES.md, "Booster packs"):
//! base / metal / height / normal / orm for the engine, plus guides (dieline), template
//! (base + guides) and flat previews. A finished graphic dropped into assets/cards/packs/art/
//! replaces the generic design without touching the film, crimps or guides.

use crate::canvas::{write_gray16, write_rgb, write_rgba, Mask};
use crate::math::*;
use crate::sdf;
use crate::text::{self, Fonts};
use rayon::prelude::*;
use std::path::Path;

/// Print resolution: 16 px/mm (406 dpi).
pub const PX_MM: f32 = 16.0;

/// A pack format: physical wrapper size and contents. Sizes are measured retail wrappers
/// (66 mm wide; 115-135 mm long; 2.5-4 mm thick when full), see the doc for sources.
#[derive(Clone, Copy, Debug)]
pub struct Format {
    pub slug: &'static str,
    pub label: &'static str,
    pub cards: u32,
    /// Packs in one sealed display box.
    pub box_packs: u32,
    /// Flat wrapper width, length and full thickness (mm).
    pub w: f32,
    pub l: f32,
    pub d: f32,
    /// Crimp seal depth at the top and bottom (mm).
    pub crimp: f32,
    /// Fin-seal flap width (mm, each side).
    pub fin: f32,
    pub note: &'static str,
}

impl Format {
    /// Tear line: just under the top crimp (mm from the top edge).
    pub fn tear_y(&self) -> f32 {
        self.crimp + 1.8
    }
    /// Where the card stack sits (mm from the top edge, top and bottom of the cards).
    pub fn cards_y(&self) -> (f32, f32) {
        let bottom = self.l - self.crimp - 2.5;
        (bottom - 88.0, bottom)
    }
    pub fn net_mm(&self) -> (f32, f32) {
        (2.0 * self.w + 2.0 * self.fin, self.l)
    }
    pub fn net_px(&self) -> (usize, usize) {
        let (w, h) = self.net_mm();
        ((w * PX_MM).round() as usize, (h * PX_MM).round() as usize)
    }
}

pub const FORMATS: [Format; 4] = [
    Format {
        slug: "booster6",
        label: "Mythbeasts booster (6 cards)",
        cards: 6,
        box_packs: 20,
        w: 66.0,
        l: 128.0,
        d: 3.0,
        crimp: 11.0,
        fin: 8.0,
        note: "The game's own pack: crates/tcg PACK_SIZE 6 (4 C, 1 U, 1 R/M, foil 1 in 6), PACKS_PER_BOX 20.",
    },
    Format {
        slug: "jp5",
        label: "Japanese-style mini (5 cards)",
        cards: 5,
        box_packs: 30,
        w: 66.0,
        l: 135.0,
        d: 2.5,
        crimp: 12.0,
        fin: 8.0,
        note: "Japanese expansion pack: 5 cards, 30 packs per box, long wrapper with a tall headroom.",
    },
    Format {
        slug: "retail10",
        label: "Western retail (10 cards)",
        cards: 10,
        box_packs: 36,
        w: 66.0,
        l: 120.0,
        d: 4.0,
        crimp: 10.0,
        fin: 8.0,
        note: "Western booster: 10 cards (+ a code card), 36 packs per display box.",
    },
    Format {
        slug: "premium11",
        label: "Premium / high-class (11 cards)",
        cards: 11,
        box_packs: 10,
        w: 70.0,
        l: 135.0,
        d: 3.5,
        crimp: 12.0,
        fin: 8.0,
        note: "Premium pack: wider wrapper, 10-11 cards, 10 packs per box, every card foil.",
    },
];

/// Where a point of the net lands on the pack.
#[derive(Clone, Copy, PartialEq)]
enum Region {
    FinA,
    /// Back panel, in back-view coordinates (bx from the viewer's left, mm).
    Back(f32),
    /// Front panel (u from the left, mm).
    Front(f32),
    FinE(f32),
}

fn region(f: &Format, n: f32) -> Region {
    let (w, fin) = (f.w, f.fin);
    if n < fin {
        Region::FinA
    } else if n < fin + w * 0.5 {
        Region::Back(w * 0.5 + n - fin)
    } else if n < fin + w * 1.5 {
        Region::Front(n - fin - w * 0.5)
    } else if n < fin + w * 2.0 {
        Region::Back(n - fin - w * 1.5)
    } else {
        Region::FinE(n - fin - 2.0 * w)
    }
}

/// Net x (mm) of a back-view coordinate.
pub fn back_to_net(f: &Format, bx: f32) -> f32 {
    if bx < f.w * 0.5 {
        f.fin + 1.5 * f.w + bx
    } else {
        f.fin + bx - 0.5 * f.w
    }
}

/// The generic design's zones (front-panel mm: x0, y0, x1, y1).
pub struct Zones {
    pub logo: (f32, f32, f32, f32),
    pub art: (f32, f32, f32, f32),
    pub badge: (f32, f32, f32),
    pub footer: (f32, f32, f32, f32),
    /// Back view: copy block, barcode, icons row.
    pub info: (f32, f32, f32, f32),
    pub barcode: (f32, f32, f32, f32),
    pub icons: (f32, f32, f32, f32),
}

pub fn zones(f: &Format) -> Zones {
    let (w, l, c) = (f.w, f.l, f.crimp);
    let top = f.tear_y() + 3.0;
    let bottom = l - c - 3.0;
    let inner = bottom - top;
    let logo_h = (inner * 0.17).clamp(14.0, 22.0);
    let logo = (5.0, top, w - 5.0, top + logo_h);
    let foot_h = (inner * 0.11).clamp(9.0, 13.0);
    let footer = (5.0, bottom - foot_h, w - 5.0, bottom);
    let art = (5.0, logo.3 + 2.0, w - 5.0, footer.1 - 2.0);
    let badge = (w - 12.5, art.3 - 9.0, 7.5);
    let info = (7.0, top + 4.0, w - 7.0, top + 4.0 + inner * 0.42);
    let barcode = (w - 7.0 - 26.0, bottom - 18.0, w - 7.0, bottom - 3.0);
    let icons = (7.0, bottom - 15.0, 7.0 + 24.0, bottom - 6.0);
    Zones { logo, art, badge, footer, info, barcode, icons }
}

/// Layers of one wrapper.
pub struct Wrapper {
    pub w: usize,
    pub h: usize,
    /// Linear albedo.
    pub base: Vec<V3>,
    /// 1 = bare metallised film (or metallic ink), 0 = opaque ink.
    pub metal: Vec<f32>,
    pub rough: Vec<f32>,
    /// Surface relief (mm).
    pub height: Vec<f32>,
    /// Dieline overlay (straight RGBA).
    pub guides: Vec<[f32; 4]>,
}

/// One pixel of design: ink colour and coverage, metal tint where the film shows.
#[derive(Clone, Copy)]
struct Ink {
    col: V3,
    /// Ink coverage (0 = film shows).
    a: f32,
    /// Tint of the bare film / metallic ink.
    tint: V3,
    rough: f32,
}

/// Text masks, in panel px (front or back view).
struct Texts {
    front: Vec<(Mask, V3, bool)>,
    back: Vec<(Mask, V3, bool)>,
}

fn centred(fonts: &Fonts, font: &fontdue::Font, size_mm: f32, s: &str, track: f32, cx_mm: f32, base_mm: f32, max_w_mm: f32) -> Mask {
    let k = PX_MM;
    let (size, sx) = text::fit(font, size_mm * k, size_mm * k * 0.5, s, track * k, max_w_mm * k);
    let w = text::measure(font, size, s, track * k) * sx;
    let _ = fonts;
    text::line(font, size, s, track * k, cx_mm * k - w * 0.5, base_mm * k, sx)
}

fn texts(f: &Format, z: &Zones, fonts: &Fonts, game: &str) -> Texts {
    let gold = V3(1.0, 0.78, 0.42);
    let white = V3(0.92, 0.92, 0.94);
    let mut front = Vec::new();
    let lw = z.logo.2 - z.logo.0;
    let lh = z.logo.3 - z.logo.1;
    front.push((centred(fonts, &fonts.logo, lh * 0.52, &game.to_uppercase(), 0.35, f.w * 0.5, z.logo.1 + lh * 0.66, lw - 4.0), gold, true));
    front.push((centred(fonts, &fonts.jp, lh * 0.17, "ミスビースト", 0.6, f.w * 0.5, z.logo.1 + lh * 0.93, lw), white, false));
    let fh = z.footer.3 - z.footer.1;
    front.push((centred(fonts, &fonts.label, fh * 0.42, "BOOSTER PACK", 0.7, f.w * 0.5, z.footer.1 + fh * 0.55, lw - 6.0), white, false));
    front.push((centred(fonts, &fonts.fine, fh * 0.22, "SET NAME  \u{00b7}  EXPANSION", 0.35, f.w * 0.5, z.footer.1 + fh * 0.9, lw), gold, true));
    let (bx, by, br) = z.badge;
    front.push((centred(fonts, &fonts.name, br * 0.95, &f.cards.to_string(), 0.0, bx, by + br * 0.22, br * 1.6), white, false));
    front.push((centred(fonts, &fonts.label, br * 0.3, "CARDS", 0.25, bx, by + br * 0.62, br * 1.5), white, false));
    // tear markers
    let ty = f.tear_y();
    front.push((centred(fonts, &fonts.label, 1.7, "TEAR", 0.25, 6.6, ty - 1.4, 8.0), white, false));
    front.push((centred(fonts, &fonts.label, 1.7, "TEAR", 0.25, f.w - 6.6, ty - 1.4, 8.0), white, false));

    let mut back = Vec::new();
    let iw = z.info.2 - z.info.0;
    back.push((centred(fonts, &fonts.logo, 4.2, &game.to_uppercase(), 0.25, f.w * 0.5, z.info.1 + 5.0, iw), gold, true));
    back.push((centred(fonts, &fonts.fine, 2.0, &format!("{} CARDS PER PACK  \u{00b7}  TRADING CARD GAME", f.cards), 0.2, f.w * 0.5, z.info.1 + 9.0, iw), white, false));
    Texts { front, back }
}

fn text_at(list: &[(Mask, V3, bool)], x_mm: f32, y_mm: f32) -> Option<(f32, V3, bool)> {
    let (px, py) = (x_mm * PX_MM, y_mm * PX_MM);
    let mut best: Option<(f32, V3, bool)> = None;
    for (m, c, metal) in list {
        let v = m.bilinear(px, py);
        if v > 0.003 {
            best = Some(match best {
                Some((a, ca, ma)) if a > v => (a, ca, ma),
                _ => (v, *c, *metal),
            });
        }
    }
    best
}

fn over(dst: Ink, col: V3, a: f32) -> Ink {
    Ink { col: dst.col.lerp(col, a), a: mix(dst.a, 1.0, a), ..dst }
}

/// Metallic ink (toner over the film): the film shows through, tinted.
fn metallic(dst: Ink, tint: V3, a: f32, rough: f32) -> Ink {
    Ink { a: dst.a * (1.0 - a), tint: dst.tint.lerp(tint, a), rough: mix(dst.rough, rough, a), ..dst }
}

const SILVER: V3 = V3(0.93, 0.93, 0.95);

/// The generic front design at panel mm (u, v).
fn front_ink(f: &Format, z: &Zones, t: &Texts, art: Option<&Graphic>, u: f32, v: f32) -> Ink {
    let g = (v / f.l).clamp(0.0, 1.0);
    let ground = hex(0x161c38).lerp(hex(0x090b15), g * 0.9);
    let mut ink = Ink { col: ground, a: 1.0, tint: SILVER, rough: 0.3 };
    let p = (u, v);
    // Sunburst behind the logo: thin metal rays.
    let (lcx, lcy) = ((z.logo.0 + z.logo.2) * 0.5, (z.logo.1 + z.logo.3) * 0.5);
    let ang = (v - lcy).atan2(u - lcx);
    let r = ((u - lcx).powi(2) + (v - lcy).powi(2)).sqrt();
    let ray = (0.5 + 0.5 * (ang * 36.0).cos()).powf(6.0);
    let fall = (1.0 - smoothstep(4.0, 44.0, r)) * smoothstep(1.0, 6.0, r);
    ink = metallic(ink, SILVER, ray * fall * 0.55, 0.16);
    // Art window: flat placeholder colour, thin metal frame.
    let d_art = sdf::rrect(p, z.art.0, z.art.1, z.art.2, z.art.3, 3.0);
    let frame = sdf::cov_soft(d_art.abs() - 0.35, 0.06);
    if d_art < 0.0 {
        let fill = match art {
            Some(gx) => gx.sample(u, v),
            None => Some((hex(0x2f4a78).lerp(hex(0x223660), g), 1.0)),
        };
        if let Some((c, a)) = fill {
            ink = over(ink, c, a * sdf::cov_soft(d_art, 0.06));
        }
    }
    ink = metallic(ink, V3(1.0, 0.8, 0.46), frame, 0.18);
    // Safe-area pinstripe frame (metal).
    let d_safe = sdf::rrect(p, 2.6, f.tear_y() + 1.2, f.w - 2.6, f.l - f.crimp - 1.2, 1.5);
    ink = metallic(ink, V3(1.0, 0.8, 0.46), sdf::cov_soft(d_safe.abs() - 0.15, 0.05) * 0.85, 0.2);
    // Badge: metal ring, ink centre.
    let (bx, by, br) = z.badge;
    let db = sdf::circle(p, (bx, by), br);
    ink = over(ink, hex(0x101430), sdf::cov_soft(db, 0.06));
    ink = metallic(ink, V3(1.0, 0.8, 0.46), sdf::cov_soft(db.abs() - 0.45, 0.06), 0.16);
    // Footer band.
    let df = sdf::rrect(p, z.footer.0, z.footer.1, z.footer.2, z.footer.3, 1.2);
    ink = over(ink, hex(0x0c0f20), sdf::cov_soft(df, 0.06) * 0.85);
    // Crimp bands: metal edge lines.
    for y in [f.crimp - 0.6, f.l - f.crimp + 0.6] {
        ink = metallic(ink, SILVER, sdf::cov_soft((v - y).abs() - 0.25, 0.05), 0.14);
    }
    // Tear notch markers: small inward triangles.
    let ty = f.tear_y();
    for (ex, s) in [(0.0f32, 1.0f32), (f.w, -1.0)] {
        let tri = [(ex, ty - 1.3), (ex + s * 2.4, ty), (ex, ty + 1.3)];
        let d = sdf::poly(p, &tri);
        ink = metallic(ink, V3(1.0, 0.8, 0.46), sdf::cov_soft(d, 0.05), 0.16);
    }
    if let Some((a, c, m)) = text_at(&t.front, u, v) {
        ink = if m { metallic(ink, c, a, 0.18) } else { over(ink, c, a) };
    }
    ink
}

/// The generic back design at back-view mm (bx, v).
fn back_ink(f: &Format, z: &Zones, t: &Texts, u: f32, v: f32) -> Ink {
    let g = (v / f.l).clamp(0.0, 1.0);
    let ground = hex(0x141a34).lerp(hex(0x090b15), g * 0.9);
    let mut ink = Ink { col: ground, a: 1.0, tint: SILVER, rough: 0.3 };
    let p = (u, v);
    // Fine metal guilloche across the panel.
    let wv = ((u * 1.9 + (v * 0.21).sin() * 3.0).sin() * 0.5 + 0.5).powf(14.0);
    ink = metallic(ink, SILVER, wv * 0.25 * smoothstep(f.crimp, f.crimp + 10.0, v) * (1.0 - smoothstep(f.l - f.crimp - 10.0, f.l - f.crimp, v)), 0.16);
    // Copy block placeholder: panel + text bars.
    let di = sdf::rrect(p, z.info.0, z.info.1, z.info.2, z.info.3, 1.5);
    ink = over(ink, hex(0x1d2446), sdf::cov_soft(di, 0.06));
    ink = metallic(ink, V3(1.0, 0.8, 0.46), sdf::cov_soft(di.abs() - 0.18, 0.05), 0.18);
    let bars_top = z.info.1 + 12.5;
    if u > z.info.0 + 3.0 && u < z.info.2 - 3.0 && v > bars_top && v < z.info.3 - 2.5 {
        let row = ((v - bars_top) / 2.6).floor();
        let fr = (v - bars_top) / 2.6 - row;
        let len = 0.55 + 0.4 * hash2f(row as i32, 3, 77);
        let x1 = z.info.0 + 3.0 + (z.info.2 - z.info.0 - 6.0) * len;
        if fr > 0.25 && fr < 0.7 && u < x1 {
            ink = over(ink, hex(0x6a7196), 0.8);
        }
    }
    // Barcode zone: white box.
    let dbc = sdf::rrect(p, z.barcode.0, z.barcode.1, z.barcode.2, z.barcode.3, 0.8);
    ink = over(ink, V3(0.85, 0.85, 0.85), sdf::cov_soft(dbc, 0.05));
    // Icons row: three circles.
    for k in 0..3 {
        let cx = z.icons.0 + 4.0 + k as f32 * 8.0;
        let d = sdf::circle(p, (cx, (z.icons.1 + z.icons.3) * 0.5), 3.3);
        ink = metallic(ink, SILVER, sdf::cov_soft(d.abs() - 0.3, 0.05), 0.18);
    }
    for y in [f.crimp - 0.6, f.l - f.crimp + 0.6] {
        ink = metallic(ink, SILVER, sdf::cov_soft((v - y).abs() - 0.25, 0.05), 0.14);
    }
    if let Some((a, c, m)) = text_at(&t.back, u, v) {
        ink = if m { metallic(ink, c, a, 0.18) } else { over(ink, c, a) };
    }
    ink
}

/// A finished graphic for one panel (RGBA, any size; stretched to the panel).
pub struct Graphic {
    w: usize,
    h: usize,
    px: Vec<(V3, f32)>,
    /// Panel size it covers (mm).
    mm: (f32, f32),
}

impl Graphic {
    pub fn load(path: &Path, mm: (f32, f32)) -> Option<Graphic> {
        let file = std::fs::File::open(path).ok()?;
        let mut dec = png::Decoder::new(std::io::BufReader::new(file));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut r = dec.read_info().ok()?;
        let mut buf = vec![0; r.output_buffer_size()];
        let info = r.next_frame(&mut buf).ok()?;
        let ch = match info.color_type {
            png::ColorType::Rgba => 4,
            png::ColorType::Rgb => 3,
            png::ColorType::GrayscaleAlpha => 2,
            _ => 1,
        };
        let (w, h) = (info.width as usize, info.height as usize);
        let px = (0..w * h)
            .map(|i| {
                let p = &buf[i * ch..];
                let c = if ch >= 3 { srgb8([p[0], p[1], p[2]]) } else { srgb8([p[0], p[0], p[0]]) };
                let a = match ch {
                    4 => p[3] as f32 / 255.0,
                    2 => p[1] as f32 / 255.0,
                    _ => 1.0,
                };
                (c, a)
            })
            .collect();
        Some(Graphic { w, h, px, mm })
    }
    fn sample(&self, u: f32, v: f32) -> Option<(V3, f32)> {
        let x = (u / self.mm.0 * self.w as f32) as i32;
        let y = (v / self.mm.1 * self.h as f32) as i32;
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return None;
        }
        Some(self.px[y as usize * self.w + x as usize])
    }
}

/// Film relief at net mm (n, v): crimp serrations, fin knurl, a faint film orange-peel.
fn relief(f: &Format, n: f32, v: f32) -> f32 {
    let c = f.crimp;
    let mut h = (vnoise(n * 1.7, v * 1.7, 9) - 0.5) * 0.004;
    let in_crimp = (1.0 - smoothstep(c - 0.6, c - 0.1, v)) + smoothstep(f.l - c + 0.1, f.l - c + 0.6, v);
    if in_crimp > 0.0 {
        // V-ridges across the seal, 1 mm pitch, and a plain margin at the cut edge.
        let ridge = 1.0 - (fract(n / 1.0) - 0.5).abs() * 2.0;
        let edge = smoothstep(0.4, 1.2, v.min(f.l - v));
        h += ridge * 0.06 * in_crimp * edge;
    }
    if matches!(region(f, n), Region::FinA | Region::FinE(_)) {
        // diamond knurl
        let a = (fract((n + v) / 0.9) - 0.5).abs();
        let b = (fract((n - v) / 0.9) - 0.5).abs();
        h += (1.0 - (a.min(b) * 4.0).min(1.0)) * 0.035;
    }
    h
}

/// Render one format's wrapper layers.
pub fn render(f: &Format, fonts: &Fonts, game: &str, art_dir: &Path) -> Wrapper {
    let (w, h) = f.net_px();
    let z = zones(f);
    let t = texts(f, &z, fonts, game);
    let full = Graphic::load(&art_dir.join(format!("{}.png", f.slug)), f.net_mm());
    let front_g = Graphic::load(&art_dir.join(format!("{}_front.png", f.slug)), (f.w, f.l));
    let back_g = Graphic::load(&art_dir.join(format!("{}_back.png", f.slug)), (f.w, f.l));
    let art_g = Graphic::load(&art_dir.join(format!("{}_art.png", f.slug)), (f.w, f.l));
    let metal_g = Graphic::load(&art_dir.join(format!("{}_metal.png", f.slug)), f.net_mm());
    let px: Vec<(V3, f32, f32, f32)> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let n = (x as f32 + 0.5) / PX_MM;
            let v = (y as f32 + 0.5) / PX_MM;
            let mut ink = match region(f, n) {
                Region::Front(u) => {
                    let mut k = front_ink(f, &z, &t, art_g.as_ref(), u, v);
                    if let Some((c, a)) = front_g.as_ref().and_then(|g| g.sample(u, v)) {
                        k = over(k, c, a);
                    }
                    k
                }
                Region::Back(bx) => {
                    let mut k = back_ink(f, &z, &t, bx, v);
                    if let Some((c, a)) = back_g.as_ref().and_then(|g| g.sample(bx, v)) {
                        k = over(k, c, a);
                    }
                    k
                }
                Region::FinA | Region::FinE(_) => {
                    let g = (v / f.l).clamp(0.0, 1.0);
                    Ink { col: hex(0x141a34).lerp(hex(0x090b15), g * 0.9), a: 1.0, tint: SILVER, rough: 0.3 }
                }
            };
            if let Some((c, a)) = full.as_ref().and_then(|g| g.sample(n, v)) {
                ink = over(ink, c, a);
            }
            if let Some((m, _)) = metal_g.as_ref().and_then(|g| g.sample(n, v)) {
                ink = metallic(ink, SILVER, m.luma(), 0.16);
            }
            // Film: bare metallised BOPP is a soft mirror; ink is gloss-lacquered.
            let metal = 1.0 - ink.a;
            let film = ink.tint * 0.98;
            let col = film.lerp(ink.col, ink.a);
            let rough = mix(ink.rough.min(0.2), 0.32, ink.a);
            (col, metal, rough, relief(f, n, v))
        })
        .collect();
    let guides = guides(f, &z, w, h);
    Wrapper {
        w,
        h,
        base: px.iter().map(|p| p.0).collect(),
        metal: px.iter().map(|p| p.1).collect(),
        rough: px.iter().map(|p| p.2).collect(),
        height: px.iter().map(|p| p.3).collect(),
        guides,
    }
}

/// Dieline overlay: cut (magenta), folds (cyan, dashed), crimp zones (red hatch), fin seal
/// (yellow), tear line and notches (green), safe area (blue, dashed), zones (white boxes).
fn guides(f: &Format, z: &Zones, w: usize, h: usize) -> Vec<[f32; 4]> {
    let k = PX_MM;
    let line = |d: f32, wmm: f32| sdf::cov_soft(d.abs() - wmm * 0.5, 0.5 / k);
    let dash = |s: f32, period: f32| if fract(s / period) < 0.6 { 1.0 } else { 0.0 };
    (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let n = (x as f32 + 0.5) / k;
            let v = (y as f32 + 0.5) / k;
            let mut o = [0.0f32; 4];
            let mut put = |c: [f32; 3], a: f32| {
                if a > o[3] {
                    o = [c[0], c[1], c[2], a];
                }
            };
            let (nw, nh) = f.net_mm();
            // cut edge
            put([1.0, 0.1, 0.8], line(n.min(nw - n).min(v).min(nh - v), 0.25));
            // folds and the fin seal lines
            for fx in [f.fin + 0.5 * f.w, f.fin + 1.5 * f.w] {
                put([0.1, 0.9, 1.0], line(n - fx, 0.2) * dash(v, 2.0));
            }
            for fx in [f.fin, f.fin + 2.0 * f.w] {
                put([1.0, 0.9, 0.1], line(n - fx, 0.2) * dash(v, 1.2));
            }
            if n < f.fin || n > f.fin + 2.0 * f.w {
                put([1.0, 0.9, 0.1], 0.18);
            }
            // crimps
            if v < f.crimp || v > f.l - f.crimp {
                let hatch = if fract((n + v) / 2.0) < 0.18 { 0.5 } else { 0.12 };
                put([1.0, 0.15, 0.1], hatch);
            }
            // tear line and notches (at both side folds)
            let ty = f.tear_y();
            put([0.2, 1.0, 0.3], line(v - ty, 0.2) * dash(n, 1.6));
            for fx in [f.fin + 0.5 * f.w, f.fin + 1.5 * f.w] {
                let d = (n - fx).abs() * 0.9 + (v - ty).abs() - 1.2;
                put([0.2, 1.0, 0.3], sdf::cov_soft(d, 0.5 / k) * 0.85);
            }
            // safe area: 3 mm inside folds, crimps and the tear line, on the front and back
            let reg = region(f, n);
            let (pu, front) = match reg {
                Region::Front(u) => (Some(u), true),
                Region::Back(bx) => (Some(bx), false),
                _ => (None, false),
            };
            if let Some(u) = pu {
                let d = sdf::rrect((u, v), 3.0, ty + 3.0, f.w - 3.0, f.l - f.crimp - 3.0, 0.0);
                put([0.3, 0.5, 1.0], line(d, 0.2) * dash(u + v, 1.4));
                let boxes: Vec<(f32, f32, f32, f32)> = if front { vec![z.logo, z.art, z.footer] } else { vec![z.info, z.barcode, z.icons] };
                for b in boxes {
                    let d = sdf::rrect((u, v), b.0, b.1, b.2, b.3, 0.0);
                    put([1.0, 1.0, 1.0], line(d, 0.15) * 0.8);
                }
                if front {
                    let (bx, by, br) = z.badge;
                    put([1.0, 1.0, 1.0], line(sdf::circle((u, v), (bx, by), br), 0.15) * 0.8);
                }
                if !front {
                    // fin keep-out: the folded fin covers bx in [w/2 - fin, w/2]
                    if u > f.w * 0.5 - f.fin && u < f.w * 0.5 {
                        put([1.0, 0.9, 0.1], 0.14);
                    }
                }
            }
            o
        })
        .collect()
}

fn srgb_u8(c: V3) -> [u8; 3] {
    [(to_srgb(c.0.clamp(0.0, 1.0)) * 255.0 + 0.5) as u8, (to_srgb(c.1.clamp(0.0, 1.0)) * 255.0 + 0.5) as u8, (to_srgb(c.2.clamp(0.0, 1.0)) * 255.0 + 0.5) as u8]
}

/// Flat studio look of the film for previews: metal gets a soft vertical gradient reflection.
fn look(wr: &Wrapper, i: usize, sheen: f32) -> V3 {
    let m = wr.metal[i];
    let env = 0.35 + 0.65 * sheen;
    wr.base[i].lerp(wr.base[i] * env, m)
}

/// Write all layers for one format into `dir`.
pub fn save(f: &Format, wr: &Wrapper, dir: &Path, fonts: &Fonts) {
    std::fs::create_dir_all(dir).unwrap();
    let (w, h) = (wr.w, wr.h);
    let base: Vec<u8> = wr.base.iter().flat_map(|c| srgb_u8(*c)).collect();
    write_rgb(&dir.join("base.png"), w, h, &base);
    let metal: Vec<f32> = wr.metal.clone();
    let metal8: Vec<u8> = metal.iter().flat_map(|m| { let q = (m.clamp(0.0, 1.0) * 255.0 + 0.5) as u8; [q, q, q] }).collect();
    write_rgb(&dir.join("metal.png"), w, h, &metal8);
    let hmax = 0.1f32;
    write_gray16(&dir.join("height.png"), w, h, &wr.height.iter().map(|x| (x / hmax * 0.5 + 0.5).clamp(0.0, 1.0)).collect::<Vec<_>>());
    // Tangent-space normal (X = +u, Y = +v down the image), slopes in mm/mm.
    let px = 1.0 / PX_MM;
    let hh = |x: isize, y: isize| wr.height[(y.clamp(0, h as isize - 1) as usize) * w + x.clamp(0, w as isize - 1) as usize];
    let enc = |v: f32| ((v * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let nrm: Vec<u8> = (0..w * h)
        .into_par_iter()
        .flat_map_iter(|i| {
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            let dx = (hh(x + 1, y) - hh(x - 1, y)) / (2.0 * px);
            let dy = (hh(x, y + 1) - hh(x, y - 1)) / (2.0 * px);
            let n = V3(-dx, -dy, 1.0).norm();
            [enc(n.0), enc(n.1), enc(n.2)]
        })
        .collect();
    write_rgb(&dir.join("normal.png"), w, h, &nrm);
    let orm: Vec<u8> = (0..w * h).flat_map(|i| [255u8, (wr.rough[i].clamp(0.0, 1.0) * 255.0) as u8, (wr.metal[i].clamp(0.0, 1.0) * 255.0) as u8]).collect();
    write_rgb(&dir.join("orm.png"), w, h, &orm);
    let g8: Vec<u8> = wr.guides.iter().flat_map(|g| { let c = srgb_u8(V3(g[0], g[1], g[2])); [c[0], c[1], c[2], (g[3] * 255.0) as u8] }).collect();
    write_rgba(&dir.join("guides.png"), w, h, &g8);
    // template = flat look + guides
    let tpl: Vec<u8> = (0..w * h)
        .flat_map(|i| {
            let g = wr.guides[i];
            let c = look(wr, i, 0.6).lerp(V3(g[0], g[1], g[2]), g[3]);
            srgb_u8(c)
        })
        .collect();
    write_rgb(&dir.join("template.png"), w, h, &tpl);
    // flat previews of the front and back panels
    let (pw, ph) = ((f.w * PX_MM) as usize, h);
    for (name, front) in [("preview_front.png", true), ("preview_back.png", false)] {
        let mut out = Vec::with_capacity(pw * ph * 3);
        for y in 0..ph {
            let sheen = 0.5 + 0.5 * ((y as f32 / ph as f32) * 5.0 + 0.7).sin();
            for x in 0..pw {
                let u = (x as f32 + 0.5) / PX_MM;
                let n = if front { f.fin + 0.5 * f.w + u } else { back_to_net(f, u) };
                let nx = ((n * PX_MM) as usize).min(w - 1);
                out.extend_from_slice(&srgb_u8(look(wr, y * w + nx, sheen)));
            }
        }
        write_rgb(&dir.join(name), pw, ph, &out);
    }
    let z = zones(f);
    let r4 = |b: (f32, f32, f32, f32)| format!("[{:.2}, {:.2}, {:.2}, {:.2}]", b.0, b.1, b.2, b.3);
    let (nw, nh) = f.net_mm();
    let (cy0, cy1) = f.cards_y();
    let json = format!(
        "{{\n  \"slug\": \"{}\",\n  \"label\": \"{}\",\n  \"cards\": {},\n  \"box_packs\": {},\n  \"width_mm\": {:.2},\n  \"length_mm\": {:.2},\n  \"thickness_mm\": {:.2},\n  \"crimp_mm\": {:.2},\n  \"fin_mm\": {:.2},\n  \"tear_y_mm\": {:.2},\n  \"cards_top_mm\": {:.2},\n  \"cards_bottom_mm\": {:.2},\n  \"net_mm\": [{:.2}, {:.2}],\n  \"net_px\": [{}, {}],\n  \"px_per_mm\": {},\n  \"front_x_mm\": {:.2},\n  \"zones_front_mm\": {{ \"logo\": {}, \"art\": {}, \"footer\": {}, \"badge\": [{:.2}, {:.2}, {:.2}] }},\n  \"zones_back_mm\": {{ \"info\": {}, \"barcode\": {}, \"icons\": {} }},\n  \"note\": \"{}\"\n}}\n",
        f.slug, f.label, f.cards, f.box_packs, f.w, f.l, f.d, f.crimp, f.fin, f.tear_y(), cy0, cy1, nw, nh, w, h, PX_MM, f.fin + 0.5 * f.w,
        r4(z.logo), r4(z.art), r4(z.footer), z.badge.0, z.badge.1, z.badge.2, r4(z.info), r4(z.barcode), r4(z.icons), f.note
    );
    std::fs::write(dir.join("pack.json"), json).unwrap();
    let _ = fonts;
}

/// All formats side by side at true relative size (front and back), with labels.
pub fn contact(path: &Path, items: &[(Format, Wrapper)], fonts: &Fonts) {
    let s = 5.0f32; // px per mm on the sheet
    let gap = 70usize;
    let top = 120usize;
    let label_h = 110usize;
    let maxl = items.iter().map(|(f, _)| f.l).fold(0.0, f32::max);
    let tw: usize = items.iter().map(|(f, _)| (f.w * s) as usize * 2 + 24 + gap).sum::<usize>() + gap;
    let th = top + (maxl * s) as usize + label_h + gap;
    let mut img: Vec<V3> = (0..tw * th)
        .map(|i| {
            let (x, y) = ((i % tw) as f32, (i / tw) as f32);
            hex(0x1c2026) * (1.0 + (fbm(x * 0.6, y * 0.6, 2, 5) - 0.5) * 0.1)
        })
        .collect();
    let mut blit_text = |img: &mut Vec<V3>, font: &fontdue::Font, size: f32, st: &str, x: f32, y: f32, c: V3| {
        let m = text::line(font, size, st, 1.5, x, y, 1.0);
        for yy in 0..m.h {
            for xx in 0..m.w {
                let v = m.d[yy * m.w + xx];
                let (gx, gy) = (m.x0 + xx as i32, m.y0 + yy as i32);
                if v > 0.0 && gx >= 0 && gy >= 0 && (gx as usize) < tw && (gy as usize) < th {
                    let p = &mut img[gy as usize * tw + gx as usize];
                    *p = p.lerp(c, v);
                }
            }
        }
    };
    blit_text(&mut img, &fonts.label, 44.0, "BOOSTER PACK FORMATS  \u{00b7}  TRUE RELATIVE SIZE (5 px/mm)", gap as f32, 80.0, V3::splat(0.75));
    let mut x0 = gap;
    for (f, wr) in items {
        let pw = (f.w * s) as usize;
        let ph = (f.l * s) as usize;
        let y0 = top + ((maxl - f.l) * s) as usize;
        for (k, front) in [true, false].into_iter().enumerate() {
            let ox = x0 + k * (pw + 24);
            for y in 0..ph {
                let sheen = 0.5 + 0.5 * ((y as f32 / ph as f32) * 5.0 + 0.7).sin();
                for x in 0..pw {
                    let u = (x as f32 + 0.5) / s;
                    let v = (y as f32 + 0.5) / s;
                    let n = if front { f.fin + 0.5 * f.w + u } else { back_to_net(f, u) };
                    let i = ((v * PX_MM) as usize).min(wr.h - 1) * wr.w + ((n * PX_MM) as usize).min(wr.w - 1);
                    // pillow shading: darker toward the folds and crimps
                    let edge = smoothstep(0.0, 4.0, u.min(f.w - u)) * 0.25 + 0.75;
                    img[(y0 + y) * tw + ox + x] = look(wr, i, sheen) * edge;
                }
            }
        }
        let ly = (top + (maxl * s) as usize + 44) as f32;
        blit_text(&mut img, &fonts.label, 24.0, f.label, x0 as f32, ly, V3::splat(0.7));
        blit_text(&mut img, &fonts.fine, 19.0, &format!("{:.0} x {:.0} x {:.1} mm  \u{00b7}  {} per box", f.w, f.l, f.d, f.box_packs), x0 as f32, ly + 32.0, V3::splat(0.5));
        x0 += pw * 2 + 24 + gap;
    }
    let bytes: Vec<u8> = img.iter().flat_map(|c| srgb_u8(*c)).collect();
    write_rgb(path, tw, th, &bytes);
    println!("wrote {}", path.display());
}
