//! Booster packs and card collections.

use crate::card::{CardId, Rarity};
use crate::db::CardDb;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

pub const PACK_SIZE: usize = 6;
/// Retail price of one pack, in yen.
pub const PACK_PRICE: u32 = 300;
pub const PACKS_PER_BOX: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pull {
    pub id: CardId,
    /// Holo foil.
    pub foil: bool,
}

/// Open one pack: 4 Commons, 1 Uncommon, 1 Rare (a Mythic 1 time in 8), and a foil in 1 pack in 6.
pub fn open_pack(db: &CardDb, set: u16, rng: &mut Rng) -> Vec<Pull> {
    let commons = db.set_rarity(set, Rarity::Common);
    let uncommons = db.set_rarity(set, Rarity::Uncommon);
    let rares = db.set_rarity(set, Rarity::Rare);
    let mythic = db.set_rarity(set, Rarity::Mythic);

    let mut pulls = Vec::with_capacity(PACK_SIZE);
    // Commons without duplicates inside one pack.
    let mut c = commons.clone();
    rng.shuffle(&mut c);
    pulls.extend(c.iter().take(4).map(|&id| Pull { id, foil: false }));
    pulls.push(Pull { id: *rng.pick(&uncommons), foil: false });
    let top = if !mythic.is_empty() && rng.chance(1.0 / 8.0) { *rng.pick(&mythic) } else { *rng.pick(&rares) };
    pulls.push(Pull { id: top, foil: false });

    if rng.chance(1.0 / 6.0) {
        // The foil replaces the first common and can be any card in the set.
        let all: Vec<CardId> = db.set(set).map(|c| c.id).collect();
        pulls[0] = Pull { id: *rng.pick(&all), foil: true };
    }
    pulls
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Owned {
    pub id: CardId,
    pub normal: u32,
    pub foil: u32,
}

impl Owned {
    pub fn total(&self) -> u32 {
        self.normal + self.foil
    }
}

/// A sorted list of owned cards (serialises cleanly to JSON).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Collection {
    entries: Vec<Owned>,
}

impl Collection {
    pub fn add(&mut self, pull: Pull) {
        let i = match self.entries.binary_search_by_key(&pull.id, |o| o.id) {
            Ok(i) => i,
            Err(i) => {
                self.entries.insert(i, Owned { id: pull.id, normal: 0, foil: 0 });
                i
            }
        };
        if pull.foil {
            self.entries[i].foil += 1;
        } else {
            self.entries[i].normal += 1;
        }
    }

    /// Remove one copy. Returns false if not owned.
    pub fn remove(&mut self, pull: Pull) -> bool {
        let Ok(i) = self.entries.binary_search_by_key(&pull.id, |o| o.id) else { return false };
        let e = &mut self.entries[i];
        let slot = if pull.foil { &mut e.foil } else { &mut e.normal };
        if *slot == 0 {
            return false;
        }
        *slot -= 1;
        if e.total() == 0 {
            self.entries.remove(i);
        }
        true
    }

    pub fn count(&self, id: CardId) -> u32 {
        self.get(id).map_or(0, |o| o.total())
    }

    pub fn get(&self, id: CardId) -> Option<&Owned> {
        self.entries.binary_search_by_key(&id, |o| o.id).ok().map(|i| &self.entries[i])
    }

    pub fn iter(&self) -> impl Iterator<Item = &Owned> {
        self.entries.iter()
    }

    pub fn unique(&self) -> usize {
        self.entries.len()
    }

    pub fn total(&self) -> u32 {
        self.entries.iter().map(|o| o.total()).sum()
    }
}
