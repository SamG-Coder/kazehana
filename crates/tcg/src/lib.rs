//! "Mythbeasts" (working name), the monster/spell card game the town plays.
//!
//! For now this crate only models what the life sim needs: cards, seasons, packs and
//! collections. The gameplay rules come later.

pub mod battle;
pub mod card;
pub mod db;
pub mod pack;
pub mod rng;
pub mod season;

pub use card::*;
pub use db::CardDb;
pub use pack::{open_pack, Collection, Owned, Pull, PACKS_PER_BOX, PACK_PRICE, PACK_SIZE};
pub use rng::Rng;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seasons_have_sixty_unique_cards() {
        let db = CardDb::generate(12);
        assert_eq!(db.all().len(), 720);
        for set in 1..=12 {
            let names: std::collections::HashSet<_> = db.set(set).map(|c| c.name.clone()).collect();
            assert_eq!(names.len(), 60, "set {set} has duplicate names");
            assert_eq!(db.set_rarity(set, Rarity::Mythic).len(), 5);
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let a = season::generate_season(3);
        let b = season::generate_season(3);
        assert!(a.iter().zip(&b).all(|(x, y)| x.name == y.name && x.hype == y.hype));
    }

    #[test]
    fn pack_odds() {
        let db = CardDb::generate(1);
        let mut rng = Rng::new(7);
        let (mut mythics, mut foils) = (0, 0);
        let n = 8000;
        let mut col = Collection::default();
        for _ in 0..n {
            let p = open_pack(&db, 1, &mut rng);
            assert_eq!(p.len(), PACK_SIZE);
            mythics += p.iter().filter(|x| !x.foil && db.get(x.id).rarity == Rarity::Mythic).count();
            foils += p.iter().filter(|x| x.foil).count();
            p.into_iter().for_each(|x| col.add(x));
        }
        let m = mythics as f32 / n as f32;
        let f = foils as f32 / n as f32;
        assert!((m - 0.125).abs() < 0.02, "mythic rate {m}");
        assert!((f - 1.0 / 6.0).abs() < 0.02, "foil rate {f}");
        assert_eq!(col.total(), (n * PACK_SIZE) as u32);
        assert_eq!(col.unique(), 60);
    }
}
