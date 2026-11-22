//! Spring bones (docs/cloth_plan.md, tier C): the joints of things that hang and swing (skirts,
//! aprons, long hair, ponytails) lag behind the body as it moves and settle back, on top of the
//! animation.
//!
//! A spring joint is any joint named `spring_*` (but not `spring_*_end`) with a child: its tail
//! is its first child's head. Each frame, after the clip is sampled, every spring's tail is moved
//! in the world as a particle with inertia, pulled back towards where the animation puts it
//! (`stiffness`, 1/s), slowed by the air (`drag`), held at the bone's length from its head; the
//! joint is then turned so the bone points at it. Chains work from the root down (joints come in
//! parent order), so a lower bone swings from its upper one's new place.
//!
//! The kind of thing comes from the name: `spring_skirt*`, `spring_apron*`, `spring_hair*`.
//! Skirt tails are kept outside a cylinder round the hips (they never swing in through the legs).

use crate::{Pose, Skeleton};
use glam::{Mat4, Quat, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct SpringParams {
    /// how fast a tail returns to where the animation puts it (1/s)
    pub stiffness: f32,
    /// velocity lost per frame at 60 Hz to the air (0..1)
    pub drag: f32,
    /// pull downwards (m/s²) beyond what the animated shape already has
    pub gravity: f32,
    /// the most a bone turns away from its animated direction (radians)
    pub max_angle: f32,
}

impl SpringParams {
    pub fn for_name(name: &str) -> SpringParams {
        if name.starts_with("spring_skirt") {
            SpringParams { stiffness: 4.5, drag: 0.14, gravity: 0.0, max_angle: 0.6 }
        } else if name.starts_with("spring_apron") {
            SpringParams { stiffness: 4.0, drag: 0.18, gravity: 0.0, max_angle: 0.7 }
        } else {
            SpringParams { stiffness: 3.5, drag: 0.12, gravity: 1.5, max_angle: 0.9 }
        }
    }
}

struct Spring {
    joint: usize,
    child: usize,
    params: SpringParams,
    skirt: bool,
    /// the tail (world): now, and the step before
    cur: Vec3,
    prev: Vec3,
    /// skirt: the tail's distance out from the hips' axis at rest (m)
    reach: f32,
}

/// A character's spring bones and their state (one per person: the state is where each tail is).
pub struct Springs {
    springs: Vec<Spring>,
    hips: Option<usize>,
    started: bool,
}

impl Springs {
    pub fn new(skel: &Skeleton) -> Springs {
        let mut springs = Vec::new();
        for (i, j) in skel.joints.iter().enumerate() {
            if !j.name.starts_with("spring_") || j.name.ends_with("_end") {
                continue;
            }
            let Some(child) = skel.joints.iter().position(|c| c.parent == Some(i)) else { continue };
            springs.push(Spring {
                joint: i,
                child,
                params: SpringParams::for_name(&j.name),
                skirt: j.name.starts_with("spring_skirt"),
                cur: Vec3::ZERO,
                prev: Vec3::ZERO,
                reach: 0.0,
            });
        }
        Springs { springs, hips: skel.find("pelvis"), started: false }
    }

    pub fn is_empty(&self) -> bool {
        self.springs.is_empty()
    }

    /// Forget the tails' motion (after a teleport): they start again where the animation is.
    pub fn reset(&mut self) {
        self.started = false;
    }

    /// Swing the springs of `pose` (sampled from the clips) for a frame of `dt` seconds, the
    /// character placed at `at` (model -> world).
    pub fn apply(&mut self, skel: &Skeleton, pose: &mut Pose, at: Mat4, dt: f32) {
        if self.springs.is_empty() {
            return;
        }
        let dt = dt.clamp(0.0, 1.0 / 20.0);
        // world transforms, rebuilt as the springs turn their joints
        let mut g: Vec<Mat4> = Vec::with_capacity(skel.joints.len());
        let mut next = 0;
        let hips_axis = |g: &[Mat4], hips: Option<usize>| hips.filter(|&h| h < g.len()).map(|h| (g[h].w_axis.truncate(), g[h].y_axis.truncate().normalize_or_zero()));
        for (i, j) in skel.joints.iter().enumerate() {
            let parent = match j.parent {
                Some(p) => g[p],
                None => at * skel.root,
            };
            let mut m = parent * pose.local[i].matrix();
            if next < self.springs.len() && self.springs[next].joint == i {
                let s = &mut self.springs[next];
                next += 1;
                let head = m.w_axis.truncate();
                let rest_tail = pose.local[s.child].t;
                let len = rest_tail.length();
                if len > 1e-5 {
                    let target = m.transform_point3(rest_tail);
                    if !self.started {
                        s.cur = target;
                        s.prev = target;
                        if s.skirt {
                            if let Some((c, up)) = hips_axis(&g, self.hips) {
                                let d = target - c;
                                s.reach = (d - up * d.dot(up)).length();
                            }
                        }
                    }
                    let p = s.params;
                    let vel = (s.cur - s.prev) * (1.0 - p.drag).powf(dt * 60.0);
                    let pull = 1.0 - (-p.stiffness * dt).exp();
                    let mut n = s.cur + vel + (target - s.cur) * pull + Vec3::NEG_Y * p.gravity * dt * dt;
                    // a skirt never swings in through the legs
                    if s.skirt && s.reach > 0.0 {
                        if let Some((c, up)) = hips_axis(&g, self.hips) {
                            let d = n - c;
                            let along = up * d.dot(up);
                            let out = d - along;
                            let min = s.reach * 0.9;
                            if out.length() < min && out.length() > 1e-5 {
                                n = c + along + out.normalize() * min;
                            }
                        }
                    }
                    // the bone keeps its length, and turns no further than it may
                    let anim_dir = (target - head) / len;
                    let mut dir = (n - head).normalize_or(anim_dir);
                    let ang = anim_dir.angle_between(dir);
                    if ang > p.max_angle {
                        let axis = anim_dir.cross(dir).normalize_or(Vec3::X);
                        dir = Quat::from_axis_angle(axis, p.max_angle) * anim_dir;
                    }
                    n = head + dir * len;
                    s.prev = s.cur;
                    s.cur = n;
                    // turn the joint so its bone points at the tail
                    let turn = Quat::from_rotation_arc(anim_dir, dir);
                    let (_, world_r, _) = m.to_scale_rotation_translation();
                    let (_, parent_r, _) = parent.to_scale_rotation_translation();
                    pose.local[i].r = (parent_r.inverse() * turn * world_r).normalize();
                    m = parent * pose.local[i].matrix();
                }
            }
            g.push(m);
        }
        self.started = true;
    }
}
