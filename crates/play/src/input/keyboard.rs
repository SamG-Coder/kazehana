//! Keyboard and mouse: keys held and pressed, the mouse's movement since the last frame.

use glam::Vec2;
use std::collections::HashSet;
use winit::keyboard::KeyCode;

/// Which keys do what.
#[derive(Clone, Debug)]
pub struct Bindings {
    pub forward: Vec<KeyCode>,
    pub back: Vec<KeyCode>,
    pub left: Vec<KeyCode>,
    pub right: Vec<KeyCode>,
    pub run: Vec<KeyCode>,
    pub crouch: Vec<KeyCode>,
    pub jump: Vec<KeyCode>,
    pub interact: Vec<KeyCode>,
    pub menu: Vec<KeyCode>,
}

impl Default for Bindings {
    fn default() -> Self {
        use KeyCode::*;
        Bindings {
            forward: vec![KeyW, ArrowUp],
            back: vec![KeyS, ArrowDown],
            left: vec![KeyA, ArrowLeft],
            right: vec![KeyD, ArrowRight],
            run: vec![ShiftLeft, ShiftRight],
            crouch: vec![ControlLeft, KeyC],
            jump: vec![Space],
            interact: vec![KeyE],
            menu: vec![Escape],
        }
    }
}

/// The keyboard's and mouse's part of a frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyFrame {
    pub movement: Vec2,
    /// mouse counts (x right, y up)
    pub mouse: Vec2,
    pub run: bool,
    pub crouch: bool,
    pub jump: bool,
    pub interact: bool,
    pub menu: bool,
    /// a key pressed this frame (moving the mouse alone does not count)
    pub active: bool,
}

#[derive(Default)]
pub struct KeyboardMouse {
    pub bindings: Bindings,
    held: HashSet<KeyCode>,
    pressed: HashSet<KeyCode>,
    mouse: Vec2,
    /// the mouse turns the view only while the window has it (captured)
    pub mouse_look: bool,
}

impl KeyboardMouse {
    /// A key went down (`down`) or up. Repeats from holding a key are ignored.
    pub fn key(&mut self, k: KeyCode, down: bool, repeat: bool) {
        if down {
            if !repeat && self.held.insert(k) {
                self.pressed.insert(k);
            }
        } else {
            self.held.remove(&k);
        }
    }

    /// Raw mouse movement (device counts; y down, as the system reports it).
    pub fn mouse_motion(&mut self, dx: f64, dy: f64) {
        if self.mouse_look {
            self.mouse += Vec2::new(dx as f32, -dy as f32);
        }
    }

    /// Forget everything held (the window lost focus).
    pub fn release_all(&mut self) {
        self.held.clear();
        self.pressed.clear();
    }

    pub fn held(&self, k: KeyCode) -> bool {
        self.held.contains(&k)
    }

    /// This frame's keys and mouse (the presses and the mouse movement are taken).
    pub fn take(&mut self) -> KeyFrame {
        let b = &self.bindings;
        let any = |ks: &[KeyCode]| ks.iter().any(|k| self.held.contains(k));
        let hit = |ks: &[KeyCode]| ks.iter().any(|k| self.pressed.contains(k));
        let axis = |pos: &[KeyCode], neg: &[KeyCode]| any(pos) as i32 as f32 - any(neg) as i32 as f32;
        let f = KeyFrame {
            movement: Vec2::new(axis(&b.right, &b.left), axis(&b.forward, &b.back)).normalize_or_zero(),
            mouse: self.mouse,
            run: any(&b.run),
            crouch: any(&b.crouch),
            jump: hit(&b.jump),
            interact: hit(&b.interact),
            menu: hit(&b.menu),
            active: !self.pressed.is_empty(),
        };
        self.pressed.clear();
        self.mouse = Vec2::ZERO;
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_to_a_frame() {
        let mut k = KeyboardMouse::default();
        k.key(KeyCode::KeyW, true, false);
        k.key(KeyCode::KeyD, true, false);
        k.key(KeyCode::Space, true, false);
        let f = k.take();
        assert!((f.movement - Vec2::new(1.0, 1.0).normalize()).length() < 1e-6);
        assert!(f.jump && f.active);
        // a press counts once; holding (and the key's repeats) do not press again
        k.key(KeyCode::Space, true, true);
        assert!(!k.take().jump);
        k.key(KeyCode::KeyW, false, false);
        k.key(KeyCode::KeyD, false, false);
        assert_eq!(k.take().movement, Vec2::ZERO);
    }

    #[test]
    fn mouse_only_when_captured() {
        let mut k = KeyboardMouse::default();
        k.mouse_motion(10.0, 5.0);
        assert_eq!(k.take().mouse, Vec2::ZERO);
        k.mouse_look = true;
        k.mouse_motion(10.0, 5.0);
        assert_eq!(k.take().mouse, Vec2::new(10.0, -5.0));
    }
}
