//! Card prices, the uncle's discount, and selling cards to other kids.

use crate::people::{Personality, Student};
use serde::{Deserialize, Serialize};
use tcg::{CardDb, CardDef, CardId, Rarity, Rng, PACK_PRICE};

/// Copies of each card opened in town so far (drives scarcity).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Market {
    opened: Vec<(CardId, u32)>,
    /// Packs opened per season, to know the expected count for each rarity.
    packs_per_set: Vec<u32>,
}

impl Market {
    pub fn record_pack(&mut self, set: u16, pulls: &[tcg::Pull]) {
        let i = set as usize;
        if self.packs_per_set.len() <= i {
            self.packs_per_set.resize(i + 1, 0);
        }
        self.packs_per_set[i] += 1;
        for p in pulls {
            match self.opened.binary_search_by_key(&p.id, |x| x.0) {
                Ok(j) => self.opened[j].1 += 1,
                Err(j) => self.opened.insert(j, (p.id, 1)),
            }
        }
    }

    pub fn opened(&self, id: CardId) -> u32 {
        self.opened.binary_search_by_key(&id, |x| x.0).map_or(0, |j| self.opened[j].1)
    }

    fn expected(&self, card: &CardDef) -> f32 {
        let packs = self.packs_per_set.get(card.id.set as usize).copied().unwrap_or(0) as f32;
        let per_pack = match card.rarity {
            Rarity::Common => 4.0 / 30.0,
            Rarity::Uncommon => 1.0 / 15.0,
            Rarity::Rare => 7.0 / 8.0 / 10.0,
            Rarity::Mythic => 1.0 / 8.0 / 5.0,
        };
        packs * per_pack
    }

    /// Fair market value in yen.
    pub fn value(&self, card: &CardDef, foil: bool, current_season: u16) -> u32 {
        let base = match card.rarity {
            Rarity::Common => 20.0,
            Rarity::Uncommon => 60.0,
            Rarity::Rare => 250.0,
            Rarity::Mythic => 1500.0,
        };
        let hype = 0.5 + card.hype * 1.5;
        let expected = self.expected(card);
        let scarcity = ((expected + 5.0) / (self.opened(card.id) as f32 + 5.0)).clamp(0.6, 1.8);
        let age = current_season.saturating_sub(card.id.set) as i32;
        let age_factor = if card.rarity == Rarity::Mythic { 1.03f32.powi(age) } else { 0.85f32.powi(age) };
        let foil_factor = if foil { 3.0 } else { 1.0 };
        let v = base * hype * scarcity * age_factor * foil_factor;
        // Round to ¥10 like real price stickers, at least ¥10.
        ((v / 10.0).round() as u32 * 10).max(10)
    }
}

/// The uncle's discount by Kizuna rank: 20% at first, up to 50%.
pub fn uncle_discount(rank: u8) -> f32 {
    (0.2 + 0.03 * rank as f32).min(0.5)
}

pub fn pack_cost(uncle_rank: u8) -> u32 {
    let c = PACK_PRICE as f32 * (1.0 - uncle_discount(uncle_rank));
    ((c / 10.0).round() as u32) * 10
}

/// Highest price this student would pay for the card right now.
pub fn willingness(s: &Student, card: &CardDef, foil: bool, value: u32, bond_rank: u8) -> u32 {
    if !s.plays_cards {
        return 0;
    }
    let wanted = s.wants.contains(&card.id);
    let mut need = if wanted {
        1.6 + 0.3 * f32::from(matches!(s.personality, Personality::Competitive | Personality::Collector))
            + 0.6 * f32::from(card.rarity == Rarity::Mythic)
    } else if s.favorites.contains(&card.attribute) {
        1.2
    } else {
        0.5
    };
    // Already has a playset and doesn't collect: not interested.
    if s.collection.count(card.id) >= 2 && s.personality != Personality::Collector {
        need *= 0.3;
    }
    let personality = match s.personality {
        Personality::Collector if foil => 1.5,
        Personality::Flashy if card.rarity >= Rarity::Rare => 1.3,
        Personality::Haggler => 0.9,
        Personality::Casual if !wanted => 0.8,
        _ => 1.0,
    };
    let trust = 1.0 + 0.03 * bond_rank as f32;
    let max = (value as f32 * need * personality * trust) as u32;
    (max / 10 * 10).min(s.wallet)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaleResponse {
    Accept(u32),
    Counter(u32),
    Refuse(&'static str),
}

#[derive(Clone, Debug)]
pub struct SaleOutcome {
    pub response: SaleResponse,
    pub bond_delta: i32,
    pub reputation_delta: i32,
}

/// Offer `card` to `s` for `asking` yen. The caller moves money and cards on `Accept`.
pub fn offer(s: &Student, card: &CardDef, foil: bool, value: u32, asking: u32, bond_rank: u8, rng: &mut Rng) -> SaleOutcome {
    let max = willingness(s, card, foil, value, bond_rank);
    let no = |why| SaleOutcome { response: SaleResponse::Refuse(why), bond_delta: 0, reputation_delta: 0 };
    if !s.plays_cards {
        return no("doesn't play cards");
    }
    if max == 0 {
        return no("isn't interested");
    }
    if asking > s.wallet {
        return no("can't afford it");
    }
    let gouge = asking as f32 > value as f32 * 1.3;
    if asking <= max {
        // Hagglers try once even when they'd pay.
        if s.personality == Personality::Haggler && rng.chance(0.6) && asking > value {
            let c = ((asking as f32 * 0.85) as u32 / 10 * 10).max(value.min(asking));
            return SaleOutcome { response: SaleResponse::Counter(c), bond_delta: 0, reputation_delta: 0 };
        }
        let (bond, rep) = if gouge {
            (-3, -2)
        } else if asking <= value {
            (2, 1)
        } else {
            (1, 0)
        };
        return SaleOutcome { response: SaleResponse::Accept(asking), bond_delta: bond, reputation_delta: rep };
    }
    if asking as f32 <= max as f32 * 1.25 || s.personality == Personality::Haggler {
        return SaleOutcome { response: SaleResponse::Counter(max), bond_delta: 0, reputation_delta: 0 };
    }
    SaleOutcome { response: SaleResponse::Refuse("too expensive"), bond_delta: -1, reputation_delta: i32::from(gouge) * -1 }
}

/// Convenience: value lookup by id.
pub fn value_of(db: &CardDb, market: &Market, id: CardId, foil: bool, season: u16) -> u32 {
    market.value(db.get(id), foil, season)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::people::generate_students;

    #[test]
    fn discount_and_pack_cost() {
        assert_eq!(pack_cost(0), 240);
        assert_eq!(pack_cost(10), 150);
    }

    #[test]
    fn need_drives_price() {
        let db = CardDb::generate(1);
        let market = Market::default();
        let students = generate_students(5, &db);
        let s = students.iter().find(|s| s.plays_cards && !s.wants.is_empty()).unwrap();
        let wanted = db.get(s.wants[0]);
        let other = db.set(1).find(|c| !s.wants.contains(&c.id) && !s.favorites.contains(&c.attribute)
            && c.rarity == wanted.rarity).unwrap();
        let mut rich = s.clone();
        rich.wallet = 100_000;
        let vw = market.value(wanted, false, 1);
        let vo = market.value(other, false, 1);
        assert!(willingness(&rich, wanted, false, vw, 0) as f32 >= vw as f32 * 1.5);
        assert!(willingness(&rich, other, false, vo, 0) as f32 <= vo as f32 * 0.6);
        // Gouging is remembered.
        let mut rng = Rng::new(1);
        let mut honest = rich.clone();
        honest.personality = Personality::Competitive;
        let out = offer(&honest, wanted, false, vw, (vw as f32 * 1.5) as u32, 0, &mut rng);
        assert!(matches!(out.response, SaleResponse::Accept(_)));
        assert!(out.bond_delta < 0 && out.reputation_delta < 0);
    }
}
