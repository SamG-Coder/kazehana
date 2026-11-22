//! Duel AI: plays one action at a time. It tries every legal action on a copy of the game
//! (without the opponent's face-down traps, which it can't see), scores the result, and takes
//! the best if it beats ending the turn. Hard also plays out the opponent's best attacks after
//! its own turn before scoring.

use super::{Action, Duel, Keyword, Unit, LEADER_HP};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Noisy scores: makes small mistakes.
    Easy,
    Normal,
    /// Also looks at the opponent's best attacks in reply.
    Hard,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Easy => "Easy",
            Level::Normal => "Normal",
            Level::Hard => "Hard",
        }
    }
}

fn leader_value(hp: i32) -> f32 {
    let hp = hp as f32;
    // losing the last few HP matters more than the first few
    hp + if hp < 10.0 { -(10.0 - hp).powi(2) * 0.08 } else { 0.0 }
}

fn unit_value(u: &Unit) -> f32 {
    let kw = [(Keyword::Guard, 0.8), (Keyword::Flying, 1.2), (Keyword::Pierce, 0.6), (Keyword::Swift, 0.2)]
        .iter()
        .filter(|(k, _)| u.has(*k))
        .map(|(_, v)| v)
        .sum::<f32>();
    let hp = u.hp().max(0) as f32;
    2.0 + u.atk() as f32 * 1.1 + hp * 0.9 + kw
}

/// How good the position is for `me` (higher is better).
pub fn evaluate(d: &Duel, me: usize) -> f32 {
    match d.result {
        Some(Some(w)) => return if w == me { 10_000.0 } else { -10_000.0 },
        Some(None) => return 0.0,
        None => {}
    }
    let (m, o) = (&d.sides[me], &d.sides[1 - me]);
    let mut s = leader_value(m.leader) - leader_value(o.leader) * 1.15;
    s += m.units().map(|(_, u)| unit_value(u)).sum::<f32>();
    s -= o.units().map(|(_, u)| unit_value(u)).sum::<f32>();
    s += (m.hand.len() as f32 - o.hand.len() as f32) * 0.7;
    s += m.traps.iter().flatten().count() as f32 * 1.5;
    // an empty board leaves the Leader open to every attacker
    if !m.has_units() {
        s -= o.units().map(|(_, u)| u.atk() as f32).sum::<f32>() * 0.5;
    }
    if !o.has_units() {
        s += m.units().map(|(_, u)| u.atk() as f32).sum::<f32>() * 0.3;
    }
    let _ = LEADER_HP;
    s
}

/// Play out the opponent's best attacks (they move after `me` ends the turn).
fn reply_attacks(d: &Duel, me: usize) -> Duel {
    let mut g = d.clone();
    if g.active == me && g.result.is_none() {
        let _ = g.apply(Action::EndTurn);
    }
    for _ in 0..6 {
        if g.result.is_some() {
            break;
        }
        let them = g.active;
        let mut best: Option<(f32, Duel)> = None;
        for a in g.legal_actions() {
            if !matches!(a, Action::Attack { .. }) {
                continue;
            }
            let mut c = g.clone();
            if c.apply(a).is_ok() {
                let v = evaluate(&c, them);
                if best.as_ref().map_or(true, |b| v > b.0) {
                    best = Some((v, c));
                }
            }
        }
        match best {
            Some((v, c)) if v > evaluate(&g, them) => g = c,
            _ => break,
        }
    }
    g
}

fn score(d: &Duel, me: usize, level: Level) -> f32 {
    match level {
        Level::Hard => 0.5 * evaluate(d, me) + 0.5 * evaluate(&reply_attacks(d, me), me),
        _ => evaluate(d, me),
    }
}

/// The AI's next action for the player to move.
pub fn choose(duel: &Duel, level: Level, rng: &mut Rng) -> Action {
    let me = duel.active;
    let view = duel.view_for(me);
    let stay = score(&view, me, level);
    let mut best = (stay + 0.05, Action::EndTurn);
    for a in view.legal_actions() {
        if a == Action::EndTurn {
            continue;
        }
        let mut c = view.clone();
        if c.apply(a).is_err() {
            continue;
        }
        let mut v = score(&c, me, level);
        if level == Level::Easy {
            v += (rng.f32() - 0.5) * 4.0;
        }
        if v > best.0 {
            best = (v, a);
        }
    }
    best.1
}
