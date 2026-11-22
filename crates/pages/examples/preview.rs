//! Draw sample pages (a few days played) for review: shots/pages/*.png
//!     cargo run -p pages --example preview
use sim::clock::at;
use sim::life::Activity;
use sim::{GameState, Location};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let db = tcg::CardDb::generate(3);
    let mut g = GameState::new(7, &db);
    // two days: breakfast, school, the counter, a konbini stop, dinner, bed
    for _ in 0..2 {
        g.location = Location::Home;
        g.advance(at::hm(7, 0) - g.time.minute, &db);
        let _ = g.eat_with_kenji();
        g.advance(20, &db);
        g.location = Location::School;
        g.advance(at::hm(8, 25) - g.time.minute, &db);
        g.enter_classroom(&db);
        g.advance(at::LUNCH - g.time.minute, &db);
        let _ = g.school_lunch();
        g.advance(at::AFTERNOON - g.time.minute, &db);
        g.enter_classroom(&db);
        g.advance(at::SCHOOL_OUT - g.time.minute, &db);
        g.talk(3);
        g.talk(10);
        g.location = Location::Path;
        let _ = g.buy("croquette", "butcher", 1);
        let _ = g.buy("ramune", "liquor shop", 1);
        let _ = g.eat("croquette");
        g.advance(10, &db);
        g.location = Location::Home;
        g.advance(at::hm(16, 30) - g.time.minute, &db);
        let _ = g.start(Activity::Counter);
        g.advance(90, &db);
        g.stop();
        g.advance(at::hm(19, 0) - g.time.minute, &db);
        let _ = g.eat_with_kenji();
        g.advance(25, &db);
        let _ = g.start(Activity::Study);
        g.advance(60, &db);
        g.stop();
        g.advance(at::hm(21, 45) - g.time.minute, &db);
        g.sleep(&db);
    }
    let today = g.time.date;
    g.life.journal.plan(sim::Date::new(2026, 4, 11), "Help Kenji (release day prep)");
    g.life.journal.plan(sim::Date::new(2026, 4, 18), "Card swap with Ren?");
    g.life.journal.plan(sim::Date::new(2026, 4, 29), "Fishing at the river");
    let plans: Vec<_> = g.life.journal.plans.iter().map(|p| (p.date, p.text.clone())).collect();
    pages::calendar::month(2026, 4, Some(today), &plans).save_png(Path::new("shots/pages/calendar_2026_04.png"))?;
    pages::calendar::month(2026, 8, None, &[]).save_png(Path::new("shots/pages/calendar_2026_08.png"))?;
    pages::journal::diary(&g).save_png(Path::new("shots/pages/journal_diary.png"))?;
    pages::journal::people(&g).save_png(Path::new("shots/pages/journal_people.png"))?;
    pages::journal::asks_and_plans(&g).save_png(Path::new("shots/pages/journal_plans.png"))?;
    school_pages()?;
    println!("pages written to shots/pages (today {today})");
    Ok(())
}

/// Schoolwork: a lesson's print played and marked, and a few weeks of notes, for maths, Japanese
/// and English (the props' samples go to assets/cache/pages).
fn school_pages() -> anyhow::Result<()> {
    use sim::lessons::paper::{board, Notes, Sheet};
    use sim::lessons::syllabus::week_of;
    use sim::school::Subject;
    let db = tcg::CardDb::generate(3);
    for (subject, date, file) in [(Subject::Maths, sim::Date::new(2026, 4, 20), "maths"), (Subject::Japanese, sim::Date::new(2026, 6, 29), "japanese"), (Subject::English, sim::Date::new(2026, 9, 14), "english")] {
        let mut g = GameState::new(11, &db);
        // the subject's first lesson that week, seated at its bell
        let wd = (0..5u8).find_map(|d| (1..=6u8).find(|&p| sim::school::my_lesson(d, p).map(|l| l.subject) == Some(subject)).map(|p| (d, p))).unwrap();
        let day = date.add_days(wd.0 as i64 - date.weekday().index() as i64);
        g.time = sim::GameTime::new(day, sim::school::PERIODS[wd.1 as usize - 1].0);
        g.location = Location::School;
        g.start(Activity::Class { focus: sim::life::Focus::Notes }).unwrap();
        let mut l = g.begin_lesson().expect("a lesson at the bell");
        let mut rng = tcg::Rng::new(5);
        while !l.play.finished {
            l.play.auto(0.7, &mut rng);
        }
        let sheet = Sheet::from_lesson(&l, true);
        let ws = pages::school::worksheet(&sheet, "九十九");
        ws.save_png(Path::new(&format!("shots/pages/worksheet_{file}.png")))?;
        // the notebook: the board of this week and the two before
        let (t, w) = sim::lessons::syllabus::term_week(day).unwrap();
        let notes: Vec<Notes> = (w.saturating_sub(2).max(1)..=w)
            .filter_map(|k| week_of(subject, t, k).map(|wk| (k, wk)))
            .map(|(k, wk)| {
                let d = day.add_days(-7 * (w - k) as i64);
                Notes { subject, date: d, lines: board(&wk, d, 11) }
            })
            .collect();
        let nb = pages::school::notebook(subject, &notes);
        nb.save_png(Path::new(&format!("shots/pages/notebook_{file}.png")))?;
        if subject == Subject::Maths {
            ws.save_png(Path::new("assets/cache/pages/worksheet_sample.png"))?;
            nb.save_png(Path::new("assets/cache/pages/notebook_sample.png"))?;
        }
    }
    Ok(())
}
