//! Where the analytic lights reach: a hash grid over the ground plane (x, z), each cell listing
//! the lights whose range touches it (rt.glsl `LightGrid`, pt.rgen `sampleLights`).
//!
//! Next-event estimation picks a light for each path vertex from a few candidates. Drawn from
//! every light in the town, a classroom's four ceiling lights were among the eight candidates
//! for only a third of its pixels, and the rest got no lamp light that frame: interiors were
//! noisy, and blotchy once denoised. From the cell's own list (often short enough to try every
//! light in it), each candidate is a light that can reach the point.

use crate::scene::LightGpu;

/// Cell size (m).
pub(super) const CELL: f32 = 4.0;
/// Hash table slots (a power of two).
const SLOTS: usize = 8192;
/// Words before the table: cell size (f32 bits), slot mask, where the light lists start, pad.
const HEAD: usize = 4;

fn slot_of(ix: i32, iz: i32) -> usize {
    ((ix as u32).wrapping_mul(73856093) ^ (iz as u32).wrapping_mul(19349663)) as usize & (SLOTS - 1)
}

/// The grid for these lights, as the words of rt.glsl's `LightGrid` buffer: the head, the table
/// (per slot: cell x, cell z, first index, count; count 0 = empty), then the lists of light
/// indices.
pub(super) fn build(lights: &[LightGpu]) -> Vec<u32> {
    let mut cells: std::collections::HashMap<(i32, i32), Vec<u32>> = Default::default();
    for (i, l) in lights.iter().enumerate() {
        let (x, z, range) = (l.pos_radius[0], l.pos_radius[2], l.color[3].max(l.pos_radius[3]));
        let (x0, x1) = (((x - range) / CELL).floor() as i32, ((x + range) / CELL).floor() as i32);
        let (z0, z1) = (((z - range) / CELL).floor() as i32, ((z + range) / CELL).floor() as i32);
        for iz in z0..=z1 {
            for ix in x0..=x1 {
                // (only cells the range circle touches: nearest point of the cell)
                let nx = x.clamp(ix as f32 * CELL, (ix + 1) as f32 * CELL);
                let nz = z.clamp(iz as f32 * CELL, (iz + 1) as f32 * CELL);
                if (nx - x).hypot(nz - z) <= range {
                    cells.entry((ix, iz)).or_default().push(i as u32);
                }
            }
        }
    }
    let base = HEAD + SLOTS * 4;
    let mut w = vec![0u32; base];
    w[0] = CELL.to_bits();
    w[1] = (SLOTS - 1) as u32;
    w[2] = base as u32;
    // fullest cells first, so they sit nearest their hash slot; a full table drops the rest
    let mut list: Vec<((i32, i32), Vec<u32>)> = cells.into_iter().collect();
    list.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    let mut placed = 0;
    for ((ix, iz), idx) in list {
        if placed >= SLOTS * 3 / 4 {
            break;
        }
        let mut s = slot_of(ix, iz);
        while w[HEAD + s * 4 + 3] != 0 {
            s = (s + 1) & (SLOTS - 1);
        }
        let e = HEAD + s * 4;
        w[e] = ix as u32;
        w[e + 1] = iz as u32;
        w[e + 2] = (w.len() - base) as u32;
        w[e + 3] = idx.len() as u32;
        w.extend_from_slice(&idx);
        placed += 1;
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(x: f32, z: f32, range: f32) -> LightGpu {
        LightGpu { pos_radius: [x, 3.0, z, 0.2], color: [1.0, 1.0, 1.0, range], spot: [0.0, -1.0, 0.0, -2.0] }
    }

    /// The cell's list as the shader finds it (pt.rgen lightCell).
    fn lookup(w: &[u32], x: f32, z: f32) -> Vec<u32> {
        let (ix, iz) = ((x / CELL).floor() as i32, (z / CELL).floor() as i32);
        let mut s = slot_of(ix, iz);
        loop {
            let e = HEAD + s * 4;
            if w[e + 3] == 0 {
                return Vec::new();
            }
            if w[e] as i32 == ix && w[e + 1] as i32 == iz {
                let o = w[2] as usize + w[e + 2] as usize;
                return w[o..o + w[e + 3] as usize].to_vec();
            }
            s = (s + 1) & (SLOTS - 1);
        }
    }

    #[test]
    fn cells_list_the_lights_in_reach() {
        let ls = [light(0.0, 0.0, 8.0), light(30.0, 0.0, 8.0), light(-200.0, 512.0, 25.0)];
        let w = build(&ls);
        assert_eq!(lookup(&w, 1.0, 1.0), vec![0]);
        assert_eq!(lookup(&w, 31.0, -2.0), vec![1]);
        assert_eq!(lookup(&w, 15.0, 0.0), Vec::<u32>::new());
        assert_eq!(lookup(&w, -190.0, 500.0), vec![2]);
        // every point within a light's range finds it
        for (i, l) in ls.iter().enumerate() {
            for k in 0..64 {
                let a = k as f32 * 0.098;
                let r = l.color[3] * 0.99 * ((k % 7) as f32 / 6.0);
                let (x, z) = (l.pos_radius[0] + r * a.cos(), l.pos_radius[2] + r * a.sin());
                assert!(lookup(&w, x, z).contains(&(i as u32)), "light {i} at ({x}, {z})");
            }
        }
    }
}
