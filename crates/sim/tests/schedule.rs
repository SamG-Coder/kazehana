//! The day-to-day schedule played out: a good week, a careless week, a whole year, every food,
//! and the awkward edges (midnight in the middle of something, closed shops, weekends).

use sim::clock::at;
use sim::life::{catalog, Activity, Focus};
use sim::{GameState, Location};
use tcg::CardDb;

fn game() -> (GameState, CardDb) {
    let db = CardDb::generate(3);
    (GameState::new(11, &db), db)
}

/// Let time pass until the clock reads `h:m` (today, or tomorrow if that's passed).
fn until(g: &mut GameState, db: &CardDb, h: u32, m: u32) {
    let t = g.time.until(at::hm(h, m));
    g.advance(t, db);
}

/// A sensible school day: breakfast, lessons, lunch, an hour at the counter, dinner, a bath,
/// study, bed at 21:45. Weekends: a long shift and a lie-in.
fn good_day(g: &mut GameState, db: &CardDb) {
    let school = g.time.school_day();
    g.location = Location::Home;
    until(g, db, 7, 0);
    g.eat_with_kenji().unwrap();
    until(g, db, 7, 40);
    if school {
        g.location = Location::School;
        until(g, db, 8, 25);
        g.start(Activity::Class { focus: Focus::Notes }).unwrap();
        until(g, db, 12, 40);
        g.school_lunch().unwrap();
        until(g, db, 13, 40);
        g.start(Activity::Class { focus: Focus::Listen }).unwrap();
        let out = at::school_out(g.time.date.weekday() as u8);
        g.advance(g.time.until(out), db);
        g.location = Location::Home;
        until(g, db, 16, 0);
        g.start(Activity::Counter).unwrap();
        until(g, db, 17, 30);
        g.stop();
    } else {
        until(g, db, 10, 0);
        g.start(Activity::Counter).unwrap();
        until(g, db, 14, 0);
        g.stop();
        g.buy("onigiri_tunamayo", "konbini", 1).unwrap();
        g.eat("onigiri_tunamayo").unwrap();
        until(g, db, 14, 30);
        g.start(Activity::Counter).unwrap();
        until(g, db, 18, 0);
        g.stop();
    }
    until(g, db, 19, 0);
    g.eat_with_kenji().unwrap();
    until(g, db, 19, 40);
    g.start(Activity::Bath).unwrap();
    until(g, db, 20, 15);
    g.start(Activity::Study).unwrap();
    until(g, db, 21, 0);
    g.stop();
    g.start(Activity::Journal).unwrap();
    until(g, db, 21, 45);
    g.sleep(db);
}

#[test]
fn a_good_week() {
    let (mut g, db) = game();
    let money = g.player.money;
    let mut lowest_energy = 100;
    let mut lowest_full = 100.0f32;
    for _ in 0..7 {
        good_day(&mut g, &db);
        lowest_energy = lowest_energy.min(g.player.energy);
        lowest_full = lowest_full.min(g.life.fullness);
    }
    assert_eq!(g.player.days_absent, 0);
    assert_eq!(g.player.days_late, 0);
    assert!(g.attended == [false; 2], "a new day has started: {:?}", g.attended);
    assert!(g.player.energy >= 90, "rested after a sensible night: {}", g.player.energy);
    assert!(lowest_full > 15.0, "never went hungry: lowest {lowest_full}");
    assert!(g.player.knowledge >= 15, "five days of lessons and study: {}", g.player.knowledge);
    // the counter pays: 5 × 1.5 h + 2 × 7.5 h at ¥150/h, the Sunday allowance, less two onigiri
    let earned = g.player.money as i64 - money as i64;
    assert_eq!(earned, (5 * 90 + 2 * 450) * 150 / 60 + 1000 - 2 * 140, "earned ¥{earned}");
    assert!(g.uncle.rank() >= 2, "Kenji got to know you behind the counter: rank {}", g.uncle.rank());
    // the diary has every day, each with meals, lessons on school days, the shop
    assert_eq!(g.life.journal.diary.len(), 7, "{:?}", g.life.journal.diary.iter().map(|d| d.date).collect::<Vec<_>>());
    for d in &g.life.journal.diary {
        assert!(d.lines.iter().any(|l| l.contains("breakfast")), "{}: {:?}", d.date, d.lines);
        assert!(d.lines.iter().any(|l| l.contains("Went to bed")), "{}: {:?}", d.date, d.lines);
    }
}

#[test]
fn a_careless_week_catches_up_with_you() {
    let (mut g, db) = game();
    // no breakfast, no lessons, junk at the konbini, bed at 1 am
    for _ in 0..5 {
        g.location = Location::Path;
        until(&mut g, &db, 13, 0);
        let _ = g.buy("potato_chips", "konbini", 1);
        let _ = g.eat("potato_chips");
        until(&mut g, &db, 1, 0);
        g.location = Location::Home;
        g.sleep(&db);
    }
    assert_eq!(g.player.days_absent, 10, "every morning and afternoon missed");
    assert!(g.player.energy < 70, "short nights: {}", g.player.energy);
    assert!(g.life.fullness < 30.0, "living on chips: {}", g.life.fullness);
    assert!(g.life.journal.diary.iter().any(|d| d.lines.iter().any(|l| l.contains("Skipped"))));
}

#[test]
fn hunger_slows_what_goes_in() {
    let (mut fed, db) = game();
    let (mut hungry, _) = game();
    for g in [&mut fed, &mut hungry] {
        g.location = Location::Home;
        g.time.minute = at::hm(19, 0);
    }
    fed.life.fullness = 80.0;
    hungry.life.fullness = 10.0;
    for g in [&mut fed, &mut hungry] {
        g.start(Activity::Study).unwrap();
        g.advance(180, &db);
    }
    assert!(fed.player.knowledge > hungry.player.knowledge, "{} vs {}", fed.player.knowledge, hungry.player.knowledge);
}

#[test]
fn a_whole_year_runs() {
    let (mut g, db) = game();
    let start = g.time.date;
    // a lazy routine: eat with Kenji, do nothing, sleep
    for _ in 0..365 {
        g.location = Location::Home;
        until(&mut g, &db, 7, 0);
        let _ = g.eat_with_kenji();
        until(&mut g, &db, 19, 0);
        let _ = g.eat_with_kenji();
        until(&mut g, &db, 22, 0);
        g.sleep(&db);
    }
    assert_eq!(g.time.date, start.add_days(365));
    assert_eq!(g.season, 13, "a new card season every month");
    assert!(g.player.money >= 3000 + 52 * 1000 - 1000, "the allowance every Sunday: ¥{}", g.player.money);
    assert!(g.life.fullness > 10.0, "two meals a day keeps you from running empty: {}", g.life.fullness);
    assert_eq!(g.life.journal.diary.len(), 365);
}

#[test]
fn every_food_can_be_bought_and_eaten() {
    let (mut g, db) = game();
    let c = catalog();
    for it in c.items.iter().filter(|i| i.cat.edible() && i.price > 0) {
        let month = (1..=12).find(|m| it.in_season(*m)).unwrap();
        g.time.date = sim::Date::new(2026, month, 10);
        g.player.money = 100_000;
        g.life.bag.stacks.clear();
        g.life.fullness = 5.0;
        g.stop();
        let place = &it.sold[0];
        g.buy(&it.id, place, 1).unwrap_or_else(|e| panic!("{} at {place}: {e}", it.id));
        g.eat(&it.id).unwrap_or_else(|e| panic!("{}: {e}", it.id));
        g.advance(it.minutes + 1, &db);
        assert!(g.doing().is_none(), "{} wasn't finished in {} minutes", it.id, it.minutes);
        let gained = g.life.fullness - 5.0 + (it.minutes + 1) as f32 * 5.0 / 60.0;
        assert!((gained - it.full).abs() < 1.5, "{}: gained {gained}, says {}", it.id, it.full);
    }
}

#[test]
fn midnight_in_the_middle_of_something() {
    let (mut g, db) = game();
    g.location = Location::Home;
    g.time.minute = at::hm(23, 40);
    g.life.meals.dinner = true;
    let day = g.time.date;
    g.start(Activity::Study).unwrap();
    g.advance(40, &db);
    assert_eq!(g.time.date, day.add_days(1));
    assert!(matches!(g.doing(), Some(Activity::Study)), "still at the desk after midnight");
    assert!(!g.life.meals.dinner, "a new day: meals reset");
    assert_eq!(g.time.minute, 20);
}

#[test]
fn the_wrong_place_or_time() {
    let (mut g, db) = game();
    // lessons on a Saturday, or from home
    g.time.date = sim::Date::new(2026, 4, 11);
    g.location = Location::School;
    g.time.minute = at::hm(10, 0);
    assert!(g.start(Activity::Class { focus: Focus::Listen }).is_err());
    // the counter before opening, or away from the shop
    g.location = Location::Home;
    g.time.minute = at::hm(9, 0);
    assert!(g.start(Activity::Counter).is_err());
    g.location = Location::School;
    g.time.minute = at::hm(12, 0);
    assert!(g.start(Activity::Counter).is_err());
    // a bath at school; reading a book you haven't got
    assert!(g.start(Activity::Bath).is_err());
    assert!(g.start(Activity::Read { item: "manga_volume".into() }).is_err());
    // meals out of hours, twice, or at school
    g.location = Location::Home;
    g.time.minute = at::hm(15, 0);
    assert!(g.eat_with_kenji().is_err());
    g.time.minute = at::hm(19, 0);
    g.eat_with_kenji().unwrap();
    g.advance(25, &db);
    assert!(g.eat_with_kenji().is_err());
    // sleeping away from home
    g.location = Location::Path;
    let t = g.time;
    g.sleep(&db);
    assert_eq!(g.time, t, "no sleeping in the street");
}

#[test]
fn reading_finishes_and_teaches() {
    let (mut g, db) = game();
    g.location = Location::Home;
    g.time.minute = at::hm(20, 0);
    g.player.money = 5000;
    g.buy("manga_volume", "bookshop", 1).unwrap();
    g.start(Activity::Read { item: "manga_volume".into() }).unwrap();
    let charm = g.player.charm;
    g.advance(61, &db);
    assert!(g.doing().is_none(), "a volume takes an hour");
    assert!(g.player.charm > charm);
    assert_eq!(g.life.bag.count("manga_volume"), 1, "you keep the book");
}

#[test]
fn the_same_choices_make_the_same_week() {
    let (mut a, db) = game();
    let (mut b, _) = game();
    for _ in 0..3 {
        good_day(&mut a, &db);
        good_day(&mut b, &db);
    }
    assert_eq!(serde_json::to_string(&a.player).unwrap(), serde_json::to_string(&b.player).unwrap());
    assert_eq!(a.time, b.time);
}

/// A Monday in 1-A by the real bells: six periods, each by its own teacher, written up in the
/// diary by name.
#[test]
fn a_monday_by_the_timetable() {
    let (mut g, db) = game();
    assert_eq!(g.time.date.weekday() as u8, 0, "the game starts on a Monday");
    good_day(&mut g, &db);
    let wd = 0;
    let morning = sim::school::day_line(wd, 1, 4);
    let afternoon = sim::school::day_line(wd, 5, 6);
    assert_eq!(morning.split(", ").count(), 4);
    let lines: Vec<&String> = g.life.journal.diary.iter().flat_map(|d| d.lines.iter()).collect();
    assert!(lines.iter().any(|l| l.contains(&morning)), "the diary should name the morning's lessons ({morning}): {lines:?}");
    assert!(lines.iter().any(|l| l.contains(&afternoon)), "and the afternoon's ({afternoon}): {lines:?}");
    // every lesson has a teacher with a name
    for p in 1..=6 {
        assert!(sim::school::describe(wd, p).is_some_and(|s| s.contains("-sensei")));
    }
    println!("{lines:?}");
}

/// Once you're in school you're in till it's out: you can't walk out of a lesson or off the
/// grounds; a trip to the toilet (once a lesson) is the only way out, and slipping away on it is
/// cutting school.
#[test]
fn once_in_school_you_stay() {
    let (mut g, db) = game();
    until(&mut g, &db, 8, 0);
    // before school, going in is your choice; once in, the gate's shut behind you
    g.location = Location::School;
    assert!(g.school_locked());
    assert!(g.leave_school(Location::Path).is_err());
    assert_eq!(g.location, Location::School);
    // in the 2nd period, seated: you can't just get up
    until(&mut g, &db, 9, 55);
    g.start(Activity::Class { focus: Focus::Listen }).unwrap();
    g.advance(5, &db);
    assert!(g.can_get_up().is_err());
    // raise your hand: once a lesson
    assert!(g.ask_toilet().is_ok());
    assert!(g.doing().is_none());
    g.start(Activity::Class { focus: Focus::Listen }).unwrap();
    assert!(g.ask_toilet().is_err(), "a second time in the same lesson: wait for the bell");
    // the next lesson: ask again, and slip out of school on it
    until(&mut g, &db, 10, 55);
    g.start(Activity::Class { focus: Focus::Listen }).unwrap();
    assert!(g.ask_toilet().is_ok());
    let absent = g.player.days_absent;
    assert!(g.leave_school(Location::Path).is_ok());
    assert_eq!(g.location, Location::Path);
    assert_eq!(g.player.days_absent, absent + 1);
    let lines: Vec<&String> = g.life.journal.diary.iter().flat_map(|d| d.lines.iter()).collect();
    assert!(lines.iter().any(|l| l.contains("Slipped out of school")));
    // after school, you come and go as you like
    let (mut h, db2) = game();
    until(&mut h, &db2, 16, 30);
    h.location = Location::School;
    assert!(!h.school_locked());
    assert!(h.leave_school(Location::Path).is_ok());
}

/// Gone to the toilet and not back in ten minutes: someone's sent to look for you.
#[test]
fn a_long_trip_to_the_toilet_is_noticed() {
    let (mut g, db) = game();
    until(&mut g, &db, 8, 0);
    g.location = Location::School;
    until(&mut g, &db, 9, 0);
    g.start(Activity::Class { focus: Focus::Listen }).unwrap();
    g.ask_toilet().unwrap();
    g.advance(12, &db);
    let lines: Vec<&String> = g.life.journal.diary.iter().flat_map(|d| d.lines.iter()).collect();
    assert!(lines.iter().any(|l| l.contains("far too long at the toilet")), "{lines:?}");
}
