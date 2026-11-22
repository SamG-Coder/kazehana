//! Card front and back composition.
//!
//! Layout (1500 x 2100 px = 63 x 88 mm at ~605 dpi). The front is built like a small shrine
//! gate: an angular name lintel across the top, two dark attribute "pillars" whose engraved
//! pattern glows in the attribute colour, a portrait 3:4 art window with a glowing rim, a type
//! rail, then a wide text plinth and a print footer with the rarity crest. Glow strength rises
//! with rarity, so tiers read at a glance.

use crate::art::{self, Art};
use crate::canvas::{foil_id, Canvas, Mask, Paint, H};
use crate::math::*;
use crate::motifs::{self, Part, Shape};
use crate::sdf::*;
use crate::text::{self, Fonts};
use std::path::Path;
use tcg::{Attribute, CardDef, Kind, Rarity};

pub const W: usize = 1500;
pub const HH: usize = 2100;
pub const CORNER: f32 = 70.0;
pub const BORDER: f32 = 48.0;

/// Region ids stored per pixel (used by foil passes and documented in CARD_TEMPLATES.md).
pub mod reg {
    pub const BORDER: u8 = 0;
    pub const BODY: u8 = 1;
    pub const PILLAR: u8 = 2;
    pub const ART: u8 = 3;
    pub const TRIM: u8 = 4;
    pub const NAME: u8 = 5;
    pub const TYPE: u8 = 6;
    pub const TEXT: u8 = 7;
    pub const CREST: u8 = 8;
    pub const ORNAMENT: u8 = 9;
    pub const INK: u8 = 10;
}

// Layout rectangles (x0, y0, x1, y1).
pub const NAME_R: (f32, f32, f32, f32) = (64.0, 64.0, 1436.0, 190.0);
pub const ART_R: (f32, f32, f32, f32) = (240.0, 208.0, 1260.0, 1568.0);
pub const PILLAR_L: (f32, f32, f32, f32) = (64.0, 196.0, 240.0, 1584.0);
pub const PILLAR_R: (f32, f32, f32, f32) = (1260.0, 196.0, 1436.0, 1584.0);
pub const TYPE_R: (f32, f32, f32, f32) = (64.0, 1590.0, 1436.0, 1666.0);
pub const TEXT_R: (f32, f32, f32, f32) = (80.0, 1678.0, 1420.0, 1976.0);
/// Reserved for future rules text / abilities (inside TEXT_R, above the flavour divider).
pub const RULES_R: (f32, f32, f32, f32) = (110.0, 1694.0, 1390.0, 1858.0);
pub const CREST_C: P = (750.0, 1990.0);
pub const MEDAL_C: P = (1370.0, 127.0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Finish {
    Normal,
    Holo,
    Reverse,
    FullArt,
    Gold,
    Cosmos,
    Ice,
    Rainbow,
    Glass,
    Sakura,
    Obsidian,
}

impl Finish {
    pub const ALL: [Finish; 11] = [
        Finish::Normal, Finish::Holo, Finish::Reverse, Finish::FullArt, Finish::Gold, Finish::Cosmos,
        Finish::Ice, Finish::Rainbow, Finish::Glass, Finish::Sakura, Finish::Obsidian,
    ];
    pub fn slug(self) -> &'static str {
        match self {
            Finish::Normal => "normal",
            Finish::Holo => "holo",
            Finish::Reverse => "reverse",
            Finish::FullArt => "fullart",
            Finish::Gold => "gold",
            Finish::Cosmos => "cosmos",
            Finish::Ice => "ice",
            Finish::Rainbow => "rainbow",
            Finish::Glass => "glass",
            Finish::Sakura => "sakura",
            Finish::Obsidian => "obsidian",
        }
    }
    pub fn from_slug(s: &str) -> Option<Finish> {
        Finish::ALL.iter().copied().find(|f| f.slug() == s)
    }
    /// Art fills the whole card behind glass panels.
    pub fn full_bleed(self) -> bool {
        matches!(self, Finish::FullArt | Finish::Rainbow)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Frame {
    Enamel,
    Gold,
    Obsidian,
}

/// Visual tier, derived from rarity, finish and attribute.
struct Style {
    crest: u8,
    border: V3,
    trim: V3,
    trim_env: f32,
    ornate: u8,
    gems: bool,
    /// Energy glow strength 0..1 (rises with rarity).
    glow: f32,
    glow_col: V3,
    frame: Frame,
}

fn style(r: Rarity, f: Finish, attr: Attribute) -> Style {
    let ac = srgb8(attr.color());
    let glow_col = ac.lerp(V3::splat(1.0), 0.22).sat(1.25);
    let charcoal = hex(0x15161b);
    let mut s = match r {
        Rarity::Common => Style { crest: 0, border: charcoal, trim: hex(0x4b4e56), trim_env: 0.35, ornate: 0, gems: false, glow: 0.30, glow_col, frame: Frame::Enamel },
        Rarity::Uncommon => Style { crest: 1, border: charcoal, trim: hex(0xdfe4ea), trim_env: 1.0, ornate: 1, gems: false, glow: 0.5, glow_col, frame: Frame::Enamel },
        Rarity::Rare => Style { crest: 2, border: charcoal, trim: hex(0xe8bd62), trim_env: 1.0, ornate: 2, gems: true, glow: 0.75, glow_col, frame: Frame::Enamel },
        Rarity::Mythic => Style { crest: 3, border: hex(0x060608), trim: hex(0xf0b45c), trim_env: 1.0, ornate: 3, gems: true, glow: 1.0, glow_col, frame: Frame::Enamel },
    };
    match f {
        Finish::Gold => {
            s = Style { crest: 4, border: hex(0xd8a64a), trim: hex(0xf3cf7a), trim_env: 1.0, ornate: 3, gems: true, glow: 0.7, glow_col: hex(0xffd27a), frame: Frame::Gold };
        }
        Finish::Obsidian => {
            s.border = hex(0x040405);
            s.trim = hex(0x34363d);
            s.trim_env = 1.0;
            s.ornate = s.ornate.max(2);
            s.gems = true;
            s.glow = 1.0;
            s.frame = Frame::Obsidian;
        }
        Finish::Sakura => {
            s.trim = hex(0xeeb3a0);
            s.trim_env = 1.0;
            s.ornate = s.ornate.max(2);
            s.glow = s.glow.max(0.6);
            s.glow_col = glow_col.lerp(hex(0xffb3cc), 0.6);
        }
        Finish::Normal => {}
        _ => s.glow = s.glow.max(0.6),
    }
    s
}

pub fn ivory() -> V3 {
    hex(0xefe6d2)
}
fn ink() -> V3 {
    hex(0x15120f)
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Monster => "MONSTER",
        Kind::Spell => "SPELL",
        Kind::Relic => "RELIC",
    }
}

fn text_paper(k: Kind) -> V3 {
    match k {
        Kind::Monster => hex(0xe6dcc6),
        Kind::Spell => hex(0xd5dbe2),
        Kind::Relic => hex(0xe2d2b4),
    }
}

/// Text box outline per kind: Monster rounded, Spell arched (concave corners), Relic chamfered.
fn text_box_sdf(k: Kind, p: P, r: (f32, f32, f32, f32), inset: f32) -> f32 {
    let (x0, y0, x1, y1) = (r.0 + inset, r.1 + inset, r.2 - inset, r.3 - inset);
    match k {
        Kind::Monster => rrect(p, x0, y0, x1, y1, 16.0 - inset * 0.5),
        Kind::Spell => {
            let b = rrect(p, x0, y0, x1, y1, 2.0);
            let cr = 30.0 - inset;
            let mut d = b;
            for c in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                d = d.max(-circle(p, c, cr));
            }
            d
        }
        Kind::Relic => chamfer(p, x0, y0, x1, y1, 30.0 - inset * 0.6),
    }
}

fn badge_sdf(k: Kind, p: P, c: P, r: f32) -> f32 {
    match k {
        Kind::Monster => circle(p, c, r),
        Kind::Spell => poly(p, &[(c.0, c.1 - r * 1.12), (c.0 + r * 1.12, c.1), (c.0, c.1 + r * 1.12), (c.0 - r * 1.12, c.1)]),
        Kind::Relic => {
            let v: Vec<P> = (0..6)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 6.0;
                    (c.0 + a.cos() * r * 1.06, c.1 + a.sin() * r * 1.06)
                })
                .collect();
            poly(p, &v)
        }
    }
}

/// Angular "blade" plate: big diagonal cuts on the top-left and bottom-right corners, small
/// ones on the other two.
fn blade_pts(r: (f32, f32, f32, f32), big: f32, small: f32) -> Vec<P> {
    let (x0, y0, x1, y1) = r;
    vec![(x0 + big, y0), (x1 - small, y0), (x1, y0 + small), (x1, y1 - big), (x1 - big, y1), (x0 + small, y1), (x0, y1 - small), (x0, y0 + big)]
}

fn inflate(r: (f32, f32, f32, f32), d: f32) -> (f32, f32, f32, f32) {
    (r.0 - d, r.1 - d, r.2 + d, r.3 + d)
}

fn draw_mask(c: &mut Canvas, m: &Mask, paint: Paint) {
    c.paint(m.bbox(0.0), |x, y| {
        let v = m.at(x, y);
        if v > 0.0 { Some((v, paint)) } else { None }
    });
}

fn draw_shape<F>(c: &mut Canvas, s: &Shape, bbox: (f32, f32, f32, f32), f: F)
where
    F: Fn(f32, f32, f32) -> Paint + Sync,
{
    c.paint(bbox, |x, y| {
        let d = s.dist((x, y));
        if d > 1.0 { None } else { Some((cov(d), f(x, y, d))) }
    });
}

/// Metal trim stroke along an SDF outline: a rounded rim of width w centred on d = 0.
fn trim_paint(d: f32, w: f32, col: V3, env: f32) -> Option<(f32, Paint)> {
    let a = d.abs() - w * 0.5;
    if a > 1.0 {
        return None;
    }
    let t = (d.abs() / (w * 0.5)).min(1.0);
    let h = 0.75 * (1.0 - t * t).max(0.0).sqrt() + 0.35;
    Some((cov(a), Paint::col(col).h(H::Max(h)).relief(1.0).env(env).metal(env).gloss(0.9).reg(reg::TRIM)))
}

/// A glow source line along an SDF outline (no colour change; adds to the glow layer).
fn glow_line(d: f32, w: f32, v: f32) -> Option<(f32, Paint)> {
    let a = d.abs() - w * 0.5;
    if a > 1.0 { None } else { Some((cov(a), Paint::none().glow_max(v))) }
}

pub struct FrontSpec<'a> {
    pub card: &'a CardDef,
    pub finish: Finish,
    pub set_name: &'a str,
    pub set_size: u16,
    pub art_root: &'a Path,
}

/// Physical emboss strength for the height layer.
pub fn emboss_strength(r: Rarity, f: Finish) -> f32 {
    match f {
        Finish::FullArt | Finish::Gold | Finish::Ice | Finish::Rainbow | Finish::Glass => 1.0,
        Finish::Sakura | Finish::Obsidian => 0.8,
        Finish::Reverse => 0.55,
        Finish::Holo | Finish::Cosmos => 0.45,
        Finish::Normal => match r {
            Rarity::Common => 0.18,
            Rarity::Uncommon => 0.25,
            Rarity::Rare => 0.38,
            Rarity::Mythic => 0.55,
        },
    }
}

fn spark_at(x: f32, y: f32, cell: f32, density: f32, seed: u32) -> f32 {
    let hs = hash2f((x / cell).floor() as i32, (y / cell).floor() as i32, seed);
    let t = 1.0 - density;
    if hs > t { 0.05 + 0.95 * (hs - t) / density } else { 0.0 }
}

pub fn front(spec: &FrontSpec, fonts: &Fonts) -> Canvas {
    let card = spec.card;
    let fin = spec.finish;
    let attr = card.attribute;
    let st = style(card.rarity, fin, attr);
    let ac = srgb8(attr.color());
    let deep = (ac * 0.30).sat(1.1);
    // near-black lacquer tinted by the attribute: the dark ground the glow plays against
    let lac = (ac * 0.085).sat(1.25) + V3::splat(0.004);
    let bleed = fin.full_bleed();
    let frame = st.frame;
    let gold_col = hex(0xd9a84e);
    let obs = hex(0x0b0b0e);
    let seed = card.id.set as u32 * 7919 + card.id.number as u32;
    let glow = st.glow;

    let mut c = Canvas::new(W, HH);
    let (wf, hf) = (W as f32, HH as f32);
    let art_ctr = ((ART_R.0 + ART_R.2) * 0.5, (ART_R.1 + ART_R.3) * 0.5);

    for y in 0..HH {
        for x in 0..W {
            let d = rrect((x as f32 + 0.5, y as f32 + 0.5), 0.0, 0.0, wf, hf, CORNER);
            c.alpha[y * W + x] = cov(d);
        }
    }

    let art: Art = if bleed {
        art::load(card, spec.art_root, W, HH)
    } else {
        art::load(card, spec.art_root, (ART_R.2 - ART_R.0) as usize, (ART_R.3 - ART_R.1) as usize)
    };
    let blurred = if bleed { Some(blur_art(&art, 14.0)) } else { None };
    let bl = blurred.as_ref();
    let glass_at = |x: f32, y: f32, dark: f32| -> V3 {
        let b = bl.unwrap();
        b.px[(y as usize).min(HH - 1) * W + (x as usize).min(W - 1)] * dark + deep * 0.3
    };

    // Frame material helper: colour + foil per frame type at a pixel.
    let frame_paint = |col: V3, x: f32, y: f32, n: f32| -> Paint {
        match frame {
            Frame::Enamel => Paint::col(col),
            Frame::Gold => Paint::col(col).env(0.9).metal(1.0).foil(1.0, foil_id::GILT, fract(n + y * 0.0004 + x * 0.0002), 0.0),
            Frame::Obsidian => Paint::col(col).gloss(1.0).metal(0.6).foil(1.0, foil_id::OBSIDIAN, fract(n * 0.5 + (x + y) * 0.0003), 0.0),
        }
    };

    // ------------------------------------------------------------------ border + body
    if bleed {
        let a = &art;
        c.paint(c.full(), |x, y| {
            let i = (y as usize).min(HH - 1) * W + (x as usize).min(W - 1);
            Some((1.0, Paint::col(a.px[i]).h(H::Set(0.0)).relief(0.0).gloss(0.85).reg(reg::ART)))
        });
        c.paint(c.full(), |x, y| {
            let inner = rrect((x, y), BORDER, BORDER, wf - BORDER, hf - BORDER, 26.0);
            if inner < -1.0 {
                return None;
            }
            Some((cov(-inner), Paint::col(glass_at(x, y, 0.4)).h(H::Set(0.18)).relief(0.3).gloss(0.95).reg(reg::BORDER)))
        });
        c.paint(c.full(), |x, y| {
            let d = rrect((x, y), BORDER, BORDER, wf - BORDER, hf - BORDER, 26.0);
            trim_paint(d, 5.0, hex(0xe6eaf0), 1.0).or_else(|| glow_line(d + 4.0, 3.0, 0.5 * glow))
        });
    } else {
        let border = st.border;
        c.paint(c.full(), |x, y| {
            let n = fbm(x * 0.02, y * 0.02, 3, seed) - 0.5;
            let mut col = border * (1.0 + n * 0.08);
            if frame == Frame::Gold {
                let g = motifs::guilloche((x, y), (wf * 0.5, hf * 0.5), 1100.0, 40.0, 18.0, 5);
                let line = smoothstep(2.2, 0.8, (g % 26.0).min(26.0 - g % 26.0));
                col = col * (1.0 - 0.25 * line);
            }
            let p = frame_paint(col, x, y, n).h(H::Set(if frame == Frame::Gold { 0.15 } else { 0.0 })).relief(if frame == Frame::Gold { 0.8 } else { 0.1 }).reg(reg::BORDER);
            Some((1.0, p))
        });
        if st.ornate >= 3 && frame == Frame::Enamel {
            for (inset, w) in [(14.0, 3.0), (22.0, 1.6)] {
                c.paint(c.full(), |x, y| {
                    let d = rrect((x, y), inset, inset, wf - inset, hf - inset, CORNER - inset);
                    trim_paint(d, w, st.trim, 1.0)
                });
            }
        }
        // Frame body: dark lacquer with faint energy rays bursting from the art.
        let body = match frame {
            Frame::Gold => gold_col * 0.85,
            Frame::Obsidian => obs,
            Frame::Enamel => lac * 0.8,
        };
        c.paint((BORDER - 2.0, BORDER - 2.0, wf - BORDER + 2.0, hf - BORDER + 2.0), |x, y| {
            let d = rrect((x, y), BORDER, BORDER, wf - BORDER, hf - BORDER, 22.0);
            if d > 1.0 {
                return None;
            }
            let n = fbm(x * 0.006, y * 0.006, 4, seed ^ 5) - 0.5;
            let ang = (y - art_ctr.1).atan2(x - art_ctr.0);
            let ray = smoothstep(0.9, 1.0, (ang * 56.0 + n * 3.0).cos());
            let p = frame_paint(body * (1.0 + n * 0.25), x, y, n).h(H::Set(0.1)).relief(0.4).gloss(0.6).reg(reg::BODY).glow(ray * 0.12 * glow);
            Some((cov(d), p))
        });
        c.paint(c.full(), |x, y| {
            let d = rrect((x, y), BORDER, BORDER, wf - BORDER, hf - BORDER, 22.0);
            trim_paint(d, 3.0, st.trim, st.trim_env).or_else(|| glow_line(d + 5.0, 2.0, 0.5 * glow))
        });
    }

    // ------------------------------------------------------------------ pillars
    if !bleed {
        for (side, pr) in [(0, PILLAR_L), (1, PILLAR_R)] {
            let (top_col, bot_col) = match frame {
                Frame::Gold => (gold_col, gold_col * 0.8),
                Frame::Obsidian => (obs * 1.4, obs),
                Frame::Enamel => (lac * 1.7, lac * 0.9),
            };
            let inner_x = if side == 0 { pr.2 } else { pr.0 };
            c.paint(inflate(pr, 2.0), |x, y| {
                let d = chamfer((x, y), pr.0, pr.1, pr.2, pr.3, 16.0);
                if d > 1.0 {
                    return None;
                }
                let t = (y - pr.1) / (pr.3 - pr.1);
                let mottle = fbm(x * 0.012, y * 0.012, 4, seed ^ 9) - 0.5;
                let mut col = top_col.lerp(bot_col, smoothstep(0.0, 1.0, t)) * (1.0 + mottle * 0.22);
                // light bleeding from the art window onto the pillar
                let prox = smoothstep(176.0, 0.0, (x - inner_x).abs());
                if frame == Frame::Enamel {
                    col = col.lerp(deep * 0.9, prox.powf(2.0) * 0.6);
                }
                let (pd, _) = motifs::pattern(attr, (x - pr.0, y), 50.0, 2.0);
                let line = cov(pd);
                col = col.lerp(col * if frame == Frame::Gold { 0.7 } else { 0.6 }, line * 0.85);
                let h = 0.15 + 0.3 * bevel(d, 10.0) - 0.12 * line;
                let ray = 0.0;
                // glow on the engraved edges only (uroko is a fill pattern, so use its outline)
                let edge = if attr == Attribute::Flame { cov(pd.abs() - 0.9) } else { line };
                let lg = (edge * 0.22 * prox.powf(1.5) + ray) * glow;
                Some((cov(d), frame_paint(col, x, y, mottle + t * 0.6).h(H::Set(h)).relief(0.7).gloss(0.7).reg(reg::PILLAR).glow(lg)))
            });
            c.paint(inflate(pr, 6.0), |x, y| {
                let d = chamfer((x, y), pr.0, pr.1, pr.2, pr.3, 16.0);
                trim_paint(d, 5.0, st.trim, st.trim_env)
            });
        }
    }

    // ------------------------------------------------------------------ art window
    let win = |p: P, grow: f32| chamfer(p, ART_R.0 - grow, ART_R.1 - grow, ART_R.2 + grow, ART_R.3 + grow, 22.0 + grow * 0.4);
    if !bleed {
        let a = &art;
        let (ax0, ay0) = (ART_R.0, ART_R.1);
        c.paint(ART_R, |x, y| {
            let d = win((x, y), 0.0);
            if d > 1.0 {
                return None;
            }
            let ix = ((x - ax0) as usize).min(a.w - 1);
            let iy = ((y - ay0) as usize).min(a.h - 1);
            Some((cov(d), Paint::col(a.px[iy * a.w + ix]).h(H::Set(0.0)).relief(0.0).gloss(0.85).reg(reg::ART)))
        });
        // Inner shadow, then an energy rim just inside the trim.
        c.paint(ART_R, |x, y| {
            let d = -win((x, y), 0.0);
            let s = smoothstep(26.0, 0.0, d) * 0.4;
            if s <= 0.0 { None } else { Some((s, Paint::col(V3::splat(0.0)).foil(0.0, 0, 0.0, 0.0))) }
        });
        c.paint(inflate(ART_R, 4.0), |x, y| glow_line(win((x, y), 0.0) + 3.5, 3.5, glow));
        c.paint(inflate(ART_R, 20.0), |x, y| trim_paint(win((x, y), 7.0), 14.0, st.trim, st.trim_env));
        if art.placeholder {
            let m = text::line(&fonts.label, 22.0, &format!("ILLUSTRATION PENDING  \u{00b7}  {}", card.id), 3.0, ART_R.0 + 34.0, ART_R.3 - 26.0, 1.0);
            draw_mask(&mut c, &m, Paint::col(V3::splat(0.85)));
        }
    }

    // ------------------------------------------------------------------ name lintel
    let name_pts = blade_pts(NAME_R, 34.0, 12.0);
    {
        let np = &name_pts;
        c.paint(inflate(NAME_R, 2.0), |x, y| {
            let d = poly((x, y), np);
            if d > 1.0 {
                return None;
            }
            let t = (x - NAME_R.0) / (NAME_R.2 - NAME_R.0);
            let tv = (y - NAME_R.1) / (NAME_R.3 - NAME_R.1);
            let n = fbm(x * 0.01, y * 0.03, 4, seed ^ 3) - 0.5;
            let p = if bleed {
                Paint::col(glass_at(x, y, 0.3))
            } else {
                let base = match frame {
                    Frame::Gold => hex(0x0c0b0a),
                    Frame::Obsidian => hex(0x060607),
                    Frame::Enamel => hex(0x0a0a0e).lerp(lac, 0.7),
                };
                // diagonal sheen sweeping from the left in the attribute colour
                let sweep = smoothstep(0.55, 0.0, t + (tv - 0.5) * 0.12);
                let col = base.lerp(deep * 1.1, sweep * 0.55) * (1.12 - 0.22 * tv) * (1.0 + n * 0.08);
                frame_paint(col, x, y, n)
            };
            let h = if bleed { 0.25 } else { 0.18 + 0.32 * bevel(d, 9.0) };
            Some((cov(d), p.h(H::Set(h)).relief(0.55).gloss(0.95).reg(reg::NAME)))
        });
        c.paint(inflate(NAME_R, 8.0), |x, y| trim_paint(poly((x, y), np), 6.0, st.trim, st.trim_env));
        // energy underline inside the plate
        c.paint((NAME_R.0, NAME_R.3 - 22.0, NAME_R.2, NAME_R.3), |x, y| {
            let fade = smoothstep(NAME_R.0 + 40.0, NAME_R.0 + 200.0, x) * smoothstep(MEDAL_C.0 - 40.0, MEDAL_C.0 - 260.0, x);
            glow_line(y - (NAME_R.3 - 14.0), 2.0, glow * fade)
        });
        if st.ornate >= 3 {
            let inner = blade_pts(inflate(NAME_R, -12.0), 26.0, 6.0);
            let ip = &inner;
            c.paint(NAME_R, |x, y| trim_paint(poly((x, y), ip), 2.0, st.trim, 1.0));
        }

        let max_w = MEDAL_C.0 - 70.0 - 104.0;
        let (size, sx) = text::fit(&fonts.name, 66.0, 46.0, &card.name, 0.5, max_w);
        let m = text::line(&fonts.name, size, &card.name, 0.5, 104.0, 151.0, sx);
        let name_col = if frame == Frame::Gold { hex(0xf2cf78) } else { hex(0xf6efdf) };
        let sh = m.blur(4.0).translate(3, 4);
        c.paint(sh.bbox(0.0), |x, y| {
            let v = sh.bilinear(x, y) * 0.75;
            if v > 0.0 { Some((v, Paint::none().col_keep_foil(V3::splat(0.0)))) } else { None }
        });
        let mut np = Paint::col(name_col).h(H::Add(0.22)).relief(0.8).reg(reg::INK).glow(0.18 * glow);
        if frame == Frame::Gold {
            np = np.env(1.0).metal(1.0).foil(1.0, foil_id::GILT, 0.3, 0.0);
        }
        draw_mask(&mut c, &m, np);
        medallion(&mut c, attr, MEDAL_C, 54.0, &st);
    }

    // ------------------------------------------------------------------ type rail
    let type_pts = blade_pts(TYPE_R, 12.0, 26.0);
    {
        let tp = &type_pts;
        c.paint(inflate(TYPE_R, 2.0), |x, y| {
            let d = poly((x, y), tp);
            if d > 1.0 {
                return None;
            }
            let brushed = vnoise(x * 0.004, y * 0.9, seed ^ 13) - 0.5;
            let p = if bleed {
                Paint::col(glass_at(x, y, 0.28))
            } else {
                let base = match frame {
                    Frame::Gold => hex(0x0c0b0a),
                    Frame::Obsidian => hex(0x060607),
                    Frame::Enamel => hex(0x09090c).lerp(lac, 0.7),
                };
                frame_paint(base * (1.0 + brushed * 0.16), x, y, brushed)
            };
            Some((cov(d), p.h(H::Set(0.16 + 0.24 * bevel(d, 8.0))).relief(0.5).gloss(0.85).reg(reg::TYPE)))
        });
        c.paint(inflate(TYPE_R, 7.0), |x, y| trim_paint(poly((x, y), tp), 5.0, st.trim, st.trim_env));
        c.paint((TYPE_R.0, TYPE_R.1, TYPE_R.2, TYPE_R.1 + 12.0), |x, y| {
            let fade = smoothstep(TYPE_R.0 + 90.0, TYPE_R.0 + 260.0, x) * smoothstep(TYPE_R.2 - 30.0, TYPE_R.2 - 220.0, x);
            glow_line(y - (TYPE_R.1 + 7.0), 2.0, glow * fade)
        });

        let bc = (124.0, 1628.0);
        let k = card.kind;
        c.paint((bc.0 - 70.0, bc.1 - 70.0, bc.0 + 70.0, bc.1 + 70.0), |x, y| {
            let d = badge_sdf(k, (x, y), bc, 46.0);
            if d > 8.0 {
                return None;
            }
            if d > 0.0 {
                return Some((cov(d - 6.0), Paint::col(st.border * 0.9).h(H::Max(0.2)).reg(reg::TYPE)));
            }
            Some((cov(d), Paint::col(st.trim).h(H::Set(0.3 + 0.55 * bevel(d, 12.0))).relief(1.0).env(st.trim_env).metal(st.trim_env).gloss(0.9).reg(reg::TRIM)))
        });
        let icon = motifs::kind_icon(k, bc, 27.0);
        draw_shape(&mut c, &icon, (bc.0 - 50.0, bc.1 - 50.0, bc.0 + 50.0, bc.1 + 50.0), |_, _, _| {
            Paint::col(deep * 0.6).h(H::Add(-0.25)).relief(1.0).env(0.0).reg(reg::INK)
        });

        let text_col = if frame == Frame::Gold { hex(0xf2cf78) } else { hex(0xf1e9d6) };
        let tm = text::line(&fonts.label, 40.0, kind_name(k), 6.0, 196.0, 1643.0, 1.0);
        let mut tpaint = Paint::col(text_col).h(H::Add(0.1)).relief(0.6).reg(reg::INK);
        if frame == Frame::Gold {
            tpaint = tpaint.env(1.0).metal(1.0).foil(1.0, foil_id::GILT, 0.5, 0.0);
        }
        draw_mask(&mut c, &tm, tpaint);
        let an = attr.to_string().to_uppercase();
        let aw = text::measure(&fonts.label, 34.0, &an, 5.0);
        let ax = TYPE_R.2 - 44.0 - aw;
        let am = text::line(&fonts.label, 34.0, &an, 5.0, ax, 1641.0, 1.0);
        let acol = if frame == Frame::Gold { hex(0xf2cf78) } else { st.glow_col.lerp(V3::splat(1.0), 0.3) };
        draw_mask(&mut c, &am, Paint::col(acol).h(H::Add(0.08)).reg(reg::INK).glow(0.3 * glow));
        let sg = motifs::sigil(attr, (ax - 34.0, 1628.0), 19.0);
        draw_shape(&mut c, &sg, (ax - 60.0, 1600.0, ax - 8.0, 1656.0), move |_, _, _| Paint::col(acol).h(H::Add(0.08)).reg(reg::INK).glow(0.4 * glow));
    }

    // ------------------------------------------------------------------ text plinth
    {
        let k = card.kind;
        let dark = frame == Frame::Obsidian;
        let paper = match frame {
            Frame::Gold => hex(0xeadcb4),
            Frame::Obsidian => hex(0x17171c).lerp(deep, 0.25),
            Frame::Enamel => text_paper(k).lerp(ac, 0.05),
        };
        let paper_at = move |x: f32, y: f32| -> V3 {
            let fib = fbm(x * 0.05, y * 0.012, 3, seed ^ 21) - 0.5;
            let mot = fbm(x * 0.004, y * 0.004, 4, seed ^ 22) - 0.5;
            if let Some(b) = bl {
                b.px[(y as usize).min(HH - 1) * W + (x as usize).min(W - 1)].lerp(hex(0xf4efe6), 0.74)
            } else {
                paper * (1.0 + fib * 0.04 + mot * 0.07)
            }
        };
        c.paint(inflate(TEXT_R, 2.0), |x, y| {
            let d = text_box_sdf(k, (x, y), TEXT_R, 0.0);
            if d > 1.0 {
                return None;
            }
            Some((cov(d), Paint::col(paper_at(x, y)).h(H::Set(0.08)).relief(0.2).gloss(if bleed || dark { 0.9 } else { 0.45 }).reg(reg::TEXT)))
        });
        c.paint(inflate(TEXT_R, 8.0), |x, y| trim_paint(text_box_sdf(k, (x, y), TEXT_R, 0.0), 6.0, st.trim, st.trim_env));
        let rule_col = if dark { st.glow_col * 0.6 } else { deep * 0.6 };
        c.paint(TEXT_R, |x, y| {
            let d = text_box_sdf(k, (x, y), TEXT_R, 12.0).abs() - 0.9;
            if d > 1.0 { None } else { Some((cov(d) * 0.35, Paint::col(rule_col))) }
        });
        // Watermark sigil in the reserved rules zone (faint).
        let wm = motifs::sigil(attr, (750.0, (RULES_R.1 + RULES_R.3) * 0.5 + 6.0), 92.0);
        let wcol = if dark { st.glow_col } else { deep };
        c.paint((620.0, 1660.0, 880.0, 1900.0), |x, y| {
            let d = wm.dist((x, y));
            if d > 1.0 {
                return None;
            }
            Some((cov(d) * if dark { 0.12 } else { 0.07 }, Paint::col(wcol).h(H::Add(0.03))))
        });
        // Flavour divider.
        let dy = 1874.0;
        let div_col = if dark { st.glow_col } else { deep * 0.7 };
        c.paint((260.0, dy - 14.0, 1240.0, dy + 14.0), |x, y| {
            let l = (y - dy).abs() - 0.9;
            let fade = smoothstep(260.0, 520.0, x) * smoothstep(1240.0, 980.0, x);
            let dia = (x - 750.0).abs() + (y - dy).abs() - 9.0;
            let d = l.min(dia);
            if d > 1.0 { None } else { Some((cov(d) * fade.max(if dia < 1.0 { 1.0 } else { 0.0 }) * 0.8, Paint::col(div_col).reg(reg::INK))) }
        });
        let (quote, source) = match card.flavor.split_once(" \u{2014} ") {
            Some((q, s)) => (q.to_string(), Some(s.to_string())),
            None => (card.flavor.clone(), None),
        };
        let fs = 35.0;
        let lines = text::wrap(&fonts.italic, fs, &quote, 1150.0);
        let fcol = if dark { hex(0xe8e0cf) } else { hex(0x2a231c) };
        let mut y = 1922.0;
        for l in &lines {
            let w = text::measure(&fonts.italic, fs, l, 0.0);
            let m = text::line(&fonts.italic, fs, l, 0.0, 750.0 - w * 0.5, y, 1.0);
            draw_mask(&mut c, &m, Paint::col(fcol).reg(reg::INK));
            y += fs * 1.2;
        }
        if let Some(s) = source {
            let s = format!("\u{2014} {s}");
            let w = text::measure(&fonts.italic, 28.0, &s, 0.0);
            let m = text::line(&fonts.italic, 28.0, &s, 0.0, TEXT_R.2 - 44.0 - w, y.max(1958.0), 1.0);
            draw_mask(&mut c, &m, Paint::col(if dark { fcol * 0.7 } else { fcol * 1.6 }).reg(reg::INK));
        }
    }

    if !bleed {
        ornaments(&mut c, &st, ac, attr);
    }
    crest(&mut c, &st, CREST_C, ac);
    footer(&mut c, fonts, card, spec);

    // ------------------------------------------------------------------ finish passes
    finish_pass(&mut c, fin, attr, seed);
    apply_glow(&mut c, &st);
    c
}

/// Foil, overlays and textures that define each shiny finish.
fn finish_pass(c: &mut Canvas, fin: Finish, attr: Attribute, s: u32) {
    let in_art = |x: f32, y: f32| x >= ART_R.0 && x < ART_R.2 && y >= ART_R.1 && y < ART_R.3;
    match fin {
        Finish::Holo => {
            let (ax0, ay0, ax1, ay1) = ART_R;
            c.paint(ART_R, |x, y| {
                let u = (x - ax0) / (ax1 - ax0);
                let v = (y - ay0) / (ay1 - ay0);
                let wob = fbm(u * 3.0, v * 3.0, 3, s) - 0.5;
                let phase = fract(u * 0.35 + v * 0.55 + wob * 0.25);
                Some((1.0, Paint::none().foil(1.0, foil_id::AURORA, phase, spark_at(x, y, 3.0, 0.015, s ^ 0xabc))))
            });
            keep_foil_in(c, &[reg::ART]);
        }
        Finish::Reverse => {
            c.paint(c.full(), |x, y| {
                let (_, id) = motifs::pattern(attr, (x, y), 64.0, 2.0);
                Some((1.0, Paint::none().foil(1.0, foil_id::SHATTER, id, spark_at(x, y, 4.0, 0.01, s ^ 0x77))))
            });
            scale_foil(c, &[(reg::ART, 0.0), (reg::TEXT, 0.3), (reg::NAME, 0.6)]);
        }
        Finish::FullArt => {
            c.paint(c.full(), |x, y| {
                let (fx, fy) = (750.0, 760.0);
                let warp = fbm(x * 0.003, y * 0.003, 3, s ^ 0x99) - 0.5;
                let ang = (y - fy).atan2(x - fx) + warp * 0.05;
                let rad = ((x - fx).powi(2) + (y - fy).powi(2)).sqrt();
                let ray = smoothstep(0.4, 1.0, (ang * 240.0).cos());
                let ring = smoothstep(0.85, 1.0, (rad * 0.05 + warp * 3.0).cos()) * 0.25;
                let line = ray.max(ring);
                let q = ang / std::f32::consts::TAU + rad * 0.0004;
                Some((1.0, Paint::none().foil(0.55 + 0.45 * line, foil_id::ETCH, fract(q), spark_at(x, y, 4.0, 0.01, s ^ 0x55)).h(H::Add(0.05 * line))))
            });
            scale_foil(c, &[(reg::TEXT, 0.15)]);
        }
        Finish::Cosmos => {
            // Nebula phase swirls through the art window; dense stars of several sizes.
            let (ax0, ay0, ax1, ay1) = ART_R;
            c.paint(ART_R, |x, y| {
                let u = (x - ax0) / (ax1 - ax0);
                let v = (y - ay0) / (ay1 - ay0);
                let sw = fbm(u * 2.2 + fbm(u * 3.0, v * 3.0, 3, s) * 1.5, v * 2.2, 5, s ^ 0x31);
                let phase = fract(sw * 1.4 + v * 0.2);
                let mut sp = spark_at(x, y, 2.5, 0.03, s ^ 0x51);
                // a few big four-point stars
                let cell = 120.0;
                let (cx, cy) = ((x / cell).floor(), (y / cell).floor());
                let h = hash2f(cx as i32, cy as i32, s ^ 0x61);
                if h > 0.72 {
                    let px = (cx + 0.2 + 0.6 * hash2f(cx as i32, cy as i32, 3)) * cell;
                    let py = (cy + 0.2 + 0.6 * hash2f(cx as i32, cy as i32, 4)) * cell;
                    let r = 6.0 + 14.0 * (h - 0.72) / 0.28;
                    let d = poly((x, y), &star_pts((px, py), 4, r, r * 0.18, 0.0));
                    if d < 0.5 {
                        sp = 0.6 + 0.4 * h;
                    }
                }
                Some((1.0, Paint::none().foil(1.0, foil_id::COSMOS, phase, sp)))
            });
            keep_foil_in(c, &[reg::ART]);
        }
        Finish::Ice => {
            // Elongated shards (rotated, stretched voronoi) with bright raised cracks.
            c.paint(c.full(), |x, y| {
                let (sa, ca) = (0.55f32).sin_cos();
                let (rx, ry) = (x * ca + y * sa, -x * sa + y * ca);
                let (d1, d2, id) = voronoi(rx / 150.0, ry / 70.0, s ^ 0x1ce);
                let edge = (d2 - d1) * 70.0;
                let crack = smoothstep(3.0, 0.8, edge);
                let fine = smoothstep(1.4, 0.3, ((d2 - d1) * 230.0 % 40.0 - 20.0).abs()) * 0.0;
                let p = Paint::none().foil(0.75 + 0.25 * crack, foil_id::ICE, id, spark_at(x, y, 3.0, 0.008, s ^ 0x9)).h(H::Add(0.12 * crack + fine));
                Some((1.0, p))
            });
            // crack lines are printed a little lighter (frosted)
            c.paint(c.full(), |x, y| {
                let (sa, ca) = (0.55f32).sin_cos();
                let (rx, ry) = (x * ca + y * sa, -x * sa + y * ca);
                let (d1, d2, _) = voronoi(rx / 150.0, ry / 70.0, s ^ 0x1ce);
                let crack = smoothstep(2.4, 0.6, (d2 - d1) * 70.0);
                if crack <= 0.0 { None } else { Some((crack * 0.28, Paint::none().col_keep_foil(hex(0xe8f4ff)))) }
            });
            scale_foil(c, &[(reg::ART, 0.7), (reg::TEXT, 0.25)]);
        }
        Finish::Rainbow => {
            // Pastel spectrum printed over the art and glass (not over the name/type plates or
            // ink, which stay legible), plus strong rainbow foil and fine diagonal etching.
            let w = c.w;
            for (i, p) in c.px.iter_mut().enumerate() {
                if matches!(p.reg, reg::NAME | reg::TYPE | reg::INK | reg::TRIM | reg::CREST) {
                    continue;
                }
                let (x, y) = ((i % w) as f32, (i / w) as f32);
                let t = (x * 0.6 + y * 0.8) / 2400.0;
                p.col = p.col.lerp(spectral(t).lerp(V3::splat(1.0), 0.45), 0.30);
            }
            c.paint(c.full(), |x, y| {
                let line = smoothstep(0.6, 1.0, ((x - y * 0.5) * 0.35).cos());
                let t = (x * 0.6 + y * 0.8) / 2400.0;
                Some((1.0, Paint::none().foil(0.8 + 0.2 * line, foil_id::RAINBOW, fract(t), spark_at(x, y, 3.0, 0.012, s ^ 0x77)).h(H::Add(0.04 * line))))
            });
            scale_foil(c, &[(reg::TEXT, 0.2)]);
            let _ = in_art;
        }
        Finish::Glass => {
            // The art becomes leaded glass: panes tinted and foiled independently.
            c.paint(ART_R, |x, y| {
                let (d1, d2, id) = voronoi(x / 150.0, y / 150.0, s ^ 0x6a5);
                let lead = (d2 - d1) * 150.0;
                let pane_tint = spectral(id).lerp(V3::splat(1.0), 0.72);
                if lead < 4.5 {
                    let b = bevel(lead - 4.5, 3.5);
                    return Some((cov(lead - 4.0), Paint::col(hex(0x222326) * (0.7 + 0.6 * b)).h(H::Set(0.4 + 0.4 * b)).relief(1.0).env(0.6).metal(0.6).reg(reg::ORNAMENT)));
                }
                Some((1.0, Paint::none().col_mul(pane_tint).foil(0.9, foil_id::GLASS, id, 0.0).h(H::Add(0.06 * smoothstep(40.0, 0.0, lead)))))
            });
            keep_foil_in(c, &[reg::ART]);
        }
        Finish::Sakura => {
            // Falling petals over the whole card: printed pale pink, rose-gold foil, raised.
            c.paint(c.full(), |x, y| {
                let cell = 120.0;
                let (gx, gy) = ((x / cell).floor() as i32, (y / cell).floor() as i32);
                let mut best = (f32::MAX, 0.0f32);
                for j in -1..=1 {
                    for i in -1..=1 {
                        let (cx, cy) = (gx + i, gy + j);
                        let h = hash2f(cx, cy, s ^ 0x5a);
                        if h < 0.78 {
                            continue;
                        }
                        let px = (cx as f32 + hash2f(cx, cy, 1)) * cell;
                        let py = (cy as f32 + hash2f(cx, cy, 2)) * cell;
                        let rot = hash2f(cx, cy, 3) * std::f32::consts::TAU;
                        let len = 14.0 + 18.0 * hash2f(cx, cy, 4);
                        let (sn, cs) = rot.sin_cos();
                        let (u, v) = ((x - px) * cs + (y - py) * sn, -(x - px) * sn + (y - py) * cs);
                        // petal: ellipse with a notch at the tip
                        let e = ((u / len).powi(2) + (v / (len * 0.62)).powi(2)).sqrt() - 1.0;
                        let notch = circle((u, v), (len * 1.02, 0.0), len * 0.28);
                        let d = e.max(-notch / len) * len * 0.62;
                        if d < best.0 {
                            best = (d, h);
                        }
                    }
                }
                let d = best.0;
                if d > 1.0 || rrect((x, y), BORDER + 6.0, BORDER + 6.0, 1500.0 - BORDER - 6.0, 2100.0 - BORDER - 6.0, 20.0) > 0.0 {
                    return None;
                }
                let a = cov(d);
                let r = reg_at(x, y);
                let opacity = if r == reg::TEXT { 0.2 } else if in_art(x, y) { 0.55 } else { 0.85 };
                Some((a * opacity, Paint::col(hex(0xf2a9bf).lerp(hex(0xfbd3de), best.1)).foil(1.0, foil_id::SAKURA, best.1, 0.0).h(H::Add(0.2 * bevel(d, 4.0))).relief(0.6)))
            });
        }
        Finish::Normal | Finish::Gold | Finish::Obsidian => {}
    }
}

/// Region lookup for passes that need it (approximate: by layout rectangles).
fn reg_at(x: f32, y: f32) -> u8 {
    if x >= TEXT_R.0 && x < TEXT_R.2 && y >= TEXT_R.1 && y < TEXT_R.3 {
        reg::TEXT
    } else {
        reg::BODY
    }
}

fn keep_foil_in(c: &mut Canvas, regions: &[u8]) {
    for p in c.px.iter_mut() {
        if !regions.contains(&p.reg) {
            p.foil = 0.0;
            p.fid = foil_id::NONE;
        }
    }
}

fn scale_foil(c: &mut Canvas, f: &[(u8, f32)]) {
    for p in c.px.iter_mut() {
        if let Some((_, k)) = f.iter().find(|(r, _)| *r == p.reg) {
            p.foil *= k;
            if p.foil <= 0.0 {
                p.fid = foil_id::NONE;
            }
        }
    }
}

/// Spread the glow sources into a printed glow: hot core, tight bloom and a wide halo, all in
/// the card's glow colour. Kept faint over the text plinth so text stays readable.
fn apply_glow(c: &mut Canvas, st: &Style) {
    let (w, h) = (c.w, c.h);
    let mut m = Mask::new(0, 0, w, h);
    for (i, p) in c.px.iter().enumerate() {
        m.d[i] = p.glow;
    }
    let b1 = m.blur(5.0);
    // wide halo from the squared source: bright features bloom, faint line work does not wash
    let mut sq = m.clone();
    for v in sq.d.iter_mut() {
        *v = *v * *v;
    }
    let b2 = sq.blur(26.0);
    let gc = st.glow_col;
    let hot = gc.lerp(V3::splat(1.0), 0.6);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let src = m.d[i];
            let (a, b) = (b1.get(x as i32, y as i32), b2.get(x as i32, y as i32));
            let p = &mut c.px[i];
            let damp = match p.reg {
                reg::TEXT => 0.2,
                reg::ART => 0.85,
                _ => 1.0,
            };
            p.col = p.col + (hot * (src * 0.9) + gc * (a * 0.5 + b * 2.4)) * damp;
        }
    }
}

fn blur_art(a: &Art, r: f32) -> Art {
    let mut chans = [Mask::new(0, 0, a.w, a.h), Mask::new(0, 0, a.w, a.h), Mask::new(0, 0, a.w, a.h)];
    for (i, p) in a.px.iter().enumerate() {
        chans[0].d[i] = p.0;
        chans[1].d[i] = p.1;
        chans[2].d[i] = p.2;
    }
    let b: Vec<Mask> = chans.iter().map(|m| m.blur(r)).collect();
    let mut px = vec![V3::splat(0.0); a.w * a.h];
    for y in 0..a.h {
        for x in 0..a.w {
            px[y * a.w + x] = V3(b[0].get(x as i32, y as i32), b[1].get(x as i32, y as i32), b[2].get(x as i32, y as i32));
        }
    }
    Art { w: a.w, h: a.h, px, placeholder: a.placeholder }
}

/// Enamel dome medallion with the attribute sigil, set in a metal bezel with a glow ring.
fn medallion(c: &mut Canvas, attr: Attribute, mc: P, r: f32, st: &Style) {
    let ac = srgb8(attr.color());
    let enamel_hi = ac.lerp(V3::splat(1.0), 0.25);
    let enamel_lo = (ac * 0.35).sat(1.2);
    let glow = st.glow;
    c.paint((mc.0 - r - 14.0, mc.1 - r - 14.0, mc.0 + r + 14.0, mc.1 + r + 14.0), |x, y| {
        let d = circle((x, y), mc, r);
        if d > 9.0 {
            return None;
        }
        if d > -7.0 {
            let t = ((d + 7.0) / 16.0 - 0.5).abs() * 2.0;
            let h = 0.6 + 0.5 * (1.0 - t * t).max(0.0).sqrt();
            return Some((cov(d - 8.0), Paint::col(st.trim).h(H::Set(h)).relief(1.0).env(st.trim_env).metal(st.trim_env).gloss(0.9).reg(reg::TRIM)));
        }
        let rr = ((x - mc.0).powi(2) + (y - mc.1).powi(2)).sqrt() / (r - 7.0);
        let dome = (1.0 - rr * rr).max(0.0).sqrt();
        let hl = ((x - mc.0 + r * 0.3).powi(2) + (y - mc.1 + r * 0.35).powi(2)).sqrt() / r;
        let col = enamel_lo.lerp(enamel_hi, (1.0 - rr) * 0.8) + V3::splat(0.25 * (-hl * hl * 9.0).exp());
        let ring = smoothstep(0.75, 1.0, rr) * glow * 0.6;
        Some((1.0, Paint::col(col).h(H::Set(0.55 + 0.45 * dome)).relief(0.4).gloss(1.0).reg(reg::ORNAMENT).glow(ring)))
    });
    let s = motifs::sigil(attr, mc, r * 0.62);
    let fg = hex(0xfbf4e2);
    c.paint((mc.0 - r, mc.1 - r, mc.0 + r, mc.1 + r), |x, y| {
        let d = s.dist((x - 2.0, y - 3.0));
        if d > 3.0 { None } else { Some((cov_soft(d, 3.0) * 0.45, Paint::col(enamel_lo * 0.3))) }
    });
    draw_shape(c, &s, (mc.0 - r, mc.1 - r, mc.0 + r, mc.1 + r), move |_, _, _| Paint::col(fg).h(H::Add(0.18)).relief(0.9).reg(reg::ORNAMENT).glow(0.35 * glow));
}

/// Corner filigree, gems and mythic flourishes.
fn ornaments(c: &mut Canvas, st: &Style, ac: V3, attr: Attribute) {
    let (x0, y0, x1, y1) = inflate(ART_R, 7.0);
    let cxm = (x0 + x1) * 0.5;
    let cym = (y0 + y1) * 0.5;
    let (hw, hh) = ((x1 - x0) * 0.5, (y1 - y0) * 0.5);
    if st.ornate >= 1 {
        let size = match st.ornate {
            1 => 46.0,
            2 => 70.0,
            _ => 92.0,
        };
        let mut parts: Vec<Part> = Vec::new();
        // Folded frame: corner at (0,0), +x/+y point into the window.
        parts.push(Part::Stroke(vec![(8.0, 2.0), (size, -4.0)], 9.0, 4.0));
        parts.push(Part::Stroke(vec![(2.0, 8.0), (-4.0, size)], 9.0, 4.0));
        parts.push(Part::Fill(vec![(-14.0, -14.0), (22.0, -14.0), (22.0, -2.0), (-2.0, 22.0), (-14.0, 22.0)]));
        if st.ornate >= 2 {
            parts.push(motifs::scroll((size * 0.35, 6.0), size * 0.55, 0.15, 1.0, 7.0));
            parts.push(motifs::scroll((6.0, size * 0.35), size * 0.55, std::f32::consts::FRAC_PI_2 - 0.15, -1.0, 7.0));
            parts.push(Part::Fill(vec![(size - 4.0, -10.0), (size + 18.0, -4.0), (size - 4.0, 2.0)]));
            parts.push(Part::Fill(vec![(-10.0, size - 4.0), (-4.0, size + 18.0), (2.0, size - 4.0)]));
        }
        if st.ornate >= 3 {
            parts.push(motifs::scroll((size * 0.75, 2.0), size * 0.5, 0.35, 1.0, 6.0));
            parts.push(motifs::scroll((2.0, size * 0.75), size * 0.5, std::f32::consts::FRAC_PI_2 - 0.35, -1.0, 6.0));
            parts.push(Part::Fill(star_pts((24.0, 24.0), 4, 18.0, 5.0, std::f32::consts::FRAC_PI_4)));
        }
        let shape = Shape { parts };
        let trim = st.trim;
        let env = st.trim_env;
        let ext = size + 30.0;
        let glow = st.glow;
        for (sxn, syn) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let cx = cxm + sxn * hw;
            let cy = cym + syn * hh;
            let bbox = (cx.min(cx - sxn * ext) - 16.0, cy.min(cy - syn * ext) - 16.0, cx.max(cx - sxn * ext) + 16.0, cy.max(cy - syn * ext) + 16.0);
            let sh = &shape;
            c.paint(bbox, |x, y| {
                let d = sh.dist(((cx - x) * sxn - 3.0 * sxn, (cy - y) * syn - 4.0 * syn));
                if d > 4.0 { None } else { Some((cov_soft(d, 4.0) * 0.5, Paint::col(V3::splat(0.0)))) }
            });
            c.paint(bbox, |x, y| {
                let d = sh.dist(((cx - x) * sxn, (cy - y) * syn));
                if d > 1.0 {
                    return None;
                }
                let inner = bevel(d, 3.5);
                Some((cov(d), Paint::col(trim).h(H::Max(0.5 + 0.5 * inner)).relief(1.0).env(env).metal(env).gloss(0.9).reg(reg::ORNAMENT).glow(0.15 * glow)))
            });
        }
    }
    if st.gems {
        let gem_r = if st.ornate >= 3 { 26.0 } else { 21.0 };
        for &gx in &[(PILLAR_L.0 + PILLAR_L.2) * 0.5, (PILLAR_R.0 + PILLAR_R.2) * 0.5] {
            for &gy in &[PILLAR_L.1 + 62.0, PILLAR_L.3 - 62.0] {
                gem(c, (gx, gy), gem_r, ac, st);
            }
        }
        if st.ornate >= 3 {
            for &gx in &[(PILLAR_L.0 + PILLAR_L.2) * 0.5, (PILLAR_R.0 + PILLAR_R.2) * 0.5] {
                let gy = (PILLAR_L.1 + PILLAR_L.3) * 0.5;
                let s = motifs::sigil(attr, (gx, gy), 46.0);
                let trim = st.trim;
                let glow = st.glow;
                c.paint((gx - 66.0, gy - 66.0, gx + 66.0, gy + 66.0), |x, y| {
                    let d = circle((x, y), (gx, gy), 60.0);
                    if d > 1.0 {
                        return None;
                    }
                    if let Some(r) = trim_paint(d + 3.0, 5.0, trim, 1.0) {
                        return Some(r);
                    }
                    Some((cov(d) * 0.7, Paint::col(V3::splat(0.0)).h(H::Set(0.1))))
                });
                draw_shape(c, &s, (gx - 50.0, gy - 50.0, gx + 50.0, gy + 50.0), move |_, _, d| {
                    Paint::col(trim).h(H::Set(0.5 + 0.4 * bevel(d, 3.0))).relief(1.0).env(1.0).metal(1.0).reg(reg::ORNAMENT).glow(0.5 * glow)
                });
            }
        }
    }
}

fn gem(c: &mut Canvas, gc: P, r: f32, ac: V3, st: &Style) {
    let lo = (ac * 0.25).sat(1.3);
    let hi = ac.lerp(V3::splat(1.0), 0.35);
    let glow = st.glow;
    c.paint((gc.0 - r - 10.0, gc.1 - r - 10.0, gc.0 + r + 10.0, gc.1 + r + 10.0), |x, y| {
        let d = circle((x, y), gc, r);
        if d > 7.0 {
            return None;
        }
        if d > -1.0 {
            return trim_paint(d - 2.0, 7.0, st.trim, st.trim_env);
        }
        let rr = (((x - gc.0).powi(2) + (y - gc.1).powi(2)).sqrt() / r).min(1.0);
        let dome = (1.0 - rr * rr).sqrt();
        let caustic = (-(((x - gc.0 - r * 0.3).powi(2) + (y - gc.1 - r * 0.35).powi(2)) / (r * r * 0.18))).exp();
        let spec = (-(((x - gc.0 + r * 0.35).powi(2) + (y - gc.1 + r * 0.4).powi(2)) / (r * r * 0.03))).exp();
        let col = lo.lerp(hi, caustic * 0.9) + V3::splat(spec * 0.9);
        Some((1.0, Paint::col(col).h(H::Set(0.6 + 0.6 * dome)).relief(0.3).gloss(1.0).reg(reg::ORNAMENT).glow(caustic * 0.6 * glow)))
    });
}

fn crest(c: &mut Canvas, st: &Style, cc: P, ac: V3) {
    let r = match st.crest {
        0 => 22.0,
        1 => 25.0,
        2 => 30.0,
        3 => 35.0,
        _ => 37.0,
    };
    let seat_r = r + 14.0;
    let border = st.border;
    let trim = st.trim;
    let glow = st.glow;
    c.paint((cc.0 - seat_r - 10.0, cc.1 - seat_r - 10.0, cc.0 + seat_r + 10.0, cc.1 + seat_r + 10.0), |x, y| {
        let d = circle((x, y), cc, seat_r);
        if d > 6.0 {
            return None;
        }
        if let Some(t) = trim_paint(d, 5.0, trim, st.trim_env) {
            if d > -2.5 {
                return Some(t);
            }
        }
        Some((cov(d), Paint::col(border * 0.8 + V3::splat(0.01)).h(H::Set(0.12)).relief(0.3).reg(reg::CREST).glow(smoothstep(-14.0, -4.0, d) * 0.5 * glow)))
    });
    if st.ornate >= 3 {
        for side in [-1.0f32, 1.0] {
            let pts: Vec<P> = vec![
                (cc.0 + side * (seat_r - 4.0), cc.1 - 10.0),
                (cc.0 + side * (seat_r + 70.0), cc.1 - 22.0),
                (cc.0 + side * (seat_r + 52.0), cc.1 - 6.0),
                (cc.0 + side * (seat_r + 80.0), cc.1 - 2.0),
                (cc.0 + side * (seat_r + 48.0), cc.1 + 8.0),
                (cc.0 + side * (seat_r + 62.0), cc.1 + 18.0),
                (cc.0 + side * (seat_r - 4.0), cc.1 + 12.0),
            ];
            let bb = (cc.0 - seat_r - 90.0, cc.1 - 40.0, cc.0 + seat_r + 90.0, cc.1 + 40.0);
            c.paint(bb, |x, y| {
                let d = poly((x, y), &pts);
                if d > 1.0 {
                    return None;
                }
                Some((cov(d), Paint::col(trim).h(H::Max(0.4 + 0.5 * bevel(d, 4.0))).relief(1.0).env(1.0).metal(1.0).reg(reg::CREST)))
            });
        }
    }
    let shape = motifs::crest(st.crest, cc, r);
    let (fill, env, foil) = match st.crest {
        0 => (hex(0xe9e3d3), 0.0, false),
        1 => (hex(0xe4e8ee), 1.0, false),
        2 => (hex(0xefc469), 1.0, true),
        3 => (hex(0xd8442c).lerp(ac, 0.15), 0.0, true),
        _ => (hex(0xf6d27e), 1.0, true),
    };
    let rim = st.trim;
    let sh = &shape;
    c.paint((cc.0 - r * 1.3, cc.1 - r * 1.3, cc.0 + r * 1.3, cc.1 + r * 1.3), |x, y| {
        let d = sh.dist((x, y));
        if d > 1.0 {
            return None;
        }
        let b = bevel(d, 5.0);
        let (col, e) = if st.crest == 3 && d > -4.0 { (rim, 1.0) } else { (fill, env) };
        let (col, e) = if st.crest == 0 && d > -3.0 { (hex(0x8a8576), 0.0) } else { (col, e) };
        let mut p = Paint::col(col).h(H::Set(0.4 + 0.6 * b)).relief(1.0).env(e).metal(e).gloss(1.0).reg(reg::CREST).glow(0.3 * glow);
        if foil {
            p = p.foil(1.0, if st.crest == 3 { foil_id::AURORA } else { foil_id::GILT }, fract(x * 0.01 + y * 0.013), 0.0);
        }
        Some((cov(d), p))
    });
}

fn footer(c: &mut Canvas, fonts: &Fonts, card: &CardDef, spec: &FrontSpec) {
    let col = if spec.finish == Finish::Gold { hex(0x2a1d08) } else { hex(0xcfc6b2) };
    let y = 2030.0;
    let id = card.id.to_string();
    let m = text::line(&fonts.label, 27.0, &id, 2.5, 100.0, y, 1.0);
    let wid = text::measure(&fonts.label, 27.0, &id, 2.5);
    draw_mask(c, &m, Paint::col(col).reg(reg::INK));
    let rar = match spec.finish {
        Finish::Gold => "SR".to_string(),
        Finish::Normal => card.rarity.mark().to_string(),
        f => format!("{} \u{00b7} {}", card.rarity.mark(), f.slug().to_uppercase()),
    };
    let rest = format!("{:03}/{:03}   {}   {}", card.id.number, spec.set_size, rar, spec.set_name.to_uppercase());
    let x0 = 100.0 + wid + 22.0;
    let (fsz, fsx) = text::fit(&fonts.fine, 21.0, 17.0, &rest, 1.5, CREST_C.0 - 70.0 - x0);
    let m = text::line(&fonts.fine, fsz, &rest, 1.5, x0, y - 1.0, fsx);
    draw_mask(c, &m, Paint::col(col * 0.85).reg(reg::INK));
    let right = "Illus. Tsukumo Studio   \u{00a9} 2026 Kazehana Card Works";
    let (rsz, rsx) = text::fit(&fonts.fine, 21.0, 17.0, right, 1.0, W as f32 - 100.0 - (CREST_C.0 + 70.0));
    let rw = text::measure(&fonts.fine, rsz, right, 1.0) * rsx;
    let m = text::line(&fonts.fine, rsz, right, 1.0, W as f32 - 100.0 - rw, y - 1.0, rsx);
    draw_mask(c, &m, Paint::col(col * 0.85).reg(reg::INK));
}


// ====================================================================== back

pub fn back(fonts: &Fonts, game: &str) -> Canvas {
    let mut c = Canvas::new(W, HH);
    let (wf, hf) = (W as f32, HH as f32);
    let ctr = (wf * 0.5, hf * 0.5);
    for y in 0..HH {
        for x in 0..W {
            let d = rrect((x as f32 + 0.5, y as f32 + 0.5), 0.0, 0.0, wf, hf, CORNER);
            c.alpha[y * W + x] = cov(d);
        }
    }
    let navy = hex(0x10142a);
    let indigo = hex(0x2a2160);
    let gold = hex(0xe2b660);
    let border = hex(0x0b0d18);

    // Border and field.
    c.paint(c.full(), |x, y| {
        let n = fbm(x * 0.02, y * 0.02, 3, 4) - 0.5;
        Some((1.0, Paint::col(border * (1.0 + n * 0.1)).h(H::Set(0.0)).relief(0.1).gloss(0.6).reg(reg::BORDER)))
    });
    let field = |p: P| rrect(p, BORDER, BORDER, wf - BORDER, hf - BORDER, 24.0);
    c.paint(c.full(), |x, y| {
        let d = field((x, y));
        if d > 1.0 {
            return None;
        }
        let r = (((x - ctr.0) / wf).powi(2) + ((y - ctr.1) / hf * 0.75).powi(2)).sqrt();
        let mut col = indigo.lerp(navy, smoothstep(0.05, 0.6, r));
        // Seigaiha waves, very low contrast.
        let (pd, _) = motifs::pattern(Attribute::Tide, (x, y), 90.0, 2.0);
        col = col.lerp(col * 1.35 + V3::splat(0.004), cov(pd) * 0.35 * smoothstep(0.15, 0.4, r));
        // Guilloche rosette around the seal.
        let g1 = motifs::guilloche((x, y), ctr, 560.0, 34.0, 24.0, 6);
        let g2 = motifs::guilloche((x, y), ctr, 640.0, 22.0, 36.0, 4);
        let gl = cov(g1 - 0.8).max(cov(g2 - 0.7));
        col = col.lerp(gold * 0.55, gl * 0.5);
        let n = fbm(x * 0.004, y * 0.004, 4, 8) - 0.5;
        col = col * (1.0 + n * 0.12);
        Some((cov(d), Paint::col(col).h(H::Set(0.06 + 0.06 * gl)).relief(0.3).gloss(0.7).reg(reg::BODY)))
    });
    for (inset, w) in [(BORDER, 5.0), (BORDER + 16.0, 2.0)] {
        c.paint(c.full(), |x, y| {
            let d = rrect((x, y), inset, inset, wf - inset, hf - inset, 24.0 - (inset - BORDER) * 0.5);
            trim_paint(d, w, gold, 1.0)
        });
    }
    // Corner ornaments: folded coordinates, scrolls in each corner.
    {
        let parts = vec![
            motifs::scroll((26.0, 8.0), 110.0, 0.05, 1.0, 8.0),
            motifs::scroll((8.0, 26.0), 110.0, std::f32::consts::FRAC_PI_2 - 0.05, -1.0, 8.0),
            Part::Fill(star_pts((30.0, 30.0), 4, 24.0, 7.0, std::f32::consts::FRAC_PI_4)),
            Part::Stroke(vec![(120.0, 4.0), (300.0, 4.0)], 5.0, 1.0),
            Part::Stroke(vec![(4.0, 120.0), (4.0, 300.0)], 5.0, 1.0),
        ];
        let shape = Shape { parts };
        let o = BORDER + 30.0;
        for (sxn, syn) in [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            let cx = if sxn > 0.0 { o } else { wf - o };
            let cy = if syn > 0.0 { o } else { hf - o };
            let bb = (cx.min(cx + sxn * 330.0) - 10.0, cy.min(cy + syn * 330.0) - 10.0, cx.max(cx + sxn * 330.0) + 10.0, cy.max(cy + syn * 330.0) + 10.0);
            let sh = &shape;
            c.paint(bb, |x, y| {
                let d = sh.dist(((x - cx) * sxn, (y - cy) * syn));
                if d > 1.0 {
                    return None;
                }
                Some((cov(d), Paint::col(gold).h(H::Max(0.4 + 0.4 * bevel(d, 3.0))).relief(1.0).env(1.0).metal(1.0).reg(reg::ORNAMENT)))
            });
        }
    }

    // Seal: rings, six attribute gems, star core.
    let seal_r = 470.0;
    c.paint((ctr.0 - seal_r - 20.0, ctr.1 - seal_r - 20.0, ctr.0 + seal_r + 20.0, ctr.1 + seal_r + 20.0), |x, y| {
        let d = circle((x, y), ctr, seal_r);
        if d > 12.0 {
            return None;
        }
        let r = seal_r + d;
        let mut best: Option<(f32, Paint)> = None;
        for (rr, w) in [(seal_r, 12.0), (seal_r - 22.0, 3.0), (300.0, 9.0), (282.0, 2.5)] {
            if let Some(t) = trim_paint(r - rr, w, gold, 1.0) {
                if best.as_ref().map_or(true, |b| t.0 > b.0) {
                    best = Some(t);
                }
            }
        }
        if let Some(b) = best {
            return Some(b);
        }
        if d < 0.0 && r > 300.0 {
            // band between rings: deep lacquer with radial engraving
            let ang = (y - ctr.1).atan2(x - ctr.0);
            let rad = smoothstep(0.6, 1.0, (ang * 72.0).cos().abs()) * 0.4;
            return Some((1.0, Paint::col(hex(0x0c0b1c).lerp(gold * 0.4, rad * smoothstep(seal_r - 40.0, seal_r - 70.0, r))).h(H::Set(0.2)).relief(0.4).reg(reg::BODY)));
        }
        None
    });
    // Inner disc: a starburst.
    c.paint((ctr.0 - 290.0, ctr.1 - 290.0, ctr.0 + 290.0, ctr.1 + 290.0), |x, y| {
        let d = circle((x, y), ctr, 282.0);
        if d > 0.5 {
            return None;
        }
        let ang = (y - ctr.1).atan2(x - ctr.0);
        let r = 282.0 + d;
        let rays = (ang * 16.0).cos() * 0.5 + 0.5;
        let col = hex(0x1d1748).lerp(hex(0x3a2f86), (1.0 - r / 282.0) * 0.8) * (0.85 + 0.3 * rays);
        Some((cov(d), Paint::col(col).h(H::Set(0.15)).relief(0.3).reg(reg::BODY)))
    });
    let star = Shape { parts: vec![Part::Fill(star_pts(ctr, 8, 250.0, 120.0, 0.0)), Part::CutCircle(ctr, 96.0)] };
    draw_shape(&mut c, &star, (ctr.0 - 260.0, ctr.1 - 260.0, ctr.0 + 260.0, ctr.1 + 260.0), |x, y, d| {
        let ang = (y - ctr.1).atan2(x - ctr.0);
        let facet = ((ang * 8.0 / std::f32::consts::PI).floor() as i32).rem_euclid(2) as f32;
        Paint::col(gold * (0.75 + 0.3 * facet)).h(H::Set(0.3 + 0.5 * bevel(d, 30.0))).relief(1.0).env(1.0).metal(1.0).reg(reg::ORNAMENT)
    });
    let st = Style { crest: 0, border, trim: gold, trim_env: 1.0, ornate: 0, gems: true, glow: 0.8, glow_col: hex(0xffcf7a), frame: Frame::Enamel };
    for (i, a) in Attribute::ALL.iter().enumerate() {
        let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 6.0;
        let gc = (ctr.0 + ang.cos() * 386.0, ctr.1 + ang.sin() * 386.0);
        medallion(&mut c, *a, gc, 52.0, &st);
    }

    // Logo ribbon across the seal.
    let name = game.to_uppercase();
    let (size, sx) = text::fit(&fonts.logo, 132.0, 84.0, &name, 12.0, 1100.0);
    let tw = text::measure(&fonts.logo, size, &name, 12.0) * sx;
    let rib_hw = (tw * 0.5 + 90.0).max(420.0);
    let rib_y = (ctr.1 - 118.0, ctr.1 + 118.0);
    let ribbon = |p: P| -> f32 {
        // banner with swallow-tail notches at both ends
        let b = rrect(p, ctr.0 - rib_hw, rib_y.0, ctr.0 + rib_hw, rib_y.1, 4.0);
        let n = ((p.0 - ctr.0).abs() - (rib_hw - 52.0)) - (p.1 - ctr.1).abs() * 0.44;
        b.max(n * 0.915)
    };
    c.paint((ctr.0 - rib_hw - 20.0, rib_y.0 - 20.0, ctr.0 + rib_hw + 20.0, rib_y.1 + 20.0), |x, y| {
        let d = ribbon((x, y));
        if d > 1.0 {
            return None;
        }
        let tv = (y - rib_y.0) / (rib_y.1 - rib_y.0);
        let col = hex(0x7a1424).lerp(hex(0x3d0710), tv) * (1.0 + 0.1 * (1.0 - (tv - 0.3).abs()));
        Some((cov(d), Paint::col(col).h(H::Set(0.45 + 0.25 * bevel(d, 10.0))).relief(0.7).gloss(0.9).reg(reg::NAME)))
    });
    for off in [-100.0f32, 100.0] {
        c.paint((ctr.0 - rib_hw, ctr.1 + off - 8.0, ctr.0 + rib_hw, ctr.1 + off + 8.0), |x, y| {
            let fade = smoothstep(rib_hw, rib_hw - 60.0, (x - ctr.0).abs());
            trim_paint(y - ctr.1 - off, 4.0, gold, 1.0).map(|(c, p)| (c * fade, p))
        });
    }
    let base = ctr.1 + 22.0;
    let m = text::line(&fonts.logo, size, &name, 12.0, ctr.0 - tw * 0.5, base, sx);
    let sh = m.blur(5.0).translate(4, 6);
    c.paint(sh.bbox(0.0), |x, y| {
        let v = sh.bilinear(x, y) * 0.8;
        if v > 0.0 { Some((v, Paint::col(V3::splat(0.0)))) } else { None }
    });
    // Gold letters with a bevel: derive the bevel from a blurred copy.
    let mb = m.blur(2.5);
    c.paint(m.bbox(0.0), |x, y| {
        let v = m.at(x, y);
        if v <= 0.0 {
            return None;
        }
        let inner = mb.bilinear(x, y);
        Some((v, Paint::col(gold).h(H::Set(0.7 + 0.5 * inner)).relief(1.0).env(1.0).metal(1.0).gloss(1.0).reg(reg::INK).glow(0.25)))
    });
    let sub = "TRADING  CARD  GAME";
    let sw = text::measure(&fonts.label, 30.0, sub, 9.0);
    let m = text::line(&fonts.label, 30.0, sub, 9.0, ctr.0 - sw * 0.5, ctr.1 + 80.0, 1.0);
    draw_mask(&mut c, &m, Paint::col(gold).h(H::Add(0.15)).relief(0.8).env(1.0).metal(1.0).reg(reg::INK));
    let jp = "ミスビースト";
    let jw = text::measure(&fonts.jp, 38.0, jp, 10.0);
    // dark cartouche above the ribbon so the katakana reads over the star
    let cy = ctr.1 - 172.0;
    let chw = jw * 0.5 + 36.0;
    c.paint((ctr.0 - chw - 12.0, cy - 50.0, ctr.0 + chw + 12.0, cy + 50.0), |x, y| {
        let d = rrect((x, y), ctr.0 - chw, cy - 34.0, ctr.0 + chw, cy + 34.0, 34.0);
        if d > 4.0 {
            return None;
        }
        if let Some(t) = trim_paint(d, 4.0, gold, 1.0) {
            if d > -2.0 {
                return Some(t);
            }
        }
        Some((cov(d), Paint::col(hex(0x120f26)).h(H::Set(0.55)).relief(0.5).reg(reg::NAME)))
    });
    let m = text::line(&fonts.jp, 38.0, jp, 10.0, ctr.0 - jw * 0.5, cy + 14.0, 1.0);
    draw_mask(&mut c, &m, Paint::col(hex(0xf0e6cc)).h(H::Add(0.12)).relief(0.6).reg(reg::INK));

    // Fine print at the bottom of the field.
    let fp = "\u{00a9} 2026 KAZEHANA CARD WORKS";
    let fw = text::measure(&fonts.fine, 22.0, fp, 4.0);
    let m = text::line(&fonts.fine, 22.0, fp, 4.0, ctr.0 - fw * 0.5, hf - BORDER - 42.0, 1.0);
    draw_mask(&mut c, &m, Paint::col(gold * 0.8).reg(reg::INK));
    // glow ring around the seal
    c.paint((ctr.0 - 520.0, ctr.1 - 520.0, ctr.0 + 520.0, ctr.1 + 520.0), |x, y| {
        let d = circle((x, y), ctr, seal_r + 10.0);
        glow_line(d, 3.0, 0.6)
    });
    apply_glow(&mut c, &st);
    c
}
