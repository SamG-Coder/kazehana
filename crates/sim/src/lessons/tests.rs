use super::play::{Input, Play, Task};
use super::syllabus::*;
use super::*;
use crate::calendar::{events_on, is_school_day, Event};
use tcg::CardDb;

fn school_days() -> Vec<Date> {
    let mut d = START_DATE;
    let mut v = Vec::new();
    while d <= syllabus::closing_day(2026) {
        if is_school_day(d) {
            v.push(d);
        }
        d = d.add_days(1);
    }
    v
}

#[test]
fn the_terms_and_their_weeks() {
    let days = school_days();
    let mut most = [0u8; 3];
    for &d in &days {
        let (t, w) = term_week(d).unwrap_or_else(|| panic!("{d} has no term week"));
        most[t as usize - 1] = most[t as usize - 1].max(w);
    }
    assert_eq!(most, TERM_WEEKS, "weeks with lessons in each term");
    assert_eq!(term_week(Date::new(2026, 8, 10)), None, "summer break");
    assert_eq!(term_week(START_DATE), Some((1, 1)));
    assert_eq!(term_week(Date::new(2027, 1, 8)), Some((3, 1)));
    for n in 1..=YEAR_WEEKS {
        let (t, w) = from_year_week(n);
        assert_eq!(year_week(t, w), n);
    }
}

#[test]
fn every_subject_has_a_topic_every_school_week() {
    for &s in &SUBJECTS {
        // the units tile each term with no gaps or overlaps
        for t in 1..=3u8 {
            let mut covered = vec![0; TERM_WEEKS[t as usize - 1] as usize + 1];
            for u in units(s).iter().filter(|u| u.term == t) {
                for w in u.from..=u.to() {
                    assert!((w as usize) < covered.len(), "{s:?} term {t}: {} runs past the term", u.title);
                    covered[w as usize] += 1;
                }
            }
            assert!(covered[1..].iter().all(|&c| c == 1), "{s:?} term {t}: weeks covered {:?}", &covered[1..]);
        }
        assert_eq!(year_of(s).len(), YEAR_WEEKS);
        for &d in &school_days() {
            assert!(week_on(s, d).is_some(), "{s:?} on {d}");
        }
    }
}

#[test]
fn the_tests_match_the_calendar_and_the_review_weeks() {
    for e in Exam::ALL {
        let start = e.start(2026);
        assert!(events_on(start).iter().any(|x| matches!(x, Event::Exams(_))), "{e:?} on {start}");
        for k in 0..3 {
            assert!(is_school_day(start.add_days(k)), "{e:?}: day {k} is a school day");
        }
        // the core subjects revise in the test week
        for s in [Subject::Japanese, Subject::Maths, Subject::English, Subject::Science, Subject::Social] {
            let w = week_on(s, start).unwrap();
            assert!(w.review(), "{s:?} in the week of the {}: {}", e.name(), w.topic);
            assert!(!e.covers(s, 2026).is_empty(), "{e:?} covers some {s:?}");
            assert!(e.covers(s, 2026).iter().all(|w| w.term == e.term() && w.week < e.week(2026)));
        }
        let n: usize = e.days().iter().map(|d| d.len()).sum();
        assert_eq!(n, if e.is_mid() { 5 } else { 9 });
    }
    assert_eq!(exam_in(Exam::Mid1.start(2026), 1), Some((Exam::Mid1, Subject::Japanese)));
    assert_eq!(exam_in(Exam::Final2.start(2026).add_days(2), 2), Some((Exam::Final2, Subject::Pe)));
    assert_eq!(exam_in(Exam::Mid1.start(2026), 3), None, "two tests that day");
}

fn check(t: &Task, s: Subject, key: &str) {
    let why = format!("{s:?} {key}: {t:?}");
    assert!(!t.prompt().is_empty(), "{why}");
    match t {
        Task::Choice { options, answer, .. } => {
            assert!((2..=4).contains(&options.len()) && *answer < options.len(), "{why}");
            let mut o = options.clone();
            o.sort();
            o.dedup();
            assert_eq!(o.len(), options.len(), "duplicate options: {why}");
        }
        Task::Order { pieces, shown, .. } => {
            assert!(pieces.len() >= 2 && shown.len() == pieces.len(), "{why}");
            assert!(shown.iter().enumerate().any(|(k, &i)| k != i), "dealt in order already: {why}");
        }
        Task::Number { answer, .. } => assert!(answer.abs() < 100_000, "{why}"),
        Task::Timing { zone, tries, .. } => assert!(zone.0 >= 0.0 && zone.1 <= 1.0 && zone.0 < zone.1 && *tries > 0, "{why}"),
        Task::Recall { seq, .. } => assert!(seq.len() >= 3 && seq.iter().all(|&d| d < 4), "{why}"),
        Task::Slider { target, tolerance, .. } => assert!((0.0..=1.0).contains(target) && *tolerance >= 0.08, "{why}"),
        Task::Reflect { options, .. } => assert!(options.len() >= 2, "{why}"),
        Task::Sort { .. } => {}
    }
}

#[test]
fn every_week_makes_good_tasks() {
    for &s in &SUBJECTS {
        for w in year_of(s) {
            assert!(w.review() || !content::falls_back(s, w.key), "{s:?}: '{}' ({}) has no content of its own", w.key, w.topic);
            for seed in 0..12u64 {
                let mut rng = Rng::new(seed * 977 + w.year_week() as u64);
                let ts = content::tasks(&w, tasks_per_lesson(s, false), &mut rng);
                assert_eq!(ts.len(), tasks_per_lesson(s, false), "{s:?} {}", w.key);
                for t in &ts {
                    check(t, s, w.key);
                }
            }
        }
    }
}

#[test]
fn the_full_games_are_varied() {
    // maths, Japanese and English each use several mechanics over the year
    for s in [Subject::Maths, Subject::Japanese, Subject::English] {
        let mut kinds = std::collections::BTreeSet::new();
        for w in year_of(s) {
            let mut rng = Rng::new(w.year_week() as u64);
            for t in content::tasks(&w, 8, &mut rng) {
                kinds.insert(t.kind());
            }
        }
        assert!(kinds.len() >= 3, "{s:?}: {kinds:?}");
    }
    // and the other subjects their own
    let kind_of = |s: Subject| {
        let mut k = std::collections::BTreeSet::new();
        for w in year_of(s) {
            for t in content::tasks(&w, 6, &mut Rng::new(3)) {
                k.insert(t.kind());
            }
        }
        k
    };
    assert!(kind_of(Subject::Pe).contains("timing"));
    assert!(kind_of(Subject::Music).contains("recall"));
    assert!(kind_of(Subject::Art).contains("slider"));
    assert!(kind_of(Subject::Moral).iter().all(|k| *k == "reflect"));
    assert!(kind_of(Subject::Science).contains("sort"));
    assert!(kind_of(Subject::Social).contains("order"));
}

#[test]
fn perfect_play_scores_full_marks() {
    for &s in &[Subject::Maths, Subject::Japanese, Subject::English, Subject::Science, Subject::Social, Subject::Art, Subject::TechHome, Subject::Moral] {
        for w in year_of(s).into_iter().step_by(3) {
            let mut p = Play::new(content::tasks(&w, 6, &mut Rng::new(5)), Condition::default());
            let mut guard = 0;
            while !p.finished && guard < 10_000 {
                guard += 1;
                match p.solve_inputs() {
                    Some(ks) => {
                        for k in ks {
                            p.input(k);
                        }
                    }
                    None => p.tick(0.05),
                }
            }
            assert!(p.finished, "{s:?} {}", w.key);
            assert!((p.score() - 1.0).abs() < 1e-4, "{s:?} {}: {:?}", w.key, p.results);
        }
    }
    // and getting everything wrong (or nothing) scores none
    let w = week_of(Subject::Maths, 1, 3).unwrap();
    let mut p = Play::new(content::tasks(&w, 4, &mut Rng::new(1)), Condition::default());
    while !p.finished {
        p.input(Input::Confirm); // 0 is never the answer to "a + b" with both non-zero... or the feedback
        p.tick(0.5);
    }
    assert!(p.score() < 1.0);
}

fn game() -> GameState {
    let db = CardDb::generate(2);
    GameState::new(3, &db)
}

/// Sit in the classroom at a given time on a given day.
fn seat(g: &mut GameState, d: Date, minute: u32) {
    g.time = GameTime::new(d, minute);
    g.location = Location::School;
    g.life.doing = Some(crate::life::Doing { activity: Activity::Class { focus: Focus::Listen }, minutes: 0 });
}

#[test]
fn a_lesson_at_the_bell_and_its_mark() {
    let mut g = game();
    let d = Date::new(2026, 4, 13); // Monday of week 2
    seat(&mut g, d, PERIODS[1].0);
    assert_eq!(g.lesson_due(), Some(2), "2nd period, seated");
    let mut l = g.begin_lesson().unwrap();
    assert_eq!(l.info.subject, school::my_lesson(0, 2).unwrap().subject);
    assert_eq!(g.lesson_due(), None, "one go each period");
    assert!(l.pace > 0.1 && l.pace < 1.0, "the period lasts a few minutes of play: {}", l.pace);
    let before = g.lessons.mastery(l.info.subject, 2);
    let mut rng = Rng::new(9);
    while !l.play.finished {
        l.play.auto(1.0, &mut rng);
    }
    let line = g.finish_lesson(&l);
    assert!(line.contains("of"), "{line}");
    assert!(g.lessons.mastery(l.info.subject, 2) > before + 0.3);
    // not seated: nothing to play
    g.life.doing = None;
    g.time.minute = PERIODS[2].0;
    assert_eq!(g.lesson_due(), None);
    // the entrance ceremony: no lessons
    seat(&mut g, START_DATE, PERIODS[0].0);
    assert_eq!(g.lesson_due(), None);
}

#[test]
fn tests_are_marked_from_play_and_what_you_learned() {
    let mut g = game();
    let d = Exam::Mid1.start(2026);
    seat(&mut g, d, PERIODS[0].0 + 1);
    let mut l = g.begin_lesson().unwrap();
    assert_eq!(l.info.exam, Some(Exam::Mid1));
    assert_eq!(l.play.tasks.len(), 10);
    let mut rng = Rng::new(1);
    while !l.play.finished {
        l.play.auto(1.0, &mut rng);
    }
    g.finish_lesson(&l);
    let m = g.lessons.test(Exam::Mid1, Subject::Japanese).unwrap().mark;
    assert_eq!(m, 60, "a perfect paper with nothing learned yet: 60");
    // knowing the weeks well lifts it
    for w in Exam::Mid1.covers(Subject::Japanese, 2026) {
        g.lessons.learn(Subject::Japanese, w.year_week(), 1.0, 1.0);
        g.lessons.learn(Subject::Japanese, w.year_week(), 1.0, 1.0);
    }
    g.lessons.played[0] = false;
    let mut l = g.begin_lesson().unwrap();
    while !l.play.finished {
        l.play.auto(1.0, &mut rng);
    }
    g.finish_lesson(&l);
    assert!(g.lessons.test(Exam::Mid1, Subject::Japanese).unwrap().mark >= 90);
}

#[test]
fn missing_lessons_and_tests_costs_you() {
    let mut g = game();
    let d = Exam::Final1.start(2026);
    g.time = GameTime::new(d, 7 * 60);
    g.location = crate::state::Location::Home;
    let db = CardDb::generate(2);
    g.advance(crate::clock::at::LUNCH + 1 - g.time.minute, &db);
    let t = g.lessons.test(Exam::Final1, Subject::Japanese).expect("marked absent");
    assert!(!t.sat && t.mark == 0);
    let s = g.lessons.subject(school::my_lesson(1, 4).unwrap().subject).unwrap();
    assert_eq!(s.terms[0].missed, 1, "4th period missed");
}

#[test]
fn report_cards_grade_the_term() {
    let mut g = game();
    g.lessons.ensure();
    // a strong term in maths, a poor one in English
    let mut add = |s: Subject, lessons: u32, missed: u32, perf: f32, mark: u32| {
        let r = g.lessons.subject_mut(s);
        r.terms[0] = TermStats { lessons, missed, played: lessons, perf };
        for e in [Exam::Mid1, Exam::Final1] {
            g.lessons.tests.push(TestMark { exam: e, subject: s, mark, sat: true, date: e.start(2026) });
        }
    };
    add(Subject::Maths, 60, 0, 54.0, 92);
    add(Subject::English, 30, 30, 9.0, 20);
    g.time = GameTime::new(Date::new(2026, 7, 17), 15 * 60 + 50);
    g.issue_report(1);
    let r = &g.lessons.reports[0];
    assert_eq!(r.grade(Subject::Maths), Some(5));
    assert!(r.grade(Subject::English).unwrap() <= 2);
    assert_eq!(r.grade(Subject::Moral), None);
    assert!(last_school_day_of_term(Date::new(2026, 7, 17)), "Marine Day is the 20th; the break starts the 21st");
    assert!(last_school_day_of_term(Date::new(2026, 12, 25)));
    assert!(!last_school_day_of_term(Date::new(2026, 12, 24)));
}

#[test]
fn the_year_ends_at_the_closing_ceremony() {
    let mut g = game();
    assert!(!g.year_over());
    assert_eq!(g.year_end_time().date, Date::new(2027, 3, 24));
    g.time = GameTime::new(Date::new(2027, 3, 24), crate::clock::at::hm(10, 29));
    assert!(!g.year_over());
    g.time.minute += 1;
    assert!(g.year_over());
    g.location = Location::School;
    let s = g.close_year();
    assert!(s.at_ceremony);
    assert_eq!(s.grades.len(), 12);
    assert_eq!(g.lessons.reports.len(), 1, "the 3rd term's card");
    assert_eq!(g.close_year(), s, "once");
}

/// The syllabus as Markdown tables (docs/lessons_plan.md is made from this):
/// `cargo test --release -p sim --lib syllabus_markdown -- --ignored --nocapture`
#[test]
#[ignore]
fn syllabus_markdown() {
    let mut out = String::new();
    for &s in &SUBJECTS {
        out += &format!("\n### {}\n\n| Term | Weeks | Unit | Topics, week by week |\n|---|---|---|---|\n", capital(s.name()));
        for u in units(s) {
            let topics: Vec<&str> = u.weeks.iter().map(|w| w.0).collect();
            out += &format!("| {} | {}–{} | {} | {} |\n", u.term, u.from, u.to(), u.title, topics.join(" · "));
        }
    }
    println!("{out}");
}

#[test]
fn kokugo_has_japanese_titles_for_every_week() {
    assert_eq!(JAPANESE_JA.len(), JAPANESE.len());
    for (u, (_, topics)) in JAPANESE.iter().zip(JAPANESE_JA) {
        assert_eq!(u.weeks.len(), topics.len(), "{}", u.title);
    }
    for w in year_of(Subject::Japanese) {
        let (unit, topic) = w.titles_local();
        assert!(unit.chars().all(|c| !c.is_ascii_alphabetic()) && topic.chars().all(|c| !c.is_ascii_alphabetic()), "{unit} / {topic}");
    }
}
