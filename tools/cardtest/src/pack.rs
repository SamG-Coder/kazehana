//! Booster pack in Akari: the flow-wrap wrapper as a real pillow-shaped pouch, the tear, the reveal.
//!
//! Geometry is built in millimetres in pack space (x right, y up, z out of the front; origin at
//! the pack centre) from assets/cards/packs/<format>/pack.json, then scaled to metres.
//!
//! The wrapper is one closed tube of film (front, both folds, the back with its fin seal) that is
//! pinched flat into the top and bottom crimps. A ragged tear line just under the top crimp splits
//! it into the body and the top strip. Tearing is drag-driven: the strip peels off from the notch
//! you grab, bending around the tear front with a fixed radius, and the front follows the cursor
//! exactly. The strip's shape depends only on how far the tear has run, so it is pre-built as
//! `TEAR_STATES` meshes per direction and swapped per frame (no BLAS rebuilds while tearing).
//! The body has `OPEN_STATES` meshes for the mouth opening as the cards slide out.

use akari::glam::{Affine3A, Quat, Vec3};
use akari::textures::{file_key, TexJob, TexKind, TexSource};
use akari::*;
use anyhow::{Context, Result};
use std::path::Path;

pub const TEAR_STATES: usize = 72;
pub const OPEN_STATES: usize = 10;
const MM: f32 = 0.001;
/// Film thickness used for the inside surface and the cut edges (exaggerated a little).
const FILM: f32 = 0.05;
const CARD_T_MM: f32 = 0.31;

#[derive(Clone, Debug)]
pub struct Spec {
    pub slug: String,
    pub label: String,
    pub cards: usize,
    pub box_packs: u32,
    pub w: f32,
    pub l: f32,
    pub d: f32,
    pub crimp: f32,
    pub fin: f32,
    pub tear_y: f32,
    pub cards_top: f32,
    pub cards_bottom: f32,
    pub net_w: f32,
    pub net_h: f32,
}

fn json_num(text: &str, key: &str) -> Option<f32> {
    let k = format!("\"{key}\":");
    let i = text.find(&k)? + k.len();
    let rest = text[i..].trim_start().trim_start_matches('[');
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn json_str(text: &str, key: &str) -> Option<String> {
    let k = format!("\"{key}\": \"");
    let i = text.find(&k)? + k.len();
    let j = text[i..].find('"')? + i;
    Some(text[i..j].to_string())
}

impl Spec {
    pub fn load(dir: &Path) -> Result<Spec> {
        let t = std::fs::read_to_string(dir.join("pack.json")).with_context(|| format!("{}/pack.json (run tools/cardgen --packs first)", dir.display()))?;
        let n = |k: &str| json_num(&t, k).with_context(|| format!("pack.json: {k}"));
        let net = t.find("\"net_mm\"").map(|i| &t[i..]).unwrap_or("");
        let nums: Vec<f32> = net.split(|c| c == '[' || c == ']').nth(1).unwrap_or("").split(',').filter_map(|s| s.trim().parse().ok()).collect();
        Ok(Spec {
            slug: json_str(&t, "slug").unwrap_or_default(),
            label: json_str(&t, "label").unwrap_or_default(),
            cards: n("cards")? as usize,
            box_packs: n("box_packs")? as u32,
            w: n("width_mm")?,
            l: n("length_mm")?,
            d: n("thickness_mm")?,
            crimp: n("crimp_mm")?,
            fin: n("fin_mm")?,
            tear_y: n("tear_y_mm")?,
            cards_top: n("cards_top_mm")?,
            cards_bottom: n("cards_bottom_mm")?,
            net_w: *nums.first().context("net_mm")?,
            net_h: *nums.get(1).context("net_mm")?,
        })
    }
    pub fn stack(&self) -> f32 {
        self.cards as f32 * CARD_T_MM + 0.1
    }
    /// Pack-space y (mm) of a distance `v` from the top edge.
    pub fn y(&self, v: f32) -> f32 {
        self.l * 0.5 - v
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn hash(i: i32, seed: u32) -> f32 {
    let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ seed.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x & 0xFFFF) as f32 / 65535.0
}
fn noise1(x: f32, seed: u32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let f = f * f * (3.0 - 2.0 * f);
    mix(hash(i as i32, seed), hash(i as i32 + 1, seed), f)
}

/// The ragged tear: distance from the top edge (mm) where the film parts at pack x.
pub fn tear_v(s: &Spec, x: f32) -> f32 {
    s.tear_y + (noise1(x * 0.45 + 3.0, 11) - 0.5) * 0.7 + (noise1(x * 2.3, 12) - 0.5) * 0.22 + (noise1(x * 9.0, 13) - 0.5) * 0.08
}

/// Net position -> (pack x, side: +1 front, -1 back).
fn net_to_x(s: &Spec, n: f32) -> (f32, f32) {
    let (w, f) = (s.w, s.fin);
    if n < f + 0.5 * w {
        (-(n - f), -1.0)
    } else if n < f + 1.5 * w {
        (n - f - w, 1.0)
    } else {
        (2.0 * w + f - n, -1.0)
    }
}

/// Half-thickness of the pouch at depth `v` (mm), with the mouth opened by `open` (0..1).
fn half_thick(s: &Spec, v: f32, open: f32) -> (f32, f32) {
    let c = s.crimp;
    let mut e = smooth(c - 0.2, c + 7.0, v) * (1.0 - smooth(s.l - c - 6.0, s.l - c + 0.2, v));
    // the headroom above the cards puffs less than the part with the cards in it
    e *= mix(0.55, 1.0, smooth(s.cards_top - 9.0, s.cards_top + 2.0, v));
    let mut h = (s.d * 0.5 * e).max(FILM * 0.6);
    let mut mouth = 0.0;
    if open > 0.0 {
        mouth = open * (1.0 - smooth(s.tear_y, s.cards_top + 16.0, v));
        h = h.max(open * (s.stack() * 0.5 + 2.6) * (1.0 - smooth(s.tear_y, s.cards_top + 16.0, v)));
    }
    (h, mouth)
}

/// A point on the outside of the wrapper (mm, pack space).
fn tube_point(s: &Spec, n: f32, v: f32, open: f32) -> Vec3 {
    let (x, side) = net_to_x(s, n);
    let hw = s.w * 0.5;
    let (h, mouth) = half_thick(s, v, open);
    let p = mix(6.0, 2.4, mouth);
    let prof = (1.0 - (x.abs() / hw).min(1.0).powf(p)).max(0.0).sqrt();
    // film puckers into soft wrinkles where it is pinched toward the crimps
    let (c, l) = (s.crimp, s.l);
    let pinch = smooth(c, c + 3.0, v) * (1.0 - smooth(c + 4.0, c + 13.0, v)) + smooth(l - c - 11.0, l - c - 3.0, v) * (1.0 - smooth(l - c - 2.0, l - c, v));
    let ph = noise1(x * 0.35, if side > 0.0 { 21 } else { 22 }) * 6.0;
    let wr = (x * std::f32::consts::TAU / 3.4 + ph).sin() * 0.09 * pinch * (1.0 - mouth);
    Vec3::new(x, s.y(v), side * (h * prof + wr * prof))
}

/// Column positions across the net (dense toward the folds).
fn columns(s: &Spec, per_quarter: usize) -> Vec<f32> {
    let (w, f) = (s.w, s.fin);
    let q = [f, f + 0.5 * w, f + w, f + 1.5 * w, f + 2.0 * w];
    let mut out = Vec::new();
    for k in 0..4 {
        let fold_at_end = k == 0 || k == 2;
        for i in 0..per_quarter {
            let t = i as f32 / per_quarter as f32;
            let tt = if fold_at_end { mix(t, (t * std::f32::consts::FRAC_PI_2).sin(), 0.75) } else { mix(t, 1.0 - (t * std::f32::consts::FRAC_PI_2).cos(), 0.75) };
            out.push(q[k] + (q[k + 1] - q[k]) * tt);
        }
    }
    out.push(q[4]);
    out
}

/// A grid of rest points (mm) with net UVs.
#[derive(Clone)]
struct Grid {
    cols: usize,
    rows: usize,
    pos: Vec<Vec3>,
    uv: Vec<[f32; 2]>,
}

/// Which part of the tube: the body (tear line down) or the strip (top edge to the tear line).
#[derive(Clone, Copy, PartialEq)]
enum Part {
    Body,
    Strip,
}

fn v_range(s: &Spec, part: Part, x: f32) -> (f32, f32) {
    match part {
        Part::Body => (tear_v(s, x), s.l),
        Part::Strip => (0.0, tear_v(s, x)),
    }
}

fn tube_grid(s: &Spec, part: Part, open: f32) -> Grid {
    let cols = columns(s, 44);
    let rows = match part {
        Part::Body => 150,
        Part::Strip => 22,
    };
    let mut g = Grid { cols: cols.len(), rows, pos: Vec::new(), uv: Vec::new() };
    for r in 0..rows {
        for &n in &cols {
            let (x, _) = net_to_x(s, n);
            let (v0, v1) = v_range(s, part, x);
            let t = r as f32 / (rows - 1) as f32;
            // body rows: denser near the mouth and the bottom crimp
            let t = if part == Part::Body { mix(t, 0.5 - 0.5 * (t * std::f32::consts::PI).cos(), 0.45) } else { t };
            let v = mix(v0, v1, t);
            g.pos.push(tube_point(s, n, v, open));
            g.uv.push([n / s.net_w, v / s.net_h]);
        }
    }
    g
}

/// The fin seal, folded flat over the back (pack x in 0..fin).
fn fin_grid(s: &Spec, part: Part, open: f32) -> Grid {
    let cols = 9;
    let rows = match part {
        Part::Body => 90,
        Part::Strip => 14,
    };
    let mut g = Grid { cols, rows, pos: Vec::new(), uv: Vec::new() };
    for r in 0..rows {
        for c in 0..cols {
            let x = s.fin * c as f32 / (cols - 1) as f32;
            let (v0, v1) = v_range(s, part, x);
            let v = mix(v0, v1, r as f32 / (rows - 1) as f32);
            let n_back = 2.0 * s.w + s.fin - x; // the back point under the fin
            let mut p = tube_point(s, n_back, v, open);
            // the free edge of the fin lifts a hair off the film
            p.z -= FILM * 1.4 + 0.04 * (c as f32 / (cols - 1) as f32);
            g.pos.push(p);
            g.uv.push([(s.fin + 2.0 * s.w + x) / s.net_w, v / s.net_h]);
        }
    }
    g
}

fn grid_indices(g: &Grid, flip: bool) -> Vec<u32> {
    let mut idx = Vec::with_capacity((g.rows - 1) * (g.cols - 1) * 6);
    for r in 0..g.rows - 1 {
        for c in 0..g.cols - 1 {
            let a = (r * g.cols + c) as u32;
            let (b, cc, d) = (a + 1, a + g.cols as u32, a + g.cols as u32 + 1);
            if flip {
                idx.extend([a, b, cc, b, d, cc]);
            } else {
                idx.extend([a, cc, b, b, cc, d]);
            }
        }
    }
    idx
}

/// Outward vertex normals of a grid (mm space), oriented so `probe` (a vertex index) points along `want`.
fn grid_normals(g: &Grid, probe: usize, want: Vec3) -> (Vec<Vec3>, bool) {
    let mut m = MeshData { positions: g.pos.iter().map(|p| p.to_array()).collect(), indices: grid_indices(g, false), ..Default::default() };
    m.generate_normals();
    let flip = Vec3::from(m.normals[probe]).dot(want) < 0.0;
    let s = if flip { -1.0 } else { 1.0 };
    (m.normals.iter().map(|n| Vec3::from(*n) * s).collect(), flip)
}

fn mesh_from(name: &str, mat: MaterialId, pos: Vec<Vec3>, uv: &[[f32; 2]], g: &Grid, flip: bool) -> MeshData {
    let mut m = MeshData {
        name: name.into(),
        material: Some(mat),
        positions: pos.iter().map(|p| (*p * MM).to_array()).collect(),
        uvs: uv.to_vec(),
        indices: grid_indices(g, flip),
        ..Default::default()
    };
    m.generate_normals();
    m.generate_tangents();
    m
}

/// Rest geometry of one part: outside, inside (offset inward) and the cut edge joining them.
struct PartRest {
    outer: Grid,
    inner: Vec<Vec3>,
    flip: bool,
    fin: Grid,
    fin_flip: bool,
    /// Cut edge along the tear: (outer, inner) per column.
    edge: Vec<(Vec3, Vec3)>,
}

fn part_rest(s: &Spec, part: Part, open: f32) -> PartRest {
    let outer = tube_grid(s, part, open);
    // probe: the front centre column, middle row
    let cols = outer.cols;
    let centre_col = (0..cols).min_by(|&a, &b| {
        let fa = outer.pos[a];
        let fb = outer.pos[b];
        let ka = if fa.z > 0.0 { fa.x.abs() } else { 1e9 };
        let kb = if fb.z > 0.0 { fb.x.abs() } else { 1e9 };
        ka.partial_cmp(&kb).unwrap()
    });
    let probe = (outer.rows / 2) * cols + centre_col.unwrap_or(cols / 2);
    let (nrm, flip) = grid_normals(&outer, probe, Vec3::Z);
    // inside surface: one film thickness in, but never through the other layer where the two
    // are pressed together (crimps)
    let inner: Vec<Vec3> = outer
        .pos
        .iter()
        .zip(&nrm)
        .zip(&outer.uv)
        .map(|((p, n), uv)| *p - *n * FILM.min(half_thick(s, uv[1] * s.net_h, open).0 * 0.7))
        .collect();
    let fin = fin_grid(s, part, open);
    let (_, fin_flip) = grid_normals(&fin, (fin.rows / 2) * fin.cols + fin.cols / 2, -Vec3::Z);
    let edge_row = match part {
        Part::Body => 0,
        Part::Strip => outer.rows - 1,
    };
    let edge = (0..cols).map(|c| (outer.pos[edge_row * cols + c], inner[edge_row * cols + c])).collect();
    PartRest { outer, inner, flip, fin, fin_flip, edge }
}

pub struct Mats {
    pub wrapper: MaterialId,
    pub inside: MaterialId,
    pub cut: MaterialId,
}

/// Meshes of one part in one pose: [outside, inside, fin, cut edge].
fn part_meshes(r: &PartRest, m: &Mats, deform: &dyn Fn(Vec3) -> Vec3, tag: &str) -> Vec<MeshData> {
    let g = &r.outer;
    let outer = mesh_from(&format!("pack {tag} outside"), m.wrapper, g.pos.iter().map(|p| deform(*p)).collect(), &g.uv, g, r.flip);
    let inner = mesh_from(&format!("pack {tag} inside"), m.inside, r.inner.iter().map(|p| deform(*p)).collect(), &g.uv, g, !r.flip);
    let fin = mesh_from(&format!("pack {tag} fin"), m.wrapper, r.fin.pos.iter().map(|p| deform(*p)).collect(), &r.fin.uv, &r.fin, r.fin_flip);
    let eg = Grid { cols: r.edge.len(), rows: 2, pos: Vec::new(), uv: vec![[0.0, 0.0]; r.edge.len() * 2] };
    let mut ep: Vec<Vec3> = r.edge.iter().map(|e| deform(e.0)).collect();
    ep.extend(r.edge.iter().map(|e| deform(e.1)));
    let cut = mesh_from(&format!("pack {tag} cut"), m.cut, ep, &eg.uv, &eg, false);
    vec![outer, inner, fin, cut]
}

/// Peel the strip: points behind the tear front bend up around it (radius `R`) and lift.
/// `p` = tear progress 0..1, `dir` = +1 tearing from the left notch, -1 from the right.
pub fn peel(s: &Spec, pt: Vec3, p: f32, dir: f32) -> Vec3 {
    let hw = s.w * 0.5;
    let xf = -dir * hw + dir * p * (s.w + 2.0);
    let d = dir * (xf - pt.x);
    if d <= 0.0 {
        return pt;
    }
    const R: f32 = 3.0;
    const TH: f32 = 2.2;
    let (phi, ax, az) = if d < R * TH {
        let phi = d / R;
        (phi, R * phi.sin(), R * (1.0 - phi.cos()))
    } else {
        let rem = d - R * TH;
        (TH, R * TH.sin() + rem * TH.cos(), R * (1.0 - TH.cos()) + rem * TH.sin())
    };
    let (nx, nz) = (dir * phi.sin(), phi.cos());
    // The hand pulls the free end up as well as back: the flap rises along its length, and the
    // band's top edge (further from the tear) leads a little, as a held strip twists.
    let lift = 0.34 * d * smooth(0.0, 20.0, d);
    let lead = (pt.y - s.y(tear_v(s, pt.x))) * 0.12 * smooth(4.0, 30.0, d);
    Vec3::new(xf - dir * ax + pt.z * nx, pt.y + lift, az + pt.z * nz + lead)
}

pub struct PackMeshes {
    /// [open state][outside, inside, fin, cut]
    pub body: Vec<Vec<MeshId>>,
    /// [dir 0 = from left, 1 = from right][tear state][outside, inside, fin, cut]
    pub strip: [Vec<Vec<MeshId>>; 2],
}

pub fn wrapper_mats(r: &mut Renderer, dir: &Path) -> Result<Mats> {
    let f = |n: &str| dir.join(n);
    let jobs = vec![
        TexJob { key: file_key(&f("base.png")), name: format!("{} base", dir.display()), kind: TexKind::Color, source: TexSource::File(f("base.png")) },
        TexJob { key: file_key(&f("normal.png")), name: format!("{} normal", dir.display()), kind: TexKind::Normal, source: TexSource::File(f("normal.png")) },
        TexJob { key: file_key(&f("orm.png")), name: format!("{} orm", dir.display()), kind: TexKind::Data, source: TexSource::File(f("orm.png")) },
    ];
    let t = r.add_textures(&jobs)?;
    let wrapper = r.add_material(Material {
        base_color: [1.0; 4],
        roughness: 1.0,
        metallic: 1.0,
        base_tex: Some(t[0]),
        normal_tex: Some(t[1]),
        mr_tex: Some(t[2]),
        occlusion_tex: Some(t[2]),
        flags: akari::scene::MAT_DOUBLE_SIDED,
        ..Default::default()
    });
    // Inside of metallised film: a soft silver mirror. The cut edge shows the white stressed core.
    let inside = r.add_material(Material { base_color: [0.82, 0.82, 0.84, 1.0], roughness: 0.42, metallic: 0.85, flags: akari::scene::MAT_DOUBLE_SIDED, ..Default::default() });
    let cut = r.add_material(Material { base_color: [0.86, 0.86, 0.88, 1.0], roughness: 0.55, metallic: 0.2, flags: akari::scene::MAT_DOUBLE_SIDED, ..Default::default() });
    Ok(Mats { wrapper, inside, cut })
}

pub fn build_meshes(r: &mut Renderer, s: &Spec, m: &Mats) -> Result<PackMeshes> {
    let t0 = std::time::Instant::now();
    let mut body = Vec::new();
    for k in 0..OPEN_STATES {
        let open = k as f32 / (OPEN_STATES - 1) as f32;
        let rest = part_rest(s, Part::Body, open);
        let ms = part_meshes(&rest, m, &|p| p, "body");
        body.push(r.add_meshes(&ms)?);
    }
    let rest = part_rest(s, Part::Strip, 0.0);
    let mut strip: [Vec<Vec<MeshId>>; 2] = [Vec::new(), Vec::new()];
    for (di, dir) in [1.0f32, -1.0].into_iter().enumerate() {
        let mut all = Vec::new();
        for k in 0..TEAR_STATES {
            let p = k as f32 / (TEAR_STATES - 1) as f32;
            all.extend(part_meshes(&rest, m, &|q| peel(s, q, p, dir), "strip"));
        }
        let ids = r.add_meshes(&all)?;
        strip[di] = ids.chunks(4).map(|c| c.to_vec()).collect();
    }
    println!("pack {}: {} body + {} strip states built in {:.2}s", s.slug, OPEN_STATES, 2 * TEAR_STATES, t0.elapsed().as_secs_f32());
    Ok(PackMeshes { body, strip })
}

// ----------------------------------------------------------------------------- contents

/// One card in the pack: the template kit to show it with, and whether it is the foil.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub kit: usize,
    pub rarity: tcg::Rarity,
    pub foil: bool,
}

/// Roll a pack with crates/tcg `open_pack` (commons first, the rare last). Formats with a
/// different count add or drop commons (a proposal: crates/tcg only defines the 6-card pack).
pub fn roll(db: &tcg::CardDb, cards: usize, seed: u64, kit_of: &dyn Fn(tcg::Rarity, bool) -> usize) -> Vec<Slot> {
    let mut rng = tcg::Rng::new(seed);
    let pulls = tcg::open_pack(db, 1, &mut rng);
    let mut v: Vec<(tcg::Rarity, bool)> = pulls.iter().map(|p| (db.get(p.id).rarity, p.foil)).collect();
    while v.len() < cards {
        v.insert(1, (tcg::Rarity::Common, false));
    }
    while v.len() > cards && v.len() > 1 {
        v.remove(1);
    }
    v.into_iter().map(|(r, f)| Slot { kit: kit_of(r, f), rarity: r, foil: f }).collect()
}

// ----------------------------------------------------------------------------- interaction

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Untouched, or torn part way (progress > 0).
    Sealed,
    /// The strip has come off and flies away; the mouth opens; the cards slide up.
    Opening,
    /// Cards stand out of the pack: click to draw them.
    Ready,
    /// The stack is out in front: each click deals the top card off to the side.
    Reveal,
    /// Every card is fanned out.
    Done,
}

pub struct PackRig {
    pub spec: Spec,
    pub meshes: PackMeshes,
    pub contents: Vec<Slot>,
    pub phase: Phase,
    /// Tear progress 0..1 (never goes back) and its target from the cursor.
    pub progress: f32,
    pub target: f32,
    /// +1 from the left notch, -1 from the right; 0 before the first grab.
    pub dir: f32,
    pub tearing: bool,
    /// Seconds in the current phase.
    pub t: f32,
    /// Reveal: cards dealt so far; per-card animation clock.
    pub dealt: usize,
    pub deal_t: Vec<f32>,
    /// Small roll of the pack in the hand while tearing.
    pub roll: f32,
    roll_v: f32,
    last_progress: f32,
    pub seed: u64,
}

pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

impl PackRig {
    pub fn new(spec: Spec, meshes: PackMeshes, contents: Vec<Slot>, seed: u64) -> PackRig {
        let n = contents.len();
        PackRig { spec, meshes, contents, phase: Phase::Sealed, progress: 0.0, target: 0.0, dir: 0.0, tearing: false, t: 0.0, dealt: 0, deal_t: vec![0.0; n], roll: 0.0, roll_v: 0.0, last_progress: 0.0, seed }
    }

    pub fn reset(&mut self, contents: Vec<Slot>, seed: u64) {
        let n = contents.len();
        self.contents = contents;
        self.phase = Phase::Sealed;
        self.progress = 0.0;
        self.target = 0.0;
        self.dir = 0.0;
        self.tearing = false;
        self.t = 0.0;
        self.dealt = 0;
        self.deal_t = vec![0.0; n];
        self.roll = 0.0;
        self.roll_v = 0.0;
        self.last_progress = 0.0;
        self.seed = seed;
    }

    /// Can a drag that starts at pack-space point `p` (mm) tear? Sets the direction.
    pub fn try_grab(&mut self, p: Vec3) -> bool {
        if self.phase != Phase::Sealed {
            return false;
        }
        let s = &self.spec;
        let v = s.l * 0.5 - p.y;
        let near_line = (v - s.tear_y).abs() < 9.0 && p.x.abs() < s.w * 0.5 + 8.0;
        if self.dir == 0.0 {
            // a fresh pack: grab at either notch
            if near_line && p.x.abs() > s.w * 0.5 - 12.0 {
                self.dir = if p.x < 0.0 { 1.0 } else { -1.0 };
                self.tearing = true;
                return true;
            }
            return false;
        }
        // part torn: grab the flap or the tear front again
        if v < s.tear_y + 12.0 && p.x.abs() < s.w * 0.5 + 40.0 && p.y < s.l * 0.5 + 60.0 {
            self.tearing = true;
            return true;
        }
        false
    }

    /// Drag position in pack space (mm): where the tear front should be.
    pub fn drag(&mut self, p: Vec3) {
        if !self.tearing || self.phase != Phase::Sealed {
            return;
        }
        let s = &self.spec;
        // The front follows the hand: progress from the notch to the cursor, across the width.
        let t = (self.dir * p.x + s.w * 0.5) / (s.w + 2.0);
        self.target = self.target.max(t.clamp(0.0, 1.0));
    }

    pub fn release(&mut self) {
        self.tearing = false;
    }

    /// Scripted tear for tests: set the target directly.
    pub fn script_tear(&mut self, dir: f32, target: f32) {
        if self.dir == 0.0 {
            self.dir = dir;
        }
        self.tearing = true;
        self.target = self.target.max(target);
    }

    /// Advance the deal (click / Space).
    pub fn click(&mut self) {
        match self.phase {
            Phase::Ready => {
                self.phase = Phase::Reveal;
                self.t = 0.0;
            }
            Phase::Reveal if self.t > 0.45 => {
                if self.dealt + 1 < self.contents.len() {
                    self.deal_t[self.dealt] = 0.0;
                    self.dealt += 1;
                } else {
                    self.phase = Phase::Done;
                    self.t = 0.0;
                }
            }
            _ => {}
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.t += dt;
        for t in self.deal_t.iter_mut() {
            *t += dt;
        }
        if self.phase == Phase::Sealed {
            // Critically damped follow: smooth at any frame rate, exact once the hand stops.
            let k = 1.0 - (-dt * 28.0).exp();
            let step = ((self.target - self.progress) * k).min(dt * 3.5);
            self.progress = (self.progress + step.max(0.0)).min(1.0);
            if self.progress >= 0.999 {
                self.progress = 1.0;
                self.phase = Phase::Opening;
                self.t = 0.0;
                self.tearing = false;
            }
        }
        // Tension: the pack rolls a little against the pull, springs back when the tear stops.
        let speed = (self.progress - self.last_progress) / dt.max(1e-4);
        self.last_progress = self.progress;
        let want = if self.phase == Phase::Sealed { -self.dir * (speed * 0.035).min(0.06) } else { 0.0 };
        let acc = (want - self.roll) * 140.0 - self.roll_v * 18.0;
        self.roll_v += acc * dt;
        self.roll += self.roll_v * dt;
        if self.phase == Phase::Opening && self.t > 1.25 {
            self.phase = Phase::Ready;
            self.t = 0.0;
        }
    }

    pub fn tear_state(&self) -> usize {
        ((self.progress * (TEAR_STATES - 1) as f32).round() as usize).min(TEAR_STATES - 1)
    }

    /// Mouth opening 0..1.
    fn open(&self) -> f32 {
        match self.phase {
            Phase::Sealed => 0.0,
            Phase::Opening => ease((self.t - 0.15) / 0.55),
            _ => 1.0,
        }
    }

    /// How far the cards have slid up out of the pack (mm).
    fn rise(&self) -> f32 {
        let s = &self.spec;
        let full = (s.cards_top - s.tear_y) + 30.0;
        match self.phase {
            Phase::Sealed => 0.0,
            Phase::Opening => full * ease_out((self.t - 0.45) / 0.8),
            _ => full,
        }
    }

    /// Instances for this frame. `pose` places the pack in the world.
    pub fn instances(&self, pose: Affine3A, kits: &[[MeshId; 3]], out: &mut Vec<Instance>) {
        let s = &self.spec;
        let pose = pose * Affine3A::from_rotation_z(self.roll);
        let scale = Affine3A::from_scale(Vec3::splat(1.0));
        let open_k = ((self.open() * (OPEN_STATES - 1) as f32).round() as usize).min(OPEN_STATES - 1);
        for &m in &self.meshes.body[open_k] {
            out.push(Instance { mesh: m, transform: pose * scale });
        }
        // the strip: attached / peeling, then flying off
        let di = if self.dir < 0.0 { 1 } else { 0 };
        let strip_pose = match self.phase {
            Phase::Sealed => Some(Affine3A::IDENTITY),
            Phase::Opening if self.t < 0.9 => {
                let k = ease(self.t / 0.9);
                let k2 = k * k;
                let dir = if self.dir == 0.0 { 1.0 } else { self.dir };
                let tr = Vec3::new(dir * (40.0 * k + 120.0 * k2), 30.0 * k + 60.0 * k2, 25.0 * k) * MM;
                let pivot = Vec3::new(dir * s.w * 0.5, s.y(s.tear_y), 0.0) * MM;
                Some(Affine3A::from_translation(tr + pivot) * Affine3A::from_rotation_z(-dir * 1.1 * k) * Affine3A::from_rotation_x(-0.6 * k) * Affine3A::from_translation(-pivot))
            }
            _ => None,
        };
        if let Some(sp) = strip_pose {
            for &m in &self.meshes.strip[di][self.tear_state()] {
                out.push(Instance { mesh: m, transform: pose * sp });
            }
        }
        // the cards
        let n = self.contents.len();
        let stack_mid_v = (s.cards_top + s.cards_bottom) * 0.5;
        let rise = self.rise();
        for (i, slot) in self.contents.iter().enumerate() {
            let kit = kits[slot.kit.min(kits.len() - 1)];
            let t = self.card_transform(i, n, stack_mid_v, rise);
            for m in kit {
                out.push(Instance { mesh: m, transform: pose * t });
            }
        }
    }

    /// Pack-space transform of card `i` (0 = the first one you see, the rare last).
    fn card_transform(&self, i: usize, n: usize, stack_mid_v: f32, rise: f32) -> Affine3A {
        let s = &self.spec;
        // In the pack: backs toward the front of the pack, the first card nearest the back.
        let zi = (i as f32 - (n as f32 - 1.0) * 0.5) * -CARD_T_MM * 1.02;
        let in_pack_pos = Vec3::new(0.0, s.y(stack_mid_v) + rise, zi);
        let face_back = Quat::from_rotation_y(std::f32::consts::PI);
        let in_pack = (in_pack_pos, face_back);
        // Reveal stack: in front of the pack, face up, the first card on top (nearest the camera).
        let stack_pos = Vec3::new(0.0, 4.0, 46.0 - i as f32 * CARD_T_MM * 1.6);
        let stack = (stack_pos, Quat::IDENTITY);
        // Dealt pile: off to the right, slightly fanned, newest on top.
        let pile = |k: usize| (Vec3::new(64.0 + k as f32 * 1.4, 2.0 - k as f32 * 1.2, 40.0 + k as f32 * CARD_T_MM * 1.6), Quat::from_rotation_z(-0.06 - k as f32 * 0.035));
        // Final fan across the view, the rare in the middle and forward.
        let fan = |k: usize| {
            let c = (n as f32 - 1.0) * 0.5;
            let a = (k as f32 - c) / c.max(1.0);
            let rare = k + 1 == n;
            let x = a * 58.0;
            (Vec3::new(if rare { 0.0 } else { x * 1.05 }, if rare { 8.0 } else { -10.0 - a.abs() * 6.0 }, if rare { 62.0 } else { 40.0 + k as f32 * 0.4 }), Quat::from_rotation_z(if rare { 0.0 } else { -a * 0.22 }))
        };
        let lerp = |a: (Vec3, Quat), b: (Vec3, Quat), t: f32, arc: f32| {
            let t = t.clamp(0.0, 1.0);
            let lift = Vec3::new(0.0, 0.0, arc * (t * std::f32::consts::PI).sin());
            (a.0.lerp(b.0, t) + lift, a.1.slerp(b.1, t))
        };
        let (p, q) = match self.phase {
            Phase::Sealed | Phase::Opening | Phase::Ready => {
                // a slight fan as they stand up out of the mouth
                let spread = if self.phase == Phase::Ready { ease(self.t / 0.4) } else { 0.0 };
                let k = i as f32 - (n as f32 - 1.0) * 0.5;
                (in_pack.0 + Vec3::new(k * 0.15 * spread, k * 1.1 * spread, 0.0), in_pack.1)
            }
            Phase::Reveal => {
                let pull = ease_out(self.t / 0.6);
                // stagger: the stack comes out together, flipping over toward the camera
                let base = lerp(in_pack, stack, pull, 14.0);
                if i < self.dealt {
                    // dealt cards slide to the pile
                    let k = i;
                    let dt = self.deal_t[k];
                    lerp(stack, pile(k), ease_out(dt / 0.32), 6.0)
                } else {
                    base
                }
            }
            Phase::Done => {
                let from = if i + 1 == n { stack } else { pile(i) };
                let delay = i as f32 * 0.05;
                lerp(from, fan(i), ease_out((self.t - delay) / 0.7), 8.0)
            }
        };
        Affine3A::from_rotation_translation(q, p * MM)
    }

    /// Pack-space notch positions (mm), for the on-screen hint.
    pub fn notches(&self) -> [Vec3; 2] {
        let s = &self.spec;
        let y = s.y(s.tear_y);
        [Vec3::new(-s.w * 0.5, y, 0.0), Vec3::new(s.w * 0.5, y, 0.0)]
    }

    /// The tear front (mm, pack space).
    pub fn front(&self) -> Vec3 {
        let s = &self.spec;
        let xf = -self.dir * s.w * 0.5 + self.dir * self.progress * (s.w + 2.0);
        Vec3::new(xf, s.y(tear_v(s, xf)), 0.0)
    }

    pub fn revealed(&self) -> Option<Slot> {
        match self.phase {
            Phase::Reveal => self.contents.get(self.dealt).copied(),
            Phase::Done => self.contents.last().copied(),
            _ => None,
        }
    }
}

pub const PACK_MM: f32 = MM;
