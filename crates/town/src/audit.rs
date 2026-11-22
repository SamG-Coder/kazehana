//! The walkways audit: is anything in the way where people walk? (`townview --audit`)
//!
//! * Along every road and lane (and the route to school), a person-sized capsule every metre, at
//!   the middle and either side: whatever it touches is in the way. Higher up (2 - 4.5 m) over
//!   the carriageway: anything there hangs over the road (a lorry has to pass under it).
//! * Every door: a person standing just in front of the opening and just behind it, the leaf
//!   swung open: whatever they touch blocks the doorway.
//!
//! Each finding names the model it belongs to (Playable::label) and where it is (plan x, y).

use crate::collide::Playable;
use crate::Town;
use akari::glam::{Vec2, Vec3};
use play::collision::Contact;
use std::collections::BTreeMap;

pub struct Finding {
    pub what: &'static str,
    pub place: String,
    pub by: String,
    pub at: [f32; 2],
    pub depth: f32,
}

pub static PROBES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
const R: f32 = 0.25; // a person's half width
const FOOT: f32 = 0.4; // the capsule's lowest point above the ground (steps and kerbs are walked over)
const HEAD: f32 = 1.65;

fn blockers(p: &Playable, a: Vec3, b: Vec3, r: f32, skip: &[play::collision::BodyId]) -> Vec<(String, f32)> {
    let mut c: Vec<Contact> = Vec::new();
    p.world.capsule_contacts(a, b, r, &mut c);
    let mut out: BTreeMap<String, f32> = BTreeMap::new();
    for k in c.iter().filter(|k| k.depth > 0.03 && !k.body.is_some_and(|b| skip.contains(&b))) {
        let name = k.body.map_or("?".to_string(), |b| p.label(b));
        let e = out.entry(name).or_insert(0.0);
        *e = e.max(k.depth);
    }
    out.into_iter().collect()
}

/// Walk the roads, the route to school and the doors; return what is in the way.
pub fn run(town: &Town, p: &mut Playable) -> Vec<Finding> {
    let mut out = Vec::new();
    let world_at = |p: &Playable, q: [f32; 2]| -> Option<Vec3> { p.world.ground(q[0], -q[1]).map(|y| Vec3::new(q[0], y, -q[1])) };
    // ---- roads: a person every metre, centre and both sides; above the carriageway, clearance
    for road in &town.plan.roads {
        let hw = road.width * 0.5;
        let field = matches!(road.kind.as_str(), "farm" | "levee");
        for (a, b) in road.pts.iter().zip(road.pts.iter().skip(1)) {
            let (a, b) = (Vec2::from(*a), Vec2::from(*b));
            let d = b - a;
            let len = d.length();
            if len < 1e-3 {
                continue;
            }
            let (u, n) = (d / len, Vec2::new(-d.y, d.x) / len);
            for k in 0..=(len as usize) {
                let c = a + u * k as f32;
                for off in [-0.6f32, 0.0, 0.6] {
                    let q = c + n * (off * hw);
                    let Some(g) = world_at(p, q.to_array()) else { continue };
                    PROBES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    for (by, depth) in blockers(p, g + Vec3::Y * (FOOT + R), g + Vec3::Y * (HEAD - R), R, &[]) {
                        out.push(Finding { what: "in the way on", place: road.name.clone(), by, at: q.to_array(), depth });
                    }
                }
                if !field && k % 2 == 0 {
                    let Some(g) = world_at(p, c.to_array()) else { continue };
                    for (by, depth) in blockers(p, g + Vec3::Y * 2.3, g + Vec3::Y * 4.2, 0.3, &[]) {
                        out.push(Finding { what: "hangs over", place: road.name.clone(), by, at: c.to_array(), depth });
                    }
                }
            }
        }
    }
    // ---- the route to school (footpaths, the school's gate)
    for w in town.plan.route_to_school.windows(2) {
        let (a, b) = (Vec2::from(w[0]), Vec2::from(w[1]));
        let len = (b - a).length();
        for k in 0..=(len as usize) {
            let q = a + (b - a) * (k as f32 / len.max(1e-3));
            let Some(g) = world_at(p, q.to_array()) else { continue };
            for (by, depth) in blockers(p, g + Vec3::Y * (FOOT + R), g + Vec3::Y * (HEAD - R), R, &[]) {
                out.push(Finding { what: "in the way on", place: "the route to school".into(), by, at: q.to_array(), depth });
            }
        }
    }
    // ---- doors: swung open, a person just in front of the opening and just behind it
    let n = p.doors.doors.len();
    let set = |p: &mut Playable, open: f32| {
        for i in 0..p.doors.doors.len() {
            p.doors.doors[i].open = open;
            p.doors.doors[i].target = open;
            let (body, at) = (p.doors.doors[i].body, p.doors.doors[i].transform());
            p.world.set_transform(body, at);
        }
    };
    set(p, 1.0);
    for i in 0..n {
        let d = &p.doors.doors[i];
        let body = d.body;
        // the opening: the leaf's closed box; its thin axis is the way through
        let (lo, hi) = (d.min, d.max);
        let size = hi - lo;
        // (a door you walk through: a cupboard's or a locker's isn't a doorway)
        if size.y < 1.5 {
            continue;
        }
        let thin = if size.x < size.z { Vec3::X } else { Vec3::Z };
        let mid = d.closed.transform_point3((lo + hi) * 0.5);
        let through = d.closed.transform_vector3(thin).normalize_or_zero();
        let floor = d.closed.transform_point3(Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5)).y;
        let label = p.label(body);
        let mut hits: BTreeMap<String, f32> = BTreeMap::new();
        for side in [-1.0f32, 1.0] {
            let c = Vec3::new(mid.x, floor, mid.z) + through * (side * 0.55);
            for (by, depth) in blockers(p, c + Vec3::Y * (FOOT + R), c + Vec3::Y * (HEAD.min(size.y.max(1.2)) - R), 0.2, &[body]) {
                let e = hits.entry(by).or_insert(0.0);
                *e = e.max(depth);
            }
        }
        for (by, depth) in hits {
            out.push(Finding { what: "blocks the doorway of", place: label.clone(), by, at: [mid.x, -mid.z], depth });
        }
        // the opening itself, the door swung open: anything across it (a rail, a bar, a step, a
        // panel lapping into it) at the foot, the middle or the head (a 1.6 m tall person's)
        // (the door's own frame stays clear: thin probes, 0.15 m in from each jamb)
        let across = d.closed.transform_vector3(if thin == Vec3::X { Vec3::Z } else { Vec3::X }).normalize_or_zero();
        let half_w = (if thin == Vec3::X { size.z } else { size.x } * 0.5 - 0.15).max(0.05);
        let mut through_hits: BTreeMap<String, f32> = BTreeMap::new();
        for h in [0.08f32, 0.9, 1.6f32.min(size.y - 0.12)] {
            for w in [-half_w, 0.0, half_w] {
                let c = Vec3::new(mid.x, floor + h, mid.z) + across * w;
                for (by, depth) in blockers(p, c - through * 0.35, c + through * 0.35, 0.04, &[body]) {
                    let e = through_hits.entry(format!("{by} (at {h:.2} m, {:+.2} across)", w)).or_insert(0.0);
                    *e = e.max(depth);
                }
            }
        }
        for (by, depth) in through_hits {
            out.push(Finding { what: "is across the opening of", place: label.clone(), by, at: [mid.x, -mid.z], depth });
        }
    }
    set(p, 0.0);
    out.extend(climbs(p));
    out
}

/// Steps that have to be walked up: the real controller walks at them from the floor in front
/// and has to end up on top (the gym's stage, from both of its side steps).
fn climbs(p: &Playable) -> Vec<Finding> {
    let mut out = Vec::new();
    // (model, the foot of the climb and the top, in the model's own Blender frame: x, y, z up)
    const CLIMBS: [(&str, &str, [f32; 3], [f32; 3]); 2] = [
        ("school_gym", "the stage's north steps", [10.8, -3.9, 0.5], [14.2, -3.9, 1.5]),
        ("school_gym", "the stage's south steps", [10.8, 3.9, 0.5], [14.2, 3.9, 1.5]),
    ];
    for (model, what, from, to) in CLIMBS {
        let Some(b) = (0..p.world.body_count()).map(|i| play::BodyId(i as u32)).find(|&b| p.label(b) == model) else { continue };
        let at = p.world.transform(b);
        let w = |v: [f32; 3]| at.transform_point3(Vec3::new(v[0], v[2], -v[1]));
        let (a, top) = (w(from), w(to));
        let dir = (top - a).with_y(0.0).normalize_or_zero();
        let mut fp = play::FirstPerson::new(a + Vec3::Y * 0.05, dir.x.atan2(-dir.z), play::Settings::default());
        let go = play::Actions { movement: Vec2::new(0.0, 1.0), ..Default::default() };
        for _ in 0..(6.0 * 120.0) as u32 {
            fp.update(&go, &p.world, 1.0 / 120.0);
            if (fp.feet - top).with_y(0.0).dot(dir) > 0.0 {
                break;
            }
        }
        PROBES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let short = top.y - fp.feet.y;
        if short > 0.1 {
            out.push(Finding { what: "can't be walked up:", place: format!("{model} {what}"), by: format!("stopped at {:?}", fp.feet), at: [fp.feet.x, -fp.feet.z], depth: short });
        } else {
            println!("climb ok: {model} {what} (from {a:.2?} to {top:.2?}, feet at {:.2})", fp.feet.y);
        }
    }
    // water isn't a floor: a person stepping off over the pool goes in (the school pool's water at
    // z 1.03, its floor about a metre below)
    if let Some(b) = (0..p.world.body_count()).map(|i| play::BodyId(i as u32)).find(|&b| p.label(b) == "school_pool") {
        let at = p.world.transform(b);
        let surface = at.transform_point3(Vec3::new(4.0, 1.03, -2.0));
        let mut fp = play::FirstPerson::new(surface + Vec3::Y * 0.3, 0.0, play::Settings::default());
        for _ in 0..(3.0 * 120.0) as u32 {
            fp.update(&play::Actions::default(), &p.world, 1.0 / 120.0);
        }
        PROBES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if fp.feet.y > surface.y - 0.5 {
            out.push(Finding { what: "is walked on, not waded into:", place: "school_pool water".into(), by: format!("feet at {:.2}, surface {:.2}", fp.feet.y, surface.y), at: [surface.x, -surface.z], depth: fp.feet.y - surface.y + 0.5 });
        } else {
            println!("water ok: school_pool (feet at {:.2}, {:.2} under the surface)", fp.feet.y, surface.y - fp.feet.y);
        }
    }
    out
}

/// The findings, grouped: what is in the way where, how often, the worst depth and a place to
/// look (plan x, y).
pub fn report(f: &[Finding]) -> String {
    let mut g: BTreeMap<(String, String, String), (usize, f32, [f32; 2])> = BTreeMap::new();
    for x in f {
        let e = g.entry((x.by.clone(), x.what.to_string(), x.place.clone())).or_insert((0, 0.0, x.at));
        e.0 += 1;
        if x.depth > e.1 {
            e.1 = x.depth;
            e.2 = x.at;
        }
    }
    let mut rows: Vec<_> = g.into_iter().collect();
    rows.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    let mut s = format!("walkways audit: {} road probes; {} findings in {} groups\n", PROBES.load(std::sync::atomic::Ordering::Relaxed), f.len(), rows.len());
    for ((by, what, place), (count, depth, at)) in rows {
        s += &format!("  {by} {what} {place}: {count} probes, up to {:.2} m deep, e.g. at plan ({:.1}, {:.1})\n", depth, at[0], at[1]);
    }
    s
}
