//! Kazehana-chō's people, as one database.
//!
//! Every house on the town plan (`assets/town/town.json`) holds a household (an apartment block
//! holds four flats). Every person has a name, an age, a household, their parents and spouse
//! (whichever household those live in, or away in the cities), a job or school, a daily routine
//! and a look. Groups tie people across households: school friend groups and clubs, the
//! neighbourhood associations (han), mothers' circles, the seniors' club and the gateball team,
//! the fire brigade.
//!
//! The database is generated once from a seed and the town plan ([`generate`]) and saved as
//! `assets/town/census.json`; `docs/census.md` is the same data as a document, house by house
//! ([`doc::markdown`]). Both are written by `cargo run -p census --example build`.
//!
//! How the town is made up (the population plan) is in `docs/population_plan.md`.

pub mod doc;
mod gen;
pub mod look;
mod names;
pub mod schedule;
pub mod school;

pub use gen::{generate, Plan};
pub use look::Look;
pub use schedule::{Activity, Day};

use serde::{Deserialize, Serialize};
use std::path::Path;

pub type PersonId = u32;
pub type HouseId = u16;
pub type HouseholdId = u16;
pub type PlaceId = u16;
pub type GroupId = u16;

/// The date ages are counted at: the start of the school year.
pub const CENSUS_DATE: (u16, u8, u8) = (2026, 4, 1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sex {
    M,
    F,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HouseKind {
    /// Tsukumodō: Uncle Kenji's shop, and your home.
    ShopHome,
    /// An old farmhouse: often three generations under one roof.
    Farmhouse,
    /// A two-storey modern house.
    Modern,
    /// A one-storey small house.
    Small,
    /// A shop on the ground floor, the family upstairs.
    Shophouse,
    /// A two-storey block of four flats.
    Apartment,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct House {
    pub id: HouseId,
    pub district: String,
    pub kind: HouseKind,
    /// The middle of the building (town plan metres: x east, y north).
    pub at: [f32; 2],
    pub households: Vec<HouseholdId>,
    /// The home as a place (schedules say "home" with this).
    pub place: PlaceId,
    /// For shophouses: what the shop sells, and whether it still opens.
    pub shop: Option<Shop>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shop {
    pub trade: String,
    pub open: bool,
    /// The shop as a place (its keepers work here).
    pub place: PlaceId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Household {
    pub id: HouseholdId,
    pub house: HouseId,
    pub family_name: String,
    /// The head first, then by age.
    pub members: Vec<PersonId>,
    /// The neighbourhood association block (a group) the household belongs to.
    pub han: GroupId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceKind {
    Home,
    Shop,
    JuniorHigh,
    Elementary,
    Station,
    StationPlaza,
    Shrine,
    InariShrine,
    Temple,
    PostOffice,
    TownOffice,
    Konbini,
    CoOp,
    Petrol,
    Drugstore,
    CarRepair,
    /// A household's paddies and vegetable plots (the farmer's own, near the house).
    Fields,
    /// Out of town: work, high school or shopping in Minato-shi (by train or car).
    MinatoShi,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Place {
    pub id: PlaceId,
    pub name: String,
    pub kind: PlaceKind,
    pub house: Option<HouseId>,
    /// Town plan metres (the site's middle).
    pub at: [f32; 2],
}

/// What someone does with their days.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Work {
    /// Under three: at home, carried, at the nursery in Minato-shi.
    Baby,
    /// Three to five: kindergarten in Minato-shi (the bus), or at home.
    Preschool,
    Elementary { grade: u8 },
    JuniorHigh { grade: u8, class: char, club: Option<Club> },
    /// High school in Minato-shi, by train.
    HighSchool { grade: u8 },
    /// Away at university or working in a city; home at New Year and Obon.
    AwayStudying,
    Shopkeeper,
    Farmer,
    /// A job at a place in town (hours in `Person::hours`).
    Local,
    /// Works in Minato-shi (train or car).
    Commuter,
    Teacher { school: PlaceKind, role: String },
    /// The temple's priest, or the shrine's.
    Priest,
    Homemaker,
    Retired,
    /// Runs Tsukumodō.
    Kenji,
    /// You: no schedule, you choose.
    Player,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Club {
    Soccer,
    Baseball,
    Kendo,
    Track,
    Tennis,
    BrassBand,
    Art,
    Science,
    Card,
}

impl Club {
    pub const ALL: [Club; 9] = [Club::Soccer, Club::Baseball, Club::Kendo, Club::Track, Club::Tennis, Club::BrassBand, Club::Art, Club::Science, Club::Card];
    pub fn name(self) -> &'static str {
        match self {
            Club::Soccer => "soccer",
            Club::Baseball => "baseball",
            Club::Kendo => "kendo",
            Club::Track => "track and field",
            Club::Tennis => "soft tennis",
            Club::BrassBand => "brass band",
            Club::Art => "art",
            Club::Science => "science",
            Club::Card => "card games",
        }
    }
    pub fn outdoor(self) -> bool {
        matches!(self, Club::Soccer | Club::Baseball | Club::Track | Club::Tennis)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Person {
    pub id: PersonId,
    pub family_name: String,
    pub given_name: String,
    pub sex: Sex,
    /// Age on [`CENSUS_DATE`].
    pub age: u8,
    /// Their household, or `None` for relatives who live away (Tokyo, Osaka, Minato-shi...).
    pub household: Option<HouseholdId>,
    /// Where they live when away.
    pub away: Option<String>,
    pub father: Option<PersonId>,
    pub mother: Option<PersonId>,
    pub spouse: Option<PersonId>,
    pub work: Work,
    /// What they'd say they do ("rice farmer", "town office", "2-B, Kazehana Junior High").
    pub title: String,
    pub workplace: Option<PlaceId>,
    /// Working hours (minutes after midnight) and the days they work (bit 0 = Monday).
    pub hours: Option<(u16, u16)>,
    pub workdays: u8,
    pub groups: Vec<GroupId>,
    pub look: Look,
}

impl Person {
    pub fn name(&self) -> String {
        format!("{} {}", self.family_name, self.given_name)
    }
    pub fn lives_here(&self) -> bool {
        self.household.is_some() && self.away.is_none()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GroupKind {
    /// Kids who hang around together.
    Friends,
    /// A junior high club.
    Club,
    /// Neighbourhood association block (班): about ten households who share the circular
    /// notice board, the rubbish point and the cleaning days.
    Han,
    /// Mothers of kids in the same class or street (ママ友).
    Mothers,
    /// The district's seniors' club (老人会).
    Seniors,
    Gateball,
    /// The volunteer fire brigade (消防団).
    FireBrigade,
    /// Elementary kids of one district (子ども会): walk to school together.
    KidsAssociation,
    /// The PTA committee.
    Pta,
    /// Old school friends.
    OldFriends,
}

/// When a group meets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meeting {
    pub place: PlaceId,
    /// Bit 0 = Monday.
    pub days: u8,
    pub from: u16,
    pub to: u16,
    /// Only on the first such day of the month.
    pub monthly: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: GroupId,
    pub kind: GroupKind,
    pub name: String,
    pub members: Vec<PersonId>,
    /// For han: the households in it.
    pub households: Vec<HouseholdId>,
    pub meets: Option<Meeting>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Census {
    pub seed: u64,
    pub places: Vec<Place>,
    pub houses: Vec<House>,
    pub households: Vec<Household>,
    pub people: Vec<Person>,
    pub groups: Vec<Group>,
    pub player: PersonId,
    pub kenji: PersonId,
    /// The junior high's timetable, built from the staff when first asked for.
    #[serde(skip)]
    pub timetable: std::sync::OnceLock<school::Timetable>,
}

/// How two people are related.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kin {
    Parent,
    Child,
    Spouse,
    Sibling,
    Grandparent,
    Grandchild,
    /// Aunt or uncle.
    ParentsSibling,
    /// Niece or nephew.
    SiblingsChild,
    Cousin,
    /// Spouse's parent.
    ParentInLaw,
    ChildInLaw,
}

impl Kin {
    pub fn word(self, sex: Sex) -> &'static str {
        let m = sex == Sex::M;
        match self {
            Kin::Parent => if m { "father" } else { "mother" },
            Kin::Child => if m { "son" } else { "daughter" },
            Kin::Spouse => if m { "husband" } else { "wife" },
            Kin::Sibling => if m { "brother" } else { "sister" },
            Kin::Grandparent => if m { "grandfather" } else { "grandmother" },
            Kin::Grandchild => if m { "grandson" } else { "granddaughter" },
            Kin::ParentsSibling => if m { "uncle" } else { "aunt" },
            Kin::SiblingsChild => if m { "nephew" } else { "niece" },
            Kin::Cousin => "cousin",
            Kin::ParentInLaw => if m { "father-in-law" } else { "mother-in-law" },
            Kin::ChildInLaw => if m { "son-in-law" } else { "daughter-in-law" },
        }
    }
}

impl Census {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        serde_json::from_slice(&std::fs::read(path)?).map_err(std::io::Error::other)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, serde_json::to_vec_pretty(self)?)
    }

    pub fn person(&self, id: PersonId) -> &Person {
        &self.people[id as usize]
    }

    pub fn household(&self, id: HouseholdId) -> &Household {
        &self.households[id as usize]
    }

    pub fn house(&self, id: HouseId) -> &House {
        self.houses.iter().find(|h| h.id == id).expect("house")
    }

    pub fn place(&self, id: PlaceId) -> &Place {
        &self.places[id as usize]
    }

    pub fn group(&self, id: GroupId) -> &Group {
        &self.groups[id as usize]
    }

    /// The place of a kind (the first, for kinds there is one of).
    pub fn place_of(&self, kind: PlaceKind) -> Option<&Place> {
        self.places.iter().find(|p| p.kind == kind)
    }

    /// Someone's home (as a place), if they live in town.
    pub fn home(&self, id: PersonId) -> Option<PlaceId> {
        let p = self.person(id);
        if p.away.is_some() {
            return None;
        }
        p.household.map(|h| self.house(self.household(h).house).place)
    }

    /// The people who live in town.
    pub fn residents(&self) -> impl Iterator<Item = &Person> {
        self.people.iter().filter(|p| p.lives_here())
    }

    pub fn children_of(&self, id: PersonId) -> Vec<PersonId> {
        self.people.iter().filter(|c| c.father == Some(id) || c.mother == Some(id)).map(|c| c.id).collect()
    }

    fn parents_of(&self, id: PersonId) -> Vec<PersonId> {
        let p = self.person(id);
        p.father.into_iter().chain(p.mother).collect()
    }

    fn siblings_of(&self, id: PersonId) -> Vec<PersonId> {
        let mut out: Vec<PersonId> = self.parents_of(id).iter().flat_map(|&par| self.children_of(par)).filter(|&c| c != id).collect();
        out.sort();
        out.dedup();
        out
    }

    /// Everyone related to someone, closest first.
    pub fn relatives(&self, id: PersonId) -> Vec<(PersonId, Kin)> {
        let mut out: Vec<(PersonId, Kin)> = Vec::new();
        let add = |out: &mut Vec<(PersonId, Kin)>, x: PersonId, k: Kin| {
            if x != id && !out.iter().any(|o| o.0 == x) {
                out.push((x, k));
            }
        };
        let me = self.person(id);
        if let Some(s) = me.spouse {
            add(&mut out, s, Kin::Spouse);
        }
        for p in self.parents_of(id) {
            add(&mut out, p, Kin::Parent);
        }
        for c in self.children_of(id) {
            add(&mut out, c, Kin::Child);
        }
        let sibs = self.siblings_of(id);
        for &s in &sibs {
            add(&mut out, s, Kin::Sibling);
        }
        for p in self.parents_of(id) {
            for g in self.parents_of(p) {
                add(&mut out, g, Kin::Grandparent);
            }
            for a in self.siblings_of(p) {
                add(&mut out, a, Kin::ParentsSibling);
                for c in self.children_of(a) {
                    add(&mut out, c, Kin::Cousin);
                }
            }
        }
        for c in self.children_of(id) {
            for g in self.children_of(c) {
                add(&mut out, g, Kin::Grandchild);
            }
            if let Some(cs) = self.person(c).spouse {
                add(&mut out, cs, Kin::ChildInLaw);
            }
        }
        for &s in &sibs {
            for c in self.children_of(s) {
                add(&mut out, c, Kin::SiblingsChild);
            }
        }
        if let Some(s) = me.spouse {
            for p in self.parents_of(s) {
                add(&mut out, p, Kin::ParentInLaw);
            }
        }
        out
    }

    /// Relatives who live in town but in another household.
    pub fn relatives_elsewhere_in_town(&self, id: PersonId) -> Vec<(PersonId, Kin)> {
        let mine = self.person(id).household;
        self.relatives(id).into_iter().filter(|(r, _)| {
            let p = self.person(*r);
            p.lives_here() && p.household != mine
        }).collect()
    }

    /// Where someone is and what they're doing (see [`schedule`]).
    /// The junior high's week (built once, from the staff: `school::Timetable::build`).
    pub fn timetable(&self) -> &school::Timetable {
        self.timetable.get_or_init(|| school::Timetable::build(self))
    }

    pub fn activity(&self, id: PersonId, day: Day, minute: u16) -> Activity {
        schedule::activity(self, id, day, minute)
    }

    /// Who is at a place at a time.
    pub fn people_at(&self, place: PlaceId, day: Day, minute: u16) -> Vec<PersonId> {
        self.residents().filter(|p| self.activity(p.id, day, minute).place == Some(place)).map(|p| p.id).collect()
    }
}
