//! Paper in the world (docs/life_plan.md: information lives on things, not menus): the wall
//! calendar hanging in Tsukumodō and the journal you open in your hands. Their pages are drawn
//! from the game state by the `pages` crate and put on the props' `kzpage_*` faces as textures,
//! drawn again only when what's on them changes.

use akari::glam::{Affine3A, Quat, Vec3};
use akari::gltf_load::Model;
use akari::textures::{TexJob, TexKind, TexSource};
use akari::{Instance, Material, MaterialId, Renderer, TextureId};
use anyhow::{anyhow, Result};
use sim::GameState;
use std::hash::{Hash, Hasher};

/// A prop's page: its material (the `kzpage_*` node's) and the texture the game draws on it.
struct Page {
    tex: TextureId,
    mat: MaterialId,
    base: Material,
    key: Option<u64>,
}

impl Page {
    fn new(r: &mut Renderer, model: &Model) -> Result<Self> {
        let part = model.parts.iter().find(|p| p.node.starts_with("kzpage")).ok_or_else(|| anyhow!("{}: no kzpage_ node", model.name))?;
        let base = model.materials.iter().find(|m| m.0 == part.material).map(|m| m.2).ok_or_else(|| anyhow!("{}: the page has no material", model.name))?;
        let tex = r.add_textures(&[TexJob { key: format!("page {}", model.name), name: format!("{} page", model.name), kind: TexKind::Page, source: TexSource::Rgba(1, 1, vec![240, 236, 228, 255]) }])?[0];
        Ok(Page { tex, mat: part.material, base, key: None })
    }

    /// Show a page: drawn (by `draw`) only if `key` has changed since the last time.
    fn show(&mut self, r: &mut Renderer, key: u64, draw: impl FnOnce() -> pages::Canvas) -> Result<()> {
        if self.key == Some(key) {
            return Ok(());
        }
        let c = draw();
        r.replace_texture(self.tex, &TexJob { key: format!("page {key:x}"), name: "page".into(), kind: TexKind::Page, source: TexSource::Rgba(c.w as u32, c.h as u32, c.to_srgb8()) })?;
        r.set_material(self.mat, Material { base_tex: Some(self.tex), base_color: [1.0, 1.0, 1.0, 1.0], ..self.base });
        self.key = Some(key);
        Ok(())
    }
}

fn hash(parts: impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

/// Which spread of the journal is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spread {
    Diary,
    People,
    Plans,
}

impl Spread {
    pub const ALL: [Spread; 3] = [Spread::Diary, Spread::People, Spread::Plans];
    pub fn turn(self, by: i32) -> Spread {
        let i = Spread::ALL.iter().position(|s| *s == self).unwrap() as i32;
        Spread::ALL[(i + by).rem_euclid(3) as usize]
    }
}

pub struct Paper {
    calendar: Option<(Model, Page, Affine3A)>,
    journal: Option<(Model, Page)>,
    /// your work on your desk at school: the lesson's print and the subject's notebook
    worksheet: Option<(Model, Page)>,
    notebook: Option<(Model, Page)>,
}

/// Your paper on your desk (sim::lessons::paper): where the desk is, the print on it (the one
/// you're working on, or the one handed back), and the subject's notebook.
pub struct DeskPaper {
    /// the school_desk prop's frame in the world
    pub desk: Affine3A,
    pub sheet: Option<sim::lessons::paper::Sheet>,
    pub notes: Option<(sim::school::Subject, Vec<sim::lessons::paper::Notes>)>,
}

/// Where a thing lies on the desk's top: Blender (x, y) on the top, turned `yaw` degrees, as the
/// desk's glTF frame (the top is 0.70 m up).
pub fn on_desk(x: f32, y: f32, yaw: f32) -> Affine3A {
    Affine3A::from_rotation_translation(Quat::from_rotation_y(yaw.to_radians()), Vec3::new(x, 0.70, -y))
}
/// The print at the left of the desk, the notebook open beside it.
pub const SHEET_ON_DESK: (f32, f32, f32) = (-0.185, -0.03, 4.0);
pub const NOTEBOOK_ON_DESK: (f32, f32, f32) = (0.135, 0.02, -2.0);

#[derive(serde::Deserialize)]
struct Spot {
    at: [f32; 3],
    yaw: f32,
}

#[derive(serde::Deserialize)]
struct PaperFile {
    calendar: Spot,
}

impl Paper {
    /// Load the props (approved ones only) and hang the calendar in Tsukumodō (`home`: its frame).
    pub fn load(r: &mut Renderer, home: Option<Affine3A>) -> Self {
        let reg = assets::Registry::open(assets::REGISTRY_PATH);
        let mut model = |id: &str| -> Option<Model> {
            match reg.as_ref().map_err(|e| anyhow!("{e:#}")).and_then(|g| g.load_model(r, id)) {
                Ok(m) => Some(m),
                Err(e) => {
                    eprintln!("{id} not loaded: {e:#}");
                    None
                }
            }
        };
        let cal = model("wall_calendar");
        let jour = model("journal_open");
        let sheet_m = model("worksheet");
        let book_m = model("notebook_open");
        let spot: Option<PaperFile> = std::fs::read("assets/buildings/shop_tsukumodo/paper.json").ok().and_then(|b| serde_json::from_slice(&b).ok());
        let calendar = match (cal, home, spot) {
            (Some(m), Some(home), Some(f)) => {
                // (Blender x, y, z -> glTF x, z, -y; yaw about Blender +Z is about +Y)
                let s = f.calendar;
                let local = Affine3A::from_rotation_translation(Quat::from_rotation_y(s.yaw.to_radians()), Vec3::new(s.at[0], s.at[2], -s.at[1]));
                Page::new(r, &m).map_err(|e| eprintln!("calendar: {e:#}")).ok().map(|p| (m, p, home * local))
            }
            _ => None,
        };
        let journal = jour.and_then(|m| Page::new(r, &m).map_err(|e| eprintln!("journal: {e:#}")).ok().map(|p| (m, p)));
        let worksheet = sheet_m.and_then(|m| Page::new(r, &m).map_err(|e| eprintln!("worksheet: {e:#}")).ok().map(|p| (m, p)));
        let notebook = book_m.and_then(|m| Page::new(r, &m).map_err(|e| eprintln!("notebook: {e:#}")).ok().map(|p| (m, p)));
        Paper { calendar, journal, worksheet, notebook }
    }

    /// Which way the calendar's page faces (world), if it hangs.
    pub fn calendar_facing(&self) -> Option<Vec3> {
        self.calendar.as_ref().map(|(_, _, at)| at.transform_vector3(Vec3::Z).normalize())
    }

    /// Where the calendar hangs (its middle), if it does.
    pub fn calendar_at(&self) -> Option<Vec3> {
        self.calendar.as_ref().map(|(m, _, at)| at.transform_point3((m.min + m.max) * 0.5))
    }

    /// The month the calendar shows, `offset` months on from today's: (year, month).
    pub fn calendar_month(g: &GameState, offset: i32) -> (i32, u32) {
        let d = g.time.date;
        let m = d.month as i32 - 1 + offset;
        (d.year + m.div_euclid(12), (m.rem_euclid(12) + 1) as u32)
    }

    /// This frame: the calendar on its wall (flipped `calendar_offset` months on), and the journal
    /// if it's open (`held`: which spread, and where in front of you), their pages up to date.
    pub fn frame(&mut self, r: &mut Renderer, g: &GameState, calendar_offset: i32, held: Option<(Spread, Affine3A)>, desk: Option<&DeskPaper>, out: &mut Vec<Instance>) -> Result<()> {
        if let Some(d) = desk {
            if let (Some(sheet), Some((m, page))) = (&d.sheet, &mut self.worksheet) {
                let key = hash(serde_json::to_string(sheet).unwrap_or_default());
                page.show(r, key, || pages::school::worksheet(sheet, "九十九"))?;
                let (x, y, yaw) = SHEET_ON_DESK;
                out.extend(m.instances(d.desk * on_desk(x, y, yaw)));
            }
            if let (Some((subject, notes)), Some((m, page))) = (&d.notes, &mut self.notebook) {
                let key = hash((format!("{subject:?}"), serde_json::to_string(notes).unwrap_or_default()));
                page.show(r, key, || pages::school::notebook(*subject, notes))?;
                let (x, y, yaw) = NOTEBOOK_ON_DESK;
                out.extend(m.instances(d.desk * on_desk(x, y, yaw)));
            }
        }
        let (year, month) = Self::calendar_month(g, calendar_offset);
        let today = g.time.date;
        let plans: Vec<(sim::Date, String)> = g.life.journal.plans.iter().map(|p| (p.date, p.text.clone())).collect();
        if let Some((m, page, at)) = &mut self.calendar {
            let key = hash((year, month, today, &plans));
            page.show(r, key, || pages::calendar::month(year, month, Some(today), &plans))?;
            out.extend(m.instances(*at));
        }
        if let (Some((spread, at)), Some((m, page))) = (held, &mut self.journal) {
            let j = &g.life.journal;
            let known: u32 = g.bonds.iter().map(|b| b.points).sum();
            let key = hash((spread as u8, today, j.diary.len(), j.diary.last().map(|d| d.lines.len()), j.asks.len(), j.plans.len(), known, g.uncle.points));
            page.show(r, key, || match spread {
                Spread::Diary => pages::journal::diary(g),
                Spread::People => pages::journal::people(g),
                Spread::Plans => pages::journal::asks_and_plans(g),
            })?;
            out.extend(m.instances(at));
        }
        Ok(())
    }
}

/// The journal held open in front of you: below your eyes, tilted up towards your face. Returns
/// where it goes, and the pitch to look down at it.
pub fn hold(eye: Vec3, yaw: f32) -> (Affine3A, f32) {
    let pitch = -38f32.to_radians();
    let fwd = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
    let dir = fwd * pitch.cos() + Vec3::Y * pitch.sin();
    let centre = eye + dir * 0.27;
    // (the book lies in the model's XZ plane, its top edge towards -Z: stand it up towards you)
    let tilt = std::f32::consts::FRAC_PI_2 + pitch + 0.05;
    let at = Affine3A::from_translation(centre) * Affine3A::from_rotation_y(-yaw) * Affine3A::from_rotation_x(tilt) * Affine3A::from_translation(Vec3::new(0.0, -0.01, 0.0));
    let look = (centre - eye).normalize();
    (at, look.y.asin())
}
