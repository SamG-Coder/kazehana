//! Mythbeasts Duel: the 1v1 battle rules (docs/BATTLE_DESIGN.md).
//!
//! A pure, deterministic rules engine: `Duel` holds the whole game, `legal_actions` lists what the
//! player to move may do, `apply` performs one action and returns the `Event`s it caused (for
//! animation and logs). Nothing here knows about rendering.
//!
//! Battle numbers are not stored on `CardDef`: `stats` derives them from rarity, kind, attribute
//! and collector number, so every generated or hand-edited card is playable.

pub mod ai;
pub mod decks;

use crate::card::{Attribute, CardDef, CardId, Kind, Rarity};
use crate::rng::Rng;
use serde::{Deserialize, Serialize};

pub const LEADER_HP: i32 = 20;
pub const MAX_SPARK: i32 = 6;
pub const HAND_MAX: usize = 7;
pub const START_HAND: usize = 4;
pub const ZONES: usize = 3;
pub const TRAP_ZONES: usize = 2;
pub const DECK_SIZE: usize = 20;
/// Extra damage for a hit on a type yours beats.
pub const SUPER_BONUS: i32 = 2;
/// A game still running after this many turns is a draw (never happens in practice).
pub const TURN_LIMIT: u32 = 80;

// ----------------------------------------------------------------------------- cards

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Keyword {
    /// Can attack on the turn it is summoned.
    Swift,
    /// Enemies must attack a Guard first.
    Guard,
    /// Can attack the enemy Leader while the enemy has monsters (unless they have a Guard).
    Flying,
    /// Leftover damage from a knockout hits the defending Leader.
    Pierce,
}

impl Keyword {
    pub fn name(self) -> &'static str {
        match self {
            Keyword::Swift => "Swift",
            Keyword::Guard => "Guard",
            Keyword::Flying => "Flying",
            Keyword::Pierce => "Pierce",
        }
    }
    pub fn rule(self) -> &'static str {
        match self {
            Keyword::Swift => "Can attack the turn it is summoned.",
            Keyword::Guard => "Enemies must attack a Guard first.",
            Keyword::Flying => "Can attack the enemy Leader over other monsters.",
            Keyword::Pierce => "Leftover damage from a knockout hits the enemy Leader.",
        }
    }
}

/// What a spell, trap or relic does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    None,
    // spells (cast from the hand)
    /// Deal damage to any monster or Leader.
    Damage(i32),
    /// Deal damage to every enemy monster.
    Volley(i32),
    /// Heal one of your monsters or your Leader.
    Heal(i32),
    /// One of your monsters gets +ATK this turn and can attack now.
    Rally(i32),
    /// One of your monsters gets +HP for good.
    Fortify(i32),
    /// Draw cards.
    Draw(u32),
    /// Send an enemy monster costing at most this much back to its owner's hand.
    Banish(i32),
    // traps (spring on the opponent's attack)
    /// Deal damage to the attacker.
    TrapBurn(i32),
    /// Cancel the attack.
    TrapNegate,
    /// Send the attacker back to its owner's hand.
    TrapBounce,
    /// The defender gets +HP for good before the hit (or your Leader heals).
    TrapBrace(i32),
    /// Cancel the attack and heal your Leader.
    TrapNegateHeal(i32),
    /// The attacker loses ATK for good.
    TrapWeaken(i32),
    // relics
    Equip { atk: i32, hp: i32, keyword: Option<Keyword> },
}

impl Effect {
    pub fn is_trap(self) -> bool {
        matches!(self, Effect::TrapBurn(_) | Effect::TrapNegate | Effect::TrapBounce | Effect::TrapBrace(_) | Effect::TrapNegateHeal(_) | Effect::TrapWeaken(_))
    }

    /// Rules text for the HUD (cards have no printed battle text yet).
    pub fn text(self) -> String {
        match self {
            Effect::None => String::new(),
            Effect::Damage(n) => format!("Deal {n} damage to a monster or Leader."),
            Effect::Volley(n) => format!("Deal {n} damage to every enemy monster."),
            Effect::Heal(n) => format!("Heal {n} HP on one of your monsters or your Leader."),
            Effect::Rally(n) => format!("One of your monsters gets +{n} ATK this turn and can attack now."),
            Effect::Fortify(n) => format!("One of your monsters gets +{n} HP."),
            Effect::Draw(n) => format!("Draw {n} card{}.", if n == 1 { "" } else { "s" }),
            Effect::Banish(n) => format!("Return an enemy monster costing {n} or less to its owner's hand."),
            Effect::TrapBurn(n) => format!("TRAP - When attacked: deal {n} damage to the attacker."),
            Effect::TrapNegate => "TRAP - When attacked: cancel the attack.".into(),
            Effect::TrapBounce => "TRAP - When attacked: return the attacker to its owner's hand.".into(),
            Effect::TrapBrace(n) => format!("TRAP - When attacked: the defender gets +{n} HP."),
            Effect::TrapNegateHeal(n) => format!("TRAP - When attacked: cancel the attack and heal your Leader {n}."),
            Effect::TrapWeaken(n) => format!("TRAP - When attacked: the attacker gets -{n} ATK."),
            Effect::Equip { atk, hp, keyword } => {
                let kw = keyword.map(|k| format!(" and {}", k.name())).unwrap_or_default();
                format!("Equip: +{atk} ATK, +{hp} HP{kw}.")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ascend {
    /// Summoned normally only.
    No,
    /// May be summoned normally, or Ascend onto one of your monsters for `ascend_cost`.
    May,
    /// Can only arrive by Ascend (Mythics).
    Must,
}

/// A card's battle numbers.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub id: CardId,
    pub kind: Kind,
    pub attribute: Attribute,
    pub rarity: Rarity,
    pub cost: i32,
    pub atk: i32,
    pub hp: i32,
    pub keyword: Option<Keyword>,
    pub ascend: Ascend,
    pub ascend_cost: i32,
    pub effect: Effect,
}

impl Stats {
    pub fn is_trap(&self) -> bool {
        self.kind == Kind::Spell && self.effect.is_trap()
    }
}

/// Does attacker type `a` beat defender type `d`? (Tide > Flame > Terra > Gale > Tide;
/// Radiant and Umbral beat each other.)
pub fn beats(a: Attribute, d: Attribute) -> bool {
    use Attribute::*;
    matches!((a, d), (Tide, Flame) | (Flame, Terra) | (Terra, Gale) | (Gale, Tide) | (Radiant, Umbral) | (Umbral, Radiant))
}

fn card_hash(id: CardId, salt: u32) -> u32 {
    let mut x = (id.set as u32).wrapping_mul(0x9E37_79B9) ^ (id.number as u32).wrapping_mul(0x85EB_CA6B) ^ salt.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^ (x >> 16)
}

fn favourite_keyword(a: Attribute) -> Keyword {
    match a {
        Attribute::Flame => Keyword::Pierce,
        Attribute::Tide => Keyword::Guard,
        Attribute::Gale => Keyword::Flying,
        Attribute::Terra => Keyword::Guard,
        Attribute::Radiant => Keyword::Swift,
        Attribute::Umbral => Keyword::Flying,
    }
}

/// Share of a monster's stat budget that goes to ATK, by type.
fn atk_share(a: Attribute) -> f32 {
    match a {
        Attribute::Flame => 0.62,
        Attribute::Gale => 0.56,
        Attribute::Umbral => 0.55,
        Attribute::Radiant => 0.47,
        Attribute::Tide => 0.44,
        Attribute::Terra => 0.42,
    }
}

/// Battle numbers for a card, derived deterministically (docs/BATTLE_DESIGN.md "Card stats").
pub fn stats(def: &CardDef) -> Stats {
    let h = |salt| card_hash(def.id, salt);
    let tier = match def.rarity {
        Rarity::Common => 0,
        Rarity::Uncommon => 1,
        Rarity::Rare => 2,
        Rarity::Mythic => 3,
    };
    let mut s = Stats {
        id: def.id,
        kind: def.kind,
        attribute: def.attribute,
        rarity: def.rarity,
        cost: 1,
        atk: 0,
        hp: 0,
        keyword: None,
        ascend: Ascend::No,
        ascend_cost: 0,
        effect: Effect::None,
    };
    match def.kind {
        Kind::Monster => {
            let (cost, budget) = match def.rarity {
                Rarity::Common => {
                    if h(1) % 2 == 0 { (1, 3) } else { (2, 5) }
                }
                Rarity::Uncommon => {
                    if h(1) % 2 == 0 { (2, 6) } else { (3, 7) }
                }
                Rarity::Rare => (4, 9),
                Rarity::Mythic => (5, 13),
            };
            let kw_chance = [0.3f32, 0.65, 1.0, 1.0][tier];
            let roll = (h(2) % 1000) as f32 / 1000.0;
            let keyword = if roll < kw_chance {
                Some(if h(3) % 10 < 7 {
                    favourite_keyword(def.attribute)
                } else {
                    [Keyword::Swift, Keyword::Guard, Keyword::Flying, Keyword::Pierce][(h(4) % 4) as usize]
                })
            } else {
                None
            };
            // keywords cost a point on cheap monsters
            let budget = if keyword.is_some_and(|k| k != Keyword::Guard) && tier < 2 { budget - 1 } else { budget };
            let jitter = (h(5) % 3) as i32 - 1;
            let atk = ((budget as f32 * atk_share(def.attribute)).round() as i32 + jitter).clamp(1, budget - 1);
            s.cost = cost;
            s.atk = atk;
            s.hp = budget - atk;
            s.keyword = keyword;
            s.ascend = match def.rarity {
                Rarity::Rare => Ascend::May,
                Rarity::Mythic => Ascend::Must,
                _ => Ascend::No,
            };
            s.ascend_cost = cost - 1;
        }
        Kind::Spell => {
            let t = tier.min(2);
            let trap = h(6) % 3 == 0;
            let (cost, effect) = if trap {
                let n = [2, 3, 4][t];
                (
                    // cancelling a whole attack is worth more than a number
                    match def.attribute {
                        Attribute::Tide | Attribute::Radiant | Attribute::Gale => [2, 2, 3][t],
                        _ => [1, 1, 2][t],
                    },
                    match def.attribute {
                        Attribute::Flame => Effect::TrapBurn(n + 1),
                        Attribute::Tide => Effect::TrapNegate,
                        Attribute::Gale => Effect::TrapBounce,
                        Attribute::Terra => Effect::TrapBrace(n + 1),
                        Attribute::Radiant => Effect::TrapNegateHeal(n),
                        Attribute::Umbral => Effect::TrapWeaken(n - 1),
                    },
                )
            } else {
                match def.attribute {
                    Attribute::Flame if h(8) % 2 == 0 => ([2, 3, 4][t], Effect::Volley([1, 2, 3][t])),
                    Attribute::Flame => ([1, 2, 3][t], Effect::Damage([2, 3, 5][t])),
                    Attribute::Tide => ([1, 2, 3][t], Effect::Heal([3, 5, 7][t])),
                    Attribute::Gale => ([1, 1, 2][t], Effect::Rally([2, 3, 4][t])),
                    Attribute::Terra => ([1, 2, 2][t], Effect::Fortify([3, 4, 6][t])),
                    Attribute::Radiant => ([2, 1, 2][t], Effect::Draw([2, 2, 3][t])),
                    Attribute::Umbral => ([1, 2, 3][t], Effect::Banish([2, 3, 5][t])),
                }
            };
            s.cost = cost;
            s.effect = effect;
        }
        Kind::Relic => {
            let t = tier.min(2);
            let keyword = if t >= 1 {
                match def.attribute {
                    Attribute::Terra => Some(Keyword::Guard),
                    Attribute::Gale => Some(Keyword::Flying),
                    Attribute::Flame => Some(Keyword::Pierce),
                    _ => None,
                }
            } else {
                None
            };
            let (atk, hp) = match (t, keyword.is_some()) {
                (0, _) => if h(7) % 2 == 0 { (1, 1) } else { (2, 0) },
                (1, true) => (1, 1),
                (1, false) => (2, 2),
                (_, true) => (2, 2),
                (_, false) => (3, 3),
            };
            s.cost = [1, 2, 3][t];
            s.effect = Effect::Equip { atk, hp, keyword };
        }
    }
    s
}

// ----------------------------------------------------------------------------- state

/// A monster on the field.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    /// Unique for the whole duel (for animations: a unit keeps its id while it lives).
    pub uid: u32,
    pub card: Stats,
    pub damage: i32,
    /// Permanent changes (Fortify, Weaken, relics are separate).
    pub atk_mod: i32,
    pub hp_mod: i32,
    /// This turn only (Rally).
    pub temp_atk: i32,
    pub relic: Option<Stats>,
    /// Summoned this turn (can't attack unless Swift or Rallied).
    pub sick: bool,
    pub attacked: bool,
}

impl Unit {
    pub fn atk(&self) -> i32 {
        let r = match self.relic.map(|r| r.effect) {
            Some(Effect::Equip { atk, .. }) => atk,
            _ => 0,
        };
        (self.card.atk + self.atk_mod + self.temp_atk + r).max(0)
    }
    pub fn max_hp(&self) -> i32 {
        let r = match self.relic.map(|r| r.effect) {
            Some(Effect::Equip { hp, .. }) => hp,
            _ => 0,
        };
        self.card.hp + self.hp_mod + r
    }
    pub fn hp(&self) -> i32 {
        self.max_hp() - self.damage
    }
    pub fn has(&self, k: Keyword) -> bool {
        self.card.keyword == Some(k) || matches!(self.relic.map(|r| r.effect), Some(Effect::Equip { keyword: Some(kk), .. }) if kk == k)
    }
    pub fn can_attack(&self) -> bool {
        !self.attacked && (!self.sick || self.has(Keyword::Swift)) && self.atk() > 0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Side {
    pub leader: i32,
    pub max_spark: i32,
    pub spark: i32,
    /// Top of the deck is the end.
    pub deck: Vec<Stats>,
    pub hand: Vec<Stats>,
    pub field: [Option<Unit>; ZONES],
    /// Face-down traps.
    pub traps: [Option<Stats>; TRAP_ZONES],
    pub discard: Vec<Stats>,
}

impl Side {
    fn new(deck: Vec<Stats>) -> Side {
        Side { leader: LEADER_HP, max_spark: 0, spark: 0, deck, hand: Vec::new(), field: [None, None, None], traps: [None, None], discard: Vec::new() }
    }
    pub fn units(&self) -> impl Iterator<Item = (usize, &Unit)> {
        self.field.iter().enumerate().filter_map(|(i, u)| u.as_ref().map(|u| (i, u)))
    }
    pub fn has_units(&self) -> bool {
        self.field.iter().any(|u| u.is_some())
    }
    pub fn has_guard(&self) -> bool {
        self.units().any(|(_, u)| u.has(Keyword::Guard))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    Unit { side: usize, zone: usize },
    Leader { side: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Summon the hand card at `card` into the empty `zone`.
    Summon { card: usize, zone: usize },
    /// Play a Rare/Mythic on top of your monster in `zone`.
    Ascend { card: usize, zone: usize },
    /// Cast a spell.
    Cast { card: usize, target: Option<Target> },
    /// Set a trap face down.
    SetTrap { card: usize, slot: usize },
    /// Equip a relic to your monster in `zone`.
    Equip { card: usize, zone: usize },
    Attack { zone: usize, target: Target },
    EndTurn,
}

/// What happened, in order (for animation, sound and the log).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    TurnStart { side: usize, turn: u32, spark: i32 },
    Draw { side: usize, card: CardId },
    /// The deck is empty: no card this turn.
    DeckOut { side: usize },
    Summon { side: usize, zone: usize, uid: u32, card: CardId },
    Ascend { side: usize, zone: usize, uid: u32, card: CardId, from: CardId },
    Cast { side: usize, card: CardId, target: Option<Target> },
    SetTrap { side: usize, slot: usize },
    Equip { side: usize, zone: usize, card: CardId },
    Attack { side: usize, zone: usize, uid: u32, target: Target },
    TrapSprung { side: usize, slot: usize, card: CardId },
    AttackCancelled { side: usize, zone: usize },
    Damage { target: Target, amount: i32, super_hit: bool },
    Heal { target: Target, amount: i32 },
    Buff { side: usize, zone: usize, atk: i32, hp: i32 },
    KnockOut { side: usize, zone: usize, uid: u32, card: CardId },
    /// Returned to its owner's hand (or discarded if the hand is full).
    Returned { side: usize, zone: usize, uid: u32, card: CardId },
    Win { side: Option<usize> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Duel {
    pub sides: [Side; 2],
    /// Whose turn it is.
    pub active: usize,
    /// Turns taken so far, counting both players (1 on the first turn).
    pub turn: u32,
    /// Some(Some(side)) won, Some(None) a draw, None still playing.
    pub result: Option<Option<usize>>,
    next_uid: u32,
    rng: Rng,
}

pub type Rejected = &'static str;

impl Duel {
    /// A new duel: both decks shuffled, 4 cards each, `first` takes turn 1.
    pub fn new(decks: [Vec<Stats>; 2], first: usize, seed: u64) -> (Duel, Vec<Event>) {
        let mut rng = Rng::new(seed);
        let [mut d0, mut d1] = decks;
        rng.shuffle(&mut d0);
        rng.shuffle(&mut d1);
        let mut duel = Duel { sides: [Side::new(d0), Side::new(d1)], active: first, turn: 0, result: None, next_uid: 1, rng };
        let mut ev = Vec::new();
        for s in [first, 1 - first] {
            for _ in 0..START_HAND {
                duel.draw(s, &mut ev);
            }
        }
        duel.start_turn(&mut ev);
        (duel, ev)
    }

    pub fn me(&self) -> &Side {
        &self.sides[self.active]
    }
    pub fn foe(&self) -> &Side {
        &self.sides[1 - self.active]
    }
    pub fn unit(&self, side: usize, zone: usize) -> Option<&Unit> {
        self.sides[side].field.get(zone).and_then(|u| u.as_ref())
    }

    fn draw(&mut self, side: usize, ev: &mut Vec<Event>) {
        let s = &mut self.sides[side];
        match s.deck.pop() {
            Some(c) if s.hand.len() < HAND_MAX => {
                s.hand.push(c);
                ev.push(Event::Draw { side, card: c.id });
            }
            Some(c) => {
                // hand full: the card is burned
                s.discard.push(c);
            }
            None => ev.push(Event::DeckOut { side }),
        }
    }

    fn start_turn(&mut self, ev: &mut Vec<Event>) {
        self.turn += 1;
        let a = self.active;
        let first_turn = self.turn == 1;
        {
            let s = &mut self.sides[a];
            s.max_spark = (s.max_spark + 1).min(MAX_SPARK);
            s.spark = s.max_spark;
            for u in s.field.iter_mut().flatten() {
                u.sick = false;
                u.attacked = false;
                u.temp_atk = 0;
            }
        }
        ev.push(Event::TurnStart { side: a, turn: self.turn, spark: self.sides[a].spark });
        if !first_turn {
            self.draw(a, ev);
        }
        if self.turn > TURN_LIMIT && self.result.is_none() {
            self.result = Some(None);
            ev.push(Event::Win { side: None });
        }
    }

    // ------------------------------------------------------------------ legality

    /// Targets a spell may be cast on (None in the list = no target needed).
    pub fn spell_targets(&self, card: &Stats) -> Vec<Option<Target>> {
        let a = self.active;
        let mine = |d: &Duel| d.sides[a].units().map(|(z, _)| Some(Target::Unit { side: a, zone: z })).collect::<Vec<_>>();
        match card.effect {
            Effect::Damage(_) => {
                let mut v = Vec::new();
                for s in [1 - a, a] {
                    v.extend(self.sides[s].units().map(|(z, _)| Some(Target::Unit { side: s, zone: z })));
                }
                v.push(Some(Target::Leader { side: 1 - a }));
                v
            }
            Effect::Heal(_) => {
                let mut v = mine(self);
                v.push(Some(Target::Leader { side: a }));
                v
            }
            Effect::Rally(_) | Effect::Fortify(_) => mine(self),
            Effect::Draw(_) => vec![None],
            Effect::Volley(_) => if self.sides[1 - a].has_units() { vec![None] } else { Vec::new() },
            Effect::Banish(max) => self.sides[1 - a].units().filter(|(_, u)| u.card.cost <= max).map(|(z, _)| Some(Target::Unit { side: 1 - a, zone: z })).collect(),
            _ => Vec::new(),
        }
    }

    /// Enemy targets the monster in `zone` may attack.
    pub fn attack_targets(&self, zone: usize) -> Vec<Target> {
        let a = self.active;
        let Some(u) = self.unit(a, zone) else { return Vec::new() };
        if !u.can_attack() {
            return Vec::new();
        }
        let foe = &self.sides[1 - a];
        let guard = foe.has_guard();
        let mut v: Vec<Target> = foe.units().filter(|(_, e)| !guard || e.has(Keyword::Guard)).map(|(z, _)| Target::Unit { side: 1 - a, zone: z }).collect();
        if !foe.has_units() || (u.has(Keyword::Flying) && !guard) {
            v.push(Target::Leader { side: 1 - a });
        }
        v
    }

    /// Every legal action for the player to move.
    pub fn legal_actions(&self) -> Vec<Action> {
        let mut v = Vec::new();
        if self.result.is_some() {
            return v;
        }
        let a = self.active;
        let me = &self.sides[a];
        for (i, c) in me.hand.iter().enumerate() {
            match c.kind {
                Kind::Monster => {
                    if c.ascend != Ascend::Must && c.cost <= me.spark {
                        for z in 0..ZONES {
                            if me.field[z].is_none() {
                                v.push(Action::Summon { card: i, zone: z });
                            }
                        }
                    }
                    if c.ascend != Ascend::No && c.ascend_cost <= me.spark {
                        for (z, _) in me.units() {
                            v.push(Action::Ascend { card: i, zone: z });
                        }
                    }
                }
                Kind::Spell if c.cost <= me.spark => {
                    if c.is_trap() {
                        for s in 0..TRAP_ZONES {
                            if me.traps[s].is_none() {
                                v.push(Action::SetTrap { card: i, slot: s });
                            }
                        }
                    } else {
                        for t in self.spell_targets(c) {
                            v.push(Action::Cast { card: i, target: t });
                        }
                    }
                }
                Kind::Relic if c.cost <= me.spark => {
                    for (z, _) in me.units() {
                        v.push(Action::Equip { card: i, zone: z });
                    }
                }
                _ => {}
            }
        }
        for z in 0..ZONES {
            for t in self.attack_targets(z) {
                v.push(Action::Attack { zone: z, target: t });
            }
        }
        v.push(Action::EndTurn);
        v
    }

    pub fn is_legal(&self, a: &Action) -> bool {
        self.legal_actions().contains(a)
    }

    // ------------------------------------------------------------------ apply

    /// Perform one action. Illegal actions are rejected and change nothing.
    pub fn apply(&mut self, action: Action) -> Result<Vec<Event>, Rejected> {
        if self.result.is_some() {
            return Err("the duel is over");
        }
        if !self.is_legal(&action) {
            return Err("not a legal action");
        }
        let a = self.active;
        let mut ev = Vec::new();
        match action {
            Action::Summon { card, zone } => {
                let c = self.sides[a].hand.remove(card);
                self.sides[a].spark -= c.cost;
                let uid = self.uid();
                self.sides[a].field[zone] = Some(Unit { uid, card: c, damage: 0, atk_mod: 0, hp_mod: 0, temp_atk: 0, relic: None, sick: true, attacked: false });
                ev.push(Event::Summon { side: a, zone, uid, card: c.id });
            }
            Action::Ascend { card, zone } => {
                let c = self.sides[a].hand.remove(card);
                self.sides[a].spark -= c.ascend_cost;
                let old = self.sides[a].field[zone].take().unwrap();
                let uid = self.uid();
                self.sides[a].discard.push(old.card);
                if let Some(r) = old.relic {
                    self.sides[a].discard.push(r);
                }
                // An evolved monster can attack this turn if the one beneath could have.
                let sick = old.sick && !old.has(Keyword::Swift);
                self.sides[a].field[zone] = Some(Unit { uid, card: c, damage: 0, atk_mod: 0, hp_mod: 0, temp_atk: 0, relic: None, sick, attacked: old.attacked });
                ev.push(Event::Ascend { side: a, zone, uid, card: c.id, from: old.card.id });
            }
            Action::Cast { card, target } => {
                let c = self.sides[a].hand.remove(card);
                self.sides[a].spark -= c.cost;
                ev.push(Event::Cast { side: a, card: c.id, target });
                self.resolve_spell(c, target, &mut ev);
                self.sides[a].discard.push(c);
            }
            Action::SetTrap { card, slot } => {
                let c = self.sides[a].hand.remove(card);
                self.sides[a].spark -= c.cost;
                self.sides[a].traps[slot] = Some(c);
                ev.push(Event::SetTrap { side: a, slot });
            }
            Action::Equip { card, zone } => {
                let c = self.sides[a].hand.remove(card);
                self.sides[a].spark -= c.cost;
                let u = self.sides[a].field[zone].as_mut().unwrap();
                if let Some(old) = u.relic.replace(c) {
                    self.sides[a].discard.push(old);
                }
                ev.push(Event::Equip { side: a, zone, card: c.id });
            }
            Action::Attack { zone, target } => self.attack(zone, target, &mut ev),
            Action::EndTurn => {
                for u in self.sides[a].field.iter_mut().flatten() {
                    u.temp_atk = 0;
                }
                self.active = 1 - a;
                self.start_turn(&mut ev);
            }
        }
        self.check_result(&mut ev);
        Ok(ev)
    }

    fn uid(&mut self) -> u32 {
        self.next_uid += 1;
        self.next_uid
    }

    fn damage(&mut self, t: Target, amount: i32, super_hit: bool, ev: &mut Vec<Event>) {
        if amount <= 0 {
            return;
        }
        match t {
            Target::Leader { side } => self.sides[side].leader -= amount,
            Target::Unit { side, zone } => {
                if let Some(u) = self.sides[side].field[zone].as_mut() {
                    u.damage += amount;
                }
            }
        }
        ev.push(Event::Damage { target: t, amount, super_hit });
    }

    fn heal(&mut self, t: Target, amount: i32, ev: &mut Vec<Event>) {
        let healed = match t {
            Target::Leader { side } => {
                let s = &mut self.sides[side];
                let before = s.leader;
                s.leader = (s.leader + amount).min(LEADER_HP);
                s.leader - before
            }
            Target::Unit { side, zone } => match self.sides[side].field[zone].as_mut() {
                Some(u) => {
                    let h = u.damage.min(amount);
                    u.damage -= h;
                    h
                }
                None => 0,
            },
        };
        if healed > 0 {
            ev.push(Event::Heal { target: t, amount: healed });
        }
    }

    /// Remove monsters at 0 HP (relics go with them).
    fn sweep(&mut self, ev: &mut Vec<Event>) {
        for side in 0..2 {
            for zone in 0..ZONES {
                let dead = self.sides[side].field[zone].as_ref().is_some_and(|u| u.hp() <= 0);
                if dead {
                    let u = self.sides[side].field[zone].take().unwrap();
                    ev.push(Event::KnockOut { side, zone, uid: u.uid, card: u.card.id });
                    self.sides[side].discard.push(u.card);
                    if let Some(r) = u.relic {
                        self.sides[side].discard.push(r);
                    }
                }
            }
        }
    }

    fn return_to_hand(&mut self, side: usize, zone: usize, ev: &mut Vec<Event>) {
        if let Some(u) = self.sides[side].field[zone].take() {
            ev.push(Event::Returned { side, zone, uid: u.uid, card: u.card.id });
            let s = &mut self.sides[side];
            if s.hand.len() < HAND_MAX {
                s.hand.push(u.card);
            } else {
                s.discard.push(u.card);
            }
            if let Some(r) = u.relic {
                s.discard.push(r);
            }
        }
    }

    fn resolve_spell(&mut self, c: Stats, target: Option<Target>, ev: &mut Vec<Event>) {
        let a = self.active;
        match (c.effect, target) {
            (Effect::Damage(n), Some(t)) => {
                self.damage(t, n, false, ev);
                self.sweep(ev);
            }
            (Effect::Volley(n), _) => {
                for (z, _) in self.sides[1 - a].units().map(|(z, u)| (z, u.uid)).collect::<Vec<_>>() {
                    self.damage(Target::Unit { side: 1 - a, zone: z }, n, false, ev);
                }
                self.sweep(ev);
            }
            (Effect::Heal(n), Some(t)) => self.heal(t, n, ev),
            (Effect::Rally(n), Some(Target::Unit { side, zone })) => {
                if let Some(u) = self.sides[side].field[zone].as_mut() {
                    u.temp_atk += n;
                    u.sick = false;
                    ev.push(Event::Buff { side, zone, atk: n, hp: 0 });
                }
            }
            (Effect::Fortify(n), Some(Target::Unit { side, zone })) => {
                if let Some(u) = self.sides[side].field[zone].as_mut() {
                    u.hp_mod += n;
                    ev.push(Event::Buff { side, zone, atk: 0, hp: n });
                }
            }
            (Effect::Draw(n), _) => {
                for _ in 0..n {
                    self.draw(a, ev);
                }
            }
            (Effect::Banish(_), Some(Target::Unit { side, zone })) => self.return_to_hand(side, zone, ev),
            _ => {}
        }
    }

    fn attack(&mut self, zone: usize, target: Target, ev: &mut Vec<Event>) {
        let a = self.active;
        let d = 1 - a;
        let uid = {
            let u = self.sides[a].field[zone].as_mut().unwrap();
            u.attacked = true;
            u.uid
        };
        ev.push(Event::Attack { side: a, zone, uid, target });

        // The defender's first face-down trap springs.
        if let Some(slot) = (0..TRAP_ZONES).find(|&s| self.sides[d].traps[s].is_some()) {
            let trap = self.sides[d].traps[slot].take().unwrap();
            ev.push(Event::TrapSprung { side: d, slot, card: trap.id });
            self.sides[d].discard.push(trap);
            let me = Target::Unit { side: a, zone };
            match trap.effect {
                Effect::TrapBurn(n) => {
                    self.damage(me, n, false, ev);
                    self.sweep(ev);
                    if self.sides[a].field[zone].is_none() {
                        return;
                    }
                }
                Effect::TrapNegate => {
                    ev.push(Event::AttackCancelled { side: a, zone });
                    return;
                }
                Effect::TrapBounce => {
                    self.return_to_hand(a, zone, ev);
                    return;
                }
                Effect::TrapBrace(n) => match target {
                    Target::Unit { side, zone: z } => {
                        if let Some(u) = self.sides[side].field[z].as_mut() {
                            u.hp_mod += n;
                            ev.push(Event::Buff { side, zone: z, atk: 0, hp: n });
                        }
                    }
                    Target::Leader { side } => self.heal(Target::Leader { side }, n, ev),
                },
                Effect::TrapNegateHeal(n) => {
                    ev.push(Event::AttackCancelled { side: a, zone });
                    self.heal(Target::Leader { side: d }, n, ev);
                    return;
                }
                Effect::TrapWeaken(n) => {
                    let u = self.sides[a].field[zone].as_mut().unwrap();
                    u.atk_mod -= n;
                    ev.push(Event::Buff { side: a, zone, atk: -n, hp: 0 });
                }
                _ => {}
            }
        }

        let att = self.sides[a].field[zone].clone().unwrap();
        match target {
            Target::Leader { side } => {
                let n = att.atk();
                self.damage(Target::Leader { side }, n, false, ev);
            }
            Target::Unit { side, zone: dz } => {
                let Some(def) = self.sides[side].field[dz].clone() else { return };
                let sup_a = beats(att.card.attribute, def.card.attribute);
                let sup_d = beats(def.card.attribute, att.card.attribute);
                let to_def = att.atk() + if sup_a { SUPER_BONUS } else { 0 };
                let to_att = def.atk() + if sup_d { SUPER_BONUS } else { 0 };
                let leftover = to_def - def.hp();
                // both hits land at once
                self.damage(target, to_def, sup_a, ev);
                self.damage(Target::Unit { side: a, zone }, to_att, sup_d, ev);
                if att.has(Keyword::Pierce) && leftover > 0 {
                    self.damage(Target::Leader { side }, leftover, false, ev);
                }
                self.sweep(ev);
            }
        }
    }

    fn check_result(&mut self, ev: &mut Vec<Event>) {
        if self.result.is_some() {
            return;
        }
        let dead = [self.sides[0].leader <= 0, self.sides[1].leader <= 0];
        let r = match dead {
            [true, true] => Some(None),
            [true, false] => Some(Some(1)),
            [false, true] => Some(Some(0)),
            _ => None,
        };
        if let Some(w) = r {
            self.result = Some(w);
            ev.push(Event::Win { side: w });
        }
    }

    /// A copy for `viewer` to think with: the opponent's face-down traps are removed (the AI only
    /// ever enumerates its own hand; the opponent's hand is used for its size alone).
    pub fn view_for(&self, viewer: usize) -> Duel {
        let mut d = self.clone();
        d.sides[1 - viewer].traps = [None, None];
        d
    }

    /// Face-down traps `side` has set (visible as backs to the other player).
    pub fn trap_count(&self, side: usize) -> usize {
        self.sides[side].traps.iter().filter(|t| t.is_some()).count()
    }
}

#[cfg(test)]
mod tests;
