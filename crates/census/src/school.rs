//! Kazehana Junior High's day and week (docs/school_plan.md): the bells, and the timetable of its
//! six classes (1-A to 3-B), who teaches each lesson and where.
//!
//! The day (a rural public junior high, as MEXT's hours ask for):
//!
//! ```text
//!  8:10  arrive: shoes into the shoe lockers, indoor shoes on
//!  8:20  morning reading (asa-dokusho)       8:30  morning meeting (asa no kai)
//!  8:50  1st period   9:50  2nd   10:50  3rd   11:50  4th      (50 minutes, 10 between)
//! 12:40  school lunch (kyūshoku), served and eaten in the classroom
//! 13:05  lunch break      13:25  cleaning (sōji)
//! 13:50  5th period  14:50  6th (not on Wednesdays)
//! 15:40  end-of-day meeting (kaeri no kai)  (Wednesday: 14:40)
//! 16:00  clubs, till 18:00 (none on Wednesdays)
//! ```
//!
//! Each class has 29 lessons a week: Japanese 4, maths 4, English 4, science 3, social studies 3,
//! PE 3, technology and home economics 2, music 1, art 1, moral education 1, integrated studies 2,
//! class activities (LHR) 1. Each subject teacher takes their subject in the years they're given;
//! the homeroom teacher takes their own class's moral education, integrated studies and LHR. Two
//! part-time lecturers come in from Minato-shi for 3rd-year maths and English (small schools are
//! staffed that way), and the vice principal teaches the 3rd years' Japanese.
//!
//! The timetable is built once from the census's teachers, deterministically: no teacher in two
//! rooms at once, no subject twice in a day for a class.

use crate::{Census, PersonId, Work};
use serde::{Deserialize, Serialize};

pub const fn hm(h: u32, m: u32) -> u32 {
    h * 60 + m
}

/// What the school is doing at a moment of a school day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Slot {
    /// Before 8:20: arriving (the gate opens at 7:50).
    Arrival,
    MorningReading,
    MorningMeeting,
    /// A lesson: the period (1..=6).
    Period(u8),
    /// The ten minutes between periods.
    Break,
    Lunch,
    LunchBreak,
    Cleaning,
    EndMeeting,
    /// After the end meeting: clubs (16:00 to 18:00, not Wednesdays), or going home.
    AfterSchool,
}

impl Slot {
    pub fn name(self) -> String {
        match self {
            Slot::Arrival => "before school".into(),
            Slot::MorningReading => "morning reading".into(),
            Slot::MorningMeeting => "morning meeting".into(),
            Slot::Period(p) => format!("{} period", ordinal(p)),
            Slot::Break => "break".into(),
            Slot::Lunch => "school lunch".into(),
            Slot::LunchBreak => "lunch break".into(),
            Slot::Cleaning => "cleaning".into(),
            Slot::EndMeeting => "end-of-day meeting".into(),
            Slot::AfterSchool => "after school".into(),
        }
    }
    /// In the classroom with the class (lessons, meetings, reading, lunch, cleaning).
    pub fn with_class(self) -> bool {
        !matches!(self, Slot::Arrival | Slot::Break | Slot::LunchBreak | Slot::AfterSchool)
    }
}

pub fn ordinal(n: u8) -> &'static str {
    ["0th", "1st", "2nd", "3rd", "4th", "5th", "6th"][n.min(6) as usize]
}

/// The bells: each period's start and end (minutes after midnight).
pub const PERIODS: [(u32, u32); 6] = [(hm(8, 50), hm(9, 40)), (hm(9, 50), hm(10, 40)), (hm(10, 50), hm(11, 40)), (hm(11, 50), hm(12, 40)), (hm(13, 50), hm(14, 40)), (hm(14, 50), hm(15, 40))];
pub const GATE_OPENS: u32 = hm(7, 50);
pub const MORNING_READING: u32 = hm(8, 20);
pub const MORNING_MEETING: u32 = hm(8, 30);
pub const LUNCH: u32 = hm(12, 40);
pub const LUNCH_BREAK: u32 = hm(13, 5);
pub const CLEANING: u32 = hm(13, 25);
pub const CLEANING_END: u32 = hm(13, 40);
pub const CLUBS: (u32, u32) = (hm(16, 0), hm(18, 0));

/// How many periods a weekday has (0 = Monday): six, five on Wednesdays.
pub fn periods_on(weekday: u8) -> u8 {
    if weekday == 2 {
        5
    } else {
        6
    }
}

/// When the end-of-day meeting starts and ends on a weekday.
pub fn end_meeting(weekday: u8) -> (u32, u32) {
    let last = PERIODS[periods_on(weekday) as usize - 1].1;
    (last, last + 10)
}

/// Whether clubs meet on a weekday.
pub fn clubs_on(weekday: u8) -> bool {
    weekday < 5 && weekday != 2
}

/// What the school is doing at `minute` on a school day.
pub fn slot_at(weekday: u8, minute: u32) -> Slot {
    let (em0, em1) = end_meeting(weekday);
    if minute < MORNING_READING {
        return Slot::Arrival;
    }
    if minute < MORNING_MEETING {
        return Slot::MorningReading;
    }
    if minute < PERIODS[0].0 && minute < MORNING_MEETING + 10 {
        return Slot::MorningMeeting;
    }
    for (i, &(a, b)) in PERIODS.iter().enumerate().take(periods_on(weekday) as usize) {
        if (a..b).contains(&minute) {
            return Slot::Period(i as u8 + 1);
        }
    }
    match minute {
        m if (LUNCH..LUNCH_BREAK).contains(&m) => Slot::Lunch,
        m if (LUNCH_BREAK..CLEANING).contains(&m) => Slot::LunchBreak,
        m if (CLEANING..CLEANING_END).contains(&m) => Slot::Cleaning,
        m if (em0..em1).contains(&m) => Slot::EndMeeting,
        m if m >= em1 => Slot::AfterSchool,
        _ => Slot::Break,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Subject {
    Japanese,
    Maths,
    English,
    Science,
    Social,
    Pe,
    TechHome,
    Music,
    Art,
    Moral,
    Integrated,
    ClassActivities,
}

impl Subject {
    /// A class's lessons of each subject in a week.
    pub const WEEK: [(Subject, u8); 12] = [
        (Subject::Japanese, 4),
        (Subject::Maths, 4),
        (Subject::English, 4),
        (Subject::Science, 3),
        (Subject::Social, 3),
        (Subject::Pe, 3),
        (Subject::TechHome, 2),
        (Subject::Music, 1),
        (Subject::Art, 1),
        (Subject::Moral, 1),
        (Subject::Integrated, 2),
        (Subject::ClassActivities, 1),
    ];
    pub fn name(self) -> &'static str {
        match self {
            Subject::Japanese => "Japanese",
            Subject::Maths => "maths",
            Subject::English => "English",
            Subject::Science => "science",
            Subject::Social => "social studies",
            Subject::Pe => "PE",
            Subject::TechHome => "technology and home economics",
            Subject::Music => "music",
            Subject::Art => "art",
            Subject::Moral => "moral education",
            Subject::Integrated => "integrated studies",
            Subject::ClassActivities => "class activities",
        }
    }
    /// Its name on a Japanese timetable.
    pub fn kanji(self) -> &'static str {
        match self {
            Subject::Japanese => "国語",
            Subject::Maths => "数学",
            Subject::English => "英語",
            Subject::Science => "理科",
            Subject::Social => "社会",
            Subject::Pe => "保健体育",
            Subject::TechHome => "技術・家庭",
            Subject::Music => "音楽",
            Subject::Art => "美術",
            Subject::Moral => "道徳",
            Subject::Integrated => "総合",
            Subject::ClassActivities => "学活",
        }
    }
    /// Where it's taught (a class's own room for the rest).
    pub fn room(self) -> Room {
        match self {
            Subject::Science => Room::Science,
            Subject::Pe => Room::Gym,
            Subject::TechHome => Room::Technology,
            Subject::Music => Room::Music,
            Subject::Art => Room::Art,
            _ => Room::Homeroom,
        }
    }
}

/// A room of the school (docs/school_plan.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Room {
    /// The class's own classroom.
    Homeroom,
    Science,
    Music,
    Art,
    Technology,
    /// PE: the gym, or the sports ground in fine months (`Room::pe_place`).
    Gym,
    StaffRoom,
}

impl Room {
    pub fn name(self, class: (u8, char)) -> String {
        match self {
            Room::Homeroom => format!("{}-{}", class.0, class.1),
            Room::Science => "the science room".into(),
            Room::Music => "the music room".into(),
            Room::Art => "the art room".into(),
            Room::Technology => "the technology room".into(),
            Room::Gym => "the gym".into(),
            Room::StaffRoom => "the staff room".into(),
        }
    }
}

/// Who teaches a lesson: someone in the census, or a part-time lecturer from Minato-shi.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Teacher {
    pub person: Option<PersonId>,
    pub name: String,
    /// "Mr" or "Ms" (how the students say it, with the family name: Nakano-sensei)
    pub family: String,
}

impl Teacher {
    pub fn sensei(&self) -> String {
        format!("{}-sensei", self.family)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lesson {
    pub subject: Subject,
    pub teacher: usize,
    pub room: Room,
}

pub const CLASSES: [(u8, char); 6] = [(1, 'A'), (1, 'B'), (2, 'A'), (2, 'B'), (3, 'A'), (3, 'B')];

/// The week's timetable: every class's lessons by weekday and period.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timetable {
    pub teachers: Vec<Teacher>,
    /// [class][weekday 0..5][period 0..6]
    pub lessons: Vec<[[Option<Lesson>; 6]; 5]>,
    /// each class's homeroom teacher (index into `teachers`)
    pub homeroom: Vec<usize>,
}

impl Timetable {
    pub fn class_index(class: (u8, char)) -> Option<usize> {
        CLASSES.iter().position(|&c| c == class)
    }

    pub fn lesson(&self, class: (u8, char), weekday: u8, period: u8) -> Option<&Lesson> {
        let c = Self::class_index(class)?;
        if weekday >= 5 || period == 0 || period > 6 {
            return None;
        }
        self.lessons[c][weekday as usize][period as usize - 1].as_ref()
    }

    pub fn teacher(&self, i: usize) -> &Teacher {
        &self.teachers[i]
    }

    /// A teacher's lesson at a weekday and period, if they have one: (class, lesson).
    pub fn teaching(&self, teacher: usize, weekday: u8, period: u8) -> Option<((u8, char), &Lesson)> {
        if weekday >= 5 || period == 0 || period > 6 {
            return None;
        }
        CLASSES.iter().enumerate().find_map(|(c, &class)| self.lessons[c][weekday as usize][period as usize - 1].as_ref().filter(|l| l.teacher == teacher).map(|l| (class, l)))
    }

    /// The teacher index of a census person, if they teach at the junior high.
    pub fn teacher_of(&self, id: PersonId) -> Option<usize> {
        self.teachers.iter().position(|t| t.person == Some(id))
    }

    /// Build the timetable from the census's junior-high staff.
    pub fn build(c: &Census) -> Timetable {
        let mut teachers: Vec<Teacher> = Vec::new();
        let mut homeroom = vec![usize::MAX; CLASSES.len()];
        let staff: Vec<_> = c.people.iter().filter(|p| matches!(&p.work, Work::Teacher { school: crate::PlaceKind::JuniorHigh, .. })).collect();
        let mut subject_teachers: Vec<(Subject, usize)> = Vec::new();
        for p in &staff {
            let Work::Teacher { role, .. } = &p.work else { continue };
            let i = teachers.len();
            teachers.push(Teacher { person: Some(p.id), name: p.name(), family: p.family_name.clone() });
            let role = role.to_lowercase();
            for (key, subj) in [("japanese", Subject::Japanese), ("maths", Subject::Maths), ("english", Subject::English), ("science", Subject::Science), ("social", Subject::Social), ("pe", Subject::Pe), ("music", Subject::Music), ("art", Subject::Art), ("technology", Subject::TechHome)] {
                if role.split(|ch: char| !ch.is_alphanumeric()).any(|w| w == key) || (key == "social" && role.contains("social")) {
                    subject_teachers.push((subj, i));
                }
            }
            if role.contains("vice principal") {
                subject_teachers.push((Subject::Japanese, i));
            }
            for (ci, &(g, k)) in CLASSES.iter().enumerate() {
                if role.contains(&format!("{g}-{} homeroom", k.to_ascii_lowercase())) {
                    homeroom[ci] = i;
                }
            }
        }
        // the part-time lecturers from Minato-shi
        for (name, family, subj) in [("Mori Takashi", "Mori", Subject::Maths), ("Sasaki Yūko", "Sasaki", Subject::English)] {
            let i = teachers.len();
            teachers.push(Teacher { person: None, name: name.into(), family: family.into() });
            subject_teachers.push((subj, i));
        }
        // who takes each subject in each year: split a subject's years between its teachers,
        // the lecturers and the vice principal taking the 3rd years
        let who = |subj: Subject, grade: u8, class: usize| -> usize {
            let mut ts: Vec<usize> = subject_teachers.iter().filter(|(s, _)| *s == subj).map(|&(_, t)| t).collect();
            ts.sort_by_key(|&t| (teachers[t].person.is_none() || staff.iter().any(|p| Some(p.id) == teachers[t].person && matches!(&p.work, Work::Teacher { role, .. } if role.contains("vice principal"))), t));
            match ts.len() {
                0 => homeroom[class],
                1 => ts[0],
                // the regular teacher takes years 1 and 2; the other (lecturer, vice principal)
                // takes the 3rd years
                _ if subj != Subject::Pe => {
                    if grade == 3 {
                        ts[1]
                    } else {
                        ts[0]
                    }
                }
                // PE: the 3rd years with the PE teacher who has a 3rd-year homeroom
                _ => {
                    let has3 = ts.iter().copied().find(|&t| CLASSES.iter().enumerate().any(|(ci, &(g, _))| g == 3 && homeroom[ci] == t));
                    match has3 {
                        Some(t3) if grade == 3 => t3,
                        Some(t3) => ts.iter().copied().find(|&t| t != t3).unwrap_or(t3),
                        None => ts[(grade as usize) % ts.len()],
                    }
                }
            }
        };
        // every class's 29 lessons, with their teachers
        let mut wanted: Vec<Vec<Lesson>> = Vec::new();
        for (ci, &(g, _)) in CLASSES.iter().enumerate() {
            let mut ls = Vec::new();
            for &(subj, n) in Subject::WEEK.iter() {
                let t = match subj {
                    Subject::Moral | Subject::Integrated | Subject::ClassActivities => homeroom[ci],
                    _ => who(subj, g, ci),
                };
                for _ in 0..n {
                    ls.push(Lesson { subject: subj, teacher: t, room: subj.room() });
                }
            }
            wanted.push(ls);
        }
        // place them: always the lesson with the fewest places left next (its class and its
        // teacher free, the subject not already that day), in one of those chosen at random
        // (the afternoon preferred for PE, art, music, technology and the homeroom lessons, the
        // morning for the rest); start again on a dead end
        let slots: Vec<(usize, usize)> = (0..5).flat_map(|d| (0..periods_on(d as u8) as usize).map(move |p| (d, p))).collect();
        let mut seed: u64 = c.seed ^ 0x5c4001;
        let mut rnd = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        for _attempt in 0..500 {
            let mut lessons: Vec<[[Option<Lesson>; 6]; 5]> = vec![Default::default(); CLASSES.len()];
            let mut busy = vec![[[false; 6]; 5]; teachers.len()];
            let mut todo: Vec<(usize, Lesson)> = wanted.iter().enumerate().flat_map(|(ci, ls)| ls.iter().cloned().map(move |l| (ci, l))).collect();
            let mut ok = true;
            while !todo.is_empty() {
                let free_for = |ci: usize, l: &Lesson, lessons: &Vec<[[Option<Lesson>; 6]; 5]>, busy: &Vec<[[bool; 6]; 5]>| -> Vec<(usize, usize)> {
                    slots.iter().copied().filter(|&(d, p)| lessons[ci][d][p].is_none() && !busy[l.teacher][d][p] && !lessons[ci][d].iter().flatten().any(|x| x.subject == l.subject)).collect()
                };
                let (k, free) = todo.iter().enumerate().map(|(k, (ci, l))| (k, free_for(*ci, l, &lessons, &busy))).min_by_key(|(_, f)| f.len()).unwrap();
                if free.is_empty() {
                    ok = false;
                    break;
                }
                let (ci, l) = todo.swap_remove(k);
                let late = matches!(l.subject, Subject::Pe | Subject::Art | Subject::Music | Subject::TechHome | Subject::Integrated | Subject::ClassActivities);
                let pref: Vec<(usize, usize)> = free.iter().copied().filter(|&(_, p)| (p >= 4) == late).collect();
                let pool = if pref.is_empty() || rnd() % 4 == 0 { &free } else { &pref };
                let (d, p) = pool[rnd() % pool.len()];
                busy[l.teacher][d][p] = true;
                lessons[ci][d][p] = Some(l);
            }
            if ok {
                return Timetable { teachers, lessons, homeroom };
            }
        }
        panic!("no timetable fits the staff");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bells() {
        assert_eq!(slot_at(0, hm(8, 0)), Slot::Arrival);
        assert_eq!(slot_at(0, hm(8, 25)), Slot::MorningReading);
        assert_eq!(slot_at(0, hm(8, 35)), Slot::MorningMeeting);
        assert_eq!(slot_at(0, hm(8, 45)), Slot::Break);
        assert_eq!(slot_at(0, hm(9, 0)), Slot::Period(1));
        assert_eq!(slot_at(0, hm(12, 0)), Slot::Period(4));
        assert_eq!(slot_at(0, hm(12, 50)), Slot::Lunch);
        assert_eq!(slot_at(0, hm(13, 10)), Slot::LunchBreak);
        assert_eq!(slot_at(0, hm(13, 30)), Slot::Cleaning);
        assert_eq!(slot_at(0, hm(15, 0)), Slot::Period(6));
        assert_eq!(slot_at(0, hm(15, 45)), Slot::EndMeeting);
        assert_eq!(slot_at(0, hm(16, 30)), Slot::AfterSchool);
        // Wednesday: five periods, home at 14:50
        assert_eq!(slot_at(2, hm(15, 0)), Slot::AfterSchool);
        assert_eq!(slot_at(2, hm(14, 45)), Slot::EndMeeting);
        assert!(!clubs_on(2) && clubs_on(0));
    }
}
