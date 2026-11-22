//! Input: what the player asks for each frame, from the keyboard and mouse or a gamepad.
//!
//! [`Input`] owns both devices. Feed it the window's keyboard and mouse events
//! ([`Input::keyboard`]), call [`Input::frame`] once a frame, act on the [`Actions`].
//!
//! Default bindings:
//!
//! | action   | keyboard / mouse       | PlayStation (Xbox)              |
//! |----------|------------------------|---------------------------------|
//! | move     | W A S D                | left stick                      |
//! | look     | mouse                  | right stick                     |
//! | run      | Shift (held)           | L3 (toggles; stops with you)    |
//! | crouch   | Ctrl or C (held)       | Circle / B (toggles)            |
//! | jump     | Space                  | Cross / A                       |
//! | interact | E                      | Square / X                      |
//! | menu     | Escape                 | Options / Menu                  |

pub mod gamepad;
pub mod keyboard;
pub mod prompts;

use glam::Vec2;
pub use gamepad::{Button, Pad, Pads};
pub use keyboard::{Bindings, KeyboardMouse};
pub use prompts::{Glyph, Kit, Prompts};

/// Which device the player used last (prompts show its buttons).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Device {
    #[default]
    Keyboard,
    Gamepad,
}

/// What the player asks for this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Actions {
    /// x right, y forward; length at most 1 (a stick part way is a slower walk)
    pub movement: Vec2,
    /// the turn this frame, radians: x towards the right, y up
    pub look: Vec2,
    pub run: bool,
    pub crouch: bool,
    /// pressed this frame
    pub jump: bool,
    /// pressed this frame
    pub interact: bool,
    /// pressed this frame
    pub menu: bool,
    pub device: Device,
    /// whose buttons to draw for hints
    pub kit: Kit,
}

/// Look speed and feel.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    /// radians per mouse count
    pub mouse: f32,
    /// radians per second at full right stick (yaw, pitch)
    pub stick: Vec2,
    pub invert_y: bool,
}

impl Default for Look {
    fn default() -> Self {
        Look { mouse: 0.0022, stick: Vec2::new(3.2, 2.2), invert_y: false }
    }
}

pub struct Input {
    kb: KeyboardMouse,
    pads: Option<Pads>,
    pub look: Look,
    run_toggle: bool,
    crouch_toggle: bool,
    device: Device,
    playstation: bool,
    pad: Pad,
}

impl Input {
    /// `gamepads`: look for controllers (a background thread watches for PlayStation ones).
    pub fn new(gamepads: bool) -> Self {
        Input { kb: KeyboardMouse::default(), pads: gamepads.then(Pads::new), look: Look::default(), run_toggle: false, crouch_toggle: false, device: Device::Keyboard, playstation: false, pad: Pad::default() }
    }

    /// The keyboard and mouse: give it the window's events.
    pub fn keyboard(&mut self) -> &mut KeyboardMouse {
        &mut self.kb
    }

    /// This frame's actions (`dt`: seconds since the last frame, for the stick's turn rate).
    /// `playing`: the player is walking about. When not (a menu or screen is open: its buttons
    /// are the screen's), nothing is asked of the body and the toggles (run, crouch) keep their
    /// state; which device is in hand is still followed.
    pub fn frame(&mut self, dt: f32, playing: bool) -> Actions {
        let k = self.kb.take();
        let p = self.pads.as_mut().map(|p| p.poll()).unwrap_or_default();
        if k.active {
            self.device = Device::Keyboard;
        }
        if p.active() {
            self.device = Device::Gamepad;
            self.playstation = p.playstation;
        }
        self.pad = p;
        if !playing {
            return Actions { device: self.device, kit: self.kit(), ..Default::default() };
        }
        // movement: whichever asks for more
        let stick = Vec2::new(p.left.x, p.left.y);
        let movement = if stick.length() > k.movement.length() { stick } else { k.movement };
        // sprint toggle (L3) holds until the stick comes back to rest; crouch toggles
        if p.pressed(Button::L3) {
            self.run_toggle = !self.run_toggle;
        }
        if stick.length() < 0.2 {
            self.run_toggle = false;
        }
        if p.pressed(Button::Circle) {
            self.crouch_toggle = !self.crouch_toggle;
        }
        if k.crouch {
            self.crouch_toggle = false;
        }
        let mut look = k.mouse * self.look.mouse + Vec2::new(p.right.x * self.look.stick.x, p.right.y * self.look.stick.y) * dt;
        if self.look.invert_y {
            look.y = -look.y;
        }
        Actions {
            movement: movement.clamp_length_max(1.0),
            look,
            run: k.run || self.run_toggle,
            crouch: k.crouch || self.crouch_toggle,
            jump: k.jump || p.pressed(Button::Cross),
            interact: k.interact || p.pressed(Button::Square),
            menu: k.menu || p.pressed(Button::Options),
            device: self.device,
            kit: self.kit(),
        }
    }

    /// Whose buttons to draw: the keyboard's, or the controller's in hand.
    pub fn kit(&self) -> Kit {
        match (self.device, self.playstation) {
            (Device::Keyboard, _) => Kit::Keys,
            (Device::Gamepad, true) => Kit::PlayStation,
            (Device::Gamepad, false) => Kit::Xbox,
        }
    }

    /// The controller this frame (its buttons held and pressed: for screens and menus).
    pub fn pad(&self) -> &Pad {
        &self.pad
    }
}
