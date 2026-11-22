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
    println!("pages written to shots/pages (today {today})");
    Ok(())
}
