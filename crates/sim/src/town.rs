//! The people of Kazehana-chō. Every house on the town plan (`assets/town/town.json`, made by
//! `tools/blender/town_plan.py`) holds one household (an apartment block holds four), and every
//! student at the junior high belongs to one of them, so each kid has a real home, a family and
//! a hamlet they walk in from.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tcg::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HouseKind {
    /// Old farmhouse (irimoya roof, shed, windbreak trees): often three generations.
    Farmhouse,
    /// Two-storey modern house.
    Modern,
    /// One-storey small house.
    Small,
    /// Shop on the ground floor, family upstairs.
    Shophouse,
    /// Two-storey apartment block (four flats).
    Apartment,
    /// Tsukumodō: Uncle Kenji's shop, and your home.
    ShopHome,
}

/// A house on the town plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HouseSlot {
    pub id: u16,
    /// The hamlet or quarter ("Centre", "Kami", "Yamanoue", ...).
    pub district: String,
    pub kind: HouseKind,
    pub households: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Grandfather,
    Grandmother,
    Father,
    Mother,
    Son,
    Daughter,
    Uncle,
    /// You.
    Nephew,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Member {
    pub given_name: String,
    pub role: Role,
    pub age: u8,
    pub job: String,
    /// The student id, for kids at Kazehana Junior High.
    pub student: Option<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Household {
    pub id: u16,
    /// `HouseSlot::id` (the house on the town plan).
    pub house: u16,
    pub family_name: String,
    pub members: Vec<Member>,
}

impl Household {
    /// Parents in their late 30s to 40s: the households that can have a junior-high kid.
    fn can_have_student(&self) -> bool {
        self.members.iter().any(|m| matches!(m.role, Role::Father | Role::Mother) && (34..=52).contains(&m.age))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Town {
    pub houses: Vec<HouseSlot>,
    pub households: Vec<Household>,
    /// Your household (Uncle Kenji and you, at Tsukumodō).
    pub home: u16,
}

/// Surnames of the town. Rural hamlets are dominated by a few old families, so each district
/// draws mostly from its own short list.
const SURNAMES: [&str; 40] = [
    "Satō", "Suzuki", "Takahashi", "Tanaka", "Itō", "Watanabe", "Yamamoto", "Nakamura", "Kobayashi", "Katō",
    "Yoshida", "Yamada", "Sasaki", "Yamaguchi", "Matsumoto", "Inoue", "Kimura", "Hayashi", "Shimizu", "Mori",
    "Ikeda", "Hashimoto", "Ishikawa", "Ogawa", "Okada", "Hasegawa", "Fujita", "Kondō", "Murakami", "Endō",
    "Aoki", "Sakamoto", "Saitō", "Fukuda", "Ōta", "Nishimura", "Miura", "Fujii", "Okamoto", "Matsuda",
];
pub const BOYS: [&str; 24] = [
    "Haruto", "Sōta", "Yūto", "Ren", "Minato", "Riku", "Hinata", "Kaito", "Daiki", "Takumi", "Kenta",
    "Shō", "Yamato", "Itsuki", "Ryō", "Hayato", "Kōki", "Taiga", "Asahi", "Sora", "Yūma", "Kōta",
    "Shun", "Tsubasa",
];
pub const GIRLS: [&str; 24] = [
    "Himari", "Yui", "Mei", "Sakura", "Hina", "Rin", "Aoi", "Mio", "Yuna", "Koharu", "Akari", "Saki",
    "Nanami", "Riko", "Kokona", "Hana", "Mitsuki", "Ema", "Ayaka", "Miyu", "Natsuki", "Chiharu",
    "Momoka", "Kaede",
];
const MEN: [&str; 16] = ["Hiroshi", "Takeshi", "Makoto", "Kazuya", "Daisuke", "Satoshi", "Naoki", "Tetsuya", "Shinji", "Masaru", "Yūji", "Kōji", "Tomoya", "Akira", "Jun", "Osamu"];
const WOMEN: [&str; 16] = ["Yuko", "Akiko", "Naomi", "Kumiko", "Megumi", "Tomoko", "Mayumi", "Kaori", "Emi", "Sachiko", "Junko", "Yumi", "Noriko", "Ayumi", "Keiko", "Hiromi"];
const OLD_MEN: [&str; 10] = ["Shigeru", "Isamu", "Tadashi", "Minoru", "Yoshio", "Kiyoshi", "Toshio", "Masao", "Haruo", "Saburō"];
const OLD_WOMEN: [&str; 10] = ["Fumiko", "Kazuko", "Setsuko", "Hanako", "Chiyo", "Toshiko", "Yoshiko", "Masako", "Teruko", "Kimiko"];

fn job(rng: &mut Rng, district: &str, kind: HouseKind, female: bool) -> String {
    let farm = kind == HouseKind::Farmhouse || !matches!(district, "Centre" | "Roadside");
    let jobs: &[&str] = match kind {
        HouseKind::Shophouse => &["runs the family shop"],
        _ if farm && !female => &["rice farmer", "rice farmer (weekends) and city hall", "JA co-op", "construction", "factory in Minato-shi", "truck driver"],
        _ if farm => &["helps on the farm", "nurse at the clinic", "part-time at the konbini", "school lunch centre", "care home", "homemaker"],
        _ if !female => &["office in the city (train)", "town office", "teacher", "factory in Minato-shi", "car repair", "post office"],
        _ => &["part-time at the drugstore", "nurse", "office in the city (train)", "homemaker", "hair salon", "teacher"],
    };
    rng.pick(jobs).to_string()
}

/// Make the households for every house.
fn populate(seed: u64, houses: &[HouseSlot]) -> (Vec<Household>, u16) {
    let mut rng = Rng::derive(seed, "town");
    // each district's old families
    let mut districts: Vec<&str> = houses.iter().map(|h| h.district.as_str()).collect();
    districts.sort();
    districts.dedup();
    let mut local: std::collections::HashMap<&str, Vec<&str>> = Default::default();
    for d in districts {
        let mut names: Vec<&str> = SURNAMES.to_vec();
        rng.shuffle(&mut names);
        local.insert(d, names[..3].to_vec());
    }
    let mut out = Vec::new();
    let mut home = 0;
    for h in houses {
        for _ in 0..h.households.max(1) {
            let id = out.len() as u16;
            if h.kind == HouseKind::ShopHome {
                home = id;
                out.push(Household {
                    id,
                    house: h.id,
                    family_name: "Tsukumo".into(),
                    members: vec![
                        Member { given_name: "Kenji".into(), role: Role::Uncle, age: 41, job: "runs Tsukumodō (games and cards)".into(), student: None },
                        Member { given_name: "You".into(), role: Role::Nephew, age: 12, job: "Year 7, Kazehana Junior High".into(), student: None },
                    ],
                });
                continue;
            }
            let family_name = if rng.chance(0.65) { *rng.pick(&local[h.district.as_str()]) } else { *rng.pick(&SURNAMES) }.to_string();
            let mut m = Vec::new();
            let person = |rng: &mut Rng, role: Role, age: u8| {
                let female = matches!(role, Role::Mother | Role::Grandmother | Role::Daughter);
                let name = match role {
                    Role::Grandfather => *rng.pick(&OLD_MEN),
                    Role::Grandmother => *rng.pick(&OLD_WOMEN),
                    Role::Father => *rng.pick(&MEN),
                    Role::Mother => *rng.pick(&WOMEN),
                    Role::Son => *rng.pick(&BOYS),
                    _ => *rng.pick(&GIRLS),
                };
                let job = match role {
                    Role::Grandfather | Role::Grandmother => if h.kind == HouseKind::Farmhouse && age < 80 { "still farms the paddies" } else { "retired" }.to_string(),
                    Role::Son | Role::Daughter => match age {
                        0..=5 => "at home / nursery".into(),
                        6..=11 => "Kazehana Elementary".into(),
                        12..=14 => "Kazehana Junior High".into(),
                        15..=17 => "high school in Minato-shi (train)".into(),
                        _ => job(rng, &h.district, h.kind, female),
                    },
                    _ => job(rng, &h.district, h.kind, female),
                };
                Member { given_name: name.into(), role, age, job, student: None }
            };
            let r = rng.f32();
            let (p_family, p_three_gen) = match h.kind {
                HouseKind::Farmhouse => (0.55, 0.75),
                HouseKind::Apartment => (0.45, 0.0),
                HouseKind::Shophouse => (0.5, 0.35),
                _ => (0.55, 0.2),
            };
            if r < p_family {
                // a family with children (none at the junior high yet: students are added after)
                let father = rng.range(33, 52) as u8;
                m.push(person(&mut rng, Role::Father, father));
                { let k = rng.range(0, 4) as u8; m.push(person(&mut rng, Role::Mother, father - k)); }
                for _ in 0..rng.range(0, 2) {
                    let age = rng.range(0, (father as i32 - 25).min(24)) as u8;
                    if (12..=14).contains(&age) {
                        continue; // junior-high kids are the students, placed below
                    }
                    let role = if rng.chance(0.5) { Role::Son } else { Role::Daughter };
                    m.push(person(&mut rng, role, age));
                }
                if rng.chance(p_three_gen) {
                    let gap = rng.range(25, 32) as u8;
                    let gf = father + gap;
                    m.push(person(&mut rng, Role::Grandfather, gf));
                    if rng.chance(0.8) {
                        { let k = rng.range(0, 4) as u8; m.push(person(&mut rng, Role::Grandmother, gf - k)); }
                    }
                }
            } else if r < p_family + 0.3 || h.kind == HouseKind::Farmhouse {
                // an older couple whose children have left for the city
                let gf = rng.range(62, 86) as u8;
                m.push(person(&mut rng, Role::Grandfather, gf));
                if rng.chance(0.8) {
                    { let k = rng.range(0, 4) as u8; m.push(person(&mut rng, Role::Grandmother, gf - k)); }
                }
            } else {
                // a younger couple
                let father = rng.range(26, 34) as u8;
                m.push(person(&mut rng, Role::Father, father));
                { let k = rng.range(0, 3) as u8; m.push(person(&mut rng, Role::Mother, father - k)); }
                if rng.chance(0.5) {
                    let role = if rng.chance(0.5) { Role::Son } else { Role::Daughter };
                    let age = rng.range(0, 5) as u8;
                    m.push(person(&mut rng, role, age));
                }
            }
            out.push(Household { id, house: h.id, family_name, members: m });
        }
    }
    (out, home)
}

impl Town {
    /// The town's households, before students are placed in them.
    pub fn new(seed: u64, houses: Vec<HouseSlot>) -> Self {
        let (households, home) = populate(seed, &houses);
        Self { houses, households, home }
    }

    /// Pick the households the school's students come from: one student each, and some with a
    /// brother or sister in another year. Returns (household, family name) for each student id.
    pub fn place_students(&mut self, seed: u64, n: usize) -> Vec<u16> {
        let mut rng = Rng::derive(seed, "students-homes");
        let mut candidates: Vec<u16> = self.households.iter().filter(|h| h.id != self.home && h.can_have_student()).map(|h| h.id).collect();
        rng.shuffle(&mut candidates);
        let siblings = n / 6;
        let singles = (n - siblings).min(candidates.len());
        let mut homes: Vec<u16> = candidates[..singles].to_vec();
        for i in 0..(n - singles) {
            homes.push(homes[i % singles.max(1)]);
        }
        rng.shuffle(&mut homes);
        homes
    }

    /// Record a student as a member of their household.
    pub fn add_student(&mut self, household: u16, student: u16, given_name: &str, boy: bool, age: u8) {
        let h = &mut self.households[household as usize];
        h.members.push(Member {
            given_name: given_name.into(),
            role: if boy { Role::Son } else { Role::Daughter },
            age,
            job: "Kazehana Junior High".into(),
            student: Some(student),
        });
    }

    pub fn household(&self, id: u16) -> &Household {
        &self.households[id as usize]
    }

    pub fn house_of(&self, household: u16) -> &HouseSlot {
        let hid = self.households[household as usize].house;
        self.houses.iter().find(|h| h.id == hid).expect("household in a house")
    }

    pub fn population(&self) -> usize {
        self.households.iter().map(|h| h.members.len()).sum()
    }
}

/// Read the houses from the town plan.
pub fn load_houses(path: &Path) -> std::io::Result<Vec<HouseSlot>> {
    #[derive(Deserialize)]
    struct Plan {
        houses: Vec<PlanHouse>,
    }
    #[derive(Deserialize)]
    struct PlanHouse {
        id: u16,
        district: String,
        kind: String,
        #[serde(rename = "type")]
        ty: String,
        households: u8,
    }
    let plan: Plan = serde_json::from_slice(&std::fs::read(path)?).map_err(std::io::Error::other)?;
    Ok(plan
        .houses
        .into_iter()
        .map(|h| HouseSlot {
            id: h.id,
            district: h.district,
            households: h.households,
            kind: match (h.kind.as_str(), h.ty.as_str()) {
                ("shop_home", _) => HouseKind::ShopHome,
                (_, "farmhouse") => HouseKind::Farmhouse,
                (_, "shophouse") => HouseKind::Shophouse,
                (_, "apartment") => HouseKind::Apartment,
                (_, "small_house") => HouseKind::Small,
                _ => HouseKind::Modern,
            },
        })
        .collect())
}

/// A stand-in town of the right size, for tests and when the plan has not been generated.
pub fn fallback_houses() -> Vec<HouseSlot> {
    let districts = ["Centre", "Kami", "Yamanoue", "Higashi", "Shimo", "Minami", "Oku"];
    let mut v = vec![HouseSlot { id: 0, district: "Centre".into(), kind: HouseKind::ShopHome, households: 1 }];
    for i in 1..480u16 {
        let kind = match i % 9 {
            0 => HouseKind::Farmhouse,
            1 | 2 => HouseKind::Small,
            3 if i % 27 == 3 => HouseKind::Apartment,
            4 => HouseKind::Shophouse,
            _ => HouseKind::Modern,
        };
        v.push(HouseSlot { id: i, district: districts[i as usize % districts.len()].into(), kind, households: if kind == HouseKind::Apartment { 4 } else { 1 } });
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everyone_has_a_home() {
        let mut t = Town::new(3, fallback_houses());
        assert!(t.population() > 1000, "population {}", t.population());
        let homes = t.place_students(3, 99);
        assert_eq!(homes.len(), 99);
        assert!(homes.iter().all(|&h| h != t.home));
        // some brothers and sisters
        let mut c = std::collections::HashMap::new();
        for h in &homes {
            *c.entry(h).or_insert(0) += 1;
        }
        assert!(c.values().any(|&n| n >= 2));
        assert_eq!(t.household(t.home).family_name, "Tsukumo");
        assert_eq!(t.house_of(t.home).kind, HouseKind::ShopHome);
        t.add_student(homes[0], 0, "Ren", true, 12);
        assert!(t.household(homes[0]).members.iter().any(|m| m.student == Some(0)));
    }

    /// The real town plan, when it has been generated: every student gets a house on it.
    #[test]
    fn the_town_plan_houses_every_student() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/town/town.json");
        let Ok(houses) = load_houses(&path) else { return };
        assert!(houses.iter().filter(|h| h.kind == HouseKind::ShopHome).count() == 1, "exactly one Tsukumodō");
        let db = tcg::CardDb::generate(1);
        let mut t = Town::new(7, houses);
        let students = crate::people::generate_students_in(7, &db, &mut t);
        assert_eq!(students.len(), crate::people::SCHOOL_SIZE - 1);
        for s in &students {
            let h = t.household(s.household);
            assert_eq!(h.family_name, s.family_name);
            assert!(h.members.iter().any(|m| m.student == Some(s.id)));
            assert!(h.members.iter().any(|m| matches!(m.role, Role::Father | Role::Mother)), "{} has parents", s.full_name());
            assert_ne!(t.house_of(s.household).kind, HouseKind::ShopHome);
            assert!(!students.iter().any(|o| o.id != s.id && o.household == s.household && o.grade == s.grade), "{} has a sibling in the same year", s.full_name());
        }
        let pop = t.population();
        assert!((1200..=1900).contains(&pop), "town population {pop}");
    }
}
