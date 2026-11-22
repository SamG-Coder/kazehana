//! The students of Kazehana Junior High, their schedules, and your bonds with them.

use crate::clock::{GameTime, Phase};
use serde::{Deserialize, Serialize};
use crate::town::{fallback_houses, Town, BOYS, GIRLS};
use tcg::{Attribute, CardDb, CardId, Collection, Rarity, Rng};

/// Total enrolment including the player.
pub const SCHOOL_SIZE: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gender {
    Boy,
    Girl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Personality {
    /// Wants the strongest cards and pays for them.
    Competitive,
    /// Chases rares, mythics and foils.
    Collector,
    /// Plays for fun; doesn't pay much.
    Casual,
    /// Always tries to talk the price down.
    Haggler,
    /// Hard to get to know, very loyal once you do.
    Shy,
    /// Wants whatever looks coolest to show off.
    Flashy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Spot {
    Classroom,
    Corridor,
    Roof,
    Library,
    Courtyard,
    Gym,
    CardClub,
    SportsGround,
    Shop,
    Path,
    Home,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Club {
    Card,
    Soccer,
    Baseball,
    Art,
    BrassBand,
    Kendo,
    Track,
    Science,
}

impl Club {
    pub fn spot(self) -> Spot {
        match self {
            Club::Card | Club::Art | Club::Science => Spot::CardClub,
            Club::Soccer | Club::Baseball | Club::Track => Spot::SportsGround,
            Club::BrassBand => Spot::Classroom,
            Club::Kendo => Spot::Gym,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Student {
    pub id: u16,
    /// Their household in the town (`Town::households`): family, home, hamlet.
    #[serde(default)]
    pub household: u16,
    pub family_name: String,
    pub given_name: String,
    pub gender: Gender,
    /// 1..=3 (中1–中3).
    pub grade: u8,
    /// 'A' or 'B'.
    pub class: char,
    pub personality: Personality,
    pub hobby: String,
    pub club: Option<Club>,
    pub plays_cards: bool,
    pub favorites: Vec<Attribute>,
    /// 0..1, how good they are at the card game.
    pub skill: f32,
    pub pocket_money_weekly: u32,
    pub wallet: u32,
    /// Cards they are hunting this season (only visible to you at Kizuna rank 4).
    pub wants: Vec<CardId>,
    pub collection: Collection,
    pub lunch_spot: Spot,
    pub free_spot: Spot,
}

impl Student {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.family_name, self.given_name)
    }

    pub fn class_name(&self) -> String {
        format!("{}-{}", self.grade, self.class)
    }

    /// Where this student is at a given time (a cheap deterministic schedule).
    pub fn spot_at(&self, t: &GameTime) -> Spot {
        let wd = t.date.weekday().index() as u64;
        let roll = hash3(self.id as u64, t.date.to_days() as u64, wd) % 100;
        match t.phase() {
            Phase::LateNight | Phase::EarlyMorning | Phase::Night => Spot::Home,
            Phase::Morning if t.school_day() => {
                if t.minute >= crate::clock::at::hm(8, 0) { Spot::Path } else { Spot::Home }
            }
            Phase::Morning => Spot::Home,
            Phase::MorningClass | Phase::AfternoonClass => Spot::Classroom,
            Phase::Lunch => if roll < 30 { Spot::Classroom } else { self.lunch_spot },
            Phase::AfterSchool => match self.club {
                // Clubs meet Mon–Thu; on Friday everyone drifts.
                Some(c) if wd < 4 => c.spot(),
                _ if self.plays_cards && roll < 35 => Spot::Shop,
                _ => self.free_spot,
            },
            Phase::Daytime => {
                let release = t.date.weekday() == crate::calendar::Weekday::Sat && t.date.day <= 7;
                let shop_chance = if release { 80 } else { 25 };
                if self.plays_cards && roll < shop_chance && (10..20).contains(&t.hour()) { Spot::Shop } else { Spot::Home }
            }
            Phase::Evening => if self.plays_cards && roll < 10 && t.hour() < 20 { Spot::Shop } else { Spot::Home },
        }
    }

    /// Recompute the want list for a newly released season.
    pub fn refresh_wants(&mut self, db: &CardDb, season: u16, rng: &mut Rng) {
        self.wants.clear();
        if !self.plays_cards {
            return;
        }
        let mut scored: Vec<(CardId, f32)> = db
            .all()
            .iter()
            .filter(|c| c.id.set + 1 >= season && c.id.set <= season) // current and previous season
            .filter(|c| self.collection.count(c.id) < 2)
            .map(|c| {
                let mut s = c.hype + rng.f32() * 0.35;
                if self.favorites.contains(&c.attribute) {
                    s += 0.35;
                }
                match self.personality {
                    Personality::Collector if c.rarity >= Rarity::Rare => s += 0.4,
                    Personality::Flashy if c.rarity == Rarity::Mythic => s += 0.5,
                    Personality::Competitive => s += c.hype * 0.5,
                    _ => {}
                }
                if c.id.set == season {
                    s += 0.15;
                }
                (c.id, s)
            })
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        let n = 3 + rng.below(4);
        self.wants = scored.into_iter().take(n).map(|x| x.0).collect();
    }
}

fn hash3(a: u64, b: u64, c: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for x in [a, b, c] {
        h = (h ^ x).wrapping_mul(0x100_0000_01b3);
        h ^= h >> 29;
    }
    h
}

// ---------------------------------------------------------------------------------------------
// Kizuna (bonds)

/// Points needed for each rank, 0..=10.
pub const RANK_POINTS: [u32; 11] = [0, 10, 25, 45, 70, 100, 135, 175, 220, 270, 330];

/// What you know about someone. Each fact unlocks at a Kizuna rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fact {
    Name,
    Hobby,
    Favorites,
    Club,
    Wants,
    Wallet,
    Rumours,
}

impl Fact {
    pub fn rank_needed(self) -> u8 {
        match self {
            Fact::Name => 1,
            Fact::Hobby | Fact::Club => 2,
            Fact::Favorites => 3,
            Fact::Wants => 4,
            Fact::Wallet => 5,
            Fact::Rumours => 6,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Bond {
    pub points: u32,
    /// The last day you talked; talking again the same day gives nothing.
    pub last_talk: Option<crate::calendar::Date>,
}

impl Bond {
    pub fn rank(&self) -> u8 {
        RANK_POINTS.iter().rposition(|&p| self.points >= p).unwrap_or(0) as u8
    }

    pub fn knows(&self, fact: Fact) -> bool {
        self.rank() >= fact.rank_needed()
    }

    pub fn add(&mut self, delta: i32) {
        self.points = (self.points as i32 + delta).max(0) as u32;
    }

    pub fn title(&self) -> &'static str {
        match self.rank() {
            0 => "Stranger",
            1..=2 => "Acquaintance",
            3..=5 => "Friend",
            6..=8 => "Close Friend",
            9 => "Best Friend",
            _ => "Kindred Spirit",
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Generation

const HOBBIES: [&str; 16] = [
    "soccer", "baseball", "drawing manga", "video games", "piano", "reading", "baking", "kendo",
    "catching beetles", "anime", "fishing", "calligraphy", "running", "idol music", "robots", "cooking",
];

/// Generate the other 99 students in a stand-in town (tests; the game uses the town plan).
pub fn generate_students(seed: u64, db: &CardDb) -> Vec<Student> {
    let mut town = Town::new(seed, fallback_houses());
    generate_students_in(seed, db, &mut town)
}

/// Generate the other 99 students, each living with their family in one of the town's houses.
/// The player is the 100th (class 1-A).
pub fn generate_students_in(seed: u64, db: &CardDb, town: &mut Town) -> Vec<Student> {
    let mut homes = town.place_students(seed, SCHOOL_SIZE - 1);
    // brothers and sisters are in different years (33 students per year, by id)
    let year = |i: usize| i / 33;
    for i in 0..homes.len() {
        let clash = |homes: &[u16], i: usize| (0..homes.len()).any(|k| k != i && year(k) == year(i) && homes[k] == homes[i]);
        if clash(&homes, i) {
            for j in 0..homes.len() {
                if year(j) != year(i) {
                    homes.swap(i, j);
                    if !clash(&homes, i) && !clash(&homes, j) {
                        break;
                    }
                    homes.swap(i, j);
                }
            }
        }
    }
    let mut rng = Rng::derive(seed, "students");
    let mut out = Vec::with_capacity(SCHOOL_SIZE - 1);
    let mut used = std::collections::HashSet::new();
    for id in 0..(SCHOOL_SIZE - 1) as u16 {
        // 33 per grade (the player fills the last first-year seat), split across classes A/B.
        let grade = (id / 33 + 1).min(3) as u8;
        let class = if (id % 33) < 17 { 'A' } else { 'B' };
        let gender = if rng.chance(0.5) { Gender::Boy } else { Gender::Girl };
        let household = homes[id as usize];
        let family = town.household(household).family_name.clone();
        let given = loop {
            let g = if gender == Gender::Boy { *rng.pick(&BOYS) } else { *rng.pick(&GIRLS) };
            if used.insert((family.clone(), g)) {
                break g;
            }
        };
        town.add_student(household, id, given, gender == Gender::Boy, 11 + grade);
        let personality = *rng.pick(&[
            Personality::Competitive, Personality::Collector, Personality::Casual, Personality::Casual,
            Personality::Haggler, Personality::Shy, Personality::Flashy,
        ]);
        let hobby = rng.pick(&HOBBIES).to_string();
        let club = match hobby.as_str() {
            "soccer" => Some(Club::Soccer),
            "baseball" => Some(Club::Baseball),
            "kendo" => Some(Club::Kendo),
            "running" => Some(Club::Track),
            "drawing manga" if rng.chance(0.6) => Some(Club::Art),
            "piano" | "idol music" if rng.chance(0.5) => Some(Club::BrassBand),
            "robots" => Some(Club::Science),
            _ => None,
        };
        // Younger kids play more; club members a bit less.
        let p_play = [0.0, 0.6, 0.45, 0.3][grade as usize] - if club.is_some() { 0.1 } else { 0.0 };
        let plays_cards = rng.chance(p_play) || personality == Personality::Collector;
        let club = if plays_cards && club.is_none() && rng.chance(0.35) { Some(Club::Card) } else { club };
        let mut favorites = vec![*rng.pick(&Attribute::ALL)];
        if rng.chance(0.4) {
            let second = *rng.pick(&Attribute::ALL);
            if second != favorites[0] {
                favorites.push(second);
            }
        }
        let pocket_money_weekly = [0, 500, 700, 1000][grade as usize] + 100 * rng.below(5) as u32;
        let lunch_spot = *rng.pick(&[Spot::Classroom, Spot::Roof, Spot::Courtyard, Spot::Library, Spot::Corridor]);
        let free_spot = *rng.pick(&[Spot::Library, Spot::Courtyard, Spot::Home, Spot::Home, Spot::Path]);
        let mut s = Student {
            id,
            household,
            family_name: family,
            given_name: given.to_string(),
            gender,
            grade,
            class,
            personality,
            hobby,
            club,
            plays_cards,
            favorites,
            skill: rng.f32().powf(1.5),
            pocket_money_weekly,
            wallet: pocket_money_weekly * (1 + rng.below(4) as u32),
            wants: Vec::new(),
            collection: Collection::default(),
            lunch_spot,
            free_spot,
        };
        if plays_cards {
            // Everyone who plays starts with a few packs' worth of the first season.
            for _ in 0..rng.range(2, 8) {
                for pull in tcg::open_pack(db, 1, &mut rng) {
                    s.collection.add(pull);
                }
            }
        }
        s.refresh_wants(db, 1, &mut rng);
        out.push(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::clock::at;

    #[test]
    fn school_population() {
        let db = CardDb::generate(1);
        let s = generate_students(42, &db);
        assert_eq!(s.len(), SCHOOL_SIZE - 1);
        let first_years = s.iter().filter(|x| x.grade == 1).count();
        assert_eq!(first_years, 33);
        let players = s.iter().filter(|x| x.plays_cards).count();
        assert!((25..=70).contains(&players), "{players} card players");
        let names: std::collections::HashSet<_> = s.iter().map(|x| x.full_name()).collect();
        assert_eq!(names.len(), s.len());
        // Same seed, same town.
        assert_eq!(generate_students(42, &db)[10].full_name(), s[10].full_name());
    }

    #[test]
    fn bonds_unlock_facts_gradually() {
        let mut b = Bond::default();
        assert!(!b.knows(Fact::Name));
        b.add(10);
        assert!(b.knows(Fact::Name) && !b.knows(Fact::Wants));
        b.add(60);
        assert_eq!(b.rank(), 4);
        assert!(b.knows(Fact::Wants));
        b.add(1000);
        assert_eq!(b.rank(), 10);
    }

    #[test]
    fn everyone_is_in_class_during_lessons() {
        let db = CardDb::generate(1);
        let s = generate_students(1, &db);
        let t = GameTime::new(Date::new(2026, 4, 8), at::hm(10, 0));
        assert!(s.iter().all(|x| x.spot_at(&t) == Spot::Classroom));
        let night = GameTime::new(Date::new(2026, 4, 8), at::hm(23, 0));
        assert!(s.iter().all(|x| x.spot_at(&night) == Spot::Home));
    }
}
