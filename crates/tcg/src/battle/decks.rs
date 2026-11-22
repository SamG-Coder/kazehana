//! Ready-made 20-card decks from a season's cards (starter decks for the duel and the AI).

use super::{stats, Stats, DECK_SIZE};
use crate::card::{Attribute, CardDef, Kind, Rarity};
use crate::db::CardDb;

/// A themed starter: name, main type, second type.
pub const STARTERS: [(&str, Attribute, Attribute); 3] = [
    ("Blaze", Attribute::Flame, Attribute::Gale),
    ("Torrent", Attribute::Tide, Attribute::Radiant),
    ("Quake", Attribute::Terra, Attribute::Umbral),
];

/// Deck shape: 12 monsters (one Mythic and one Rare when the types have them, up to 4 Uncommons,
/// the rest Commons), 5 spells,
/// 3 relics. At most 2 copies of a card, 1 of a Rare or Mythic.
pub fn starter(db: &CardDb, set: u16, main: Attribute, second: Attribute) -> Vec<Stats> {
    let pool: Vec<&CardDef> = db.set(set).collect();
    // both of the deck's types share the slots (by collector number), then anything else
    let rank = |c: &CardDef| -> u32 { if c.attribute == main || c.attribute == second { 0 } else { 1 } };
    let mut deck: Vec<Stats> = Vec::new();
    let take = |kind: Kind, rarities: &[Rarity], want: usize, copies: u32, deck: &mut Vec<Stats>| {
        let mut cands: Vec<&&CardDef> = pool.iter().filter(|c| c.kind == kind && rarities.contains(&c.rarity)).collect();
        cands.sort_by_key(|c| (rank(c), c.id.number));
        let mut got = 0;
        for c in cands {
            for _ in 0..copies {
                if got < want {
                    deck.push(stats(c));
                    got += 1;
                }
            }
        }
        got
    };
    // the stars: a Mythic and a Rare (each needs a monster to ascend onto)
    let stars = take(Kind::Monster, &[Rarity::Mythic], 1, 1, &mut deck) + take(Kind::Monster, &[Rarity::Rare], 1, 1, &mut deck);
    // a solid middle: up to 4 Uncommons, then Commons
    let mids = take(Kind::Monster, &[Rarity::Uncommon], 4, 2, &mut deck);
    take(Kind::Monster, &[Rarity::Common], 12 - stars - mids, 2, &mut deck);
    take(Kind::Spell, &[Rarity::Common, Rarity::Uncommon, Rarity::Rare], 5, 2, &mut deck);
    take(Kind::Relic, &[Rarity::Common, Rarity::Uncommon, Rarity::Rare], 3, 1, &mut deck);
    // top up with commons of any kind if the set is thin
    if deck.len() < DECK_SIZE {
        let have: Vec<_> = deck.iter().map(|s| s.id).collect();
        for c in pool.iter().filter(|c| c.rarity == Rarity::Common && !have.contains(&c.id)) {
            if deck.len() >= DECK_SIZE {
                break;
            }
            deck.push(stats(c));
        }
    }
    deck.truncate(DECK_SIZE);
    deck
}

/// The starter called `name` (case-insensitive), from `set`.
pub fn starter_named(db: &CardDb, set: u16, name: &str) -> Option<Vec<Stats>> {
    STARTERS.iter().find(|s| s.0.eq_ignore_ascii_case(name)).map(|s| starter(db, set, s.1, s.2))
}
