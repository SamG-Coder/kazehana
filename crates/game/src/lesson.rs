//! Lessons at your desk in 1-A (docs/lessons_plan.md). Sit down at your desk, and at the bell
//! the lesson is worked on the paper in front of you: the print handed out (the week's questions,
//! your answers in pencil as you write them) and the subject's notebook beside it (the board
//! copied down while you take notes). The period's clock runs while you work, so the lesson ends
//! with the bell; then the print is marked in red and handed back. And the year's end: the
//! closing ceremony's summary and the report cards.
//!
//! Seated in a lesson: left/right point at an answer on the print, confirm writes it (a number:
//! up/down +-1, left/right +-10; an order: left/right and confirm to place each piece, down to rub
//! out) · back puts your head down (doze; back again to wake) · binder key: take notes ·
//! calendar key: look up at the board or down at your paper · people key: raise your hand (the
//! toilet).

use super::{Game, Key, Mode};
use crate::draw::{self, DIM, GOLD, INK};
use akari::glam::Vec3;
use akari::{Rgba, Ui};
use sim::lessons::paper::Sheet;
use sim::lessons::play::{choice_input, show_number, Input, Task};
use sim::lessons::{Lesson, SUBJECTS};
use sim::life::{Activity, Focus};
use sim::school::Slot;
use sim::Location;

const GOOD: Rgba = Rgba(120, 220, 150, 255);
const BAD: Rgba = Rgba(240, 110, 110, 255);

/// What you're looking at in your seat during a lesson (the calendar key goes round them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// bent over the print, writing
    Sheet,
    /// the notebook beside it
    Notebook,
    /// up at the board (and the teacher)
    Board,
}

/// A lesson being worked at your desk.
pub struct LessonView {
    pub lesson: Lesson,
    /// game minutes not yet passed
    pub acc: f32,
    /// taking notes (the binder key)
    pub notes: bool,
    /// the focus we last set (to notice being woken by the teacher)
    pub set: Focus,
    /// how it went, once it's over (the print handed back): shown till you confirm
    pub result: Option<String>,
    /// the answer you're pointing at on the print (a choice)
    pub sel: usize,
    /// what you're looking at
    pub view: View,
}

impl LessonView {
    pub fn new(lesson: Lesson) -> Self {
        let notes = lesson.play.cond.notes;
        LessonView { lesson, acc: 0.0, notes, set: if notes { Focus::Notes } else { Focus::Listen }, result: None, sel: 0, view: View::Sheet }
    }

    /// What you're writing for the question you're on (drawn faint on the print).
    pub fn draft(&self) -> Option<String> {
        let p = &self.lesson.play;
        match p.task()? {
            Task::Choice { options, .. } => options.get(self.sel.min(options.len().saturating_sub(1))).cloned(),
            Task::Number { decimals, .. } if p.cur.value != 0 => Some(show_number(p.cur.value, *decimals)),
            Task::Order { pieces, shown, .. } => {
                let left: Vec<usize> = shown.iter().copied().filter(|i| !p.cur.picked.contains(i)).collect();
                let mut seq: Vec<&str> = p.cur.picked.iter().map(|&i| pieces[i].as_str()).collect();
                if let Some(&next) = left.get(p.cur.sel.min(left.len().saturating_sub(1))) {
                    seq.push(pieces[next].as_str());
                }
                (!seq.is_empty()).then(|| seq.join(" → "))
            }
            Task::Reflect { options, .. } => options.get(p.cur.sel).map(|o| o.0.clone()),
            _ => None,
        }
    }
}

fn to_input(k: Key) -> Option<Input> {
    Some(match k {
        Key::Up => Input::Up,
        Key::Down => Input::Down,
        Key::Left => Input::Left,
        Key::Right => Input::Right,
        Key::Confirm => Input::Confirm,
        Key::Back => Input::Back,
        _ => return None,
    })
}

/// Whether a task is worked on paper (PE's timing, music's phrases and art's colours belong to
/// the gym and the special rooms).
fn on_paper(t: &Task) -> bool {
    !matches!(t, Task::Timing { .. } | Task::Recall { .. } | Task::Slider { .. })
}

/// Your seat at your desk, in the desk's frame (glTF axes): the chair 0.40 m behind it; your eye
/// about 1.08 m up, a little forward of the chair's middle.
const SEAT: Vec3 = Vec3::new(0.0, 0.0, 0.40);
const EYE_SEATED: f32 = 1.08;

impl Game {
    /// Your seat in the world: (where your feet go, your eye, the yaw facing the board).
    pub fn seat(&self) -> Option<(Vec3, Vec3, f32)> {
        let d = self.your_desk?;
        let feet = d.transform_point3(SEAT);
        let eye = d.transform_point3(SEAT + Vec3::new(0.0, EYE_SEATED, -0.04));
        let f = d.transform_vector3(-Vec3::Z);
        Some((feet, eye, f.x.atan2(-f.z)))
    }

    /// In your seat at your desk, sitting through lessons.
    pub fn seated(&self) -> bool {
        matches!(self.state.doing(), Some(Activity::Class { .. })) && self.seat().is_some_and(|(feet, _, _)| (self.player.feet - feet).with_y(0.0).length() < 0.6)
    }

    /// (update, in town) Where you are, for the sim: at school in the junior high, at home in
    /// Tsukumodō, otherwise out and about.
    pub(super) fn town_location(&mut self) {
        if !self.in_town {
            return;
        }
        let p = self.player.feet;
        let loc = match self.seat() {
            Some((feet, _, _)) if (p - feet).with_y(0.0).length() < 70.0 => Location::School,
            _ if (p - self.start.0).with_y(0.0).length() < 25.0 => Location::Home,
            _ => Location::Path,
        };
        if loc != self.state.location {
            self.state.location = loc;
        }
    }

    /// (prompt) By your desk, standing: you can sit down.
    pub(super) fn desk_prompt(&self) -> Option<String> {
        if !self.in_town || self.state.doing().is_some() {
            return None;
        }
        let (feet, _, _) = self.seat()?;
        ((self.player.feet - feet).with_y(0.0).length() < 1.3).then(|| "Sit at your desk".to_string())
    }

    /// (interact) Sit down at your desk: in your seat, facing the board.
    pub(super) fn sit_at_desk(&mut self) -> bool {
        if self.desk_prompt().is_none() {
            return false;
        }
        let Some((feet, _, yaw)) = self.seat() else { return false };
        match self.state.start(Activity::Class { focus: Focus::Listen }) {
            Ok(_) => {
                self.player.teleport(feet, yaw);
                self.say("You sit down at your desk.");
            }
            Err(e) => self.say(e),
        }
        true
    }

    /// (each frame, after update) Seated, the camera is your eyes in your seat: up at the board,
    /// or bent over your paper while you work on it (leaning in, your eyes on the page).
    pub fn seat_camera(&mut self) {
        const FOV: f32 = 65.0 * std::f32::consts::PI / 180.0;
        const FOV_PAPER: f32 = 44.0 * std::f32::consts::PI / 180.0;
        if !self.seated() {
            self.cam.fov_y = FOV;
            return;
        }
        let (Some((_, eye, yaw)), Some(d)) = (self.seat(), self.your_desk) else { return };
        let view = match &self.mode {
            Mode::Lesson(v) => v.view,
            _ => View::Board,
        };
        let at = |(x, y, w): (f32, f32, f32)| (d * crate::paper::on_desk(x, y, w)).transform_point3(Vec3::ZERO);
        let target = match view {
            View::Sheet => Some(at(crate::paper::SHEET_ON_DESK)),
            View::Notebook => Some(at(crate::paper::NOTEBOOK_ON_DESK)),
            View::Board => None,
        };
        match target {
            Some(t) => {
                // squared up to the page and leaning in over it: still facing the front (so the
                // page reads upright), shifted across in front of it, bent a little forward
                let f = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
                let r = Vec3::new(yaw.cos(), 0.0, yaw.sin());
                let rel = t - eye;
                let pos = eye + r * rel.dot(r) * 0.9 + f * rel.dot(f) * 0.25 + Vec3::Y * rel.y * 0.15;
                let to = t - pos;
                self.cam.position = pos;
                self.cam.yaw = yaw;
                self.cam.pitch = to.y.atan2(to.dot(f));
                self.cam.fov_y = FOV_PAPER;
            }
            None => {
                self.cam.position = eye;
                self.cam.yaw = yaw;
                self.cam.pitch = -0.08;
                self.cam.fov_y = FOV;
            }
        }
    }

    /// Your paper on your desk while you're in your seat: the print you're working on (or the
    /// one just handed back), the subject's notebook. (The app draws it: crate::paper.)
    pub fn desk_paper(&self) -> Option<crate::paper::DeskPaper> {
        if !self.seated() {
            return None;
        }
        let desk = self.your_desk?;
        let (sheet, subject) = match &self.mode {
            Mode::Lesson(v) => {
                let sheet = if v.result.is_some() { self.state.lessons.sheets.last().cloned() } else { Some(Sheet::live(&v.lesson, v.draft())) };
                (sheet, Some(v.lesson.info.subject))
            }
            _ => {
                // (the last print handed back today stays on the desk)
                let today = self.state.time.date;
                let s = self.state.lessons.sheets.last().filter(|s| s.date == today).cloned();
                let sub = s.as_ref().map(|s| s.subject);
                (s, sub)
            }
        };
        let notes = subject.map(|s| (s, self.state.lessons.notes.iter().filter(|n| n.subject == s).cloned().collect()));
        Some(crate::paper::DeskPaper { desk, sheet, notes })
    }

    /// (update, exploring) Seated at the bell: the lesson starts. In town, only the lessons
    /// taught in your classroom are worked at your desk (the special rooms' come with their
    /// rooms).
    pub(super) fn lesson_check(&mut self) {
        if !matches!(self.mode, Mode::Explore) {
            return;
        }
        if self.in_town {
            let Some(p) = self.state.lesson_due() else { return };
            let Some(info) = self.state.lesson_info(p) else { return };
            use sim::school::Subject as S;
            if info.exam.is_none() && matches!(info.subject, S::Science | S::Pe | S::TechHome | S::Music | S::Art) {
                return;
            }
        }
        if let Some(l) = self.state.begin_lesson() {
            self.mode = Mode::Lesson(Box::new(LessonView::new(l)));
        }
    }

    /// (update) The year's over: the closing ceremony's summary.
    pub(super) fn year_check(&mut self) -> bool {
        if !self.state.year_over() {
            return false;
        }
        if !matches!(self.mode, Mode::YearEnd { .. }) {
            if let Mode::Lesson(v) = &mut self.mode {
                v.lesson.play.end();
                let l = v.lesson.clone();
                self.state.finish_lesson(&l);
            }
            self.state.stop();
            self.state.close_year();
            self.mode = Mode::YearEnd { page: 0 };
        }
        true
    }

    /// (update, in a lesson) The lesson's clock: your work and the period run together.
    pub(super) fn lesson_update(&mut self, dt: f32) {
        let Mode::Lesson(v) = &mut self.mode else { return };
        if v.result.is_some() {
            return;
        }
        // head down on the desk is dozing; the teacher may wake you (the sim sets you back to
        // listening)
        let now = match self.state.doing() {
            Some(Activity::Class { focus }) => Some(*focus),
            _ => None,
        };
        if v.set == Focus::Doze && now == Some(Focus::Listen) && v.lesson.play.dozing {
            v.lesson.play.dozing = false;
        }
        let want = if v.lesson.play.dozing {
            Focus::Doze
        } else if v.notes {
            Focus::Notes
        } else {
            Focus::Listen
        };
        if now.is_some() && now != Some(want) {
            self.state.set_focus(want);
        }
        v.set = want;
        // (on paper there's no "right!" after each answer: you go on to the next question; it's
        // marked when it's handed back)
        if v.lesson.play.feedback.is_some() {
            v.lesson.play.input(Input::Confirm);
            v.sel = 0;
        }
        // (a task not done on paper here: you go through it as well as you know the week)
        if v.lesson.play.task().is_some_and(|t| !on_paper(t)) && !v.lesson.play.dozing {
            let m = self.state.lessons.mastery(v.lesson.info.subject, v.lesson.info.week.year_week());
            let mut rng = tcg::Rng::new(self.state.time.minute as u64 ^ v.lesson.play.i as u64);
            v.lesson.play.auto(0.35 + 0.6 * m, &mut rng);
        }
        v.lesson.play.tick(dt);
        v.acc += dt * v.lesson.pace;
        let whole = v.acc.floor();
        if whole >= 1.0 {
            v.acc -= whole;
            self.state.advance(whole as u32, &self.db);
        }
        // the bell (or you've left your seat: the toilet)
        let wd = self.state.time.date.weekday() as u8;
        let p = v.lesson.info.period;
        let still = self.state.time.date == v.lesson.info.date && sim::school::slot_at(wd, self.state.time.minute) == Slot::Period(p) && matches!(self.state.doing(), Some(Activity::Class { .. }));
        if !still {
            v.lesson.play.end();
        }
        if v.lesson.play.finished {
            let l = v.lesson.clone();
            let line = self.state.finish_lesson(&l);
            if let Mode::Lesson(v) = &mut self.mode {
                v.view = View::Sheet;
                v.result = Some(if still { line } else { format!("{line} (The bell.)") });
            }
        }
    }

    /// (key, in a lesson)
    pub(super) fn lesson_key(&mut self, mut v: Box<LessonView>, k: Key) -> Mode {
        if v.result.is_some() {
            return if matches!(k, Key::Confirm | Key::Back) { Mode::Explore } else { Mode::Lesson(v) };
        }
        match k {
            Key::Binder => {
                v.notes = !v.notes;
                v.lesson.play.cond.notes = v.notes;
            }
            Key::Calendar => {
                v.view = match v.view {
                    View::Sheet => View::Notebook,
                    View::Notebook => View::Board,
                    View::Board => View::Sheet,
                }
            }
            Key::People => {
                // raise your hand: let out to the toilet, the rest of the lesson goes without you
                match self.state.ask_toilet() {
                    Ok(line) => {
                        v.lesson.play.end();
                        let got = self.state.finish_lesson(&v.lesson);
                        self.say(line);
                        self.say(got);
                        return Mode::Explore;
                    }
                    Err(line) => self.say(line),
                }
            }
            k => {
                let Some(i) = to_input(k) else { return Mode::Lesson(v) };
                let task = v.lesson.play.task().cloned();
                let free = v.lesson.play.feedback.is_none() && !v.lesson.play.dozing;
                match task {
                    // a choice: point at it on the print, confirm writes it
                    Some(Task::Choice { options, .. }) if free && i != Input::Back => {
                        let n = options.len().max(1);
                        match i {
                            Input::Left | Input::Up => v.sel = (v.sel + n - 1) % n,
                            Input::Right | Input::Down => v.sel = (v.sel + 1) % n,
                            _ => {
                                let pick = choice_input(n, v.sel.min(n - 1));
                                v.lesson.play.input(pick);
                                v.sel = 0;
                            }
                        }
                    }
                    _ => v.lesson.play.input(i),
                }
            }
        }
        Mode::Lesson(v)
    }

    pub(super) fn year_key(&mut self, page: usize, k: Key) -> Mode {
        let pages = 1 + self.state.lessons.reports.len();
        match k {
            Key::Right | Key::Down | Key::Confirm => Mode::YearEnd { page: (page + 1) % pages },
            Key::Left | Key::Up => Mode::YearEnd { page: (page + pages - 1) % pages },
            _ => Mode::YearEnd { page },
        }
    }

    // ------------------------------------------------------------------ drawing

    /// In a lesson the work is on your desk (drawn in the world); on screen only the clock, what
    /// you're doing, and the buttons.
    pub(super) fn draw_lesson(&self, ui: &mut Ui, v: &LessonView) {
        let s = ui.scale();
        draw::hud(ui, &self.state, &format!("1-A · {}", v.lesson.info.title()));
        draw::toasts(ui, &self.toasts);
        let p = &v.lesson.play;
        if p.dozing {
            // (eyes closed)
            ui.rect(0.0, 0.0, ui.width, ui.height, Rgba(4, 4, 8, 225));
            ui.text_centered(ui.width * 0.5, ui.height * 0.45, 40.0 * s, DIM, "z z z . . .");
        }
        if let Some(r) = &v.result {
            // the print handed back
            let w = (ui.measure(26.0 * s, r) + 80.0 * s).max(520.0 * s);
            let (x, y) = (ui.width * 0.5 - w * 0.5, ui.height - 230.0 * s);
            draw::panel(ui, x, y, w, 110.0 * s);
            ui.text_centered(ui.width * 0.5, y + 22.0 * s, 26.0 * s, INK, r);
            ui.text_centered(ui.width * 0.5, y + 64.0 * s, 18.0 * s, DIM, &format!("Handed back, marked: {:.0}% of the lesson", p.score() * 100.0));
            draw::hints(ui, &self.buttons(), "{confirm} Put it away");
            return;
        }
        // the time left on the question you're on: a thin line along the foot of the screen
        if p.task().is_some() && !p.dozing {
            let left = ((p.limit - p.t) / p.limit.max(0.01)).clamp(0.0, 1.0);
            let bw = 420.0 * s;
            let (bx, by) = (ui.width * 0.5 - bw * 0.5, ui.height - 96.0 * s);
            ui.rect(bx, by, bw, 4.0 * s, Rgba(255, 255, 255, 40));
            ui.rect(bx, by, bw * left, 4.0 * s, if left < 0.25 { BAD } else { GOOD });
        }
        let mut flags = Vec::new();
        if v.notes {
            flags.push("✎ taking notes");
        }
        match v.view {
            View::Notebook => flags.push("reading your notebook"),
            View::Board => flags.push("looking at the board"),
            View::Sheet => {}
        }
        if !flags.is_empty() {
            ui.text_centered(ui.width * 0.5, ui.height - 130.0 * s, 18.0 * s, GOLD, &flags.join(" · "));
        }
        let hint = match p.task() {
            _ if p.dozing => "{back} Wake up",
            Some(Task::Number { .. }) => "{browse} Write the number · {confirm} Done · {back} Doze · {binder} Notes · {calendar} Look · {people} Raise your hand",
            Some(Task::Order { .. }) => "{sideways} Choose · {confirm} Write it · {back} Doze · {binder} Notes · {calendar} Look · {people} Raise your hand",
            Some(Task::Sort { .. }) => "{sideways} Which side · {back} Doze · {binder} Notes · {calendar} Look · {people} Raise your hand",
            Some(Task::Reflect { .. }) => "{choose} Choose · {confirm} Write it · {back} Doze · {calendar} Look · {people} Raise your hand",
            _ => "{sideways} Choose · {confirm} Write it · {back} Doze · {binder} Notes · {calendar} Look · {people} Raise your hand",
        };
        draw::hints(ui, &self.buttons(), hint);
    }

    pub(super) fn draw_year_end(&self, ui: &mut Ui, page: usize) {
        let s = ui.scale();
        draw::dim(ui, 0.92);
        let (w, h) = (ui.width.min(1200.0 * s), ui.height - 120.0 * s);
        let (x, y) = ((ui.width - w) * 0.5, 50.0 * s);
        draw::panel(ui, x, y, w, h);
        let r = &self.state.lessons;
        if page == 0 {
            ui.text(x + 36.0 * s, y + 24.0 * s, 36.0 * s, INK, "The closing ceremony");
            ui.text(x + 36.0 * s, y + 70.0 * s, 20.0 * s, DIM, "24 March 2027. The school year is over.");
            let mut ly = y + 120.0 * s;
            if let Some(sum) = &r.summary {
                for l in &sum.lines {
                    ui.text(x + 36.0 * s, ly, 22.0 * s, INK, l);
                    ly += 34.0 * s;
                }
            }
        } else if let Some(card) = r.reports.get(page - 1) {
            ui.text(x + 36.0 * s, y + 24.0 * s, 34.0 * s, INK, &format!("Report card · {} term", ["1st", "2nd", "3rd"][card.term as usize - 1]));
            ui.text(x + 36.0 * s, y + 70.0 * s, 20.0 * s, DIM, &format!("{} · 1-A · mornings or afternoons skipped {} · late {}", card.date, card.absent, card.late));
            let mut ly = y + 120.0 * s;
            for &sub in &SUBJECTS {
                let Some(line) = card.lines.iter().find(|l| l.subject == sub) else { continue };
                ui.text(x + 36.0 * s, ly, 24.0 * s, INK, sub.name());
                if let Some(g) = line.grade {
                    ui.text(x + 420.0 * s, ly - 2.0 * s, 30.0 * s, if g >= 4 { GOOD } else if g <= 2 { BAD } else { GOLD }, &g.to_string());
                }
                ui.text(x + 480.0 * s, ly + 2.0 * s, 20.0 * s, DIM, &line.comment);
                ly += 40.0 * s;
            }
        }
        draw::hints(ui, &self.buttons(), "{sideways} The report cards");
    }

    /// --screen lesson-<subject>: sit down for the next lesson of a subject (or the current
    /// date's 1st period) and open it (screenshots, tests).
    pub fn lesson_screen(&mut self, which: &str) {
        let want = which.strip_prefix("lesson").unwrap_or("").trim_start_matches('-');
        let subject = SUBJECTS.iter().copied().find(|s| !want.is_empty() && s.name().to_lowercase().replace(' ', "").starts_with(&want.to_lowercase()));
        let mut d = self.state.time.date;
        for _ in 0..60 {
            if sim::calendar::is_school_day(d) && !sim::lessons::syllabus::ceremony_day(d) {
                let wd = d.weekday() as u8;
                let p = (1..=sim::school::periods_on(wd)).find(|&p| subject.is_none_or(|s| sim::school::my_lesson(wd, p).map(|l| l.subject) == Some(s)));
                if let Some(p) = p {
                    self.state.time = sim::GameTime::new(d, sim::school::PERIODS[p as usize - 1].0);
                    self.state.location = sim::Location::School;
                    if let Some((feet, _, yaw)) = self.seat() {
                        self.player.teleport(feet, yaw);
                    }
                    let _ = self.state.start(Activity::Class { focus: Focus::Listen });
                    self.mode = Mode::Explore;
                    self.lesson_check();
                    return;
                }
            }
            d = d.add_days(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use akari::glam::Vec3;
    use sim::{Date, GameState, GameTime, Location};
    use tcg::CardDb;

    fn new_game() -> Game {
        let db = CardDb::generate(12);
        let state = GameState::new(7, &db);
        let levels = [crate::world::build_home(), crate::world::build_path(), crate::world::build_school()];
        Game::new(db, state, levels)
    }

    fn seated(g: &mut Game, d: Date, minute: u32) {
        g.state.time = GameTime::new(d, minute);
        g.state.location = Location::School;
        g.state.start(Activity::Class { focus: Focus::Listen }).unwrap();
    }

    fn idle() -> play::Actions {
        play::Actions::default()
    }

    fn press(g: &mut Game, i: Input) {
        let k = match i {
            Input::Up => Key::Up,
            Input::Down => Key::Down,
            Input::Left => Key::Left,
            Input::Right => Key::Right,
            Input::Confirm => Key::Confirm,
            Input::Back => Key::Back,
        };
        g.key(k, false);
    }

    #[test]
    fn the_bell_starts_the_lesson_and_you_play_it() {
        let mut g = new_game();
        // Monday 13 April, seated a minute before the 2nd period's bell
        let d = Date::new(2026, 4, 13);
        seated(&mut g, d, sim::school::PERIODS[1].0 - 1);
        for _ in 0..240 {
            g.update(1.0 / 60.0, &idle(), false);
            if matches!(g.mode, Mode::Lesson(_)) {
                break;
            }
        }
        let Mode::Lesson(v) = &g.mode else { panic!("no lesson at the bell") };
        let subject = v.lesson.info.subject;
        assert_eq!(subject, sim::school::my_lesson(0, 2).unwrap().subject);
        let mut ui = Ui::new().unwrap();
        ui.begin(1920, 1080);
        g.draw(&mut ui);
        // play it perfectly with the keys
        for _ in 0..20_000 {
            let Mode::Lesson(v) = &g.mode else { break };
            if v.result.is_some() {
                break;
            }
            // (a choice: point at it on the print, then write it)
            let choice = match v.lesson.play.task() {
                Some(Task::Choice { answer, .. }) if v.lesson.play.feedback.is_none() => Some(*answer),
                _ => None,
            };
            if let Some(a) = choice {
                for _ in 0..a {
                    g.key(Key::Right, false);
                }
                g.key(Key::Confirm, false);
                continue;
            }
            match v.lesson.play.solve_inputs() {
                Some(ks) => {
                    for k in ks {
                        press(&mut g, k);
                    }
                    ui.begin(1920, 1080);
                    g.draw(&mut ui);
                }
                None => g.update(1.0 / 60.0, &idle(), false),
            }
        }
        let Mode::Lesson(v) = &g.mode else { panic!("the lesson should show how it went") };
        assert!(v.result.as_deref().unwrap().contains("of"), "{:?}", v.result);
        assert!((v.lesson.play.score() - 1.0).abs() < 1e-4);
        ui.begin(1920, 1080);
        g.draw(&mut ui);
        let wk = sim::lessons::syllabus::week_on(subject, d).unwrap().year_week();
        assert!(g.state.lessons.mastery(subject, wk) > 0.4);
        assert!(g.state.time.minute >= sim::school::PERIODS[1].0 && g.state.time.minute < sim::school::PERIODS[1].1);
        g.key(Key::Confirm, false);
        assert!(matches!(g.mode, Mode::Explore));
        assert!(matches!(g.state.doing(), Some(Activity::Class { .. })), "still at your desk");
    }

    #[test]
    fn sitting_at_your_desk_and_working_on_the_paper() {
        let mut g = new_game();
        // the town: your desk somewhere, you standing by it on a school morning
        g.in_town = true;
        let desk = akari::glam::Affine3A::from_rotation_translation(akari::glam::Quat::from_rotation_y(1.0), Vec3::new(10.0, 5.0, -20.0));
        g.your_desk = Some(desk);
        g.start = (Vec3::new(500.0, 0.0, 500.0), 0.0);
        let (feet, _, _) = g.seat().unwrap();
        g.place_player(feet + Vec3::new(0.5, 0.0, 0.3), 0.0, 0.0);
        g.state.time = GameTime::new(Date::new(2026, 4, 13), sim::school::PERIODS[1].0 - 3);
        g.update(1.0 / 60.0, &idle(), false);
        assert_eq!(g.state.location, Location::School, "by your desk is at school");
        assert_eq!(g.prompt().as_deref(), Some("Sit at your desk"));
        g.key(Key::Confirm, false);
        assert!(g.seated(), "in your seat");
        // the bell: maths (Monday's 2nd period) is worked on the print on your desk
        for _ in 0..600 {
            g.update(1.0 / 60.0, &idle(), false);
            g.seat_camera();
            if matches!(g.mode, Mode::Lesson(_)) {
                break;
            }
        }
        assert!(matches!(g.mode, Mode::Lesson(_)), "the lesson at the bell");
        assert!(g.cam.pitch < -0.6, "looking down at your paper: {}", g.cam.pitch);
        let paper = g.desk_paper().expect("paper on the desk");
        let sheet = paper.sheet.expect("the print");
        assert_eq!(sheet.current, Some(0), "on the first question");
        // write an answer: it goes on the print in pencil
        let n = sheet.items.len();
        if let Mode::Lesson(v) = &g.mode {
            if let Some(Task::Number { .. }) = v.lesson.play.task() {
                g.key(Key::Up, false);
                let live = g.desk_paper().unwrap().sheet.unwrap();
                assert_eq!(live.items[0].given.as_deref(), Some("1"));
                assert!(live.items[0].drafting);
            }
        }
        g.key(Key::Confirm, false);
        g.update(1.0 / 60.0, &idle(), false);
        let after = g.desk_paper().unwrap().sheet.unwrap();
        assert!(after.items[0].given.is_some() && !after.items[0].drafting, "written");
        assert_eq!(after.current, Some(1).filter(|_| n > 1));
        // look at the notebook, then up at the board
        g.key(Key::Calendar, false);
        g.key(Key::Calendar, false);
        g.seat_camera();
        assert!(g.cam.pitch > -0.2, "looking at the board");
        // to the bell: the print is marked and handed back, and stays on the desk
        for _ in 0..(60 * 400) {
            g.update(1.0 / 60.0, &idle(), false);
            if matches!(&g.mode, Mode::Lesson(v) if v.result.is_some()) {
                break;
            }
        }
        let back = g.desk_paper().unwrap().sheet.unwrap();
        assert!(back.marked, "handed back marked");
        g.key(Key::Confirm, false);
        assert!(matches!(g.mode, Mode::Explore));
        assert!(g.desk_paper().unwrap().sheet.unwrap().marked, "still on your desk");
    }

    #[test]
    fn the_bell_ends_a_lesson_you_dont_finish() {
        let mut g = new_game();
        let d = Date::new(2026, 4, 14);
        seated(&mut g, d, sim::school::PERIODS[0].0);
        g.update(1.0 / 60.0, &idle(), false);
        assert!(matches!(g.mode, Mode::Lesson(_)));
        // head down: doze through it
        g.key(Key::Back, false);
        g.update(1.0 / 60.0, &idle(), false);
        assert!(matches!(g.state.doing(), Some(Activity::Class { focus: Focus::Doze | Focus::Listen })));
        for _ in 0..(60 * 600) {
            g.update(1.0 / 60.0, &idle(), false);
            if let Mode::Lesson(v) = &g.mode {
                if v.result.is_some() {
                    break;
                }
            }
        }
        let Mode::Lesson(v) = &g.mode else { panic!("expected the lesson's result") };
        assert!(v.result.is_some());
        assert!(v.lesson.play.score() < 0.5, "dozed: {}", v.lesson.play.score());
        assert!(g.state.time.minute >= sim::school::PERIODS[0].1 || v.lesson.play.results.len() == v.lesson.play.tasks.len());
    }

    #[test]
    fn notes_and_every_subject_open() {
        for name in ["lesson-maths", "lesson-japanese", "lesson-english", "lesson-science", "lesson-social", "lesson-pe", "lesson-tech", "lesson-music", "lesson-art", "lesson-moral", "lesson-integrated", "lesson-class"] {
            let mut g = new_game();
            g.state.time = GameTime::new(Date::new(2026, 10, 5), 7 * 60);
            g.lesson_screen(name);
            let Mode::Lesson(v) = &g.mode else { panic!("{name}: no lesson") };
            let want = name.trim_start_matches("lesson-");
            assert!(v.lesson.info.subject.name().to_lowercase().replace(' ', "").starts_with(want), "{name}: {:?}", v.lesson.info.subject);
            g.key(Key::Binder, false);
            g.update(0.5, &idle(), false);
            assert!(matches!(g.state.doing(), Some(Activity::Class { focus: Focus::Notes })), "{name}: notes");
            let mut ui = Ui::new().unwrap();
            ui.begin(1280, 720);
            g.draw(&mut ui);
        }
    }

    #[test]
    fn the_year_ends_at_the_closing_ceremony() {
        let mut g = new_game();
        g.state.time = GameTime::new(Date::new(2027, 3, 24), sim::clock::at::hm(10, 29));
        g.state.location = Location::School;
        for _ in 0..120 {
            g.update(1.0 / 60.0, &idle(), true);
        }
        assert!(matches!(g.mode, Mode::YearEnd { page: 0 }));
        assert!(g.state.lessons.summary.as_ref().unwrap().at_ceremony);
        let mut ui = Ui::new().unwrap();
        ui.begin(1920, 1080);
        g.draw(&mut ui);
        g.key(Key::Right, false);
        assert!(matches!(g.mode, Mode::YearEnd { page: 1 }));
        g.draw(&mut ui);
        // time doesn't go on past the end
        let t = g.state.time;
        g.update(1.0, &idle(), true);
        assert_eq!(g.state.time, t);
    }
}
