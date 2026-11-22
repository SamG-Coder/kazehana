//! Everything you can hold, buy, eat, give or find, and the places things are kept: your school
//! bag, your room's shelf, the fridge. The catalog is data (`assets/data/items.json`).

use crate::calendar::Date;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Meal,
    Snack,
    Sweet,
    Drink,
    Ingredient,
    Fruit,
    School,
    Book,
    Card,
    Tool,
    Gift,
    Toiletry,
}

impl Category {
    /// You can eat or drink it.
    pub fn edible(self) -> bool {
        matches!(self, Category::Meal | Category::Snack | Category::Sweet | Category::Drink | Category::Fruit | Category::Ingredient)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub jp: String,
    pub cat: Category,
    /// Yen (0: not sold, you're given it or find it).
    #[serde(default)]
    pub price: u32,
    /// How much it fills you (a meal is about 45 of 100).
    #[serde(default)]
    pub full: f32,
    /// Energy it gives back (negative: takes).
    #[serde(default)]
    pub energy: f32,
    /// How long eating, drinking, reading or using it takes (game minutes).
    #[serde(default)]
    pub minutes: u32,
    /// Days it stays good (0: today only; none: doesn't go off).
    #[serde(default)]
    pub keeps: Option<u16>,
    /// Where it's sold or found: "konbini", "vending", "school", "home", "Tsukumodō",
    /// "drugstore", "co-op", "shrine", "festival", or a shop trade from the census ("bakery"...).
    #[serde(rename = "where", default)]
    pub sold: Vec<String>,
    /// Only in these months (empty: all year).
    #[serde(default)]
    pub months: Vec<u32>,
    #[serde(default)]
    pub desc: String,
    /// The prop it uses in the world, once one is made and approved.
    #[serde(default)]
    pub model: Option<String>,
}

impl Item {
    pub fn in_season(&self, month: u32) -> bool {
        self.months.is_empty() || self.months.contains(&month)
    }
    pub fn sold_at(&self, place: &str) -> bool {
        self.sold.iter().any(|w| w == place)
    }
    /// Gone off on `today`, if bought (made) on `on`.
    pub fn spoiled(&self, on: Date, today: Date) -> bool {
        match self.keeps {
            Some(k) => today.to_days() - on.to_days() > k as i64,
            None => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Catalog {
    pub items: Vec<Item>,
    index: HashMap<String, usize>,
}

impl Catalog {
    pub fn from_json(json: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct File {
            items: Vec<Item>,
        }
        let f: File = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut index = HashMap::new();
        for (i, it) in f.items.iter().enumerate() {
            if index.insert(it.id.clone(), i).is_some() {
                return Err(format!("item {} twice", it.id));
            }
        }
        Ok(Catalog { items: f.items, index })
    }

    /// The game's catalog (built in from `assets/data/items.json`).
    pub fn builtin() -> Self {
        Self::from_json(include_str!("../../../assets/data/items.json")).expect("assets/data/items.json")
    }

    pub fn get(&self, id: &str) -> Option<&Item> {
        self.index.get(id).map(|&i| &self.items[i])
    }

    /// What a place sells this month.
    pub fn sold_at(&self, place: &str, month: u32) -> Vec<&Item> {
        self.items.iter().filter(|i| i.price > 0 && i.sold_at(place) && i.in_season(month)).collect()
    }
}

/// A pile of the same thing, made or bought on one day.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub item: String,
    pub count: u16,
    pub bought: Date,
}

/// Somewhere things are kept: a number of slots, each holding a stack of one thing.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Store {
    pub slots: u16,
    pub stacks: Vec<Stack>,
}

/// How many of one thing fit in a slot.
pub const STACK: u16 = 10;

impl Store {
    pub fn new(slots: u16) -> Self {
        Store { slots, stacks: Vec::new() }
    }

    pub fn count(&self, item: &str) -> u16 {
        self.stacks.iter().filter(|s| s.item == item).map(|s| s.count).sum()
    }

    pub fn free_slots(&self) -> u16 {
        self.slots.saturating_sub(self.stacks.len() as u16)
    }

    /// Whether `n` more of something fit.
    pub fn fits(&self, item: &str, n: u16, on: Date) -> bool {
        let room: u16 = self.stacks.iter().filter(|s| s.item == item && s.bought == on).map(|s| STACK - s.count).sum();
        let need = n.saturating_sub(room);
        need.div_ceil(STACK) <= self.free_slots()
    }

    /// Put things in. Returns false (and puts nothing in) if they don't fit.
    pub fn add(&mut self, item: &str, mut n: u16, on: Date) -> bool {
        if !self.fits(item, n, on) {
            return false;
        }
        for s in self.stacks.iter_mut().filter(|s| s.item == item && s.bought == on) {
            let k = n.min(STACK - s.count);
            s.count += k;
            n -= k;
        }
        while n > 0 {
            let k = n.min(STACK);
            self.stacks.push(Stack { item: item.to_string(), count: k, bought: on });
            n -= k;
        }
        true
    }

    /// Take one out (the oldest first). Returns the day it was bought.
    pub fn take(&mut self, item: &str) -> Option<Date> {
        let i = self.stacks.iter().enumerate().filter(|(_, s)| s.item == item).min_by_key(|(_, s)| s.bought).map(|(i, _)| i)?;
        let on = self.stacks[i].bought;
        self.stacks[i].count -= 1;
        if self.stacks[i].count == 0 {
            self.stacks.remove(i);
        }
        Some(on)
    }

    /// Throw out what's gone off; returns what went.
    pub fn spoil(&mut self, catalog: &Catalog, today: Date) -> Vec<Stack> {
        let (gone, keep): (Vec<Stack>, Vec<Stack>) = self.stacks.drain(..).partition(|s| catalog.get(&s.item).is_some_and(|i| i.spoiled(s.bought, today)));
        self.stacks = keep;
        gone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_is_sound() {
        let c = Catalog::builtin();
        assert!(c.items.len() >= 80, "{} items", c.items.len());
        let food = c.items.iter().filter(|i| i.cat.edible()).count();
        assert!(food >= 50, "{food} things to eat and drink");
        for i in &c.items {
            assert!(!i.sold.is_empty(), "{} isn't sold or found anywhere", i.id);
            assert!(i.months.iter().all(|m| (1..=12).contains(m)), "{}", i.id);
            if i.cat.edible() && i.cat != Category::Ingredient {
                assert!(i.minutes > 0, "{} takes no time to eat", i.id);
            }
            assert!(!i.desc.is_empty() && !i.jp.is_empty(), "{} needs a description and its Japanese name", i.id);
        }
        // the konbini always has something to eat, in any month
        for m in 1..=12 {
            assert!(c.sold_at("konbini", m).iter().filter(|i| i.cat.edible() && i.full >= 15.0).count() >= 5, "month {m}");
        }
        // the shops the census can open sell something
        for trade in ["bakery", "tofu shop", "butcher", "greengrocer", "wagashi (sweets) shop", "fishmonger", "ramen shop", "soba shop", "stationery shop", "bookshop", "florist"] {
            assert!(!c.sold_at(trade, 4).is_empty() || !c.sold_at(trade, 8).is_empty(), "{trade} sells nothing");
        }
    }

    #[test]
    fn bags_fill_and_food_goes_off() {
        let c = Catalog::builtin();
        let d = Date::new(2026, 4, 8);
        let mut bag = Store::new(2);
        assert!(bag.add("onigiri_salmon", 3, d));
        assert!(!bag.add("cola", 12, d), "12 colas need two slots; only one is free");
        assert!(bag.add("cola", 6, d));
        assert!(!bag.fits("melon_pan", 1, d));
        assert_eq!(bag.count("onigiri_salmon"), 3);
        assert_eq!(bag.take("onigiri_salmon"), Some(d));
        // next day the onigiri (keeps a day) is fine; two days on it's gone
        assert!(bag.spoil(&c, d.add_days(1)).is_empty());
        let gone = bag.spoil(&c, d.add_days(2));
        assert_eq!(gone.iter().map(|s| s.item.as_str()).collect::<Vec<_>>(), vec!["onigiri_salmon"]);
        assert_eq!(bag.count("cola"), 6);
    }
}
