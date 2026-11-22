//! Card data model for "Mythbeasts" (working name): monsters, spells and relics.
//!
//! Gameplay rules are intentionally not designed yet. A card here carries only what the life sim
//! needs: identity, type, attribute, rarity, a "hype" value that drives demand, and art data.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Attribute {
    Flame,
    Tide,
    Gale,
    Terra,
    Radiant,
    Umbral,
}

impl Attribute {
    pub const ALL: [Attribute; 6] = [
        Attribute::Flame, Attribute::Tide, Attribute::Gale, Attribute::Terra, Attribute::Radiant, Attribute::Umbral,
    ];

    /// Frame colour (sRGB) for the card renderer and UI.
    pub fn color(self) -> [u8; 3] {
        match self {
            Attribute::Flame => [214, 74, 44],
            Attribute::Tide => [46, 116, 204],
            Attribute::Gale => [92, 186, 150],
            Attribute::Terra => [150, 110, 62],
            Attribute::Radiant => [236, 196, 74],
            Attribute::Umbral => [108, 64, 156],
        }
    }

    pub fn look(self) -> &'static str {
        match self {
            Attribute::Flame => "roaring fire, embers and molten light, reds and oranges",
            Attribute::Tide => "crashing waves, deep-sea glow and bubbles, blues and teals",
            Attribute::Gale => "swirling wind, storm clouds and lightning, greens and silvers",
            Attribute::Terra => "cracked stone, crystals and roots, browns and ambers",
            Attribute::Radiant => "holy light, golden halos and feathers, golds and whites",
            Attribute::Umbral => "shadow mist, glowing runes and moonlight, purples and blacks",
        }
    }
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Mythic,
}

impl Rarity {
    pub fn mark(self) -> &'static str {
        match self {
            Rarity::Common => "C",
            Rarity::Uncommon => "U",
            Rarity::Rare => "R",
            Rarity::Mythic => "M",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Monster,
    Spell,
    Relic,
}

/// Stable card identity: season number + collector number. Displayed as "S01-023".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CardId {
    pub set: u16,
    pub number: u16,
}

impl fmt::Display for CardId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "S{:02}-{:03}", self.set, self.number)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardDef {
    pub id: CardId,
    pub name: String,
    pub kind: Kind,
    pub attribute: Attribute,
    pub rarity: Rarity,
    /// How much kids talk about this card, from 0 to 1. It stands in for "playability" until the
    /// rules exist, and drives demand and price.
    pub hype: f32,
    pub flavor: String,
    /// Prompt for the illustration (generated with ChatGPT; see tools/cardgen).
    pub art_prompt: String,
    /// Illustration path relative to assets/cards/art, e.g. "S01/S01-023.png".
    pub art: String,
}
