//! Write who lives where (assets/town/households.json) for the town map:
//!     cargo run -p sim --example households [seed]
use sim::town::{load_houses, Town};
use std::path::Path;

fn main() -> std::io::Result<()> {
    let seed = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(7u64);
    let houses = load_houses(Path::new("assets/town/town.json"))?;
    let db = tcg::CardDb::generate(1);
    let mut town = Town::new(seed, houses);
    let students = sim::people::generate_students_in(seed, &db, &mut town);
    let out = serde_json::json!({
        "seed": seed,
        "population": town.population(),
        "home": town.home,
        "households": town.households,
        "students": students.iter().map(|s| serde_json::json!({
            "id": s.id, "name": s.full_name(), "class": s.class_name(), "household": s.household,
            "house": town.household(s.household).house, "district": town.house_of(s.household).district,
        })).collect::<Vec<_>>(),
    });
    std::fs::write("assets/town/households.json", serde_json::to_vec_pretty(&out)?)?;
    println!("{} people in {} households; {} students", town.population(), town.households.len(), students.len());
    Ok(())
}
