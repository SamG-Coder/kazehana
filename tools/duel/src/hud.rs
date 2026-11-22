//! The duel's HUD in the main game's style (crates/game/src/draw.rs): translucent dark panels with
//! a thin red accent line, ink / dim / gold text, fading toasts at the bottom left and a hints line
//! at the bottom centre with the real button drawings (assets/ui/buttons) and keycaps.

use akari::{Rgba, Ui};

pub const INK: Rgba = Rgba(245, 242, 235, 255);
pub const DIM: Rgba = Rgba(185, 182, 176, 255);
pub const ACCENT: Rgba = Rgba(232, 72, 86, 255);
pub const GOLD: Rgba = Rgba(236, 196, 92, 255);
pub const PANEL: Rgba = Rgba(16, 18, 26, 214);
pub const GOOD: Rgba = Rgba(120, 220, 150, 255);
const G_INK: Rgba = Rgba(23, 18, 15, 255);
const G_BONE: Rgba = Rgba(242, 234, 219, 255);

/// A translucent panel with a soft drop shadow and a thin accent line on top.
pub fn panel(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, accent: Rgba) {
    let s = ui.scale();
    ui.rect(x + 4.0 * s, y + 6.0 * s, w, h, Rgba(0, 0, 0, 70));
    ui.rect(x, y, w, h, PANEL);
    ui.rect(x, y, w, 3.0 * s, accent.alpha(0.9));
}

/// A key's cap `h` high with its label. Returns its width.
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

/// A button drawing (`mouse_left`, ...) `h` high. Returns its width.
pub fn glyph(ui: &mut Ui, name: &str, x: f32, y: f32, h: f32, draw: bool) -> f32 {
    let m = h / 16.0;
    let w = if draw { ui.image(name, x - m, y - m, h + 2.0 * m, Rgba(255, 255, 255, 255)) } else { ui.image_width(name, h + 2.0 * m) };
    (w - 2.0 * m).max(0.0)
}

/// Buttons from crates/play prompts (keycaps or drawings), side by side. Returns their width.
pub fn draw_glyphs(ui: &mut Ui, gs: &[play::Glyph], x: f32, y: f32, h: f32) -> f32 {
    glyph_row(ui, gs, x, y, h, true)
}

pub fn glyphs_width(ui: &mut Ui, gs: &[play::Glyph], h: f32) -> f32 {
    glyph_row(ui, gs, 0.0, 0.0, h, false)
}

fn glyph_row(ui: &mut Ui, gs: &[play::Glyph], x: f32, y: f32, h: f32, draw: bool) -> f32 {
    let mut w = 0.0;
    for (k, g) in gs.iter().enumerate() {
        if k > 0 {
            w += h * 0.12;
        }
        w += match g {
            play::Glyph::Key(label) => keycap(ui, x + w, y, h, label, draw),
            play::Glyph::Image(name) => glyph(ui, name, x + w, y, h, draw),
        };
    }
    w
}

fn hint_row(ui: &mut Ui, items: &[(Vec<play::Glyph>, &str)], x: f32, y: f32, h: f32, px: f32, draw: bool) -> f32 {
    let mut w = 0.0;
    for (k, (gs, words)) in items.iter().enumerate() {
        if k > 0 {
            w += h * 0.9;
        }
        w += glyph_row(ui, gs, x + w, y, h, draw);
        w += h * 0.25;
        if draw {
            ui.text(x + w, y + (h - px * 1.25) * 0.5, px, DIM, words);
        }
        w += ui.measure(px, words) + h * 0.15;
    }
    w
}

/// The hints line at the bottom centre, with the buttons of the device in hand.
pub fn hints_glyphs(ui: &mut Ui, items: &[(Vec<play::Glyph>, &str)]) {
    let s = ui.scale();
    let (h, px) = (30.0 * s, 18.0 * s);
    let y = ui.height - 50.0 * s;
    let w = hint_row(ui, items, 0.0, y, h, px, false);
    let x = ui.width * 0.5 - w * 0.5;
    ui.rect(x - 16.0 * s, y - 6.0 * s, w + 32.0 * s, h + 12.0 * s, Rgba(10, 12, 18, 150));
    hint_row(ui, items, x, y, h, px, true);
}

/// Recent messages at the bottom left, fading out (newest at the bottom).
pub fn toasts(ui: &mut Ui, msgs: &[(String, f32)], bottom: f32) {
    let s = ui.scale();
    let mut y = bottom - 40.0 * s;
    for (m, age) in msgs.iter().rev().take(5) {
        let a = (1.0 - (age - 6.0) / 2.0).clamp(0.0, 1.0);
        if a <= 0.0 {
            continue;
        }
        let w = ui.measure(19.0 * s, m) + 28.0 * s;
        ui.rect(28.0 * s, y, w, 32.0 * s, PANEL.alpha(a * 0.9));
        ui.text(42.0 * s, y + 5.0 * s, 19.0 * s, INK.alpha(a), m);
        y -= 38.0 * s;
    }
}

/// A Leader's panel: name, HP bar, Spark gems, counts.
#[allow(clippy::too_many_arguments)]
pub fn leader_panel(ui: &mut Ui, x: f32, y: f32, name: &str, sub: &str, hp: i32, max_hp: i32, spark: (i32, i32), counts: &str, active: bool) {
    let s = ui.scale();
    let (w, h) = (480.0 * s, 132.0 * s);
    panel(ui, x, y, w, h, if active { GOLD } else { ACCENT });
    ui.text_shadow(x + 18.0 * s, y + 12.0 * s, 26.0 * s, INK, name);
    let nw = ui.measure(26.0 * s, name);
    ui.text_shadow(x + 28.0 * s + nw, y + 18.0 * s, 17.0 * s, DIM, sub);
    if active {
        ui.text_right(x + w - 18.0 * s, y + 16.0 * s, 16.0 * s, GOLD, "TO MOVE");
    }
    // HP: big number and bar
    let frac = (hp.max(0) as f32 / max_hp as f32).clamp(0.0, 1.0);
    ui.text_shadow(x + 18.0 * s, y + 44.0 * s, 44.0 * s, if frac < 0.35 { ACCENT } else { INK }, &hp.max(0).to_string());
    let hw = ui.measure(44.0 * s, &hp.max(0).to_string());
    ui.text_shadow(x + 24.0 * s + hw, y + 64.0 * s, 18.0 * s, DIM, &format!("/ {max_hp} HP"));
    let (bx, by, bw) = (x + 150.0 * s, y + 68.0 * s, w - 168.0 * s);
    ui.rect(bx, by, bw, 10.0 * s, Rgba(255, 255, 255, 40));
    ui.rect(bx, by, bw * frac, 10.0 * s, if frac < 0.35 { ACCENT } else { GOOD });
    // Spark gems
    ui.text_shadow(x + 18.0 * s, y + 100.0 * s, 16.0 * s, DIM, "SPARK");
    for i in 0..spark.1.max(0) {
        let gx = x + 84.0 * s + i as f32 * 22.0 * s;
        let lit = i < spark.0;
        ui.rect(gx, y + 101.0 * s, 15.0 * s, 15.0 * s, if lit { GOLD } else { Rgba(255, 255, 255, 34) });
    }
    ui.text_right(x + w - 18.0 * s, y + 100.0 * s, 16.0 * s, DIM, counts);
}

/// A dotted arrow from `a` to `b` (screen px).
pub fn arrow(ui: &mut Ui, a: (f32, f32), b: (f32, f32), c: Rgba) {
    let s = ui.scale();
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let n = (len / (14.0 * s)) as usize;
    for i in 0..n {
        let t = i as f32 / n.max(1) as f32;
        let r = (3.0 + 3.0 * t) * s;
        ui.rect(a.0 + dx * t - r, a.1 + dy * t - r, 2.0 * r, 2.0 * r, c.alpha(0.5 + 0.5 * t));
    }
    let r = 11.0 * s;
    ui.rect(b.0 - r, b.1 - r, 2.0 * r, 2.0 * r, c);
    ui.outline(b.0 - r - 4.0 * s, b.1 - r - 4.0 * s, 2.0 * r + 8.0 * s, 2.0 * r + 8.0 * s, 2.0 * s, INK);
}
