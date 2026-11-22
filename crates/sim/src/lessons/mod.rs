//! Lessons as mini-games, marks by subject, the term tests and report cards, and the end of the
//! year (docs/lessons_plan.md).
//!
//! - **At the bell**, seated in a period, the lesson is there to play (`GameState::begin_lesson`):
//!   a few minutes of the week's syllabus (`syllabus`) as a mini-game (`play`, `content`).
//!   `finish_lesson` takes what you got.
//! - **Each week's topic** of each subject has a mastery (0..1): what you've learned of it. Played
//!   lessons teach by how well you play; lessons you sit through without playing teach by your
//!   focus (listening, notes, doodling, dozing).
//! - **The tests** (teiki tesuto): mid-terms in the 1st and 2nd terms (five subjects), finals at
//!   the end of every term (nine). On test days, periods 1 to 3 are the tests, played like a
//!   lesson; the mark is what you score and what you'd learned of the weeks it covers.
//! - **Report cards** (tsūchihyō) on each term's last day: 1 to 5 for the nine subjects with marks,
//!   a comment for moral education, integrated studies and class activities.
//! - **The year ends** at the closing ceremony on 24 March 2027 (`year_over`, `close_year`).

pub mod content;
pub mod paper;
pub mod play;
pub mod syllabus;

pub use play::{Condition, Input, Play, Task, Virtue};
pub use syllabus::{Week, SUBJECTS, YEAR_WEEKS};

use crate::calendar::{is_school_day, nth_weekday, Date, Weekday, START_DATE};
use crate::clock::GameTime;
use crate::life::{Activity, Focus};
use crate::school::{self, Slot, Subject, PERIODS};
use crate::state::{GameState, Location};
use serde::{Deserialize, Serialize};
use syllabus::{subject_index, term_week, week_on};
use tcg::Rng;

/// The term tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exam {
    Mid1,
    Final1,
    Mid2,
    Final2,
    YearEnd,
}

/// The five subjects of a mid-term, and the nine of a final.
pub const MID_SUBJECTS: [&[Subject]; 3] = [&[Subject::Japanese, Subject::Maths], &[Subject::English, Subject::Science], &[Subject::Social]];
pub const FINAL_SUBJECTS: [&[Subject]; 3] = [&[Subject::Japanese, Subject::Maths, Subject::Music], &[Subject::English, Subject::Science, Subject::Art], &[Subject::Social, Subject::Pe, Subject::TechHome]];

impl Exam {
    pub const ALL: [Exam; 5] = [Exam::Mid1, Exam::Final1, Exam::Mid2, Exam::Final2, Exam::YearEnd];

    pub fn name(self) -> &'static str {
        match self {
            Exam::Mid1 => "1st-term mid-term tests",
            Exam::Final1 => "1st-term finals",
            Exam::Mid2 => "2nd-term mid-term tests",
            Exam::Final2 => "2nd-term finals",
            Exam::YearEnd => "year-end tests",
        }
    }

    pub fn term(self) -> u8 {
        match self {
            Exam::Mid1 | Exam::Final1 => 1,
            Exam::Mid2 | Exam::Final2 => 2,
            Exam::YearEnd => 3,
        }
    }

    pub fn is_mid(self) -> bool {
        matches!(self, Exam::Mid1 | Exam::Mid2)
    }

    /// The first of its three days (the calendar's: `calendar::events_on`).
    pub fn start(self, school_year: i32) -> Date {
        let y = school_year;
        match self {
            Exam::Mid1 => nth_weekday(y, 5, Weekday::Tue, 3),
            Exam::Final1 => nth_weekday(y, 7, Weekday::Tue, 1),
            Exam::Mid2 => nth_weekday(y, 10, Weekday::Tue, 3),
            Exam::Final2 => nth_weekday(y, 12, Weekday::Tue, 1),
            // (Wednesday to Friday: the 4th Tuesday of February 2027 is the Emperor's Birthday)
            Exam::YearEnd => nth_weekday(y + 1, 2, Weekday::Wed, 4),
        }
    }

    pub fn days(self) -> &'static [&'static [Subject]; 3] {
        if self.is_mid() {
            &MID_SUBJECTS
        } else {
            &FINAL_SUBJECTS
        }
    }

    pub fn subjects(self) -> Vec<Subject> {
        self.days().iter().flat_map(|d| d.iter().copied()).collect()
    }

    /// The week of its term it falls in.
    pub fn week(self, school_year: i32) -> u8 {
        term_week(self.start(school_year)).map(|x| x.1).unwrap_or(1)
    }

    /// The weeks of a subject a test covers: since the last test of the term (the mid-term's
    /// subjects), or the whole term so far.
    pub fn covers(self, subject: Subject, school_year: i32) -> Vec<Week> {
        let t = self.term();
        let end = self.week(school_year);
        let from = match self {
            Exam::Final1 if Exam::Mid1.subjects().contains(&subject) => Exam::Mid1.week(school_year) + 1,
            Exam::Final2 if Exam::Mid2.subjects().contains(&subject) => Exam::Mid2.week(school_year) + 1,
            _ => 1,
        };
        (from..end).filter_map(|w| syllabus::week_of(subject, t, w)).collect()
    }
}

/// The tests on a date: (which, day 0..3).
pub fn exam_on(d: Date) -> Option<(Exam, usize)> {
    let y = syllabus::school_year(d);
    Exam::ALL.iter().find_map(|&e| {
        let k = d.to_days() - e.start(y).to_days();
        (0..3).contains(&k).then_some((e, k as usize))
    })
}

/// The test in a period, if that period is one.
pub fn exam_in(d: Date, period: u8) -> Option<(Exam, Subject)> {
    let (e, day) = exam_on(d)?;
    e.days()[day].get(period as usize - 1).map(|&s| (e, s))
}

/// How many lessons a week a subject has (1-A's timetable).
pub fn per_week(s: Subject) -> u8 {
    Subject::WEEK.iter().find(|x| x.0 == s).map(|x| x.1).unwrap_or(1)
}

/// Whether a subject gets a mark (1–5) on the report card.
pub fn graded(s: Subject) -> bool {
    !matches!(s, Subject::Moral | Subject::Integrated | Subject::ClassActivities)
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct TermStats {
    /// lessons sat through (three quarters of the period, at least)
    pub lessons: u32,
    /// lessons missed
    pub missed: u32,
    /// lessons played as a mini-game
    pub played: u32,
    /// the sum of each lesson's score (0..1)
    pub perf: f32,
}

impl TermStats {
    /// The average lesson (missed lessons count as nothing), 0..1.
    pub fn average(&self) -> Option<f32> {
        let n = self.lessons + self.missed;
        (n > 0).then(|| self.perf / n as f32)
    }
    pub fn attendance(&self) -> Option<f32> {
        let n = self.lessons + self.missed;
        (n > 0).then(|| self.lessons as f32 / n as f32)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SubjectRecord {
    /// what you've learned of each week's topic (index: the week of the year, 1..=44)
    pub mastery: Vec<f32>,
    pub terms: [TermStats; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestMark {
    pub exam: Exam,
    pub subject: Subject,
    /// 0..=100
    pub mark: u32,
    /// false: absent (marked 0)
    pub sat: bool,
    pub date: Date,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReportLine {
    pub subject: Subject,
    /// 1..=5 (none for moral education, integrated studies, class activities)
    pub grade: Option<u8>,
    pub comment: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReportCard {
    pub term: u8,
    pub date: Date,
    pub lines: Vec<ReportLine>,
    /// mornings or afternoons skipped, and late, so far this year
    pub absent: u32,
    pub late: u32,
}

impl ReportCard {
    pub fn grade(&self, s: Subject) -> Option<u8> {
        self.lines.iter().find(|l| l.subject == s).and_then(|l| l.grade)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct YearSummary {
    /// the year's mark for each subject (the three terms' together)
    pub grades: Vec<(Subject, Option<u8>)>,
    pub lines: Vec<String>,
    pub at_ceremony: bool,
}

/// Your school record.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Record {
    /// by `syllabus::SUBJECTS`
    pub subjects: Vec<SubjectRecord>,
    pub tests: Vec<TestMark>,
    pub reports: Vec<ReportCard>,
    /// today (the day the next two belong to)
    pub day: Option<Date>,
    /// which of today's periods were played
    pub played: [bool; 6],
    /// minutes seated in each focus (listen, notes, doodle, doze) in each of today's periods
    pub focus: [[u16; 4]; 6],
    /// what your answers in discussions leaned to, by `Virtue` (kindness, honesty, courage,
    /// fairness, responsibility, respect)
    pub virtues: [u32; 6],
    pub summary: Option<YearSummary>,
    /// your notebooks: each lesson's board copied down, when you took notes
    #[serde(default)]
    pub notes: Vec<paper::Notes>,
    /// the marked prints handed back (the latest of each subject's lessons)
    #[serde(default)]
    pub sheets: Vec<paper::Sheet>,
}

impl Record {
    fn ensure(&mut self) {
        if self.subjects.len() != SUBJECTS.len() {
            self.subjects = vec![SubjectRecord { mastery: vec![0.0; YEAR_WEEKS + 1], terms: Default::default() }; SUBJECTS.len()];
        }
    }

    fn today(&mut self, d: Date) {
        if self.day != Some(d) {
            self.day = Some(d);
            self.played = [false; 6];
            self.focus = [[0; 4]; 6];
        }
    }

    pub fn subject(&self, s: Subject) -> Option<&SubjectRecord> {
        self.subjects.get(subject_index(s))
    }

    fn subject_mut(&mut self, s: Subject) -> &mut SubjectRecord {
        self.ensure();
        &mut self.subjects[subject_index(s)]
    }

    /// What you've learned of a week of a subject.
    pub fn mastery(&self, s: Subject, year_week: usize) -> f32 {
        self.subject(s).and_then(|r| r.mastery.get(year_week).copied()).unwrap_or(0.0)
    }

    /// The average of a subject's weeks (the given ones; all so far if empty).
    pub fn mastery_of(&self, s: Subject, weeks: &[Week]) -> f32 {
        if weeks.is_empty() {
            return 0.0;
        }
        weeks.iter().map(|w| self.mastery(s, w.year_week())).sum::<f32>() / weeks.len() as f32
    }

    /// A lesson's worth of learning about a week: `perf` (0..1), times `mult`. Subjects with fewer
    /// lessons a week learn more per lesson.
    fn learn(&mut self, s: Subject, year_week: usize, perf: f32, mult: f32) {
        let k = (0.5 * perf * mult).clamp(0.0, 0.95);
        let per = 1.0 - (1.0 - k).powf(4.0 / per_week(s) as f32);
        let m = &mut self.subject_mut(s).mastery[year_week.min(YEAR_WEEKS)];
        *m += (1.0 - *m) * per;
    }

    pub fn tests_of(&self, e: Exam) -> impl Iterator<Item = &TestMark> {
        self.tests.iter().filter(move |t| t.exam == e)
    }

    pub fn test(&self, e: Exam, s: Subject) -> Option<&TestMark> {
        self.tests.iter().find(|t| t.exam == e && t.subject == s)
    }

    fn virtue(&mut self, v: Virtue) {
        let i = match v {
            Virtue::Kindness => 0,
            Virtue::Honesty => 1,
            Virtue::Courage => 2,
            Virtue::Fairness => 3,
            Virtue::Responsibility => 4,
            Virtue::Respect => 5,
        };
        self.virtues[i] += 1;
    }
}

/// What's on in a period: the lesson (or test), its week, its teacher.
#[derive(Clone, Debug, PartialEq)]
pub struct LessonInfo {
    pub date: Date,
    pub period: u8,
    pub subject: Subject,
    pub week: Week,
    pub exam: Option<Exam>,
    pub teacher: String,
}

impl LessonInfo {
    /// "Maths · 1. Positive and negative numbers" / "1st-term finals · maths"
    pub fn title(&self) -> String {
        match self.exam {
            Some(e) => format!("{}: {}", capital(e.name()), self.subject.name()),
            None => format!("{} · {}", capital(self.subject.name()), self.week.unit),
        }
    }
    /// The week's topic ("Adding and subtracting"), or what the test covers.
    pub fn topic(&self) -> String {
        match self.exam {
            Some(_) => "Answer what you can. No talking.".into(),
            None => self.week.topic.to_string(),
        }
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// A lesson being played.
#[derive(Clone, Debug)]
pub struct Lesson {
    pub info: LessonInfo,
    pub play: Play,
    /// game minutes per real second while it's played: the period lasts as long as the game
    pub pace: f32,
}

/// How many tasks a full period of a subject has.
pub fn tasks_per_lesson(s: Subject, exam: bool) -> usize {
    match (s, exam) {
        (Subject::Pe, true) => 8,
        (_, true) => 10,
        (Subject::Maths | Subject::Japanese | Subject::English, _) => 8,
        (Subject::Pe, _) => 4,
        (Subject::Moral | Subject::ClassActivities, _) => 2,
        (Subject::Integrated, _) => 5,
        _ => 6,
    }
}

fn focus_index(f: Focus) -> usize {
    match f {
        Focus::Listen => 0,
        Focus::Notes => 1,
        Focus::Doodle => 2,
        Focus::Doze => 3,
    }
}

/// How much of a lesson goes in, sitting there without playing, by focus.
const PASSIVE: [f32; 4] = [0.45, 0.6, 0.15, 0.0];

/// The time the school year ends: the closing ceremony (9:00 in the gym) and the last homeroom,
/// with the report cards, on 24 March 2027.
pub fn year_end(school_year: i32) -> GameTime {
    GameTime::new(syllabus::closing_day(school_year), crate::clock::at::hm(10, 30))
}

impl GameState {
    /// The lesson or test in a period today (none on weekends, holidays, ceremony days).
    pub fn lesson_info(&self, period: u8) -> Option<LessonInfo> {
        let d = self.time.date;
        if !is_school_day(d) || syllabus::ceremony_day(d) || period == 0 || period > school::periods_on(d.weekday() as u8) {
            return None;
        }
        let tt = school::timetable();
        let homeroom = tt.teacher(tt.homeroom[census::school::Timetable::class_index(school::MY_CLASS).unwrap_or(0)]).sensei();
        if let Some((e, s)) = exam_in(d, period) {
            let week = week_on(s, d).or_else(|| syllabus::week_of(s, e.term(), 1))?;
            return Some(LessonInfo { date: d, period, subject: s, week, exam: Some(e), teacher: homeroom });
        }
        let l = school::my_lesson(d.weekday() as u8, period)?;
        let week = week_on(l.subject, d)?;
        Some(LessonInfo { date: d, period, subject: l.subject, week, exam: None, teacher: tt.teacher(l.teacher).sensei() })
    }

    /// A lesson waiting to be played: you're at your desk, in a period, and haven't played it.
    pub fn lesson_due(&self) -> Option<u8> {
        if self.location != Location::School || !matches!(self.doing(), Some(Activity::Class { .. })) {
            return None;
        }
        let wd = self.time.date.weekday() as u8;
        let Slot::Period(p) = school::slot_at(wd, self.time.minute) else { return None };
        if self.lessons.day == Some(self.time.date) && self.lessons.played[p as usize - 1] {
            return None;
        }
        let left = PERIODS[p as usize - 1].1.saturating_sub(self.time.minute);
        (left >= 5 && self.lesson_info(p).is_some()).then_some(p)
    }

    /// Start the lesson that's due: its tasks for the week (fewer if you sat down late), shaped by
    /// how you are.
    pub fn begin_lesson(&mut self) -> Option<Lesson> {
        let p = self.lesson_due()?;
        let info = self.lesson_info(p)?;
        let left = PERIODS[p as usize - 1].1 - self.time.minute;
        let full = tasks_per_lesson(info.subject, info.exam.is_some());
        let n = ((full as f32 * left as f32 / 50.0).ceil() as usize).clamp(1, full);
        let mut rng = Rng::derive(self.seed ^ (info.date.to_days() as u64).wrapping_mul(31) ^ p as u64, "lesson");
        let tasks = match info.exam {
            Some(e) => content::exam_tasks(info.subject, &e.covers(info.subject, syllabus::school_year(info.date)), n, &mut rng),
            None => content::tasks(&info.week, n, &mut rng),
        };
        let notes = matches!(self.doing(), Some(Activity::Class { focus: Focus::Notes }));
        let cond = Condition { energy: self.player.energy, fullness: self.life.fullness, notes };
        let play = Play::new(tasks, cond);
        let pace = (left as f32 / play.expected_seconds().max(1.0)).clamp(0.08, 2.0);
        let d = self.time.date;
        self.lessons.today(d);
        self.lessons.played[p as usize - 1] = true;
        Some(Lesson { info, play, pace })
    }

    /// Change how you're sitting through the lesson (notes, dozing...), if you're in one.
    pub fn set_focus(&mut self, focus: Focus) {
        if let Some(d) = self.life.doing.as_mut() {
            if let Activity::Class { focus: f } = &mut d.activity {
                *f = focus;
            }
        }
    }

    /// The lesson's over (the bell, or you finished early): what you got of it. Returns a line
    /// for the screen.
    pub fn finish_lesson(&mut self, l: &Lesson) -> String {
        let s = l.info.subject;
        let score = l.play.score();
        let d = l.info.date;
        let year = syllabus::school_year(d);
        let (right, n) = (l.play.right(), l.play.tasks.len());
        self.lessons.ensure();
        for v in &l.play.virtues {
            self.lessons.virtue(*v);
            let p = &mut self.life.progress;
            match v {
                Virtue::Kindness | Virtue::Respect => p.charm += 0.5,
                Virtue::Courage | Virtue::Honesty => p.courage += 0.5,
                Virtue::Fairness | Virtue::Responsibility => {
                    p.charm += 0.25;
                    p.courage += 0.25;
                }
            }
        }
        self.life.progress.knowledge += score * 1.5;
        let line = if let Some(e) = l.info.exam {
            let cov = e.covers(s, year);
            let m = if cov.is_empty() { self.lessons.mastery_of(s, &syllabus::year_of(s)) } else { self.lessons.mastery_of(s, &cov) };
            let mark = (100.0 * (0.6 * score + 0.4 * m)).round().clamp(0.0, 100.0) as u32;
            self.lessons.tests.retain(|t| !(t.exam == e && t.subject == s));
            self.lessons.tests.push(TestMark { exam: e, subject: s, mark, sat: true, date: d });
            self.log(format!("{}: {} test, {mark} marks.", capital(e.name()), s.name()));
            format!("{} test: {right} of {n}. Your mark will be about {mark}.", capital(s.name()))
        } else {
            let mult = l.play.cond.learn_mult();
            let w = l.info.week;
            if w.review() {
                for k in content::review_weeks(&w) {
                    self.lessons.learn(s, k, score * 0.5, mult);
                }
            } else {
                self.lessons.learn(s, w.year_week(), score, mult);
            }
            let t = &mut self.lessons.subject_mut(s).terms[w.term as usize - 1];
            t.played += 1;
            t.perf += score;
            // taking notes: the board goes into the subject's notebook
            if l.play.cond.notes && !self.lessons.notes.iter().any(|n| n.subject == s && n.date == d) {
                let lines = paper::board(&w, d, self.seed);
                self.lessons.notes.push(paper::Notes { subject: s, date: d, lines });
            }
            self.log(format!("{} ({}): {right} of {n}.", capital(s.name()), w.topic));
            if matches!(s, Subject::Moral | Subject::ClassActivities) {
                format!("{}: you said what you thought.", capital(s.name()))
            } else {
                format!("{}: {right} of {n}{}", capital(s.name()), if l.play.best_streak >= 4 { format!(", a run of {}!", l.play.best_streak) } else { ".".into() })
            }
        };
        // the print, marked and handed back (kept: the latest few of each subject)
        let mut sheet = paper::Sheet::from_lesson(l, true);
        if let Some(t) = self.lessons.test(l.info.exam.unwrap_or(Exam::Mid1), s).filter(|_| l.info.exam.is_some()) {
            sheet.mark = Some(t.mark);
        }
        self.lessons.sheets.push(sheet);
        if self.lessons.sheets.iter().filter(|x| x.subject == s).count() > 6 {
            if let Some(i) = self.lessons.sheets.iter().position(|x| x.subject == s) {
                self.lessons.sheets.remove(i);
            }
        }
        if l.play.dozed > 20.0 {
            self.log(format!("Dozed through part of {}.", s.name()));
        }
        line
    }

    /// (state.rs: each minute seated in a period)
    pub(crate) fn lessons_sat(&mut self, period_index: usize, focus: Focus) {
        let d = self.time.date;
        self.lessons.today(d);
        if let Some(f) = self.lessons.focus.get_mut(period_index) {
            f[focus_index(focus)] = f[focus_index(focus)].saturating_add(1);
        }
    }

    /// (state.rs: the bell at the end of the morning's or the day's lessons) What each period of
    /// the block came to: lessons sat through or missed, tests taken or not, and on a term's last
    /// day, the report card.
    pub(crate) fn lessons_bell(&mut self, afternoon: bool, periods: &[usize]) {
        let d = self.time.date;
        if !is_school_day(d) {
            return;
        }
        self.lessons.ensure();
        self.lessons.today(d);
        let year = syllabus::school_year(d);
        let ceremony = syllabus::ceremony_day(d);
        for &p in periods.iter().filter(|_| !ceremony) {
            let period = p as u8 + 1;
            let Some(info) = self.lesson_info(period) else { continue };
            let len = PERIODS[p].1 - PERIODS[p].0;
            let sat = self.life.class_minutes[p] * 4 >= len * 3;
            let played = self.lessons.played[p];
            let fm = self.lessons.focus[p];
            let total: u16 = fm.iter().sum();
            let passive = if total == 0 { 0.0 } else { (0..4).map(|k| PASSIVE[k] * fm[k] as f32).sum::<f32>() / total as f32 };
            let s = info.subject;
            if let Some(e) = info.exam {
                if played {
                    continue;
                }
                let cov = e.covers(s, year);
                let m = self.lessons.mastery_of(s, &cov);
                let (mark, ok) = if sat { (((0.6 * 0.9 * m + 0.4 * m) * 100.0 * (0.5 + passive)).round().min(100.0) as u32, true) } else { (0, false) };
                self.lessons.tests.retain(|t| !(t.exam == e && t.subject == s));
                self.lessons.tests.push(TestMark { exam: e, subject: s, mark, sat: ok, date: d });
                if !ok {
                    self.log(format!("Missed the {} test.", s.name()));
                }
                continue;
            }
            let w = info.week;
            let ti = w.term as usize - 1;
            if sat {
                self.lessons.subject_mut(s).terms[ti].lessons += 1;
                if !played {
                    if w.review() {
                        for k in content::review_weeks(&w) {
                            self.lessons.learn(s, k, passive * 0.5, 1.0);
                        }
                    } else {
                        self.lessons.learn(s, w.year_week(), passive, 1.0);
                    }
                    self.lessons.subject_mut(s).terms[ti].perf += passive;
                }
            } else {
                self.lessons.subject_mut(s).terms[ti].missed += 1;
            }
        }
        if afternoon {
            if let Some((t, _)) = term_week(d) {
                if t < 3 && last_school_day_of_term(d) {
                    self.issue_report(t);
                }
            }
        }
    }

    /// The report card for a term.
    fn issue_report(&mut self, term: u8) {
        if self.lessons.reports.iter().any(|r| r.term == term) {
            return;
        }
        self.lessons.ensure();
        let lines = SUBJECTS
            .iter()
            .map(|&s| {
                let st = self.lessons.subject(s).map(|r| r.terms[term as usize - 1]).unwrap_or_default();
                let marks: Vec<u32> = self.lessons.tests.iter().filter(|t| t.subject == s && t.exam.term() == term).map(|t| t.mark).collect();
                let tavg = (!marks.is_empty()).then(|| marks.iter().sum::<u32>() as f32 / marks.len() as f32);
                let l = st.average().unwrap_or(0.0);
                let a = st.attendance().unwrap_or(0.0);
                if !graded(s) {
                    let comment = match a {
                        a if a >= 0.95 => "Took a full part all term.",
                        a if a >= 0.75 => "Took part well.",
                        a if a > 0.0 => "Missed a good deal of it.",
                        _ => "Was hardly ever there.",
                    };
                    return ReportLine { subject: s, grade: None, comment: comment.into() };
                }
                let score = match tavg {
                    Some(t) => 0.5 * t + 35.0 * l + 15.0 * a,
                    None => 70.0 * l + 30.0 * a,
                };
                let grade = match score {
                    x if x >= 85.0 => 5,
                    x if x >= 70.0 => 4,
                    x if x >= 45.0 => 3,
                    x if x >= 25.0 => 2,
                    _ => 1,
                };
                let tests = tavg.map(|t| format!("tests {:.0} · ", t)).unwrap_or_default();
                ReportLine { subject: s, grade: Some(grade), comment: format!("{tests}lessons {:.0}% · attendance {:.0}%", l * 100.0, a * 100.0) }
            })
            .collect();
        let card = ReportCard { term, date: self.time.date, lines, absent: self.player.days_absent, late: self.player.days_late };
        let line = format!("Got my {} report card.", ["1st-term", "2nd-term", "3rd-term"][term as usize - 1]);
        self.lessons.reports.push(card);
        self.say(format!("{line} (It's in your journal.)"));
        self.log(line);
    }

    /// When this school year ends (the closing ceremony).
    pub fn year_end_time(&self) -> GameTime {
        year_end(syllabus::school_year(START_DATE))
    }

    /// Whether the school year (and the game) is over.
    pub fn year_over(&self) -> bool {
        self.time >= self.year_end_time()
    }

    /// The closing ceremony: the 3rd term's report card, and the year in sum. Once; after that it
    /// returns the same summary.
    pub fn close_year(&mut self) -> YearSummary {
        if let Some(s) = &self.lessons.summary {
            return s.clone();
        }
        self.lessons.ensure();
        self.issue_report(3);
        let at = self.location == Location::School;
        let grades: Vec<(Subject, Option<u8>)> = SUBJECTS
            .iter()
            .map(|&s| {
                let gs: Vec<u8> = self.lessons.reports.iter().filter_map(|r| r.grade(s)).collect();
                (s, (!gs.is_empty()).then(|| (gs.iter().map(|&g| g as f32).sum::<f32>() / gs.len() as f32).round() as u8))
            })
            .collect();
        let mut lines = vec!["Kazehana Junior High, class 1-A: the 2026–27 school year.".to_string()];
        lines.push(if at { "You were there for the closing ceremony.".into() } else { "You weren't at the closing ceremony.".into() });
        for (s, g) in &grades {
            let terms: Vec<String> = self.lessons.reports.iter().map(|r| r.grade(*s).map(|g| g.to_string()).unwrap_or_else(|| "–".into())).collect();
            match g {
                Some(g) => lines.push(format!("{}: {g}  (terms: {})", capital(s.name()), terms.join(", "))),
                None => lines.push(format!("{}: {}", capital(s.name()), self.lessons.reports.last().and_then(|r| r.lines.iter().find(|l| l.subject == *s)).map(|l| l.comment.as_str()).unwrap_or(""))),
            }
        }
        let sat: Vec<u32> = self.lessons.tests.iter().map(|t| t.mark).collect();
        if !sat.is_empty() {
            lines.push(format!("Tests: {} taken, averaging {:.0}.", sat.len(), sat.iter().sum::<u32>() as f32 / sat.len() as f32));
        }
        if let Some((best, _)) = grades.iter().filter_map(|(s, g)| g.map(|g| (*s, g))).max_by_key(|x| x.1) {
            lines.push(format!("Best subject: {}.", best.name()));
        }
        lines.push(format!("Mornings or afternoons skipped: {} · late: {}.", self.player.days_absent, self.player.days_late));
        lines.push(format!("Knowledge {} · charm {} · courage {} · ¥{}.", self.player.knowledge, self.player.charm, self.player.courage, self.player.money));
        lines.push(format!("Cards: {} in your collection.", self.player.collection.total()));
        let friends = self.bonds.iter().filter(|b| b.rank() >= 3).count();
        lines.push(format!("Friends: {friends}."));
        let summary = YearSummary { grades, lines, at_ceremony: at };
        self.lessons.summary = Some(summary.clone());
        self.log("The closing ceremony. The school year's over.");
        summary
    }
}

/// Whether no school day is left in a date's term after it.
pub fn last_school_day_of_term(d: Date) -> bool {
    let t = syllabus::term_of(d);
    let mut x = d.add_days(1);
    for _ in 0..60 {
        if is_school_day(x) {
            return syllabus::term_of(x) != t;
        }
        x = x.add_days(1);
    }
    true
}

#[cfg(test)]
mod tests;
