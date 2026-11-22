//! Shop stock as quality-gated props (docs/ASSET_QUALITY.md): `tools/cardgen --props`.
//!
//!   booster6_pack     one sealed booster6 pack (66 x 128 x 3 mm), the real print and metallised film
//!   booster6_box      the 20-pack counter display box: open top, sloped sides, a printed header,
//!                     20 packs standing inside
//!   single_stand_*    a single card in a top-loader on an acrylic easel (for the showcases)
//!
//! -> assets/props/<id>/<id>.gltf (+ .bin, textures/). Frame: glTF y up, origin at the bottom
//! centre, front facing -Z (Blender +Y), like tools/blender/build_shop_props.py. Everything is
//! built facing +Z and turned round at the end.

use crate::canvas::{write_rgb, Mask};
use crate::gltfw::{write, Mat, Prim};
use crate::math::*;
use crate::pack::{Format, FORMATS};
use crate::sdf;
use crate::text::{self, Fonts};
use rayon::prelude::*;
use std::path::Path;

const MM: f32 = 0.001;

// ----------------------------------------------------------------------------- images

fn read_rgb8(path: &Path) -> (usize, usize, Vec<[u8; 3]>) {
    let f = std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut dec = png::Decoder::new(std::io::BufReader::new(f));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).unwrap();
    let ch = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        _ => 1,
    };
    let (w, h) = (info.width as usize, info.height as usize);
    let px = (0..w * h).map(|i| if ch >= 3 { [buf[i * ch], buf[i * ch + 1], buf[i * ch + 2]] } else { [buf[i * ch]; 3] }).collect();
    (w, h, px)
}

/// Area-averaged resize (good for both shrinking and stretching).
fn resize(src: &[[u8; 3]], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<[u8; 3]> {
    (0..dw * dh)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % dw, i / dw);
            let (fx0, fx1) = (x as f32 * sw as f32 / dw as f32, (x + 1) as f32 * sw as f32 / dw as f32);
            let (fy0, fy1) = (y as f32 * sh as f32 / dh as f32, (y + 1) as f32 * sh as f32 / dh as f32);
            let (x0, x1) = (fx0.floor() as usize, (fx1.ceil() as usize).max(fx0.floor() as usize + 1).min(sw));
            let (y0, y1) = (fy0.floor() as usize, (fy1.ceil() as usize).max(fy0.floor() as usize + 1).min(sh));
            let mut acc = [0.0f32; 3];
            let mut n = 0.0;
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let p = src[yy * sw + xx];
                    for k in 0..3 {
                        acc[k] += p[k] as f32;
                    }
                    n += 1.0;
                }
            }
            [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8]
        })
        .collect()
}

fn save(path: &Path, w: usize, h: usize, px: &[[u8; 3]]) {
    let flat: Vec<u8> = px.iter().flatten().copied().collect();
    write_rgb(path, w, h, &flat);
}

/// Copy a layer set (base / normal / orm) resized to w x h into `tex` as `<prefix>_<layer>.png`.
fn layers(src: &Path, tex: &Path, prefix: &str, w: usize, h: usize) {
    for l in ["base", "normal", "orm"] {
        let (sw, sh, px) = read_rgb8(&src.join(format!("{l}.png")));
        save(&tex.join(format!("{prefix}_{l}.png")), w, h, &resize(&px, sw, sh, w, h));
    }
}

/// Plain 1024 textures for glass and card edges.
fn plain(tex: &Path) {
    const N: usize = 2048;
    save(&tex.join("plain_white_base.png"), N, N, &vec![[232, 232, 228]; N * N]);
    save(&tex.join("plain_flat_normal.png"), N, N, &vec![[128, 128, 255]; N * N]);
    save(&tex.join("plain_edge_orm.png"), N, N, &vec![[255, 170, 0]; N * N]);
    // clear plastic: smooth, not metal
    save(&tex.join("plain_glass_orm.png"), N, N, &vec![[255, 13, 0]; N * N]);
}

fn mat(name: &str, prefix: &str, normal_scale: f32) -> Mat {
    Mat { name: name.into(), base: format!("{prefix}_base.png"), normal: Some(format!("{prefix}_normal.png")), orm: Some(format!("{prefix}_orm.png")), normal_scale, double_sided: false }
}

fn glass_mat(name: &str) -> Mat {
    // "*clear_glass*" materials are thin clear glass in Akari (gltf_load)
    Mat { name: name.into(), base: "plain_white_base.png".into(), normal: Some("plain_flat_normal.png".into()), orm: Some("plain_glass_orm.png".into()), normal_scale: 1.0, double_sided: true }
}

/// Turn a prim built facing +Z (mm) into the prop frame (m, facing -Z).
fn finish(p: &mut Prim) {
    for q in p.pos.iter_mut() {
        *q = [-q[0] * MM, q[1] * MM, -q[2] * MM];
    }
    for n in p.nrm.iter_mut() {
        *n = [-n[0], n[1], -n[2]];
    }
}

/// A triangle grid prim from rows x cols of points.
fn grid(cols: usize, rows: usize, pos: Vec<[f32; 3]>, uv: Vec<[f32; 2]>, flip: bool) -> Prim {
    let mut idx = Vec::new();
    for r in 0..rows - 1 {
        for c in 0..cols - 1 {
            let a = (r * cols + c) as u32;
            let (b, cc, d) = (a + 1, a + cols as u32, a + cols as u32 + 1);
            if flip { idx.extend([a, b, cc, b, d, cc]) } else { idx.extend([a, cc, b, b, cc, d]) }
        }
    }
    let mut p = Prim { pos, nrm: Vec::new(), uv, idx };
    p.smooth_normals();
    p
}

// ----------------------------------------------------------------------------- the pack

fn half_thick(f: &Format, v: f32) -> f32 {
    let c = f.crimp;
    let (cards_top, _) = f.cards_y();
    let mut e = smoothstep(c - 0.2, c + 7.0, v) * (1.0 - smoothstep(f.l - c - 6.0, f.l - c + 0.2, v));
    e *= mix(0.55, 1.0, smoothstep(cards_top - 9.0, cards_top + 2.0, v));
    (f.d * 0.5 * e).max(0.03)
}

fn net_to_x(f: &Format, n: f32) -> (f32, f32) {
    let (w, fin) = (f.w, f.fin);
    if n < fin + 0.5 * w {
        (-(n - fin), -1.0)
    } else if n < fin + 1.5 * w {
        (n - fin - w, 1.0)
    } else {
        (2.0 * w + fin - n, -1.0)
    }
}

fn tube_point(f: &Format, n: f32, v: f32) -> [f32; 3] {
    let (x, side) = net_to_x(f, n);
    let hw = f.w * 0.5;
    let prof = (1.0 - (x.abs() / hw).min(1.0).powf(6.0)).max(0.0).sqrt();
    let (c, l) = (f.crimp, f.l);
    let pinch = smoothstep(c, c + 3.0, v) * (1.0 - smoothstep(c + 4.0, c + 13.0, v)) + smoothstep(l - c - 11.0, l - c - 3.0, v) * (1.0 - smoothstep(l - c - 2.0, l - c, v));
    let ph = vnoise(x * 0.35, side, 21) * 6.0;
    let wr = (x * std::f32::consts::TAU / 3.4 + ph).sin() * 0.09 * pinch;
    [x, l - v, side * (half_thick(f, v) * prof + wr * prof)]
}

/// The sealed pack (mm, facing +Z, bottom at y = 0): the wrapper tube and the folded fin.
fn pack_prim(f: &Format, per_quarter: usize, rows: usize) -> Prim {
    let (w, fin) = (f.w, f.fin);
    let q = [fin, fin + 0.5 * w, fin + w, fin + 1.5 * w, fin + 2.0 * w];
    let mut cols = Vec::new();
    for k in 0..4 {
        let fold_end = k == 0 || k == 2;
        for i in 0..per_quarter {
            let t = i as f32 / per_quarter as f32;
            let tt = if fold_end { mix(t, (t * std::f32::consts::FRAC_PI_2).sin(), 0.75) } else { mix(t, 1.0 - (t * std::f32::consts::FRAC_PI_2).cos(), 0.75) };
            cols.push(q[k] + (q[k + 1] - q[k]) * tt);
        }
    }
    cols.push(q[4]);
    let (nw, nh) = f.net_mm();
    let vrow = |r: usize| {
        let t = r as f32 / (rows - 1) as f32;
        f.l * mix(t, 0.5 - 0.5 * (t * std::f32::consts::PI).cos(), 0.5)
    };
    let (mut pos, mut uv) = (Vec::new(), Vec::new());
    for r in 0..rows {
        let v = vrow(r);
        for &n in &cols {
            pos.push(tube_point(f, n, v));
            uv.push([n / nw, v / nh]);
        }
    }
    let mut tube = grid(cols.len(), rows, pos.clone(), uv.clone(), false);
    // outward: the front centre must face +Z
    let probe = (rows / 2) * cols.len() + cols.iter().position(|&n| n >= fin + w).unwrap_or(0);
    if tube.nrm[probe][2] < 0.0 {
        tube = grid(cols.len(), rows, pos, uv, true);
    }
    // the fin, folded flat over the back (x in 0..fin)
    let fc = 6;
    let (mut fp, mut fu) = (Vec::new(), Vec::new());
    for r in 0..rows {
        let v = vrow(r);
        for c in 0..fc {
            let x = fin * c as f32 / (fc - 1) as f32;
            let mut p = tube_point(f, 2.0 * w + fin - x, v);
            p[2] -= 0.07 + 0.03 * (c as f32 / (fc - 1) as f32);
            fp.push(p);
            fu.push([(fin + 2.0 * w + x) / nw, v / nh]);
        }
    }
    let mut finp = grid(fc, rows, fp.clone(), fu.clone(), false);
    if finp.nrm[(rows / 2) * fc + 2][2] > 0.0 {
        finp = grid(fc, rows, fp, fu, true);
    }
    tube.append(&finp);
    tube
}

fn booster6_pack(root: &Path, f: &Format) {
    let id = "booster6_pack";
    let dir = root.join("assets/props").join(id);
    let tex = dir.join("textures");
    std::fs::create_dir_all(&tex).unwrap();
    layers(&root.join("assets/cards/packs").join(f.slug), &tex, "booster6_pack_film", 2048, 2048);
    let mut p = pack_prim(f, 26, 84);
    finish(&mut p);
    write(&dir, id, &[(0, p)], &[Mat { double_sided: true, ..mat("booster6_pack_film", "booster6_pack_film", 1.0) }]);
    println!("wrote {}", dir.display());
}

// ----------------------------------------------------------------------------- the display box

/// Box outer size (mm): width, depth, front height, side top at the back, back/header height.
const BOX_W: f32 = 74.0;
const BOX_D: f32 = 74.0;
const BOX_FRONT: f32 = 60.0;
const BOX_SIDE_BACK: f32 = 104.0;
const BOX_BACK: f32 = 176.0;
const BOARD: f32 = 1.6;

/// Atlas regions (px in a 2048 atlas): x, y, w, h.
#[derive(Clone, Copy)]
enum Reg {
    Header,
    Front,
    Side,
    Inner,
    BackOut,
    Board,
    Kraft,
}

fn reg(r: Reg) -> (f32, f32, f32, f32) {
    match r {
        Reg::Header => (0.0, 0.0, 512.0, 1024.0),
        Reg::Front => (512.0, 0.0, 512.0, 512.0),
        Reg::Side => (1024.0, 0.0, 512.0, 512.0),
        Reg::Inner => (1536.0, 0.0, 512.0, 512.0),
        Reg::BackOut => (512.0, 512.0, 512.0, 1024.0),
        Reg::Board => (1024.0, 512.0, 512.0, 512.0),
        Reg::Kraft => (1536.0, 512.0, 512.0, 512.0),
    }
}

/// Paint the box atlas: base, normal and ORM.
fn box_atlas(tex: &Path, fonts: &Fonts, game: &str) {
    const N: usize = 2048;
    let gold = V3(1.0, 0.78, 0.42);
    // text masks in atlas pixels
    let txt = |font: &fontdue::Font, size: f32, s: &str, track: f32, cx: f32, base: f32, maxw: f32| -> Mask {
        let (sz, sx) = text::fit(font, size, size * 0.5, s, track, maxw);
        let w = text::measure(font, sz, s, track) * sx;
        text::line(font, sz, s, track, cx - w * 0.5, base, sx)
    };
    let up = game.to_uppercase();
    let (hx, hy, hw, hh) = reg(Reg::Header);
    let (fx, fy, fw, fh) = reg(Reg::Front);
    let (sx, sy, sw, sh) = reg(Reg::Side);
    let (bx, by, bw, bh) = reg(Reg::BackOut);
    let metal_text = vec![
        txt(&fonts.logo, 92.0, &up, 8.0, hx + hw * 0.5, hy + hh * 0.30, hw - 40.0),
        txt(&fonts.logo, 64.0, &up, 6.0, fx + fw * 0.5, fy + fh * 0.42, fw - 40.0),
        txt(&fonts.logo, 46.0, &up, 4.0, sx + sw * 0.5, sy + sh * 0.62, sw - 60.0),
        txt(&fonts.logo, 70.0, &up, 6.0, bx + bw * 0.5, by + bh * 0.16, bw - 60.0),
    ];
    let white_text = vec![
        txt(&fonts.jp, 30.0, "ミスビースト", 6.0, hx + hw * 0.5, hy + hh * 0.36, hw),
        txt(&fonts.label, 58.0, "BOOSTER BOX", 6.0, hx + hw * 0.5, hy + hh * 0.52, hw - 60.0),
        txt(&fonts.label, 30.0, "20 PACKS  \u{00b7}  6 CARDS PER PACK", 2.0, hx + hw * 0.5, hy + hh * 0.585, hw - 50.0),
        txt(&fonts.label, 30.0, "BOOSTER PACK  \u{00b7}  \u{00a5}300", 2.0, fx + fw * 0.5, fy + fh * 0.62, fw - 40.0),
        txt(&fonts.label, 24.0, "20 PACKS", 3.0, sx + sw * 0.5, sy + sh * 0.72, sw - 60.0),
        txt(&fonts.label, 26.0, "TRADING CARD GAME", 3.0, bx + bw * 0.5, by + bh * 0.22, bw - 60.0),
    ];
    let px: Vec<(V3, f32, f32, f32)> = (0..N * N)
        .into_par_iter()
        .map(|i| {
            let (x, y) = ((i % N) as f32 + 0.5, (i / N) as f32 + 0.5);
            let inr = |r: Reg| {
                let (rx, ry, rw, rh) = reg(r);
                x >= rx && x < rx + rw && y >= ry && y < ry + rh
            };
            let local = |r: Reg| {
                let (rx, ry, rw, rh) = reg(r);
                ((x - rx) / rw, (y - ry) / rh)
            };
            // (colour, metal, rough, relief)
            let ground = |v: f32| hex(0x161c38).lerp(hex(0x090b15), v * 0.9);
            let mut c;
            let mut metal = 0.0f32;
            let mut rough = 0.55f32;
            let mut h = 0.0f32;
            if inr(Reg::Board) {
                // raw board edge: off-white with a corrugated flute line
                let (u, _) = local(Reg::Board);
                c = hex(0xd8d2c4).lerp(hex(0xb8ae98), (0.5 + 0.5 * (u * 120.0).sin()) * 0.25);
                rough = 0.85;
                h = (u * 120.0).sin() * 0.02;
            } else if inr(Reg::Kraft) {
                let (u, v) = local(Reg::Kraft);
                c = hex(0x9b7b52) * (1.0 + (vnoise(u * 90.0, v * 90.0, 5) - 0.5) * 0.12);
                rough = 0.9;
                h = vnoise(u * 160.0, v * 160.0, 6) * 0.01;
            } else if inr(Reg::Inner) {
                let (u, v) = local(Reg::Inner);
                c = hex(0x1d2446) * (1.0 + (vnoise(u * 40.0, v * 40.0, 7) - 0.5) * 0.06);
                rough = 0.6;
            } else {
                // printed panels: the pack's midnight ink with a metallic sunburst
                let (r, (u, v)) = [Reg::Header, Reg::Front, Reg::Side, Reg::BackOut].iter().find(|r| inr(**r)).map(|r| (*r, local(*r))).unwrap_or((Reg::Inner, (0.0, 0.0)));
                c = ground(v);
                rough = 0.32;
                let (cx, cy, aspect) = match r {
                    Reg::Header => (0.5, 0.3, 2.0),
                    Reg::Front => (0.5, 0.42, 1.0),
                    Reg::Side => (0.5, 0.62, 1.0),
                    _ => (0.5, 0.16, 2.0),
                };
                let (dx, dy) = (u - cx, (v - cy) * aspect);
                let ang = dy.atan2(dx);
                let rr = (dx * dx + dy * dy).sqrt();
                let ray = (0.5 + 0.5 * (ang * 30.0).cos()).powf(6.0) * (1.0 - smoothstep(0.05, 0.75, rr)) * smoothstep(0.02, 0.08, rr);
                metal = metal.max(ray * 0.55);
                // gold pinstripe border
                let edge = u.min(1.0 - u).min((v.min(1.0 - v)) * aspect);
                let stripe = sdf::cov_soft((edge - 0.045).abs() - 0.004, 0.002);
                metal = metal.max(stripe);
                if let Reg::Header = r {
                    // the art window: a flat placeholder colour, like the pack
                    let d = sdf::rrect((u, v * 2.0), 0.1, 1.25, 0.9, 1.85, 0.04);
                    let fill = sdf::cov_soft(d, 0.004);
                    c = c.lerp(hex(0x2f4a78).lerp(hex(0x223660), v), fill);
                    metal *= 1.0 - fill;
                    metal = metal.max(sdf::cov_soft(d.abs() - 0.006, 0.002));
                }
            }
            for m in &metal_text {
                let a = m.bilinear(x, y);
                metal = metal.max(a);
            }
            for m in &white_text {
                let a = m.bilinear(x, y);
                c = c.lerp(V3(0.92, 0.92, 0.94), a);
                metal *= 1.0 - a;
            }
            // metallised areas: gold foil, a soft mirror
            let col = c.lerp(gold * 0.95, metal);
            rough = mix(rough, 0.18, metal);
            h += metal * 0.015;
            (col, metal, rough, h)
        })
        .collect();
    let base: Vec<[u8; 3]> = px.iter().map(|p| [(to_srgb(p.0 .0.clamp(0.0, 1.0)) * 255.0) as u8, (to_srgb(p.0 .1.clamp(0.0, 1.0)) * 255.0) as u8, (to_srgb(p.0 .2.clamp(0.0, 1.0)) * 255.0) as u8]).collect();
    save(&tex.join("booster6_box_print_base.png"), N, N, &base);
    let orm: Vec<[u8; 3]> = px.iter().map(|p| [255, (p.2 * 255.0) as u8, (p.1 * 255.0) as u8]).collect();
    save(&tex.join("booster6_box_print_orm.png"), N, N, &orm);
    let hgt = |x: isize, y: isize| px[(y.clamp(0, N as isize - 1) as usize) * N + x.clamp(0, N as isize - 1) as usize].3;
    let nrm: Vec<[u8; 3]> = (0..N * N)
        .into_par_iter()
        .map(|i| {
            let (x, y) = ((i % N) as isize, (i / N) as isize);
            let dx = (hgt(x + 1, y) - hgt(x - 1, y)) * 6.0;
            let dy = (hgt(x, y + 1) - hgt(x, y - 1)) * 6.0;
            let n = V3(-dx, -dy, 1.0).norm();
            [((n.0 * 0.5 + 0.5) * 255.0) as u8, ((n.1 * 0.5 + 0.5) * 255.0) as u8, ((n.2 * 0.5 + 0.5) * 255.0) as u8]
        })
        .collect();
    save(&tex.join("booster6_box_print_normal.png"), N, N, &nrm);
}

/// One face of a box: four corners (ccw seen from outside) mapped to an atlas region.
fn quad(p: &mut Prim, c: [[f32; 3]; 4], r: Reg) {
    let (rx, ry, rw, rh) = reg(r);
    let pad = 2.0;
    let uvs = [[rx + pad, ry + rh - pad], [rx + rw - pad, ry + rh - pad], [rx + rw - pad, ry + pad], [rx + pad, ry + pad]];
    let base = p.pos.len() as u32;
    let e1 = [c[1][0] - c[0][0], c[1][1] - c[0][1], c[1][2] - c[0][2]];
    let e2 = [c[3][0] - c[0][0], c[3][1] - c[0][1], c[3][2] - c[0][2]];
    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
    for k in 0..4 {
        p.pos.push(c[k]);
        p.nrm.push([n[0] / l, n[1] / l, n[2] / l]);
        p.uv.push([uvs[k][0] / 2048.0, uvs[k][1] / 2048.0]);
    }
    p.idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// A cuboid panel; `outer` is the region for the face whose normal is `out_dir` (0..6: -x +x -y
/// +y -z +z), `inner` for its opposite, the rest raw board.
fn panel(p: &mut Prim, lo: [f32; 3], hi: [f32; 3], out_dir: usize, outer: Reg, inner: Reg) {
    let (x0, y0, z0, x1, y1, z1) = (lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]);
    let faces: [[[f32; 3]; 4]; 6] = [
        [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]], // -x (seen from -x: z grows to the right)
        [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]], // +x
        [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]], // -y
        [[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]], // +y
        [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]], // -z
        [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], // +z
    ];
    let opp = out_dir ^ 1;
    for (k, f) in faces.iter().enumerate() {
        let r = if k == out_dir { outer } else if k == opp { inner } else { Reg::Board };
        quad(p, *f, r);
    }
}

/// A side wall: the sloped profile (front low, back high) extruded through the board thickness.
fn side_wall(p: &mut Prim, x0: f32, x1: f32, outer_dir_plus: bool) {
    let (zf, zb) = (BOX_D * 0.5 - BOARD, -BOX_D * 0.5 + BOARD);
    let prof = [[zf, BOARD], [zb, BOARD], [zb, BOX_SIDE_BACK], [zf, BOX_FRONT]];
    let (rx, ry, rw, rh) = reg(Reg::Side);
    let map = |z: f32, y: f32, region: Reg| {
        let (ox, oy, ow, oh) = reg(region);
        let _ = (rx, ry, rw, rh);
        let u = (z - zb) / (zf - zb);
        let v = 1.0 - y / BOX_SIDE_BACK;
        [(ox + 2.0 + u * (ow - 4.0)) / 2048.0, (oy + 2.0 + v * (oh - 4.0)) / 2048.0]
    };
    // the two big faces
    for (x, plus) in [(x1, true), (x0, false)] {
        let region = if plus == outer_dir_plus { Reg::Side } else { Reg::Inner };
        let base = p.pos.len() as u32;
        let n = if plus { [1.0, 0.0, 0.0] } else { [-1.0, 0.0, 0.0] };
        for q in &prof {
            p.pos.push([x, q[1], q[0]]);
            p.nrm.push(n);
            // seen from outside the profile reads the same either way round
            let uv = map(if plus { q[0] } else { zf + zb - q[0] }, q[1], region);
            p.uv.push(uv);
        }
        // the profile is a convex quad: fan it, facing out
        if plus {
            p.idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            p.idx.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    // the edges round the profile
    for k in 0..4 {
        let (a, b) = (prof[k], prof[(k + 1) % 4]);
        quad(p, [[x0, a[1], a[0]], [x0, b[1], b[0]], [x1, b[1], b[0]], [x1, a[1], a[0]]], Reg::Board);
    }
}

fn booster6_box(root: &Path, f: &Format, fonts: &Fonts, game: &str) {
    let id = "booster6_box";
    let dir = root.join("assets/props").join(id);
    let tex = dir.join("textures");
    std::fs::create_dir_all(&tex).unwrap();
    box_atlas(&tex, fonts, game);
    layers(&root.join("assets/cards/packs").join(f.slug), &tex, "booster6_pack_film", 2048, 2048);
    let (hw, hd) = (BOX_W * 0.5, BOX_D * 0.5);
    let mut b = Prim::default();
    // base, front, back (rising into the header), the two sloped sides
    panel(&mut b, [-hw, 0.0, -hd], [hw, BOARD, hd], 2, Reg::Kraft, Reg::Inner);
    panel(&mut b, [-hw, BOARD, hd - BOARD], [hw, BOX_FRONT, hd], 5, Reg::Front, Reg::Inner);
    panel(&mut b, [-hw, BOARD, -hd], [hw, BOX_BACK, -hd + BOARD], 4, Reg::BackOut, Reg::Header);
    side_wall(&mut b, hw - BOARD, hw, true);
    side_wall(&mut b, -hw, -hw + BOARD, false);
    // 20 packs standing in a row, the first just behind the front wall
    let lp = pack_prim(f, 8, 22);
    let mut packs = Prim::default();
    for i in 0..20 {
        let mut q = lp.clone();
        let jitter = |k: u32| hash2f(i, k as i32, 31) - 0.5;
        let lean = (2.5 + jitter(1) * 2.0).to_radians();
        let yaw = (jitter(2) * 2.0).to_radians();
        let (cl, sl, cy, sy) = (lean.cos(), lean.sin(), yaw.cos(), yaw.sin());
        // yaw * lean (lean back: the top goes towards -z)
        let m = [[cy, sy * sl, sy * cl], [0.0, cl, -sl], [-sy, cy * sl, cy * cl]];
        let z = hd - BOARD - 2.2 - i as f32 * 3.2;
        q.transform(m, [jitter(3) * 1.2, BOARD + 0.05, z]);
        packs.append(&q);
    }
    finish(&mut b);
    finish(&mut packs);
    write(
        &dir,
        id,
        &[(0, b), (1, packs)],
        &[mat("booster6_box_print", "booster6_box_print", 1.0), Mat { double_sided: true, ..mat("booster6_pack_film", "booster6_pack_film", 1.0) }],
    );
    println!("wrote {}", dir.display());
}

// ----------------------------------------------------------------------------- singles

const CARD_W: f32 = 63.0;
const CARD_H: f32 = 88.0;
const CARD_T: f32 = 0.31;
const CORNER: f32 = 2.94;

/// The card slab (mm, centred, facing +Z): front, back (mirrored u) and edge prims.
fn card_prims() -> (Prim, Prim, Prim) {
    let (ru, rv) = (CORNER / CARD_W, CORNER / CARD_H);
    let mut ol = Vec::new();
    for (cx, cy, a0) in [(0.5 - ru, -(0.5 - rv), -0.5f32), (0.5 - ru, 0.5 - rv, 0.0), (-(0.5 - ru), 0.5 - rv, 0.5), (-(0.5 - ru), -(0.5 - rv), 1.0)] {
        for k in 0..=8 {
            let a = (a0 + k as f32 / 8.0 * 0.5) * std::f32::consts::PI;
            ol.push((cx + a.cos() * ru + 0.5, 0.5 - (cy + a.sin() * rv)));
        }
    }
    let n = ol.len();
    let pt = |u: f32, v: f32, z: f32| [(u - 0.5) * CARD_W, (0.5 - v) * CARD_H, z];
    let face = |z: f32, nz: f32, mirror: bool| {
        let mut p = Prim::default();
        let uv = |u: f32, v: f32| if mirror { [1.0 - u, v] } else { [u, v] };
        p.pos.push(pt(0.5, 0.5, z));
        p.nrm.push([0.0, 0.0, nz]);
        p.uv.push(uv(0.5, 0.5));
        for &(u, v) in &ol {
            p.pos.push(pt(u, v, z));
            p.nrm.push([0.0, 0.0, nz]);
            p.uv.push(uv(u, v));
        }
        for i in 0..n as u32 {
            let (a, b) = (1 + i, 1 + (i + 1) % n as u32);
            if nz > 0.0 { p.idx.extend([0, a, b]) } else { p.idx.extend([0, b, a]) }
        }
        p
    };
    let z = CARD_T * 0.5;
    let mut e = Prim::default();
    for i in 0..n {
        let (u0, v0) = ol[i];
        let (u1, v1) = ol[(i + 1) % n];
        let (p0, p1) = (pt(u0, v0, 0.0), pt(u1, v1, 0.0));
        let d = [p1[0] - p0[0], p1[1] - p0[1]];
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
        let nn = [d[1] / l, -d[0] / l, 0.0];
        let base = e.pos.len() as u32;
        for (p, zz, uu) in [(p0, z, 0.0), (p0, -z, 1.0), (p1, -z, 1.0), (p1, z, 0.0)] {
            e.pos.push([p[0], p[1], zz]);
            e.nrm.push(nn);
            e.uv.push([uu, i as f32 / n as f32]);
        }
        e.idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (face(z, 1.0, false), face(-z, -1.0, true), e)
}

/// A thin flat box (mm), outward normals; UVs are planar per face (for glass and acrylic).
fn slab(p: &mut Prim, lo: [f32; 3], hi: [f32; 3]) {
    panel(p, lo, hi, 5, Reg::Board, Reg::Board);
}

fn rotate_x(p: &mut Prim, a: f32, pivot: [f32; 3]) {
    let (c, s) = (a.cos(), a.sin());
    p.transform([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], [-pivot[0], -pivot[1], -pivot[2]]);
    p.transform([[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]], pivot);
}

fn single_stand(root: &Path, id: &str, template: &str) {
    let dir = root.join("assets/props").join(id);
    let tex = dir.join("textures");
    std::fs::create_dir_all(&tex).unwrap();
    let tdir = root.join("assets/cards/templates");
    layers(&tdir.join(template), &tex, &format!("{id}_face"), 2048, 2048);
    layers(&tdir.join("back"), &tex, "card_back", 2048, 2048);
    plain(&tex);
    // the top-loader: 3 x 4 in rigid PVC, open at the top; the card sits 3 mm off its bottom
    let (tw, th) = (76.2, 101.6);
    let (sheet, gap) = (0.5, 0.9);
    let mut tl = Prim::default();
    let zt = gap * 0.5 + sheet;
    slab(&mut tl, [-tw * 0.5, 0.0, gap * 0.5], [tw * 0.5, th, zt]);
    slab(&mut tl, [-tw * 0.5, 0.0, -zt], [tw * 0.5, th, -gap * 0.5]);
    slab(&mut tl, [-tw * 0.5, 0.0, -gap * 0.5], [-tw * 0.5 + 3.2, th - 2.0, gap * 0.5]);
    slab(&mut tl, [tw * 0.5 - 3.2, 0.0, -gap * 0.5], [tw * 0.5, th - 2.0, gap * 0.5]);
    slab(&mut tl, [-tw * 0.5, 0.0, -gap * 0.5], [tw * 0.5, 3.0, gap * 0.5]);
    let (mut front, mut back, mut edge) = card_prims();
    for p in [&mut front, &mut back, &mut edge] {
        p.transform([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], [0.0, 3.0 + CARD_H * 0.5, 0.0]);
    }
    // the acrylic easel: a base plate, a lip at the front, a back rest; the top-loader leans 14°
    let lean = -14f32.to_radians();
    let (bz0, bz1) = (-26.0, 22.0);
    let mut stand = Prim::default();
    slab(&mut stand, [-42.0, 0.0, bz0], [42.0, 3.0, bz1]);
    slab(&mut stand, [-42.0, 3.0, 16.0], [42.0, 9.0, 19.0]);
    let mut rest = Prim::default();
    slab(&mut rest, [-34.0, 0.0, -1.5], [34.0, 78.0, 1.5]);
    rotate_x(&mut rest, lean, [0.0, 0.0, 0.0]);
    rest.transform([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], [0.0, 3.0, 14.0 - 0.95 - 1.5 - 0.25]);
    stand.append(&rest);
    // the top-loader stands on the base plate, against the rest
    let place = |p: &mut Prim| {
        rotate_x(p, lean, [0.0, 0.0, 0.0]);
        p.transform([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], [0.0, 3.05, 14.0]);
    };
    for p in [&mut tl, &mut front, &mut back, &mut edge] {
        place(p);
    }
    let mut prims = vec![(0usize, front), (1, back), (2, edge), (3, tl), (4, stand)];
    for (_, p) in prims.iter_mut() {
        finish(p);
    }
    let mats = [
        mat(&format!("{id}_face"), &format!("{id}_face"), 1.0),
        mat("card_back", "card_back", 1.0),
        Mat { name: "card_edge".into(), base: "plain_white_base.png".into(), normal: Some("plain_flat_normal.png".into()), orm: Some("plain_edge_orm.png".into()), normal_scale: 1.0, double_sided: false },
        glass_mat("toploader_clear_glass"),
        glass_mat("acrylic_stand_clear_glass"),
    ];
    write(&dir, id, &prims, &mats);
    println!("wrote {}", dir.display());
}

pub fn build(root: &Path, fonts: &Fonts, game: &str) {
    let f = FORMATS.iter().find(|f| f.slug == "booster6").unwrap();
    booster6_pack(root, f);
    booster6_box(root, f, fonts, game);
    for (id, t) in [("single_stand_gold", "front_gold"), ("single_stand_holo", "front_holo"), ("single_stand_obsidian", "front_obsidian")] {
        single_stand(root, id, t);
    }
}
