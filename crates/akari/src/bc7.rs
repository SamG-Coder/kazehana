//! BC7 (mode 6) texture compression on the CPU: 4x4 RGBA blocks in 16 bytes, a quarter of RGBA8,
//! visually lossless for the Quixel scans (16 interpolation steps between two near-8-bit
//! endpoints per block, colour and alpha together, which alpha-tested leaf cards need).
//!
//! Endpoints start on the block's principal axis, are refitted by least squares to the chosen
//! indices, and the shared p-bits are searched; `psnr` measures the result against the source.
//! Blocks where alpha varies apart from colour (leaf card edges) also try mode 5 (separate colour
//! and alpha endpoints and indices); the more accurate mode wins. Colour under fully transparent
//! texels hardly counts: it is never seen.

/// BC7 4-bit index interpolation weights (/64).
const W4: [u32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

fn interp(e0: [u32; 4], e1: [u32; 4], w: u32) -> [u32; 4] {
    std::array::from_fn(|c| ((64 - w) * e0[c] + w * e1[c] + 32) >> 6)
}

fn err(a: [u32; 4], b: [u8; 4]) -> u32 {
    (0..4).map(|c| (a[c] as i32 - b[c] as i32).pow(2) as u32).sum()
}

/// Expand a 7-bit endpoint with its p-bit to 8 bits.
fn expand(q: [u32; 4], p: u32) -> [u32; 4] {
    q.map(|v| (v << 1) | p)
}

/// Quantise an endpoint (0..255 floats) to 7 bits for p-bit `p`.
fn quant(e: [f32; 4], p: u32) -> [u32; 4] {
    e.map(|v| (((v - p as f32) / 2.0).round()).clamp(0.0, 127.0) as u32)
}

/// Best indices and total error for two expanded endpoints.
fn assign(px: &[[u8; 4]; 16], e0: [u32; 4], e1: [u32; 4]) -> ([u8; 16], u32) {
    let pal: [[u32; 4]; 16] = std::array::from_fn(|i| interp(e0, e1, W4[i]));
    let mut idx = [0u8; 16];
    let mut total = 0;
    for (k, p) in px.iter().enumerate() {
        let (mut best, mut be) = (0u8, u32::MAX);
        for (i, c) in pal.iter().enumerate() {
            let e = err(*c, *p);
            if e < be {
                be = e;
                best = i as u8;
            }
        }
        idx[k] = best;
        total += be;
    }
    (idx, total)
}

/// Encode one 4x4 block (row-major texels): the better of modes 6 and 5.
pub fn encode_block(px: &[[u8; 4]; 16]) -> [u8; 16] {
    let m6 = encode_mode6(px);
    if px.iter().all(|p| p[3] == px[0][3]) {
        return m6; // constant alpha: mode 6 (16 colour steps) is the better one
    }
    let m5 = encode_mode5(px);
    if block_error(px, &decode_block(&m5)) < block_error(px, &decode_block(&m6)) {
        m5
    } else {
        m6
    }
}

/// Error of a decoded block. Colour under fully transparent texels is never seen (alpha test),
/// and a leaf colour there is better than the source's black (no dark fringes when filtered):
/// it counts only to break ties.
fn block_error(px: &[[u8; 4]; 16], dec: &[[u8; 4]; 16]) -> f64 {
    let mut e = 0.0;
    for k in 0..16 {
        let wc = if px[k][3] < 8 { 1e-4 } else { 1.0 };
        for c in 0..3 {
            e += wc * (dec[k][c] as f64 - px[k][c] as f64).powi(2);
        }
        e += (dec[k][3] as f64 - px[k][3] as f64).powi(2);
    }
    e
}

/// Mode 5: colour (7-bit endpoints, 2-bit indices) and alpha (8-bit endpoints, 2-bit indices)
/// fitted apart.
fn encode_mode5(px: &[[u8; 4]; 16]) -> [u8; 16] {
    const W2: [u32; 4] = [0, 21, 43, 64];
    let lerp = |a: u32, b: u32, w: u32| ((64 - w) * a + w * b + 32) >> 6;
    // colour: principal axis over the texels that are seen (alpha >= 8), else all
    let seen: Vec<usize> = (0..16).filter(|&k| px[k][3] >= 8).collect();
    let set: Vec<usize> = if seen.is_empty() { (0..16).collect() } else { seen };
    let mut mean = [0f32; 3];
    for &k in &set {
        for c in 0..3 {
            mean[c] += px[k][c] as f32 / set.len() as f32;
        }
    }
    let mut cov = [[0f32; 3]; 3];
    for &k in &set {
        let d: [f32; 3] = std::array::from_fn(|c| px[k][c] as f32 - mean[c]);
        for i in 0..3 {
            for j in 0..3 {
                cov[i][j] += d[i] * d[j];
            }
        }
    }
    let mut axis = [1.0f32, 1.0, 1.0];
    for _ in 0..8 {
        let n: [f32; 3] = std::array::from_fn(|i| (0..3).map(|j| cov[i][j] * axis[j]).sum());
        let len = n.iter().map(|v| v * v).sum::<f32>().sqrt();
        if len < 1e-6 {
            break;
        }
        axis = n.map(|v| v / len);
    }
    let (mut tmin, mut tmax) = (f32::MAX, f32::MIN);
    for &k in &set {
        let t: f32 = (0..3).map(|c| (px[k][c] as f32 - mean[c]) * axis[c]).sum();
        tmin = tmin.min(t);
        tmax = tmax.max(t);
    }
    let q = |v: f32| (v.clamp(0.0, 255.0) / 255.0 * 127.0).round() as u32;
    let mut c0: [u32; 3] = std::array::from_fn(|c| q(mean[c] + axis[c] * tmin));
    let mut c1: [u32; 3] = std::array::from_fn(|c| q(mean[c] + axis[c] * tmax));
    let ex = |v: u32| (v << 1) | (v >> 6);
    let pal = |c0: [u32; 3], c1: [u32; 3]| -> [[u32; 3]; 4] { std::array::from_fn(|i| std::array::from_fn(|c| lerp(ex(c0[c]), ex(c1[c]), W2[i]))) };
    let pick = |pal: &[[u32; 3]; 4], p: [u8; 4]| -> u8 {
        (0..4).min_by_key(|&i| (0..3).map(|c| (pal[i][c] as i32 - p[c] as i32).pow(2)).sum::<i32>()).unwrap() as u8
    };
    let pc = pal(c0, c1);
    let mut ci: [u8; 16] = std::array::from_fn(|k| pick(&pc, px[k]));
    let cerr = |c0: [u32; 3], c1: [u32; 3], ci: &[u8; 16]| -> i64 {
        let pl = pal(c0, c1);
        set.iter().map(|&k| (0..3).map(|c| (pl[ci[k] as usize][c] as i64 - px[k][c] as i64).pow(2)).sum::<i64>()).sum()
    };
    // least-squares refit of the colour endpoints to the indices (over the seen texels), twice
    for _ in 0..2 {
        let (mut aa, mut ab, mut bb) = (0f32, 0f32, 0f32);
        let (mut ax, mut bx) = ([0f32; 3], [0f32; 3]);
        for &k in &set {
            let t = W2[ci[k] as usize] as f32 / 64.0;
            let (a, b) = (1.0 - t, t);
            aa += a * a;
            ab += a * b;
            bb += b * b;
            for c in 0..3 {
                ax[c] += a * px[k][c] as f32;
                bx[c] += b * px[k][c] as f32;
            }
        }
        let det = aa * bb - ab * ab;
        if det.abs() < 1e-6 {
            break;
        }
        let n0: [u32; 3] = std::array::from_fn(|c| q((ax[c] * bb - bx[c] * ab) / det));
        let n1: [u32; 3] = std::array::from_fn(|c| q((bx[c] * aa - ax[c] * ab) / det));
        let np = pal(n0, n1);
        let ni: [u8; 16] = std::array::from_fn(|k| pick(&np, px[k]));
        if cerr(n0, n1, &ni) < cerr(c0, c1, &ci) {
            (c0, c1, ci) = (n0, n1, ni);
        } else {
            break;
        }
    }
    // alpha: its own endpoints and indices
    let (amin, amax) = px.iter().fold((255u32, 0u32), |(a, b), p| (a.min(p[3] as u32), b.max(p[3] as u32)));
    let (mut a0, mut a1) = (amin, amax);
    let apal: [u32; 4] = std::array::from_fn(|i| lerp(a0, a1, W2[i]));
    let mut ai: [u8; 16] = std::array::from_fn(|k| (0..4).min_by_key(|&i| (apal[i] as i32 - px[k][3] as i32).abs()).unwrap() as u8);
    // anchors: index 0's top bit clear
    if ci[0] >= 2 {
        std::mem::swap(&mut c0, &mut c1);
        ci = ci.map(|i| 3 - i);
    }
    if ai[0] >= 2 {
        std::mem::swap(&mut a0, &mut a1);
        ai = ai.map(|i| 3 - i);
    }
    let mut bits: u128 = 1 << 5; // mode 5, rotation 0
    let mut pos = 8;
    let mut put = |v: u128, n: u32, pos: &mut u32| {
        bits |= v << *pos;
        *pos += n;
    };
    for c in 0..3 {
        put(c0[c] as u128, 7, &mut pos);
        put(c1[c] as u128, 7, &mut pos);
    }
    put(a0 as u128, 8, &mut pos);
    put(a1 as u128, 8, &mut pos);
    put(ci[0] as u128, 1, &mut pos);
    for &i in &ci[1..] {
        put(i as u128, 2, &mut pos);
    }
    put(ai[0] as u128, 1, &mut pos);
    for &i in &ai[1..] {
        put(i as u128, 2, &mut pos);
    }
    debug_assert_eq!(pos, 128);
    bits.to_le_bytes()
}

/// Mode 6: colour and alpha on one line, 7-bit endpoints + p-bits, 4-bit indices.
fn encode_mode6(px: &[[u8; 4]; 16]) -> [u8; 16] {
    let f: Vec<[f32; 4]> = px.iter().map(|p| p.map(|v| v as f32)).collect();
    let mut mean = [0f32; 4];
    for p in &f {
        for c in 0..4 {
            mean[c] += p[c] / 16.0;
        }
    }
    // principal axis (power iteration on the covariance)
    let mut cov = [[0f32; 4]; 4];
    for p in &f {
        let d: [f32; 4] = std::array::from_fn(|c| p[c] - mean[c]);
        for i in 0..4 {
            for j in 0..4 {
                cov[i][j] += d[i] * d[j];
            }
        }
    }
    let mut axis = [1.0f32, 1.0, 1.0, 0.5];
    for _ in 0..8 {
        let n: [f32; 4] = std::array::from_fn(|i| (0..4).map(|j| cov[i][j] * axis[j]).sum());
        let len = n.iter().map(|v| v * v).sum::<f32>().sqrt();
        if len < 1e-6 {
            break;
        }
        axis = n.map(|v| v / len);
    }
    let (mut tmin, mut tmax) = (f32::MAX, f32::MIN);
    for p in &f {
        let t: f32 = (0..4).map(|c| (p[c] - mean[c]) * axis[c]).sum();
        tmin = tmin.min(t);
        tmax = tmax.max(t);
    }
    let mut e0: [f32; 4] = std::array::from_fn(|c| (mean[c] + axis[c] * tmin).clamp(0.0, 255.0));
    let mut e1: [f32; 4] = std::array::from_fn(|c| (mean[c] + axis[c] * tmax).clamp(0.0, 255.0));

    // endpoints, p-bits, indices, error
    type Candidate = ([u32; 4], [u32; 4], u32, u32, [u8; 16], u32);
    let mut best: Option<Candidate> = None;
    for round in 0..3 {
        // p-bits: all four combinations, keep the best
        for p0 in 0..2u32 {
            for p1 in 0..2u32 {
                let (q0, q1) = (quant(e0, p0), quant(e1, p1));
                let (idx, total) = assign(px, expand(q0, p0), expand(q1, p1));
                if best.as_ref().is_none_or(|b| total < b.5) {
                    best = Some((q0, q1, p0, p1, idx, total));
                }
            }
        }
        if round == 2 || best.as_ref().unwrap().5 == 0 {
            break;
        }
        // least-squares refit of the endpoints to the chosen indices
        let idx = best.as_ref().unwrap().4;
        let (mut aa, mut ab, mut bb) = (0f32, 0f32, 0f32);
        let (mut ax, mut bx) = ([0f32; 4], [0f32; 4]);
        for (k, p) in f.iter().enumerate() {
            let t = W4[idx[k] as usize] as f32 / 64.0;
            let (a, b) = (1.0 - t, t);
            aa += a * a;
            ab += a * b;
            bb += b * b;
            for c in 0..4 {
                ax[c] += a * p[c];
                bx[c] += b * p[c];
            }
        }
        let det = aa * bb - ab * ab;
        if det.abs() < 1e-6 {
            break;
        }
        e0 = std::array::from_fn(|c| ((ax[c] * bb - bx[c] * ab) / det).clamp(0.0, 255.0));
        e1 = std::array::from_fn(|c| ((bx[c] * aa - ax[c] * ab) / det).clamp(0.0, 255.0));
    }
    let (mut q0, mut q1, mut p0, mut p1, mut idx, _) = best.unwrap();
    // the anchor (texel 0) index must have its top bit clear: swap the endpoints if not
    if idx[0] >= 8 {
        std::mem::swap(&mut q0, &mut q1);
        std::mem::swap(&mut p0, &mut p1);
        idx = idx.map(|i| 15 - i);
    }
    pack(q0, q1, p0, p1, &idx)
}

fn pack(q0: [u32; 4], q1: [u32; 4], p0: u32, p1: u32, idx: &[u8; 16]) -> [u8; 16] {
    let mut bits: u128 = 1 << 6; // mode 6
    let mut pos = 7;
    let mut put = |v: u128, n: u32, pos: &mut u32| {
        bits |= v << *pos;
        *pos += n;
    };
    for c in 0..4 {
        put(q0[c] as u128, 7, &mut pos);
        put(q1[c] as u128, 7, &mut pos);
    }
    put(p0 as u128, 1, &mut pos);
    put(p1 as u128, 1, &mut pos);
    put(idx[0] as u128, 3, &mut pos);
    for &i in &idx[1..] {
        put(i as u128, 4, &mut pos);
    }
    debug_assert_eq!(pos, 128);
    bits.to_le_bytes()
}

/// Decode a mode-6 or mode-5 block (mode choice, tests and the quality check).
pub fn decode_block(b: &[u8; 16]) -> [[u8; 4]; 16] {
    let bits = u128::from_le_bytes(*b);
    let get = |pos: u32, n: u32| ((bits >> pos) & ((1u128 << n) - 1)) as u32;
    if bits & 0x7F == 1 << 5 {
        // mode 5, rotation 0
        const W2: [u32; 4] = [0, 21, 43, 64];
        let lerp = |a: u32, b: u32, w: u32| ((64 - w) * a + w * b + 32) >> 6;
        let ex = |v: u32| (v << 1) | (v >> 6);
        let c0: [u32; 3] = std::array::from_fn(|c| ex(get(8 + c as u32 * 14, 7)));
        let c1: [u32; 3] = std::array::from_fn(|c| ex(get(15 + c as u32 * 14, 7)));
        let (a0, a1) = (get(50, 8), get(58, 8));
        return std::array::from_fn(|k| {
            let ci = if k == 0 { get(66, 1) } else { get(67 + (k as u32 - 1) * 2, 2) };
            let ai = if k == 0 { get(97, 1) } else { get(98 + (k as u32 - 1) * 2, 2) };
            let w = W2[ci as usize];
            [lerp(c0[0], c1[0], w) as u8, lerp(c0[1], c1[1], w) as u8, lerp(c0[2], c1[2], w) as u8, lerp(a0, a1, W2[ai as usize]) as u8]
        });
    }
    let mut q0 = [0u32; 4];
    let mut q1 = [0u32; 4];
    for c in 0..4 {
        q0[c] = get(7 + c as u32 * 14, 7);
        q1[c] = get(14 + c as u32 * 14, 7);
    }
    let (p0, p1) = (get(63, 1), get(64, 1));
    let (e0, e1) = (expand(q0, p0), expand(q1, p1));
    std::array::from_fn(|k| {
        let i = if k == 0 { get(65, 3) } else { get(68 + (k as u32 - 1) * 4, 4) };
        interp(e0, e1, W4[i as usize]).map(|v| v as u8)
    })
}

/// Encode a whole RGBA8 image (`w` x `h`); edge blocks repeat the last row / column.
pub fn encode_image(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    let mut out = Vec::with_capacity((bw * bh * 16) as usize);
    for by in 0..bh {
        for bx in 0..bw {
            let px: [[u8; 4]; 16] = std::array::from_fn(|k| {
                let (x, y) = ((bx * 4 + k as u32 % 4).min(w - 1), (by * 4 + k as u32 / 4).min(h - 1));
                let i = ((y * w + x) * 4) as usize;
                [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
            });
            out.extend_from_slice(&encode_block(&px));
        }
    }
    out
}

/// PSNR (dB) of the encoded image against its source, over all four channels.
pub fn psnr(w: u32, h: u32, rgba: &[u8], blocks: &[u8]) -> f64 {
    let bw = w.div_ceil(4);
    let mut se = 0f64;
    for y in 0..h {
        for x in 0..w {
            let b = ((y / 4) * bw + x / 4) as usize;
            let dec = decode_block(blocks[b * 16..b * 16 + 16].try_into().unwrap());
            let d = dec[((y % 4) * 4 + x % 4) as usize];
            let i = ((y * w + x) * 4) as usize;
            for c in 0..4 {
                se += (d[c] as f64 - rgba[i + c] as f64).powi(2);
            }
        }
    }
    let mse = se / (w as f64 * h as f64 * 4.0);
    if mse == 0.0 {
        99.0
    } else {
        10.0 * (255.0 * 255.0 / mse).log10()
    }
}

/// BC4: one channel per 4x4 block in 8 bytes, 8 levels between two 8-bit endpoints (normal map
/// x / y as BC5, single data channels such as occlusion or translucency). Endpoints are the
/// block's range, then nudged inwards while that lowers the error.
pub fn bc4_block(v: &[u8; 16]) -> [u8; 8] {
    let (mn, mx) = v.iter().fold((255u8, 0u8), |(a, b), &x| (a.min(x), b.max(x)));
    let encode = |e0: u32, e1: u32| -> (u64, u32) {
        let pal: [u32; 8] = std::array::from_fn(|i| match i {
            0 => e0,
            1 => e1,
            k => ((8 - k as u32) * e0 + (k as u32 - 1) * e1 + 3) / 7,
        });
        let mut bits = (e0 as u64) | ((e1 as u64) << 8);
        let mut err = 0u32;
        for (k, &x) in v.iter().enumerate() {
            let (i, e) = (0..8).map(|i| (i, (pal[i] as i32 - x as i32).unsigned_abs())).min_by_key(|t| t.1).unwrap();
            bits |= (i as u64) << (16 + 3 * k);
            err += e * e;
        }
        (bits, err)
    };
    // 8-level mode needs e0 > e1
    let (hi, lo) = if mx == mn { (mx.max(1) as u32, mx.max(1) as u32 - 1) } else { (mx as u32, mn as u32) };
    let mut best = encode(hi, lo);
    for d0 in 0..3u32 {
        for d1 in 0..3u32 {
            let (a, b) = (hi.saturating_sub(d0), lo + d1);
            if a > b && (d0 | d1) != 0 {
                let r = encode(a, b);
                if r.1 < best.1 {
                    best = r;
                }
            }
        }
    }
    best.0.to_le_bytes()
}

pub fn bc4_decode(b: &[u8; 8]) -> [u8; 16] {
    let bits = u64::from_le_bytes(*b);
    let (e0, e1) = ((bits & 255) as u32, ((bits >> 8) & 255) as u32);
    let pal: [u32; 8] = std::array::from_fn(|i| match i {
        0 => e0,
        1 => e1,
        k if e0 > e1 => ((8 - k as u32) * e0 + (k as u32 - 1) * e1 + 3) / 7,
        k if k < 6 => ((6 - k as u32) * e0 + (k as u32 - 1) * e1 + 2) / 5,
        6 => 0,
        _ => 255,
    });
    std::array::from_fn(|k| pal[((bits >> (16 + 3 * k)) & 7) as usize] as u8)
}

/// Encode channels `chans` (1: BC4, 2: BC5) of an RGBA8 image; edge blocks repeat the border.
pub fn encode_channels(w: u32, h: u32, rgba: &[u8], chans: &[usize]) -> Vec<u8> {
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    let mut out = Vec::with_capacity((bw * bh * 8 * chans.len() as u32) as usize);
    for by in 0..bh {
        for bx in 0..bw {
            for &c in chans {
                let v: [u8; 16] = std::array::from_fn(|k| {
                    let (x, y) = ((bx * 4 + k as u32 % 4).min(w - 1), (by * 4 + k as u32 / 4).min(h - 1));
                    rgba[((y * w + x) * 4) as usize + c]
                });
                out.extend_from_slice(&bc4_block(&v));
            }
        }
    }
    out
}

/// PSNR (dB) of BC4 / BC5 data against the source channels.
pub fn psnr_channels(w: u32, h: u32, rgba: &[u8], blocks: &[u8], chans: &[usize]) -> f64 {
    let bw = w.div_ceil(4);
    let mut se = 0f64;
    for by in 0..h.div_ceil(4) {
        for bx in 0..bw {
            for (j, &c) in chans.iter().enumerate() {
                let o = (((by * bw + bx) as usize) * chans.len() + j) * 8;
                let dec = bc4_decode(blocks[o..o + 8].try_into().unwrap());
                for k in 0..16u32 {
                    let (x, y) = (bx * 4 + k % 4, by * 4 + k / 4);
                    if x < w && y < h {
                        se += (dec[k as usize] as f64 - rgba[((y * w + x) * 4) as usize + c] as f64).powi(2);
                    }
                }
            }
        }
    }
    let mse = se / (w as f64 * h as f64 * chans.len() as f64);
    if mse == 0.0 {
        99.0
    } else {
        10.0 * (255.0 * 255.0 / mse).log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_block_is_exact_or_nearly() {
        let px = [[120u8, 200, 33, 255]; 16];
        let d = decode_block(&encode_block(&px));
        for p in d {
            for c in 0..4 {
                assert!((p[c] as i32 - px[0][c] as i32).abs() <= 1, "{p:?}");
            }
        }
    }

    #[test]
    fn gradient_block_is_close() {
        let px: [[u8; 4]; 16] = std::array::from_fn(|k| [(k * 16) as u8, (255 - k * 16) as u8, (k * 8) as u8, 255]);
        let d = decode_block(&encode_block(&px));
        let e: f64 = (0..16).map(|k| (0..4).map(|c| (d[k][c] as f64 - px[k][c] as f64).powi(2)).sum::<f64>()).sum::<f64>() / 64.0;
        let psnr = 10.0 * (255.0f64 * 255.0 / e.max(1e-9)).log10();
        assert!(psnr > 38.0, "psnr {psnr}");
    }

    #[test]
    fn leaf_edge_keeps_its_cutout() {
        // a leaf card's edge: varied greens where opaque, cut out elsewhere
        let px: [[u8; 4]; 16] = std::array::from_fn(|k| {
            let g = 90 + ((k * 37) % 40) as u8;
            if (k % 4) + (k / 4) < 3 { [0, 0, 0, 0] } else { [40 + g / 4, g, 30, 255] }
        });
        let d = decode_block(&encode_block(&px));
        for k in 0..16 {
            // the cutout survives an alpha test at 0.5 exactly
            assert_eq!(d[k][3] >= 128, px[k][3] >= 128, "texel {k}: {:?} vs {:?}", d[k], px[k]);
            if px[k][3] == 255 {
                assert!((d[k][1] as i32 - px[k][1] as i32).abs() <= 16, "texel {k} green {} vs {}", d[k][1], px[k][1]);
            }
        }
    }

    #[test]
    fn normal_map_channels_survive_bc5() {
        // x and y of a bumpy normal map vary independently
        let (w, h) = (64u32, 64u32);
        let mut s = 7u32;
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                let n = (s >> 26) as i32 - 32;
                rgba.extend_from_slice(&[(128 + ((x as f32 * 0.7).sin() * 60.0) as i32 + n).clamp(0, 255) as u8, (128 + ((y as f32 * 0.5).cos() * 60.0) as i32 - n).clamp(0, 255) as u8, 255, 255]);
            }
        }
        let b = encode_channels(w, h, &rgba, &[0, 1]);
        let p = psnr_channels(w, h, &rgba, &b, &[0, 1]);
        assert!(p > 35.0, "psnr {p}"); // +-32 random per texel: far harsher than real normal maps
    }

    #[test]
    fn noisy_image_is_visually_lossless() {
        // a textured image (smooth variation plus fine noise), like a ground scan
        let (w, h) = (64u32, 64u32);
        let mut s = 1u32;
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                let n = (s >> 24) as i32 % 24 - 12;
                let base = [90 + x as i32, 70 + y as i32, 50 + (x + y) as i32 / 2];
                for c in base {
                    rgba.push((c + n).clamp(0, 255) as u8);
                }
                rgba.push(255);
            }
        }
        let b = encode_image(w, h, &rgba);
        let p = psnr(w, h, &rgba, &b);
        assert!(p > 38.0, "psnr {p}");
    }
}

