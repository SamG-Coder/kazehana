//! First-person camera. World axes: +X east, +Y up, -Z north.

use glam::{Mat4, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub position: Vec3,
    /// Radians; 0 looks north (-Z), positive turns towards east (+X).
    pub yaw: f32,
    /// Radians; positive looks up.
    pub pitch: f32,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { position: Vec3::new(0.0, 1.5, 0.0), yaw: 0.0, pitch: 0.0, fov_y: 70f32.to_radians(), near: 0.05, far: 2000.0 }
    }
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), -self.yaw.cos() * self.pitch.cos())
    }

    /// Forward direction flattened onto the ground plane.
    pub fn forward_flat(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos())
    }

    pub fn right_flat(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin())
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_to_rh(self.position, self.forward(), Vec3::Y)
    }

    /// Vulkan projection: depth 0..1, Y flipped so +Y is up on screen.
    pub fn proj(&self, aspect: f32) -> Mat4 {
        let mut p = Mat4::perspective_rh(self.fov_y, aspect, self.near, self.far);
        p.y_axis.y *= -1.0;
        p
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.proj(aspect) * self.view()
    }
}
