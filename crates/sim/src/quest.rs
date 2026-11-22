//! Quests: what you set out to do, step by step. Each step names a goal (what finishes it) and,
//! when it has one, a place (the game turns a place into a spot in the town: the waypoint on the
//! map and the compass). Quests are kept in the journal, where each can be set active or put
//! aside; the active quests' steps are tracked (the first active one's place is the waypoint).
//!
//! The story's first: the first day of school. Get ready in your room, say goodbye to Uncle Kenji
//! at the counter (he's already opened up), walk to school, put your bag in your locker in 1-A
//! (your timetable is taped inside its door) and take your seat before the morning meeting.

use crate::calendar::Date;
use serde::{Deserialize, Serialize};

/// A place a step leads to (the game knows where each one is in the town).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    /// your room in Tsukumodō: the closet with your uniform
    YourCloset,
    /// Tsukumodō's counter, where Uncle Kenji is when the shop's open
    ShopCounter,
    /// the junior high's front gate
    SchoolGate,
    /// your locker at the back of 1-A
    YourLocker,
    /// your desk in 1-A
    YourSeat,
}

impl Place {
    pub fn name(self) -> &'static str {
        match self {
            Place::YourCloset => "Your room",
            Place::ShopCounter => "Tsukumodō counter",
            Place::SchoolGate => "Kazehana Junior High",
            Place::YourLocker => "Your locker (1-A)",
            Place::YourSeat => "Your seat (1-A)",
        }
    }
}

/// What finishes a step (the game reports these as they happen: `Quests::event`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Goal {
    GotReady,
    SaidGoodbye,
    Reached(Place),
    StoredBag,
    Seated,
    WorkedShift,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub text: String,
    pub goal: Goal,
    pub place: Option<Place>,
    /// by when (minute of the day), if it matters
    pub by: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestId {
    FirstDay,
    HelpUncle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Quest {
    pub id: QuestId,
    pub title: String,
    /// who asked, or where it came from
    pub from: String,
    pub about: String,
    pub steps: Vec<Step>,
    /// the step you're on (== steps.len(): done)
    pub step: usize,
    /// tracked: its steps show on the compass and the map
    pub active: bool,
    pub started: Date,
}

impl Quest {
    pub fn done(&self) -> bool {
        self.step >= self.steps.len()
    }
    pub fn current(&self) -> Option<&Step> {
        self.steps.get(self.step)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Quests {
    pub list: Vec<Quest>,
    /// the quest picked on the journal's page (not saved)
    #[serde(skip)]
    pub sel: usize,
}

fn step(text: &str, goal: Goal, place: Option<Place>, by: Option<u32>) -> Step {
    Step { text: text.into(), goal, place, by }
}

impl Quests {
    /// The first day of school (the story's start).
    pub fn first_day(today: Date) -> Quest {
        use crate::clock::at::hm;
        Quest {
            id: QuestId::FirstDay,
            title: "First day at Kazehana Junior High".into(),
            from: "The story".into(),
            about: "Your first day at a new school, in a town you only moved to last week. Uncle Kenji has the shop open already. Homeroom starts at 8:30 — don't be late.".into(),
            steps: vec![
                step("Get changed into your uniform and grab your bag", Goal::GotReady, Some(Place::YourCloset), None),
                step("Say goodbye to Uncle Kenji at the counter", Goal::SaidGoodbye, Some(Place::ShopCounter), None),
                step("Walk to school", Goal::Reached(Place::SchoolGate), Some(Place::SchoolGate), Some(hm(8, 25))),
                step("Put your bag in your locker at the back of 1-A", Goal::StoredBag, Some(Place::YourLocker), Some(hm(8, 30))),
                step("Take your seat before homeroom", Goal::Seated, Some(Place::YourSeat), Some(hm(8, 30))),
            ],
            step: 0,
            active: true,
            started: today,
        }
    }

    /// Uncle Kenji's: give him a hand at the counter after school (he pays: your first money).
    pub fn help_uncle(today: Date) -> Quest {
        Quest {
            id: QuestId::HelpUncle,
            title: "A hand at the counter".into(),
            from: "Uncle Kenji".into(),
            about: "\"Afternoons get busy once school's out. Work a shift behind the counter and I'll pay you — ¥600 for four hours.\"".into(),
            steps: vec![step("Work a shift at Tsukumodō's counter after school", Goal::WorkedShift, Some(Place::ShopCounter), None)],
            step: 0,
            active: false,
            started: today,
        }
    }

    pub fn has(&self, id: QuestId) -> bool {
        self.list.iter().any(|q| q.id == id)
    }

    pub fn add(&mut self, q: Quest) {
        if !self.has(q.id) {
            self.list.push(q);
        }
    }

    pub fn get(&self, id: QuestId) -> Option<&Quest> {
        self.list.iter().find(|q| q.id == id)
    }

    /// Something happened: every quest on a step it finishes moves on (active or not). Returns
    /// the lines to tell the player (a step done, a quest done).
    pub fn event(&mut self, goal: Goal) -> Vec<String> {
        let mut out = Vec::new();
        for q in self.list.iter_mut().filter(|q| !q.done()) {
            if q.current().is_some_and(|s| s.goal == goal) {
                q.step += 1;
                match q.current() {
                    Some(next) => out.push(format!("{}: {}", q.title, next.text)),
                    None => out.push(format!("Quest complete: {}", q.title)),
                }
            }
        }
        out
    }

    /// Is this step of an unfinished quest the one being waited on (whether or not tracked)?
    pub fn waiting_for(&self, goal: Goal) -> bool {
        self.list.iter().any(|q| !q.done() && q.current().is_some_and(|s| s.goal == goal))
    }

    /// Set a quest tracked or not (the journal).
    pub fn set_active(&mut self, i: usize, on: bool) {
        if let Some(q) = self.list.get_mut(i) {
            q.active = on;
        }
    }

    /// The tracked steps: (quest, step) of every active, unfinished quest.
    pub fn tracked(&self) -> impl Iterator<Item = (&Quest, &Step)> {
        self.list.iter().filter(|q| q.active && !q.done()).filter_map(|q| q.current().map(|s| (q, s)))
    }

    /// Where the compass and the map point: the first tracked step with a place.
    pub fn waypoint(&self) -> Option<(Place, &Quest, &Step)> {
        self.tracked().find_map(|(q, s)| s.place.map(|p| (p, q, s)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_day_in_order() {
        let mut q = Quests::default();
        q.add(Quests::first_day(Date::new(2026, 4, 6)));
        q.add(Quests::help_uncle(Date::new(2026, 4, 6)));
        assert_eq!(q.waypoint().unwrap().0, Place::YourCloset);
        // out of order: nothing moves
        assert!(q.event(Goal::SaidGoodbye).is_empty());
        assert!(!q.event(Goal::GotReady).is_empty());
        assert_eq!(q.waypoint().unwrap().0, Place::ShopCounter);
        q.event(Goal::SaidGoodbye);
        q.event(Goal::Reached(Place::SchoolGate));
        q.event(Goal::StoredBag);
        assert_eq!(q.waypoint().unwrap().0, Place::YourSeat);
        let said = q.event(Goal::Seated);
        assert!(said.iter().any(|l| l.starts_with("Quest complete")));
        // the uncle's isn't tracked till you set it active in the journal
        assert!(q.waypoint().is_none());
        q.set_active(1, true);
        assert_eq!(q.waypoint().unwrap().0, Place::ShopCounter);
        q.set_active(1, false);
        assert!(q.waypoint().is_none());
    }
}
