//! Card database: every released card, loaded from assets/cards/sets/*.json (or generated).

use crate::card::{CardDef, CardId, Rarity};
use crate::season::generate_season;
use std::collections::HashMap;
use std::path::Path;

#[derive(Default)]
pub struct CardDb {
    cards: Vec<CardDef>,
    index: HashMap<CardId, usize>,
}

impl CardDb {
    pub fn from_cards(cards: Vec<CardDef>) -> Self {
        let index = cards.iter().enumerate().map(|(i, c)| (c.id, i)).collect();
        Self { cards, index }
    }

    /// Generate seasons 1..=sets in memory.
    pub fn generate(sets: u16) -> Self {
        Self::from_cards((1..=sets).flat_map(generate_season).collect())
    }

    /// Load every `Sxx.json` in `dir`; seasons with no JSON yet are generated.
    pub fn load_or_generate(dir: &Path, sets: u16) -> Self {
        let mut cards = Vec::new();
        for set in 1..=sets {
            let path = dir.join(format!("S{set:02}.json"));
            let loaded = std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str::<Vec<CardDef>>(&s).map_err(|e| {
                    eprintln!("warning: {} is invalid ({e}); regenerating in memory", path.display());
                }).ok());
            cards.extend(loaded.unwrap_or_else(|| generate_season(set)));
        }
        Self::from_cards(cards)
    }

    pub fn get(&self, id: CardId) -> &CardDef {
        &self.cards[self.index[&id]]
    }

    pub fn try_get(&self, id: CardId) -> Option<&CardDef> {
        self.index.get(&id).map(|&i| &self.cards[i])
    }

    pub fn all(&self) -> &[CardDef] {
        &self.cards
    }

    pub fn set(&self, set: u16) -> impl Iterator<Item = &CardDef> {
        self.cards.iter().filter(move |c| c.id.set == set)
    }

    pub fn set_rarity(&self, set: u16, rarity: Rarity) -> Vec<CardId> {
        self.set(set).filter(|c| c.rarity == rarity).map(|c| c.id).collect()
    }
}
