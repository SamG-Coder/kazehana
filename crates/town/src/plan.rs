//! `assets/town/town.json`, as written by tools/blender/town_plan.py.

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Plan {
    pub name: String,
    pub extent: [f32; 4],
    pub terrain: Terrain,
    pub roads: Vec<Road>,
    pub rail: Vec<[f32; 2]>,
    /// the track's level (ballast top) at each rail point
    #[serde(default)]
    pub rail_h: Vec<f32>,
    pub river: River,
    pub sites: Vec<Site>,
    pub houses: Vec<House>,
    pub home: usize,
    pub school_gate: [f32; 2],
    pub paddies: Vec<Vec<[f32; 2]>>,
    pub paddy_levels: Vec<f32>,
    #[serde(default)]
    pub hatake: Vec<Vec<[f32; 2]>>,
    pub poles: Vec<[f32; 2]>,
    /// (x, y, species, height m)
    pub trees: Vec<(f32, f32, String, f32)>,
    pub route_to_school: Vec<[f32; 2]>,
}

#[derive(Deserialize)]
pub struct Terrain {
    pub file: String,
    pub landuse: String,
    pub origin: [f32; 2],
    pub cell: f32,
    pub nx: usize,
    pub ny: usize,
}

#[derive(Deserialize)]
pub struct Road {
    pub name: String,
    pub kind: String,
    pub width: f32,
    pub pts: Vec<[f32; 2]>,
    /// Graded height at each point.
    pub h: Option<Vec<f32>>,
}

#[derive(Deserialize)]
pub struct River {
    pub pts: Vec<[f32; 2]>,
    /// Water level at each point.
    pub water: Vec<f32>,
    pub water_half: f32,
    pub levee_half: f32,
}

#[derive(Deserialize)]
pub struct Building {
    pub name: String,
    pub poly: Vec<[f32; 2]>,
    pub floors: u32,
}

#[derive(Deserialize)]
pub struct Site {
    pub name: String,
    pub kind: String,
    pub poly: Vec<[f32; 2]>,
    pub ground: Option<f32>,
    pub buildings: Vec<Building>,
}

#[derive(Deserialize)]
pub struct House {
    pub id: u16,
    pub district: String,
    pub kind: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub name: Option<String>,
    pub plot: Vec<[f32; 2]>,
    pub house: Vec<[f32; 2]>,
    pub parking: Option<Vec<[f32; 2]>>,
    pub shed: Option<Vec<[f32; 2]>>,
    pub entrance: [f32; 2],
    pub gate: [f32; 2],
    pub yaw: f32,
    pub garden: String,
    pub floors: u32,
    pub roof: String,
    pub front: String,
    pub households: u32,
    pub road: String,
    pub ground: f32,
    #[serde(default)]
    pub retaining: f32,
    #[serde(default)]
    pub boundary: Boundary,
}

/// How a plot is closed (tools/blender/town_plan.py `boundaries`).
#[derive(Deserialize, Default)]
pub struct Boundary {
    pub segments: Vec<Segment>,
    pub posts: Vec<[f32; 2]>,
    pub mailbox: Option<Mailbox>,
}

#[derive(Deserialize)]
pub struct Segment {
    /// block, hedge or stone
    pub kind: String,
    pub a: [f32; 2],
    pub b: [f32; 2],
}

#[derive(Deserialize)]
pub struct Mailbox {
    pub at: [f32; 2],
    pub yaw: f32,
}
