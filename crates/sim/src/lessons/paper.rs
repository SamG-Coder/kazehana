//! Your work on paper (docs/lessons_plan.md): what the `pages` crate draws onto the props on
//! your desk.
//!
//! - **The worksheet** (purinto): handed out at the start of a lesson, the week's questions
//!   printed on it; you write your answers on it as you go; at the end it's collected, marked in
//!   red and handed back. You keep the marked ones.
//! - **The subject notebook**: what the teacher writes on the board (the week's topic, the point
//!   of it, a few worked examples), copied down in your hand if you take notes. You can read it
//!   back at home to study.

use super::content;
use super::play::Task;
use super::syllabus::Week;
use super::{Exam, Lesson};
use crate::calendar::Date;
use crate::school::Subject;
use serde::{Deserialize, Serialize};
use tcg::Rng;

/// A subject as it's written on a Japanese school page.
pub fn subject_jp(s: Subject) -> &'static str {
    match s {
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

/// Whether a subject's pages are written vertically (tategaki), right to left.
pub fn vertical(s: Subject) -> bool {
    s == Subject::Japanese
}

/// One question on a worksheet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SheetItem {
    pub prompt: String,
    /// "choice", "sort", "number", "order", "reflect" (the paper's layout for it)
    pub kind: String,
    /// the printed choices (ア, イ, ウ, エ), or the sort's two headings
    pub options: Vec<String>,
    /// the unit after a number's answer box
    pub unit: String,
    /// what you wrote (none: still blank)
    pub given: Option<String>,
    /// how the teacher marked it (once marked)
    pub right: Option<bool>,
    /// the answer, written in red beside a wrong one
    pub solution: String,
    /// what you're writing now (not yet your answer): drawn faint
    #[serde(default)]
    pub drafting: bool,
}

/// A worksheet: a lesson's questions and your answers on them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub subject: Subject,
    pub unit: String,
    pub topic: String,
    pub date: Date,
    pub teacher: String,
    /// a test paper, not a lesson's print
    pub exam: Option<Exam>,
    pub items: Vec<SheetItem>,
    /// marked and handed back
    pub marked: bool,
    /// a test's mark out of 100 (written at the top)
    pub mark: Option<u32>,
    /// the question you're on (a pencil arrow by it)
    #[serde(default)]
    pub current: Option<usize>,
}

impl Sheet {
    /// The print for a lesson as it stands: answered so far, marked if `marked`. Practical tasks
    /// (timing, recall, mixing a colour) aren't on paper.
    pub fn from_lesson(l: &Lesson, marked: bool) -> Sheet {
        let p = &l.play;
        let items = p
            .tasks
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let (kind, options, unit) = match t {
                    Task::Choice { options, .. } => ("choice", options.clone(), String::new()),
                    Task::Sort { bins, .. } => ("sort", bins.to_vec(), String::new()),
                    Task::Number { unit, .. } => ("number", Vec::new(), unit.clone()),
                    Task::Order { pieces, shown, .. } => ("order", shown.iter().map(|&k| pieces[k].clone()).collect(), String::new()),
                    Task::Reflect { options, .. } => ("reflect", options.iter().map(|o| o.0.clone()).collect(), String::new()),
                    Task::Timing { .. } | Task::Recall { .. } | Task::Slider { .. } => return None,
                };
                let prompt = match t {
                    // (a sort's prompt carries its item after the colon: the item is the question)
                    Task::Sort { prompt, item, .. } if vertical(l.info.subject) => format!("{item}　{}", prompt.split(':').next().unwrap_or(prompt)),
                    Task::Sort { prompt, item, .. } => format!("{}: {item}", prompt.split(':').next().unwrap_or(prompt)),
                    t => t.prompt().to_string(),
                };
                let done = i < p.results.len();
                Some(SheetItem {
                    prompt,
                    kind: kind.into(),
                    options,
                    unit,
                    given: if done { p.given.get(i).cloned().flatten() } else { None },
                    right: (marked && kind != "reflect").then(|| done && p.results[i] >= 0.999),
                    solution: t.answer_text(),
                    drafting: false,
                })
            })
            .collect();
        let (unit, topic) = l.info.week.titles_local();
        Sheet { subject: l.info.subject, unit: unit.to_string(), topic: topic.to_string(), date: l.info.date, teacher: l.info.teacher.clone(), exam: l.info.exam, items, marked, mark: None, current: None }
    }

    /// The print as you're working on it: answered so far, the question you're on marked, and
    /// what you're writing for it (`draft`, if anything) in faint pencil.
    pub fn live(l: &Lesson, draft: Option<String>) -> Sheet {
        let mut sh = Sheet::from_lesson(l, false);
        let p = &l.play;
        if !p.finished && p.feedback.is_none() {
            // (which sheet item is the current task: the paper ones before it)
            let paper = |t: &Task| !matches!(t, Task::Timing { .. } | Task::Recall { .. } | Task::Slider { .. });
            if p.tasks.get(p.i).is_some_and(paper) {
                let k = p.tasks[..p.i].iter().filter(|t| paper(t)).count();
                sh.current = Some(k);
                if let (Some(d), Some(it)) = (draft, sh.items.get_mut(k)) {
                    it.given = Some(d);
                    it.drafting = true;
                }
            }
        }
        sh
    }

    /// (right, out of) once marked
    pub fn score(&self) -> (usize, usize) {
        let marked: Vec<&SheetItem> = self.items.iter().filter(|i| i.right.is_some()).collect();
        (marked.iter().filter(|i| i.right == Some(true)).count(), marked.len())
    }
}

/// What a line in your notebook is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteKind {
    /// the lesson's title, boxed
    Title,
    /// the point of it (red pen)
    Point,
    /// an example worked on the board
    Example,
    /// its answer
    Answer,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoteLine {
    pub kind: NoteKind,
    pub text: String,
}

/// A lesson's notes in a subject notebook.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Notes {
    pub subject: Subject,
    pub date: Date,
    pub lines: Vec<NoteLine>,
}

/// What the teacher writes on the board in a week's lesson (and you copy down): the topic, its
/// point, and a few examples worked through. The same each time for a lesson (seeded by it).
pub fn board(w: &Week, date: Date, seed: u64) -> Vec<NoteLine> {
    let mut rng = Rng::derive(seed ^ date.to_days() as u64, w.key);
    let (unit, topic) = w.titles_local();
    let mut lines = vec![NoteLine { kind: NoteKind::Title, text: topic.to_string() }];
    lines.push(NoteLine { kind: NoteKind::Point, text: if vertical(w.subject) { unit.to_string() } else { format!("{} · {unit}", subject_jp(w.subject)) } });
    for t in content::tasks(w, 3, &mut rng) {
        match &t {
            Task::Timing { .. } | Task::Recall { .. } | Task::Slider { .. } => continue,
            Task::Reflect { prompt, .. } => {
                lines.push(NoteLine { kind: NoteKind::Example, text: prompt.clone() });
                continue;
            }
            Task::Sort { prompt, item, .. } => {
                let head = prompt.split(':').next().unwrap_or(prompt);
                let text = if vertical(w.subject) { format!("{item}　{head}") } else { format!("{head}: {item}") };
                lines.push(NoteLine { kind: NoteKind::Example, text })
            }
            t => lines.push(NoteLine { kind: NoteKind::Example, text: t.prompt().to_string() }),
        }
        lines.push(NoteLine { kind: NoteKind::Answer, text: t.answer_text() });
    }
    lines
}
