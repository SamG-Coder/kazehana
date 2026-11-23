//! Checks on the built town's ground, from the plan as exported (assets/town: run
//! tools/blender/town_plan.py first):
//!
//! - nowhere does the terrain rise through an asphalt road (the road ribbon must lie on top);
//! - nothing grows on driveways or under houses (their land use must not be one grass grows on).
//!
//! `cargo test -p town --release --test ground_checks -- --nocapture` prints the worst places.

use std::path::Path;
use town::plants::grows_on;
use town::{road_profile, Town};

fn town() -> Town {
    Town::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/town")).expect("assets/town (run tools/blender/town_plan.py)")
}

/// The terrain mesh is the 1 m height grid (bilinear here); a road ribbon follows its graded
/// centre-line heights with the camber profile. Where the ground stands above the asphalt it
/// shows through it.
#[test]
fn the_ground_never_rises_through_a_road() {
    let t = town();
    const TOLERANCE: f32 = 0.005; // m
    let mut bad: Vec<(f32, String, [f32; 2])> = Vec::new();
    let mut samples = 0usize;
    for road in &t.plan.roads {
        if matches!(road.kind.as_str(), "farm" | "levee") {
            continue; // (field tracks: gravel on the ground as it is)
        }
        let Some(h) = &road.h else { continue };
        let hw = road.width / 2.0;
        for i in 1..road.pts.len() {
            let (a, b) = (road.pts[i - 1], road.pts[i]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = dx.hypot(dy);
            if len < 1e-3 {
                continue;
            }
            let (ux, uy) = (dx / len, dy / len);
            let n = (len / 0.5).ceil() as usize;
            for k in 0..=n {
                let s = k as f32 / n as f32;
                let (cx, cy) = (a[0] + dx * s, a[1] + dy * s);
                let ch = h[i - 1] + (h[i] - h[i - 1]) * s;
                let m = (hw / 0.25) as i32;
                for j in -m..=m {
                    let off = j as f32 * 0.25;
                    let (x, y) = (cx - uy * off, cy + ux * off);
                    // (where another road is on top, its ribbon covers this one: not this test's)
                    let road_top = ch + road_profile(&road.kind, off, hw);
                    let ground = t.ground([x, y]);
                    samples += 1;
                    // (a retaining wall holds the ground back where another road passes close
                    // above: town::retaining_wall_at, built along the edge)
                    let walled = off.abs() >= hw - 0.6 && town::retaining_wall_at(&t.plan, road, akari::glam::Vec2::new(cx - uy * off.signum() * (hw + 0.05), cy + ux * off.signum() * (hw + 0.05)), ch).is_some();
                    if ground > road_top + TOLERANCE && !covered_by_other(&t, road, [x, y], ground) && !walled {
                        bad.push((ground - road_top, format!("{} (road {:.2}, ground {:.2}, segment {i}, {:.0}% along, {off:+.2} across)", road.name, road_top, ground, s * 100.0), [x, y]));
                    }
                }
            }
        }
    }
    bad.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (d, name, p) in bad.iter().take(15) {
        println!("ground {:.3} m above {name} at plan ({:.1}, {:.1})", d, p[0], p[1]);
    }
    assert!(bad.is_empty(), "{} of {samples} road samples have the ground above the asphalt (worst {:.3} m)", bad.len(), bad.first().map_or(0.0, |b| b.0));
}

/// Is the point on another asphalt road whose surface is above the ground there (that road's
/// ribbon hides this one's)?
fn covered_by_other(t: &Town, me: &town::plan::Road, p: [f32; 2], ground: f32) -> bool {
    t.plan.roads.iter().filter(|r| !std::ptr::eq(*r, me) && !matches!(r.kind.as_str(), "farm" | "levee")).any(|r| {
        let Some(h) = &r.h else { return false };
        r.pts.windows(2).enumerate().any(|(i, w)| {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l2 = (dx * dx + dy * dy).max(1e-6);
            let s = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0);
            let (qx, qy) = (a[0] + dx * s, a[1] + dy * s);
            let d = (p[0] - qx).hypot(p[1] - qy);
            d <= r.width / 2.0 && h[i] + (h[i + 1] - h[i]) * s + road_profile(&r.kind, d, r.width / 2.0) >= ground
        })
    })
}

/// Sample points inside a convex quad (a 0.25 m grid over it).
fn inside(q: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let (x0, x1) = q.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
    let (y0, y1) = q.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
    let within = |p: [f32; 2]| {
        let n = q.len();
        let side = |k: usize| {
            let (a, b) = (q[k], q[(k + 1) % n]);
            (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
        };
        (0..n).all(|k| side(k) >= 0.0) || (0..n).all(|k| side(k) <= 0.0)
    };
    let mut out = Vec::new();
    let mut y = y0 + 0.4;
    while y < y1 - 0.4 {
        let mut x = x0 + 0.4;
        while x < x1 - 0.4 {
            if within([x, y]) {
                out.push([x, y]);
            }
            x += 0.25;
        }
        y += 0.25;
    }
    out
}

/// Driveway slabs and buildings stand on land where nothing grows (grass and weeds would come up
/// through them).
#[test]
fn nothing_grows_on_driveways_or_under_houses() {
    let t = town();
    let lu = town::load_landuse(&t.dir.join(&t.plan.terrain.landuse)).expect("land use");
    let ter = &t.plan.terrain;
    let (x0, y1) = (ter.origin[0], ter.origin[1] + (ter.ny - 1) as f32);
    let code = |p: [f32; 2]| {
        // (row 0 is the north edge: the row covering plan y is y1 + 1 - y, floored)
        let (i, j) = ((p[0] - x0).floor() as usize, (y1 + 1.0 - p[1]).floor() as usize);
        lu[j * ter.nx + i]
    };
    let mut bad = Vec::new();
    for h in &t.plan.houses {
        for (what, poly) in [("driveway", h.parking.as_ref()), ("house", Some(&h.house))] {
            let Some(poly) = poly else { continue };
            for p in inside(poly) {
                if grows_on(code(p)) {
                    bad.push((h.id, what, p, code(p)));
                }
            }
        }
    }
    for (id, what, p, c) in bad.iter().take(15) {
        println!("plot {id}: grass grows on its {what} at plan ({:.1}, {:.1}) (land use {c})", p[0], p[1]);
    }
    assert!(bad.is_empty(), "{} points of driveways and houses are land grass grows on", bad.len());
}

/// The paddies' levees (the grass ridges round them) run over the fields only: never across a
/// road, a plot, a site or the river.
#[test]
fn levees_stay_on_the_fields() {
    let t = town();
    let lu = town::LandUse::load(&t).expect("land use");
    let mut bad = Vec::new();
    for q in &t.plan.paddies {
        for (a, b) in town::levee_spans(&lu, q) {
            let n = ((b - a).length() / 0.5).ceil() as usize;
            for k in 1..n {
                let p = a.lerp(b, k as f32 / n as f32);
                let c = lu.at(p);
                if !matches!(c, 0 | 1 | 2) {
                    bad.push((p, c));
                }
            }
        }
    }
    for (p, c) in bad.iter().take(10) {
        println!("a levee over land use {c} at plan ({:.1}, {:.1})", p.x, p.y);
    }
    assert!(bad.is_empty(), "{} levee points off the fields", bad.len());
}
