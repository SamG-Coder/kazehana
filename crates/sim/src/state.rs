//! The whole saveable game state, and the day-to-day actions the player can take.

use crate::calendar::{events_on, released_season, Event, Weekday, START_DATE};
use crate::clock::{at, GameTime, Phase};
use crate::economy::{self, Market, SaleOutcome, SaleResponse};
use crate::life::{catalog, Activity, Doing, Focus, Life};
use crate::people::{students_from_census, Bond, Fact, Student};
use crate::town::{fallback_houses, HouseSlot, Town};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tcg::{CardDb, Collection, Pull, Rng};

pub const WEEKLY_ALLOWANCE: u32 = 1000;
pub const SHIFT_WAGE: u32 = 600;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub money: u32,
    pub knowledge: u32,
    pub charm: u32,
    pub courage: u32,
    /// 0..=100. Activities cost energy; sleep restores it.
    pub energy: i32,
    /// Town-wide reputation, spread by gossip.
    pub reputation: i32,
    pub collection: Collection,
    /// Where the collection sits in the binder (which card in which sleeve).
    #[serde(default)]
    pub binder: crate::binder::Binder,
    pub days_late: u32,
    pub days_absent: u32,
}

/// A school morning's getting ready (cleared when the date changes).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Ready {
    pub date: Option<crate::calendar::Date>,
    /// in your uniform, your bag on your shoulder
    pub dressed: bool,
    /// said 行ってきます to Uncle Kenji
    pub said_bye: bool,
    /// your bag's in your locker at school
    pub bag_stored: bool,
}

/// Where the player physically is (the three levels).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Location {
    Home,
    Path,
    School,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameState {
    pub seed: u64,
    pub time: GameTime,
    pub location: Location,
    pub player: Player,
    pub students: Vec<Student>,
    /// Every household in town (students' families, Uncle Kenji and you).
    #[serde(default)]
    pub town: Town,
    /// Same indices as `students`.
    pub bonds: Vec<Bond>,
    pub uncle: Bond,
    pub market: Market,
    pub season: u16,
    /// Whether the player sat through enough of today's lessons to count (morning / afternoon).
    pub attended: [bool; 2],
    /// Hunger, what you're doing, what you carry, the journal.
    #[serde(default)]
    pub life: Life,
    /// Lessons, marks, tests and report cards (sim::lessons).
    #[serde(default)]
    pub lessons: crate::lessons::Record,
    /// What you've set out to do (sim::quest; kept in the journal).
    #[serde(default)]
    pub quests: crate::quest::Quests,
    /// Getting ready for school today: changed, said goodbye, the bag in the locker.
    #[serde(default)]
    pub ready: Ready,
    pub rng: Rng,
    /// Messages for the HUD, newest last.
    #[serde(skip)]
    pub messages: Vec<String>,
}

impl GameState {
    /// A new game in a stand-in town (tests).
    pub fn new(seed: u64, db: &CardDb) -> Self {
        Self::with_houses(seed, db, fallback_houses())
    }

    /// A new game. The town and its people are the census's (assets/town/census.json: the one
    /// list of people, whose schedule the world draws); `houses` is no longer used to make them.
    pub fn with_houses(seed: u64, db: &CardDb, houses: Vec<HouseSlot>) -> Self {
        let _ = houses;
        let mut town = Town::from_census(crate::school::census());
        let students = students_from_census(seed, db, &mut town);
        let bonds = vec![Bond::default(); students.len()];
        let mut s = Self {
            seed,
            time: GameTime::new(START_DATE, at::WAKE),
            location: Location::Home,
            player: Player {
                name: "You".into(),
                money: 3000,
                knowledge: 0,
                charm: 0,
                courage: 0,
                energy: 100,
                reputation: 0,
                collection: Collection::default(),
                binder: Default::default(),
                days_late: 0,
                days_absent: 0,
            },
            students,
            town,
            bonds,
            uncle: Bond { points: 10, last_talk: None },
            market: Market::default(),
            season: 1,
            attended: [false; 2],
            life: Life::default(),
            lessons: Default::default(),
            quests: Default::default(),
            ready: Ready::default(),
            rng: Rng::derive(seed, "game"),
            messages: Vec::new(),
        };
        s.say("Uncle Kenji: \"First day at Kazehana Junior High! Don't be late — homeroom is at 8:30.\"");
        s
    }

    /// The story's start: the first day of school, 6:45 in the morning, in your room in
    /// Tsukumodō with no money of your own yet. Uncle Kenji has opened the shop.
    pub fn new_story(seed: u64, db: &CardDb) -> Self {
        let mut s = Self::new(seed, db);
        s.messages.clear();
        s.time = GameTime::new(START_DATE, at::hm(6, 45));
        s.player.money = 0;
        s.quests.add(crate::quest::Quests::first_day(START_DATE));
        s.quests.add(crate::quest::Quests::help_uncle(START_DATE));
        s.ready = Ready { date: Some(START_DATE), ..Default::default() };
        s.say("It's your first day at Kazehana Junior High. Get ready — homeroom is at 8:30.");
        s
    }

    /// Today's getting ready (started afresh each date).
    pub fn ready_today(&mut self) -> &mut Ready {
        if self.ready.date != Some(self.time.date) {
            self.ready = Ready { date: Some(self.time.date), ..Default::default() };
        }
        &mut self.ready
    }

    /// Something a quest may be waiting for has happened: move the quests on and say so.
    pub fn quest_event(&mut self, goal: crate::quest::Goal) {
        for line in self.quests.event(goal) {
            self.say(line);
        }
    }

    /// Get changed and pick up your bag (your closet at home).
    pub fn get_ready(&mut self) -> String {
        let r = self.ready_today();
        if r.dressed {
            return "You're already in your uniform, bag on your shoulder.".into();
        }
        r.dressed = true;
        self.quest_event(crate::quest::Goal::GotReady);
        "You change into the new uniform — the collar's stiff — and sling your school bag over your shoulder.".into()
    }

    /// Say goodbye to Uncle Kenji on the way out (行ってきます).
    pub fn say_goodbye(&mut self) -> Vec<String> {
        let dressed = self.ready_today().dressed;
        if !dressed {
            return vec!["\"You're not going like that, are you? Uniform. Bag. Go on.\"".into()];
        }
        self.ready_today().said_bye = true;
        self.uncle.add(1);
        self.quest_event(crate::quest::Goal::SaidGoodbye);
        let first = self.time.date == START_DATE;
        let mut lines = vec!["\"Ittekimasu!\" (I'm off!)".into()];
        lines.push(if first {
            "\"Itterasshai! First day — just be yourself. And come straight back, I might need a hand at the counter.\"".into()
        } else {
            "\"Itterasshai. Don't dawdle at the konbini.\"".into()
        });
        lines
    }

    /// Your bag into your locker at the back of the classroom.
    pub fn store_bag(&mut self) -> String {
        let r = self.ready_today();
        if !r.dressed {
            return "You didn't bring your bag.".into();
        }
        if r.bag_stored {
            return "Your bag's already in your locker.".into();
        }
        r.bag_stored = true;
        self.quest_event(crate::quest::Goal::StoredBag);
        "You push your bag into your locker. Your timetable is taped inside the door.".into()
    }

    pub fn say(&mut self, msg: impl Into<String>) {
        self.messages.push(msg.into());
        if self.messages.len() > 50 {
            self.messages.remove(0);
        }
    }

    pub fn phase(&self) -> Phase {
        self.time.phase()
    }

    /// Let time pass, awake: a minute at a time (hunger, energy, whatever you're doing, lessons,
    /// midnight). Time only ever moves forward through here and `sleep`.
    pub fn advance(&mut self, minutes: u32, db: &CardDb) {
        for _ in 0..minutes {
            self.minute(false, db);
        }
    }

    fn minute(&mut self, asleep: bool, db: &CardDb) {
        let before = self.time;
        let days = self.time.advance(1);
        // the bells: the end of each period, of the morning's lessons (lunch) and of the day
        if before.school_day() && days == 0 {
            let wd = before.date.weekday() as u8;
            let out = at::school_out(wd);
            if before.minute < at::LUNCH && self.time.minute >= at::LUNCH {
                self.end_lessons(0);
            }
            if before.minute < out && self.time.minute >= out {
                self.end_lessons(1);
            }
        }
        // out of a lesson "at the toilet" too long: someone's sent to look for you
        if let Some((since, period)) = self.life.toilet {
            let m = self.time.minute;
            if !self.school_locked() {
                self.life.toilet = None;
            } else if period < 100 && m >= since + 10 {
                self.life.toilet = Some((since, period + 100));
                self.say("Someone from your class comes looking for you: the teacher's waiting.");
                self.log("Took far too long at the toilet in the middle of a lesson.");
            }
        }
        // hunger (slower asleep) and energy
        let life = &mut self.life;
        life.fullness = (life.fullness - if asleep { 2.5 } else { 5.0 } / 60.0).max(0.0);
        if !asleep {
            let mut rate = life.doing.as_ref().map(|d| d.activity.energy_per_hour()).unwrap_or(-1.5);
            if life.hungry() {
                rate -= 4.0;
            }
            life.progress.energy += rate / 60.0;
            self.settle();
        }
        if let Some(d) = self.life.doing.take() {
            self.do_minute(d);
        }
        for _ in 0..days {
            self.new_day(db);
        }
    }

    /// Move whole points from the fractions earned into your stats, money and bonds.
    fn settle(&mut self) {
        let p = &mut self.life.progress;
        let whole = |x: &mut f32| -> i32 {
            // (a tiny tolerance: sixty 1/60ths add up to 0.99999)
            let w = (*x + x.signum() * 1e-4).trunc();
            *x -= w;
            w as i32
        };
        let k = whole(&mut p.knowledge);
        let c = whole(&mut p.charm);
        let g = whole(&mut p.courage);
        let e = whole(&mut p.energy);
        let m = whole(&mut p.money);
        let u = whole(&mut p.uncle);
        let pl = &mut self.player;
        pl.knowledge = (pl.knowledge as i32 + k).max(0) as u32;
        pl.charm = (pl.charm as i32 + c).max(0) as u32;
        pl.courage = (pl.courage as i32 + g).max(0) as u32;
        pl.energy = (pl.energy + e).clamp(0, 100);
        pl.money = (pl.money as i32 + m).max(0) as u32;
        if u != 0 {
            self.uncle.add(u);
        }
    }

    /// Write a line in today's diary.
    pub fn log(&mut self, line: impl Into<String>) {
        let d = self.time.date;
        self.life.journal.write(d, line);
    }

    /// What you're doing, if anything.
    pub fn doing(&self) -> Option<&Activity> {
        self.life.doing.as_ref().map(|d| &d.activity)
    }

    /// How fast time runs now (game minutes per real second).
    pub fn pace(&self) -> f32 {
        self.doing().map(|a| a.pace()).unwrap_or(1.0)
    }

    /// Once you've gone in, you're in school until it's out (from the gate opening to the end of
    /// the day's last meeting): you can't walk off the grounds.
    pub fn school_locked(&self) -> bool {
        let m = self.time.minute;
        let wd = self.time.date.weekday() as u8;
        self.location == Location::School && self.time.school_day() && (census::school::GATE_OPENS..at::school_out(wd)).contains(&m)
    }

    /// In a lesson now (a period or a class meeting), seated: you can't just get up and go.
    /// Raise your hand and ask for the toilet (`ask_toilet`).
    pub fn can_get_up(&self) -> Result<(), String> {
        let in_class = matches!(self.doing(), Some(Activity::Class { .. }));
        let wd = self.time.date.weekday() as u8;
        if in_class && self.school_locked() && crate::school::in_lessons(wd, self.time.minute).is_some() {
            return Err("You can't just walk out of a lesson. Raise your hand to ask for the toilet.".into());
        }
        Ok(())
    }

    /// Raise your hand: "May I go to the toilet?" Once a lesson, the teacher lets you go (back
    /// within ten minutes, or someone's sent to look for you). Outside lessons, just go.
    pub fn ask_toilet(&mut self) -> Result<String, String> {
        if !self.school_locked() {
            return Err("You can go whenever you like.".into());
        }
        let wd = self.time.date.weekday() as u8;
        let m = self.time.minute;
        let period = match crate::school::slot_at(wd, m) {
            crate::school::Slot::Period(n) => n,
            crate::school::Slot::MorningMeeting | crate::school::Slot::EndMeeting => 0,
            _ => return Ok("It's not a lesson: you can just go.".into()),
        };
        let teacher = if period > 0 { crate::school::my_lesson(wd, period).map(|l| crate::school::timetable().teacher(l.teacher).sensei()) } else { None }
            .unwrap_or_else(|| crate::school::timetable().teacher(crate::school::timetable().homeroom[0]).sensei());
        if period > 0 && self.life.toilet_asked[period as usize - 1] {
            return Err(format!("\"You've just been, Tsukumo. It can wait till the bell.\" ({teacher})"));
        }
        if period > 0 {
            self.life.toilet_asked[period as usize - 1] = true;
        }
        self.life.doing = None;
        self.life.toilet = Some((m, period));
        Ok(format!("\"Go on. Be quick.\" ({teacher})"))
    }

    /// Leave the school's grounds (for the road, home, anywhere). While school's in, only by
    /// way of a trip to the toilet: and that's cutting school.
    pub fn leave_school(&mut self, to: Location) -> Result<String, String> {
        if !self.school_locked() {
            self.location = to;
            return Ok(String::new());
        }
        if self.life.toilet.is_none() {
            return Err("You can't leave while school's in. (Unless you ask to go to the toilet...)".into());
        }
        self.life.toilet = None;
        self.location = to;
        if !self.life.cut_school {
            self.life.cut_school = true;
            self.player.days_absent += 1;
            self.log("Slipped out of school on a trip to the toilet.");
        }
        Ok("You slip out past the shoe lockers. Nobody stops you... this time.".into())
    }

    /// Start doing something. You stop whatever you were doing first.
    pub fn start(&mut self, activity: Activity) -> Result<String, String> {
        let m = self.time.minute;
        let home = self.location == Location::Home;
        match &activity {
            Activity::Class { .. } => {
                if self.location != Location::School || !self.time.school_day() {
                    return Err("There are no lessons now.".into());
                }
                if !(at::hm(8, 0)..at::school_out(self.time.date.weekday() as u8)).contains(&m) {
                    return Err("There are no lessons now.".into());
                }
                // (you can take your seat a quarter of an hour before the afternoon's lessons)
                if (at::LUNCH..at::AFTERNOON - 15).contains(&m) {
                    return Err("It's lunchtime.".into());
                }
            }
            Activity::Study | Activity::Tv | Activity::Bath | Activity::Journal if !home => return Err("You can do that at home.".into()),
            Activity::Counter => {
                if !home {
                    return Err("The counter is in Tsukumodō.".into());
                }
                if !(at::SHOP_OPEN..at::SHOP_CLOSE).contains(&m) {
                    return Err("The shop's closed.".into());
                }
            }
            Activity::Read { item } => {
                if self.life.bag.count(item) == 0 && !(home && self.life.room.count(item) > 0) {
                    return Err("You don't have that with you.".into());
                }
            }
            _ => {}
        }
        self.stop();
        let what = activity.doing();
        if matches!(activity, Activity::Class { .. }) {
            self.life.toilet = None; // (back at your desk)
        }
        self.life.doing = Some(Doing { activity, minutes: 0 });
        Ok(format!("You start {what}."))
    }

    /// Stop what you're doing. Returns a line about it, if you were doing anything.
    pub fn stop(&mut self) -> Option<String> {
        let d = self.life.doing.take()?;
        let (h, mm) = (d.minutes / 60, d.minutes % 60);
        let took = if h > 0 { format!("{h} h {mm} min") } else { format!("{mm} min") };
        let line = match &d.activity {
            Activity::Eat { item, left, .. } if *left > 0 => format!("You put the {} down half-eaten.", catalog().get(item).map(|i| i.name.as_str()).unwrap_or("food")),
            Activity::Counter => format!("You helped in the shop for {took}."),
            Activity::Study => format!("You studied for {took}."),
            a => format!("You stopped {} ({took}).", a.doing()),
        };
        if d.minutes >= 20 {
            match &d.activity {
                Activity::Counter => {
                    self.log(format!("Helped Uncle Kenji in the shop ({took})."));
                    // (an hour or more counts as a shift: Uncle Kenji's quest)
                    if d.minutes >= 60 {
                        self.quest_event(crate::quest::Goal::WorkedShift);
                    }
                }
                Activity::Study => self.log(format!("Studied at my desk ({took}).")),
                _ => {}
            }
        }
        Some(line)
    }

    /// A minute of what you're doing.
    fn do_minute(&mut self, mut d: Doing) {
        d.minutes += 1;
        let m = self.time.minute;
        let focus_mult = if self.life.starving() { 0.25 } else if self.life.hungry() { 0.5 } else { 1.0 };
        let mut done: Option<String> = None;
        let mut caught = false;
        match &mut d.activity {
            Activity::Class { focus } => {
                let wd = self.time.date.weekday() as u8;
                let period = match crate::school::slot_at(wd, m) {
                    crate::school::Slot::Period(n) if self.time.school_day() && self.location == Location::School => Some(n as usize - 1),
                    _ => None,
                };
                if let Some(i) = period {
                    let start = crate::school::PERIODS[i].0;
                    if m == start {
                        if let Some(line) = crate::school::describe(wd, i as u8 + 1) {
                            self.say(format!("The bell. {line}."));
                        }
                    }
                    if self.life.class_minutes[i] == 0 && m > start + 5 {
                        self.life.class_late[i] = true;
                    }
                    self.life.class_minutes[i] += 1;
                    self.lessons_sat(i, *focus);
                    let p = &mut self.life.progress;
                    match focus {
                        Focus::Listen => p.knowledge += 0.6 / 60.0 * focus_mult,
                        Focus::Notes => p.knowledge += 0.95 / 60.0 * focus_mult,
                        Focus::Doodle => {
                            p.knowledge += 0.15 / 60.0 * focus_mult;
                            p.charm += 0.25 / 60.0;
                        }
                        Focus::Doze => {
                            if self.rng.chance(1.0 / 90.0) {
                                *focus = Focus::Listen;
                                caught = true;
                            }
                        }
                    }
                }
            }
            Activity::Study => self.life.progress.knowledge += 1.2 / 60.0 * focus_mult,
            Activity::Read { item } => {
                let it = catalog().get(item);
                let total = it.map(|i| i.minutes).unwrap_or(60).max(1);
                let p = &mut self.life.progress;
                match item.as_str() {
                    "library_book" => p.knowledge += 0.8 / 60.0,
                    "card_magazine" => p.knowledge += 0.4 / 60.0,
                    _ => p.charm += 1.0 / 60.0,
                }
                if d.minutes >= total {
                    done = Some(format!("Read {}.", it.map(|i| i.name.as_str()).unwrap_or("a book")));
                }
            }
            Activity::Eat { item, left, total } => {
                if let Some(it) = catalog().get(item) {
                    self.life.fullness = (self.life.fullness + it.full / *total as f32).min(100.0);
                    self.life.progress.energy += it.energy / *total as f32;
                }
                *left = left.saturating_sub(1);
                if *left == 0 {
                    let name = catalog().get(item).map(|i| crate::life::in_sentence(&i.name)).unwrap_or_default();
                    done = Some(format!("Had {name}."));
                }
            }
            Activity::Counter => {
                if !(at::SHOP_OPEN..at::SHOP_CLOSE).contains(&m) {
                    done = Some("Closing time.".into());
                } else {
                    let p = &mut self.life.progress;
                    p.money += SHIFT_WAGE as f32 / 4.0 / 60.0;
                    p.charm += 0.5 / 60.0;
                    p.uncle += 0.75 / 60.0;
                }
            }
            Activity::Bath => {
                if d.minutes >= 30 {
                    done = Some("A long hot bath.".into());
                }
            }
            Activity::Journal => {
                self.life.progress.knowledge += 0.3 / 60.0;
                if d.minutes >= 15 {
                    done = Some("Wrote up the day in my journal.".into());
                }
            }
            Activity::Tv | Activity::Wait => {}
        }
        if caught {
            self.life.caught_dozing += 1;
            self.say("\"Tsukumo! Are we keeping you up?\" The class laughs.");
            self.log("Got caught dozing in class.");
        }
        // lessons end with the bell
        let bell = matches!(d.activity, Activity::Class { .. }) && (m == at::LUNCH || m == at::school_out(self.time.date.weekday() as u8));
        if let Some(line) = done {
            if matches!(d.activity, Activity::Counter) && d.minutes >= 60 {
                self.quest_event(crate::quest::Goal::WorkedShift);
            }
            self.say(line.clone());
            self.log(line);
        } else if bell {
            self.say(if m == at::LUNCH { "The bell rings for lunch." } else { "The last bell. School's out." });
        } else {
            self.life.doing = Some(d);
        }
        self.settle();
    }

    /// The bell at the end of the morning's lessons (i = 0: periods 1 to 4) or the day's
    /// (i = 1: 5 and 6): which periods did you sit through (three quarters of each, at least)?
    fn end_lessons(&mut self, i: usize) {
        let wd = self.time.date.weekday() as u8;
        let periods: Vec<usize> = if i == 0 { (0..4).collect() } else { (4..crate::school::periods_on(wd) as usize).collect() };
        let len = |p: usize| crate::school::PERIODS[p].1 - crate::school::PERIODS[p].0;
        let sat: Vec<usize> = periods.iter().copied().filter(|&p| self.life.class_minutes[p] * 4 >= len(p) * 3).collect();
        let first = periods.first().copied().unwrap_or(0);
        let which = if i == 0 { "the morning's" } else { "the afternoon's" };
        let lessons = crate::school::day_line(wd, first as u8 + 1, *periods.last().unwrap_or(&first) as u8 + 1);
        if sat.len() == periods.len() {
            self.attended[i] = true;
            if periods.iter().any(|&p| self.life.class_late[p]) {
                self.player.days_late += 1;
                self.log(format!("Late for {which} lessons ({lessons})."));
            } else {
                self.log(format!("Sat through {which} lessons ({lessons})."));
            }
        } else if sat.is_empty() && periods.iter().all(|&p| self.life.class_minutes[p] < 15) {
            self.player.days_absent += 1;
            self.say(format!("You skipped {which} lessons. Your homeroom teacher will hear about this."));
            self.log(format!("Skipped {which} lessons."));
        } else {
            let missed: Vec<&str> = periods.iter().filter(|p| !sat.contains(p)).filter_map(|&p| crate::school::my_lesson(wd, p as u8 + 1)).map(|l| l.subject.name()).collect();
            self.log(format!("Missed {} in {which} lessons.", missed.join(" and ")));
        }
        // each period's lesson or test, for your marks (sim::lessons)
        self.lessons_bell(i == 1, &periods);
    }

    fn new_day(&mut self, db: &CardDb) {
        self.attended = [false; 2];
        self.life.class_minutes = [0; 6];
        self.life.class_late = [false; 6];
        self.life.toilet = None;
        self.life.toilet_asked = [false; 6];
        self.life.cut_school = false;
        self.life.meals = Default::default();
        let today = self.time.date;
        let mut gone = Vec::new();
        for store in [&mut self.life.bag, &mut self.life.room, &mut self.life.fridge] {
            gone.extend(store.spoil(catalog(), today));
        }
        for g in gone {
            let name = catalog().get(&g.item).map(|i| i.name.clone()).unwrap_or(g.item.clone());
            self.log(format!("Threw out {name} (it had gone off)."));
        }
        let d = self.time.date;
        if d.weekday() == Weekday::Sun {
            self.player.money += WEEKLY_ALLOWANCE;
            for s in &mut self.students {
                s.wallet += s.pocket_money_weekly;
            }
            self.say(format!("Uncle Kenji gave you your ¥{WEEKLY_ALLOWANCE} allowance."));
        }
        let season = released_season(d);
        if season != self.season {
            self.season = season;
            for s in &mut self.students {
                s.refresh_wants(db, season, &mut self.rng);
            }
        }
        for e in events_on(d) {
            if let Event::SetRelease(n) = e {
                self.say(format!("Release day! {} is on the shelves.", tcg::season::theme(n).name));
            } else {
                self.say(format!("Today: {e}"));
            }
        }
    }

    pub fn uncle_rank(&self) -> u8 {
        self.uncle.rank()
    }

    pub fn pack_cost(&self) -> u32 {
        economy::pack_cost(self.uncle_rank())
    }

    /// Buy and open one pack of the current season from the uncle.
    pub fn buy_pack(&mut self, db: &CardDb) -> Result<Vec<Pull>, String> {
        if self.location != Location::Home {
            return Err("Packs are sold at home, in Uncle Kenji's shop.".into());
        }
        let cost = self.pack_cost();
        if self.player.money < cost {
            return Err(format!("You need ¥{cost} for a pack."));
        }
        self.player.money -= cost;
        let pulls = tcg::open_pack(db, self.season, &mut self.rng);
        self.market.record_pack(self.season, &pulls);
        for p in &pulls {
            self.player.collection.add(*p);
        }
        Ok(pulls)
    }

    /// Talk to a student (once a day counts). Returns what you learned, if anything new.
    pub fn talk(&mut self, idx: usize) -> String {
        let today = self.time.date;
        if self.player.energy < 5 {
            return "You're too tired to make conversation.".into();
        }
        let s = &self.students[idx];
        let bond = &mut self.bonds[idx];
        if bond.last_talk == Some(today) {
            return format!("You've already chatted with {} today.", if bond.knows(Fact::Name) { s.given_name.as_str() } else { "them" });
        }
        let before = bond.rank();
        // Shy students warm up slowly; charm helps everyone.
        let shy = s.personality == crate::people::Personality::Shy;
        let gain = 4 + (self.player.charm / 10).min(6) as i32 - if shy && before < 3 { 2 } else { 0 };
        bond.add(gain);
        bond.last_talk = Some(today);
        let who = if bond.knows(Fact::Name) { s.full_name() } else { format!("a {}-year from class {}", s.grade, s.class_name()) };
        self.life.journal.write(today, format!("Talked with {who}."));
        self.player.energy -= 5;
        self.player.charm += 1;
        let after = bond.rank();
        if after > before {
            let learned = match after {
                1 => format!("You introduced yourselves: {} ({}).", s.full_name(), s.class_name()),
                2 => format!("{} likes {}.", s.given_name, s.hobby),
                3 => format!("{}'s favourite cards are {:?}.", s.given_name, s.favorites),
                4 => format!("{} told you which cards they're hunting.", s.given_name),
                5 => format!("{} trusts you with money talk now.", s.given_name),
                6 => format!("{} starts sharing rumours with you.", s.given_name),
                _ => format!("You and {} are getting really close.", s.given_name),
            };
            self.say(learned.clone());
            learned
        } else {
            "You chatted for a while.".into()
        }
    }

    /// Sit down at your desk for lessons (they play out; get up whenever you like).
    pub fn enter_classroom(&mut self, _db: &CardDb) -> String {
        match self.start(Activity::Class { focus: Focus::Listen }) {
            Ok(_) => "You take your seat.".into(),
            Err(e) => e,
        }
    }

    /// Help behind the counter (any time the shop's open; stop when you like, paid by the hour).
    pub fn work_shift(&mut self, _db: &CardDb) -> String {
        if self.player.energy < 10 {
            return "You're too tired to work.".into();
        }
        match self.start(Activity::Counter) {
            Ok(_) => "You tie on an apron and take the counter.".into(),
            Err(e) => e,
        }
    }

    /// Breakfast (6:30-8:15) or dinner (18:30-20:45) with Uncle Kenji, at home.
    pub fn eat_with_kenji(&mut self) -> Result<String, String> {
        if self.location != Location::Home {
            return Err("Kenji cooks at home.".into());
        }
        let m = self.time.minute;
        let item = if (at::WAKE..at::hm(8, 15)).contains(&m) && !self.life.meals.breakfast {
            self.life.meals.breakfast = true;
            "kenji_breakfast"
        } else if (at::hm(18, 30)..at::hm(20, 45)).contains(&m) && !self.life.meals.dinner {
            self.life.meals.dinner = true;
            "kenji_dinner"
        } else {
            return Err("It isn't mealtime. (Breakfast's at seven, dinner at seven.)".into());
        };
        self.uncle.add(1);
        self.start_eating(item)
    }

    /// School lunch with your class (lunchtime on a school day).
    pub fn school_lunch(&mut self) -> Result<String, String> {
        if self.location != Location::School || !self.time.school_day() || !(at::LUNCH..at::AFTERNOON).contains(&self.time.minute) {
            return Err(format!("School lunch is at {}:{:02} in the classroom.", at::LUNCH / 60, at::LUNCH % 60));
        }
        if self.life.meals.lunch {
            return Err("You've had lunch.".into());
        }
        self.life.meals.lunch = true;
        self.start_eating("school_lunch")
    }

    fn start_eating(&mut self, item: &str) -> Result<String, String> {
        let it = catalog().get(item).ok_or("no such food")?;
        let total = it.minutes.max(1);
        self.start(Activity::Eat { item: item.to_string(), left: total, total })
    }

    /// Eat or drink something you have (your bag; at home also your shelf and the fridge).
    pub fn eat(&mut self, item: &str) -> Result<String, String> {
        let it = catalog().get(item).ok_or("no such thing")?;
        if !it.cat.edible() {
            return Err(format!("You can't eat {}.", it.name));
        }
        let home = self.location == Location::Home;
        let taken = self.life.bag.take(item).or_else(|| if home { self.life.fridge.take(item).or_else(|| self.life.room.take(item)) } else { None });
        if taken.is_none() {
            return Err(format!("You don't have any {}.", it.name));
        }
        self.start_eating(item)
    }

    /// Buy something at a place ("konbini", "vending", a shop's trade...). It goes in your bag.
    pub fn buy(&mut self, item: &str, place: &str, n: u16) -> Result<String, String> {
        let it = catalog().get(item).ok_or("no such thing")?;
        if !it.sold_at(place) || it.price == 0 {
            return Err(format!("They don't sell {} here.", it.name));
        }
        if !it.in_season(self.time.date.month) {
            return Err(format!("{} isn't in season.", it.name));
        }
        let cost = it.price * n as u32;
        if self.player.money < cost {
            return Err(format!("You need ¥{cost}."));
        }
        let today = self.time.date;
        if !self.life.bag.add(item, n, today) {
            return Err("Your bag's full.".into());
        }
        self.player.money -= cost;
        let name = crate::life::in_sentence(&it.name);
        self.log(format!("Bought {name}{} at the {place} (¥{cost}).", if n > 1 { format!(" ×{n}") } else { String::new() }));
        Ok(format!("You bought {name} for ¥{cost}."))
    }

    /// Sleep: the one time the clock jumps. At night, until 6:30 (energy for tomorrow comes from
    /// how long you slept, and going to bed hungry costs you); in the day, a nap of an hour and a
    /// half.
    pub fn sleep(&mut self, db: &CardDb) -> String {
        if self.location != Location::Home {
            return "You can only sleep at home.".into();
        }
        self.stop();
        let m = self.time.minute;
        let nap = (at::hm(8, 0)..at::hm(18, 0)).contains(&m);
        let hungry = self.life.hungry();
        let mins = if nap { 90 } else { self.time.until(at::WAKE) };
        let went = self.time.date;
        for _ in 0..mins {
            self.minute(true, db);
        }
        if nap {
            self.player.energy = (self.player.energy + 25).min(100);
            self.log("Had a nap.");
            return "You wake from a nap, feeling better.".into();
        }
        let hours = mins as f32 / 60.0;
        let mut e = (hours / 8.0 * 100.0).min(100.0) as i32;
        if hungry {
            e -= 15;
        }
        self.player.energy = e.clamp(20, 100);
        let late = m >= at::CURFEW || m < at::hm(5, 0);
        self.life.journal.write(went, if late { "Went to bed late." } else { "Went to bed." });
        match (late, hungry) {
            (_, true) => "Your stomach kept you up half the night.",
            (true, _) => "You slept, but went to bed too late. Still tired.",
            _ => "You slept well.",
        }
        .into()
    }

    /// Offer one of your cards to a student.
    pub fn sell(&mut self, db: &CardDb, idx: usize, pull: Pull, asking: u32) -> SaleOutcome {
        let card = db.get(pull.id);
        let value = self.market.value(card, pull.foil, self.season);
        let rank = self.bonds[idx].rank();
        let out = economy::offer(&self.students[idx], card, pull.foil, value, asking, rank, &mut self.rng);
        if let SaleResponse::Accept(price) = out.response {
            if self.player.collection.remove(pull) {
                self.player.binder.reconcile(&self.player.collection);
                let s = &mut self.students[idx];
                s.wallet -= price;
                s.collection.add(pull);
                s.wants.retain(|&w| w != pull.id || s.collection.count(w) < 2);
                self.player.money += price;
            }
        }
        self.bonds[idx].add(out.bond_delta);
        self.player.reputation += out.reputation_delta;
        out
    }

    /// Students who are at a spot right now (for placing them in the level).
    pub fn students_at(&self, spot: crate::people::Spot) -> Vec<usize> {
        (0..self.students.len()).filter(|&i| self.students[i].spot_at(&self.time) == spot).collect()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        serde_json::from_slice(&std::fs::read(path)?).map_err(std::io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_school_day() {
        let db = CardDb::generate(12);
        let mut g = GameState::new(1, &db);
        // Walk to school, arrive 8:20, sit through the morning (lessons play out, no skip).
        g.advance(at::hm(8, 20) - g.time.minute, &db);
        g.location = Location::School;
        g.enter_classroom(&db);
        assert_eq!(g.time.minute, at::hm(8, 20), "sitting down doesn't move the clock");
        g.advance(at::LUNCH - g.time.minute, &db);
        assert!(g.attended[0] && g.player.days_late == 0);
        assert!(g.doing().is_none(), "the bell ends the lesson");
        assert!(g.player.knowledge >= 2);
        // lunch, then the afternoon
        g.school_lunch().unwrap();
        g.advance(at::AFTERNOON - g.time.minute, &db);
        g.enter_classroom(&db);
        g.advance(at::SCHOOL_OUT - g.time.minute, &db);
        assert!(g.attended[1]);
        // Home, buy packs, sleep.
        g.location = Location::Home;
        let pulls = g.buy_pack(&db).unwrap();
        assert_eq!(pulls.len(), 6);
        assert_eq!(g.player.money, 3000 - g.pack_cost());
        g.advance(at::hm(21, 30) - g.time.minute, &db);
        g.sleep(&db);
        assert_eq!(g.time.date, START_DATE.add_days(1));
    }

    #[test]
    fn a_month_passes_and_a_season_releases() {
        let db = CardDb::generate(12);
        let mut g = GameState::new(1, &db);
        for _ in 0..30 {
            g.advance(24 * 60, &db);
        }
        assert_eq!(g.season, 2, "Tidecaller's Deep is out by 6 May");
        assert!(g.player.money >= 3000 + 4 * WEEKLY_ALLOWANCE);
    }

    #[test]
    fn save_roundtrip() {
        let db = CardDb::generate(1);
        let g = GameState::new(9, &db);
        let p = std::env::temp_dir().join("kazehana_test_save.json");
        g.save(&p).unwrap();
        let h = GameState::load(&p).unwrap();
        assert_eq!(h.students[3].full_name(), g.students[3].full_name());
        assert_eq!(h.time, g.time);
    }
}
