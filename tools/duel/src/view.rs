//! The physical table: scene loading, where every card sits (layout), and the card objects that
//! glide between those places (so a summoned card flies from the hand to its zone).

use akari::glam::{Affine3A, Quat, Vec3};
use akari::textures::{file_key, TexJob, TexKind, TexSource};
use akari::*;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tcg::battle::{Duel, Stats, TRAP_ZONES, ZONES};
use tcg::CardId;

pub const CARD_W: f32 = 0.063;
pub const CARD_H: f32 = 0.088;
pub const CARD_T: f32 = 0.00031;
const CORNER: f32 = 0.00294;

/// Playmat layout (mm from the mat centre, +y towards the near player; tools/cardgen playmat.rs).
pub const MAT_MM: (f32, f32) = (640.0, 500.0);
const MONSTER_X: [f32; 3] = [-92.0, 0.0, 92.0];
const MONSTER_Y: f32 = 66.0;
const BACK_Y: f32 = 184.0;
const BACK_X: [f32; 5] = [-232.0, -116.0, 0.0, 116.0, 232.0];
pub const ZONE_MM: (f32, f32) = (74.0, 100.0);

/// The west play table in Tsukumodō (layout.json: Blender (-1.2, 0.95), top 0.72 m above the
/// 0.15 m floor). Engine frame: x east, y up, z = -Blender y.
pub const TABLE: Vec3 = Vec3::new(-1.2, 0.87, -0.95);
/// Mat surface (a 3 mm neoprene mat on the table top).
pub fn mat_origin() -> Vec3 {
    TABLE + Vec3::new(0.0, 0.003, 0.0)
}

/// A playmat point (mm) for `side` (0 = near, 1 = far: the same layout turned 180°).
pub fn mat_point(side: usize, x_mm: f32, y_mm: f32) -> Vec3 {
    let s = if side == 0 { 1.0 } else { -1.0 };
    mat_origin() + Vec3::new(s * x_mm * 0.001, 0.0, s * y_mm * 0.001)
}

pub fn monster_zone(side: usize, zone: usize) -> Vec3 {
    mat_point(side, MONSTER_X[zone], MONSTER_Y)
}
pub fn trap_zone(side: usize, slot: usize) -> Vec3 {
    mat_point(side, BACK_X[if slot == 0 { 1 } else { 3 }], BACK_Y)
}
pub fn leader_zone(side: usize) -> Vec3 {
    mat_point(side, BACK_X[2], BACK_Y)
}
pub fn deck_zone(side: usize) -> Vec3 {
    mat_point(side, BACK_X[4], BACK_Y)
}
pub fn discard_zone(side: usize) -> Vec3 {
    mat_point(side, BACK_X[0], BACK_Y)
}

/// A card lying flat, readable by `side`'s player; face down if `!face_up`.
pub fn lying(side: usize, face_up: bool) -> Quat {
    let turn = if side == 0 { Quat::IDENTITY } else { Quat::from_rotation_y(std::f32::consts::PI) };
    let lie = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let flip = if face_up { Quat::IDENTITY } else { Quat::from_rotation_y(std::f32::consts::PI) };
    turn * lie * flip
}

// ----------------------------------------------------------------------------- meshes

fn card_point(u: f32, v: f32, z: f32) -> [f32; 3] {
    [(u - 0.5) * CARD_W, (0.5 - v) * CARD_H, z]
}

fn outline_ccw() -> Vec<(f32, f32)> {
    let ru = CORNER / CARD_W;
    let rv = CORNER / CARD_H;
    let mut pts = Vec::new();
    let corners = [(0.5 - ru, -(0.5 - rv), -0.5f32), (0.5 - ru, 0.5 - rv, 0.0), (-(0.5 - ru), 0.5 - rv, 0.5), (-(0.5 - ru), -(0.5 - rv), 1.0)];
    for (cx, cy, a0) in corners {
        for k in 0..=8 {
            let a = (a0 + k as f32 / 8.0 * 0.5) * std::f32::consts::PI;
            pts.push((cx + a.cos() * ru + 0.5, 0.5 - (cy + a.sin() * rv)));
        }
    }
    pts
}

/// The card slab: front face, back face, edge.
fn card_meshes(front: MaterialId, back: MaterialId, edge: MaterialId) -> Vec<MeshData> {
    let ol = outline_ccw();
    let n = ol.len();
    let z = CARD_T * 0.5;
    let face = |name: &str, mat: MaterialId, zf: f32, nz: f32, mirror: bool| {
        let mut m = MeshData { name: name.into(), material: Some(mat), ..Default::default() };
        let uvof = |u: f32, v: f32| if mirror { [1.0 - u, v] } else { [u, v] };
        m.positions.push(card_point(0.5, 0.5, zf));
        m.normals.push([0.0, 0.0, nz]);
        m.uvs.push(uvof(0.5, 0.5));
        for &(u, v) in &ol {
            m.positions.push(card_point(u, v, zf));
            m.normals.push([0.0, 0.0, nz]);
            m.uvs.push(uvof(u, v));
        }
        for i in 0..n as u32 {
            let (a, b) = (1 + i, 1 + (i + 1) % n as u32);
            if nz > 0.0 { m.indices.extend([0, a, b]) } else { m.indices.extend([0, b, a]) }
        }
        m.generate_tangents();
        m
    };
    let mut meshes = vec![face("card front", front, z, 1.0, false), face("card back", back, -z, -1.0, true)];
    let mut e = MeshData { name: "card edge".into(), material: Some(edge), ..Default::default() };
    for i in 0..n {
        let (u0, v0) = ol[i];
        let (u1, v1) = ol[(i + 1) % n];
        let (p0, p1) = (card_point(u0, v0, 0.0), card_point(u1, v1, 0.0));
        let d = Vec3::new(p1[0] - p0[0], p1[1] - p0[1], 0.0);
        let nrm = Vec3::new(d.y, -d.x, 0.0).normalize_or_zero();
        let base = e.positions.len() as u32;
        for (p, zz) in [(p0, z), (p0, -z), (p1, -z), (p1, z)] {
            e.positions.push([p[0], p[1], zz]);
            e.normals.push(nrm.to_array());
            e.uvs.push([0.0, 0.0]);
        }
        e.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    meshes.push(e);
    meshes
}

fn layered_material(r: &mut Renderer, dir: &Path, flags: u32) -> Result<MaterialId> {
    let m = layered(r, dir, flags)?;
    Ok(r.add_material(m))
}

fn layered(r: &mut Renderer, dir: &Path, flags: u32) -> Result<Material> {
    let f = |n: &str| dir.join(n);
    for n in ["base.png", "normal.png", "orm.png"] {
        anyhow::ensure!(f(n).exists(), "missing {} (run tools/cardgen)", f(n).display());
    }
    let jobs = vec![
        TexJob { key: file_key(&f("base.png")), name: format!("{} base", dir.display()), kind: TexKind::Color, source: TexSource::File(f("base.png")) },
        TexJob { key: file_key(&f("normal.png")), name: format!("{} normal", dir.display()), kind: TexKind::Normal, source: TexSource::File(f("normal.png")) },
        TexJob { key: file_key(&f("orm.png")), name: format!("{} orm", dir.display()), kind: TexKind::Data, source: TexSource::File(f("orm.png")) },
    ];
    let t = r.add_textures(&jobs)?;
    Ok(Material {
        base_color: [1.0; 4],
        roughness: 1.0,
        metallic: 1.0,
        base_tex: Some(t[0]),
        normal_tex: Some(t[1]),
        mr_tex: Some(t[2]),
        occlusion_tex: Some(t[2]),
        flags,
        ..Default::default()
    })
}

/// A flat rounded-rectangle ring (outer half size `hx` x `hz`, `width` wide), facing up.
fn ring(name: &str, mat: MaterialId, hx: f32, hz: f32, radius: f32, width: f32) -> MeshData {
    let mut m = MeshData { name: name.into(), material: Some(mat), ..Default::default() };
    let pts = |inset: f32| -> Vec<[f32; 3]> {
        let (ax, az, r) = (hx - inset, hz - inset, (radius - inset).max(0.0005));
        let mut v = Vec::new();
        for (cx, cz, a0) in [(ax - r, az - r, 0.0f32), (-(ax - r), az - r, 0.5), (-(ax - r), -(az - r), 1.0), (ax - r, -(az - r), 1.5)] {
            for k in 0..=6 {
                let a = (a0 + k as f32 / 6.0 * 0.5) * std::f32::consts::PI;
                v.push([cx + a.cos() * r, 0.0, cz + a.sin() * r]);
            }
        }
        v
    };
    let (o, i) = (pts(0.0), pts(width));
    let n = o.len() as u32;
    for (a, b) in o.iter().zip(&i) {
        m.positions.push(*a);
        m.positions.push(*b);
        m.normals.extend([[0.0, 1.0, 0.0]; 2]);
        m.uvs.extend([[0.0, 0.0], [1.0, 1.0]]);
    }
    for k in 0..n {
        let (a, b) = (2 * k, 2 * ((k + 1) % n));
        m.indices.extend([a, b, a + 1, a + 1, b, b + 1]);
    }
    m.generate_tangents();
    m
}

/// Glows on the mat and the sparks of a hit (lit by themselves, seen by the camera only).
pub struct Fx {
    /// Zone rings: [own (green), enemy (red), hot (bright gold-white), ready (soft gold)].
    pub zone: [MeshId; 4],
    /// A ring just round a hand card: [playable (gold), hot].
    pub card: [MeshId; 2],
    /// Hit light, per colour (type colours in `Attribute::ALL` order, then white) and per
    /// brightness step (`FX_LEVELS`, dim to bright: an effect fades by stepping down):
    /// a thin ring of light 1 m across (scaled as it sweeps out over the mat) ...
    pub wave: [[MeshId; FX_LEVELS]; 7],
    /// ... a flash round a zone ...
    pub flash: [[MeshId; FX_LEVELS]; 7],
    /// ... and a fine streak 14 mm long along +Z.
    pub streak: [[MeshId; FX_LEVELS]; 7],
}

pub const FX_LEVELS: usize = 6;

fn glow(r: &mut Renderer, rgb: [f32; 3], k: f32) -> MaterialId {
    r.add_material(Material {
        base_color: [0.0, 0.0, 0.0, 1.0],
        emissive: [rgb[0] * k, rgb[1] * k, rgb[2] * k],
        roughness: 1.0,
        flags: akari::scene::MAT_UNLIT_EMISSIVE,
        ..Default::default()
    })
}

fn build_fx(r: &mut Renderer) -> Result<Fx> {
    let lin = |c: [u8; 3]| c.map(|v| ((v as f32 / 255.0 + 0.055) / 1.055).powf(2.4));
    let mats = [glow(r, [0.12, 1.0, 0.3], 0.07), glow(r, [1.0, 0.08, 0.06], 0.08), glow(r, [1.0, 0.8, 0.4], 0.2), glow(r, [1.0, 0.62, 0.15], 0.04)];
    let (zx, zz) = (ZONE_MM.0 * 0.0005 + 0.003, ZONE_MM.1 * 0.0005 + 0.003);
    let mut meshes: Vec<MeshData> = mats.iter().enumerate().map(|(i, m)| ring(&format!("zone glow {i}"), *m, zx, zz, 0.006, 0.0028)).collect();
    let cm = [glow(r, [1.0, 0.62, 0.15], 0.05), glow(r, [1.0, 0.8, 0.4], 0.16)];
    for (i, m) in cm.iter().enumerate() {
        meshes.push(ring(&format!("card glow {i}"), *m, CARD_W * 0.5 + 0.0025, CARD_H * 0.5 + 0.0025, 0.005, 0.002));
    }
    let mut cols: Vec<[f32; 3]> = tcg::Attribute::ALL.iter().map(|a| lin(a.color())).collect();
    cols.push([1.0, 0.95, 0.85]);
    let base = meshes.len();
    for (i, c) in cols.iter().enumerate() {
        for l in 0..FX_LEVELS {
            let k = (l + 1) as f32 / FX_LEVELS as f32;
            let k = k * k;
            let mw = glow(r, *c, 0.32 * k);
            meshes.push(ring(&format!("hit wave {i} {l}"), mw, 0.5, 0.5, 0.5, 0.022));
            let mf = glow(r, *c, 0.3 * k);
            meshes.push(ring(&format!("hit flash {i} {l}"), mf, ZONE_MM.0 * 0.0005 + 0.006, ZONE_MM.1 * 0.0005 + 0.006, 0.008, 0.005));
            let ms = glow(r, *c, 0.9 * k);
            let mut b = MeshBuilder::new(&format!("hit streak {i} {l}"), ms, 1.0);
            b.aabb(Vec3::new(-0.00035, -0.00035, -0.007), Vec3::new(0.00035, 0.00035, 0.007));
            meshes.push(b.finish());
        }
    }
    let ids = r.add_meshes(&meshes)?;
    let at = |i: usize, l: usize, kind: usize| ids[base + (i * FX_LEVELS + l) * 3 + kind];
    Ok(Fx {
        zone: [ids[0], ids[1], ids[2], ids[3]],
        card: [ids[4], ids[5]],
        wave: std::array::from_fn(|i| std::array::from_fn(|l| at(i, l, 0))),
        flash: std::array::from_fn(|i| std::array::from_fn(|l| at(i, l, 1))),
        streak: std::array::from_fn(|i| std::array::from_fn(|l| at(i, l, 2))),
    })
}

/// Everything static plus the card kits.
pub struct Scene {
    pub statics: Vec<Instance>,
    /// [front, back, edge] per card id.
    pub kits: HashMap<CardId, [MeshId; 3]>,
    /// The same, greyed out (cards in your hand you can't play right now).
    pub dim_kits: HashMap<CardId, [MeshId; 3]>,
    /// A card seen only from the back (face-down cards, decks, the opponent's hand).
    pub back_kit: [MeshId; 3],
    pub lights: Vec<Light>,
    pub fx: Fx,
}

pub fn build(r: &mut Renderer, root: &Path, cards: &[Stats]) -> Result<Scene> {
    let t0 = std::time::Instant::now();
    let mut statics = Vec::new();
    // the shop and its fittings
    let shop = root.join("assets/buildings/shop_tsukumodo");
    let m = gltf_load::load(r, &shop.join("shop_tsukumodo.gltf"), LoadOptions::default()).context("shop")?;
    statics.extend(m.instances(Affine3A::IDENTITY));
    let layout = std::fs::read_to_string(shop.join("layout.json")).context("shop layout")?;
    let mut props: HashMap<String, Model> = HashMap::new();
    for e in parse_layout(&layout) {
        if !props.contains_key(&e.0) {
            let p = root.join("assets/props").join(&e.0).join(format!("{}.gltf", e.0));
            match gltf_load::load(r, &p, LoadOptions::default()) {
                Ok(m) => {
                    props.insert(e.0.clone(), m);
                }
                Err(err) => {
                    eprintln!("prop {}: {err:#}", e.0);
                    continue;
                }
            }
        }
        let at = Vec3::new(e.1[0], e.1[2], -e.1[1]);
        let t = Affine3A::from_rotation_translation(Quat::from_rotation_y(e.2.to_radians()), at);
        statics.extend(props[&e.0].instances(t));
    }

    // the playmat: a 3 mm neoprene slab, printed on top
    let mat_dir = root.join("assets/cards/playmat");
    let mat_mat = layered_material(r, &mat_dir, 0)?;
    let edge_mat = r.add_material(Material::color([0.05, 0.05, 0.06], 0.9));
    let (hw, hd) = (MAT_MM.0 * 0.0005, MAT_MM.1 * 0.0005);
    let o = mat_origin();
    let mut top = MeshData { name: "playmat".into(), material: Some(mat_mat), ..Default::default() };
    for (u, v) in [(0.0f32, 0.0f32), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
        top.positions.push([o.x - hw + u * 2.0 * hw, o.y, o.z - hd + v * 2.0 * hd]);
        top.normals.push([0.0, 1.0, 0.0]);
        top.uvs.push([u, v]);
    }
    top.indices = vec![0, 2, 1, 0, 3, 2];
    top.generate_tangents();
    let mut slab = MeshBuilder::new("playmat edge", edge_mat, 1.0);
    slab.aabb(Vec3::new(o.x - hw, o.y - 0.003, o.z - hd), Vec3::new(o.x + hw, o.y - 0.0002, o.z + hd));

    // card kits: every card in either deck, plus the shared back
    let tdir = root.join("assets/cards/templates");
    let back = layered_material(r, &tdir.join("back"), 0)?;
    let edge = r.add_material(Material::color([0.86, 0.85, 0.82], 0.7));
    let mut meshes = vec![top, slab.finish()];
    let mut order = Vec::new();
    for c in cards {
        if order.contains(&c.id) {
            continue;
        }
        order.push(c.id);
        let dir = tdir.join("cards").join(format!("{}_normal", c.id));
        let fm = layered(r, &dir, 0).with_context(|| format!("card {} (run tools/cardgen --set {})", c.id, c.id.set))?;
        let front = r.add_material(fm);
        let dim = r.add_material(Material { base_color: [0.30, 0.30, 0.32, 1.0], ..fm });
        meshes.extend(card_meshes(front, back, edge));
        meshes.extend(card_meshes(dim, back, edge));
    }
    meshes.extend(card_meshes(back, back, edge));
    let ids = r.add_meshes(&meshes)?;
    statics.push(Instance { mesh: ids[0], transform: Affine3A::IDENTITY });
    statics.push(Instance { mesh: ids[1], transform: Affine3A::IDENTITY });
    let mut kits = HashMap::new();
    let mut dim_kits = HashMap::new();
    for (i, id) in order.iter().enumerate() {
        let b = 2 + i * 6;
        kits.insert(*id, [ids[b], ids[b + 1], ids[b + 2]]);
        dim_kits.insert(*id, [ids[b + 3], ids[b + 4], ids[b + 5]]);
    }
    let b = 2 + order.len() * 6;
    let back_kit = [ids[b], ids[b + 1], ids[b + 2]];

    // light: two fluorescent fittings over the play table, a softer one over the till
    let warm = Vec3::new(1.0, 0.97, 0.92);
    let lights = vec![
        Light { position: TABLE + Vec3::new(-0.35, 1.35, 0.0), radius: 0.25, intensity: warm * 0.16, range: 8.0, spot: None },
        Light { position: TABLE + Vec3::new(0.35, 1.35, 0.0), radius: 0.25, intensity: warm * 0.16, range: 8.0, spot: None },
        Light { position: Vec3::new(0.0, 2.6, -2.2), radius: 0.3, intensity: warm * 0.12, range: 8.0, spot: None },
    ];
    println!("scene built in {:.1}s ({} cards)", t0.elapsed().as_secs_f32(), order.len());
    let fx = build_fx(r)?;
    Ok(Scene { statics, kits, dim_kits, back_kit, lights, fx })
}

/// (id, Blender position, yaw degrees) from the shop's layout.json (a small line parser).
fn parse_layout(text: &str) -> Vec<(String, [f32; 3], f32)> {
    let mut out = Vec::new();
    for chunk in text.split('{').skip(1) {
        let id = chunk.split("\"id\":").nth(1).and_then(|s| s.split('"').nth(1)).map(String::from);
        let at: Vec<f32> = chunk
            .split("\"at\":")
            .nth(1)
            .and_then(|s| s.split('[').nth(1))
            .and_then(|s| s.split(']').next())
            .map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect())
            .unwrap_or_default();
        let yaw = chunk.split("\"yaw\":").nth(1).and_then(|s| s.trim().trim_end_matches(|c| c == '}' || c == ',' || c == ']').split(|c: char| c == ',' || c == '}' || c.is_whitespace()).find(|t| !t.is_empty())).and_then(|v| v.parse().ok()).unwrap_or(0.0);
        if let (Some(id), 3) = (id, at.len()) {
            out.push((id, [at[0], at[1], at[2]], yaw));
        }
    }
    out
}

// ----------------------------------------------------------------------------- card objects

/// Stable identity of a physical card on the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A card in a hand (serial numbers never repeat).
    Hand(u64),
    Unit(u32),
    /// The relic tucked under a unit.
    Relic(u32),
    Trap(usize, usize),
    /// The top of a discard pile.
    DiscardTop(usize),
    /// A deck card (index from the bottom).
    Deck(usize, usize),
    /// A spell on its way from the hand to the discard pile.
    Spell(u64),
    /// The Leader's emblem card.
    Leader(usize),
}

/// Where a card should be.
#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub pos: Vec3,
    pub rot: Quat,
    pub card: Option<CardId>,
    pub side: usize,
}

/// A card that glides towards its place.
#[derive(Clone, Copy, Debug)]
pub struct Obj {
    pub pos: Vec3,
    pub rot: Quat,
    pub card: Option<CardId>,
    pub side: usize,
    /// No longer in the layout: flying to its side's discard pile, then removed.
    pub leaving: bool,
    /// Extra lift / offsets from effects (lunge, hover).
    pub offset: Vec3,
    /// Extra turn from effects (an attacker rearing up), applied over `rot`.
    pub tilt: Quat,
}

/// A hand card on the table: a gentle fan beyond the mat's edge, in front of its player. The
/// hovered card is picked up a little and tipped towards you so it can be read.
pub fn hand_place(side: usize, i: usize, n: usize, hover: bool) -> (Vec3, Quat) {
    let k = i as f32 - (n as f32 - 1.0) * 0.5;
    let spacing = if n > 6 { 66.0 - (n as f32 - 6.0) * 4.0 } else { 68.0 };
    let table_y = TABLE.y + CARD_T * 0.5 + 0.0003 + i as f32 * CARD_T * 1.2;
    let mut p = mat_point(side, k * spacing, 306.0 + k.abs() * 4.0);
    p.y = table_y;
    let fan = Quat::from_rotation_y(-k * 0.035 * if side == 0 { 1.0 } else { -1.0 });
    let mut rot = fan * lying(side, side == 0);
    if hover {
        // tip the card up towards the player, hinged on its near (bottom) edge so no part of it
        // goes below the table, and lift it a finger's width
        let tilt = 0.9f32;
        let half = CARD_H * 0.5;
        let bottom_edge = p + fan * Vec3::new(0.0, 0.0, half);
        p = bottom_edge + fan * Vec3::new(0.0, half * tilt.sin(), -half * tilt.cos()) + Vec3::Y * 0.008;
        rot = fan * Quat::from_rotation_x(tilt) * lying(side, true);
    }
    (p, rot)
}

/// Where every card of `d` belongs. `hand_keys[side]` names the hand cards in order.
pub fn layout(d: &Duel, hand_keys: &[Vec<u64>; 2], hover: Option<usize>, out: &mut HashMap<Key, Place>) {
    out.clear();
    for side in 0..2 {
        let s = &d.sides[side];
        // hand
        let n = s.hand.len();
        for (i, c) in s.hand.iter().enumerate() {
            let Some(&key) = hand_keys[side].get(i) else { continue };
            let (pos, rot) = hand_place(side, i, n, side == 0 && hover == Some(i));
            out.insert(Key::Hand(key), Place { pos, rot, card: if side == 0 { Some(c.id) } else { None }, side });
        }
        // field
        for z in 0..ZONES {
            if let Some(u) = &s.field[z] {
                let tilt = if u.attacked { Quat::from_rotation_y(if side == 0 { -0.32 } else { 0.32 }) } else { Quat::IDENTITY };
                let p = monster_zone(side, z) + Vec3::Y * (CARD_T * 1.5 + 0.0004);
                out.insert(Key::Unit(u.uid), Place { pos: p, rot: tilt * lying(side, true), card: Some(u.card.id), side });
                if let Some(r) = u.relic {
                    let off = if side == 0 { -1.0 } else { 1.0 };
                    let rp = monster_zone(side, z) + Vec3::new(0.0, CARD_T * 0.5 + 0.0002, off * 0.024);
                    out.insert(Key::Relic(u.uid), Place { pos: rp, rot: tilt * lying(side, true), card: Some(r.id), side });
                }
            }
        }
        for t in 0..TRAP_ZONES {
            if s.traps[t].is_some() {
                out.insert(Key::Trap(side, t), Place { pos: trap_zone(side, t) + Vec3::Y * (CARD_T * 0.5 + 0.0002), rot: lying(side, false), card: None, side });
            }
        }
        // each Leader is marked by a card-back emblem in its zone
        out.insert(Key::Leader(side), Place { pos: leader_zone(side) + Vec3::Y * (CARD_T * 0.5 + 0.0002), rot: lying(side, false), card: None, side });
        if let Some(c) = s.discard.last() {
            out.insert(Key::DiscardTop(side), Place { pos: discard_zone(side) + Vec3::Y * (CARD_T * 0.5 + 0.0002 + 0.0003 * s.discard.len().min(20) as f32), rot: lying(side, true), card: Some(c.id), side });
        }
        for k in 0..s.deck.len() {
            out.insert(Key::Deck(side, k), Place { pos: deck_zone(side) + Vec3::Y * (CARD_T * 0.5 + 0.0002 + k as f32 * CARD_T * 1.05), rot: lying(side, false), card: None, side });
        }
    }
}

pub fn spawn_from(place: &Place) -> Obj {
    Obj { pos: place.pos, rot: place.rot, card: place.card, side: place.side, leaving: false, offset: Vec3::ZERO, tilt: Quat::IDENTITY }
}

/// Instances for a card object.
pub fn card_instances(sc: &Scene, card: Option<CardId>, dim: bool, t: Affine3A, out: &mut Vec<Instance>) {
    let kits = if dim { &sc.dim_kits } else { &sc.kits };
    let kit = card.and_then(|c| kits.get(&c)).copied().unwrap_or(sc.back_kit);
    for m in kit {
        out.push(Instance { mesh: m, transform: t });
    }
}
