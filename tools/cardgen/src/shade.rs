//! Turning the layered canvas into output layers, and the foil preview renderer.
//!
//! The preview mirrors what the engine shader should do (see docs/CARD_TEMPLATES.md): per-pixel
//! half vector from a point light and a perspective camera, foil hue driven by the half vector's
//! tangent-plane component plus a baked phase, glitter flashes from a per-pixel seed.

use crate::canvas::{foil_id, Canvas, Mask};
use crate::math::*;
use crate::sdf::*;
use rayon::prelude::*;

pub struct Layers {
    pub w: usize,
    pub h: usize,
    /// Print colour with baked printed relief (linear).
    pub base: Vec<V3>,
    pub alpha: Vec<f32>,
    /// Physical emboss height 0..1.
    pub height: Vec<f32>,
    /// R foil strength, G pattern id * 40, B phase, A glitter seed.
    pub foil: Vec<[u8; 4]>,
    /// R metallic, G gloss, B printed-relief amount.
    pub spec: Vec<[u8; 3]>,
}

/// Normal-map slope scale for design height (px).
const K_DESIGN: f32 = 16.0;
/// Normal slope scale for the physical height layer when previewing.
pub const K_PHYS: f32 = 30.0;

fn normal_at(h: &[f32], w: usize, hh: usize, x: usize, y: usize, k: f32) -> V3 {
    let g = |x: isize, y: isize| h[(y.clamp(0, hh as isize - 1) as usize) * w + x.clamp(0, w as isize - 1) as usize];
    let (xi, yi) = (x as isize, y as isize);
    let dx = (g(xi + 1, yi) - g(xi - 1, yi)) * 0.5;
    let dy = (g(xi, yi + 1) - g(xi, yi - 1)) * 0.5;
    V3(-dx * k, -dy * k, 1.0).norm()
}

/// Studio environment for printed metallic ink: broad soft bands plus a top-left key.
fn env_shade(n: V3, x: f32, y: f32) -> f32 {
    let t = x * 0.00055 + y * 0.00042 + n.0 * 0.55 + n.1 * 0.75;
    let band = 0.5 + 0.5 * (t * std::f32::consts::TAU * 1.15 + 0.6).sin();
    let key = (-n.0 * 0.6 - n.1 * 0.75).max(0.0);
    let under = (n.0 * 0.5 + n.1 * 0.7).max(0.0);
    0.42 + 0.75 * band * band + 1.1 * key - 0.55 * under
}

pub fn finalize(c: &Canvas, emboss: f32) -> Layers {
    let (w, h) = (c.w, c.h);
    let hd: Vec<f32> = c.px.iter().map(|p| p.h).collect();
    // Blurred height for ambient occlusion and soft cast shadows.
    let mut m = Mask::new(0, 0, w, h);
    m.d.copy_from_slice(&hd);
    let hb = m.blur(7.0);
    let light = V3(-0.45, -0.55, 0.70).norm();
    let card_d = |x: f32, y: f32| rrect((x, y), 0.0, 0.0, w as f32, h as f32, crate::design::CORNER);

    let base: Vec<V3> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let p = &c.px[i];
            let n = normal_at(&hd, w, h, x, y, K_DESIGN);
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let shade = (n.dot(light) / light.2).max(0.0);
            let mut col = p.col * mix(1.0, shade, p.relief);
            if p.env > 0.0 {
                let e = env_shade(n, fx, fy);
                col = col.lerp(p.col * e, p.env);
            }
            // AO + cast shadow toward bottom-right.
            let here = hd[i];
            let ao = (hb.get(x as i32, y as i32) - here).max(0.0);
            let cast = (hb.get(x as i32 - 5, y as i32 - 6) - here).max(0.0);
            let occ = (ao * 0.9 + cast * 1.1).min(0.65);
            col = col * (1.0 - occ);
            // Card stock and ink texture.
            let fib = fbm(fx * 0.35, fy * 0.08, 2, 1234) - 0.5;
            let mot = fbm(fx * 0.01, fy * 0.01, 3, 4321) - 0.5;
            let grain = hash2f(x as i32, y as i32, 77) - 0.5;
            col = col * (1.0 + fib * 0.025 + mot * 0.03 + grain * 0.02);
            // Edge wear: the white card core shows a hair at the cut edge.
            let ed = -card_d(fx, fy);
            if ed < 3.0 {
                let wear = smoothstep(3.0, 0.5, ed) * (0.35 + 0.4 * vnoise(fx * 0.08, fy * 0.08, 9));
                col = col.lerp(V3::splat(0.55), wear * 0.6);
            }
            col.clamp01()
        })
        .collect();

    let height: Vec<f32> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % w, i / w);
            // smooth paper tooth, not per-pixel noise (noise would scramble foil normals)
            let grain = vnoise(x as f32 * 0.35, y as f32 * 0.35, 31) * 0.0004;
            (c.px[i].h * emboss * 0.55 + grain).clamp(0.0, 1.0)
        })
        .collect();
    let foil = c
        .px
        .iter()
        .map(|p| {
            if p.foil <= 0.0 || p.fid == foil_id::NONE {
                [0, 0, 0, 0]
            } else {
                [(p.foil.clamp(0.0, 1.0) * 255.0) as u8, p.fid * foil_id::STEP, (fract(p.phase) * 255.0) as u8, (p.spark.clamp(0.0, 1.0) * 255.0) as u8]
            }
        })
        .collect();
    let spec = c
        .px
        .iter()
        .map(|p| [(p.metal.max(p.env).clamp(0.0, 1.0) * 255.0) as u8, (p.gloss.clamp(0.0, 1.0) * 255.0) as u8, (p.relief.clamp(0.0, 1.0) * 255.0) as u8])
        .collect();
    Layers { w, h, base, alpha: c.alpha.clone(), height, foil, spec }
}

/// Normal-map slope scale for the engine normal map (exaggerates the 0.08 mm emboss a little
/// so it reads under real lighting).
pub const K_ENGINE: f32 = 20.0;

/// Engine maps: (normal RGB8, ORM RGB8).
/// Normal: tangent space, X = +u (right), Y = +v (down the image, matching UV v), Z out.
/// ORM: R occlusion (1), G roughness, B metallic. Foil counts as metal under the ink, so a
/// renderer without the foil shader still shows coloured metallic reflections there.
pub fn engine_maps(l: &Layers) -> (Vec<u8>, Vec<u8>) {
    let (w, h) = (l.w, l.h);
    let enc = |v: f32| ((v * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let nrm: Vec<u8> = (0..w * h)
        .into_par_iter()
        .flat_map_iter(|i| {
            let (x, y) = (i % w, i / w);
            let n = normal_at(&l.height, w, h, x, y, K_ENGINE);
            [enc(n.0), enc(n.1), enc(n.2)]
        })
        .collect();
    let orm: Vec<u8> = (0..w * h)
        .flat_map(|i| {
            let s = l.spec[i];
            let f = l.foil[i];
            let foil = f[0] as f32 / 255.0;
            let metal_ink = s[0] as f32 / 255.0;
            let gloss = s[1] as f32 / 255.0;
            // Partly metallic: without the foil shader a full mirror hides the print; this keeps the
            // ink colour with a bright glossy sheen.
            let metal = metal_ink.max(foil * 0.35);
            let rough = mix(mix(0.72, 0.36, gloss), 0.16, foil.max(metal_ink * 0.8));
            [255u8, (rough * 255.0) as u8, (metal * 255.0) as u8]
        })
        .collect();
    (nrm, orm)
}

/// Foil reflectance for one pixel. `ang` is the diffraction coordinate (tangent-plane half
/// vector projected on the grating direction), `lobe` the closeness to the mirror direction
/// (0..1). Shared with the shader notes in docs/CARD_TEMPLATES.md.
pub fn foil_color(fid: u8, phase: f32, spark: f32, ang: f32, lobe: f32) -> V3 {
    use std::f32::consts::TAU;
    let silver = V3::splat(0.55);
    let mut f = match fid {
        foil_id::AURORA => {
            let hue = fract(phase + ang * 1.1);
            silver.lerp(spectral(hue), 0.30 + 0.40 * lobe) * (0.55 + 1.0 * lobe)
        }
        foil_id::SHATTER => {
            // every pattern cell has its own phase, so cells flash in and out independently
            let hue = fract(phase * 0.15 + ang * 1.1);
            let flash = 0.5 + 0.5 * ((phase * 5.0 + ang * 2.0) * TAU).sin();
            silver.lerp(spectral(hue), 0.30 + 0.35 * lobe) * (0.45 + 0.8 * lobe + 0.35 * flash * flash)
        }
        foil_id::ETCH => {
            let hue = fract(phase + ang * 1.2);
            silver.lerp(spectral(hue), 0.30 + 0.30 * lobe) * (0.5 + 1.0 * lobe)
        }
        foil_id::GILT => {
            let band = 0.5 + 0.5 * ((ang * 1.4 + phase) * TAU).sin();
            V3(1.0, 0.74, 0.36) * (0.30 + 0.55 * band * band + 1.1 * lobe) + spectral(fract(ang + phase)) * 0.05
        }
        foil_id::SILVER => V3::splat(0.45 + 1.0 * lobe),
        foil_id::COSMOS => {
            // deep nebula tint: violet/teal/magenta drifting with angle, darker than aurora
            let hue = fract(0.62 + phase * 0.5 + ang * 0.7);
            let neb = spectral(hue).lerp(V3(0.25, 0.2, 0.55), 0.35);
            V3::splat(0.35).lerp(neb, 0.55 + 0.3 * lobe) * (0.5 + 1.1 * lobe)
        }
        foil_id::ICE => {
            // icy silver-blue; each shard flips bright/dim at its own angle
            let flash = 0.5 + 0.5 * ((phase * 7.0 + ang * 2.4) * TAU).sin();
            let tint = V3(0.78, 0.9, 1.0).lerp(spectral(fract(phase + ang * 0.6)), 0.25);
            tint * (0.3 + 0.55 * flash * flash + 0.9 * lobe)
        }
        foil_id::RAINBOW => {
            let hue = fract(phase + ang * 0.9);
            V3::splat(0.7).lerp(spectral(hue), 0.55 + 0.25 * lobe) * (0.6 + 0.9 * lobe)
        }
        foil_id::GLASS => {
            // each pane keeps its own colour and lights up when it faces the light
            let shine = 0.5 + 0.5 * ((phase * 4.0 + ang * 1.5) * TAU).sin();
            spectral(phase).lerp(V3::splat(1.0), 0.6) * (0.45 + 0.6 * shine + 0.8 * lobe)
        }
        foil_id::SAKURA => {
            let band = 0.5 + 0.5 * ((ang * 1.2 + phase) * TAU).sin();
            V3(1.0, 0.72, 0.74) * (0.45 + 0.45 * band + 1.0 * lobe) + spectral(fract(ang * 0.8 + phase)) * 0.08
        }
        foil_id::OBSIDIAN => {
            // black mirror with an oil-slick sheen only near the highlight
            V3::splat(0.05) + (V3::splat(0.6).lerp(spectral(fract(phase + ang * 0.8)), 0.5)) * (1.6 * lobe.powf(60.0))
        }
        _ => V3::splat(0.0),
    };
    if spark > 0.0 {
        // glitter: each fleck has its own facet angle and flashes in a narrow window
        let g = (((spark * 13.0 + ang * 2.5) * TAU).cos()).max(0.0).powf(60.0);
        f = f + V3::splat(g * 3.0);
    }
    f
}

/// Camera tilt for a preview: (x, y) offsets of the camera in card widths.
pub fn preview(l: &Layers, tilt: (f32, f32)) -> Vec<[f32; 4]> {
    let (w, h) = (l.w, l.h);
    let (wf, hf) = (w as f32, h as f32);
    let cam = V3(wf * 0.5 + tilt.0 * wf, hf * 0.5 + tilt.1 * hf, hf * 2.4);
    let lamp = V3(-wf * 0.35 + tilt.0 * wf * 0.3, -hf * 0.30, hf * 1.5);
    (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let p = V3(x as f32 + 0.5, y as f32 + 0.5, 0.0);
            let n = normal_at(&l.height, w, h, x, y, K_PHYS);
            let v = (cam - p).norm();
            let lt = (lamp - p).norm();
            let hv = (v + lt).norm();
            let ndl = n.dot(lt).max(0.0);
            let ndh = n.dot(hv).max(0.0);
            let base = l.base[i];
            let s = l.spec[i];
            let metal = s[0] as f32 / 255.0;
            let gloss = s[1] as f32 / 255.0;
            // Diffuse, normalised so a flat card under the lamp is ~1.
            let mut col = base * (0.60 + 0.48 * ndl);
            // Varnish sheen and metallic ink specular.
            let sheen = gloss * (0.035 * ndh.powf(30.0) + 0.05 * ndh.powf(400.0));
            col = col + V3::splat(sheen);
            col = col + base * (metal * 0.9 * ndh.powf(18.0));
            // Foil.
            let f = l.foil[i];
            if f[0] > 0 {
                let strength = f[0] as f32 / 255.0;
                let fid = ((f[1] as f32) / foil_id::STEP as f32).round() as u8;
                let phase = f[2] as f32 / 255.0;
                let spark = f[3] as f32 / 255.0;
                let tn = hv - n * hv.dot(n);
                let ang = (tn.0 * 0.8 + tn.1 * 0.6) * 6.0;
                let lobe = ndh.powf(5.0);
                let fc = foil_color(fid, phase, spark, ang, lobe);
                let foiled = match fid {
                    // metallic stamps: the foil *is* the colour, tinted by the base
                    foil_id::GILT | foil_id::SILVER | foil_id::SAKURA => fc * (0.22 + 1.5 * base.luma()),
                    foil_id::OBSIDIAN => fc + base * 0.6,
                    // printed ink over holographic foil: ink filters the reflection
                    _ => base * fc * 1.3 + fc * 0.12 * base.luma().sqrt(),
                };
                col = col.lerp(foiled, strength);
            }
            let a = l.alpha[i];
            [col.0, col.1, col.2, a]
        })
        .collect()
}

pub fn to_rgba8(px: &[[f32; 4]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(px.len() * 4);
    for p in px {
        out.push((to_srgb(p[0]) * 255.0 + 0.5) as u8);
        out.push((to_srgb(p[1]) * 255.0 + 0.5) as u8);
        out.push((to_srgb(p[2]) * 255.0 + 0.5) as u8);
        out.push((p[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
    }
    out
}

/// Area-average resample of a linear RGBA image.
pub fn resample(src: &[[f32; 4]], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<[f32; 4]> {
    let sx = sw as f32 / dw as f32;
    let sy = sh as f32 / dh as f32;
    (0..dw * dh)
        .into_par_iter()
        .map(|i| {
            let (x, y) = (i % dw, i / dw);
            let (x0, x1) = (x as f32 * sx, (x + 1) as f32 * sx);
            let (y0, y1) = (y as f32 * sy, (y + 1) as f32 * sy);
            let mut acc = [0.0f32; 4];
            let mut wsum = 0.0;
            for yy in (y0.floor() as usize)..(y1.ceil() as usize).min(sh) {
                let wy = (y1.min(yy as f32 + 1.0) - y0.max(yy as f32)).max(0.0);
                for xx in (x0.floor() as usize)..(x1.ceil() as usize).min(sw) {
                    let wx = (x1.min(xx as f32 + 1.0) - x0.max(xx as f32)).max(0.0);
                    let wgt = wx * wy;
                    let s = src[yy * sw + xx];
                    // premultiply
                    acc[0] += s[0] * s[3] * wgt;
                    acc[1] += s[1] * s[3] * wgt;
                    acc[2] += s[2] * s[3] * wgt;
                    acc[3] += s[3] * wgt;
                    wsum += wgt;
                }
            }
            let a = acc[3] / wsum.max(1e-9);
            if acc[3] > 1e-6 {
                [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], a]
            } else {
                [0.0, 0.0, 0.0, 0.0]
            }
        })
        .collect()
}
