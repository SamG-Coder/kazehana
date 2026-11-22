//! Tiling 3D noise for the volumetric clouds (shaders/clouds.glsl), after Schneider's Horizon Zero
//! Dawn / Nubis cloudscapes: a 128^3 shape texture (R: Perlin-Worley, the billowy base; G, B, A:
//! Worley fBm at rising frequencies that erode it into cauliflower heads) and a 32^3 detail
//! texture (R, G, B: finer Worley fBm for the wispy edges). Every channel tiles, so the shader can
//! repeat it across the sky. Built once (a few seconds on all cores) and cached in assets/cache. (Ported from SmartImposter.)

use std::path::Path;

pub const SHAPE: usize = 128;
pub const DETAIL: usize = 32;
const CACHE: &str = "assets/cache/cloud_noise.bin";
const MAGIC: &[u8; 8] = b"SICLD\0\0\x01";

fn hash(x: i32, y: i32, z: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343) ^ (y as u32).wrapping_mul(0xd8163841) ^ (z as u32).wrapping_mul(0xcb1ab31f) ^ seed.wrapping_mul(0x9e3779b9);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846ca68b);
    h ^ (h >> 16)
}

fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Worley (cellular) noise of period `cells` over the unit cube: 1 - distance to the nearest
/// feature point, one random point per cell, wrapping.
fn worley(p: [f32; 3], cells: i32, seed: u32) -> f32 {
    let q = [p[0] * cells as f32, p[1] * cells as f32, p[2] * cells as f32];
    let c = [q[0].floor() as i32, q[1].floor() as i32, q[2].floor() as i32];
    let mut best = f32::MAX;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (x, y, z) = (c[0] + dx, c[1] + dy, c[2] + dz);
                let (wx, wy, wz) = (x.rem_euclid(cells), y.rem_euclid(cells), z.rem_euclid(cells));
                let h = hash(wx, wy, wz, seed);
                let f = [x as f32 + unit(h), y as f32 + unit(hash(wx, wy, wz, seed ^ 0x51ed)), z as f32 + unit(hash(wx, wy, wz, seed ^ 0xa3c5))];
                let d = (f[0] - q[0]).powi(2) + (f[1] - q[1]).powi(2) + (f[2] - q[2]).powi(2);
                best = best.min(d);
            }
        }
    }
    (1.0 - best.sqrt()).clamp(0.0, 1.0)
}

/// Worley fBm: three octaves at f, 2f, 4f cells.
fn worley_fbm(p: [f32; 3], f: i32, seed: u32) -> f32 {
    worley(p, f, seed) * 0.625 + worley(p, f * 2, seed + 1) * 0.25 + worley(p, f * 4, seed + 2) * 0.125
}

/// Gradient (Perlin) noise of period `cells`, in [-1, 1].
fn perlin(p: [f32; 3], cells: i32, seed: u32) -> f32 {
    let q = [p[0] * cells as f32, p[1] * cells as f32, p[2] * cells as f32];
    let c = [q[0].floor() as i32, q[1].floor() as i32, q[2].floor() as i32];
    let f = [q[0] - c[0] as f32, q[1] - c[1] as f32, q[2] - c[2] as f32];
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let u = [fade(f[0]), fade(f[1]), fade(f[2])];
    let grad = |x: i32, y: i32, z: i32, dx: f32, dy: f32, dz: f32| {
        let h = hash(x.rem_euclid(cells), y.rem_euclid(cells), z.rem_euclid(cells), seed);
        // 12 edge directions of a cube
        let g: [f32; 3] = match h % 12 {
            0 => [1., 1., 0.],
            1 => [-1., 1., 0.],
            2 => [1., -1., 0.],
            3 => [-1., -1., 0.],
            4 => [1., 0., 1.],
            5 => [-1., 0., 1.],
            6 => [1., 0., -1.],
            7 => [-1., 0., -1.],
            8 => [0., 1., 1.],
            9 => [0., -1., 1.],
            10 => [0., 1., -1.],
            _ => [0., -1., -1.],
        };
        g[0] * dx + g[1] * dy + g[2] * dz
    };
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let mut v = [0.0f32; 8];
    for (k, item) in v.iter_mut().enumerate() {
        let (ox, oy, oz) = ((k & 1) as i32, ((k >> 1) & 1) as i32, ((k >> 2) & 1) as i32);
        *item = grad(c[0] + ox, c[1] + oy, c[2] + oz, f[0] - ox as f32, f[1] - oy as f32, f[2] - oz as f32);
    }
    let x00 = l(v[0], v[1], u[0]);
    let x10 = l(v[2], v[3], u[0]);
    let x01 = l(v[4], v[5], u[0]);
    let x11 = l(v[6], v[7], u[0]);
    l(l(x00, x10, u[1]), l(x01, x11, u[1]), u[2])
}

fn perlin_fbm(p: [f32; 3], f: i32, seed: u32) -> f32 {
    let (mut s, mut a, mut n) = (0.0, 1.0, 0.0);
    for o in 0..4 {
        s += a * perlin(p, f << o, seed + o as u32);
        n += a;
        a *= 0.5;
    }
    s / n
}

fn remap(v: f32, lo: f32, hi: f32, nlo: f32, nhi: f32) -> f32 {
    nlo + (v - lo) / (hi - lo) * (nhi - nlo)
}

/// Fill an n^3 RGBA8 volume, slices in parallel.
fn volume(n: usize, f: impl Fn([f32; 3]) -> [f32; 4] + Sync) -> Vec<u8> {
    let mut out = vec![0u8; n * n * n * 4];
    let threads = std::thread::available_parallelism().map(|v| v.get()).unwrap_or(4);
    let per = n.div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in out.chunks_mut(per * n * n * 4).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (i, px) in chunk.chunks_mut(4).enumerate() {
                    let idx = k * per * n * n + i;
                    let (x, y, z) = (idx % n, (idx / n) % n, idx / (n * n));
                    let p = [(x as f32 + 0.5) / n as f32, (y as f32 + 0.5) / n as f32, (z as f32 + 0.5) / n as f32];
                    let v = f(p);
                    for c in 0..4 {
                        px[c] = (v[c].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    }
                }
            });
        }
    });
    out
}

/// (shape 128^3 RGBA8, detail 32^3 RGBA8): from the cache, or built and cached.
pub fn load_or_build() -> (Vec<u8>, Vec<u8>) {
    let (ns, nd) = (SHAPE * SHAPE * SHAPE * 4, DETAIL * DETAIL * DETAIL * 4);
    if let Ok(b) = std::fs::read(CACHE) {
        if b.len() == 8 + ns + nd && &b[..8] == MAGIC {
            return (b[8..8 + ns].to_vec(), b[8 + ns..].to_vec());
        }
    }
    let t = std::time::Instant::now();
    let shape = volume(SHAPE, |p| {
        // Perlin-Worley: Perlin fBm whose low values are cut by inverted Worley (billows)
        let pw = perlin_fbm(p, 4, 7) * 0.5 + 0.5;
        let w = worley_fbm(p, 4, 11);
        let perlin_worley = remap(pw, w - 1.0, 1.0, 0.0, 1.0).clamp(0.0, 1.0);
        [perlin_worley, worley_fbm(p, 4, 21), worley_fbm(p, 8, 31), worley_fbm(p, 16, 41)]
    });
    let detail = volume(DETAIL, |p| [worley_fbm(p, 2, 51), worley_fbm(p, 4, 61), worley_fbm(p, 8, 71), 1.0]);
    if let Some(dir) = Path::new(CACHE).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut out = Vec::with_capacity(8 + ns + nd);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&shape);
    out.extend_from_slice(&detail);
    let _ = std::fs::write(CACHE, out);
    println!("Cloud noise built in {:.1}s", t.elapsed().as_secs_f32());
    (shape, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_tiles_and_spans() {
        // periodic: the value at the opposite face matches
        let a = worley([0.0, 0.3, 0.7], 8, 1);
        let b = worley([1.0, 0.3, 0.7], 8, 1);
        assert!((a - b).abs() < 1e-4);
        let c = perlin([0.2, 0.0, 0.5], 4, 3);
        let d = perlin([0.2, 1.0, 0.5], 4, 3);
        assert!((c - d).abs() < 1e-4);
        // Worley spans a useful range
        let vals: Vec<f32> = (0..200).map(|i| worley([i as f32 * 0.0137, 0.4, 0.2], 6, 9)).collect();
        let (lo, hi) = vals.iter().fold((1.0f32, 0.0f32), |(l, h), v| (l.min(*v), h.max(*v)));
        assert!(lo < 0.3 && hi > 0.45, "{lo} {hi}"); // (a line rarely passes right through a feature point)
    }
}
