//! Monthly seasons (sets) and a placeholder set generator.
//!
//! The generator gives every season 60 named cards with art prompts so the shop, packs and the
//! economy can run. tools/cardgen writes them to assets/cards/sets/Sxx.json. From then on the
//! JSON is the source of truth and can be rewritten by hand.

use crate::card::*;
use crate::rng::Rng;
use std::collections::HashSet;

pub const CARDS_PER_SET: u16 = 60;

pub struct SeasonTheme {
    pub number: u16,
    pub name: &'static str,
    /// Calendar month the season releases in (1–12).
    pub month: u32,
    pub featured: Attribute,
    pub adjectives: &'static [&'static str],
    /// Visual direction for the illustration prompts.
    pub visual: &'static str,
}

pub const SEASONS: [SeasonTheme; 12] = [
    SeasonTheme { number: 1, name: "Dawn of Dragons", month: 4, featured: Attribute::Flame,
        adjectives: &["Dawn", "Young", "Blazing", "Crested", "First-Flight", "Ember"],
        visual: "dragons waking at sunrise over misty mountains and cherry blossoms" },
    SeasonTheme { number: 2, name: "Tidecaller's Deep", month: 5, featured: Attribute::Tide,
        adjectives: &["Abyssal", "Coral", "Tidal", "Pearl", "Drowned", "Riptide"],
        visual: "sunken temples and glowing sea creatures deep underwater" },
    SeasonTheme { number: 3, name: "Stormbreak", month: 6, featured: Attribute::Gale,
        adjectives: &["Thunder", "Storm", "Cyclone", "Rain-Soaked", "Static", "Howling"],
        visual: "lightning-split skies, endless rain and floating storm islands" },
    SeasonTheme { number: 4, name: "Starfall Sanctum", month: 7, featured: Attribute::Radiant,
        adjectives: &["Starborn", "Celestial", "Astral", "Comet", "Nova", "Wishing"],
        visual: "a temple among the stars with falling comets and the Milky Way" },
    SeasonTheme { number: 5, name: "Night of a Thousand Lanterns", month: 8, featured: Attribute::Umbral,
        adjectives: &["Phantom", "Lantern", "Spectral", "Festival", "Haunted", "Midnight"],
        visual: "a ghostly summer festival with floating lanterns and masked spirits" },
    SeasonTheme { number: 6, name: "Moonlit Wilds", month: 9, featured: Attribute::Terra,
        adjectives: &["Moonlit", "Feral", "Wildwood", "Silver", "Prowling", "Howling"],
        visual: "a huge full moon over an ancient forest full of beasts" },
    SeasonTheme { number: 7, name: "Ember Crown", month: 10, featured: Attribute::Flame,
        adjectives: &["Crowned", "Inferno", "Scarlet", "Molten", "Royal", "Ashen"],
        visual: "a volcano kingdom of crimson autumn leaves and fire knights" },
    SeasonTheme { number: 8, name: "Fox Lords", month: 11, featured: Attribute::Radiant,
        adjectives: &["Nine-Tailed", "Golden", "Trickster", "Shrine", "Foxfire", "Sacred"],
        visual: "golden rice fields, endless shrine gates and magical fox spirits" },
    SeasonTheme { number: 9, name: "Frostbound", month: 12, featured: Attribute::Tide,
        adjectives: &["Frost", "Glacial", "Snowbound", "Icicle", "Blizzard", "Crystal"],
        visual: "a frozen fortress in a blizzard with ice giants and snow spirits" },
    SeasonTheme { number: 10, name: "Dreamwalkers", month: 1, featured: Attribute::Umbral,
        adjectives: &["Dreaming", "Mirror", "Twilight", "Sleepless", "Illusory", "Lucid"],
        visual: "a floating dream world with upside-down castles and a giant sunrise" },
    SeasonTheme { number: 11, name: "Oni Uprising", month: 2, featured: Attribute::Terra,
        adjectives: &["Oni", "Iron-Horned", "Warlord", "Thunder-Drum", "Masked", "Rampaging"],
        visual: "armoured oni warlords storming a mountain fortress at dusk" },
    SeasonTheme { number: 12, name: "Rise of the Phoenix", month: 3, featured: Attribute::Gale,
        adjectives: &["Reborn", "Phoenix", "Soaring", "Spring", "Radiant-Wing", "Eternal"],
        visual: "a phoenix rising over a blooming valley at the end of winter" },
];

pub fn theme(set: u16) -> &'static SeasonTheme {
    &SEASONS[((set.max(1) - 1) % 12) as usize]
}

fn monsters(a: Attribute) -> &'static [&'static str] {
    match a {
        Attribute::Flame => &["Drake", "Salamander", "Phoenix Chick", "Lava Golem", "Fire Imp", "Hellhound", "Ember Wyrm", "Magma Turtle"],
        Attribute::Tide => &["Kraken", "Sea Serpent", "Kappa", "Siren", "Coral Knight", "Leviathan Pup", "Jellyfish Mage", "Shark Warrior"],
        Attribute::Gale => &["Griffin", "Thunderbird", "Harpy", "Wind Sprite", "Storm Falcon", "Tengu", "Sky Ray", "Cloud Wyvern"],
        Attribute::Terra => &["Golem", "Treant", "Rock Troll", "Dire Wolf", "Mole Knight", "Earth Wyrm", "Mushroom Ogre", "Boar Titan"],
        Attribute::Radiant => &["Paladin", "Unicorn", "Light Dragon", "Seraph", "Kitsune", "Sun Lion", "Star Knight", "Holy Golem"],
        Attribute::Umbral => &["Wraith", "Shadow Ninja", "Vampire Bat", "Lich", "Night Mare", "Ghost Cat", "Bone Dragon", "Dark Witch"],
    }
}

fn mythic_names(a: Attribute) -> &'static [&'static str] {
    match a {
        Attribute::Flame => &["Ignivar", "Pyraxis", "Kagura the Undying Flame"],
        Attribute::Tide => &["Thalassor", "Mizuchi", "Queen Nerelle"],
        Attribute::Gale => &["Zephyrion", "Raijin's Herald", "Aerok"],
        Attribute::Terra => &["Gorvok", "Yamata", "Old Man Mountain"],
        Attribute::Radiant => &["Solaria", "Amaterael", "Lumen Rex"],
        Attribute::Umbral => &["Nocthar", "Kuroyami", "The Hollow King"],
    }
}

fn spells(a: Attribute) -> &'static [&'static str] {
    match a {
        Attribute::Flame => &["Fireball", "Meteor Rain", "Blaze Wall", "Inferno Burst"],
        Attribute::Tide => &["Tsunami", "Healing Spring", "Whirlpool", "Ice Lance"],
        Attribute::Gale => &["Lightning Bolt", "Tornado", "Tailwind", "Chain Spark"],
        Attribute::Terra => &["Earthquake", "Stone Skin", "Vine Snare", "Landslide"],
        Attribute::Radiant => &["Holy Shield", "Revive", "Judgement Ray", "Blessing"],
        Attribute::Umbral => &["Curse", "Soul Drain", "Shadow Step", "Nightmare"],
    }
}

fn relics(a: Attribute) -> &'static [&'static str] {
    match a {
        Attribute::Flame => &["Dragonfire Blade", "Ember Crown", "Salamander Gauntlet"],
        Attribute::Tide => &["Trident", "Pearl of Tides", "Coral Shield"],
        Attribute::Gale => &["Storm Bow", "Feather Cloak", "Thunder Bell"],
        Attribute::Terra => &["Titan Hammer", "Root Staff", "Golem Heart"],
        Attribute::Radiant => &["Sun Sword", "Angel Halo", "Mirror Shield"],
        Attribute::Umbral => &["Cursed Mask", "Shadow Dagger", "Grimoire"],
    }
}

fn rarity_of(number: u16) -> Rarity {
    match number {
        1..=30 => Rarity::Common,
        31..=45 => Rarity::Uncommon,
        46..=55 => Rarity::Rare,
        _ => Rarity::Mythic,
    }
}

pub fn generate_season(set: u16) -> Vec<CardDef> {
    let th = theme(set);
    let mut rng = Rng::derive(set as u64, "season");
    let mut used = HashSet::new();
    let mut cards = Vec::with_capacity(CARDS_PER_SET as usize);

    for number in 1..=CARDS_PER_SET {
        let id = CardId { set, number };
        let rarity = rarity_of(number);
        let attribute = if rarity == Rarity::Mythic {
            // Five mythics: the featured attribute always gets one, the rest rotate.
            if number == 60 { th.featured } else { Attribute::ALL[(number as usize + set as usize) % 6] }
        } else if rng.chance(0.3) {
            th.featured
        } else {
            *rng.pick(&Attribute::ALL)
        };
        let kind = if rarity == Rarity::Mythic {
            Kind::Monster
        } else {
            let r = rng.f32();
            if r < 0.65 { Kind::Monster } else if r < 0.88 { Kind::Spell } else { Kind::Relic }
        };
        let name = match (kind, rarity) {
            (Kind::Monster, Rarity::Mythic) => {
                let m = mythic_names(attribute);
                format!("{}, {} Sovereign", m[(set as usize + number as usize) % m.len()], rng.pick(th.adjectives))
            }
            (Kind::Monster, _) => unique_name(&mut rng, &mut used, th.adjectives, monsters(attribute)),
            (Kind::Spell, _) => unique_name(&mut rng, &mut used, th.adjectives, spells(attribute)),
            (Kind::Relic, _) => unique_name(&mut rng, &mut used, th.adjectives, relics(attribute)),
        };
        // Hype: rarer and on-theme cards get talked about more, with a random spread so every
        // season has a few surprise "must-have" commons.
        let base = match rarity {
            Rarity::Common => 0.15,
            Rarity::Uncommon => 0.3,
            Rarity::Rare => 0.5,
            Rarity::Mythic => 0.8,
        };
        let theme_bonus = if attribute == th.featured { 0.1 } else { 0.0 };
        let hype = (base + theme_bonus + (rng.f32() - 0.5) * 0.3).clamp(0.02, 1.0);

        let subject = match (kind, rarity) {
            (Kind::Monster, Rarity::Mythic) => format!(
                "{name}, a legendary colossal monster, awe-inspiring, dramatic low angle, glowing aura"),
            (Kind::Monster, _) => format!(
                "a {} creature called \"{name}\", dynamic battle pose, cool and a little cute, full body",
                attribute.to_string().to_lowercase()),
            (Kind::Spell, _) => format!("the magic spell \"{name}\" being cast, a burst of energy, no visible people"),
            (Kind::Relic, _) => format!("the magical item \"{name}\" floating on a pedestal, glowing with power"),
        };
        let art_prompt = format!(
            "Trading card illustration for a fantasy monster card game. Subject: {subject}. \
             Attribute styling: {}. Set theme: {}. Vibrant anime fantasy style, bold rim light, \
             centred subject, detailed atmospheric background. Portrait 3:4 aspect. \
             No text, no lettering, no card border, no logos.",
            attribute.look(), th.visual);
        let flavor = format!("{} — {}", rng.pick(FLAVOR), th.name);
        cards.push(CardDef {
            id,
            name,
            kind,
            attribute,
            rarity,
            hype,
            flavor,
            art_prompt,
            art: format!("S{set:02}/{id}.png"),
        });
    }
    cards
}

const FLAVOR: &[&str] = &[
    "Legends say it only appears to the bravest summoners.",
    "Nobody who has seen it up close agrees on its size.",
    "Small, loud and absolutely convinced it is a dragon.",
    "Its roar can be heard three valleys away.",
    "Ancient, patient and very, very hungry.",
    "Some spells are cast. This one is unleashed.",
    "Forged before the first king, still sharp.",
];

fn unique_name(rng: &mut Rng, used: &mut HashSet<String>, adjs: &[&str], nouns: &[&str]) -> String {
    for _ in 0..64 {
        let n = format!("{} {}", rng.pick(adjs), rng.pick(nouns));
        if used.insert(n.clone()) {
            return n;
        }
    }
    let n = format!("{} {} II", rng.pick(adjs), rng.pick(nouns));
    used.insert(n.clone());
    n
}
