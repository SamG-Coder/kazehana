//! What to draw for an action: the buttons bound to it on whatever is in hand. The drawings are
//! Rust4x4's (tools/buttons): images named `<kit>_<control>` (an Xbox's or a PlayStation's),
//! `dpad_<ways>` and `mouse_<button>` in assets/ui/buttons; a key is drawn as a keycap with its
//! label (the UI draws those itself).
//!
//! The play crate's own actions are bound here already (`Prompts::new`); a game adds its own
//! (`bind_keys`, `bind_pad`), so a hint names an action and never a key.

use super::keyboard::Bindings;
use std::collections::HashMap;
use winit::keyboard::KeyCode;

/// Whose buttons to draw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kit {
    #[default]
    Keys,
    Xbox,
    PlayStation,
}

/// One button drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// a keycap with this on it
    Key(String),
    /// an image in assets/ui/buttons
    Image(String),
}

#[derive(Clone, Default)]
pub struct Prompts {
    keys: HashMap<String, Vec<KeyCode>>,
    mouse: HashMap<String, Vec<String>>,
    /// a controller's controls by their names: south east west north, l1 r1 l2 r2 l3 r3,
    /// select start, stick_left, stick_right (and _x, _y), dpad_<ways>
    pad: HashMap<String, Vec<String>>,
}

impl Prompts {
    /// The play crate's actions (move, look, run, crouch, jump, interact, menu) as bound.
    pub fn new(b: &Bindings) -> Self {
        let mut p = Prompts::default();
        let first = |v: &[KeyCode]| v.first().copied().into_iter().collect::<Vec<_>>();
        p.bind_keys("move", &[first(&b.forward), first(&b.left), first(&b.back), first(&b.right)].concat());
        p.bind_keys("run", &first(&b.run));
        p.bind_keys("crouch", &first(&b.crouch));
        p.bind_keys("jump", &first(&b.jump));
        p.bind_keys("interact", &first(&b.interact));
        p.bind_keys("menu", &first(&b.menu));
        p.bind_mouse("look", &["move"]);
        for (a, c) in [("move", "stick_left"), ("look", "stick_right"), ("run", "l3"), ("crouch", "east"), ("jump", "south"), ("interact", "west"), ("menu", "start")] {
            p.bind_pad(a, &[c]);
        }
        p
    }

    pub fn bind_keys(&mut self, action: &str, keys: &[KeyCode]) {
        self.keys.insert(action.into(), keys.to_vec());
    }

    pub fn bind_mouse(&mut self, action: &str, buttons: &[&str]) {
        self.mouse.insert(action.into(), buttons.iter().map(|s| s.to_string()).collect());
    }

    pub fn bind_pad(&mut self, action: &str, controls: &[&str]) {
        self.pad.insert(action.into(), controls.iter().map(|s| s.to_string()).collect());
    }

    /// The buttons for an action on this kit (empty: nothing bound there, so no hint).
    pub fn glyphs(&self, action: &str, kit: Kit) -> Vec<Glyph> {
        match kit {
            Kit::Keys => {
                let keys = self.keys.get(action).map(|ks| ks.iter().map(|k| Glyph::Key(key_label(*k))));
                let mouse = self.mouse.get(action).map(|ms| ms.iter().map(|m| Glyph::Image(format!("mouse_{m}"))));
                keys.into_iter().flatten().chain(mouse.into_iter().flatten()).collect()
            }
            Kit::Xbox | Kit::PlayStation => {
                let family = if kit == Kit::Xbox { "xbox" } else { "playstation" };
                self.pad
                    .get(action)
                    .map(|cs| cs.iter().map(|c| Glyph::Image(if c.starts_with("dpad_") { c.clone() } else { format!("{family}_{c}") })).collect())
                    .unwrap_or_default()
            }
        }
    }
}

/// What is printed on a key.
pub fn key_label(k: KeyCode) -> String {
    use KeyCode::*;
    let s = match k {
        Space => "Space",
        Enter | NumpadEnter => "Enter",
        Escape => "Esc",
        Backspace => "Bksp",
        Tab => "Tab",
        ShiftLeft | ShiftRight => "Shift",
        ControlLeft | ControlRight => "Ctrl",
        AltLeft | AltRight => "Alt",
        ArrowUp => "↑",
        ArrowDown => "↓",
        ArrowLeft => "←",
        ArrowRight => "→",
        BracketLeft => "[",
        BracketRight => "]",
        _ => {
            let d = format!("{k:?}");
            return d.trim_start_matches("Key").trim_start_matches("Digit").to_string();
        }
    };
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_for_actions() {
        let mut p = Prompts::new(&Bindings::default());
        assert_eq!(p.glyphs("move", Kit::Keys), ["W", "A", "S", "D"].map(|k| Glyph::Key(k.into())));
        assert_eq!(p.glyphs("interact", Kit::Keys), vec![Glyph::Key("E".into())]);
        assert_eq!(p.glyphs("look", Kit::Keys), vec![Glyph::Image("mouse_move".into())]);
        assert_eq!(p.glyphs("jump", Kit::PlayStation), vec![Glyph::Image("playstation_south".into())]);
        assert_eq!(p.glyphs("jump", Kit::Xbox), vec![Glyph::Image("xbox_south".into())]);
        p.bind_pad("choose", &["dpad_up_down"]);
        assert_eq!(p.glyphs("choose", Kit::Xbox), vec![Glyph::Image("dpad_up_down".into())]);
        assert!(p.glyphs("binder", Kit::PlayStation).is_empty());
    }
}
