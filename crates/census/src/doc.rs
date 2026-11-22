//! The census as a document (`docs/census.md`): the town in numbers, the schools, the
//! workplaces, the groups, then every house and who lives there.

use crate::schedule::{hm, Day};
use crate::*;
use std::fmt::Write;

/// A school day at the start of the year (Wednesday 8 April 2026).
pub const SAMPLE_DAY: Day = Day { weekday: 2, month: 4, date: 8, school_day: true };

fn kind_name(k: HouseKind) -> &'static str {
    match k {
        HouseKind::ShopHome => "Tsukumodō (shop and home)",
        HouseKind::Farmhouse => "farmhouse",
        HouseKind::Modern => "modern house",
        HouseKind::Small => "small house",
        HouseKind::Shophouse => "shophouse",
        HouseKind::Apartment => "apartment block",
    }
}

/// How someone is related to the head of their household.
fn relation_to_head(c: &Census, head: PersonId, p: PersonId) -> String {
    if p == head {
        return "head".into();
    }
    let sex = c.person(p).sex;
    c.relatives(head).into_iter().find(|(r, _)| *r == p).map(|(_, k)| k.word(sex).to_string()).unwrap_or_else(|| "lodger".into())
}

/// A typical school-day, as the places someone passes through.
pub fn day_line(c: &Census, p: PersonId, day: Day) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut last = String::new();
    for h in [6u16, 7, 8, 10, 12, 14, 16, 17, 18, 19, 21] {
        let a = c.activity(p, day, hm(h, 0));
        let at = match a.place {
            Some(pl) if c.home(p) == Some(pl) => "home".to_string(),
            Some(pl) => c.place(pl).name.clone(),
            None => "out".into(),
        };
        let s = format!("{} ({})", a.doing, at);
        if s != last {
            out.push(format!("{h}:00 {}", a.doing.to_string() + if at == "home" || at == "out" { "" } else { " @ " } + if at == "home" || at == "out" { "" } else { &at }));
            last = s;
        }
    }
    out.join(" → ")
}

fn pct(a: usize, b: usize) -> String {
    format!("{:.0}%", 100.0 * a as f32 / b.max(1) as f32)
}

pub fn markdown(c: &Census) -> String {
    let mut s = String::new();
    let res: Vec<&Person> = c.residents().collect();
    let n = res.len();
    let _ = writeln!(s, "# Kazehana-chō census\n");
    let _ = writeln!(s, "Generated from seed {} and the town plan by `cargo run -p census --example build` (the data: `assets/town/census.json`; how the town is made up: `docs/population_plan.md`). Ages are on 1 April 2026.\n", c.seed);

    // --- numbers
    let _ = writeln!(s, "## The town in numbers\n");
    let away = c.people.iter().filter(|p| p.away.as_deref().is_some_and(|w| w != "deceased")).count();
    let hhs = c.households.iter().filter(|h| !h.members.iter().all(|&m| !c.person(m).lives_here())).count();
    let _ = writeln!(s, "- **{n}** people live in town, in **{hhs}** households ({:.2} a household), in **{}** houses.", n as f32 / hhs.max(1) as f32, c.houses.len());
    let _ = writeln!(s, "- **{away}** more are family who live away (the cities, university), on record as relatives.");
    let old = res.iter().filter(|p| p.age >= 65).count();
    let kids = res.iter().filter(|p| p.age < 15).count();
    let _ = writeln!(s, "- 65 and over: {} ({old}); under 15: {} ({kids}).\n", pct(old, n), pct(kids, n));
    let bands: [(u8, u8, &str); 10] = [(0, 5, "0–5"), (6, 11, "6–11 (elementary)"), (12, 14, "12–14 (junior high)"), (15, 17, "15–17 (high school)"), (18, 29, "18–29"), (30, 44, "30–44"), (45, 64, "45–64"), (65, 74, "65–74"), (75, 84, "75–84"), (85, 120, "85+")];
    let _ = writeln!(s, "| Age | Men | Women | All | |\n|---|---:|---:|---:|---|");
    for (a, b, name) in bands {
        let m = res.iter().filter(|p| (a..=b).contains(&p.age) && p.sex == Sex::M).count();
        let f = res.iter().filter(|p| (a..=b).contains(&p.age) && p.sex == Sex::F).count();
        let _ = writeln!(s, "| {name} | {m} | {f} | {} | {} |", m + f, "█".repeat(((m + f) as f32 / n.max(1) as f32 * 100.0).round() as usize));
    }
    let _ = writeln!(s);
    // household types
    let mut kinds: std::collections::BTreeMap<&str, usize> = Default::default();
    for h in &c.households {
        let live: Vec<&Person> = h.members.iter().map(|&m| c.person(m)).filter(|p| p.lives_here()).collect();
        let gens = {
            let mut v = 0;
            if live.iter().any(|p| p.age >= 60) { v += 1; }
            if live.iter().any(|p| (25..60).contains(&p.age)) { v += 1; }
            if live.iter().any(|p| p.age < 25) { v += 1; }
            v
        };
        let k = match (live.len(), gens) {
            (0, _) => "empty (family away)",
            (1, _) if live[0].age >= 65 => "one person, 65+",
            (1, _) => "one person",
            (2, _) if live.iter().all(|p| p.age >= 60) => "older couple",
            (_, 3) => "three generations",
            _ if live.iter().any(|p| p.age < 18) => "parents and children",
            _ if live.iter().any(|p| p.age < 40) => "parents and grown children",
            _ => "other adults",
        };
        *kinds.entry(k).or_default() += 1;
    }
    let _ = writeln!(s, "| Household | Count |\n|---|---:|");
    for (k, v) in kinds {
        let _ = writeln!(s, "| {k} | {v} |");
    }
    let _ = writeln!(s);
    // districts
    let mut districts: Vec<String> = c.houses.iter().map(|h| h.district.clone()).collect();
    districts.sort();
    districts.dedup();
    let _ = writeln!(s, "| District | Houses | People | 65+ | Main families |\n|---|---:|---:|---:|---|");
    for d in &districts {
        let houses: Vec<&House> = c.houses.iter().filter(|h| &h.district == d).collect();
        let people: Vec<&Person> = res.iter().copied().filter(|p| p.household.is_some_and(|h| c.house(c.household(h).house).district == *d)).collect();
        let mut fams: std::collections::HashMap<&str, usize> = Default::default();
        for h in houses.iter().flat_map(|h| &h.households) {
            *fams.entry(c.household(*h).family_name.as_str()).or_default() += 1;
        }
        let mut f: Vec<(&str, usize)> = fams.into_iter().collect();
        f.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let main: Vec<String> = f.iter().take(3).map(|(n, k)| format!("{n} ({k})")).collect();
        let _ = writeln!(s, "| {d} | {} | {} | {} | {} |", houses.len(), people.len(), pct(people.iter().filter(|p| p.age >= 65).count(), people.len()), main.join(", "));
    }
    let _ = writeln!(s);

    // --- schools
    let _ = writeln!(s, "## Kazehana Junior High\n");
    for grade in 1..=3u8 {
        for class in ['A', 'B'] {
            let mut kids: Vec<&Person> = res.iter().copied().filter(|p| matches!(&p.work, Work::JuniorHigh { grade: g, class: k, .. } if *g == grade && *k == class) || (p.work == Work::Player && grade == 1 && class == 'A')).collect();
            kids.sort_by(|a, b| a.family_name.cmp(&b.family_name).then(a.given_name.cmp(&b.given_name)));
            let teacher = res.iter().find(|p| matches!(&p.work, Work::Teacher { school: PlaceKind::JuniorHigh, role } if role.contains(&format!("{grade}-{class}")))).map(|p| p.name()).unwrap_or_default();
            let _ = writeln!(s, "**{grade}-{class}** ({} students; homeroom: {teacher}): {}\n", kids.len(), kids.iter().map(|k| {
                let club = match &k.work { Work::JuniorHigh { club: Some(c), .. } => format!(" ({})", c.name()), _ => String::new() };
                format!("{}{club}", k.name())
            }).collect::<Vec<_>>().join(", "));
        }
    }
    for (school, name) in [(PlaceKind::JuniorHigh, "Junior high staff"), (PlaceKind::Elementary, "Elementary school staff")] {
        let staff: Vec<String> = res.iter().filter(|p| matches!(&p.work, Work::Teacher { school: s, .. } if *s == school)).map(|p| format!("{} ({})", p.name(), match &p.work { Work::Teacher { role, .. } => role.as_str(), _ => "" })).collect();
        let _ = writeln!(s, "**{name}** ({}): {}\n", staff.len(), staff.join("; "));
    }

    // --- workplaces
    let _ = writeln!(s, "## Who works where\n");
    for pl in &c.places {
        if matches!(pl.kind, PlaceKind::Home | PlaceKind::Fields) {
            continue;
        }
        let staff: Vec<String> = res.iter().filter(|p| p.workplace == Some(pl.id) && !matches!(p.work, Work::Teacher { .. })).map(|p| {
            let h = p.hours.map(|(a, b)| format!(" {}:{:02}–{}:{:02}", a / 60, a % 60, b / 60, b % 60)).unwrap_or_default();
            format!("{}{h}", p.name())
        }).collect();
        if !staff.is_empty() {
            let _ = writeln!(s, "- **{}**: {}", pl.name, staff.join("; "));
        }
    }
    let shut: Vec<String> = c.houses.iter().filter_map(|h| h.shop.as_ref().filter(|s| !s.open).map(|s| c.place(s.place).name.clone())).collect();
    let _ = writeln!(s, "- **Closed shops** (shutters down, the family still upstairs): {}\n", shut.join(", "));
    let commuters = res.iter().filter(|p| p.work == Work::Commuter).count();
    let farmers = res.iter().filter(|p| p.work == Work::Farmer).count();
    let _ = writeln!(s, "{commuters} people work in Minato-shi (the train or the car); {farmers} farm.\n");

    // --- groups
    let _ = writeln!(s, "## Groups\n");
    for g in &c.groups {
        if g.kind == GroupKind::Han {
            continue;
        }
        let when = g.meets.as_ref().map(|m| {
            let days: Vec<&str> = (0..7).filter(|d| m.days & (1 << d) != 0).map(|d| ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][d]).collect();
            format!(" — {} {}, {}:{:02}–{}:{:02} at {}", if m.monthly { "first" } else { "" }, if days.len() == 7 { "daily".to_string() } else { days.join("/") }, m.from / 60, m.from % 60, m.to / 60, m.to % 60, c.place(m.place).name)
        }).unwrap_or_default();
        let _ = writeln!(s, "- **{}** ({}){}: {}", g.name, g.members.len(), when, g.members.iter().map(|&m| c.person(m).name()).collect::<Vec<_>>().join(", "));
    }
    let _ = writeln!(s, "\nNeighbourhood blocks (han, about ten households each): {}.\n", c.groups.iter().filter(|g| g.kind == GroupKind::Han).count());

    // --- houses
    let _ = writeln!(s, "## Houses\n");
    let _ = writeln!(s, "A school day (Wednesday 8 April) for each person; relatives in other houses in town.\n");
    for d in &districts {
        let _ = writeln!(s, "### {d}\n");
        for h in c.houses.iter().filter(|h| &h.district == d) {
            let shop = h.shop.as_ref().map(|sh| format!(", {} ({})", sh.trade, if sh.open { "open" } else { "closed" })).unwrap_or_default();
            let _ = writeln!(s, "#### House {} — {}{} — {}\n", h.id, kind_name(h.kind), shop, c.place(h.place).name);
            for &hh in &h.households {
                let hhd = c.household(hh);
                let head = hhd.members.iter().copied().find(|&m| c.person(m).lives_here()).unwrap_or(hhd.members[0]);
                let _ = writeln!(s, "{} family ({}):\n", hhd.family_name, c.group(hhd.han).name);
                let _ = writeln!(s, "| Name | Age | | Does | A school day | Relatives in town |\n|---|---:|---|---|---|---|");
                for &m in &hhd.members {
                    let p = c.person(m);
                    let rel = relation_to_head(c, head, m);
                    let (title, day) = match &p.away {
                        Some(w) if w == "deceased" => continue,
                        Some(w) => (format!("away: {w}"), String::new()),
                        None => (p.title.clone(), day_line(c, m, SAMPLE_DAY)),
                    };
                    let kin: Vec<String> = c.relatives_elsewhere_in_town(m).into_iter().take(6).map(|(r, k)| {
                        let rp = c.person(r);
                        format!("{} {} (house {})", k.word(rp.sex), rp.name(), c.household(rp.household.unwrap()).house)
                    }).collect();
                    let _ = writeln!(s, "| {} | {} | {rel} | {title} | {day} | {} |", p.name(), p.age, kin.join("; "));
                }
                let _ = writeln!(s);
            }
        }
    }
    s
}
