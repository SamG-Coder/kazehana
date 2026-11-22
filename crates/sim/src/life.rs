//! Your days, minute by minute: hunger and energy, the things you do (each at its own pace, and
//! you can stop any time), what you carry and keep, the journal. Time never jumps except when
//! you sleep (`GameState::sleep`); everything else runs through `GameState::advance`, a minute at
//! a time. The plan is `docs/life_plan.md`.

use crate::calendar::Date;
use crate::items::{Catalog, Store};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The game's item catalog.
pub fn catalog() -> &'static Catalog {
    static C: OnceLock<Catalog> = OnceLock::new();
    C.get_or_init(Catalog::builtin)
}

/// An item's name as it reads mid-sentence ("Had school lunch", "Bought a melon pan"... but
/// "Kenji's dinner" keeps its capital).
pub fn in_sentence(name: &str) -> String {
    if name.starts_with("Kenji") {
        return name.to_string();
    }
    let mut c = name.chars();
    c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// What you do with a lesson.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Focus {
    /// Follow along.
    Listen,
    /// Take careful notes: more goes in, more tiring.
    Notes,
    /// Nod off: rests you, teaches nothing, and teachers notice.
    Doze,
    /// Draw in the margins.
    Doodle,
}

/// Something you're doing. While you do it, time runs at its pace; you can stop whenever you
/// like and keep what you got.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Activity {
    /// Sitting through lessons at your desk.
    Class { focus: Focus },
    /// Studying at your desk at home (or in the library).
    Study,
    /// Reading something you have (a book, manga, a magazine).
    Read { item: String },
    /// Eating or drinking something, `left` minutes to go of `total`.
    Eat { item: String, left: u32, total: u32 },
    /// Helping behind Tsukumodō's counter.
    Counter,
    /// Sitting and letting time pass (a bench, the station, your room).
    Wait,
    /// The bath.
    Bath,
    /// TV with Uncle Kenji.
    Tv,
    /// Writing up the day in your journal.
    Journal,
}

impl Activity {
    /// Game minutes per real second while you do it (walking about: 1).
    pub fn pace(&self) -> f32 {
        match self {
            Activity::Class { .. } => 20.0,
            Activity::Study => 12.0,
            Activity::Read { .. } => 10.0,
            Activity::Eat { .. } => 3.0,
            Activity::Counter => 15.0,
            Activity::Wait => 30.0,
            Activity::Bath => 8.0,
            Activity::Tv => 12.0,
            Activity::Journal => 6.0,
        }
    }

    /// What it looks like you're doing (for the journal and prompts).
    pub fn doing(&self) -> String {
        match self {
            Activity::Class { focus: Focus::Listen } => "listening in class".into(),
            Activity::Class { focus: Focus::Notes } => "taking notes in class".into(),
            Activity::Class { focus: Focus::Doze } => "dozing in class".into(),
            Activity::Class { focus: Focus::Doodle } => "doodling in class".into(),
            Activity::Study => "studying".into(),
            Activity::Read { item } => format!("reading {}", catalog().get(item).map(|i| i.name.as_str()).unwrap_or("something")),
            Activity::Eat { item, .. } => format!("eating {}", catalog().get(item).map(|i| i.name.as_str()).unwrap_or("something")),
            Activity::Counter => "helping in the shop".into(),
            Activity::Wait => "passing the time".into(),
            Activity::Bath => "in the bath".into(),
            Activity::Tv => "watching TV".into(),
            Activity::Journal => "writing in your journal".into(),
        }
    }

    /// Energy per hour (negative: tiring).
    pub fn energy_per_hour(&self) -> f32 {
        match self {
            Activity::Class { focus: Focus::Notes } => -5.0,
            Activity::Class { focus: Focus::Doze } => 5.0,
            Activity::Class { .. } => -3.0,
            Activity::Study => -8.0,
            Activity::Read { .. } => 1.0,
            Activity::Eat { .. } => 0.0,
            Activity::Counter => -6.0,
            Activity::Wait => 4.0,
            Activity::Bath => 14.0,
            Activity::Tv => 3.0,
            Activity::Journal => 1.0,
        }
    }
}

/// Something you're in the middle of.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Doing {
    pub activity: Activity,
    /// Minutes spent so far.
    pub minutes: u32,
}

/// Fractions of a point earned so far (stats move in whole points).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Progress {
    pub knowledge: f32,
    pub charm: f32,
    pub courage: f32,
    pub energy: f32,
    pub money: f32,
    pub uncle: f32,
}

/// Today's meals (reset each morning).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Meals {
    pub breakfast: bool,
    pub lunch: bool,
    pub dinner: bool,
}

/// A day in the diary.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiaryDay {
    pub date: Date,
    pub lines: Vec<String>,
}

/// Something someone asked of you.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ask {
    pub from: String,
    pub text: String,
    pub given: Date,
    pub due: Option<Date>,
    pub done: bool,
}

/// Something you've planned for a day (written on the calendar too).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub date: Date,
    pub text: String,
}

/// Your journal: the diary writes itself from your day; what people asked; your plans.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Journal {
    pub diary: Vec<DiaryDay>,
    pub asks: Vec<Ask>,
    pub plans: Vec<Plan>,
}

impl Journal {
    /// Add a line to a day's diary (the same line once a day).
    pub fn write(&mut self, date: Date, line: impl Into<String>) {
        let line = line.into();
        if self.diary.last().map(|d| d.date) != Some(date) {
            self.diary.push(DiaryDay { date, lines: Vec::new() });
        }
        let day = self.diary.last_mut().unwrap();
        if !day.lines.contains(&line) {
            day.lines.push(line);
        }
    }

    pub fn day(&self, date: Date) -> Option<&DiaryDay> {
        self.diary.iter().rev().find(|d| d.date == date)
    }

    pub fn plan(&mut self, date: Date, text: impl Into<String>) {
        let p = Plan { date, text: text.into() };
        if !self.plans.contains(&p) {
            self.plans.push(p);
            self.plans.sort_by_key(|p| p.date);
        }
    }

    pub fn plans_on(&self, date: Date) -> impl Iterator<Item = &Plan> {
        self.plans.iter().filter(move |p| p.date == date)
    }
}

/// How full you are, what you're doing, what you have.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Life {
    /// 0 (starving) ..= 100 (stuffed).
    pub fullness: f32,
    pub doing: Option<Doing>,
    /// Your school bag.
    pub bag: Store,
    /// The shelf in your room.
    pub room: Store,
    /// The kitchen fridge (Kenji's, but you can use it).
    pub fridge: Store,
    pub journal: Journal,
    /// Minutes at your desk in each of today's six periods, and whether you sat down late.
    pub class_minutes: [u32; 6],
    pub class_late: [bool; 6],
    pub meals: Meals,
    pub progress: Progress,
    /// Caught dozing today (a teacher remembers).
    pub caught_dozing: u32,
    /// Let out of a lesson to the toilet: the minute you went, and the period (school rules:
    /// docs/school_plan.md). Back at your desk ends it.
    #[serde(default)]
    pub toilet: Option<(u32, u8)>,
    /// The periods today you've already asked to go in (once a lesson).
    #[serde(default)]
    pub toilet_asked: [bool; 6],
    /// Today you slipped out of school before it was out.
    #[serde(default)]
    pub cut_school: bool,
}

impl Default for Life {
    fn default() -> Self {
        Life {
            fullness: 70.0,
            doing: None,
            bag: Store::new(8),
            room: Store::new(24),
            fridge: Store::new(16),
            journal: Journal::default(),
            class_minutes: [0; 6],
            class_late: [false; 6],
            meals: Meals::default(),
            progress: Progress::default(),
            caught_dozing: 0,
            toilet: None,
            toilet_asked: [false; 6],
            cut_school: false,
        }
    }
}

impl Life {
    pub fn hungry(&self) -> bool {
        self.fullness < 25.0
    }
    pub fn starving(&self) -> bool {
        self.fullness < 8.0
    }
    /// How your stomach feels, as you'd say it.
    pub fn stomach(&self) -> &'static str {
        match self.fullness as i32 {
            ..=7 => "starving",
            8..=24 => "hungry",
            25..=54 => "fine",
            55..=84 => "full",
            _ => "stuffed",
        }
    }
}
