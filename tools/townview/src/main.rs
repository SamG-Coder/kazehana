//! Kazehana-chō viewer: the town built from the plan (crates/town), lit by the real sun.
//!
//! Interactive: walk with W A S D or a gamepad's left stick (play crate: Shift / L3 run, Ctrl /
//! Circle crouch, Space / Cross jump, E / Square open doors), look with the mouse (click to
//! capture, Esc release) or the right stick; F fly; [ / ] hour -/+, L DLSS on/off, F12 screenshot.
//! `--gate` renders the shot set to shots/town/ in a hidden window and exits; `--shot NAME` only
//! that shot. Options: --time HH:MM --date YYYY-MM-DD --no-dlss --validation --bounces N

use akari::glam::{Vec2, Vec3};
use akari::winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};
use akari::*;
use anyhow::{Context, Result};
use sim::clock::{self, at, GameTime};
use sim::Date;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use town::Town;

const EYE: f32 = 1.5;

/// A shot taken after walking: the player starts at the shot's place and walks forward for
/// `secs` (using what is in front first, if `interact`), with collision and doors.
#[derive(Clone, Copy)]
struct Walk {
    secs: f32,
    interact: bool,
    /// stand where it is (only look and use)
    still: bool,
    /// sweep the view up and down and side to side while walking (as a player looking round)
    look: bool,
}

struct Shot {
    walk: Option<Walk>,
    name: String,
    time: u32,
    /// plan position; height above the ground
    at: [f32; 2],
    up: f32,
    yaw: f32,
    pitch: f32,
}

/// The noren shots (Tsukumodō's back doorway, its cloth simulated): (name, how long the player
/// walks from 1.8 m in front of it into the back (None: no one), first person or watched from the
/// shop). Placed once the town is loaded (App::noren_shot).
const NOREN_SHOTS: &[(&str, Option<f32>, bool)] = &[
    ("noren_rest", None, false),
    ("noren_walk_fp_touch", Some(1.1), true),
    ("noren_walk_fp_through", Some(1.2), true),
    ("noren_watch_parting", Some(1.15), false),
    ("noren_watch_falling_back", Some(2.4), false),
];

fn yaw_to(from: [f32; 2], to: [f32; 2]) -> f32 {
    // plan (x, y) -> world (x, -z): yaw 0 looks north (+y), positive towards east (+x)
    (to[0] - from[0]).atan2(to[1] - from[1])
}

fn shots(town: &Town) -> Vec<Shot> {
    let p = &town.plan;
    let route = &p.route_to_school;
    let home = &p.houses[p.home];
    let near = |pts: &[[f32; 2]], q: Vec2| pts.iter().enumerate().min_by(|a, b| (Vec2::from(*a.1) - q).length().total_cmp(&(Vec2::from(*b.1) - q).length())).map(|x| x.0).unwrap_or(0);
    let rail_mid = Vec2::from(p.rail[p.rail.len() / 2]);
    let ri = near(route, Vec2::new(route[0][0], rail_mid.y)); // (the route runs north-south)
    let river_pt = p.river.pts.iter().map(|q| Vec2::from(*q)).min_by(|a, b| (a.x - route[route.len() / 2][0]).abs().total_cmp(&(b.x - route[route.len() / 2][0]).abs())).unwrap();
    let bi = near(route, river_pt);
    let mid = (ri + bi) / 2;
    let h = |i: usize| route[i.min(route.len() - 1)];
    let gate = home.gate;
    // the level crossing on the way to school: where the route meets the track
    let xing = {
        let rl: Vec<Vec2> = p.rail.iter().map(|q| Vec2::from(*q)).collect();
        let mut best = (f32::MAX, Vec2::ZERO);
        for w in route.windows(2) {
            let (a, b) = (Vec2::from(w[0]), Vec2::from(w[1]));
            for k in 0..=20 {
                let q = a.lerp(b, k as f32 / 20.0);
                let d = rl.windows(2).map(|r| {
                    let t = ((q - r[0]).dot(r[1] - r[0]) / (r[1] - r[0]).length_squared().max(1e-6)).clamp(0.0, 1.0);
                    (r[0] + (r[1] - r[0]) * t - q).length()
                }).fold(f32::MAX, f32::min);
                if d < best.0 {
                    best = (d, q);
                }
            }
        }
        best.1
    };
    let toward = (Vec2::from(h(ri.saturating_sub(4))) - xing).normalize_or(Vec2::Y);
    let side = Vec2::new(-toward.y, toward.x);
    let xc = xing + toward * 7.0 + side * 1.2;
    let xs = xing + side * 11.0 + toward * 7.0;
    let mut v = vec![
        Shot { walk: None, name: "crossing_close".into(), time: at::hm(8, 5), at: xc.to_array(), up: EYE, yaw: yaw_to(xc.to_array(), (xing - toward * 6.0).to_array()), pitch: -8.0f32.to_radians() },
        Shot { walk: None, name: "crossing_side".into(), time: at::hm(8, 5), at: xs.to_array(), up: EYE + 0.8, yaw: yaw_to(xs.to_array(), xing.to_array()), pitch: -9.0f32.to_radians() },
        Shot { walk: None, name: "home_street_morning".into(), time: at::hm(7, 50), at: gate, up: EYE, yaw: yaw_to(gate, h(6)), pitch: -2.0f32.to_radians() },
        Shot { walk: None, name: "level_crossing".into(), time: at::hm(8, 5), at: h(ri.saturating_sub(4)), up: EYE, yaw: yaw_to(h(ri.saturating_sub(4)), h(ri + 10)), pitch: -1.0f32.to_radians() },
        Shot { walk: None, name: "paddies".into(), time: at::hm(8, 12), at: h(mid), up: EYE, yaw: yaw_to(h(mid), h(mid + 20)) - 0.6, pitch: 0.0 },
        Shot { walk: None, name: "bridge".into(), time: at::hm(8, 18), at: h(bi), up: EYE + 0.2, yaw: yaw_to(h(bi), h(bi + 2)) + 1.45, pitch: -4.0f32.to_radians() },
        Shot { walk: None, name: "school_gate".into(), time: at::hm(8, 25), at: p.school_gate, up: EYE, yaw: yaw_to(p.school_gate, h(route.len() / 2)), pitch: 2.0f32.to_radians() },
        Shot { walk: None, name: "aerial_centre".into(), time: at::hm(10, 30), at: [gate[0] + 120.0, gate[1] + 160.0], up: 110.0, yaw: yaw_to([gate[0] + 120.0, gate[1] + 160.0], h(mid)), pitch: -24.0f32.to_radians() },
        Shot { walk: None, name: "home_street_dusk".into(), time: at::hm(18, 15), at: gate, up: EYE, yaw: yaw_to(gate, h(6)), pitch: -2.0f32.to_radians() },
    ];
    // close-ups: the prefectural road, the old road, grass on the levee
    let road_pt = |name: &str, x: f32| -> Option<([f32; 2], [f32; 2])> {
        let r = p.roads.iter().find(|r| r.name.starts_with(name))?;
        let i = r.pts.iter().enumerate().min_by(|a, b| (a.1[0] - x).abs().total_cmp(&(b.1[0] - x).abs()))?.0;
        Some((r.pts[i], r.pts[(i + 1).min(r.pts.len() - 1)]))
    };
    if let Some((a, b)) = road_pt("prefectural", route[0][0] + 90.0) {
        let side = [a[0], a[1] - 2.2];
        v.push(Shot { walk: None, name: "road_pref".into(), time: at::hm(10, 40), at: side, up: EYE, yaw: yaw_to(b, a), pitch: -7.0f32.to_radians() });
        v.push(Shot { walk: None, name: "road_pref_down".into(), time: at::hm(10, 40), at: side, up: EYE, yaw: yaw_to(b, a), pitch: -38.0f32.to_radians() });
    }
    if let Some((a, b)) = road_pt("old road", -1000.0) {
        v.push(Shot { walk: None, name: "road_old".into(), time: at::hm(15, 30), at: a, up: EYE, yaw: yaw_to(a, b), pitch: -9.0f32.to_radians() });
    }
    {
        let rp = river_pt;
        let levee = [rp.x, rp.y + 14.5];
        v.push(Shot { walk: None, name: "grass_levee".into(), time: at::hm(14, 0), at: levee, up: EYE, yaw: 1.4, pitch: -14.0f32.to_radians() });
    }
    // the modern gable houses near home, seen from their road
    {
        let hm = home.gate;
        let mut near: Vec<_> = p.houses.iter().filter(|x| x.ty == "modern_gable").collect();
        let mut smalls: Vec<_> = p.houses.iter().filter(|x| x.ty == "small_house").collect();
        smalls.sort_by(|a, b| (a.gate[0] - hm[0]).hypot(a.gate[1] - hm[1]).total_cmp(&(b.gate[0] - hm[0]).hypot(b.gate[1] - hm[1])));
        for (k, pl) in smalls.iter().take(2).enumerate() {
            let (gx, gy) = (pl.gate[0], pl.gate[1]);
            let c = pl.house.iter().fold([0.0f32; 2], |s, q| [s[0] + q[0] / 4.0, s[1] + q[1] / 4.0]);
            let (dx, dy) = (gx - c[0], gy - c[1]);
            let l = dx.hypot(dy).max(1e-3);
            let at = [gx + dx / l * 8.0 - dy / l * 4.0, gy + dy / l * 8.0 + dx / l * 4.0];
            v.push(Shot { walk: None, name: format!("house_small_{k}"), time: at::hm(10, 0), at, up: EYE, yaw: yaw_to(at, c), pitch: 2.0f32.to_radians() });
        }
        let mut hips: Vec<_> = p.houses.iter().filter(|x| x.ty == "modern_hip").collect();
        hips.sort_by(|a, b| (a.gate[0] - hm[0]).hypot(a.gate[1] - hm[1]).total_cmp(&(b.gate[0] - hm[0]).hypot(b.gate[1] - hm[1])));
        for (k, pl) in hips.iter().take(2).enumerate() {
            let (gx, gy) = (pl.gate[0], pl.gate[1]);
            let c = pl.house.iter().fold([0.0f32; 2], |s, q| [s[0] + q[0] / 4.0, s[1] + q[1] / 4.0]);
            let (dx, dy) = (gx - c[0], gy - c[1]);
            let l = dx.hypot(dy).max(1e-3);
            let at = [gx + dx / l * 9.0 - dy / l * 5.0, gy + dy / l * 9.0 + dx / l * 5.0];
            v.push(Shot { walk: None, name: format!("house_hip_{k}"), time: at::hm(10, 0), at, up: EYE, yaw: yaw_to(at, c), pitch: 7.0f32.to_radians() });
        }
        near.sort_by(|a, b| (a.gate[0] - hm[0]).hypot(a.gate[1] - hm[1]).total_cmp(&(b.gate[0] - hm[0]).hypot(b.gate[1] - hm[1])));
        // and the nearest one whose road runs along its side (door on the gable end)
        if let Some(side) = near.iter().find(|x| x.garden == "left" || x.garden == "right") {
            let (gx, gy) = (side.gate[0], side.gate[1]);
            let c = side.house.iter().fold([0.0f32; 2], |s, q| [s[0] + q[0] / 4.0, s[1] + q[1] / 4.0]);
            let (dx, dy) = (gx - c[0], gy - c[1]);
            let l = dx.hypot(dy).max(1e-3);
            let at = [gx + dx / l * 9.0 - dy / l * 4.0, gy + dy / l * 9.0 + dx / l * 4.0];
            v.push(Shot { walk: None, name: "house_gable_side".into(), time: at::hm(10, 0), at, up: EYE, yaw: yaw_to(at, c), pitch: 6.0f32.to_radians() });
        }
        for (k, pl) in near.iter().take(2).enumerate() {
            let (gx, gy) = (pl.gate[0], pl.gate[1]);
            let c = pl.house.iter().fold([0.0f32; 2], |s, q| [s[0] + q[0] / 4.0, s[1] + q[1] / 4.0]);
            let (dx, dy) = (gx - c[0], gy - c[1]);
            let l = dx.hypot(dy).max(1e-3);
            let at = [gx + dx / l * 9.0 - dy / l * 6.0, gy + dy / l * 9.0 + dx / l * 6.0];
            v.push(Shot { walk: None, name: format!("house_gable_{k}"), time: at::hm(10, 0), at, up: EYE, yaw: yaw_to(at, c), pitch: 6.0f32.to_radians() });
        }
    }
    // the plots' outdoors near home: a block-wall plot and a hedge plot, seen from the road
    for (name, front) in [("plot_block", "block"), ("plot_hedge", "hedge"), ("plot_stone", "stone")] {
        let hm = home.gate;
        if let Some(pl) = p.houses.iter().filter(|x| x.front == front && x.ty != "shophouse").min_by(|a, b| (a.gate[0] - hm[0]).hypot(a.gate[1] - hm[1]).total_cmp(&(b.gate[0] - hm[0]).hypot(b.gate[1] - hm[1]))) {
            let (gx, gy) = (pl.gate[0], pl.gate[1]);
            let (dx, dy) = (gx - pl.entrance[0], gy - pl.entrance[1]);
            let l = dx.hypot(dy).max(1e-3);
            let (ox, oy) = (dx / l, dy / l);
            // on the plot's own road, beyond its centre line, a little along
            let road_pt = p.roads.iter().find(|r| r.name == pl.road).and_then(|r| {
                r.pts.iter().min_by(|a, b| (a[0] - gx).hypot(a[1] - gy).total_cmp(&(b[0] - gx).hypot(b[1] - gy))).copied()
            });
            let at = match road_pt {
                Some(rp) => {
                    let (rx, ry) = (rp[0] - gx, rp[1] - gy);
                    let rl = rx.hypot(ry).max(1e-3);
                    [rp[0] + rx / rl * 2.0 - ry / rl * 4.0, rp[1] + ry / rl * 2.0 + rx / rl * 4.0]
                }
                None => [gx + ox * 11.0 - oy * 3.0, gy + oy * 11.0 + ox * 3.0],
            };
            v.push(Shot { walk: None, name: name.into(), time: at::hm(10, 0), at, up: EYE, yaw: yaw_to(at, [gx, gy]), pitch: -4.0f32.to_radians() });
        }
    }
    if let Some(hm) = p.houses.iter().find(|x| x.district == "Higashi" && x.ty == "farmhouse") {
        let g = hm.gate;
        let back = [g[0] + (g[0] - hm.entrance[0]) * 2.5, g[1] + (g[1] - hm.entrance[1]) * 2.5];
        v.push(Shot { walk: None, name: "hamlet_higashi".into(), time: at::hm(15, 0), at: back, up: EYE, yaw: yaw_to(back, hm.entrance), pitch: 3.0f32.to_radians() });
    }
    // ---- survey: every section of the town from its nearest road (gap review)
    let nearest_road = |q: [f32; 2]| -> [f32; 2] {
        p.roads
            .iter()
            .flat_map(|r| r.pts.iter())
            .min_by(|a, b| (a[0] - q[0]).hypot(a[1] - q[1]).total_cmp(&(b[0] - q[0]).hypot(b[1] - q[1])))
            .copied()
            .unwrap_or(q)
    };
    let from_road = |name: String, c: [f32; 2], back: f32, up: f32, pitch: f32| -> Shot {
        let r = nearest_road(c);
        let (dx, dy) = (r[0] - c[0], r[1] - c[1]);
        let l = dx.hypot(dy).max(1e-3);
        let at = [r[0] + dx / l * back, r[1] + dy / l * back];
        Shot { walk: None, name, time: at::hm(11, 0), at, up, yaw: yaw_to(at, c), pitch: pitch.to_radians() }
    };
    for s in &p.sites {
        let n = s.poly.len() as f32;
        let c = s.poly.iter().fold([0.0f32; 2], |a, q| [a[0] + q[0] / n, a[1] + q[1] / n]);
        let slug: String = s.name.to_lowercase().chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' }).collect();
        v.push(from_road(format!("survey_{slug}"), c, 4.0, EYE, -3.0));
    }
    let centre = |h: &town::plan::House| h.plot.iter().fold([0.0f32; 2], |a, q| [a[0] + q[0] / 4.0, a[1] + q[1] / 4.0]);
    v.push(from_road("survey_home_tsukumodo".into(), centre(home), 3.0, EYE, -2.0));
    {
        // Tsukumodō from the street: across the road, and up close at the shopfront
        let g = Vec2::from(home.gate);
        let c = Vec2::from(centre(home));
        let out = (g - c).normalize_or(Vec2::Y);
        let side = Vec2::new(-out.y, out.x);
        let far = g + out * 9.0 + side * 5.0;
        let near = g + out * 3.2 - side * 1.5;
        v.push(Shot { walk: None, name: "shop_street".into(), time: at::hm(10, 30), at: far.to_array(), up: EYE, yaw: yaw_to(far.to_array(), (c + out * 4.0).to_array()), pitch: 9.0f32.to_radians() });
        v.push(Shot { walk: None, name: "shop_front".into(), time: at::hm(10, 30), at: near.to_array(), up: EYE, yaw: yaw_to(near.to_array(), (g - out * 2.0 + side * 0.5).to_array()), pitch: 0.0 });
        // inside: from just inside the door, and from the play tables towards the counter
        let door = g - out * 1.9 - side * 0.3;
        let counter = g - out * 6.9; // (the house front is at the gate; the counter 6.6 m in)
        v.push(Shot { walk: None, name: "shop_inside".into(), time: at::hm(10, 30), at: door.to_array(), up: 1.55, yaw: yaw_to(door.to_array(), (counter - side * 1.5).to_array()), pitch: -8.0f32.to_radians() });
        let corner = g - out * 2.3 + side * 3.3;
        v.push(Shot { walk: None, name: "shop_inside_tables".into(), time: at::hm(10, 30), at: corner.to_array(), up: 1.6, yaw: yaw_to(corner.to_array(), (counter - side * 1.5).to_array()), pitch: -14.0f32.to_radians() });
        // walking in: the automatic doors open as the player comes; inside, up to the counter
        let hc = Vec2::from(home.house.iter().fold([0.0f32; 2], |a, q| [a[0] + q[0] / 4.0, a[1] + q[1] / 4.0]));
        let start = hc + out * 9.0;
        let to_door = yaw_to(start.to_array(), hc.to_array());
        for (name, secs) in [("walk_shop_approach", 0.0f32), ("walk_shop_enter", 2.6), ("walk_shop_inside", 6.5)] {
            v.push(Shot { walk: Some(Walk { secs, interact: false, still: false, look: false }), name: name.into(), time: at::hm(10, 30), at: start.to_array(), up: 0.0, yaw: to_door, pitch: -4.0f32.to_radians() });
        }
        // the back door: a hinged one, opened by hand (which end it is depends on how the
        // house is turned: both are tried)
        let x_dir = {
            let q: Vec<Vec2> = home.house.iter().map(|p| Vec2::from(*p)).collect();
            (q[1] - q[0]).normalize_or(Vec2::X)
        };
        let back_door = hc - out * 5.0 - x_dir * 2.8;
        let from = back_door - out * 2.0 + x_dir * 0.4;
        for (name, interact) in [("walk_backdoor_shut", false), ("walk_backdoor_open", true)] {
            v.push(Shot { walk: Some(Walk { secs: 2.0, interact, still: true, look: false }), name: name.into(), time: at::hm(10, 30), at: from.to_array(), up: 0.0, yaw: yaw_to(from.to_array(), back_door.to_array()), pitch: -8.0f32.to_radians() });
        }
        // along the shelves, close, looking up and down and round (the lag test)
        {
            let along = x_dir;
            let shelf_side = hc - out * 1.0 + Vec2::new(-along.y, along.x) * 3.3;
            let from = shelf_side + out * 2.2;
            v.push(Shot { walk: Some(Walk { secs: 6.0, interact: false, still: false, look: true }), name: "walk_shelves".into(), time: at::hm(10, 30), at: from.to_array(), up: 0.0, yaw: yaw_to(from.to_array(), (shelf_side - out * 2.0).to_array()), pitch: -10.0f32.to_radians() });
        }
        {
            let (feet, yaw) = town::scene::home_start(town);
            let at = [feet.x, -feet.z];
            v.push(Shot { walk: None, name: "character_test".into(), time: at::hm(10, 30), at, up: 1.4, yaw, pitch: -12.0f32.to_radians() });
        }
        v.push(Shot { walk: None, name: "shop_dusk".into(), time: at::hm(18, 20), at: far.to_array(), up: EYE, yaw: yaw_to(far.to_array(), (c + out * 4.0).to_array()), pitch: 9.0f32.to_radians() });
    }
    for ty in ["shophouse", "farmhouse", "apartment"] {
        if let Some(h) = p.houses.iter().filter(|x| x.ty == ty && x.id != home.id).min_by(|a, b| (a.gate[0] - home.gate[0]).hypot(a.gate[1] - home.gate[1]).total_cmp(&(b.gate[0] - home.gate[0]).hypot(b.gate[1] - home.gate[1]))) {
            v.push(from_road(format!("survey_{ty}"), centre(h), 3.0, EYE, -2.0));
        }
    }
    if let Some(hk) = p.hatake.first() {
        let n = hk.len() as f32;
        let c = hk.iter().fold([0.0f32; 2], |a, q| [a[0] + q[0] / n, a[1] + q[1] / n]);
        v.push(from_road("survey_hatake".into(), c, 2.0, EYE, -6.0));
    }
    v.push(Shot { walk: None, name: "survey_aerial_town".into(), time: at::hm(11, 0), at: [home.gate[0] - 200.0, home.gate[1] - 420.0], up: 260.0, yaw: yaw_to([home.gate[0] - 200.0, home.gate[1] - 420.0], home.gate), pitch: -28.0f32.to_radians() });
    v
}

struct Args {
    gate: bool,
    only: Option<String>,
    date: Date,
    minute: Option<u32>,
    dlss: bool,
    validation: bool,
    bounces: u32,
    no_plants: bool,
    /// skinned characters (glTF) to stand in the shop in a row, playing KZ_CLIP (default idle;
    /// "rest" for none); --character more than once for a line-up
    characters: Vec<PathBuf>,
}

fn parse_args() -> Args {
    let mut a = Args { gate: false, only: None, date: Date::new(2026, 4, 8), minute: None, dlss: true, validation: false, bounces: 2, no_plants: false, characters: Vec::new() };
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < v.len() {
        let next = v.get(i + 1).cloned().unwrap_or_default();
        match v[i].as_str() {
            "--gate" => a.gate = true,
            "--shot" => {
                a.gate = true;
                a.only = Some(next.clone());
                i += 1;
            }
            "--no-dlss" => a.dlss = false,
            "--no-plants" => a.no_plants = true,
            "--character" => {
                a.characters.push(PathBuf::from(&next));
                i += 1;
            }
            "--validation" => a.validation = true,
            "--bounces" => {
                a.bounces = next.parse().unwrap_or(2);
                i += 1;
            }
            "--time" => {
                let p: Vec<u32> = next.split(':').filter_map(|x| x.parse().ok()).collect();
                a.minute = Some(at::hm(p.first().copied().unwrap_or(10), p.get(1).copied().unwrap_or(0)));
                i += 1;
            }
            "--date" => {
                let p: Vec<i32> = next.split('-').filter_map(|x| x.parse().ok()).collect();
                if p.len() == 3 {
                    a.date = Date::new(p[0], p[1] as u32, p[2] as u32);
                }
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    a
}

fn sky_frame(t: &GameTime) -> (Vec3, f32, Vec3, f32, f32) {
    let s = clock::sky(t);
    let sun = Vec3::from(s.sun_dir).normalize();
    let moon = Vec3::from(s.moon_dir).normalize();
    let sun_i = if s.sun_elevation.to_degrees() > -18.0 { 1.0 } else { 0.0 };
    let moon_up = ((s.moon_elevation.to_degrees() + 1.0) / 6.0).clamp(0.0, 1.0);
    let moon_i = 2.5e-6 * s.moon_light * moon_up;
    let stars = (1.0 - ((s.sun_elevation.to_degrees() + 4.0) / 8.0).clamp(0.0, 1.0)).powi(2);
    (sun, sun_i, moon, moon_i, stars)
}

struct App {
    args: Args,
    town: Town,
    shots: Vec<Shot>,
    window: Option<Arc<Window>>,
    r: Option<Renderer>,
    instances: Vec<Instance>,
    /// the town without plants
    /// the town (shared with the game: town::scene)
    world: Option<town::scene::TownWorld>,
    cam: Camera,
    time: GameTime,
    keys: HashSet<KeyCode>,
    captured: bool,
    fly: bool,
    last: Instant,
    start: Instant,
    shot_i: usize,
    shot_frames: u32,
    shot_request: Option<PathBuf>,
    /// the player: input, body, the town's collision and doors
    input: play::Input,
    fp: play::FirstPerson,
    playable: Option<town::collide::Playable>,
    prompt: Option<&'static str>,
    /// animated people (town::people)
    people: Vec<(town::people::CharacterModel, town::people::Person)>,
}

impl App {
    /// Walk the player for `secs` (scripted: straight ahead), doors and all.
    fn simulate(&mut self, w: Walk) {
        let Some(p) = self.playable.as_mut() else { return };
        let dt = 1.0 / 120.0;
        // (each step timed: a player's frame of walking, doors and the look-at test)
        let mut times: Vec<f64> = Vec::new();
        for k in 0..((w.secs / dt) as u32 + 1) {
            let t0 = Instant::now();
            let ph = k as f32 * dt;
            let look = if w.look { Vec2::new((ph * 2.1).sin() * 0.03, (ph * 3.3).cos() * 0.025) } else { Vec2::ZERO };
            let a = play::Actions { movement: Vec2::new(if w.look { (ph * 0.7).sin() * 0.6 } else { 0.0 }, if w.secs > 0.0 && !w.still { 1.0 } else { 0.0 }), look, interact: w.interact && k == 30, ..Default::default() };
            if a.interact {
                p.doors.interact(&p.world, self.fp.eye(), self.fp.look_dir());
            }
            self.fp.update(&a, &p.world, dt);
            p.doors.update(&mut p.world, &self.fp, dt);
            let _ = p.doors.looked_at(&p.world, self.fp.eye(), self.fp.look_dir(), 2.2);
            if let Some(wd) = self.world.as_mut() {
                wd.cloth.step(dt, self.fp.eye(), &[town::cloth::person(&self.fp)]);
            }
            times.push(t0.elapsed().as_secs_f64() * 1e3);
        }
        if w.secs > 0.5 {
            times.sort_by(f64::total_cmp);
            let avg = times.iter().sum::<f64>() / times.len() as f64;
            println!("  walking: {} steps, avg {:.3} ms, 99% {:.3} ms, worst {:.3} ms per step", times.len(), avg, times[times.len() * 99 / 100], times[times.len() - 1]);
        }
    }

    fn apply_shot(&mut self, i: usize) {
        if let Some(w) = &mut self.world {
            w.cloth.reset();
        }
        let s = &self.shots[i];
        self.time = GameTime::new(self.args.date, self.args.minute.unwrap_or(s.time));
        let g = self.town.surface(s.at);
        self.cam.position = Vec3::new(s.at[0], g + s.up, -s.at[1]);
        self.cam.yaw = s.yaw;
        self.cam.pitch = s.pitch;
        if let Some(w) = s.walk {
            let (yaw, pitch) = (s.yaw, s.pitch);
            if let Some(p) = self.playable.as_mut() {
                for d in &mut p.doors.doors {
                    d.open = 0.0;
                    d.target = 0.0;
                    p.world.set_transform(d.body, d.transform());
                }
            }
            self.fp.teleport(Vec3::new(self.cam.position.x, g + 0.05, self.cam.position.z), yaw);
            self.fp.pitch = pitch;
            self.simulate(Walk { secs: 0.0, interact: false, still: true, look: false }); // (settle on the floor)
            self.simulate(w);
            self.cam.position = self.fp.eye();
            println!("{}: walked to plan ({:.2}, {:.2}), feet {:.2}", self.shots[i].name, self.fp.feet.x, -self.fp.feet.z, self.fp.feet.y);
        }
        self.noren_shot(i);
        if let Some(w) = &mut self.world {
            w.relist();
        }
        if let Some(r) = &mut self.r {
            r.reset_history();
        }
    }

    /// The noren shots (`NOREN_SHOTS`): placed from where the cloth hangs. The player walks from
    /// the shop through the doorway's noren; the camera is theirs, or stands in the shop and
    /// watches.
    fn noren_shot(&mut self, i: usize) {
        let Some(&(_, walk, first_person)) = NOREN_SHOTS.iter().find(|n| n.0 == self.shots[i].name) else { return };
        let (Some(w), Some(p)) = (self.world.as_mut(), self.playable.as_mut()) else { return };
        let Some(c) = w.cloth.pieces.iter().find(|c| c.name == "kzcloth_noren") else { return };
        let (centre, f) = (c.centre, c.facing);
        let side = Vec3::Y.cross(f);
        let floor = p.world.raycast(Vec3::new(centre.x, c.bottom, centre.z) + f * 0.6, Vec3::NEG_Y, 3.0).map_or(c.bottom - 1.15, |h| h.point.y);
        let look = |from: Vec3, to: Vec3| {
            let d = (to - from).normalize();
            (d.x.atan2(-d.z), d.y.asin())
        };
        // the player: from 1.8 m in front, through the slit
        let start = Vec3::new(centre.x, floor + 0.05, centre.z) + f * 1.8;
        self.fp.teleport(start, look(start, start - f).0);
        self.fp.pitch = -6.0f32.to_radians();
        self.simulate(Walk { secs: 0.0, interact: false, still: true, look: false });
        if let Some(w) = self.world.as_mut() {
            // a little of the house's draughts first (not all frozen at its rest)
            for _ in 0..240 {
                w.cloth.step(1.0 / 120.0, start, &[]);
            }
        }
        if let Some(secs) = walk {
            self.simulate(Walk { secs, interact: false, still: false, look: false });
        }
        let to_plane = (self.fp.feet - centre).dot(f);
        println!("{}: the player {:.2} m in front of the noren (negative: through it)", self.shots[i].name, to_plane);
        if first_person && walk.is_some() {
            self.cam.position = self.fp.eye();
            self.cam.yaw = self.fp.yaw;
            self.cam.pitch = self.fp.pitch;
        } else {
            // watching from the shop, a little to one side
            let at = Vec3::new(centre.x, floor + 1.45, centre.z) + f * if walk.is_some() { 2.2 } else { 1.6 } + side * if walk.is_some() { 1.0 } else { 0.35 };
            let (yaw, pitch) = look(at, centre - Vec3::Y * 0.1);
            self.cam.position = at;
            self.cam.yaw = yaw;
            self.cam.pitch = pitch;
        }
    }

    fn frame(&mut self, el: &ActiveEventLoop) -> Result<()> {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        if !self.args.gate {
            self.input.keyboard().mouse_look = self.captured;
            let a = self.input.frame(dt, true);
            if self.fly || self.playable.is_none() {
                // free flight (Space up, Ctrl down)
                self.cam.yaw += a.look.x;
                self.cam.pitch = (self.cam.pitch + a.look.y).clamp(-1.5, 1.5);
                let speed = if a.run { 60.0 } else { 15.0 };
                let (f, rt) = (self.cam.forward_flat(), self.cam.right_flat());
                let up = self.keys.contains(&KeyCode::Space) as i32 as f32 - self.keys.contains(&KeyCode::ControlLeft) as i32 as f32;
                self.cam.position += (f * a.movement.y + rt * a.movement.x + Vec3::Y * up) * speed * dt;
                let g = self.town.surface([self.cam.position.x, -self.cam.position.z]);
                self.cam.position.y = self.cam.position.y.max(g + 0.5);
            } else if let Some(p) = self.playable.as_mut() {
                if a.interact {
                    p.doors.interact(&p.world, self.fp.eye(), self.fp.look_dir());
                }
                self.fp.update(&a, &p.world, dt);
                p.doors.update(&mut p.world, &self.fp, dt);
                self.prompt = p.doors.looked_at(&p.world, self.fp.eye(), self.fp.look_dir(), 2.2).map(|_| if a.device == play::input::Device::Gamepad { "[Square] door" } else { "[E] door" });
                self.cam.position = self.fp.eye();
                self.cam.yaw = self.fp.yaw;
                self.cam.pitch = self.fp.pitch;
            }
        }
        let (sun, sun_i, moon, moon_i, stars) = sky_frame(&self.time);
        // the town round the camera: its plants listed again as it moves, the doors as they are
        if let (Some(w), Some(r)) = (self.world.as_mut(), self.r.as_mut()) {
            // the cloth (the noren), with the player in it when walking (still for the gate's
            // shots: they show it as the walk left it)
            let walking = !self.fly && self.playable.is_some();
            let people = [town::cloth::person(&self.fp)];
            w.cloth.step(if self.args.gate { 0.0 } else { dt }, self.cam.position, if walking { &people[..] } else { &[] });
            let t = Instant::now();
            let n = w.frame(r, &self.town, self.cam.position, self.playable.as_ref(), &mut self.instances)?;
            for (m, person) in &mut self.people {
                person.frame(r, m, dt, &mut self.instances)?;
            }
            if self.args.gate && n > 0 {
                println!("plants: {n} instances ({:.0} ms)", t.elapsed().as_secs_f64() * 1e3);
            }
        }
        let capture = self.shot_request.is_some() || (self.args.gate && self.shot_frames == 100);
        let desc = FrameDesc {
            camera: &self.cam,
            sun_dir: sun,
            sun_illuminance: sun_i,
            moon_dir: moon,
            moon_illuminance: moon_i,
            stars,
            altitude: self.cam.position.y + 20.0,
            ground_albedo: 0.12,
            haze: 1.0,
            instances: &self.instances,
            lights: &[],
            time: (now - self.start).as_secs_f32(),
            dt,
            exposure_ev: 0.0,
            debug: 0,
            capture,
            ui: None,
        };
        let r = self.r.as_mut().unwrap();
        let img = r.render(&desc)?;
        let (w, h) = r.extent();
        if let Some(px) = img {
            let path = if self.args.gate {
                // (KZ_SHOT_FILE: where to save it, so several runs at once don't overwrite each other)
                std::env::var("KZ_SHOT_FILE").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(format!("shots/town/{}.png", self.shots[self.shot_i].name)))
            } else {
                self.shot_request.take().unwrap()
            };
            save_png(&path, w, h, &px)?;
            println!("saved {}  (scene luminance {:.2e})", path.display(), self.r.as_ref().unwrap().scene_luminance);
        }
        if self.args.gate {
            self.shot_frames += 1;
            if self.shot_frames > 100 {
                self.shot_frames = 0;
                self.shot_i += 1;
                if self.shot_i >= self.shots.len() {
                    el.exit();
                    return Ok(());
                }
                self.apply_shot(self.shot_i);
            }
        }
        if let Some(w) = &self.window {
            let t = self.time;
            let p = self.cam.position;
            let mode = if self.fly { " — flying (F to walk)" } else { "" };
            let prompt = self.prompt.map(|s| format!(" — {s}")).unwrap_or_default();
            w.set_title(&format!("Kazehana-cho — {} {:02}:{:02} — plan ({:.0}, {:.0}){mode}{prompt}", t.date, t.minute / 60, t.minute % 60, p.x, -p.z));
            w.request_redraw();
        }
        Ok(())
    }
}

fn save_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let headless = self.args.gate;
        let attrs = Window::default_attributes().with_title("Kazehana-cho").with_inner_size(PhysicalSize::new(1920, 1080)).with_visible(!headless).with_active(!headless);
        let window = Arc::new(el.create_window(attrs).expect("window"));
        let opts = RendererOptions { validation: self.args.validation, vsync: !headless, dlss: if self.args.dlss { Some(1) } else { None }, render_scale: 1.0, bounces: self.args.bounces };
        let mut r = match Renderer::new(&window, opts) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("renderer: {e:#}");
                el.exit();
                return;
            }
        };
        match town::scene::TownWorld::load(&mut r, &self.town, !self.args.no_plants) {
            Ok((w, p)) => {
                self.world = Some(w);
                self.playable = Some(p);
                // characters standing in the shop in a row across the view, facing the door
                let n = self.args.characters.len();
                for (i, path) in self.args.characters.clone().iter().enumerate() {
                    match town::people::CharacterModel::load(&mut r, path) {
                        Ok(m) => {
                            let (feet, yaw) = town::scene::home_start(&self.town);
                            let fwd = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
                            let right = Vec3::new(yaw.cos(), 0.0, yaw.sin());
                            let across = (i as f32 - (n as f32 - 1.0) * 0.5) * 0.8;
                            let dist = std::env::var("KZ_PEOPLE_DIST").ok().and_then(|d| d.parse::<f32>().ok()).unwrap_or(if n > 1 { 3.4 } else { 2.5 });
                            let at = feet + fwd * dist + right * across;
                            let t = akari::glam::Affine3A::from_rotation_translation(akari::glam::Quat::from_rotation_y(-yaw), at); // (a glTF character faces +Z: turned to face the camera)
                            let want = std::env::var("KZ_CLIP").unwrap_or_else(|_| "idle".into());
                            let clip = if want == "rest" { None } else { m.clip(&want).or(if m.clips.is_empty() { None } else { Some(0) }) };
                            println!("character {}: {} joints, {} clips", path.display(), m.skeleton.joints.len(), m.clips.len());
                            let mut person = town::people::Person::new(t, clip);
                            person.time = i as f32 * 1.7; // (each a different moment of the clip)
                            self.people.push((m, person));
                        }
                        Err(e) => eprintln!("character {}: {e:#}", path.display()),
                    }
                }
            }
            Err(e) => {
                eprintln!("town: {e:#}");
                el.exit();
                return;
            }
        }
        self.r = Some(r);
        self.window = Some(window.clone());
        if self.args.gate {
            self.apply_shot(0);
        }
        self.last = Instant::now();
        window.request_redraw();
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.args.gate && self.r.is_some() {
            if let Err(e) = self.frame(el) {
                eprintln!("frame: {e:#}");
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Focused(false) => self.input.keyboard().release_all(),
            WindowEvent::Resized(s) => {
                if let Some(r) = &mut self.r {
                    r.resize(s.width, s.height);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                if let Some(w) = &self.window {
                    let _ = w.set_cursor_grab(CursorGrabMode::Confined).or_else(|_| w.set_cursor_grab(CursorGrabMode::Locked));
                    w.set_cursor_visible(false);
                }
                self.captured = true;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(k) = event.physical_key else { return };
                if event.state == ElementState::Pressed {
                    if !event.repeat {
                        match k {
                            KeyCode::Escape => {
                                if self.captured {
                                    if let Some(w) = &self.window {
                                        let _ = w.set_cursor_grab(CursorGrabMode::None);
                                        w.set_cursor_visible(true);
                                    }
                                    self.captured = false;
                                } else {
                                    el.exit()
                                }
                            }
                            KeyCode::BracketLeft => self.time.minute = (self.time.minute + 1440 - 60) % 1440,
                            KeyCode::BracketRight => self.time.minute = (self.time.minute + 60) % 1440,
                            KeyCode::KeyF => self.fly = !self.fly,
                            KeyCode::KeyL => {
                                if let Some(r) = &mut self.r {
                                    let m = if r.dlss_mode().is_some() { None } else { Some(1) };
                                    let _ = r.set_dlss(m);
                                }
                            }
                            KeyCode::F12 => {
                                let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                                self.shot_request = Some(PathBuf::from(format!("shots/town_{stamp}.png")));
                            }
                            _ => {}
                        }
                    }
                    self.keys.insert(k);
                } else {
                    self.keys.remove(&k);
                }
                self.input.keyboard().key(k, event.state == ElementState::Pressed, event.repeat);
            }
            WindowEvent::RedrawRequested => {
                if self.r.is_some() && !self.args.gate {
                    if let Err(e) = self.frame(el) {
                        eprintln!("frame: {e:#}");
                        el.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.input.keyboard().mouse_motion(delta.0, delta.1);
        }
    }
}

fn main() -> Result<()> {
    let args = parse_args();
    let town = Town::load(Path::new("assets/town")).context("loading the town plan")?;
    let mut shots = shots(&town);
    // the noren's (placed once the town is loaded: App::noren_shot)
    let g = town.plan.houses[town.plan.home].gate;
    shots.extend(NOREN_SHOTS.iter().map(|n| Shot { walk: None, name: n.0.into(), time: at::hm(10, 30), at: g, up: EYE, yaw: 0.0, pitch: 0.0 }));
    if let Some(o) = &args.only {
        shots.retain(|s| o.split(',').any(|n| n == s.name));
        anyhow::ensure!(!shots.is_empty(), "no shot named {o}");
    }
    let mut cam = Camera { fov_y: 62f32.to_radians(), far: 6000.0, ..Default::default() };
    let home = town.plan.houses[town.plan.home].gate;
    cam.position = Vec3::new(home[0], town.ground(home) + EYE, -home[1]);
    cam.yaw = std::f32::consts::PI;
    let time = GameTime::new(args.date, args.minute.unwrap_or(at::hm(8, 0)));
    let headless_run = args.gate;
    let town_ground = town.ground(home);
    let event_loop = EventLoop::new().context("event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        args,
        town,
        shots,
        window: None,
        r: None,
        instances: Vec::new(),
        world: None,
        cam,
        time,
        keys: HashSet::new(),
        captured: false,
        fly: false,
        last: Instant::now(),
        start: Instant::now(),
        shot_i: 0,
        shot_frames: 0,
        shot_request: None,
        input: play::Input::new(!headless_run),
        fp: play::FirstPerson::new(Vec3::new(home[0], town_ground, -home[1]), std::f32::consts::PI, play::Settings::default()),
        playable: None,
        prompt: None,
        people: Vec::new(),
    };
    event_loop.run_app(&mut app)?;
    drop(app.r.take());
    Ok(())
}
