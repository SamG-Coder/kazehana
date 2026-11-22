//! `--gridtour`: the whole town photographed from straight above, cell by cell (hidden window),
//! for reviewing what's built where against the census. Each cell is 200 x 112.5 m (16:9), the
//! camera 88 m over the ground (65 degree field of view): its photo is the cell edge to edge.
//! Shots go to shots/grid/r<row>_c<col>.png, north up, and the cells' plan bounds to
//! shots/grid/cells.txt (row col x0 y0 x1 y1).

use crate::game::Game;
use akari::glam::Vec3;

pub const CELL_W: f32 = 200.0;
pub const CELL_H: f32 = 112.5;
/// the town's lived-in part (plan x, y): the houses run -1475..1511 east-west, -495..767
pub const X0: f32 = -1500.0;
pub const Y0: f32 = -562.5;
pub const COLS: u32 = 15;
pub const ROWS: u32 = 12;

pub struct GridTour {
    k: u32,
    wait: u32,
    only: Option<Vec<(u32, u32)>>,
    pub done: bool,
}

impl GridTour {
    /// `only`: "r,c;r,c" to reshoot just those cells
    pub fn new(only: Option<&str>) -> Self {
        let only = only.map(|s| s.split(';').filter_map(|rc| rc.split_once(',').and_then(|(r, c)| Some((r.trim().parse().ok()?, c.trim().parse().ok()?)))).collect());
        Self { k: 0, wait: 40, only, done: false }
    }

    /// The plan bounds of a cell (row 0 the northernmost).
    pub fn bounds(r: u32, c: u32) -> [f32; 4] {
        let x0 = X0 + c as f32 * CELL_W;
        let y1 = Y0 + (ROWS - r) as f32 * CELL_H;
        [x0, y1 - CELL_H, x0 + CELL_W, y1]
    }

    /// Before each frame: the next cell when it's time; a screenshot name to take now.
    pub fn frame(&mut self, g: &mut Game) -> Option<String> {
        if self.done {
            return None;
        }
        g.hold_still = true;
        g.no_hud = true;
        if self.wait > 0 {
            self.wait -= 1;
            return None;
        }
        self.wait = 24;
        if self.k == 0 {
            let _ = std::fs::create_dir_all("shots/grid");
            let rows: Vec<String> = (0..ROWS * COLS).map(|k| {
                let (r, c) = (k / COLS, k % COLS);
                let b = Self::bounds(r, c);
                format!("{r} {c} {} {} {} {}", b[0], b[1], b[2], b[3])
            }).collect();
            let _ = std::fs::write("shots/grid/cells.txt", rows.join("\n"));
        }
        loop {
            if self.k >= ROWS * COLS {
                self.done = true;
                return None;
            }
            let (r, c) = (self.k / COLS, self.k % COLS);
            self.k += 1;
            if self.only.as_ref().is_some_and(|o| !o.contains(&(r, c))) {
                continue;
            }
            let b = Self::bounds(r, c);
            let (px, py) = ((b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5);
            let ground = g.world.as_ref().and_then(|w| w.world.ground(px, -py)).unwrap_or(22.0);
            // north (plan +y, world -z) up the picture: facing north, looking straight down
            let eye = Vec3::new(px, ground + 88.3, -py);
            g.place_player(eye - Vec3::Y * crate::game::EYE, 0.0, -1.5707);
            g.cam.pitch = -std::f32::consts::FRAC_PI_2 + 1e-3;
            g.cam.position = eye;
            g.cut = true;
            return Some(format!("r{r:02}_c{c:02}"));
        }
    }
}
