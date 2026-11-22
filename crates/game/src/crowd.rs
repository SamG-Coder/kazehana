//! The townspeople the world draws: whoever the census schedule puts near you, where their place
//! is (sim::presence: who's in a room; the town's layouts: where its seats and spots are;
//! town::people: their models and animations).
//!
//! So far a class's homeroom: 1-A's students at their desks, the teacher at the front. Each
//! person's model is loaded the first time they're needed. None of the people are approved yet:
//! they're shown "in review" (as unapproved buildings are) from the people builder's output
//! (assets/cache/people), so they can be judged in place.
//!
//! Animations: seated people play `sit` / `sit_write`, the teacher `teach` (the anime people
//! sessions' clips: the seat at 0.42 m, the desk's edge 0.18 m in front). Until a model has
//! `sit`, its student stands in the aisle beside their desk rather than in it; until it has
//! `teach`, the teacher plays `idle`, facing the class. With `teach` the teacher is placed facing
//! the board: the clip turns them round to the class and back.

use crate::game::Game;
use akari::glam::{Affine3A, Quat, Vec3};
use akari::{Instance, Renderer};
use sim::presence::{Doing, Spot};
use std::collections::HashMap;
use std::path::PathBuf;
use town::people::{CharacterModel, Person};

/// How near the room you have to be for its people to be drawn (m).
const NEAR: f32 = 60.0;

/// Someone placed: who, where (their model's frame), what they're doing, and whether they're in
/// a seat (to sit, if their model can).
struct Placed {
    person: u32,
    seat: Option<Affine3A>,
    stand: Affine3A,
    doing: Doing,
}

#[derive(Default)]
pub struct Crowd {
    /// each person's model, once loaded (None: couldn't be)
    models: HashMap<u32, Option<CharacterModel>>,
    people: HashMap<u32, Person>,
    here: Vec<Placed>,
    /// the game minute `here` was worked out for
    at: Option<(sim::Date, u32)>,
    told: bool,
}

fn model_path(id: u32) -> PathBuf {
    PathBuf::from(format!("assets/cache/people/p{id}/p{id}.gltf"))
}

/// The clip a person plays for what they're doing (the first their model has).
fn clips(doing: Doing) -> &'static [&'static str] {
    match doing {
        Doing::Working => &["sit_write", "sit", "idle"],
        Doing::Sitting | Doing::Eating => &["sit", "idle"],
        Doing::Teaching => &["teach", "idle"],
        Doing::Talking => &["idle"],
    }
}

impl Crowd {
    pub fn new() -> Self {
        Self::default()
    }

    /// Who's where now: Uncle Kenji behind Tsukumodō's counter while he's running the shop, and
    /// 1-A's homeroom from the schedule, placed on the town's spots.
    fn place(g: &Game) -> Vec<Placed> {
        let mut out = Self::kenji(g);
        out.extend(Self::homeroom(g));
        out
    }

    fn kenji(g: &Game) -> Vec<Placed> {
        let Some((counter, front)) = g.story_rt.counter else { return Vec::new() };
        if (counter - g.player.feet).with_y(0.0).length() > NEAR {
            return Vec::new();
        }
        let c = sim::school::census();
        let Some(k) = c.people.iter().find(|p| matches!(p.work, census::Work::Kenji)) else { return Vec::new() };
        let day = sim::presence::day(g.state.time.date);
        let a = census::schedule::activity(c, k.id, day, g.state.time.minute as u16);
        if a.doing != "running Tsukumodō" {
            return Vec::new();
        }
        // behind the counter, facing the shop's door (a glTF character faces +Z)
        let at = counter - front * 0.75;
        let stand = Affine3A::from_rotation_translation(Quat::from_rotation_y(front.x.atan2(front.z)), Vec3::new(at.x, counter.y, at.z));
        vec![Placed { person: k.id, seat: None, stand, doing: Doing::Talking }]
    }

    fn homeroom(g: &Game) -> Vec<Placed> {
        let room = "1-A";
        let spot = |name: &str| g.spots.iter().find(|(r, s, _)| r == room && s == name).map(|x| x.2);
        let Some(front) = spot("front") else { return Vec::new() };
        if (front.translation.x - g.player.feet.x).hypot(front.translation.z - g.player.feet.z) > NEAR {
            return Vec::new();
        }
        let t = g.state.time;
        // (a glTF character faces +Z: turned round to face the desk's front, -Z)
        let turn = Affine3A::from_quat(Quat::from_rotation_y(std::f32::consts::PI));
        sim::presence::in_homeroom(sim::school::MY_CLASS, t.date, t.minute)
            .into_iter()
            .filter_map(|p| match p.spot {
                Spot::Seat(n) => {
                    let desk = spot(&format!("seat:{n}"))?;
                    // on the chair 0.40 m behind the desk; or standing in the aisle beside it
                    let seat = desk * Affine3A::from_translation(Vec3::new(0.0, 0.0, 0.40)) * turn;
                    let stand = desk * Affine3A::from_translation(Vec3::new(0.50, 0.0, 0.25)) * turn;
                    Some(Placed { person: p.person, seat: Some(seat), stand, doing: p.doing })
                }
                Spot::Front => Some(Placed { person: p.person, seat: None, stand: front, doing: p.doing }),
            })
            .collect()
    }

    /// This frame: who's here, animated, into `out`.
    pub fn frame(&mut self, r: &mut Renderer, g: &Game, dt: f32, out: &mut Vec<Instance>) -> anyhow::Result<()> {
        if !g.in_town {
            return Ok(());
        }
        let now = (g.state.time.date, g.state.time.minute);
        if self.at != Some(now) {
            self.here = Self::place(g);
            self.at = Some(now);
            let ids: Vec<u32> = self.here.iter().map(|p| p.person).collect();
            self.people.retain(|id, _| ids.contains(id));
        }
        for p in &self.here {
            let m = self.models.entry(p.person).or_insert_with(|| {
                let path = model_path(p.person);
                match CharacterModel::load(r, &path) {
                    Ok(m) => Some(m),
                    Err(e) => {
                        eprintln!("crowd: p{} not loaded: {e:#}", p.person);
                        None
                    }
                }
            });
            let Some(m) = m.as_ref() else { continue };
            if !self.told {
                println!("crowd: people shown in review (not yet approved: assets/cache/people)");
                self.told = true;
            }
            let clip = clips(p.doing).iter().find_map(|c| m.clip(c));
            let sits = p.seat.is_some() && m.clip("sit").is_some();
            let mut at = if sits { p.seat.unwrap() } else { p.stand };
            // (teaching: placed facing the board; the clip turns them round to the class and back)
            if p.doing == Doing::Teaching && m.clip("teach").is_some() {
                at *= Affine3A::from_quat(Quat::from_rotation_y(std::f32::consts::PI));
            }
            let person = self.people.entry(p.person).or_insert_with(|| {
                let mut x = Person::new(at, clip);
                // (each at a different moment of the clip)
                x.time = (p.person % 17) as f32 * 0.7;
                x
            });
            person.at = at;
            person.play(clip, 0.3);
            person.frame(r, m, dt, out)?;
        }
        Ok(())
    }
}
