//! The player's card binder: which card sits in which sleeve. A ring binder holds refill sheets of
//! nine top-loading pockets; each pocket takes two cards back to back, one showing on each side
//! of the sheet. The cards themselves stay in `Player::collection`: a sleeved card is still an
//! owned card, and the binder only says where it is.
//!
//! Pages: page `2 * s` is the front of sheet `s`, page `2 * s + 1` its back. Opened at spread `k`,
//! the left page is the back of sheet `k - 1` and the right page the front of sheet `k`.

use serde::{Deserialize, Serialize};
use tcg::{Collection, Pull};

pub const POCKETS: usize = 9;
/// Refill sheets in a new binder.
pub const DEFAULT_SHEETS: usize = 10;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Sheet {
    /// The cards facing out of the front, pocket by pocket (row by row from the top left).
    pub front: [Option<Pull>; POCKETS],
    /// The cards facing out of the back, as seen from the back (pocket 0 is then top left too).
    pub back: [Option<Pull>; POCKETS],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Binder {
    /// Which binder this is (its model; covers and brands come with the story).
    pub model: String,
    pub sheets: Vec<Sheet>,
}

impl Default for Binder {
    fn default() -> Self {
        Binder { model: "binder_ring4".into(), sheets: vec![Sheet::default(); DEFAULT_SHEETS] }
    }
}

impl Binder {
    pub fn pages(&self) -> usize {
        self.sheets.len() * 2
    }

    /// Spreads: 0 shows only the first sheet's front (on the right), `sheets` only the last back.
    pub fn spreads(&self) -> usize {
        self.sheets.len() + 1
    }

    pub fn page(&self, page: usize) -> Option<&[Option<Pull>; POCKETS]> {
        let s = self.sheets.get(page / 2)?;
        Some(if page % 2 == 0 { &s.front } else { &s.back })
    }

    fn page_mut(&mut self, page: usize) -> Option<&mut [Option<Pull>; POCKETS]> {
        let s = self.sheets.get_mut(page / 2)?;
        Some(if page % 2 == 0 { &mut s.front } else { &mut s.back })
    }

    pub fn get(&self, page: usize, pocket: usize) -> Option<Pull> {
        self.page(page).and_then(|p| p.get(pocket).copied().flatten())
    }

    /// How many copies of `pull` (same card, same finish) are sleeved.
    pub fn sleeved(&self, pull: Pull) -> u32 {
        self.sheets.iter().flat_map(|s| s.front.iter().chain(s.back.iter())).filter(|x| **x == Some(pull)).count() as u32
    }

    pub fn total(&self) -> u32 {
        self.sheets.iter().flat_map(|s| s.front.iter().chain(s.back.iter())).filter(|x| x.is_some()).count() as u32
    }

    /// Copies of `pull` owned and not in a sleeve.
    pub fn loose_count(&self, col: &Collection, pull: Pull) -> u32 {
        let owned = col.get(pull.id).map_or(0, |o| if pull.foil { o.foil } else { o.normal });
        owned.saturating_sub(self.sleeved(pull))
    }

    /// Every owned card that is not in a sleeve, with how many loose copies (collection order).
    pub fn loose(&self, col: &Collection) -> Vec<(Pull, u32)> {
        let mut out = Vec::new();
        for o in col.iter() {
            for foil in [true, false] {
                let p = Pull { id: o.id, foil };
                let n = self.loose_count(col, p);
                if n > 0 {
                    out.push((p, n));
                }
            }
        }
        out
    }

    /// Slide a loose card into an empty pocket. False if the pocket is taken, doesn't exist, or
    /// no loose copy is owned.
    pub fn put(&mut self, col: &Collection, page: usize, pocket: usize, pull: Pull) -> bool {
        if self.loose_count(col, pull) == 0 || pocket >= POCKETS {
            return false;
        }
        match self.page_mut(page) {
            Some(p) if p[pocket].is_none() => {
                p[pocket] = Some(pull);
                true
            }
            _ => false,
        }
    }

    /// Pull a card out of its pocket (it goes back to the loose cards).
    pub fn take(&mut self, page: usize, pocket: usize) -> Option<Pull> {
        self.page_mut(page).and_then(|p| p.get_mut(pocket).and_then(Option::take))
    }

    /// After cards leave the collection (sold, traded): empty sleeves that hold more copies than
    /// are still owned, from the back of the binder forwards. Returns how many were emptied.
    pub fn reconcile(&mut self, col: &Collection) -> u32 {
        let mut emptied = 0;
        for page in (0..self.pages()).rev() {
            for pocket in (0..POCKETS).rev() {
                let Some(pull) = self.get(page, pocket) else { continue };
                let owned = col.get(pull.id).map_or(0, |o| if pull.foil { o.foil } else { o.normal });
                if self.sleeved(pull) > owned {
                    self.take(page, pocket);
                    emptied += 1;
                }
            }
        }
        emptied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcg::CardId;

    fn pull(n: u16, foil: bool) -> Pull {
        Pull { id: CardId { set: 1, number: n }, foil }
    }

    #[test]
    fn sleeves_hold_only_owned_loose_cards() {
        let mut col = Collection::default();
        col.add(pull(3, false));
        col.add(pull(3, false));
        col.add(pull(3, true));
        let mut b = Binder::default();
        assert!(b.put(&col, 0, 0, pull(3, false)));
        assert!(!b.put(&col, 0, 0, pull(3, false)), "pocket taken");
        assert!(b.put(&col, 1, 0, pull(3, false)), "the same pocket's other side");
        assert!(!b.put(&col, 2, 4, pull(3, false)), "no loose copy left");
        assert!(b.put(&col, 2, 4, pull(3, true)));
        assert!(b.loose(&col).is_empty());
        assert_eq!(b.total(), 3);
        assert_eq!(b.take(0, 0), Some(pull(3, false)));
        assert_eq!(b.loose(&col), vec![(pull(3, false), 1)]);
    }

    #[test]
    fn selling_a_sleeved_card_empties_its_sleeve() {
        let mut col = Collection::default();
        col.add(pull(7, false));
        let mut b = Binder::default();
        assert!(b.put(&col, 5, 8, pull(7, false)));
        col.remove(pull(7, false));
        assert_eq!(b.reconcile(&col), 1);
        assert_eq!(b.total(), 0);
    }

    #[test]
    fn saves_and_loads() {
        let mut col = Collection::default();
        col.add(pull(1, true));
        let mut b = Binder::default();
        b.put(&col, 3, 2, pull(1, true));
        let s = serde_json::to_string(&b).unwrap();
        let c: Binder = serde_json::from_str(&s).unwrap();
        assert_eq!(b, c);
    }
}
