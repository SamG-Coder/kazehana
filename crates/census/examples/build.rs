//! Make the town's people and write them out:
//!     cargo run -p census --example build [seed]
//! -> assets/town/census.json (the database), docs/census.md (the same, house by house)
use std::path::Path;

fn main() -> std::io::Result<()> {
    let seed = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(7u64);
    let plan = census::Plan::load(Path::new("assets/town/town.json"))?;
    let c = census::generate(seed, &plan);
    c.save(Path::new("assets/town/census.json"))?;
    std::fs::write("docs/census.md", census::doc::markdown(&c))?;
    let live = c.residents().count();
    println!("{live} people in town ({} on record with family away), {} households, {} groups", c.people.len() - live, c.households.len(), c.groups.len());
    Ok(())
}
