use super::ai::{choose, Level};
use super::decks::{starter, STARTERS};
use super::*;
use crate::db::CardDb;

fn monster(atk: i32, hp: i32, attribute: Attribute, keyword: Option<Keyword>) -> Stats {
    Stats {
        id: CardId { set: 1, number: 1 },
        kind: Kind::Monster,
        attribute,
        rarity: Rarity::Common,
        cost: 1,
        atk,
        hp,
        keyword,
        ascend: Ascend::No,
        ascend_cost: 0,
        effect: Effect::None,
    }
}

fn unit(uid: u32, s: Stats) -> Unit {
    Unit { uid, card: s, damage: 0, atk_mod: 0, hp_mod: 0, temp_atk: 0, relic: None, sick: false, attacked: false }
}

/// A duel with empty hands and decks, player 0 to move.
fn bare() -> Duel {
    let (mut d, _) = Duel::new([Vec::new(), Vec::new()], 0, 1);
    d.sides[0].spark = 6;
    d
}

#[test]
fn every_card_has_sane_stats() {
    let db = CardDb::generate(12);
    for c in db.all() {
        let s = stats(c);
        match c.kind {
            Kind::Monster => {
                assert!(s.atk >= 1 && s.hp >= 1, "{} {:?}", c.id, s);
                assert!(s.cost >= 1 && s.cost <= 5);
            }
            Kind::Spell => assert!(!matches!(s.effect, Effect::None | Effect::Equip { .. })),
            Kind::Relic => assert!(matches!(s.effect, Effect::Equip { .. })),
        }
    }
}

#[test]
fn starters_are_twenty_cards() {
    let db = CardDb::generate(12);
    for set in 1..=12 {
        for (_, a, b) in STARTERS {
            let d = starter(&db, set, a, b);
            assert_eq!(d.len(), DECK_SIZE, "set {set} {a:?}/{b:?}");
            assert!(d.iter().filter(|s| s.rarity == Rarity::Mythic).count() <= 1);
        }
    }
}

#[test]
fn both_monsters_deal_damage_with_super_bonus() {
    let mut d = bare();
    d.sides[0].field[0] = Some(unit(10, monster(3, 4, Attribute::Tide, None)));
    d.sides[1].field[0] = Some(unit(11, monster(2, 6, Attribute::Flame, None)));
    d.apply(Action::Attack { zone: 0, target: Target::Unit { side: 1, zone: 0 } }).unwrap();
    // Tide beats Flame: 3 + 2 = 5 to the defender; the defender strikes back for 2
    assert_eq!(d.unit(1, 0).unwrap().hp(), 1);
    assert_eq!(d.unit(0, 0).unwrap().hp(), 2);
}

#[test]
fn leader_is_protected_by_monsters_except_from_flyers() {
    let mut d = bare();
    d.sides[0].field[0] = Some(unit(10, monster(3, 3, Attribute::Gale, None)));
    d.sides[0].field[1] = Some(unit(12, monster(2, 2, Attribute::Gale, Some(Keyword::Flying))));
    d.sides[1].field[2] = Some(unit(11, monster(1, 1, Attribute::Terra, None)));
    assert!(!d.attack_targets(0).contains(&Target::Leader { side: 1 }));
    assert!(d.attack_targets(1).contains(&Target::Leader { side: 1 }));
    // a Guard stops the flyer too, and must be attacked first
    d.sides[1].field[1] = Some(unit(13, monster(1, 4, Attribute::Terra, Some(Keyword::Guard))));
    assert_eq!(d.attack_targets(1), vec![Target::Unit { side: 1, zone: 1 }]);
}

#[test]
fn pierce_carries_leftover_damage() {
    let mut d = bare();
    d.sides[0].field[0] = Some(unit(10, monster(6, 3, Attribute::Flame, Some(Keyword::Pierce))));
    d.sides[1].field[0] = Some(unit(11, monster(1, 2, Attribute::Tide, None)));
    d.apply(Action::Attack { zone: 0, target: Target::Unit { side: 1, zone: 0 } }).unwrap();
    assert!(d.unit(1, 0).is_none());
    assert_eq!(d.sides[1].leader, LEADER_HP - 4);
}

#[test]
fn traps_spring_on_attack() {
    let mut d = bare();
    d.sides[0].field[0] = Some(unit(10, monster(4, 3, Attribute::Gale, None)));
    let mut trap = monster(0, 0, Attribute::Tide, None);
    trap.kind = Kind::Spell;
    trap.effect = Effect::TrapNegate;
    d.sides[1].traps[0] = Some(trap);
    let ev = d.apply(Action::Attack { zone: 0, target: Target::Leader { side: 1 } }).unwrap();
    assert!(ev.iter().any(|e| matches!(e, Event::TrapSprung { .. })));
    assert_eq!(d.sides[1].leader, LEADER_HP);
    assert!(d.sides[1].traps[0].is_none());
}

#[test]
fn summoned_monsters_wait_unless_swift() {
    let mut d = bare();
    d.sides[0].hand = vec![monster(2, 2, Attribute::Flame, None), monster(2, 2, Attribute::Flame, Some(Keyword::Swift))];
    d.apply(Action::Summon { card: 0, zone: 0 }).unwrap();
    d.apply(Action::Summon { card: 0, zone: 1 }).unwrap();
    assert!(d.attack_targets(0).is_empty());
    assert!(!d.attack_targets(1).is_empty());
}

#[test]
fn illegal_actions_are_rejected() {
    let mut d = bare();
    assert!(d.apply(Action::Summon { card: 0, zone: 0 }).is_err());
    assert!(d.apply(Action::Attack { zone: 0, target: Target::Leader { side: 1 } }).is_err());
}

/// Play a whole AI game; returns (winner, turns).
fn play(db: &CardDb, a: usize, b: usize, la: Level, lb: Level, seed: u64) -> (Option<usize>, u32) {
    let decks = [starter(db, 1, STARTERS[a].1, STARTERS[a].2), starter(db, 1, STARTERS[b].1, STARTERS[b].2)];
    let (mut d, _) = Duel::new(decks, (seed % 2) as usize, seed);
    let mut rng = Rng::new(seed ^ 77);
    let mut steps = 0;
    while d.result.is_none() {
        let lv = if d.active == 0 { la } else { lb };
        let act = choose(&d, lv, &mut rng);
        d.apply(act).expect("AI chose an illegal action");
        steps += 1;
        assert!(steps < 5000, "runaway game");
    }
    (d.result.unwrap(), d.turn)
}

#[test]
fn ai_games_finish_and_are_balanced() {
    let db = CardDb::generate(1);
    let n = 90;
    let (mut first_wins, mut decided, mut turns) = (0, 0, 0u32);
    let mut deck_wins = [0u32; 3];
    let mut deck_games = [0u32; 3];
    for g in 0..n {
        let (a, b) = ((g % 3) as usize, ((g / 3 + 1 + g % 3) % 3) as usize);
        let seed = 1000 + g as u64;
        let first = (seed % 2) as usize;
        let (w, t) = play(&db, a, b, Level::Normal, Level::Normal, seed);
        turns += t;
        if let Some(w) = w {
            decided += 1;
            if w == first {
                first_wins += 1;
            }
            deck_wins[if w == 0 { a } else { b }] += 1;
        }
        deck_games[a] += 1;
        deck_games[b] += 1;
    }
    let avg = turns as f32 / n as f32;
    let fw = first_wins as f32 / decided.max(1) as f32;
    eprintln!("balance: {n} games, {decided} decided, avg {avg:.1} turns, first player wins {:.0}%", fw * 100.0);
    for i in 0..3 {
        eprintln!("  {}: {:.0}% of games won", STARTERS[i].0, 100.0 * deck_wins[i] as f32 / deck_games[i].max(1) as f32);
    }
    assert!(decided as f32 >= n as f32 * 0.9, "too many draws");
    assert!((8.0..=45.0).contains(&avg), "games last {avg} turns");
    assert!((0.3..=0.7).contains(&fw), "first player wins {fw}");
}

#[test]
fn hard_beats_easy_most_of_the_time() {
    let db = CardDb::generate(1);
    let mut hard = 0;
    let n = 24;
    for g in 0..n {
        let seed = 5000 + g as u64;
        // swap seats every game
        let (la, lb, hard_side) = if g % 2 == 0 { (Level::Hard, Level::Easy, 0) } else { (Level::Easy, Level::Hard, 1) };
        if play(&db, (g % 3) as usize, ((g + 1) % 3) as usize, la, lb, seed).0 == Some(hard_side) {
            hard += 1;
        }
    }
    eprintln!("hard beat easy {hard}/{n}");
    assert!(hard * 10 >= n * 6, "hard won only {hard}/{n}");
}

/// `cargo test -p tcg --release balance_report -- --ignored --nocapture`: a big AI-vs-AI run.
#[test]
#[ignore]
fn balance_report() {
    let db = CardDb::generate(1);
    let n: u32 = std::env::var("DUEL_GAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(900);
    let (mut first_wins, mut decided, mut turns) = (0u32, 0u32, 0u32);
    let mut wins = [[0u32; 3]; 3];
    let mut games = [[0u32; 3]; 3];
    let mut short = 0;
    let mut long = 0;
    for g in 0..n {
        let (a, b) = ((g % 3) as usize, ((g / 3) % 3) as usize);
        let seed = 90_000 + g as u64;
        let first = (seed % 2) as usize;
        let (w, t) = play(&db, a, b, Level::Normal, Level::Normal, seed);
        turns += t;
        short += (t < 12) as u32;
        long += (t > 34) as u32;
        games[a][b] += 1;
        if let Some(w) = w {
            decided += 1;
            first_wins += (w == first) as u32;
            if w == 0 {
                wins[a][b] += 1;
            }
        }
    }
    eprintln!("{n} games: {decided} decided, avg {:.1} turns ({short} under 12, {long} over 34), first player wins {:.1}%", turns as f32 / n as f32, 100.0 * first_wins as f32 / decided.max(1) as f32);
    for a in 0..3 {
        let row: Vec<String> = (0..3).map(|b| format!("{:>3.0}%", 100.0 * wins[a][b] as f32 / games[a][b].max(1) as f32)).collect();
        eprintln!("  {:8} vs {:?}: {}", STARTERS[a].0, STARTERS.map(|s| s.0), row.join(" "));
    }
}

#[test]
#[ignore]
fn print_starters() {
    let db = CardDb::generate(1);
    for (n, a, b) in STARTERS {
        let d = starter(&db, 1, a, b);
        eprintln!("== {n}");
        for s in &d {
            eprintln!("  {:?} {:?} {:?} cost {} {}/{} {:?} {:?} {}", s.kind, s.attribute, s.rarity, s.cost, s.atk, s.hp, s.keyword, s.ascend, s.effect.text());
        }
    }
}
