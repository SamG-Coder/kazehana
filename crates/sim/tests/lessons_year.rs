//! The whole school year played out, lesson by lesson, to the closing ceremony: a student who
//! goes every day and plays every lesson, and one who never goes.

use sim::clock::at;
use sim::lessons::{Exam, SUBJECTS};
use sim::life::{Activity, Focus};
use sim::{GameState, Location};
use tcg::{CardDb, Rng};

fn until(g: &mut GameState, db: &CardDb, h: u32, m: u32) {
    let t = g.time.until(at::hm(h, m));
    g.advance(t, db);
}

/// Sit through the day's lessons from now till `end`, playing each one at the bell with `skill`.
fn lessons_until(g: &mut GameState, db: &CardDb, end: u32, skill: f32, rng: &mut Rng, played: &mut u32) {
    while g.time.minute < end && !g.year_over() {
        if let Some(mut l) = g.begin_lesson() {
            while !l.play.finished {
                l.play.auto(skill, rng);
            }
            g.finish_lesson(&l);
            *played += 1;
            // (in the game the period's clock runs while it's played, to the bell)
            let bell = sim::school::PERIODS[l.info.period as usize - 1].1;
            g.advance(bell.min(end).saturating_sub(g.time.minute).max(1), db);
        } else {
            g.advance(1, db);
        }
    }
}

fn good_year(seed: u64, skill: f32) -> (GameState, u32) {
    let db = CardDb::generate(3);
    let mut g = GameState::new(seed, &db);
    let mut rng = Rng::new(seed);
    let mut played = 0;
    while !g.year_over() {
        g.location = Location::Home;
        if g.time.school_day() {
            until(&mut g, &db, 7, 0);
            let _ = g.eat_with_kenji();
            until(&mut g, &db, 7, 55);
            g.location = Location::School;
            until(&mut g, &db, 8, 25);
            if g.year_over() {
                break;
            }
            let _ = g.start(Activity::Class { focus: Focus::Notes });
            lessons_until(&mut g, &db, at::LUNCH, skill, &mut rng, &mut played);
            if g.year_over() {
                break;
            }
            let _ = g.school_lunch();
            until(&mut g, &db, 13, 40);
            let _ = g.start(Activity::Class { focus: Focus::Listen });
            let out = at::school_out(g.time.date.weekday() as u8);
            lessons_until(&mut g, &db, out, skill, &mut rng, &mut played);
            g.location = Location::Home;
        }
        until(&mut g, &db, 19, 0);
        let _ = g.eat_with_kenji();
        until(&mut g, &db, 21, 30);
        g.sleep(&db);
    }
    (g, played)
}

#[test]
fn a_whole_year_of_lessons_to_the_closing_ceremony() {
    let (mut g, played) = good_year(21, 0.8);
    assert!(g.year_over());
    assert_eq!(g.time.date, sim::Date::new(2027, 3, 24), "the game ends on the closing ceremony's day");
    // about 29 lessons a week over the year's ~200 school days, less the ceremonies and tests
    assert!(played > 900, "played {played} lessons");
    // every test taken
    let r = &g.lessons;
    for e in Exam::ALL {
        for s in e.subjects() {
            let t = r.test(e, s).unwrap_or_else(|| panic!("no {s:?} mark in the {}", e.name()));
            assert!(t.sat && t.mark >= 40, "{e:?} {s:?}: {}", t.mark);
        }
    }
    assert_eq!(r.tests.len(), 5 + 9 + 5 + 9 + 9);
    // the first two report cards came out on the terms' last days, the third at the ceremony
    assert_eq!(r.reports.len(), 2);
    assert_eq!(r.reports[0].date, sim::Date::new(2026, 7, 17));
    assert_eq!(r.reports[1].date, sim::Date::new(2026, 12, 25));
    let summary = g.close_year();
    assert!(summary.at_ceremony);
    assert_eq!(g.lessons.reports.len(), 3);
    for (s, grade) in &summary.grades {
        match grade {
            Some(gr) => assert!((3..=5).contains(gr), "{s:?}: {gr} for a good student"),
            None => assert!(!sim::lessons::graded(*s)),
        }
    }
    // what was learned: the weeks of every subject were covered
    for &s in &SUBJECTS {
        let all = sim::lessons::syllabus::year_of(s);
        let m = g.lessons.mastery_of(s, &all);
        assert!(m > 0.4, "{s:?}: mastery {m}");
    }
    assert_eq!(g.player.days_absent, 0);
    println!("{}", summary.lines.join("\n"));
}

#[test]
fn a_year_skipping_school() {
    let db = CardDb::generate(3);
    let mut g = GameState::new(5, &db);
    while !g.year_over() {
        g.location = Location::Home;
        until(&mut g, &db, 21, 30);
        g.sleep(&db);
    }
    let s = g.close_year();
    assert!(!s.at_ceremony);
    assert!(g.lessons.tests.iter().all(|t| !t.sat && t.mark == 0));
    for (sub, grade) in &s.grades {
        if let Some(gr) = grade {
            assert_eq!(*gr, 1, "{sub:?}");
        }
    }
    assert!(g.player.days_absent > 300);
}
