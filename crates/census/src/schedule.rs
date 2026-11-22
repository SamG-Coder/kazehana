//! Where everyone is through the day. A routine comes from what someone does (school, the
//! shop, the paddies, the train to Minato-shi...), their hours, the season, and the groups they
//! belong to (club practice, gateball, the mothers' coffee, friends hanging out). Small daily
//! variations come from a hash of the person and the date, so the same day always plays out the
//! same way.

use crate::{Census, GroupKind, Person, PersonId, PlaceId, PlaceKind, Work};
use serde::{Deserialize, Serialize};

/// A day, as schedules see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Day {
    /// 0 = Monday .. 6 = Sunday.
    pub weekday: u8,
    pub month: u8,
    pub date: u8,
    /// Lessons today (not a weekend, holiday or school break).
    pub school_day: bool,
}

impl Day {
    pub fn weekend(&self) -> bool {
        self.weekday >= 5
    }
    fn key(&self) -> u64 {
        (self.month as u64) * 32 + self.date as u64
    }
}

/// Where someone is and what they're doing. `place: None` is somewhere not in town (Minato-shi,
/// away) or on the way between places.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    pub place: Option<PlaceId>,
    pub doing: &'static str,
    pub group: Option<crate::GroupId>,
}

pub const fn hm(h: u16, m: u16) -> u16 {
    h * 60 + m
}

fn roll(id: PersonId, day: &Day, salt: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for x in [id as u64, day.key(), day.weekday as u64, salt] {
        h = (h ^ x).wrapping_mul(0x100_0000_01b3);
        h ^= h >> 29;
    }
    h % 100
}

fn at(place: Option<PlaceId>, doing: &'static str) -> Activity {
    Activity { place, doing, group: None }
}

/// A group of the person's that meets now, if any.
fn meeting(c: &Census, p: &Person, day: &Day, minute: u16, kinds: &[GroupKind]) -> Option<Activity> {
    for &g in &p.groups {
        let g = c.group(g);
        if !kinds.contains(&g.kind) {
            continue;
        }
        let Some(m) = &g.meets else { continue };
        if m.days & (1 << day.weekday) == 0 || !(m.from..m.to).contains(&minute) {
            continue;
        }
        if m.monthly && day.date > 7 {
            continue;
        }
        let doing = match g.kind {
            GroupKind::Gateball => "playing gateball",
            GroupKind::Seniors => "at the seniors' club",
            GroupKind::FireBrigade => "fire brigade drill",
            GroupKind::Han => "neighbourhood clean-up",
            GroupKind::Mothers => "chatting with the other mothers",
            GroupKind::Pta => "PTA meeting",
            GroupKind::OldFriends => "out with old friends",
            GroupKind::Club => "club practice",
            GroupKind::Friends => "hanging out with friends",
            GroupKind::KidsAssociation => "playing with the neighbourhood kids",
        };
        return Some(Activity { place: Some(m.place), doing, group: Some(g.id) });
    }
    None
}

/// Where someone is at a minute of a day.
pub fn activity(c: &Census, id: PersonId, day: Day, minute: u16) -> Activity {
    let p = c.person(id);
    if p.away.is_some() {
        return at(None, "away");
    }
    let home = c.home(id);
    let d = &day;
    let work_now = |p: &Person| -> bool {
        match p.hours {
            Some((a, b)) => p.workdays & (1 << d.weekday) != 0 && (a..b).contains(&minute),
            None => false,
        }
    };
    let sleep = |wake: u16, bed: u16| minute < wake || minute >= bed;
    let station = c.place_of(PlaceKind::Station).map(|p| p.id);
    let kind_place = |k: PlaceKind| c.place_of(k).map(|p| p.id);
    match &p.work {
        Work::Player => at(home, "you decide"),
        Work::Baby => at(home, if sleep(hm(7, 0), hm(19, 30)) { "asleep" } else { "at home" }),
        Work::Preschool => {
            if sleep(hm(6, 45), hm(20, 30)) {
                at(home, "asleep")
            } else if !d.weekend() && roll(id, d, 1) < 60 && (hm(8, 30)..hm(15, 0)).contains(&minute) {
                at(None, "at kindergarten in Minato-shi")
            } else {
                at(home, "at home")
            }
        }
        Work::Elementary { grade } => {
            if sleep(hm(6, 30), hm(21, 0)) {
                return at(home, "asleep");
            }
            let out = if *grade <= 2 { hm(14, 30) } else { hm(15, 15) };
            if d.school_day {
                if (hm(7, 30)..hm(8, 0)).contains(&minute) {
                    return Activity { place: None, doing: "walking to school with the neighbourhood kids", group: p.groups.iter().copied().find(|&g| c.group(g).kind == GroupKind::KidsAssociation) };
                }
                if (hm(8, 0)..out).contains(&minute) {
                    return at(kind_place(PlaceKind::Elementary), "lessons");
                }
                if (out..hm(17, 0)).contains(&minute) && roll(id, d, 2) < 60 {
                    if let Some(a) = meeting(c, p, d, minute, &[GroupKind::KidsAssociation]) {
                        return a;
                    }
                }
            } else if (hm(10, 0)..hm(12, 0)).contains(&minute) || (hm(14, 0)..hm(17, 0)).contains(&minute) {
                if roll(id, d, 3) < 50 {
                    if let Some(a) = meeting(c, p, d, minute, &[GroupKind::KidsAssociation]) {
                        return a;
                    }
                }
            }
            at(home, "at home")
        }
        Work::JuniorHigh { club, .. } => {
            if sleep(hm(6, 30), hm(22, 30)) {
                return at(home, "asleep");
            }
            let jh = kind_place(PlaceKind::JuniorHigh);
            if d.school_day {
                if (hm(7, 40)..hm(8, 10)).contains(&minute) {
                    return at(None, "walking to school");
                }
                if (hm(8, 10)..hm(15, 40)).contains(&minute) {
                    return at(jh, "lessons");
                }
                // clubs every weekday but Wednesday
                if club.is_some() && d.weekday != 2 && (hm(15, 50)..hm(17, 50)).contains(&minute) {
                    return at(jh, "club practice");
                }
                if (hm(15, 40)..hm(18, 30)).contains(&minute) && roll(id, d, 4) < 55 {
                    if let Some(a) = meeting(c, p, d, minute, &[GroupKind::Friends]) {
                        return a;
                    }
                }
                return at(home, "at home");
            }
            if d.weekday == 5 && club.is_some_and(|c| c.outdoor()) && (hm(8, 30)..hm(12, 0)).contains(&minute) {
                return at(jh, "Saturday club practice");
            }
            if (hm(13, 30)..hm(17, 30)).contains(&minute) && roll(id, d, 5) < if d.weekday == 6 { 35 } else { 50 } {
                if let Some(a) = meeting(c, p, d, minute, &[GroupKind::Friends]) {
                    return a;
                }
            }
            at(home, "at home")
        }
        Work::HighSchool { .. } => {
            if sleep(hm(6, 0), hm(23, 0)) {
                return at(home, "asleep");
            }
            if d.school_day {
                if (hm(6, 40)..hm(6, 55)).contains(&minute) || (hm(18, 40)..hm(18, 55)).contains(&minute) {
                    return at(station, "on the train to high school");
                }
                if (hm(6, 55)..hm(18, 40)).contains(&minute) {
                    return at(None, "at high school in Minato-shi");
                }
            } else if (hm(13, 0)..hm(17, 0)).contains(&minute) && roll(id, d, 6) < 40 {
                return at(kind_place(if roll(id, d, 7) < 50 { PlaceKind::Konbini } else { PlaceKind::StationPlaza }), "hanging around");
            }
            at(home, "at home")
        }
        Work::AwayStudying => at(None, "away"),
        Work::Kenji => {
            if sleep(hm(7, 0), hm(24, 0)) {
                return at(home, "asleep");
            }
            if work_now(p) {
                return at(home, "running Tsukumodō");
            }
            at(home, "at home")
        }
        Work::Shopkeeper | Work::Local | Work::Priest => {
            let wake = if matches!(p.work, Work::Priest) { hm(5, 30) } else { hm(6, 0) };
            if sleep(wake, hm(23, 0)) {
                return at(home, "asleep");
            }
            if work_now(p) {
                let doing = match p.work {
                    Work::Shopkeeper => "minding the shop",
                    Work::Priest => "at the temple",
                    _ => "at work",
                };
                return at(p.workplace, doing);
            }
            off_duty(c, p, d, minute, home)
        }
        Work::Teacher { school, .. } => {
            if sleep(hm(6, 0), hm(23, 30)) {
                return at(home, "asleep");
            }
            if d.school_day && (hm(7, 30)..hm(18, 30)).contains(&minute) {
                return at(kind_place(*school), "teaching");
            }
            if d.weekday == 5 && roll(id, d, 8) < 40 && (hm(8, 30)..hm(12, 0)).contains(&minute) {
                return at(kind_place(*school), "watching Saturday club practice");
            }
            off_duty(c, p, d, minute, home)
        }
        Work::Commuter => {
            if sleep(hm(5, 45), hm(23, 30)) {
                return at(home, "asleep");
            }
            if !d.weekend() {
                if (hm(6, 50)..hm(7, 10)).contains(&minute) || (hm(19, 10)..hm(19, 25)).contains(&minute) {
                    return at(station, "on the train");
                }
                if (hm(7, 10)..hm(19, 10)).contains(&minute) {
                    return at(None, "at work in Minato-shi");
                }
            }
            off_duty(c, p, d, minute, home)
        }
        Work::Farmer => {
            if sleep(hm(5, 0), hm(21, 30)) {
                return at(home, "asleep");
            }
            let fields = p.workplace;
            let busy = match d.month {
                4 | 5 => (hm(6, 0)..hm(11, 30)).contains(&minute) || (hm(13, 30)..hm(17, 30)).contains(&minute),
                6..=8 => (hm(5, 30)..hm(9, 0)).contains(&minute) || (hm(16, 0)..hm(18, 30)).contains(&minute),
                9 | 10 => (hm(7, 0)..hm(12, 0)).contains(&minute) || (hm(13, 0)..hm(17, 0)).contains(&minute),
                _ => roll(id, d, 9) < 40 && (hm(9, 0)..hm(11, 30)).contains(&minute),
            };
            if busy && roll(id, d, 10) < 85 {
                return at(fields, "working in the paddies");
            }
            off_duty(c, p, d, minute, home)
        }
        Work::Homemaker | Work::Retired => {
            let old = p.age >= 65;
            if sleep(if old { hm(5, 30) } else { hm(6, 0) }, if old { hm(21, 0) } else { hm(23, 0) }) {
                return at(home, "asleep");
            }
            off_duty(c, p, d, minute, home)
        }
    }
}

/// Time off: groups, errands, walks, the vegetable plot, home.
fn off_duty(c: &Census, p: &Person, d: &Day, minute: u16, home: Option<PlaceId>) -> Activity {
    if let Some(a) = meeting(c, p, d, minute, &[GroupKind::Gateball, GroupKind::Seniors, GroupKind::FireBrigade, GroupKind::Han, GroupKind::Mothers, GroupKind::Pta, GroupKind::OldFriends]) {
        return a;
    }
    let kind_place = |k: PlaceKind| c.place_of(k).map(|p| p.id);
    let id = p.id;
    let old = p.age >= 65;
    if old && (hm(6, 0)..hm(7, 0)).contains(&minute) && roll(id, d, 11) < 55 {
        let k = if roll(id, d, 12) < 50 { PlaceKind::Shrine } else { PlaceKind::Temple };
        return at(kind_place(k), "morning walk");
    }
    if (hm(10, 0)..hm(11, 0)).contains(&minute) && roll(id, d, 13) < 30 {
        let k = if roll(id, d, 14) < 60 { PlaceKind::Konbini } else { PlaceKind::Drugstore };
        return at(kind_place(k), "shopping");
    }
    if old && (hm(14, 0)..hm(16, 0)).contains(&minute) && roll(id, d, 15) < 25 {
        let k = [PlaceKind::Temple, PlaceKind::StationPlaza, PlaceKind::Shrine, PlaceKind::PostOffice][roll(id, d, 16) as usize % 4];
        return at(kind_place(k), "chatting");
    }
    // a vegetable plot: retired farmers and their wives keep at it
    if old && p.workplace.is_some_and(|w| c.place(w).kind == PlaceKind::Fields) && (hm(8, 0)..hm(10, 30)).contains(&minute) && roll(id, d, 17) < 50 {
        return at(p.workplace, "tending the vegetable plot");
    }
    at(home, "at home")
}
