//! Drawing helpers for the game's UI: panels, the HUD, in-game cards (placeholder frames until
//! the card templates from tools/cardgen are wired in).

use akari::{Rgba, Ui};
use sim::{GameState, Phase};
use tcg::{CardDef, Kind, Rarity};

pub const INK: Rgba = Rgba(245, 242, 235, 255);
pub const DIM: Rgba = Rgba(185, 182, 176, 255);
pub const ACCENT: Rgba = Rgba(232, 72, 86, 255);
pub const GOLD: Rgba = Rgba(236, 196, 92, 255);
pub const PANEL: Rgba = Rgba(16, 18, 26, 214);

/// A translucent panel with a thin accent line on top.
pub fn panel(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32) {
    let s = ui.scale();
    ui.rect(x + 4.0 * s, y + 6.0 * s, w, h, Rgba(0, 0, 0, 70));
    ui.rect(x, y, w, h, PANEL);
    ui.rect(x, y, w, 3.0 * s, ACCENT.alpha(0.9));
}

/// Dim the 3D view behind a full-screen menu.
pub fn dim(ui: &mut Ui, a: f32) {
    let (w, h) = (ui.width, ui.height);
    ui.rect(0.0, 0.0, w, h, Rgba(6, 8, 14, (a * 255.0) as u8));
}

pub fn phase_color(p: Phase) -> Rgba {
    match p {
        Phase::LateNight | Phase::Night => Rgba(110, 120, 220, 255),
        Phase::EarlyMorning | Phase::Morning => Rgba(255, 190, 120, 255),
        Phase::MorningClass | Phase::AfternoonClass => Rgba(120, 200, 160, 255),
        Phase::Lunch => Rgba(255, 220, 110, 255),
        Phase::AfterSchool | Phase::Daytime => Rgba(130, 190, 255, 255),
        Phase::Evening => Rgba(255, 140, 110, 255),
    }
}

/// Date / time / money panel (top left) and location (top right).
pub fn hud(ui: &mut Ui, st: &GameState, location: &str) {
    let s = ui.scale();
    let t = st.time;
    let (x, y) = (28.0 * s, 26.0 * s);
    panel(ui, x, y, 360.0 * s, 150.0 * s);
    let wd = t.date.weekday();
    ui.text_shadow(x + 18.0 * s, y + 12.0 * s, 26.0 * s, DIM, &format!("{}月{}日 ({})", t.date.month, t.date.day, wd.kanji()));
    let wdc = if wd.is_weekend() || sim::calendar::holiday(t.date).is_some() { ACCENT } else { DIM };
    ui.text_shadow(x + 190.0 * s, y + 14.0 * s, 22.0 * s, wdc, &format!("{:?} {} {}", wd, t.date.day, t.date.month_name()));
    ui.text_shadow(x + 18.0 * s, y + 44.0 * s, 54.0 * s, INK, &format!("{:02}:{:02}", t.minute / 60, t.minute % 60));
    let ph = t.phase();
    ui.text_shadow(x + 178.0 * s, y + 64.0 * s, 26.0 * s, phase_color(ph), ph.label());
    ui.text_shadow(x + 18.0 * s, y + 112.0 * s, 24.0 * s, GOLD, &format!("¥{}", st.player.money));
    // energy
    let (bx, by, bw) = (x + 170.0 * s, y + 120.0 * s, 170.0 * s);
    ui.text_shadow(bx, by - 24.0 * s, 16.0 * s, DIM, "ENERGY");
    ui.rect(bx, by, bw, 10.0 * s, Rgba(255, 255, 255, 40));
    let e = (st.player.energy as f32 / 100.0).clamp(0.0, 1.0);
    ui.rect(bx, by, bw * e, 10.0 * s, if e < 0.25 { ACCENT } else { Rgba(120, 220, 150, 255) });

    // location + season (top right)
    let w = 400.0 * s;
    let rx = ui.width - 28.0 * s;
    panel(ui, rx - w, y, w, 78.0 * s);
    ui.text_right(rx - 18.0 * s, y + 10.0 * s, 28.0 * s, INK, location);
    let season = tcg::season::theme(st.season).name;
    ui.text_right(rx - 18.0 * s, y + 46.0 * s, 18.0 * s, DIM, &format!("Season {}: {}", st.season, season));
}

/// Recent messages, bottom left, fading out.
pub fn toasts(ui: &mut Ui, msgs: &[(String, f32)]) {
    let s = ui.scale();
    let mut y = ui.height - 150.0 * s;
    for (m, age) in msgs.iter().rev().take(4) {
        let a = (1.0 - (age - 7.0) / 2.0).clamp(0.0, 1.0);
        if a <= 0.0 {
            continue;
        }
        let w = ui.measure(20.0 * s, m) + 28.0 * s;
        ui.rect(28.0 * s, y, w, 34.0 * s, PANEL.alpha(a * 0.9));
        ui.text(42.0 * s, y + 5.0 * s, 20.0 * s, INK.alpha(a), m);
        y -= 40.0 * s;
    }
}

// ------------------------------------------------------------------ buttons
// Whatever is pressed for something is drawn as it is on what is in hand: a keycap, the mouse,
// an Xbox controller's buttons or a PlayStation's (Rust4x4's drawings: tools/buttons). A hint
// names actions (`{interact} Talk`), never keys: play::Prompts knows what is bound to each.

/// The buttons to draw: what is bound to what, and whose.
pub struct Buttons<'a> {
    pub prompts: &'a play::Prompts,
    pub kit: play::Kit,
}

const G_INK: Rgba = Rgba(23, 18, 15, 255);
const G_BONE: Rgba = Rgba(242, 234, 219, 255);

/// A key's cap, `h` high, with its label: ink, a bone edge, a deeper lip at the bottom. Returns
/// its width.
pub fn keycap(ui: &mut Ui, x: f32, y: f32, h: f32, label: &str, draw: bool) -> f32 {
    let px = h * 0.47;
    let w = (ui.measure(px, label) + h * 0.54).max(h);
    if draw {
        let e = (h * 0.035).max(1.0);
        ui.rect(x, y, w, h, G_BONE.alpha(0.7));
        ui.rect(x + e, y + e, w - 2.0 * e, h - e - h * 0.13, G_INK);
        ui.text_centered(x + w * 0.5, y + (h - h * 0.13) * 0.5 - px * 0.62, px, G_BONE, label);
    }
    w
}

/// One button, `h` high (the images carry a margin of 2/32 of it round them). Returns its width.
pub fn glyph(ui: &mut Ui, g: &play::Glyph, x: f32, y: f32, h: f32, draw: bool) -> f32 {
    match g {
        play::Glyph::Key(label) => keycap(ui, x, y, h, label, draw),
        play::Glyph::Image(name) => {
            let m = h / 16.0;
            let w = if draw { ui.image(name, x - m, y - m, h + 2.0 * m, Rgba::WHITE) } else { ui.image_width(name, h + 2.0 * m) };
            (w - 2.0 * m).max(0.0)
        }
    }
}

/// The buttons for an action side by side. Returns their width (0: nothing bound on this kit).
pub fn action(ui: &mut Ui, b: &Buttons, act: &str, x: f32, y: f32, h: f32, draw: bool) -> f32 {
    let gs = b.prompts.glyphs(act, b.kit);
    let gap = h * 0.12;
    let mut w = 0.0;
    for (k, g) in gs.iter().enumerate() {
        if k > 0 {
            w += gap;
        }
        w += glyph(ui, g, x + w, y, h, draw);
    }
    w
}

/// A hint's parts: words, and `{action}`s.
fn parts(spec: &str) -> Vec<(bool, &str)> {
    let mut out = Vec::new();
    let mut rest = spec;
    while let Some(i) = rest.find('{') {
        if i > 0 {
            out.push((false, &rest[..i]));
        }
        let j = rest[i..].find('}').map_or(rest.len(), |j| i + j);
        out.push((true, &rest[i + 1..j]));
        rest = &rest[(j + 1).min(rest.len())..];
    }
    if !rest.is_empty() {
        out.push((false, rest));
    }
    out
}

/// A line of hints: `{action} words` items between " · ". An item with an action that has
/// nothing bound on the kit in hand is left out. Draws it (`draw`) with its left at x (centred on
/// x when `centre`); returns its width.
#[allow(clippy::too_many_arguments)]
fn hint_line(ui: &mut Ui, b: &Buttons, spec: &str, x: f32, y: f32, h: f32, px: f32, colour: Rgba, centre: bool, draw: bool) -> f32 {
    let items: Vec<Vec<(bool, &str)>> = spec.split(" · ").map(parts).filter(|ps| ps.iter().all(|(a, s)| !a || !b.prompts.glyphs(s, b.kit).is_empty())).collect();
    let sep = h * 0.9;
    let measure = |ui: &mut Ui, draw: bool, x0: f32| -> f32 {
        let mut w = 0.0;
        for (k, item) in items.iter().enumerate() {
            if k > 0 {
                w += sep;
            }
            for (is_act, s) in item {
                if *is_act {
                    w += action(ui, b, s, x0 + w, y, h, draw);
                } else {
                    let t = s.trim();
                    if t.is_empty() {
                        continue;
                    }
                    w += h * 0.25;
                    if draw {
                        ui.text(x0 + w, y + (h - px * 1.25) * 0.5, px, colour, t);
                    }
                    w += ui.measure(px, t) + h * 0.15;
                }
            }
        }
        w
    };
    let w = measure(ui, false, 0.0);
    if draw {
        measure(ui, true, if centre { x - w * 0.5 } else { x });
    }
    w
}

/// The prompt at the bottom centre: the button for `act`, then what it does ("Talk to ...").
pub fn prompt(ui: &mut Ui, b: &Buttons, act: &str, text: &str) {
    let s = ui.scale();
    let (h, px) = (34.0 * s, 24.0 * s);
    let bw = action(ui, b, act, 0.0, 0.0, h, false);
    let w = bw + ui.measure(px, text) + 48.0 * s;
    let (x, y) = (ui.width * 0.5 - w * 0.5, ui.height - 120.0 * s);
    panel(ui, x, y, w, 52.0 * s);
    action(ui, b, act, x + 14.0 * s, y + 9.0 * s, h, true);
    ui.text(x + 28.0 * s + bw, y + 11.0 * s, px, INK, text);
}

/// The hints line at the bottom of the screen ("{choose} Choose · {confirm} Confirm").
pub fn hints(ui: &mut Ui, b: &Buttons, spec: &str) {
    let s = ui.scale();
    let (h, px) = (30.0 * s, 18.0 * s);
    let y = ui.height - 50.0 * s;
    let w = hint_line(ui, b, spec, 0.0, y, h, px, DIM, false, false);
    let x = ui.width * 0.5 - w * 0.5;
    ui.rect(x - 16.0 * s, y - 6.0 * s, w + 32.0 * s, h + 12.0 * s, Rgba(10, 12, 18, 150));
    hint_line(ui, b, spec, ui.width * 0.5, y, h, px, DIM, true, true);
}

/// The button for an action with its words, right-aligned at `rx` ("▼ {confirm}").
pub fn hint_right(ui: &mut Ui, b: &Buttons, spec: &str, rx: f32, y: f32) {
    let s = ui.scale();
    let (h, px) = (26.0 * s, 18.0 * s);
    let w = hint_line(ui, b, spec, 0.0, y, h, px, DIM, false, false);
    hint_line(ui, b, spec, rx - w, y, h, px, DIM, false, true);
}


pub fn rarity_color(r: Rarity) -> Rgba {
    match r {
        Rarity::Common => Rgba(170, 176, 186, 255),
        Rarity::Uncommon => Rgba(90, 200, 190, 255),
        Rarity::Rare => GOLD,
        Rarity::Mythic => Rgba(220, 120, 255, 255),
    }
}

pub fn rarity_name(r: Rarity) -> &'static str {
    match r {
        Rarity::Common => "Common",
        Rarity::Uncommon => "Uncommon",
        Rarity::Rare => "Rare",
        Rarity::Mythic => "Mythic",
    }
}

fn shade(c: [u8; 3], k: f32) -> Rgba {
    Rgba((c[0] as f32 * k).min(255.0) as u8, (c[1] as f32 * k).min(255.0) as u8, (c[2] as f32 * k).min(255.0) as u8, 255)
}

/// Card size for a width (63 x 88 mm proportions).
pub fn card_h(w: f32) -> f32 {
    w * 88.0 / 63.0
}

/// An in-game card face (placeholder frame), top-left at (x, y).
pub fn card(ui: &mut Ui, x: f32, y: f32, w: f32, c: &CardDef, foil: bool, selected: bool) {
    let h = card_h(w);
    let k = w / 300.0; // design units: a 300 px wide card
    let att = c.attribute.color();
    ui.rect(x + 6.0 * k, y + 8.0 * k, w, h, Rgba(0, 0, 0, 90));
    if selected {
        ui.rect(x - 6.0 * k, y - 6.0 * k, w + 12.0 * k, h + 12.0 * k, INK);
    }
    // border by rarity
    let rc = rarity_color(c.rarity);
    ui.rect(x, y, w, h, Rgba(18, 18, 22, 255));
    ui.outline(x + 4.0 * k, y + 4.0 * k, w - 8.0 * k, h - 8.0 * k, 5.0 * k, rc);
    if c.rarity == Rarity::Mythic {
        let bands = [Rgba(255, 110, 140, 255), Rgba(255, 210, 110, 255), Rgba(120, 230, 170, 255), Rgba(120, 170, 255, 255), Rgba(200, 130, 255, 255)];
        let bw = (w - 8.0 * k) / bands.len() as f32;
        for (i, b) in bands.iter().enumerate() {
            ui.rect(x + 4.0 * k + i as f32 * bw, y + 4.0 * k, bw, 5.0 * k, *b);
            ui.rect(x + 4.0 * k + i as f32 * bw, y + h - 9.0 * k, bw, 5.0 * k, *b);
        }
    }
    // body in the attribute colour
    let (ix, iy, iw, ih) = (x + 12.0 * k, y + 12.0 * k, w - 24.0 * k, h - 24.0 * k);
    ui.gradient(ix, iy, iw, ih, shade(att, 1.05), shade(att, 0.45));
    // name bar
    ui.rect(ix + 6.0 * k, iy + 6.0 * k, iw - 12.0 * k, 34.0 * k, Rgba(250, 246, 236, 235));
    let mut name_px = 20.0 * k;
    while ui.measure(name_px, &c.name) > iw - 60.0 * k && name_px > 10.0 * k {
        name_px -= 1.0;
    }
    ui.text(ix + 14.0 * k, iy + 11.0 * k, name_px, Rgba(24, 22, 26, 255), &c.name);
    ui.text_right(ix + iw - 14.0 * k, iy + 12.0 * k, 18.0 * k, Rgba(24, 22, 26, 255), c.rarity.mark());
    // art window (placeholder: the art arrives from ChatGPT; assets/cards/art)
    let (ax, ay, aw) = (ix + 10.0 * k, iy + 48.0 * k, iw - 20.0 * k);
    let ah = aw * 0.78;
    ui.gradient(ax, ay, aw, ah, shade(att, 0.75), shade(att, 0.22));
    ui.outline(ax, ay, aw, ah, 2.0 * k, Rgba(250, 246, 236, 200));
    let glyph = match c.kind {
        Kind::Monster => "獣",
        Kind::Spell => "術",
        Kind::Relic => "宝",
    };
    ui.text_centered(ax + aw * 0.5, ay + ah * 0.18, 96.0 * k, Rgba(255, 255, 255, 70), glyph);
    if foil {
        // static sheen stripes (the real foil comes with the card templates)
        for i in 0..6 {
            let t = i as f32 / 6.0;
            ui.rect(ax + t * aw, ay, aw / 14.0, ah, Rgba(255, 255, 255, 38));
        }
        ui.rect(ax + aw - 62.0 * k, ay + 8.0 * k, 54.0 * k, 22.0 * k, GOLD);
        ui.text_centered(ax + aw - 35.0 * k, ay + 10.0 * k, 15.0 * k, Rgba(30, 20, 10, 255), "HOLO");
    }
    // type line
    let ty = ay + ah + 8.0 * k;
    ui.rect(ix + 6.0 * k, ty, iw - 12.0 * k, 26.0 * k, Rgba(20, 20, 24, 200));
    ui.text(ix + 14.0 * k, ty + 3.0 * k, 16.0 * k, INK, &format!("{:?} — {}", c.kind, c.attribute));
    // flavour
    ui.text_wrapped(ix + 14.0 * k, ty + 34.0 * k, iw - 28.0 * k, 14.0 * k, Rgba(250, 246, 236, 230), &c.flavor);
    // footer
    ui.text(ix + 10.0 * k, iy + ih - 22.0 * k, 13.0 * k, Rgba(250, 246, 236, 200), &c.id.to_string());
}

/// The back of a card.
pub fn card_back(ui: &mut Ui, x: f32, y: f32, w: f32) {
    let h = card_h(w);
    let k = w / 300.0;
    ui.rect(x + 6.0 * k, y + 8.0 * k, w, h, Rgba(0, 0, 0, 90));
    ui.rect(x, y, w, h, Rgba(14, 16, 34, 255));
    ui.outline(x + 8.0 * k, y + 8.0 * k, w - 16.0 * k, h - 16.0 * k, 4.0 * k, GOLD);
    ui.gradient(x + 20.0 * k, y + 20.0 * k, w - 40.0 * k, h - 40.0 * k, Rgba(46, 30, 90, 255), Rgba(12, 40, 80, 255));
    ui.text_centered(x + w * 0.5, y + h * 0.40, 34.0 * k, GOLD, "MYTHBEASTS");
    ui.text_centered(x + w * 0.5, y + h * 0.40 + 44.0 * k, 16.0 * k, DIM, "TRADING CARD GAME");
}
