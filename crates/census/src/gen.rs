//! Making the town's people (the plan is in `docs/population_plan.md`).
//!
//! 1. Old families (clans): an elderly couple in a farmhouse or shophouse (or an older house),
//!    their grown children: the heir stays and brings up the next generation under the same roof,
//!    other sons set up house nearby, daughters marry into other local families or leave, the
//!    rest live in the cities.
//! 2. Newcomers fill the houses no family holds: young couples, families who moved out from the
//!    city, teachers in the apartment flats, widows and widowers on their own.
//! 3. The junior high's year groups are brought to exactly 32 + 33 + 34 (with you, 33 per year).
//! 4. Jobs (shops, farms, the town's workplaces, the train to Minato-shi), the teachers of both
//!    schools, classes and clubs.
//! 5. Groups: neighbourhood blocks, friend groups, mothers, seniors, gateball, fire brigade, PTA.
//! 6. Looks.

use crate::look::{self, Look};
use crate::names;
use crate::*;
use std::collections::HashMap;
use tcg::Rng;

/// The houses and sites of the town plan.
#[derive(Clone, Debug)]
pub struct Plan {
    pub houses: Vec<PlanHouse>,
    pub sites: Vec<(String, PlaceKind, [f32; 2])>,
}

#[derive(Clone, Debug)]
pub struct PlanHouse {
    pub id: HouseId,
    pub district: String,
    pub kind: HouseKind,
    pub flats: u8,
    pub at: [f32; 2],
}

fn centroid(poly: &[[f32; 2]]) -> [f32; 2] {
    let n = poly.len().max(1) as f32;
    [poly.iter().map(|p| p[0]).sum::<f32>() / n, poly.iter().map(|p| p[1]).sum::<f32>() / n]
}

impl Plan {
    /// Read `assets/town/town.json`.
    pub fn load(path: &std::path::Path) -> std::io::Result<Plan> {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?).map_err(std::io::Error::other)?;
        let pts = |v: &serde_json::Value| -> Vec<[f32; 2]> {
            v.as_array().map(|a| a.iter().filter_map(|p| Some([p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32])).collect()).unwrap_or_default()
        };
        let mut houses = Vec::new();
        for h in v["houses"].as_array().into_iter().flatten() {
            let ty = h["type"].as_str().unwrap_or("");
            let kind = match (h["kind"].as_str().unwrap_or(""), ty) {
                ("shop_home", _) => HouseKind::ShopHome,
                (_, "farmhouse") => HouseKind::Farmhouse,
                (_, "shophouse") => HouseKind::Shophouse,
                (_, "apartment") => HouseKind::Apartment,
                (_, "small_house") => HouseKind::Small,
                _ => HouseKind::Modern,
            };
            houses.push(PlanHouse {
                id: h["id"].as_u64().unwrap_or(0) as HouseId,
                district: h["district"].as_str().unwrap_or("Centre").to_string(),
                kind,
                flats: h["households"].as_u64().unwrap_or(1).max(1) as u8,
                at: centroid(&pts(&h["house"])),
            });
        }
        let mut sites = Vec::new();
        for s in v["sites"].as_array().into_iter().flatten() {
            let name = s["name"].as_str().unwrap_or("").to_string();
            let kind = match (s["kind"].as_str().unwrap_or(""), name.as_str()) {
                ("school", n) if n.contains("Junior") => PlaceKind::JuniorHigh,
                ("school", _) => PlaceKind::Elementary,
                ("station", _) => PlaceKind::Station,
                ("plaza", _) => PlaceKind::StationPlaza,
                ("shrine", _) => PlaceKind::Shrine,
                ("inari", _) => PlaceKind::InariShrine,
                ("temple", _) => PlaceKind::Temple,
                ("civic", n) if n.contains("post") => PlaceKind::PostOffice,
                ("civic", _) => PlaceKind::TownOffice,
                ("konbini", _) => PlaceKind::Konbini,
                (_, n) if n.contains("co-op") => PlaceKind::CoOp,
                (_, n) if n.contains("petrol") => PlaceKind::Petrol,
                (_, n) if n.contains("drugstore") => PlaceKind::Drugstore,
                (_, n) if n.contains("car repair") => PlaceKind::CarRepair,
                _ => continue,
            };
            sites.push((name, kind, centroid(&pts(&s["poly"]))));
        }
        Ok(Plan { houses, sites })
    }

    /// A stand-in town of the right size (tests, and before the plan is generated).
    pub fn fallback() -> Plan {
        let districts = ["Centre", "Kami", "Yamanoue", "Higashi", "Shimo", "Minami", "Oku"];
        let mut houses = vec![PlanHouse { id: 0, district: "Centre".into(), kind: HouseKind::ShopHome, flats: 1, at: [0.0, 0.0] }];
        for i in 1..471u16 {
            let kind = match i % 8 {
                0 => HouseKind::Farmhouse,
                1 | 2 => HouseKind::Small,
                3 if i % 50 == 3 => HouseKind::Apartment,
                4 => HouseKind::Shophouse,
                _ => HouseKind::Modern,
            };
            let d = i as usize % districts.len();
            let a = d as f32 * 0.9 + (i as f32 * 0.37).sin();
            let r = if d == 0 { 80.0 } else { 500.0 + (i % 13) as f32 * 15.0 };
            houses.push(PlanHouse { id: i, district: districts[d].into(), kind, flats: if kind == HouseKind::Apartment { 4 } else { 1 }, at: [a.cos() * r, a.sin() * r] });
        }
        let sites = [
            ("Kazehana Junior High School", PlaceKind::JuniorHigh), ("Kazehana Elementary School", PlaceKind::Elementary), ("Kazehana Station", PlaceKind::Station),
            ("station plaza", PlaceKind::StationPlaza), ("Hachiman shrine", PlaceKind::Shrine), ("Shōrin-ji temple", PlaceKind::Temple), ("post office", PlaceKind::PostOffice),
            ("town office branch", PlaceKind::TownOffice), ("Inari shrine", PlaceKind::InariShrine), ("konbini", PlaceKind::Konbini), ("JA farm co-op", PlaceKind::CoOp),
            ("petrol station", PlaceKind::Petrol), ("drugstore", PlaceKind::Drugstore), ("car repair", PlaceKind::CarRepair),
        ]
        .iter()
        .enumerate()
        .map(|(i, (n, k))| (n.to_string(), *k, [(i as f32 * 1.3).cos() * 150.0, (i as f32 * 1.3).sin() * 150.0]))
        .collect();
        Plan { houses, sites }
    }
}

struct Slot {
    house: usize,
    free: u8,
}

struct G {
    rng: Rng,
    c: Census,
    slots: Vec<Slot>,
    local: HashMap<String, Vec<&'static str>>,
    /// Unmarried daughters of the old families, living away for now, who may marry into a
    /// family in town.
    brides: Vec<PersonId>,
    /// Full names in use (no two living people share one).
    used: std::collections::HashSet<(String, String)>,
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

impl G {
    fn place(&mut self, name: String, kind: PlaceKind, house: Option<HouseId>, at: [f32; 2]) -> PlaceId {
        let id = self.c.places.len() as PlaceId;
        self.c.places.push(Place { id, name, kind, house, at });
        id
    }

    fn surname(&mut self, district: &str) -> String {
        let local = self.local[district].clone();
        let p = if district == "Centre" { 0.25 } else { 0.6 };
        if self.rng.chance(p) { *self.rng.pick(&local) } else { *self.rng.pick(&names::SURNAMES) }.to_string()
    }

    fn new_household(&mut self, slot: usize, family: &str) -> HouseholdId {
        let id = self.c.households.len() as HouseholdId;
        self.slots[slot].free -= 1;
        let house = self.slots[slot].house;
        self.c.houses[house].households.push(id);
        let hid = self.c.houses[house].id;
        self.c.households.push(Household { id, house: hid, family_name: family.to_string(), members: Vec::new(), han: 0 });
        id
    }

    /// A free slot of one of these kinds, near a point (a few nearest, one picked at random).
    fn take_slot(&mut self, kinds: &[HouseKind], near: Option<[f32; 2]>) -> Option<usize> {
        let mut c: Vec<(f32, usize)> = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.free > 0 && kinds.contains(&self.c.houses[s.house].kind))
            .map(|(i, s)| (near.map(|n| dist(n, self.c.houses[s.house].at)).unwrap_or(0.0), i))
            .collect();
        if c.is_empty() {
            return None;
        }
        if near.is_some() {
            c.sort_by(|a, b| a.0.total_cmp(&b.0));
            c.truncate(4);
        }
        Some(c[self.rng.below(c.len())].1)
    }

    fn given_name(&mut self, family: &str, sex: Sex, age: u8, taken: &[String]) -> String {
        let pool = names::given(sex, age);
        for _ in 0..60 {
            let n = *self.rng.pick(pool);
            if !taken.iter().any(|t| t == n) && !self.used.contains(&(family.to_string(), n.to_string())) {
                self.used.insert((family.to_string(), n.to_string()));
                return n.to_string();
            }
        }
        self.rng.pick(pool).to_string()
    }

    /// Names already used close by (the household, and brothers and sisters).
    fn taken_names(&self, hh: Option<HouseholdId>, father: Option<PersonId>, mother: Option<PersonId>) -> Vec<String> {
        let mut t: Vec<String> = hh.map(|h| self.c.households[h as usize].members.iter().map(|&m| self.c.people[m as usize].given_name.clone()).collect()).unwrap_or_default();
        for p in &self.c.people {
            if (father.is_some() && p.father == father) || (mother.is_some() && p.mother == mother) {
                t.push(p.given_name.clone());
            }
        }
        t
    }

    fn person(&mut self, family: &str, sex: Sex, age: u8, hh: Option<HouseholdId>, father: Option<PersonId>, mother: Option<PersonId>) -> PersonId {
        let taken = self.taken_names(hh, father, mother);
        let given_name = self.given_name(family, sex, age, &taken);
        let id = self.c.people.len() as PersonId;
        self.c.people.push(Person {
            id,
            family_name: family.to_string(),
            given_name,
            sex,
            age,
            household: hh,
            away: None,
            father,
            mother,
            spouse: None,
            work: Work::Retired,
            title: String::new(),
            workplace: None,
            hours: None,
            workdays: 0,
            groups: Vec::new(),
            look: Look::blank(),
        });
        if let Some(h) = hh {
            self.c.households[h as usize].members.push(id);
        }
        id
    }

    fn move_to(&mut self, p: PersonId, hh: HouseholdId) {
        if let Some(old) = self.c.people[p as usize].household {
            self.c.households[old as usize].members.retain(|&m| m != p);
        }
        self.c.people[p as usize].household = Some(hh);
        self.c.people[p as usize].away = None;
        self.c.households[hh as usize].members.push(p);
    }

    fn wed(&mut self, a: PersonId, b: PersonId) {
        self.c.people[a as usize].spouse = Some(b);
        self.c.people[b as usize].spouse = Some(a);
    }

    /// Marry someone who lives in `hh`. A man may marry a daughter of another old family (she
    /// moves in and takes his name); otherwise the spouse comes from out of town. A daughter who
    /// is her family's heir takes a husband into her family (mukoyōshi).
    fn marry(&mut self, p: PersonId, hh: HouseholdId) -> Option<PersonId> {
        if self.rng.chance(0.12) {
            return None; // (some never marry: the bachelor farmer is a real thing)
        }
        let (sex, age, family, father) = {
            let x = &self.c.people[p as usize];
            (x.sex, x.age, x.family_name.clone(), x.father)
        };
        if sex == Sex::M {
            let pick = self.brides.iter().position(|&b| {
                let w = &self.c.people[b as usize];
                (age as i32 - w.age as i32) >= -2 && (age as i32 - w.age as i32) <= 7 && w.father != father && w.family_name != family
            });
            if let Some(i) = pick.filter(|_| self.rng.chance(0.7)) {
                let w = self.brides.remove(i);
                self.move_to(w, hh);
                let given = self.c.people[w as usize].given_name.clone();
                if self.used.contains(&(family.clone(), given.clone())) {
                    let taken = self.taken_names(Some(hh), None, None);
                    let wage = self.c.people[w as usize].age;
                    self.c.people[w as usize].given_name = self.given_name(&family, Sex::F, wage, &taken);
                } else {
                    self.used.insert((family.clone(), given));
                }
                self.c.people[w as usize].family_name = family;
                self.wed(p, w);
                return Some(w);
            }
            let wa = (age as i32 - self.rng.range(-2, 5)).clamp(18, 99) as u8;
            let w = self.person(&family, Sex::F, wa, Some(hh), None, None);
            self.wed(p, w);
            Some(w)
        } else {
            let ha = (age as i32 + self.rng.range(-2, 5)).clamp(18, 99) as u8;
            let h = self.person(&family, Sex::M, ha, Some(hh), None, None);
            self.wed(p, h);
            Some(h)
        }
    }

    /// Children for a couple living in `hh` (or a single parent). Grown children may have left.
    fn children(&mut self, father: Option<PersonId>, mother: Option<PersonId>, hh: HouseholdId, max_n: usize) {
        let mage = mother.or(father).map(|m| self.c.people[m as usize].age as i32).unwrap_or(30);
        let family = self.c.households[hh as usize].family_name.clone();
        let r = self.rng.f32();
        let n = if r < 0.12 { 0 } else if r < 0.36 { 1 } else if r < 0.8 { 2 } else { 3 }.min(max_n);
        let mut ages: Vec<i32> = Vec::new();
        for _ in 0..n {
            for _ in 0..12 {
                let a = mage - (26.0 + 12.0 * self.rng.f32().powf(1.3)) as i32;
                if a >= 0 && !ages.iter().any(|&b| (a - b).abs() < 1) {
                    ages.push(a);
                    break;
                }
            }
        }
        ages.sort_by(|a, b| b.cmp(a));
        for a in ages {
            let sex = if self.rng.chance(0.51) { Sex::M } else { Sex::F };
            let k = self.person(&family, sex, a as u8, Some(hh), father, mother);
            // grown children: away at university or working in the cities, or still at home
            if a >= 18 && self.rng.chance(if a <= 22 { 0.7 } else { 0.55 }) {
                let w = if a <= 22 { "university in Sendai".to_string() } else { self.rng.pick(&names::AWAY).to_string() };
                self.c.people[k as usize].away = Some(w);
            }
        }
    }

    /// An old family: the elders in this slot, their children and grandchildren.
    fn clan(&mut self, slot: usize) {
        let house = self.slots[slot].house;
        let (district, kind, at) = {
            let h = &self.c.houses[house];
            (h.district.clone(), h.kind, h.at)
        };
        let family = self.surname(&district);
        let hh = self.new_household(slot, &family);
        let ga = self.rng.range(66, 92) as u8;
        let gma = (ga as i32 - self.rng.range(0, 4)) as u8;
        let gf_alive = self.rng.chance(if ga >= 82 { 0.5 } else { 0.82 });
        let gm_alive = self.rng.chance(if gma >= 84 { 0.65 } else { 0.92 });
        let gf = self.person(&family, Sex::M, ga, gf_alive.then_some(hh).or(None), None, None);
        let gm = self.person(&family, Sex::F, gma, gm_alive.then_some(hh), None, None);
        self.wed(gf, gm);
        // the dead are kept (so their children are brothers and sisters) but live nowhere
        for (p, alive) in [(gf, gf_alive), (gm, gm_alive)] {
            if !alive {
                self.c.people[p as usize].away = Some("deceased".into());
            }
        }
        let r = self.rng.f32();
        let n = if r < 0.15 { 1 } else if r < 0.6 { 2 } else { 3 };
        let mut kids: Vec<(u8, Sex)> = Vec::new();
        for _ in 0..n {
            for _ in 0..12 {
                let a = gma as i32 - self.rng.range(23, 36);
                if a >= 20 && !kids.iter().any(|k| (k.0 as i32 - a).abs() < 1) {
                    kids.push((a as u8, if self.rng.chance(0.51) { Sex::M } else { Sex::F }));
                    break;
                }
            }
        }
        kids.sort_by(|a, b| b.0.cmp(&a.0));
        let heir = kids.iter().position(|k| k.1 == Sex::M).unwrap_or(0);
        let stays = match kind {
            HouseKind::Farmhouse | HouseKind::Shophouse => self.rng.chance(0.75),
            _ => self.rng.chance(0.4),
        } || !(gf_alive || gm_alive);
        for (i, (age, sex)) in kids.into_iter().enumerate() {
            let k = self.person(&family, sex, age, None, Some(gf), Some(gm));
            if i == heir && stays {
                self.move_to(k, hh);
                let sp = self.marry(k, hh);
                let (f, m) = if sex == Sex::M { (Some(k), sp) } else { (sp, Some(k)) };
                if sp.is_some() || self.rng.chance(0.1) {
                    self.children(f, m, hh, 3);
                }
                continue;
            }
            if sex == Sex::M && self.rng.chance(0.4) {
                if let Some(s) = self.take_slot(&[HouseKind::Modern, HouseKind::Small, HouseKind::Apartment], Some(at)) {
                    let own = self.new_household(s, &family);
                    self.move_to(k, own);
                    let sp = self.marry(k, own);
                    if sp.is_some() {
                        self.children(Some(k), sp, own, 3);
                    }
                    continue;
                }
            }
            if sex == Sex::F && age <= 50 && self.rng.chance(0.45) {
                self.c.people[k as usize].away = Some("Minato-shi".into());
                self.brides.push(k);
                continue;
            }
            let w = self.rng.pick(&names::AWAY).to_string();
            self.c.people[k as usize].away = Some(w);
        }
    }

    /// A household of people with no old family here.
    fn newcomers(&mut self, slot: usize) {
        let house = self.slots[slot].house;
        let (district, kind) = {
            let h = &self.c.houses[house];
            (h.district.clone(), h.kind)
        };
        let family = if self.rng.chance(0.35) { self.surname(&district) } else { self.rng.pick(&names::SURNAMES).to_string() };
        let hh = self.new_household(slot, &family);
        let r = self.rng.f32();
        let couple = |g: &mut G, age: u8| {
            let m = g.person(&family, Sex::M, age, Some(hh), None, None);
            let wa = (age as i32 - g.rng.range(-2, 4)).clamp(20, 99) as u8;
            let w = g.person(&family, Sex::F, wa, Some(hh), None, None);
            g.wed(m, w);
            (m, w)
        };
        match kind {
            HouseKind::Apartment => {
                if r < 0.35 {
                    let sex = if self.rng.chance(0.5) { Sex::M } else { Sex::F };
                    let age_ = self.rng.range(23, 35) as u8;
                    let p = self.person(&family, sex, age_, Some(hh), None, None);
                    self.c.people[p as usize].title = "teacher".into(); // (a teacher posted here: see jobs)
                } else if r < 0.65 {
                    let a = self.rng.range(24, 34) as u8;
                    let (m, w) = couple(self, a);
                    self.children(Some(m), Some(w), hh, 1);
                } else if r < 0.8 {
                    let sex = if self.rng.chance(0.5) { Sex::M } else { Sex::F };
                    let age_ = self.rng.range(22, 45) as u8;
                    self.person(&family, sex, age_, Some(hh), None, None);
                } else if r < 0.9 {
                    let age_ = self.rng.range(28, 44) as u8;
                    let m = self.person(&family, Sex::F, age_, Some(hh), None, None);
                    self.children(None, Some(m), hh, 2);
                } else {
                    let sex = if self.rng.chance(0.75) { Sex::F } else { Sex::M };
                    let age_ = self.rng.range(66, 88) as u8;
                    self.person(&family, sex, age_, Some(hh), None, None);
                }
            }
            HouseKind::Small => {
                if r < 0.5 {
                    let sex = if self.rng.chance(0.75) { Sex::F } else { Sex::M };
                    let age_ = self.rng.range(68, 94) as u8;
                    self.person(&family, sex, age_, Some(hh), None, None);
                } else if r < 0.75 {
                    let a = self.rng.range(64, 86) as u8;
                    couple(self, a);
                } else if r < 0.9 {
                    let sex = if self.rng.chance(0.5) { Sex::M } else { Sex::F };
                    let age_ = self.rng.range(30, 62) as u8;
                    self.person(&family, sex, age_, Some(hh), None, None);
                } else {
                    let a = self.rng.range(25, 33) as u8;
                    let (m, w) = couple(self, a);
                    self.children(Some(m), Some(w), hh, 1);
                }
            }
            HouseKind::Shophouse => {
                let a = self.rng.range(62, 84) as u8;
                couple(self, a);
            }
            _ => {
                if r < 0.5 {
                    let a = self.rng.range(34, 52) as u8;
                    let (m, w) = couple(self, a);
                    self.children(Some(m), Some(w), hh, 3);
                } else if r < 0.92 {
                    let a = self.rng.range(50, 68) as u8;
                    let (m, w) = couple(self, a);
                    self.children(Some(m), Some(w), hh, 2);
                } else {
                    let a = self.rng.range(26, 33) as u8;
                    let (m, w) = couple(self, a);
                    self.children(Some(m), Some(w), hh, 1);
                }
            }
        }
    }

    fn resident(&self, p: &Person) -> bool {
        p.lives_here()
    }

    fn mother_age(&self, p: &Person) -> Option<u8> {
        p.mother.or(p.father).map(|m| self.c.people[m as usize].age)
    }

    fn sibling_has_age(&self, p: &Person, age: u8) -> bool {
        self.c.people.iter().any(|o| o.id != p.id && o.age == age && ((p.mother.is_some() && o.mother == p.mother) || (p.father.is_some() && o.father == p.father)))
    }

    /// Bring the junior high's year groups to the right size by nudging children's ages (within
    /// what their parents' ages allow), adding a child to a family if need be.
    fn fit_school_years(&mut self, targets: &[(u8, usize)]) {
        for &(age, want) in targets {
            for _ in 0..400 {
                let have: Vec<PersonId> = self.c.people.iter().filter(|p| self.resident(p) && p.age == age && p.household != Some(0)).map(|p| p.id).collect();
                if have.len() == want {
                    break;
                }
                if have.len() > want {
                    // move one out of this year (to an age outside the junior high)
                    let pick = have[self.rng.below(have.len())];
                    let p = self.c.people[pick as usize].clone();
                    let ma = self.mother_age(&p).unwrap_or(40);
                    let to = [age.saturating_sub(4), age + 4, age.saturating_sub(3), age + 3, 10, 16]
                        .into_iter()
                        .find(|&t| !(12..=14).contains(&t) && ma as i32 - t as i32 >= 19 && !self.sibling_has_age(&p, t));
                    if let Some(t) = to {
                        self.c.people[pick as usize].age = t;
                    }
                    continue;
                }
                // bring one in: a new child for a family with a mother of the right age (moving
                // a child from a neighbouring year would only leave a gap there); failing that,
                // a child of a nearby age
                if self.rng.chance(0.8) && self.new_child(age) {
                    continue;
                }
                let mut cands: Vec<(i32, PersonId)> = self
                    .c
                    .people
                    .iter()
                    .filter(|p| self.resident(p) && p.household != Some(0) && (p.father.is_some() || p.mother.is_some()))
                    .filter(|p| !(12..=14).contains(&p.age) && (p.age as i32 - age as i32).abs() <= 4)
                    .filter(|p| self.mother_age(p).is_some_and(|m| m as i32 - age as i32 >= 19 && m as i32 - age as i32 <= 46))
                    .filter(|p| !self.sibling_has_age(p, age))
                    .map(|p| ((p.age as i32 - age as i32).abs(), p.id))
                    .collect();
                cands.sort();
                if let Some(&(_, id)) = cands.first() {
                    self.c.people[id as usize].age = age;
                    continue;
                }
                if !self.new_child(age) {
                    break;
                }
            }
        }
    }

    /// A child of this age for a couple in town who could have one (and don't have one this age).
    fn new_child(&mut self, age: u8) -> bool {
        {
            {
                let fams: Vec<(PersonId, Option<PersonId>, HouseholdId)> = self
                    .c
                    .people
                    .iter()
                    .filter(|m| self.resident(m) && m.sex == Sex::F && (age as i32 + 22..=age as i32 + 44).contains(&(m.age as i32)) && m.spouse.is_some())
                    .filter(|m| !self.c.people.iter().any(|k| k.mother == Some(m.id) && k.age == age))
                    .filter_map(|m| Some((m.id, m.spouse, m.household?)))
                    .collect();
                if fams.is_empty() {
                    return false;
                }
                let (m, f, hh) = fams[self.rng.below(fams.len())];
                let family = self.c.households[hh as usize].family_name.clone();
                let sex = if self.rng.chance(0.5) { Sex::M } else { Sex::F };
                self.person(&family, sex, age, Some(hh), f, Some(m));
                true
            }
        }
    }
}

/// Local workplaces: (kind, staff, hours, workdays mask, title).
const LOCAL_JOBS: [(PlaceKind, usize, (u16, u16), u8, &str); 9] = [
    (PlaceKind::CoOp, 6, (510, 1020), 0b0011_1111, "JA farm co-op"),
    (PlaceKind::PostOffice, 4, (540, 1020), 0b0001_1111, "post office"),
    (PlaceKind::TownOffice, 7, (510, 1035), 0b0001_1111, "town office branch"),
    (PlaceKind::Konbini, 7, (360, 840), 0b0111_1111, "part-time at the konbini"),
    (PlaceKind::Drugstore, 5, (540, 1200), 0b0111_1111, "drugstore"),
    (PlaceKind::Petrol, 3, (420, 1140), 0b0111_1111, "petrol station"),
    (PlaceKind::CarRepair, 3, (510, 1080), 0b0011_1111, "car repair"),
    (PlaceKind::Station, 1, (390, 930), 0b0111_1111, "station staff"),
    (PlaceKind::Temple, 0, (360, 1020), 0b0111_1111, "temple"),
];

/// Make the town's people.
pub fn generate(seed: u64, plan: &Plan) -> Census {
    let mut g = G {
        rng: Rng::derive(seed, "census"),
        c: Census { seed, places: Vec::new(), houses: Vec::new(), households: Vec::new(), people: Vec::new(), groups: Vec::new(), player: 0, kenji: 0 },
        slots: Vec::new(),
        local: HashMap::new(),
        brides: Vec::new(),
        used: Default::default(),
    };
    for (name, kind, at) in &plan.sites {
        g.place(name.clone(), *kind, None, *at);
    }
    for h in &plan.houses {
        let place = g.place(String::new(), PlaceKind::Home, Some(h.id), h.at);
        g.c.houses.push(House { id: h.id, district: h.district.clone(), kind: h.kind, at: h.at, households: Vec::new(), place, shop: None });
        g.slots.push(Slot { house: g.c.houses.len() - 1, free: h.flats });
    }
    let mut districts: Vec<String> = g.c.houses.iter().map(|h| h.district.clone()).collect();
    districts.sort();
    districts.dedup();
    for d in &districts {
        let mut s: Vec<&'static str> = names::SURNAMES.to_vec();
        g.rng.shuffle(&mut s);
        g.local.insert(d.clone(), s[..3].to_vec());
    }

    // Tsukumodō: Uncle Kenji and you
    let shop_slot = g.slots.iter().position(|s| g.c.houses[s.house].kind == HouseKind::ShopHome).expect("Tsukumodō on the plan");
    let home = g.new_household(shop_slot, "Tsukumo");
    let kenji = g.person("Tsukumo", Sex::M, 41, Some(home), None, None);
    g.c.people[kenji as usize].given_name = "Kenji".into();
    let player = g.person("Tsukumo", Sex::M, 12, Some(home), None, None);
    g.c.people[player as usize].given_name = "You".into();
    g.c.kenji = kenji;
    g.c.player = player;

    // 1. the old families
    let mut anchors: Vec<usize> = Vec::new();
    for (i, s) in g.slots.iter().enumerate() {
        let p = match g.c.houses[s.house].kind {
            HouseKind::Farmhouse => 1.0,
            HouseKind::Shophouse => 0.8,
            HouseKind::Modern => 0.3,
            HouseKind::Small => 0.25,
            _ => 0.0,
        };
        if s.free > 0 && g.rng.chance(p) {
            anchors.push(i);
        }
    }
    g.rng.shuffle(&mut anchors);
    for s in anchors {
        if g.slots[s].free > 0 {
            g.clan(s);
        }
    }
    // 2. newcomers in every house still empty
    for s in 0..g.slots.len() {
        while g.slots[s].free > 0 {
            g.newcomers(s);
        }
    }
    // 3. the junior high's years: 32 first-years (you are the 33rd), 33, 34
    g.fit_school_years(&[(12, 32), (13, 33), (14, 34)]);

    jobs(&mut g);
    school(&mut g);
    groups(&mut g);
    // household order: oldest first; homes named after their families
    for hh in 0..g.c.households.len() {
        let mut m = g.c.households[hh].members.clone();
        m.sort_by_key(|&p| std::cmp::Reverse(g.c.people[p as usize].age));
        g.c.households[hh].members = m;
    }
    for h in 0..g.c.houses.len() {
        let fams: Vec<String> = g.c.houses[h].households.iter().map(|&x| g.c.households[x as usize].family_name.clone()).collect();
        let place = g.c.houses[h].place as usize;
        g.c.places[place].name = match g.c.houses[h].kind {
            HouseKind::ShopHome => "Tsukumodō".into(),
            HouseKind::Apartment => format!("apartments ({})", g.c.houses[h].district),
            _ => format!("the {} house", fams.first().cloned().unwrap_or_default()),
        };
    }
    // 6. looks
    for i in 0..g.c.people.len() {
        let (sex, age, work, wp) = {
            let p = &g.c.people[i];
            (p.sex, p.age, p.work.clone(), p.workplace.map(|w| g.c.places[w as usize].kind))
        };
        let outdoors = matches!(work, Work::Farmer) || matches!(wp, Some(PlaceKind::Petrol) | Some(PlaceKind::Fields));
        g.c.people[i].look = look::make(&mut g.rng, sex, age, &work, wp, outdoors);
    }
    // Kenji and you look the same in every town
    {
        let k = &mut g.c.people[kenji as usize].look;
        k.hair = look::HairStyle::Spiky;
        k.hair_color = [26, 23, 23];
        k.stubble = true;
        k.glasses = None;
        k.height_cm = 170;
        k.build = look::Build::Sturdy;
        k.accessories = vec![look::Accessory::Headband];
        k.day = look::Clothes { outfit: look::Outfit::ShopApron, colors: [[72, 72, 78], [62, 62, 62], [40, 40, 40], [34, 46, 84]] };
        let y = &mut g.c.people[player as usize].look;
        y.hair = look::HairStyle::Short;
        y.hair_color = [26, 22, 22];
        y.glasses = None;
        y.height_cm = 153;
    }
    g.c
}

fn set_job(g: &mut G, p: PersonId, work: Work, title: &str, place: Option<PlaceId>, hours: Option<(u16, u16)>, days: u8) {
    let x = &mut g.c.people[p as usize];
    x.work = work;
    x.title = title.to_string();
    x.workplace = place;
    x.hours = hours;
    x.workdays = days;
}

/// 4. Jobs and schooling.
fn jobs(g: &mut G) {
    let n = g.c.people.len();
    let place_of = |g: &G, k: PlaceKind| g.c.places.iter().find(|p| p.kind == k).map(|p| p.id);
    let household_kind = |g: &G, p: &Person| p.household.map(|h| g.c.houses.iter().find(|x| x.id == g.c.households[h as usize].house).unwrap().kind);

    // children and young people by age
    for i in 0..n {
        let (age, live) = (g.c.people[i].age, g.c.people[i].lives_here());
        if g.c.people[i].away.is_some() {
            let w = g.c.people[i].away.clone().unwrap();
            let title = if w == "deceased" { "(died)".to_string() } else if w.starts_with("university") { w.clone() } else { format!("lives in {w}") };
            set_job(g, i as PersonId, if age <= 22 && w.starts_with("university") { Work::AwayStudying } else { Work::Retired }, &title, None, None, 0);
            continue;
        }
        if !live {
            continue;
        }
        let (work, title): (Work, String) = match age {
            0..=2 => (Work::Baby, "baby".into()),
            3..=5 => (Work::Preschool, "kindergarten".into()),
            6..=11 => (Work::Elementary { grade: age - 5 }, format!("year {}, Kazehana Elementary", age - 5)),
            12..=14 => (Work::JuniorHigh { grade: age - 11, class: 'A', club: None }, String::new()),
            15..=17 => (Work::HighSchool { grade: age - 14 }, format!("year {}, Minato-shi High School", age - 14)),
            _ => continue,
        };
        set_job(g, i as PersonId, work, &title, None, None, 0);
    }
    let kenji = g.c.kenji;
    let shop = g.c.houses.iter().find(|h| h.kind == HouseKind::ShopHome).unwrap().place;
    set_job(g, kenji, Work::Kenji, "runs Tsukumodō (games and cards)", Some(shop), Some((600, 1200)), 0b0111_1011);
    let player = g.c.player;
    set_job(g, player, Work::Player, "you", None, None, 0);

    // shops: the shophouse families keep a shop, or it has closed
    for h in 0..g.c.houses.len() {
        if g.c.houses[h].kind != HouseKind::Shophouse || g.c.houses[h].households.is_empty() {
            continue;
        }
        let hh = g.c.houses[h].households[0];
        let members = g.c.households[hh as usize].members.clone();
        let family = g.c.households[hh as usize].family_name.clone();
        let keepers: Vec<PersonId> = members.iter().copied().filter(|&m| (25..=84).contains(&g.c.people[m as usize].age) && g.c.people[m as usize].work == Work::Retired && g.c.people[m as usize].title.is_empty()).collect();
        let youngest = keepers.iter().map(|&m| g.c.people[m as usize].age).min().unwrap_or(99);
        let mut open = !keepers.is_empty() && g.rng.chance(if youngest < 65 { 0.45 } else { 0.2 });
        let trade = g.rng.pick(&names::TRADES).to_string();
        // (a town this size has one of most things; a second barber or izakaya at most)
        let same = g.c.houses.iter().filter(|x| x.shop.as_ref().is_some_and(|s| s.open && s.trade == trade)).count();
        if same >= if matches!(trade.as_str(), "izakaya" | "hair salon" | "barber" | "soba shop") { 2 } else { 1 } {
            open = false;
        }
        let at = g.c.houses[h].at;
        let hid = g.c.houses[h].id;
        let place = g.place(format!("{family} {trade}"), PlaceKind::Shop, Some(hid), at);
        g.c.houses[h].shop = Some(Shop { trade: trade.clone(), open, place });
        if open {
            let closed_day = g.rng.below(7) as u8;
            let days = 0b0111_1111 & !(1 << closed_day);
            let hours = if youngest >= 70 { (600, 1020) } else { (570, 1140) };
            // the couple who run it (the younger generation if there is one)
            let mut ks = keepers.clone();
            ks.sort_by_key(|&m| g.c.people[m as usize].age);
            let lead = ks[0];
            let mut run = vec![lead];
            if let Some(s) = g.c.people[lead as usize].spouse.filter(|s| ks.contains(s)) {
                run.push(s);
            }
            for p in run {
                set_job(g, p, Work::Shopkeeper, &format!("runs the {trade}"), Some(place), Some(hours), days);
            }
        }
    }

    // the temple's priest: the old family nearest the temple
    if let Some(t) = place_of(g, PlaceKind::Temple) {
        let tat = g.c.places[t as usize].at;
        let mut best: Option<(f32, PersonId)> = None;
        for p in &g.c.people {
            if p.lives_here() && p.sex == Sex::M && (35..=75).contains(&p.age) && p.work == Work::Retired && p.title.is_empty() {
                let h = g.c.houses.iter().find(|x| x.id == g.c.households[p.household.unwrap() as usize].house).unwrap();
                let d = dist(h.at, tat);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, p.id));
                }
            }
        }
        if let Some((_, p)) = best {
            set_job(g, p, Work::Priest, "priest of Shōrin-ji", Some(t), Some((360, 1020)), 0b0111_1111);
        }
    }

    // teachers: the junior high's twelve and the elementary school's ten
    let jh_roles = [
        "principal", "vice principal", "Japanese, 1-A homeroom", "maths, 1-B homeroom", "science, 2-A homeroom", "English, 2-B homeroom",
        "social studies, 3-A homeroom", "PE, 3-B homeroom", "music", "art and technology", "PE", "school nurse",
    ];
    let es_roles = ["principal", "vice principal", "year 1 homeroom", "year 2 homeroom", "year 3 homeroom", "year 4 homeroom", "year 5 homeroom", "year 6 homeroom", "music", "school nurse"];
    let mut pool: Vec<PersonId> = g.c.people.iter().filter(|p| p.lives_here() && p.title == "teacher").map(|p| p.id).collect();
    let mut locals: Vec<PersonId> = g
        .c
        .people
        .iter()
        .filter(|p| p.lives_here() && (26..=60).contains(&p.age) && p.work == Work::Retired && p.title.is_empty() && household_kind(g, p) != Some(HouseKind::Farmhouse))
        .map(|p| p.id)
        .collect();
    g.rng.shuffle(&mut locals);
    pool.extend(locals);
    let mut pool = pool.into_iter();
    for (school, roles, name) in [(PlaceKind::JuniorHigh, &jh_roles[..], "Kazehana Junior High"), (PlaceKind::Elementary, &es_roles[..], "Kazehana Elementary")] {
        let sp = place_of(g, school);
        for role in roles {
            // the principal and vice principal are the oldest; take someone of a fitting age
            let p = loop {
                let Some(p) = pool.next() else { break None };
                let age = g.c.people[p as usize].age;
                if role.contains("principal") && age < 48 {
                    continue;
                }
                break Some(p);
            };
            let Some(p) = p else { break };
            set_job(g, p, Work::Teacher { school, role: role.to_string() }, &format!("{role}, {name}"), sp, Some((450, 1110)), 0b0001_1111);
        }
    }
    // "teacher" was only a marker for the flats: anyone left over is a teacher in Minato-shi
    for i in 0..n {
        if g.c.people[i].title == "teacher" && g.c.people[i].work == Work::Retired {
            g.c.people[i].title = "teacher at a school in Minato-shi".into();
            g.c.people[i].work = Work::Commuter;
            g.c.people[i].workdays = 0b0001_1111;
        }
    }

    // farms: the farmhouse families farm their paddies
    for h in 0..g.c.houses.len() {
        if g.c.houses[h].kind != HouseKind::Farmhouse {
            continue;
        }
        let hid = g.c.houses[h].id;
        let at = g.c.houses[h].at;
        for &hh in &g.c.houses[h].households.clone() {
            let family = g.c.households[hh as usize].family_name.clone();
            let fields = g.place(format!("the {family} paddies"), PlaceKind::Fields, Some(hid), [at[0] + 25.0, at[1]]);
            for &m in &g.c.households[hh as usize].members.clone() {
                let p = &g.c.people[m as usize];
                if !p.lives_here() || p.work != Work::Retired || !p.title.is_empty() || p.age < 20 {
                    continue;
                }
                let (age, sex) = (p.age, p.sex);
                let farms = match age {
                    20..=49 => g.rng.chance(if sex == Sex::M { 0.35 } else { 0.25 }),
                    50..=64 => g.rng.chance(if sex == Sex::M { 0.75 } else { 0.55 }),
                    65..=82 => g.rng.chance(0.65),
                    _ => false,
                };
                if farms {
                    set_job(g, m, Work::Farmer, if age >= 65 { "still farms the paddies" } else { "rice farmer" }, Some(fields), None, 0b0111_1111);
                } else {
                    // (retired farmers keep the vegetable plot)
                    g.c.people[m as usize].workplace = Some(fields);
                }
            }
        }
    }

    // the town's workplaces
    for (kind, staff, hours, days, title) in LOCAL_JOBS {
        let Some(place) = place_of(g, kind) else { continue };
        let pat = g.c.places[place as usize].at;
        let near: Vec<(f32, PersonId)> = g
            .c
            .people
            .iter()
            .filter(|p| p.lives_here() && (18..=68).contains(&p.age) && p.work == Work::Retired && p.title.is_empty())
            .map(|p| {
                let h = g.c.houses.iter().find(|x| x.id == g.c.households[p.household.unwrap() as usize].house).unwrap();
                (dist(h.at, pat), p.id)
            })
            .collect();
        let mut c: Vec<(f32, PersonId)> = near.into_iter().map(|(d, p)| (d * (0.3 + 2.0 * g.rng.f32()), p)).collect();
        c.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut per_household: HashMap<HouseholdId, usize> = HashMap::new();
        let mut i = 0;
        for (_, p) in c {
            if i == staff {
                break;
            }
            let hh = g.c.people[p as usize].household.unwrap();
            let k = per_household.entry(hh).or_default();
            if *k >= 1 {
                continue;
            }
            *k += 1;
            // shifts at the places open long hours: an early and a late one
            let mid = (hours.0 + hours.1) / 2;
            let h = match kind {
                PlaceKind::Konbini => if i % 2 == 0 { (360, 840) } else { (840, 1320) },
                PlaceKind::Drugstore | PlaceKind::Petrol => if i % 2 == 0 { (hours.0, mid) } else { (mid, hours.1) },
                _ => hours,
            };
            set_job(g, p, Work::Local, title, Some(place), Some(h), days);
            i += 1;
        }
    }

    // everyone else of working age: the train to Minato-shi, the farm, the home
    for i in 0..n {
        let p = &g.c.people[i];
        if !p.lives_here() || p.work != Work::Retired || !p.title.is_empty() {
            continue;
        }
        let (age, sex) = (p.age, p.sex);
        if age >= 65 {
            g.c.people[i].title = "retired".into();
            continue;
        }
        if age < 18 {
            continue;
        }
        let r = g.rng.f32();
        let (work, title) = if sex == Sex::M {
            if r < 0.5 {
                (Work::Commuter, *g.rng.pick(&["office in Minato-shi", "factory in Minato-shi", "city hall, Minato-shi", "bank in Minato-shi"]))
            } else if r < 0.8 {
                (Work::Commuter, *g.rng.pick(&["construction", "truck driver", "electrician", "builder"]))
            } else if r < 0.92 {
                (Work::Commuter, *g.rng.pick(&["care home in Minato-shi", "hospital in Minato-shi"]))
            } else {
                (Work::Homemaker, "between jobs")
            }
        } else if r < 0.4 {
            (Work::Commuter, *g.rng.pick(&["nurse at the hospital in Minato-shi", "office in Minato-shi", "care home in Minato-shi", "supermarket in Minato-shi", "school lunch centre"]))
        } else if r < 0.6 {
            (Work::Commuter, "part-time in Minato-shi")
        } else {
            (Work::Homemaker, "homemaker")
        };
        let days = if work == Work::Commuter { 0b0001_1111 } else { 0 };
        set_job(g, i as PersonId, work, title, None, None, days);
    }
}

/// Classes and clubs at the junior high.
fn school(g: &mut G) {
    let player = g.c.player;
    for grade in 1..=3u8 {
        let mut ids: Vec<PersonId> = g.c.people.iter().filter(|p| matches!(p.work, Work::JuniorHigh { grade: gr, .. } if gr == grade) && p.id != player).map(|p| p.id).collect();
        g.rng.shuffle(&mut ids);
        // boys and girls dealt alternately into A and B
        ids.sort_by_key(|&p| g.c.people[p as usize].sex == Sex::F);
        let mut a = if grade == 1 { 1 } else { 0 }; // (you are in 1-A)
        let mut b = 0;
        for p in ids {
            let class = if a <= b { a += 1; 'A' } else { b += 1; 'B' };
            let sex = g.c.people[p as usize].sex;
            let club = if g.rng.chance(0.85) {
                let w: &[(Club, u32)] = if sex == Sex::M {
                    &[(Club::Soccer, 25), (Club::Baseball, 20), (Club::Kendo, 10), (Club::Track, 12), (Club::Tennis, 10), (Club::BrassBand, 5), (Club::Art, 4), (Club::Science, 6), (Club::Card, 8)]
                } else {
                    &[(Club::Tennis, 20), (Club::BrassBand, 22), (Club::Track, 12), (Club::Art, 15), (Club::Kendo, 6), (Club::Science, 6), (Club::Card, 6), (Club::Soccer, 3)]
                };
                let total: u32 = w.iter().map(|x| x.1).sum();
                let mut r = g.rng.below(total as usize) as u32;
                let mut out = w[0].0;
                for &(c, x) in w {
                    if r < x {
                        out = c;
                        break;
                    }
                    r -= x;
                }
                Some(out)
            } else {
                None
            };
            let title = format!("{grade}-{class}, Kazehana Junior High{}", club.map(|c| format!(" ({} club)", c.name())).unwrap_or_default());
            g.c.people[p as usize].work = Work::JuniorHigh { grade, class, club };
            g.c.people[p as usize].title = title;
        }
    }
    g.c.people[player as usize].title = "1-A, Kazehana Junior High (you)".into();
}

fn add_group(g: &mut G, kind: GroupKind, name: String, members: Vec<PersonId>, households: Vec<HouseholdId>, meets: Option<Meeting>) -> GroupId {
    let id = g.c.groups.len() as GroupId;
    for &m in &members {
        g.c.people[m as usize].groups.push(id);
    }
    g.c.groups.push(Group { id, kind, name, members, households, meets });
    id
}

fn nearest_place(g: &G, at: [f32; 2], kinds: &[PlaceKind]) -> Option<PlaceId> {
    g.c.places.iter().filter(|p| kinds.contains(&p.kind)).min_by(|a, b| dist(a.at, at).total_cmp(&dist(b.at, at))).map(|p| p.id)
}

const ALL_DAYS: u8 = 0b0111_1111;

/// 5. Groups.
fn groups(g: &mut G) {
    let hm = |h: u16, m: u16| h * 60 + m;
    let mut districts: Vec<String> = g.c.houses.iter().map(|h| h.district.clone()).collect();
    districts.sort();
    districts.dedup();
    let house_at = |g: &G, hh: HouseholdId| g.c.houses.iter().find(|x| x.id == g.c.households[hh as usize].house).unwrap().at;
    let district_of = |g: &G, hh: HouseholdId| g.c.houses.iter().find(|x| x.id == g.c.households[hh as usize].house).unwrap().district.clone();

    // neighbourhood blocks: about ten nearby households in a district
    for d in &districts {
        let mut left: Vec<HouseholdId> = g.c.households.iter().filter(|h| district_of(g, h.id) == *d).map(|h| h.id).collect();
        let mut n = 1;
        while !left.is_empty() {
            let first = left.remove(0);
            let a = house_at(g, first);
            left.sort_by(|x, y| dist(house_at(g, *x), a).total_cmp(&dist(house_at(g, *y), a)));
            let take = left.len().min(9);
            let mut hs: Vec<HouseholdId> = vec![first];
            hs.extend(left.drain(..take));
            let adults: Vec<PersonId> = hs.iter().flat_map(|&h| g.c.households[h as usize].members.clone()).filter(|&p| g.c.people[p as usize].age >= 18 && g.c.people[p as usize].lives_here()).collect();
            let place = g.c.houses.iter().find(|x| x.id == g.c.households[first as usize].house).unwrap().place;
            let id = add_group(g, GroupKind::Han, format!("{d} block {n}"), adults, hs.clone(), Some(Meeting { place, days: 1 << 6, from: hm(8, 0), to: hm(9, 0), monthly: true }));
            for h in hs {
                g.c.households[h as usize].han = id;
            }
            n += 1;
        }
    }

    // elementary kids of each district walk to school together and play after school
    for d in &districts {
        let kids: Vec<PersonId> = g.c.people.iter().filter(|p| matches!(p.work, Work::Elementary { .. }) && p.household.is_some_and(|h| district_of(g, h) == *d)).map(|p| p.id).collect();
        if kids.is_empty() {
            continue;
        }
        let centre = {
            let pts: Vec<[f32; 2]> = kids.iter().map(|&k| house_at(g, g.c.people[k as usize].household.unwrap())).collect();
            centroid(&pts)
        };
        let place = nearest_place(g, centre, &[PlaceKind::InariShrine, PlaceKind::Shrine, PlaceKind::Temple, PlaceKind::StationPlaza, PlaceKind::Elementary]);
        if let Some(place) = place {
            add_group(g, GroupKind::KidsAssociation, format!("{d} kids' association"), kids, Vec::new(), Some(Meeting { place, days: ALL_DAYS, from: hm(10, 0), to: hm(17, 0), monthly: false }));
        }
    }

    // junior high friend groups: same year, mostly same sex, same class or club or district
    for grade in 1..=3u8 {
        let ids: Vec<PersonId> = g.c.people.iter().filter(|p| matches!(p.work, Work::JuniorHigh { grade: gr, .. } if gr == grade) || (grade == 1 && p.work == Work::Player)).map(|p| p.id).collect();
        let info = |g: &G, p: PersonId| {
            let x = &g.c.people[p as usize];
            let (class, club) = match &x.work {
                Work::JuniorHigh { class, club, .. } => (*class, *club),
                _ => ('A', None),
            };
            (x.sex, class, club, x.household.map(|h| district_of(g, h)).unwrap_or_default())
        };
        let mut left: Vec<PersonId> = ids.iter().copied().filter(|&p| p != g.c.player).collect();
        g.rng.shuffle(&mut left);
        let mut n = 0;
        while let Some(seed) = left.pop() {
            if g.rng.chance(0.1) {
                continue; // (a few keep to themselves)
            }
            let si = info(g, seed);
            let base: Vec<(i32, PersonId)> = left
                .iter()
                .map(|&o| {
                    let oi = info(g, o);
                    (4 * (oi.0 == si.0) as i32 + 3 * (oi.1 == si.1) as i32 + 3 * (oi.2.is_some() && oi.2 == si.2) as i32 + 2 * (oi.3 == si.3) as i32, o)
                })
                .collect();
            let mut scored: Vec<(i32, PersonId)> = base.into_iter().map(|(s, o)| (s + g.rng.below(2) as i32, o)).collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            let k = g.rng.range(2, 4) as usize;
            let mut members = vec![seed];
            for (s, o) in scored.into_iter().take(k) {
                if s >= 5 {
                    members.push(o);
                }
            }
            if members.len() < 2 {
                continue;
            }
            left.retain(|x| !members.contains(x));
            let anchor = house_at(g, g.c.people[members[0] as usize].household.unwrap());
            let place = match g.rng.below(4) {
                0 => g.c.houses.iter().find(|x| x.id == g.c.households[g.c.people[members[0] as usize].household.unwrap() as usize].house).map(|h| h.place),
                _ => nearest_place(g, anchor, &[PlaceKind::Konbini, PlaceKind::StationPlaza, PlaceKind::Shrine, PlaceKind::InariShrine, PlaceKind::Temple]),
            };
            n += 1;
            let sex = if g.c.people[seed as usize].sex == Sex::M { "boys" } else { "girls" };
            add_group(g, GroupKind::Friends, format!("year {grade} {sex}, group {n}"), members, Vec::new(), place.map(|place| Meeting { place, days: ALL_DAYS, from: hm(13, 30), to: hm(18, 30), monthly: false }));
        }
    }

    // clubs
    for club in Club::ALL {
        let members: Vec<PersonId> = g.c.people.iter().filter(|p| matches!(p.work, Work::JuniorHigh { club: Some(c), .. } if c == club)).map(|p| p.id).collect();
        if members.is_empty() {
            continue;
        }
        let jh = nearest_place(g, [0.0, 0.0], &[PlaceKind::JuniorHigh]);
        add_group(g, GroupKind::Club, format!("{} club", club.name()), members, Vec::new(), jh.map(|place| Meeting { place, days: 0b0001_1011, from: hm(15, 50), to: hm(17, 50), monthly: false }));
    }

    // mothers at home with kids at the schools, by district
    for d in &districts {
        let mut moms: Vec<PersonId> = g
            .c
            .people
            .iter()
            .filter(|m| m.sex == Sex::F && m.lives_here() && m.work == Work::Homemaker && m.household.is_some_and(|h| district_of(g, h) == *d))
            .filter(|m| g.c.people.iter().any(|k| k.mother == Some(m.id) && matches!(k.work, Work::Elementary { .. } | Work::JuniorHigh { .. } | Work::Preschool)))
            .map(|m| m.id)
            .collect();
        g.rng.shuffle(&mut moms);
        for (i, chunk) in moms.chunks(4).enumerate() {
            if chunk.len() < 2 {
                continue;
            }
            let at = house_at(g, g.c.people[chunk[0] as usize].household.unwrap());
            let place = if i % 2 == 0 { nearest_place(g, at, &[PlaceKind::Konbini, PlaceKind::Drugstore]) } else { g.c.people[chunk[1] as usize].household.map(|h| g.c.houses.iter().find(|x| x.id == g.c.households[h as usize].house).unwrap().place) };
            add_group(g, GroupKind::Mothers, format!("{d} mothers {}", i + 1), chunk.to_vec(), Vec::new(), place.map(|place| Meeting { place, days: 1 << 2, from: hm(10, 0), to: hm(11, 30), monthly: false }));
        }
    }

    // seniors' clubs by district; the gateball team
    for d in &districts {
        let old: Vec<PersonId> = g.c.people.iter().filter(|p| p.lives_here() && p.age >= 65 && p.household.is_some_and(|h| district_of(g, h) == *d)).map(|p| p.id).collect();
        if old.len() < 3 {
            continue;
        }
        let at = house_at(g, g.c.people[old[0] as usize].household.unwrap());
        let place = nearest_place(g, at, &[PlaceKind::Temple, PlaceKind::Shrine, PlaceKind::InariShrine, PlaceKind::TownOffice]);
        add_group(g, GroupKind::Seniors, format!("{d} seniors' club"), old, Vec::new(), place.map(|place| Meeting { place, days: 1 << 3, from: hm(10, 0), to: hm(12, 0), monthly: true }));
    }
    if let Some(es) = nearest_place(g, [0.0, 0.0], &[PlaceKind::Elementary]) {
        let at = g.c.places[es as usize].at;
        let near: Vec<(f32, PersonId)> = g.c.people.iter().filter(|p| p.lives_here() && (65..=84).contains(&p.age) && matches!(p.work, Work::Retired)).map(|p| (dist(house_at(g, p.household.unwrap()), at), p.id)).collect();
        let mut c: Vec<(f32, PersonId)> = near.into_iter().map(|(d, p)| (d * (0.6 + g.rng.f32()), p)).collect();
        c.sort_by(|a, b| a.0.total_cmp(&b.0));
        let team: Vec<PersonId> = c.into_iter().take(14).map(|x| x.1).collect();
        add_group(g, GroupKind::Gateball, "Kazehana gateball club".into(), team, Vec::new(), Some(Meeting { place: es, days: 0b0001_0010, from: hm(8, 30), to: hm(10, 30), monthly: false }));
    }

    // the fire brigade: local men, drill the first Sunday morning
    if let Some(to) = nearest_place(g, [0.0, 0.0], &[PlaceKind::TownOffice]) {
        let mut men: Vec<PersonId> = g.c.people.iter().filter(|p| p.lives_here() && p.sex == Sex::M && (20..=52).contains(&p.age) && !matches!(p.work, Work::Teacher { .. } | Work::Kenji)).map(|p| p.id).collect();
        g.rng.shuffle(&mut men);
        men.sort_by_key(|&p| !matches!(g.c.people[p as usize].work, Work::Farmer | Work::Local | Work::Shopkeeper));
        men.truncate(18);
        add_group(g, GroupKind::FireBrigade, "Kazehana volunteer fire brigade".into(), men, Vec::new(), Some(Meeting { place: to, days: 1 << 6, from: hm(7, 0), to: hm(9, 0), monthly: true }));
    }

    // the junior high PTA committee
    if let Some(jh) = nearest_place(g, [0.0, 0.0], &[PlaceKind::JuniorHigh]) {
        let mut parents: Vec<PersonId> = g.c.people.iter().filter(|p| matches!(p.work, Work::JuniorHigh { .. })).filter_map(|k| k.mother.or(k.father)).filter(|&p| g.c.people[p as usize].lives_here()).collect();
        parents.sort();
        parents.dedup();
        g.rng.shuffle(&mut parents);
        parents.truncate(6);
        add_group(g, GroupKind::Pta, "Kazehana Junior High PTA committee".into(), parents, Vec::new(), Some(Meeting { place: jh, days: 1 << 1, from: hm(19, 0), to: hm(20, 30), monthly: true }));
    }

    // Kenji's old classmates who stayed in town
    let kenji = g.c.kenji;
    let mut friends: Vec<PersonId> = g.c.people.iter().filter(|p| p.lives_here() && p.sex == Sex::M && (39..=44).contains(&p.age) && p.id != kenji).map(|p| p.id).collect();
    g.rng.shuffle(&mut friends);
    friends.truncate(3);
    friends.insert(0, kenji);
    let izakaya = g.c.houses.iter().find_map(|h| h.shop.as_ref().filter(|s| s.open && s.trade == "izakaya").map(|s| s.place));
    let place = izakaya.or_else(|| g.c.people[friends[1.min(friends.len() - 1)] as usize].household.map(|h| g.c.houses.iter().find(|x| x.id == g.c.households[h as usize].house).unwrap().place));
    add_group(g, GroupKind::OldFriends, "Kenji's old classmates".into(), friends, Vec::new(), place.map(|place| Meeting { place, days: 1 << 4, from: hm(20, 30), to: hm(23, 0), monthly: true }));
}
