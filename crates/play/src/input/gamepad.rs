//! Gamepads. Two ways in:
//!
//! - PlayStation controllers (DualSense, DualSense Edge, DualShock 4) read directly from their own
//!   HID reports, on a thread of their own: Windows does not always hand them to its gamepad
//!   interface (a DualSense on USB can be listed by the system and by Steam and still not by
//!   Windows Gaming Input), and their reports are simple and known.
//! - Everything else the system reports as a gamepad (Xbox controllers and the like), through
//!   gilrs, mapped onto the same buttons (Cross = A, Circle = B, Square = X, Triangle = Y, ...).
//!
//! Ported from SmartImposter (src/gamepad.rs), with the full button set and no flight bindings.

use gilrs::{Axis, Button as GButton, EventType, GamepadId, Gilrs};
use glam::Vec2;
use std::sync::{Arc, Mutex};

/// A controller's buttons, by their PlayStation names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Button {
    Cross = 1,
    Circle = 1 << 1,
    Square = 1 << 2,
    Triangle = 1 << 3,
    L1 = 1 << 4,
    R1 = 1 << 5,
    L2 = 1 << 6,
    R2 = 1 << 7,
    Create = 1 << 8,
    Options = 1 << 9,
    L3 = 1 << 10,
    R3 = 1 << 11,
    Ps = 1 << 12,
    Touchpad = 1 << 13,
    Up = 1 << 14,
    Down = 1 << 15,
    Left = 1 << 16,
    Right = 1 << 17,
}

/// A controller this frame. Sticks: x right, y forward / up, after the dead zone and the
/// response curve; triggers 0..1.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pad {
    pub left: Vec2,
    pub right: Vec2,
    pub l2: f32,
    pub r2: f32,
    /// buttons held (bits of [`Button`])
    pub held: u32,
    /// buttons pressed since the last frame
    pub pressed_bits: u32,
    pub connected: bool,
    /// a PlayStation controller (else drawn as an Xbox's)
    pub playstation: bool,
}

impl Pad {
    pub fn held(&self, b: Button) -> bool {
        self.held & b as u32 != 0
    }
    pub fn pressed(&self, b: Button) -> bool {
        self.pressed_bits & b as u32 != 0
    }
    /// taken in hand this frame: a press, a trigger pulled or a stick pushed past half way (a
    /// stick resting off its middle changes nothing)
    pub fn active(&self) -> bool {
        self.pressed_bits != 0 || self.left.length() > 0.5 || self.right.length() > 0.5 || self.l2 > 0.5 || self.r2 > 0.5
    }
}

/// A stick's raw position to what it asks for: nothing inside the dead zone (a stick at rest is
/// never exactly centred), then a curve that is gentle near the centre and reaches all of it at
/// the rim.
pub fn shape(x: f32, y: f32) -> Vec2 {
    const DEAD: f32 = 0.14;
    let v = Vec2::new(x, y);
    let m = v.length();
    if m <= DEAD {
        return Vec2::ZERO;
    }
    let t = ((m - DEAD) / (1.0 - DEAD)).min(1.0);
    v * (t.powf(1.7) / m)
}

fn trigger(v: f32) -> f32 {
    ((v - 0.06) / 0.94).clamp(0.0, 1.0)
}

// ------------------------------------------------------------------------- PlayStation (HID)

const SONY: u16 = 0x054C;
/// DualSense, DualSense Edge, DualShock 4 (two models), its wireless adaptor
const SONY_PADS: [u16; 5] = [0x0CE6, 0x0DF2, 0x05C4, 0x09CC, 0x0BA0];

/// A PlayStation controller's sticks, triggers and buttons, as its report gives them.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct SonyReport {
    /// left x, left y, right x, right y: 0..255, 128 at rest, y down
    sticks: [u8; 4],
    l2: u8,
    r2: u8,
    /// low four bits the D-pad (0 up, clockwise, 8 at rest), then Square, Cross, Circle, Triangle
    b0: u8,
    /// L1, R1, L2, R2, Create / Share, Options, L3, R3
    b1: u8,
    /// PS, touchpad (the rest: a counter on the DualShock 4)
    b2: u8,
}

/// One input report to what it says. The DualSense on USB (report 1, 64 bytes) and on Bluetooth
/// (report 0x31, one byte more in front) send sticks, triggers, a counter, buttons; the
/// DualShock 4 (report 1 on USB, 0x11 on Bluetooth), and a DualSense on Bluetooth before it is
/// asked for its full report, send sticks, buttons, triggers.
fn sony_report(dualsense: bool, r: &[u8]) -> Option<SonyReport> {
    let (at, sense) = match (r.first()?, dualsense) {
        (0x31, true) => (2, true),
        (0x01, true) if r.len() >= 60 => (1, true),
        (0x01, _) => (1, false),
        (0x11, false) => (3, false),
        _ => return None,
    };
    let r = r.get(at..at + 10)?;
    let sticks = [r[0], r[1], r[2], r[3]];
    Some(if sense {
        SonyReport { sticks, l2: r[4], r2: r[5], b0: r[7], b1: r[8], b2: r[9] }
    } else {
        SonyReport { sticks, l2: r[7], r2: r[8], b0: r[4], b1: r[5], b2: r[6] & 3 }
    })
}

impl SonyReport {
    fn buttons(&self) -> u32 {
        use Button::*;
        let mut b = 0u32;
        let mut set = |on: bool, x: Button| {
            if on {
                b |= x as u32
            }
        };
        set(self.b0 & 0x10 != 0, Square);
        set(self.b0 & 0x20 != 0, Cross);
        set(self.b0 & 0x40 != 0, Circle);
        set(self.b0 & 0x80 != 0, Triangle);
        let d = self.b0 & 15;
        set(matches!(d, 7 | 0 | 1), Up);
        set(matches!(d, 1..=3), Right);
        set(matches!(d, 3..=5), Down);
        set(matches!(d, 5..=7), Left);
        for (bit, x) in [(0x01, L1), (0x02, R1), (0x04, L2), (0x08, R2), (0x10, Create), (0x20, Options), (0x40, L3), (0x80, R3)] {
            set(self.b1 & bit != 0, x);
        }
        set(self.b2 & 0x01 != 0, Ps);
        set(self.b2 & 0x02 != 0, Touchpad);
        b
    }
}

/// The newest report, and the presses since the last poll.
#[derive(Default)]
struct SonyState {
    connected: bool,
    now: SonyReport,
    pressed: u32,
}

impl SonyState {
    fn take(&mut self) -> Pad {
        let r = self.now;
        let axis = |v: u8| ((v as f32 - 127.5) / 127.5).clamp(-1.0, 1.0);
        let pad = Pad {
            left: shape(axis(r.sticks[0]), -axis(r.sticks[1])),
            right: shape(axis(r.sticks[2]), -axis(r.sticks[3])),
            l2: trigger(r.l2 as f32 / 255.0),
            r2: trigger(r.r2 as f32 / 255.0),
            held: r.buttons(),
            pressed_bits: self.pressed,
            connected: true,
            playstation: true,
        };
        self.pressed = 0;
        pad
    }
}

/// Find a PlayStation controller, read it until it goes, look again (a read waits for the next
/// report; a controller sends 250 to 1000 a second).
fn sony_thread(state: Arc<Mutex<SonyState>>) {
    let Ok(mut api) = hidapi::HidApi::new() else { return };
    loop {
        let _ = api.refresh_devices();
        let found = api.device_list().find(|d| d.vendor_id() == SONY && SONY_PADS.contains(&d.product_id())).cloned();
        let Some((info, dev)) = found.and_then(|info| info.open_device(&api).ok().map(|d| (info, d))) else {
            std::thread::sleep(std::time::Duration::from_millis(1000));
            continue;
        };
        let dualsense = matches!(info.product_id(), 0x0CE6 | 0x0DF2);
        if dualsense {
            // (on Bluetooth a DualSense sends its full report once this has been asked for)
            let mut cal = [0u8; 64];
            cal[0] = 0x05;
            let _ = dev.get_feature_report(&mut cal);
        }
        println!("Controller: {}", info.product_string().unwrap_or(if dualsense { "DualSense" } else { "DualShock 4" }));
        let mut buf = [0u8; 128];
        let mut last = 0u32;
        let mut quiet = 0;
        loop {
            match dev.read_timeout(&mut buf, 500) {
                Ok(0) => {
                    // (nothing for a while: switched off, or gone)
                    quiet += 1;
                    if quiet > 6 {
                        break;
                    }
                }
                Ok(n) => {
                    quiet = 0;
                    let Some(r) = sony_report(dualsense, &buf[..n]) else { continue };
                    let b = r.buttons();
                    let mut s = state.lock().unwrap();
                    s.connected = true;
                    s.pressed |= b & !last;
                    s.now = r;
                    last = b;
                }
                Err(_) => break,
            }
        }
        state.lock().unwrap().connected = false;
        println!("Controller disconnected");
    }
}

// ------------------------------------------------------------------------- the rest (gilrs)

/// A controller's name says it is a PlayStation's (Windows calls them "Wireless Controller").
fn is_playstation(name: &str) -> bool {
    let n = name.to_lowercase();
    ["dualsense", "dualshock", "playstation", "ps4", "ps5", "sony", "wireless controller"].iter().any(|k| n.contains(k))
}

pub struct Pads {
    gilrs: Option<Gilrs>,
    /// the controller last touched
    active: Option<GamepadId>,
    last_held: u32,
    sony: Arc<Mutex<SonyState>>,
}

impl Default for Pads {
    fn default() -> Self {
        Self::new()
    }
}

impl Pads {
    pub fn new() -> Self {
        let sony = Arc::new(Mutex::new(SonyState::default()));
        let state = sony.clone();
        let _ = std::thread::Builder::new().name("controller".into()).spawn(move || sony_thread(state));
        let gilrs = Gilrs::new().map_err(|e| println!("Controllers: PlayStation only ({e})")).ok();
        Pads { gilrs, active: None, last_held: 0, sony }
    }

    /// The controller in use this frame (a PlayStation one first, if connected).
    pub fn poll(&mut self) -> Pad {
        {
            let mut s = self.sony.lock().unwrap();
            if s.connected {
                return s.take();
            }
        }
        let Some(gilrs) = self.gilrs.as_mut() else { return Pad::default() };
        while let Some(ev) = gilrs.next_event() {
            match ev.event {
                EventType::Connected => {
                    println!("Controller: {}", gilrs.gamepad(ev.id).name());
                    self.active = Some(ev.id);
                }
                EventType::Disconnected => {
                    println!("Controller disconnected");
                    if self.active == Some(ev.id) {
                        self.active = gilrs.gamepads().map(|(id, _)| id).find(|&id| id != ev.id);
                    }
                }
                EventType::ButtonPressed(..) | EventType::AxisChanged(..) | EventType::ButtonChanged(..) => self.active = Some(ev.id),
                _ => {}
            }
        }
        if self.active.is_none() {
            self.active = gilrs.gamepads().next().map(|(id, _)| id);
        }
        let Some(g) = self.active.and_then(|id| gilrs.connected_gamepad(id)) else { return Pad::default() };
        // (a trigger is a button with a value on most controllers, an axis on some)
        let trig = |b: GButton, a: Axis| trigger(g.button_data(b).map(|d| d.value()).unwrap_or(0.0).max(g.value(a)).max(if g.is_pressed(b) { 1.0 } else { 0.0 }));
        let mut held = 0u32;
        for (gb, b) in [
            (GButton::South, Button::Cross),
            (GButton::East, Button::Circle),
            (GButton::West, Button::Square),
            (GButton::North, Button::Triangle),
            (GButton::LeftTrigger, Button::L1),
            (GButton::RightTrigger, Button::R1),
            (GButton::LeftTrigger2, Button::L2),
            (GButton::RightTrigger2, Button::R2),
            (GButton::Select, Button::Create),
            (GButton::Start, Button::Options),
            (GButton::LeftThumb, Button::L3),
            (GButton::RightThumb, Button::R3),
            (GButton::Mode, Button::Ps),
            (GButton::DPadUp, Button::Up),
            (GButton::DPadDown, Button::Down),
            (GButton::DPadLeft, Button::Left),
            (GButton::DPadRight, Button::Right),
        ] {
            if g.is_pressed(gb) {
                held |= b as u32;
            }
        }
        let pad = Pad {
            left: shape(g.value(Axis::LeftStickX), g.value(Axis::LeftStickY)),
            right: shape(g.value(Axis::RightStickX), g.value(Axis::RightStickY)),
            l2: trig(GButton::LeftTrigger2, Axis::LeftZ),
            r2: trig(GButton::RightTrigger2, Axis::RightZ),
            held,
            pressed_bits: held & !self.last_held,
            connected: true,
            playstation: is_playstation(g.name()),
        };
        self.last_held = held;
        pad
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playstation_reports() {
        // a DualSense on USB: report 1, then sticks, triggers, a counter, buttons
        let mut usb = [0u8; 64];
        usb[..11].copy_from_slice(&[0x01, 10, 20, 30, 40, 50, 60, 99, 0x28, 0x21, 0x01]);
        let want = SonyReport { sticks: [10, 20, 30, 40], l2: 50, r2: 60, b0: 0x28, b1: 0x21, b2: 0x01 };
        assert_eq!(sony_report(true, &usb), Some(want));
        // the same on Bluetooth: report 0x31 and one byte more in front
        let mut bt = [0u8; 78];
        bt[..12].copy_from_slice(&[0x31, 0x01, 10, 20, 30, 40, 50, 60, 99, 0x28, 0x21, 0x01]);
        assert_eq!(sony_report(true, &bt), Some(want));
        // a DualShock 4 on USB: sticks, buttons, triggers
        let mut ds4 = [0u8; 64];
        ds4[..10].copy_from_slice(&[0x01, 10, 20, 30, 40, 0x28, 0x21, 0x01, 50, 60]);
        assert_eq!(sony_report(false, &ds4), Some(want));
        assert_eq!(sony_report(true, &[0x09, 1, 2, 3]), None);
    }

    #[test]
    fn playstation_buttons() {
        // Cross and the D-pad right; L1, Options; PS
        let r = SonyReport { sticks: [128; 4], b0: 0x20 | 2, b1: 0x01 | 0x20, b2: 0x01, ..Default::default() };
        let b = r.buttons();
        for x in [Button::Cross, Button::Right, Button::L1, Button::Options, Button::Ps] {
            assert!(b & x as u32 != 0, "{x:?}");
        }
        for x in [Button::Circle, Button::Square, Button::Up, Button::Down, Button::Left, Button::R3] {
            assert!(b & x as u32 == 0, "{x:?}");
        }
        // at rest: nothing asked; presses are acted on once
        let mut s = SonyState { connected: true, now: SonyReport { sticks: [128, 127, 128, 128], b0: 8, ..Default::default() }, pressed: Button::Cross as u32 };
        let p = s.take();
        assert!(p.left == Vec2::ZERO && p.right == Vec2::ZERO && p.pressed(Button::Cross) && !p.held(Button::Cross));
        assert!(!s.take().pressed(Button::Cross));
        // left stick forward: y up
        s.now.sticks = [128, 0, 255, 128];
        let p = s.take();
        assert!(p.left.y > 0.99 && p.right.x > 0.99);
    }

    #[test]
    fn stick_curve() {
        assert_eq!(shape(0.08, -0.05), Vec2::ZERO);
        let v = shape(1.0, 0.0);
        assert!((v.x - 1.0).abs() < 1e-6 && v.y == 0.0);
        let h = shape(0.5, 0.0).x;
        assert!(h > 0.15 && h < 0.35, "half stick asks {h}");
        let mut last = 0.0;
        for i in 0..=100 {
            let v = shape(i as f32 / 100.0, 0.0).x;
            assert!(v >= last);
            last = v;
        }
    }

    /// The PlayStation reader for ten seconds, a line whenever what it reads changes:
    /// `cargo test -p play --release sony_probe -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn sony_probe() {
        let mut pads = Pads::new();
        let t = std::time::Instant::now();
        let mut last = String::new();
        while t.elapsed().as_secs_f32() < 10.0 {
            let p = pads.poll();
            let line = format!("{:?} {:?} l2 {:.2} r2 {:.2} held {:#07x}", p.left, p.right, p.l2, p.r2, p.held);
            if line != last {
                println!("{:.2}s {line}", t.elapsed().as_secs_f32());
                last = line;
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }
}
