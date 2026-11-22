//! A day of life: time only jumps when you sleep; hunger, food, lessons, the counter, the
//! journal.

use sim::clock::at;
use sim::life::{Activity, Focus};
use sim::{GameState, Location};
use tcg::CardDb;

fn game() -> (GameState, CardDb) {
    let db = CardDb::generate(3);
    (GameState::new(5, &db), db)
}

#[test]
fn time_only_jumps_when_you_sleep() {
    let (mut g, db) = game();
    let t0 = g.time;
    g.location = Location::Home;
    g.start(Activity::Study).unwrap();
    assert_eq!(g.time, t0, "starting something doesn't move the clock");
    assert!(g.pace() > 1.0, "but time runs faster while you study");
    g.advance(30, &db);
    g.stop().unwrap();
    assert_eq!(g.time.minute, t0.minute + 30);
    // the counter and the classroom no longer skip hours
    g.time.minute = at::hm(11, 0);
    g.work_shift(&db);
    assert_eq!(g.time.minute, at::hm(11, 0));
    // sleep does jump: to 6:30 tomorrow
    g.time.minute = at::hm(21, 30);
    let day = g.time.date;
    g.sleep(&db);
    assert_eq!((g.time.date, g.time.minute), (day.add_days(1), at::WAKE));
}

#[test]
fn you_get_hungry_and_food_fixes_it() {
    let (mut g, db) = game();
    g.life.fullness = 60.0;
    g.advance(8 * 60, &db);
    assert!(g.life.hungry(), "eight hours without food: {}", g.life.fullness);
    let e = g.player.energy;
    // buy two onigiri at the konbini, eat one
    g.location = Location::Path;
    let money = g.player.money;
    g.buy("onigiri_salmon", "konbini", 2).unwrap();
    assert_eq!(g.player.money, money - 300);
    assert_eq!(g.life.bag.count("onigiri_salmon"), 2);
    g.eat("onigiri_salmon").unwrap();
    g.advance(5, &db);
    assert!(g.doing().is_none(), "a three-minute onigiri is finished");
    assert!(g.life.fullness > 25.0, "fullness {}", g.life.fullness);
    assert_eq!(g.life.bag.count("onigiri_salmon"), 1);
    assert!(g.player.energy >= e - 1);
    // things that aren't sold here, or out of season, or don't fit
    assert!(g.buy("croquette", "konbini", 1).is_err());
    assert!(g.buy("kakigori", "festival", 1).is_err(), "no shaved ice in April");
    assert!(g.buy("cola", "vending", 99).is_err());
    // the onigiri left in the bag goes off in two days
    g.advance(2 * 24 * 60, &db);
    assert_eq!(g.life.bag.count("onigiri_salmon"), 0);
    assert!(g.life.journal.diary.iter().any(|d| d.lines.iter().any(|l| l.contains("gone off"))));
}

#[test]
fn lessons_count_by_being_there() {
    let (mut g, db) = game();
    // a Monday: skip the morning entirely
    g.location = Location::School;
    g.advance(at::LUNCH - g.time.minute + 1, &db);
    assert!(!g.attended[0]);
    assert_eq!(g.player.days_absent, 1);
    // the afternoon: sit down ten minutes late, stay to the bell
    g.advance(at::AFTERNOON + 10 - g.time.minute, &db);
    g.start(Activity::Class { focus: Focus::Notes }).unwrap();
    let k = g.player.knowledge;
    g.advance(at::SCHOOL_OUT - g.time.minute, &db);
    assert!(g.attended[1], "{} minutes of {}", g.life.class_minutes[1], at::SCHOOL_OUT - at::AFTERNOON);
    assert_eq!(g.player.days_late, 1);
    assert!(g.player.knowledge > k);
    assert!(g.doing().is_none(), "the bell ends it");
}

#[test]
fn helping_in_the_shop_pays_by_the_hour() {
    let (mut g, db) = game();
    g.location = Location::Home;
    g.time.minute = at::hm(16, 0);
    let money = g.player.money;
    g.start(Activity::Counter).unwrap();
    g.advance(120, &db);
    let earned = g.player.money - money;
    assert!((290..=310).contains(&earned), "two hours: ¥{earned}");
    // it stops at closing time
    g.advance(at::SHOP_CLOSE - g.time.minute + 2, &db);
    assert!(g.doing().is_none());
    assert!(g.start(Activity::Counter).is_err(), "the shop's closed");
}

#[test]
fn meals_with_kenji_and_school_lunch() {
    let (mut g, db) = game();
    g.location = Location::Home;
    g.time.minute = at::hm(7, 0);
    g.life.fullness = 20.0;
    g.eat_with_kenji().unwrap();
    assert!(g.eat_with_kenji().is_err(), "one breakfast");
    g.advance(20, &db);
    assert!(g.life.fullness > 55.0);
    g.location = Location::School;
    assert!(g.school_lunch().is_err(), "not lunchtime yet");
    g.advance(at::LUNCH - g.time.minute, &db);
    g.school_lunch().unwrap();
    g.advance(25, &db);
    assert!(g.life.meals.lunch);
}

#[test]
fn bedtime_and_an_empty_stomach_set_tomorrow() {
    let (mut g, db) = game();
    g.location = Location::Home;
    g.time.minute = at::hm(22, 0);
    g.life.fullness = 80.0;
    g.sleep(&db);
    assert_eq!(g.player.energy, 100);
    // a late night on an empty stomach
    g.advance(at::hm(23, 59) - g.time.minute + 61, &db); // (to 1:00)
    g.life.fullness = 5.0;
    g.sleep(&db);
    assert!(g.player.energy < 60, "energy {}", g.player.energy);
    // a nap in the afternoon is an hour and a half
    g.time.minute = at::hm(14, 0);
    g.sleep(&db);
    assert_eq!(g.time.minute, at::hm(15, 30));
}

#[test]
fn the_journal_writes_itself() {
    let (mut g, db) = game();
    g.location = Location::Home;
    g.time.minute = at::hm(13, 0);
    g.start(Activity::Counter).unwrap();
    g.advance(60, &db);
    g.stop();
    g.buy("melon_pan", "bakery", 1).unwrap();
    let today = g.life.journal.day(g.time.date).unwrap();
    assert!(today.lines.iter().any(|l| l.contains("Helped Uncle Kenji")), "{:?}", today.lines);
    assert!(today.lines.iter().any(|l| l.contains("melon pan")));
    g.life.journal.plan(g.time.date.add_days(5), "Release day: help Kenji");
    assert_eq!(g.life.journal.plans_on(g.time.date.add_days(5)).count(), 1);
}
