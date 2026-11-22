//! cardgen: renders the Mythbeasts card templates.
//!
//! For every rarity/finish template it writes layered PNGs (base colour, foil mask, emboss
//! height, spec) plus static previews with the foil simulated, then a contact sheet.
//!
//! Usage: cardgen [--game NAME] [--out DIR] [--art DIR] [--card S01-023 --finish holo]
//! Run from tools/cardgen (paths default to the repo's assets folders).

mod art;
mod canvas;
mod design;
mod math;
mod motifs;
mod gltfw;
mod pack;
mod props;
mod playmat;
mod sdf;
mod shade;
mod text;

use canvas::{write_gray16, write_rgb, write_rgba, Mask};
use design::{Finish, FrontSpec};
use math::*;
use std::path::{Path, PathBuf};
use tcg::{Attribute, CardDb, CardDef, CardId, Kind, Rarity};

struct Args {
    game: String,
    out: PathBuf,
    art: PathBuf,
    card: Option<String>,
    finish: Option<String>,
    /// Render the booster pack wrappers instead of the cards.
    packs: bool,
    /// Render every card of this season (normal finish) into <out>/cards/.
    set: Option<u16>,
    /// Render the duel playmat.
    playmat: bool,
}

fn args() -> Args {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut a = Args {
        game: "Mythbeasts".into(),
        out: root.join("assets/cards/templates"),
        art: root.join("assets/cards/art"),
        card: None,
        finish: None,
        packs: false,
        set: None,
        playmat: false,
    };
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < v.len() {
        let next = v.get(i + 1).cloned().unwrap_or_default();
        if v[i] == "--packs" {
            a.packs = true;
            i += 1;
            continue;
        }
        if v[i] == "--props" {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            let fonts = text::Fonts::load();
            props::build(&root, &fonts, &a.game);
            std::process::exit(0);
        }
        if v[i] == "--playmat" {
            a.playmat = true;
            i += 1;
            continue;
        }
        match v[i].as_str() {
            "--game" => a.game = next,
            "--out" => a.out = next.into(),
            "--art" => a.art = next.into(),
            "--card" => a.card = Some(next),
            "--finish" => a.finish = Some(next),
            "--set" => a.set = Some(next.parse().expect("--set takes a season number")),
            other => panic!("unknown argument {other}"),
        }
        i += 2;
    }
    a
}

fn finish_from(s: &str) -> Finish {
    Finish::from_slug(s).unwrap_or_else(|| panic!("finish must be one of {:?}", Finish::ALL.map(|f| f.slug())))
}

/// Pick a showcase card: first match on rarity, then preferred kind/attribute.
fn pick<'a>(db: &'a CardDb, set: u16, r: Rarity, kind: Kind, attr: Attribute) -> &'a CardDef {
    let all: Vec<&CardDef> = db.set(set).filter(|c| c.rarity == r).collect();
    all.iter()
        .find(|c| c.kind == kind && c.attribute == attr)
        .or_else(|| all.iter().find(|c| c.kind == kind))
        .or_else(|| all.iter().find(|c| c.attribute == attr))
        .copied()
        .unwrap_or(all[0])
}

struct Out {
    slug: String,
    label: String,
    preview: Vec<[f32; 4]>,
}

fn save_layers(dir: &Path, l: &shade::Layers, foil: bool) -> Vec<[f32; 4]> {
    std::fs::create_dir_all(dir).unwrap();
    let (w, h) = (l.w, l.h);
    let mut base = Vec::with_capacity(w * h * 4);
    for (c, a) in l.base.iter().zip(&l.alpha) {
        base.extend_from_slice(&[(to_srgb(c.0) * 255.0 + 0.5) as u8, (to_srgb(c.1) * 255.0 + 0.5) as u8, (to_srgb(c.2) * 255.0 + 0.5) as u8, (a * 255.0 + 0.5) as u8]);
    }
    write_rgba(&dir.join("base.png"), w, h, &base);
    let foil_bytes: Vec<u8> = l.foil.iter().flat_map(|p| *p).collect();
    write_rgba(&dir.join("foil.png"), w, h, &foil_bytes);
    write_gray16(&dir.join("height.png"), w, h, &l.height);
    let spec: Vec<u8> = l.spec.iter().flat_map(|p| *p).collect();
    write_rgb(&dir.join("spec.png"), w, h, &spec);
    // Engine maps (glTF-style): tangent-space normal from the emboss height, and ORM.
    let (nrm, orm) = shade::engine_maps(l);
    write_rgb(&dir.join("normal.png"), w, h, &nrm);
    write_rgb(&dir.join("orm.png"), w, h, &orm);
    let p = shade::preview(l, (0.0, 0.0));
    write_rgba(&dir.join("preview.png"), w, h, &shade::to_rgba8(&p));
    if foil {
        let t = shade::preview(l, (0.55, 0.25));
        write_rgba(&dir.join("preview_tilt.png"), w, h, &shade::to_rgba8(&t));
        tilt_strip(&dir.join("tilt_strip.png"), l);
    }
    p
}

/// The finish a foil pull shows until `tcg` has a finish per pull (the proposal in
/// docs/CARD_TEMPLATES.md): reverse holo on Commons and Uncommons, holo on Rares and Mythics.
pub fn foil_finish(r: Rarity) -> Finish {
    match r {
        Rarity::Common | Rarity::Uncommon => Finish::Reverse,
        Rarity::Rare | Rarity::Mythic => Finish::Holo,
    }
}

/// Only what the engine loads: base, foil, normal, ORM (no previews, height or spec).
fn save_engine_layers(dir: &Path, l: &shade::Layers) {
    std::fs::create_dir_all(dir).unwrap();
    let (w, h) = (l.w, l.h);
    let mut base = Vec::with_capacity(w * h * 4);
    for (c, a) in l.base.iter().zip(&l.alpha) {
        base.extend_from_slice(&[(to_srgb(c.0) * 255.0 + 0.5) as u8, (to_srgb(c.1) * 255.0 + 0.5) as u8, (to_srgb(c.2) * 255.0 + 0.5) as u8, (a * 255.0 + 0.5) as u8]);
    }
    write_rgba(&dir.join("base.png"), w, h, &base);
    let foil_bytes: Vec<u8> = l.foil.iter().flat_map(|p| *p).collect();
    write_rgba(&dir.join("foil.png"), w, h, &foil_bytes);
    let (nrm, orm) = shade::engine_maps(l);
    write_rgb(&dir.join("normal.png"), w, h, &nrm);
    write_rgb(&dir.join("orm.png"), w, h, &orm);
}

/// Five views of the same card as it is tilted left to right (camera sweep), at 1/3 size, to
/// show the angle-dependent foil.
fn tilt_strip(path: &Path, l: &shade::Layers) {
    let (cw, ch) = (500usize, 700usize);
    let n = 5;
    let gap = 20;
    let w = n * cw + (n - 1) * gap;
    let mut img = vec![[0.02f32, 0.022, 0.026, 1.0]; w * ch];
    for k in 0..n {
        let t = -0.6 + 1.2 * k as f32 / (n - 1) as f32;
        let p = shade::preview(l, (t, -0.1 + 0.2 * (k as f32 / (n - 1) as f32)));
        let s = shade::resample(&p, l.w, l.h, cw, ch);
        for y in 0..ch {
            for x in 0..cw {
                let q = s[y * cw + x];
                let o = &mut img[y * w + k * (cw + gap) + x];
                for c in 0..3 {
                    o[c] = o[c] * (1.0 - q[3]) + q[c] * q[3];
                }
            }
        }
    }
    write_rgba(path, w, ch, &shade::to_rgba8(&img));
}

fn main() {
    let a = args();
    let fonts = text::Fonts::load();
    let db = CardDb::generate(12);
    std::fs::create_dir_all(&a.out).unwrap();

    // The duel playmat.
    if a.playmat {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/cards/playmat");
        playmat::render(&dir, &fonts, &a.game);
        return;
    }

    // Every card of a season, for the duel (skips cards already rendered unless CARDGEN_FORCE).
    if let Some(set) = a.set {
        use rayon::prelude::*;
        let force = std::env::var("CARDGEN_FORCE").is_ok();
        let th = tcg::season::theme(set);
        let cards: Vec<&CardDef> = db.set(set).collect();
        let done = std::sync::atomic::AtomicUsize::new(0);
        // --finish: a finish slug for every card, or "foil" = the finish a `Pull { foil: true }`
        // shows (Common/Uncommon: reverse holo, Rare/Mythic: holo; docs/CARD_TEMPLATES.md)
        let pick = |card: &CardDef| -> Finish {
            match a.finish.as_deref() {
                None => Finish::Normal,
                Some("foil") => foil_finish(card.rarity),
                Some(f) => finish_from(f),
            }
        };
        cards.par_iter().with_max_len(1).for_each(|card| {
            let fin = pick(card);
            let dir = a.out.join("cards").join(format!("{}_{}", card.id, fin.slug()));
            if !force && dir.join("orm.png").exists() {
                return;
            }
            let spec = FrontSpec { card, finish: fin, set_name: th.name, set_size: tcg::season::CARDS_PER_SET, art_root: &a.art };
            let c = design::front(&spec, &fonts);
            let l = shade::finalize(&c, design::emboss_strength(card.rarity, fin));
            save_engine_layers(&dir, &l);
            let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            println!("{n}: {} {}", card.id, card.name);
        });
        return;
    }

    // Booster pack wrappers.
    if a.packs {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/cards/packs");
        let mut items = Vec::new();
        for f in pack::FORMATS {
            if std::env::var("CARDGEN_ONLY").map_or(false, |o| !f.slug.contains(o.as_str())) {
                continue;
            }
            let t0 = std::time::Instant::now();
            let wr = pack::render(&f, &fonts, &a.game, &root.join("art"));
            pack::save(&f, &wr, &root.join(f.slug), &fonts);
            println!("pack {}: {}x{} px in {:.1}s", f.slug, wr.w, wr.h, t0.elapsed().as_secs_f32());
            items.push((f, wr));
        }
        pack::contact(&root.join("contact_packs.png"), &items, &fonts);
        return;
    }

    // Single card mode.
    if let Some(id) = &a.card {
        let (set, num) = id.trim_start_matches('S').split_once('-').expect("card id like S01-023");
        let cid = CardId { set: set.parse().unwrap(), number: num.parse().unwrap() };
        let card = db.get(cid);
        let fin = finish_from(a.finish.as_deref().unwrap_or("normal"));
        let th = tcg::season::theme(cid.set);
        let spec = FrontSpec { card, finish: fin, set_name: th.name, set_size: tcg::season::CARDS_PER_SET, art_root: &a.art };
        let c = design::front(&spec, &fonts);
        let l = shade::finalize(&c, design::emboss_strength(card.rarity, fin));
        let dir = a.out.join("cards").join(format!("{}_{}", cid, fin.slug()));
        save_layers(&dir, &l, fin != Finish::Normal);
        println!("wrote {}", dir.display());
        return;
    }

    let set = 1;
    let th = tcg::season::theme(set);
    let showcase: Vec<(&str, &str, &CardDef, Finish)> = vec![
        ("front_common", "Common", pick(&db, set, Rarity::Common, Kind::Monster, Attribute::Flame), Finish::Normal),
        ("front_uncommon", "Uncommon", pick(&db, set, Rarity::Uncommon, Kind::Spell, Attribute::Tide), Finish::Normal),
        ("front_rare", "Rare", pick(&db, set, Rarity::Rare, Kind::Relic, Attribute::Terra), Finish::Normal),
        ("front_mythic", "Mythic", pick(&db, set, Rarity::Mythic, Kind::Monster, Attribute::Umbral), Finish::Normal),
        ("front_holo", "Holo", pick(&db, 2, Rarity::Rare, Kind::Monster, Attribute::Tide), Finish::Holo),
        ("front_reverse", "Reverse holo", pick(&db, 4, Rarity::Common, Kind::Monster, Attribute::Radiant), Finish::Reverse),
        ("front_fullart", "Full-art", pick(&db, set, Rarity::Mythic, Kind::Monster, Attribute::Terra), Finish::FullArt),
        ("front_gold", "Gold secret", pick(&db, set, Rarity::Mythic, Kind::Monster, Attribute::Flame), Finish::Gold),
        ("front_cosmos", "Cosmos holo", pick(&db, 4, Rarity::Rare, Kind::Monster, Attribute::Umbral), Finish::Cosmos),
        ("front_ice", "Cracked ice", pick(&db, 9, Rarity::Rare, Kind::Monster, Attribute::Tide), Finish::Ice),
        ("front_rainbow", "Rainbow rare", pick(&db, 12, Rarity::Mythic, Kind::Monster, Attribute::Gale), Finish::Rainbow),
        ("front_glass", "Stained glass", pick(&db, 8, Rarity::Rare, Kind::Spell, Attribute::Radiant), Finish::Glass),
        ("front_sakura", "Sakura", pick(&db, 6, Rarity::Uncommon, Kind::Monster, Attribute::Gale), Finish::Sakura),
        ("front_obsidian", "Obsidian", pick(&db, 5, Rarity::Mythic, Kind::Monster, Attribute::Umbral), Finish::Obsidian),
    ];
    let only: Option<String> = std::env::var("CARDGEN_ONLY").ok();

    let mut outs = Vec::new();
    let mut manifest = String::from("[\n");
    for (slug, label, card, fin) in &showcase {
        if only.as_ref().map_or(false, |o| !slug.contains(o.as_str())) {
            continue;
        }
        let t0 = std::time::Instant::now();
        let spec = FrontSpec { card, finish: *fin, set_name: tcg::season::theme(card.id.set).name, set_size: tcg::season::CARDS_PER_SET, art_root: &a.art };
        let c = design::front(&spec, &fonts);
        let l = shade::finalize(&c, design::emboss_strength(card.rarity, *fin));
        let p = save_layers(&a.out.join(slug), &l, *fin != Finish::Normal);
        println!("{slug}: {} \"{}\" ({:?} {:?} {:?}) in {:.1}s", card.id, card.name, card.rarity, card.kind, card.attribute, t0.elapsed().as_secs_f32());
        manifest += &format!(
            "  {{ \"template\": \"{slug}\", \"label\": \"{label}\", \"card\": \"{}\", \"name\": \"{}\", \"rarity\": \"{:?}\", \"finish\": \"{}\", \"kind\": \"{:?}\", \"attribute\": \"{:?}\" }},\n",
            card.id, card.name, card.rarity, fin.slug(), card.kind, card.attribute
        );
        outs.push(Out { slug: slug.to_string(), label: label.to_string(), preview: p });
    }
    if only.as_ref().map_or(true, |o| "back".contains(o.as_str())) {
        let c = design::back(&fonts, &a.game);
        let l = shade::finalize(&c, 0.6);
        let p = save_layers(&a.out.join("back"), &l, false);
        println!("back done");
        manifest += "  { \"template\": \"back\", \"label\": \"Card back\" }\n";
        outs.push(Out { slug: "back".into(), label: "Back".into(), preview: p });
    }
    manifest += "]\n";
    if only.is_none() {
        std::fs::write(a.out.join("templates.json"), manifest).unwrap();
        contact_sheet(&a.out.join("contact_sheet.png"), &outs, &fonts, &a.game);
        attribute_sheet(&a, &db, &fonts);
    }
}

/// All templates side by side on a dark play mat, with soft shadows and labels.
fn contact_sheet(path: &Path, outs: &[Out], fonts: &text::Fonts, game: &str) {
    let (cw, ch) = (500usize, 700usize);
    let cols = 5usize;
    let rows = (outs.len() + cols - 1) / cols;
    let gap = 70usize;
    let label_h = 70usize;
    let top = 130usize;
    let w = cols * cw + (cols + 1) * gap;
    let h = top + rows * (ch + label_h + gap) + gap / 2;
    let mut img: Vec<V3> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f32, (i / w) as f32);
            let n = fbm(x * 0.6, y * 0.6, 2, 5) - 0.5;
            let v = 1.0 - 0.35 * (((x / w as f32) - 0.5).powi(2) + ((y / h as f32) - 0.45).powi(2)) * 2.0;
            hex(0x1c2026) * (v * (1.0 + n * 0.12))
        })
        .collect();
    let blend = |img: &mut Vec<V3>, x: usize, y: usize, c: V3, a: f32| {
        if x < w && y < h {
            let p = &mut img[y * w + x];
            *p = p.lerp(c, a.clamp(0.0, 1.0));
        }
    };
    // Title
    let title = format!("{}  \u{00b7}  CARD TEMPLATES", game.to_uppercase());
    let m = text::line(&fonts.label, 44.0, &title, 6.0, gap as f32, 82.0, 1.0);
    for yy in 0..m.h {
        for xx in 0..m.w {
            let v = m.d[yy * m.w + xx];
            if v > 0.0 {
                blend(&mut img, (m.x0 + xx as i32) as usize, (m.y0 + yy as i32) as usize, V3::splat(0.75), v);
            }
        }
    }
    for (k, o) in outs.iter().enumerate() {
        let (col, row) = (k % cols, k / cols);
        let x0 = gap + col * (cw + gap);
        let y0 = top + row * (ch + label_h + gap);
        let small = shade::resample(&o.preview, design::W, design::HH, cw, ch);
        // Shadow from alpha.
        let mut am = Mask::new(0, 0, cw, ch);
        for (i, p) in small.iter().enumerate() {
            am.d[i] = p[3];
        }
        let sh = am.blur(14.0);
        for yy in 0..sh.h as i32 {
            for xx in 0..sh.w as i32 {
                let gx = sh.x0 + xx;
                let gy = sh.y0 + yy;
                let v = sh.get(gx, gy) * 0.65;
                let (tx, ty) = (x0 as i32 + gx + 10, y0 as i32 + gy + 16);
                if v > 0.0 && tx >= 0 && ty >= 0 {
                    blend(&mut img, tx as usize, ty as usize, V3::splat(0.0), v);
                }
            }
        }
        for yy in 0..ch {
            for xx in 0..cw {
                let p = small[yy * cw + xx];
                blend(&mut img, x0 + xx, y0 + yy, V3(p[0], p[1], p[2]), p[3]);
            }
        }
        let lw = text::measure(&fonts.label, 30.0, &o.label, 2.0);
        let m = text::line(&fonts.label, 30.0, &o.label, 2.0, x0 as f32 + (cw as f32 - lw) * 0.5, (y0 + ch + 50) as f32, 1.0);
        for yy in 0..m.h {
            for xx in 0..m.w {
                let v = m.d[yy * m.w + xx];
                if v > 0.0 {
                    blend(&mut img, (m.x0 + xx as i32) as usize, (m.y0 + yy as i32) as usize, V3::splat(0.62), v);
                }
            }
        }
        let _ = &o.slug;
    }
    let bytes: Vec<u8> = img.iter().flat_map(|c| [(to_srgb(c.0) * 255.0) as u8, (to_srgb(c.1) * 255.0) as u8, (to_srgb(c.2) * 255.0) as u8]).collect();
    write_rgb(path, w, h, &bytes);
    println!("wrote {}", path.display());
}

/// One Common of every attribute plus each kind, to check colours and kind styling.
fn attribute_sheet(a: &Args, db: &CardDb, fonts: &text::Fonts) {
    let set = 1;
    let th = tcg::season::theme(set);
    let mut outs = Vec::new();
    for attr in Attribute::ALL {
        let card = db
            .all()
            .iter()
            .filter(|c| c.attribute == attr && c.rarity == Rarity::Common)
            .min_by_key(|c| (c.id.set, c.id.number))
            .unwrap();
        let spec = FrontSpec { card, finish: Finish::Normal, set_name: tcg::season::theme(card.id.set).name, set_size: 60, art_root: &a.art };
        let c = design::front(&spec, &fonts);
        let l = shade::finalize(&c, 0.18);
        outs.push(Out { slug: String::new(), label: format!("{attr}  \u{00b7}  {:?}", card.kind), preview: shade::preview(&l, (0.0, 0.0)) });
    }
    for kind in [Kind::Spell, Kind::Relic] {
        let card = db.set(set).find(|c| c.kind == kind && c.rarity == Rarity::Uncommon).unwrap_or_else(|| db.set(set).find(|c| c.kind == kind).unwrap());
        let spec = FrontSpec { card, finish: Finish::Normal, set_name: th.name, set_size: 60, art_root: &a.art };
        let c = design::front(&spec, &fonts);
        let l = shade::finalize(&c, 0.25);
        outs.push(Out { slug: String::new(), label: format!("{:?}  \u{00b7}  {}", kind, card.attribute), preview: shade::preview(&l, (0.0, 0.0)) });
    }
    contact_sheet(&a.out.join("contact_attributes.png"), &outs, fonts, &a.game);
}
