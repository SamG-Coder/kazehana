//! A lesson as a mini-game: a run of short tasks played with the d-pad (or arrows), confirm and
//! back. Pure logic, no drawing: the game crate shows it (`game::lesson`), and tests and the year's
//! sim test play it by its inputs or by `auto`.
//!
//! The kinds of task, each its own mechanic:
//! - **Choice**: up to four answers on the four directions; press one. `timed` choices fall like
//!   rain (kanji readings, old kana): answer before they land.
//! - **Sort**: one thing at a time into the left or the right bin.
//! - **Number**: dial in a number (up/down ±1, right/left ±10), confirm.
//! - **Order**: put pieces in order (left/right to choose, confirm to place, down to undo).
//! - **Timing**: a marker sweeps across; confirm while it's in the zone (PE).
//! - **Recall**: watch a phrase of notes on the four directions, then play it back (music).
//! - **Slider**: turn a colour to match (left/right), confirm (art).
//! - **Reflect**: no wrong answer: choose what you'd do, and why (moral education, class
//!   activities).

use serde::{Deserialize, Serialize};
use tcg::Rng;

/// A button press in a lesson.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Input {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
}

/// What an answer in a moral-education or class discussion leans to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Virtue {
    Kindness,
    Honesty,
    Courage,
    Fairness,
    Responsibility,
    Respect,
}

/// What the slider turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SliderKind {
    /// a hue, 0..1 round the colour wheel
    Hue,
    /// lightness, 0 (black) .. 1 (white)
    Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Task {
    Choice { prompt: String, options: Vec<String>, answer: usize, timed: bool },
    Sort { prompt: String, bins: [String; 2], item: String, right: bool },
    Number { prompt: String, answer: i64, decimals: u8, unit: String },
    /// `pieces` in the right order; `shown` the order they're dealt in (indices into `pieces`)
    Order { prompt: String, pieces: Vec<String>, shown: Vec<usize> },
    /// `speed`: sweeps per second; `zone`: (start, end) along 0..1; `tries`: how many goes
    Timing { prompt: String, speed: f32, zone: (f32, f32), tries: u8 },
    Recall { prompt: String, labels: [String; 4], seq: Vec<u8> },
    Slider { prompt: String, kind: SliderKind, target: f32, tolerance: f32 },
    Reflect { prompt: String, options: Vec<(String, Virtue)> },
}

impl Task {
    pub fn prompt(&self) -> &str {
        match self {
            Task::Choice { prompt, .. }
            | Task::Sort { prompt, .. }
            | Task::Number { prompt, .. }
            | Task::Order { prompt, .. }
            | Task::Timing { prompt, .. }
            | Task::Recall { prompt, .. }
            | Task::Slider { prompt, .. }
            | Task::Reflect { prompt, .. } => prompt,
        }
    }

    /// The kind, as a word (tests, the plan).
    pub fn kind(&self) -> &'static str {
        match self {
            Task::Choice { timed: true, .. } => "rain",
            Task::Choice { .. } => "choice",
            Task::Sort { .. } => "sort",
            Task::Number { .. } => "number",
            Task::Order { .. } => "order",
            Task::Timing { .. } => "timing",
            Task::Recall { .. } => "recall",
            Task::Slider { .. } => "slider",
            Task::Reflect { .. } => "reflect",
        }
    }

    /// Seconds allowed (before tiredness and hunger shorten it).
    pub fn limit(&self) -> f32 {
        match self {
            Task::Choice { timed: true, .. } => 7.0,
            Task::Choice { .. } => 14.0,
            Task::Sort { .. } => 7.0,
            Task::Number { .. } => 30.0,
            Task::Order { pieces, .. } => 8.0 + 4.0 * pieces.len() as f32,
            Task::Timing { speed, tries, .. } => 2.5 + *tries as f32 * (1.2 / speed.max(0.3) + 0.6),
            Task::Recall { seq, .. } => 4.0 + seq.len() as f32 * 2.2,
            Task::Slider { .. } => 18.0,
            Task::Reflect { .. } => 25.0,
        }
    }

    /// About how long a player takes (to pace the period's clock to the game).
    pub fn typical(&self) -> f32 {
        match self {
            Task::Choice { timed: true, .. } => 3.5,
            Task::Choice { .. } => 7.0,
            Task::Sort { .. } => 3.0,
            Task::Number { .. } => 12.0,
            Task::Order { pieces, .. } => 3.0 + 2.5 * pieces.len() as f32,
            Task::Recall { seq, .. } => 2.0 + seq.len() as f32 * 1.4,
            Task::Slider { .. } => 9.0,
            Task::Reflect { .. } => 12.0,
            t => t.limit() * 0.8,
        }
    }

    /// The right answer as you'd write it on paper.
    pub fn answer_text(&self) -> String {
        match self {
            Task::Number { answer, decimals, .. } => show_number(*answer, *decimals),
            Task::Sort { bins, right, .. } => bins[*right as usize].clone(),
            t => t.solution(),
        }
    }

    /// A plausible wrong answer (a bot's mistakes, on paper).
    pub fn wrong_guess(&self, rng: &mut Rng) -> Option<String> {
        Some(match self {
            Task::Choice { options, answer, .. } => {
                let w: Vec<&String> = options.iter().enumerate().filter(|(i, _)| i != answer).map(|x| x.1).collect();
                (*rng.pick(&w)).clone()
            }
            Task::Sort { bins, right, .. } => bins[!*right as usize].clone(),
            Task::Number { answer, decimals, .. } => {
                let off = *rng.pick(&[-2i64, -1, 1, 2, 10]);
                // (the classic slip: the sign)
                let v = if *answer != 0 && rng.chance(0.4) { -answer } else { answer + off };
                show_number(v, *decimals)
            }
            Task::Order { pieces, shown, .. } => shown.iter().map(|&i| pieces[i].as_str()).collect::<Vec<_>>().join(" → "),
            _ => return None,
        })
    }

    /// The right answer, said out loud (feedback).
    pub fn solution(&self) -> String {
        match self {
            Task::Choice { options, answer, .. } => options[*answer].clone(),
            Task::Sort { bins, right, .. } => bins[*right as usize].clone(),
            Task::Number { answer, decimals, unit, .. } => format!("{}{}{}", show_number(*answer, *decimals), if unit.is_empty() { "" } else { " " }, unit),
            Task::Order { pieces, .. } => pieces.join(" → "),
            Task::Recall { labels, seq, .. } => seq.iter().map(|&i| labels[i as usize].as_str()).collect::<Vec<_>>().join(" "),
            Task::Timing { .. } | Task::Slider { .. } | Task::Reflect { .. } => String::new(),
        }
    }
}

/// A number with `decimals` places (`answer` counts the smallest place: 27, 1 → "2.7").
pub fn show_number(v: i64, decimals: u8) -> String {
    if decimals == 0 {
        return v.to_string();
    }
    let p = 10i64.pow(decimals as u32);
    let sign = if v < 0 { "-" } else { "" };
    format!("{sign}{}.{:0w$}", v.abs() / p, v.abs() % p, w = decimals as usize)
}

/// Which direction answers option `i` of `n` (2: left/right; 3: left/up/right; 4: up/right/down/left).
pub fn choice_input(n: usize, i: usize) -> Input {
    match (n, i) {
        (2, 0) => Input::Left,
        (2, _) => Input::Right,
        (3, 0) => Input::Left,
        (3, 1) => Input::Up,
        (3, _) => Input::Right,
        (_, 0) => Input::Up,
        (_, 1) => Input::Right,
        (_, 2) => Input::Down,
        _ => Input::Left,
    }
}

fn choice_of(n: usize, k: Input) -> Option<usize> {
    (0..n.min(4)).find(|&i| choice_input(n, i) == k)
}

/// How you are when the bell goes: it shapes the lesson.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Condition {
    /// 0..=100
    pub energy: i32,
    /// 0..=100
    pub fullness: f32,
    /// taking notes (more sinks in; more tiring)
    pub notes: bool,
}

impl Default for Condition {
    fn default() -> Self {
        Condition { energy: 100, fullness: 60.0, notes: false }
    }
}

impl Condition {
    /// The time you get for each task: tired and hungry, it goes faster than you can think.
    pub fn time_mult(&self) -> f32 {
        let e = (self.energy.clamp(0, 100) as f32 / 100.0) * 0.45 + 0.55;
        let h = if self.fullness < 8.0 {
            0.6
        } else if self.fullness < 25.0 {
            0.8
        } else {
            1.0
        };
        e * h
    }
    /// How much of what you get right stays with you.
    pub fn learn_mult(&self) -> f32 {
        let h = if self.fullness < 8.0 {
            0.5
        } else if self.fullness < 25.0 {
            0.75
        } else {
            1.0
        };
        h * if self.notes { 1.2 } else { 1.0 }
    }
}

/// The current task's working state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cursor {
    /// selected option (Reflect) / piece among those left (Order)
    pub sel: usize,
    /// the dial (Number)
    pub value: i64,
    /// pieces placed so far (Order: indices into `pieces`)
    pub picked: Vec<usize>,
    /// where the sweeping marker is (Timing: 0..1), and which way it goes
    pub marker: f32,
    pub dir: f32,
    pub hits: u8,
    pub tries: u8,
    /// what you've played back (Recall), and how long the phrase has been shown
    pub played: Vec<u8>,
    pub shown_for: f32,
    /// the slider (Slider)
    pub slider: f32,
}

/// What you just got: shown for a moment before the next task.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Feedback {
    /// 0..1
    pub score: f32,
    pub text: String,
    pub t: f32,
}

pub const FEEDBACK_SECONDS: f32 = 1.3;
/// Seconds per note when a Recall phrase is shown.
pub const RECALL_NOTE: f32 = 0.6;

/// A lesson being played.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Play {
    pub tasks: Vec<Task>,
    pub i: usize,
    /// a score (0..1) for each task done
    pub results: Vec<f32>,
    /// what you put for each task done, as it's written on your paper (none: left blank)
    #[serde(default)]
    pub given: Vec<Option<String>>,
    pub cur: Cursor,
    /// seconds into this task, and the time allowed
    pub t: f32,
    pub limit: f32,
    pub feedback: Option<Feedback>,
    pub streak: u32,
    pub best_streak: u32,
    pub cond: Condition,
    /// head down on the desk (Back): the tasks go by without you
    pub dozing: bool,
    pub dozed: f32,
    /// what your answers in a discussion leaned to
    pub virtues: Vec<Virtue>,
    pub finished: bool,
}

impl Play {
    pub fn new(tasks: Vec<Task>, cond: Condition) -> Play {
        let mut p = Play { tasks, i: 0, results: Vec::new(), given: Vec::new(), cur: Cursor::default(), t: 0.0, limit: 0.0, feedback: None, streak: 0, best_streak: 0, cond, dozing: false, dozed: 0.0, virtues: Vec::new(), finished: false };
        p.enter();
        p
    }

    /// About how many seconds the whole lesson takes to play.
    pub fn expected_seconds(&self) -> f32 {
        self.tasks.iter().map(|t| t.typical() + FEEDBACK_SECONDS).sum()
    }

    pub fn task(&self) -> Option<&Task> {
        if self.finished {
            None
        } else {
            self.tasks.get(self.i)
        }
    }

    /// The share of the lesson you got (tasks not done count as nothing).
    pub fn score(&self) -> f32 {
        if self.tasks.is_empty() {
            return 0.0;
        }
        self.results.iter().sum::<f32>() / self.tasks.len() as f32
    }

    /// How many tasks you got right (half marks and up).
    pub fn right(&self) -> usize {
        self.results.iter().filter(|&&r| r >= 0.5).count()
    }

    fn enter(&mut self) {
        self.t = 0.0;
        self.cur = Cursor { dir: 1.0, slider: 0.5, ..Default::default() };
        if let Some(task) = self.tasks.get(self.i) {
            self.limit = task.limit() * self.cond.time_mult();
            if let Task::Timing { tries, .. } = task {
                // (a timing task gives you all your goes whatever the time)
                self.limit = self.limit.max(task.limit());
                self.cur.tries = 0;
                let _ = tries;
            }
            if let Task::Recall { seq, .. } = task {
                // (the showing doesn't count against you)
                self.limit += seq.len() as f32 * RECALL_NOTE;
            }
        } else {
            self.finished = true;
        }
    }

    /// Grade the current task and show how it went.
    fn grade(&mut self, score: f32, text: String, given: Option<String>) {
        let score = score.clamp(0.0, 1.0);
        self.results.push(score);
        self.given.push(given);
        if score >= 0.5 {
            self.streak += 1;
            self.best_streak = self.best_streak.max(self.streak);
        } else {
            self.streak = 0;
        }
        self.feedback = Some(Feedback { score, text, t: 0.0 });
    }

    fn next(&mut self) {
        self.feedback = None;
        self.i += 1;
        self.enter();
    }

    /// The bell: the lesson's over, whatever's left undone.
    pub fn end(&mut self) {
        self.finished = true;
        self.feedback = None;
    }

    /// Time passes (real seconds).
    pub fn tick(&mut self, dt: f32) {
        if self.finished {
            return;
        }
        if let Some(f) = &mut self.feedback {
            f.t += dt;
            if f.t >= FEEDBACK_SECONDS {
                self.next();
            }
            return;
        }
        if self.dozing {
            self.dozed += dt;
        }
        // (dozing, the lesson goes by three times as fast and you answer nothing)
        self.t += if self.dozing { dt * 3.0 } else { dt };
        let Some(task) = self.tasks.get(self.i).cloned() else {
            self.finished = true;
            return;
        };
        match &task {
            Task::Timing { speed, .. } if !self.dozing => {
                let c = &mut self.cur;
                c.marker += c.dir * speed * dt;
                if c.marker > 1.0 {
                    c.marker = 2.0 - c.marker;
                    c.dir = -1.0;
                } else if c.marker < 0.0 {
                    c.marker = -c.marker;
                    c.dir = 1.0;
                }
            }
            Task::Recall { .. } => self.cur.shown_for += dt,
            _ => {}
        }
        if self.t >= self.limit {
            let partial = match &task {
                Task::Timing { tries, .. } => self.cur.hits as f32 / *tries as f32,
                Task::Order { pieces, .. } => correct_prefix(&self.cur.picked) as f32 / pieces.len() as f32 * 0.5,
                Task::Recall { seq, .. } => self.cur.played.len() as f32 / seq.len() as f32 * 0.5,
                _ => 0.0,
            };
            let text = if self.dozing { "(You missed it.)".to_string() } else { format!("Time! {}", task.solution()) };
            let given = match &task {
                Task::Order { pieces, .. } if !self.cur.picked.is_empty() => Some(self.cur.picked.iter().map(|&i| pieces[i].as_str()).collect::<Vec<_>>().join(" → ")),
                _ => None,
            };
            self.grade(partial, text, given);
        }
    }

    /// Whether the Recall phrase is still being shown (and which note, if so).
    pub fn recall_showing(&self) -> Option<usize> {
        match self.task() {
            Some(Task::Recall { seq, .. }) => {
                let k = (self.cur.shown_for / RECALL_NOTE) as usize;
                (k < seq.len()).then_some(k)
            }
            _ => None,
        }
    }

    /// A button press.
    pub fn input(&mut self, k: Input) {
        if self.finished {
            return;
        }
        if k == Input::Back {
            self.dozing = !self.dozing;
            return;
        }
        if self.feedback.is_some() {
            // (confirm skips the feedback)
            if k == Input::Confirm {
                self.next();
            }
            return;
        }
        if self.dozing {
            return;
        }
        let Some(task) = self.tasks.get(self.i).cloned() else { return };
        match task {
            Task::Choice { options, answer, .. } => {
                if let Some(i) = choice_of(options.len(), k) {
                    let ok = i == answer;
                    self.grade(if ok { 1.0 } else { 0.0 }, if ok { "Right!".into() } else { format!("It's {}.", options[answer]) }, Some(options[i].clone()));
                }
            }
            Task::Sort { bins, right, .. } => {
                let pick = match k {
                    Input::Left => Some(false),
                    Input::Right => Some(true),
                    _ => None,
                };
                if let Some(p) = pick {
                    let ok = p == right;
                    self.grade(if ok { 1.0 } else { 0.0 }, if ok { "Right!".into() } else { format!("It goes in {}.", bins[right as usize]) }, Some(bins[p as usize].clone()));
                }
            }
            Task::Number { answer, decimals, unit, .. } => match k {
                Input::Up => self.cur.value += 1,
                Input::Down => self.cur.value -= 1,
                Input::Right => self.cur.value += 10,
                Input::Left => self.cur.value -= 10,
                Input::Confirm => {
                    let ok = self.cur.value == answer;
                    let sol = format!("{}{}{}", show_number(answer, decimals), if unit.is_empty() { "" } else { " " }, unit);
                    let mine = show_number(self.cur.value, decimals);
                    self.grade(if ok { 1.0 } else { 0.0 }, if ok { "Right!".into() } else { format!("It's {sol}.") }, Some(mine));
                }
                Input::Back => {}
            },
            Task::Order { pieces, shown, .. } => {
                let left: Vec<usize> = shown.iter().copied().filter(|i| !self.cur.picked.contains(i)).collect();
                match k {
                    Input::Left => self.cur.sel = (self.cur.sel + left.len().max(1) - 1) % left.len().max(1),
                    Input::Right | Input::Up => self.cur.sel = (self.cur.sel + 1) % left.len().max(1),
                    Input::Down => {
                        self.cur.picked.pop();
                    }
                    Input::Confirm => {
                        if let Some(&p) = left.get(self.cur.sel.min(left.len().saturating_sub(1))) {
                            self.cur.picked.push(p);
                            self.cur.sel = 0;
                        }
                        if self.cur.picked.len() == pieces.len() {
                            let n = correct_places(&self.cur.picked);
                            let s = n as f32 / pieces.len() as f32;
                            let mine = self.cur.picked.iter().map(|&i| pieces[i].as_str()).collect::<Vec<_>>().join(" → ");
                            self.grade(if s >= 0.999 { 1.0 } else { s * 0.6 }, if s >= 0.999 { "Right!".into() } else { format!("The order: {}", pieces.join(" → ")) }, Some(mine));
                        }
                    }
                    Input::Back => {}
                }
            }
            Task::Timing { zone, tries, .. } => {
                if k == Input::Confirm {
                    let w = zone.1 - zone.0;
                    let mid = (zone.0 + zone.1) * 0.5;
                    // (tired, the zone feels narrower: your timing's off)
                    let half = w * 0.5 * (0.6 + 0.4 * self.cond.time_mult());
                    if (self.cur.marker - mid).abs() <= half {
                        self.cur.hits += 1;
                    }
                    self.cur.tries += 1;
                    if self.cur.tries >= tries {
                        let s = self.cur.hits as f32 / tries as f32;
                        self.grade(s, format!("{} of {tries}", self.cur.hits), None);
                    }
                }
            }
            Task::Recall { seq, labels, .. } => {
                if self.recall_showing().is_some() {
                    return;
                }
                let d = match k {
                    Input::Up => Some(0u8),
                    Input::Right => Some(1),
                    Input::Down => Some(2),
                    Input::Left => Some(3),
                    _ => None,
                };
                if let Some(d) = d {
                    let n = self.cur.played.len();
                    if seq[n] == d {
                        self.cur.played.push(d);
                        if self.cur.played.len() == seq.len() {
                            self.grade(1.0, "Right!".into(), None);
                        }
                    } else {
                        let s = n as f32 / seq.len() as f32 * 0.5;
                        let sol = seq.iter().map(|&i| labels[i as usize].as_str()).collect::<Vec<_>>().join(" ");
                        self.grade(s, format!("It went {sol}."), None);
                    }
                }
            }
            Task::Slider { kind, target, tolerance, .. } => match k {
                Input::Left | Input::Down => self.cur.slider = step_slider(kind, self.cur.slider, -0.025),
                Input::Right | Input::Up => self.cur.slider = step_slider(kind, self.cur.slider, 0.025),
                Input::Confirm => {
                    let d = slider_distance(kind, self.cur.slider, target);
                    let s = (1.0 - d / tolerance).clamp(0.0, 1.0);
                    let s = if d <= tolerance * 0.35 { 1.0 } else { s };
                    self.grade(s, if s >= 0.999 { "Spot on!".into() } else if s >= 0.5 { "Close.".into() } else { "Not quite.".into() }, None);
                }
                Input::Back => {}
            },
            Task::Reflect { options, .. } => match k {
                Input::Up | Input::Left => self.cur.sel = (self.cur.sel + options.len() - 1) % options.len(),
                Input::Down | Input::Right => self.cur.sel = (self.cur.sel + 1) % options.len(),
                Input::Confirm => {
                    self.virtues.push(options[self.cur.sel].1);
                    self.grade(1.0, "You say what you think.".into(), Some(options[self.cur.sel].0.clone()));
                }
                Input::Back => {}
            },
        }
    }

    /// The presses that answer the current task right (tests; `None` when it takes timing:
    /// Timing, or a Recall phrase still being shown).
    pub fn solve_inputs(&self) -> Option<Vec<Input>> {
        if self.feedback.is_some() {
            return Some(vec![Input::Confirm]);
        }
        let task = self.task()?;
        Some(match task {
            Task::Choice { options, answer, .. } => vec![choice_input(options.len(), *answer)],
            Task::Sort { right, .. } => vec![if *right { Input::Right } else { Input::Left }],
            Task::Number { answer, .. } => {
                let mut v = Vec::new();
                let d = answer - self.cur.value;
                let (tens, ones) = (d / 10, d % 10);
                for _ in 0..tens.abs() {
                    v.push(if tens > 0 { Input::Right } else { Input::Left });
                }
                for _ in 0..ones.abs() {
                    v.push(if ones > 0 { Input::Up } else { Input::Down });
                }
                v.push(Input::Confirm);
                v
            }
            Task::Order { pieces, shown, .. } => {
                let mut v = Vec::new();
                let mut picked = self.cur.picked.clone();
                for want in 0..pieces.len() {
                    if picked.contains(&want) {
                        continue;
                    }
                    let left: Vec<usize> = shown.iter().copied().filter(|i| !picked.contains(i)).collect();
                    let at = left.iter().position(|&i| i == want).unwrap();
                    for _ in 0..at {
                        v.push(Input::Right);
                    }
                    v.push(Input::Confirm);
                    picked.push(want);
                }
                v
            }
            Task::Recall { seq, .. } => {
                self.recall_showing().is_none().then_some(())?;
                seq[self.cur.played.len()..].iter().map(|&d| [Input::Up, Input::Right, Input::Down, Input::Left][d as usize]).collect()
            }
            Task::Slider { kind, target, .. } => {
                let mut v = Vec::new();
                let mut s = self.cur.slider;
                for _ in 0..80 {
                    if slider_distance(*kind, s, *target) <= 0.0126 {
                        break;
                    }
                    let up = step_slider(*kind, s, 0.025);
                    let down = step_slider(*kind, s, -0.025);
                    if slider_distance(*kind, up, *target) < slider_distance(*kind, down, *target) {
                        v.push(Input::Right);
                        s = up;
                    } else {
                        v.push(Input::Left);
                        s = down;
                    }
                }
                v.push(Input::Confirm);
                v
            }
            Task::Reflect { .. } => vec![Input::Confirm],
            Task::Timing { .. } => return None,
        })
    }

    /// Play the current task without the buttons: right with probability `skill` (the year's
    /// sim test, and lessons you sit through without playing).
    pub fn auto(&mut self, skill: f32, rng: &mut Rng) {
        if self.finished {
            return;
        }
        if self.feedback.is_some() {
            self.next();
            return;
        }
        let Some(task) = self.task().cloned() else { return };
        match task {
            Task::Reflect { options, .. } => {
                let i = rng.below(options.len());
                self.virtues.push(options[i].1);
                self.grade(1.0, String::new(), Some(options[i].0.clone()));
            }
            Task::Timing { tries, .. } => {
                let hits = (0..tries).filter(|_| rng.chance(skill)).count();
                self.grade(hits as f32 / tries as f32, String::new(), None);
            }
            t => {
                let ok = rng.chance(skill);
                let given = if ok { Some(t.answer_text()) } else { t.wrong_guess(rng) };
                self.grade(if ok { 1.0 } else { 0.0 }, String::new(), given);
            }
        }
        self.next();
    }
}

fn correct_prefix(picked: &[usize]) -> usize {
    picked.iter().enumerate().take_while(|(k, &p)| *k == p).count()
}

fn correct_places(picked: &[usize]) -> usize {
    picked.iter().enumerate().filter(|(k, &p)| *k == p).count()
}

fn step_slider(kind: SliderKind, v: f32, d: f32) -> f32 {
    match kind {
        SliderKind::Hue => (v + d).rem_euclid(1.0),
        SliderKind::Value => (v + d).clamp(0.0, 1.0),
    }
}

fn slider_distance(kind: SliderKind, a: f32, b: f32) -> f32 {
    match kind {
        SliderKind::Hue => {
            let d = (a - b).rem_euclid(1.0);
            d.min(1.0 - d)
        }
        SliderKind::Value => (a - b).abs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(tasks: Vec<Task>) -> Play {
        Play::new(tasks, Condition::default())
    }

    #[test]
    fn number_dial_and_order() {
        let mut p = play(vec![
            Task::Number { prompt: "(-3) × 4".into(), answer: -12, decimals: 0, unit: String::new() },
            Task::Order { prompt: "order".into(), pieces: vec!["a".into(), "b".into(), "c".into()], shown: vec![2, 0, 1] },
        ]);
        for k in p.solve_inputs().unwrap() {
            p.input(k);
        }
        assert_eq!(p.results, vec![1.0]);
        p.input(Input::Confirm); // skip the feedback
        for k in p.solve_inputs().unwrap() {
            p.input(k);
        }
        assert_eq!(p.results, vec![1.0, 1.0]);
        p.tick(FEEDBACK_SECONDS + 0.1);
        assert!(p.finished && p.score() == 1.0);
    }

    #[test]
    fn wrong_answers_timeouts_and_dozing() {
        let mut p = play(vec![
            Task::Choice { prompt: "?".into(), options: vec!["a".into(), "b".into(), "c".into(), "d".into()], answer: 2, timed: true },
            Task::Sort { prompt: "?".into(), bins: ["l".into(), "r".into()], item: "x".into(), right: true },
            Task::Number { prompt: "?".into(), answer: 5, decimals: 0, unit: String::new() },
        ]);
        p.input(Input::Up); // option 0: wrong
        assert_eq!(p.results, vec![0.0]);
        p.tick(2.0);
        // let the sort run out
        p.tick(10.0);
        assert_eq!(p.results.len(), 2);
        p.tick(2.0);
        // head down: the number goes by
        p.input(Input::Back);
        assert!(p.dozing);
        for _ in 0..40 {
            p.tick(0.5);
        }
        assert!(p.finished && p.score() == 0.0 && p.dozed > 0.0);
    }

    #[test]
    fn timing_recall_slider_reflect() {
        let mut p = play(vec![
            Task::Timing { prompt: "jump".into(), speed: 1.0, zone: (0.4, 0.6), tries: 2 },
            Task::Recall { prompt: "play".into(), labels: ["do".into(), "re".into(), "mi".into(), "so".into()], seq: vec![0, 2, 1] },
            Task::Slider { prompt: "mix".into(), kind: SliderKind::Hue, target: 0.9, tolerance: 0.1 },
            Task::Reflect { prompt: "what would you do?".into(), options: vec![("help".into(), Virtue::Kindness), ("tell".into(), Virtue::Honesty)] },
        ]);
        // wait for the marker to sweep into the middle, twice
        let mut presses = 0;
        while presses < 2 {
            p.tick(1.0 / 120.0);
            if (p.cur.marker - 0.5).abs() < 0.02 {
                p.input(Input::Confirm);
                presses += 1;
                p.tick(0.1);
            }
        }
        assert_eq!(p.results, vec![1.0]);
        p.input(Input::Confirm);
        assert_eq!(p.solve_inputs(), None, "the phrase is being shown");
        p.tick(RECALL_NOTE * 3.0 + 0.05);
        for k in p.solve_inputs().unwrap() {
            p.input(k);
        }
        p.input(Input::Confirm);
        for k in p.solve_inputs().unwrap() {
            p.input(k);
        }
        p.input(Input::Confirm);
        p.input(Input::Down);
        p.input(Input::Confirm);
        assert_eq!(p.results, vec![1.0, 1.0, 1.0, 1.0]);
        assert_eq!(p.virtues, vec![Virtue::Honesty]);
    }

    #[test]
    fn tired_and_hungry_means_less_time() {
        let rested = Condition { energy: 100, fullness: 70.0, notes: false };
        let wrecked = Condition { energy: 10, fullness: 5.0, notes: false };
        assert!(wrecked.time_mult() < rested.time_mult() * 0.5);
        assert!(wrecked.learn_mult() < rested.learn_mult());
        assert_eq!(show_number(27, 1), "2.7");
        assert_eq!(show_number(-5, 1), "-0.5");
    }
}
