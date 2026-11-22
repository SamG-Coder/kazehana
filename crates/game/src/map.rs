//! The town map (M) and the compass: a clean, dark map drawn from the town plan itself, so it is
//! sharp at every zoom (roads by class, the river, the railway, every house and site, fields and
//! forest from the land-use map underneath); the places you can go, a waypoint you choose (or the
//! tracked quest's), and the way there along the roads (a route over the road network, as a GPS
//! would show it). The compass along the top of the screen shows where you're facing and where
//! the waypoint is.
//!
//! Plan coordinates: x east, y north (metres); the world is (x, -z).

use akari::{Rgba, Ui};
use std::collections::{BinaryHeap, HashMap};

/// A place on the map's list.
#[derive(Clone, Debug)]
pub struct Spot {
    pub name: String,
    pub at: [f32; 2],
    /// a one-letter mark for the list and the map
    pub mark: char,
}

pub struct Road {
    pts: Vec<[f32; 2]>,
    width: f32,
    class: u8, // 0 field track, 1 lane, 2 street, 3 main road
}

pub struct TownMap {
    roads: Vec<Road>,
    rail: Vec<[f32; 2]>,
    river: Vec<[f32; 2]>,
    river_half: f32,
    houses: Vec<Vec<[f32; 2]>>,
    sites: Vec<(Vec<[f32; 2]>, u8)>,
    buildings: Vec<Vec<[f32; 2]>>,
    fields: Vec<(Vec<[f32; 2]>, bool)>,
    /// the land-use raster (image "town_map_base"): its plan extent (x0, y0, x1, y1)
    base: Option<[f32; 4]>,
    base_px: Option<(u32, u32, Vec<u8>)>,
    pub spots: Vec<Spot>,
    #[allow(dead_code)]
    pub home: [f32; 2],
    /// the road network: nodes (plan) and their neighbours (node, metres)
    nodes: Vec<[f32; 2]>,
    adj: Vec<Vec<(u32, f32)>>,
}

pub const C_LAND: Rgba = Rgba(34, 39, 43, 255);
const C_WATER: Rgba = Rgba(52, 96, 138, 255);
const C_RAIL: Rgba = Rgba(128, 134, 142, 255);
const C_HOUSE: Rgba = Rgba(66, 72, 80, 255);
const C_BUILDING: Rgba = Rgba(84, 92, 104, 255);
const C_PADDY: Rgba = Rgba(44, 54, 40, 255);
const C_HATAKE: Rgba = Rgba(54, 50, 40, 255);
const C_ROUTE: Rgba = Rgba(196, 92, 232, 255);
pub const C_QUEST: Rgba = Rgba(246, 206, 72, 255);
pub const C_PIN: Rgba = Rgba(232, 96, 180, 255);

fn road_colour(class: u8) -> Rgba {
    match class {
        0 => Rgba(86, 90, 82, 255),
        1 => Rgba(150, 156, 162, 255),
        2 => Rgba(196, 200, 205, 255),
        _ => Rgba(232, 210, 128, 255),
    }
}

impl TownMap {
    pub fn build(town: &town::Town) -> Self {
        let p = &town.plan;
        let class = |k: &str| match k {
            "farm" | "levee" => 0,
            "lane" => 1,
            "pref" | "old" => 3,
            _ => 2,
        };
        let roads: Vec<Road> = p.roads.iter().map(|r| Road { pts: r.pts.clone(), width: r.width, class: class(&r.kind) }).collect();
        // the land-use map, 5 m a pixel, coloured (forest, fields, built-up)
        let (base, base_px) = match town::load_landuse(&town.dir.join(&p.terrain.landuse)) {
            Ok(lu) => {
                let (nx, ny) = (p.terrain.nx, p.terrain.ny);
                const K: usize = 5;
                let (w, h) = (nx / K, ny / K);
                let mut px = vec![0u8; w * h * 4];
                for j in 0..h {
                    for i in 0..w {
                        // (row 0 of the land use is the north edge)
                        let code = lu[(j * K) * nx + i * K];
                        let c = match code {
                            6 => Rgba(29, 44, 36, 255),       // forest
                            1 => Rgba(40, 49, 38, 255),       // paddies
                            2 => Rgba(48, 46, 38, 255),       // vegetable plots
                            7 => C_WATER,                     // riverbed
                            8 => Rgba(38, 46, 40, 255),       // levee
                            4 | 5 | 13 => Rgba(40, 44, 48, 255), // concrete, asphalt, ballast
                            _ => C_LAND,
                        };
                        px[(j * w + i) * 4..][..4].copy_from_slice(&[c.0, c.1, c.2, 255]);
                    }
                }
                let x0 = p.terrain.origin[0];
                let y1 = p.terrain.origin[1] + ny as f32;
                (Some([x0, y1 - (h * K) as f32, x0 + (w * K) as f32, y1]), Some((w as u32, h as u32, px)))
            }
            Err(_) => (None, None),
        };
        let kind = |k: &str| match k {
            "school" => 1,
            "shrine" | "inari" | "temple" => 2,
            "station" => 3,
            _ => 0,
        };
        // the places on the list: home first, then school, then the town's
        let c = sim::school::census();
        let home = p.houses[p.home].gate;
        let mut spots = vec![Spot { name: "Home — Tsukumodō".into(), at: home, mark: 'H' }, Spot { name: "Kazehana Junior High".into(), at: p.school_gate, mark: 'S' }];
        use census::PlaceKind as K;
        for pl in &c.places {
            let mark = match pl.kind {
                K::Station => 'R',
                K::Konbini => 'K',
                K::Shrine | K::InariShrine => '神',
                K::Temple => '寺',
                K::Elementary => 'E',
                K::PostOffice => 'P',
                K::TownOffice => 'T',
                K::CoOp | K::Drugstore | K::Petrol | K::CarRepair => '¥',
                _ => continue,
            };
            let mut name = pl.name.clone();
            if let Some(f) = name.get_mut(0..1) {
                f.make_ascii_uppercase();
            }
            spots.push(Spot { name, at: pl.at, mark });
        }
        let mut m = TownMap {
            roads,
            rail: p.rail.clone(),
            river: p.river.pts.clone(),
            river_half: p.river.water_half,
            houses: p.houses.iter().map(|h| h.house.clone()).collect(),
            sites: p.sites.iter().map(|s| (s.poly.clone(), kind(&s.kind))).collect(),
            buildings: p.sites.iter().flat_map(|s| s.buildings.iter().filter(|b| b.name != "sports ground").map(|b| b.poly.clone())).collect(),
            fields: p.paddies.iter().map(|q| (q.clone(), true)).chain(p.hatake.iter().map(|q| (q.clone(), false))).collect(),
            base,
            base_px,
            spots,
            home,
            nodes: Vec::new(),
            adj: Vec::new(),
        };
        m.build_graph();
        m
    }

    /// The road network as a graph: each road's points, every few metres, joined along the road;
    /// roads joined where they meet (points of two roads within a few metres).
    fn build_graph(&mut self) {
        let mut nodes: Vec<[f32; 2]> = Vec::new();
        let mut adj: Vec<Vec<(u32, f32)>> = Vec::new();
        let d = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
        for r in &self.roads {
            let mut prev: Option<u32> = None;
            for w in r.pts.windows(2) {
                let len = d(w[0], w[1]);
                let n = (len / 8.0).ceil().max(1.0) as usize;
                for k in 0..=n {
                    if k == 0 && prev.is_some() {
                        continue;
                    }
                    let t = k as f32 / n as f32;
                    let q = [w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t];
                    let id = nodes.len() as u32;
                    nodes.push(q);
                    adj.push(Vec::new());
                    if let Some(p) = prev {
                        let l = d(nodes[p as usize], q);
                        adj[p as usize].push((id, l));
                        adj[id as usize].push((p, l));
                    }
                    prev = Some(id);
                }
            }
        }
        // junctions: nodes of different roads close together (a hash of 6 m cells)
        let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        let cell = |q: [f32; 2]| ((q[0] / 6.0).floor() as i32, (q[1] / 6.0).floor() as i32);
        for (i, q) in nodes.iter().enumerate() {
            grid.entry(cell(*q)).or_default().push(i as u32);
        }
        for i in 0..nodes.len() {
            let (cx, cy) = cell(nodes[i]);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for &j in grid.get(&(cx + dx, cy + dy)).map(|v| v.as_slice()).unwrap_or(&[]) {
                        let j = j as usize;
                        if j <= i || adj[i].iter().any(|e| e.0 as usize == j) {
                            continue;
                        }
                        let l = d(nodes[i], nodes[j]);
                        if l < 5.0 {
                            adj[i].push((j as u32, l));
                            adj[j].push((i as u32, l));
                        }
                    }
                }
            }
        }
        self.nodes = nodes;
        self.adj = adj;
    }

    fn nearest_node(&self, q: [f32; 2]) -> Option<usize> {
        self.nodes.iter().enumerate().min_by(|a, b| {
            let da = (a.1[0] - q[0]).powi(2) + (a.1[1] - q[1]).powi(2);
            let db = (b.1[0] - q[0]).powi(2) + (b.1[1] - q[1]).powi(2);
            da.total_cmp(&db)
        }).map(|x| x.0)
    }

    /// The way from `a` to `b` along the roads (A*), as plan points from a to b; None if the
    /// roads don't join up.
    pub fn route(&self, a: [f32; 2], b: [f32; 2]) -> Option<Vec<[f32; 2]>> {
        let (s, t) = (self.nearest_node(a)?, self.nearest_node(b)?);
        #[derive(PartialEq)]
        struct Item(f32, usize);
        impl Eq for Item {}
        impl PartialOrd for Item {
            fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(o))
            }
        }
        impl Ord for Item {
            fn cmp(&self, o: &Self) -> std::cmp::Ordering {
                o.0.total_cmp(&self.0)
            }
        }
        let h = |i: usize| (self.nodes[i][0] - self.nodes[t][0]).hypot(self.nodes[i][1] - self.nodes[t][1]);
        let n = self.nodes.len();
        let mut g = vec![f32::INFINITY; n];
        let mut from = vec![u32::MAX; n];
        let mut open = BinaryHeap::new();
        g[s] = 0.0;
        open.push(Item(h(s), s));
        while let Some(Item(_, i)) = open.pop() {
            if i == t {
                break;
            }
            for &(j, l) in &self.adj[i] {
                let j = j as usize;
                let ng = g[i] + l;
                if ng < g[j] {
                    g[j] = ng;
                    from[j] = i as u32;
                    open.push(Item(ng + h(j), j));
                }
            }
        }
        if !g[t].is_finite() {
            return None;
        }
        let mut path = vec![b, self.nodes[t]];
        let mut i = t;
        while i != s {
            i = from[i] as usize;
            path.push(self.nodes[i]);
        }
        path.push(a);
        path.reverse();
        Some(path)
    }

    /// The land-use raster into the UI atlas (once).
    pub fn upload(&mut self, ui: &mut Ui) {
        if let Some((w, h, px)) = self.base_px.take() {
            if !ui.add_image("town_map_base", w, h, &px) {
                eprintln!("map: no room in the UI atlas for the land-use layer");
                self.base = None;
            }
        }
    }

    /// Draw the map into the rect (x, y, w, h) of the screen: centred on `centre`, `zoom` px a
    /// metre. `player`: where you are and which way you face (yaw, clockwise from north);
    /// `pin`: the waypoint and its colour; `route`: the way there.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, ui: &mut Ui, rect: [f32; 4], centre: [f32; 2], zoom: f32, player: ([f32; 2], f32), pin: Option<([f32; 2], Rgba)>, route: Option<&[[f32; 2]]>, quest: Option<[f32; 2]>) {
        let [x, y, w, h] = rect;
        let s = zoom * ui.scale();
        let to = |q: [f32; 2]| [x + w * 0.5 + (q[0] - centre[0]) * s, y + h * 0.5 - (q[1] - centre[1]) * s];
        // the visible plan rect (a margin round it)
        let (vx0, vx1) = (centre[0] - w * 0.5 / s - 20.0, centre[0] + w * 0.5 / s + 20.0);
        let (vy0, vy1) = (centre[1] - h * 0.5 / s - 20.0, centre[1] + h * 0.5 / s + 20.0);
        let vis = |pts: &[[f32; 2]]| pts.iter().any(|q| q[0] > vx0 && q[0] < vx1 && q[1] > vy0 && q[1] < vy1);
        ui.rect(x, y, w, h, C_LAND);
        // the land-use layer under it all
        if let Some([bx0, by0, bx1, by1]) = self.base {
            let (cx0, cx1, cy0, cy1) = (vx0.max(bx0), vx1.min(bx1), vy0.max(by0), vy1.min(by1));
            if cx0 < cx1 && cy0 < cy1 {
                let (a, b) = (to([cx0, cy1]), to([cx1, cy0]));
                let uv = [(cx0 - bx0) / (bx1 - bx0), (by1 - cy1) / (by1 - by0), (cx1 - bx0) / (bx1 - bx0), (by1 - cy0) / (by1 - by0)];
                ui.image_part("town_map_base", a[0], a[1], b[0] - a[0], b[1] - a[1], uv, Rgba(255, 255, 255, 255));
            }
        }
        for (q, paddy) in &self.fields {
            if vis(q) {
                ui.poly(&q.iter().map(|p| to(*p)).collect::<Vec<_>>(), if *paddy { C_PADDY } else { C_HATAKE });
            }
        }
        for (q, k) in &self.sites {
            if vis(q) {
                let c = match k {
                    1 => Rgba(44, 52, 62, 255),
                    2 => Rgba(54, 48, 40, 255),
                    3 => Rgba(48, 50, 56, 255),
                    _ => Rgba(40, 45, 50, 255),
                };
                ui.poly(&q.iter().map(|p| to(*p)).collect::<Vec<_>>(), c);
            }
        }
        // the river: its water as a wide line
        for wd in self.river.windows(2) {
            if vis(wd) {
                ui.line(to(wd[0]), to(wd[1]), (2.0 * self.river_half * s).max(3.0), C_WATER);
            }
        }
        for q in &self.houses {
            if vis(q) {
                ui.poly(&q.iter().map(|p| to(*p)).collect::<Vec<_>>(), C_HOUSE);
            }
        }
        for q in &self.buildings {
            if vis(q) {
                ui.poly(&q.iter().map(|p| to(*p)).collect::<Vec<_>>(), C_BUILDING);
            }
        }
        // roads: an outline, then the fill, by class (main roads last, on top)
        for pass in 0..2 {
            for class in 0..4u8 {
                for r in self.roads.iter().filter(|r| r.class == class) {
                    let px = (r.width * s).max(match class {
                        0 => 1.2,
                        1 => 1.8,
                        2 => 2.4,
                        _ => 3.2,
                    });
                    for wd in r.pts.windows(2) {
                        if !vis(wd) {
                            continue;
                        }
                        if pass == 0 {
                            ui.line(to(wd[0]), to(wd[1]), px + 2.0, Rgba(18, 20, 22, 255));
                        } else {
                            ui.line(to(wd[0]), to(wd[1]), px, road_colour(class));
                        }
                    }
                }
            }
        }
        // the railway: grey, with ties
        for wd in self.rail.windows(2) {
            if vis(wd) {
                ui.line(to(wd[0]), to(wd[1]), 3.0, C_RAIL);
            }
        }
        // the way to the waypoint
        if let Some(r) = route {
            for wd in r.windows(2) {
                ui.line(to(wd[0]), to(wd[1]), (4.0 * ui.scale()).max(3.0), C_ROUTE);
            }
        }
        // places: a round badge with their mark, and the name at closer zooms
        let k = ui.scale();
        for sp in &self.spots {
            let p = to(sp.at);
            if p[0] < x || p[0] > x + w || p[1] < y || p[1] > y + h {
                continue;
            }
            badge(ui, p, 11.0 * k, Rgba(24, 28, 32, 235), sp.mark, Rgba(235, 238, 240, 255));
            if zoom > 0.9 {
                ui.text_shadow(p[0] + 14.0 * k, p[1] - 9.0 * k, 15.0 * k, Rgba(225, 228, 232, 255), &sp.name);
            }
        }
        if let Some(q) = quest {
            diamond(ui, to(q), 11.0 * k, C_QUEST);
        }
        if let Some((q, c)) = pin {
            diamond(ui, to(q), 11.0 * k, c);
        }
        arrow(ui, to(player.0), player.1, 13.0 * k);
    }
}

/// A round badge with a letter (a place on the map).
pub fn badge(ui: &mut Ui, c: [f32; 2], r: f32, bg: Rgba, mark: char, fg: Rgba) {
    let n = 16;
    let pts: Vec<[f32; 2]> = (0..n).map(|i| {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        [c[0] + r * a.cos(), c[1] + r * a.sin()]
    }).collect();
    let rim: Vec<[f32; 2]> = (0..n).map(|i| {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        [c[0] + (r + 1.5) * a.cos(), c[1] + (r + 1.5) * a.sin()]
    }).collect();
    ui.poly(&rim, Rgba(230, 232, 235, 200));
    ui.poly(&pts, bg);
    let s = mark.to_string();
    let px = r * 1.3;
    let tw = ui.measure(px, &s);
    ui.text(c[0] - tw * 0.5, c[1] - px * 0.62, px, fg, &s);
}

/// A waypoint mark: a diamond with a dark rim.
pub fn diamond(ui: &mut Ui, c: [f32; 2], r: f32, col: Rgba) {
    let d = |r: f32| [[c[0], c[1] - r], [c[0] + r, c[1]], [c[0], c[1] + r], [c[0] - r, c[1]]];
    ui.poly(&d(r + 2.0), Rgba(16, 18, 20, 230));
    ui.poly(&d(r), col);
}

/// You: a white arrowhead pointing along yaw (clockwise from north, the screen's up).
pub fn arrow(ui: &mut Ui, c: [f32; 2], yaw: f32, r: f32) {
    let rot = |x: f32, y: f32| [c[0] + x * yaw.cos() - y * yaw.sin(), c[1] + x * yaw.sin() + y * yaw.cos()];
    let (tip, l, b, rr) = (rot(0.0, -r), rot(-0.75 * r, 0.8 * r), rot(0.0, 0.35 * r), rot(0.75 * r, 0.8 * r));
    let grow = |p: [f32; 2], k: f32| [c[0] + (p[0] - c[0]) * k, c[1] + (p[1] - c[1]) * k];
    ui.tri(grow(tip, 1.25), grow(l, 1.25), grow(b, 1.25), Rgba(16, 18, 20, 230));
    ui.tri(grow(tip, 1.25), grow(b, 1.25), grow(rr, 1.25), Rgba(16, 18, 20, 230));
    ui.tri(tip, l, b, Rgba(250, 250, 250, 255));
    ui.tri(tip, b, rr, Rgba(250, 250, 250, 255));
}

/// The compass along the top of the screen: the heading's ticks and letters, and the waypoint
/// (its bearing from you, its distance).
pub fn compass(ui: &mut Ui, yaw: f32, marks: &[(f32, Rgba, String)]) {
    let k = ui.scale();
    let (cx, top, half) = (ui.width * 0.5, 22.0 * k, 300.0 * k);
    let span = 100f32.to_radians(); // degrees shown either side
    ui.gradient(cx - half, top, 2.0 * half, 30.0 * k, Rgba(10, 12, 14, 150), Rgba(10, 12, 14, 60));
    let px = |a: f32| {
        let d = (a - yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        (d.abs() <= span).then(|| cx + d / span * half)
    };
    for i in 0..72 {
        let a = (i as f32 * 5.0).to_radians();
        let Some(x) = px(a) else { continue };
        let fade = 1.0 - ((x - cx).abs() / half).powi(2);
        let al = (fade * 255.0) as u8;
        let (len, label) = match i * 5 {
            0 => (12.0, Some("N")),
            45 => (9.0, Some("NE")),
            90 => (12.0, Some("E")),
            135 => (9.0, Some("SE")),
            180 => (12.0, Some("S")),
            225 => (9.0, Some("SW")),
            270 => (12.0, Some("W")),
            315 => (9.0, Some("NW")),
            d if d % 15 == 0 => (7.0, None),
            _ => (4.0, None),
        };
        ui.rect(x - 0.75 * k, top + 2.0 * k, 1.5 * k, len * k, Rgba(230, 232, 235, al));
        if let Some(t) = label {
            let size = if t.len() == 1 { 17.0 } else { 13.0 } * k;
            let tw = ui.measure(size, t);
            let col = if t == "N" { Rgba(240, 96, 80, al) } else { Rgba(235, 238, 240, al) };
            ui.text(x - tw * 0.5, top + 13.0 * k, size, col, t);
        }
    }
    ui.rect(cx - 1.0 * k, top - 4.0 * k, 2.0 * k, 38.0 * k, Rgba(255, 255, 255, 200));
    for (bearing, col, label) in marks {
        let x = match px(*bearing) {
            Some(x) => x,
            // behind you: pinned to the edge it's nearer to
            None => {
                let d = (*bearing - yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                if d > 0.0 { cx + half } else { cx - half }
            }
        };
        diamond(ui, [x, top + 15.0 * k], 7.0 * k, *col);
        let tw = ui.measure(14.0 * k, label);
        ui.text_shadow(x - tw * 0.5, top + 31.0 * k, 14.0 * k, Rgba(240, 242, 245, 255), label);
    }
}

/// Bearing (clockwise from north) from a to b, plan coordinates.
pub fn bearing(a: [f32; 2], b: [f32; 2]) -> f32 {
    (b[0] - a[0]).atan2(b[1] - a[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_to_school_by_road() {
        let Ok(t) = town::Town::load(std::path::Path::new("../../assets/town")) else { return };
        let m = TownMap::build(&t);
        let r = m.route(m.home, t.plan.school_gate).expect("a way to school");
        let len: f32 = r.windows(2).map(|w| (w[0][0] - w[1][0]).hypot(w[0][1] - w[1][1])).sum();
        let straight = (m.home[0] - t.plan.school_gate[0]).hypot(m.home[1] - t.plan.school_gate[1]);
        // on foot it fits the morning: up at 6:45, homeroom at 8:30, the walk well under an hour
        // of game time (or you'd be late however you hurried)
        let walk = play::Settings::default().walk;
        let mins = len / walk * sim::life::EXPLORE_PACE;
        println!("home to the school gate: route {len:.0} m, straight {straight:.0} m, {mins:.0} game minutes on foot");
        assert!(mins < 45.0, "the walk to school takes {mins:.0} game minutes");
        assert!(len >= straight && len < straight * 2.5, "route {len:.0} m, straight {straight:.0} m");
        assert!(m.spots.len() >= 8, "{} places", m.spots.len());
    }
}
