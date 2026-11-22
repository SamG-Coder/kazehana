//! The approval registry (assets/registry.json): the only way the game loads 3D assets.
//! `Registry::load_model` refuses anything not approved, or changed since approval.

use crate::check::{asset_files, fingerprint, hash_files, Category, Report};
use akari::{LoadOptions, Model, Renderer};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const REGISTRY_PATH: &str = "assets/registry.json";
pub const QC_DIR: &str = "assets/qc";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Approved,
    Rejected,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub source: String,
    pub category: Category,
    /// Content hash of every file at approval.
    pub hash: String,
    /// Sizes / modification times at approval (skips re-hashing when unchanged).
    pub fingerprint: String,
    pub status: Status,
    pub reviewer: String,
    pub notes: String,
    pub date: u64,
    /// Warnings accepted at review.
    pub warnings: Vec<String>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Registry {
    pub assets: BTreeMap<String, Entry>,
    #[serde(skip)]
    path: PathBuf,
}

/// The look-dev record written next to the renders.
#[derive(Serialize, Deserialize)]
pub struct Lookdev {
    pub hash: String,
    pub images: Vec<String>,
    pub date: u64,
}

pub fn qc_dir(id: &str) -> PathBuf {
    Path::new(QC_DIR).join(id)
}

pub fn read_report(id: &str) -> Result<Report> {
    let p = qc_dir(id).join("report.json");
    serde_json::from_slice(&std::fs::read(&p).with_context(|| format!("no check report for '{id}' (run assetcheck check first)"))?).context("report.json")
}

pub fn write_report(r: &Report) -> Result<()> {
    let d = qc_dir(&r.id);
    std::fs::create_dir_all(&d)?;
    std::fs::write(d.join("report.json"), serde_json::to_vec_pretty(r)?)?;
    Ok(())
}

pub fn read_lookdev(id: &str) -> Option<Lookdev> {
    serde_json::from_slice(&std::fs::read(qc_dir(id).join("lookdev.json")).ok()?).ok()
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Registry {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let mut r: Registry = match std::fs::read(&path) {
            Ok(b) => serde_json::from_slice(&b).with_context(|| format!("parsing {}", path.display()))?,
            Err(_) => Registry::default(),
        };
        r.path = path;
        Ok(r)
    }

    pub fn save(&self) -> Result<()> {
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&self.path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    /// Approve an asset: its latest report must have passed and the look-dev renders must be of
    /// the current files.
    pub fn approve(&mut self, id: &str, reviewer: &str, notes: &str) -> Result<()> {
        let rep = read_report(id)?;
        if !rep.passed {
            bail!("'{id}' failed automated checks; fix and re-check before approving");
        }
        let files = asset_files(Path::new(&rep.source))?;
        let hash = hash_files(&files)?;
        if hash != rep.hash {
            bail!("'{id}' changed since it was checked; run assetcheck check again");
        }
        match read_lookdev(id) {
            Some(l) if l.hash == hash => {}
            _ => bail!("'{id}' has no look-dev renders of its current files; run assetcheck lookdev first"),
        }
        let warnings = rep.findings.iter().filter(|f| f.level == crate::check::Level::Warn).map(|f| format!("{}: {}", f.check, f.message)).collect();
        self.assets.insert(
            id.into(),
            Entry { source: rep.source, category: rep.category, hash, fingerprint: fingerprint(&files), status: Status::Approved, reviewer: reviewer.into(), notes: notes.into(), date: now(), warnings },
        );
        self.save()
    }

    pub fn reject(&mut self, id: &str, reviewer: &str, notes: &str) -> Result<()> {
        let rep = read_report(id)?;
        self.assets.insert(
            id.into(),
            Entry { source: rep.source, category: rep.category, hash: rep.hash, fingerprint: String::new(), status: Status::Rejected, reviewer: reviewer.into(), notes: notes.into(), date: now(), warnings: vec![] },
        );
        self.save()
    }

    /// Is `id` approved, with its files unchanged since approval?
    pub fn verify(&self, id: &str) -> Result<&Entry> {
        let Some(e) = self.assets.get(id) else { bail!("asset '{id}' is not in the registry: it has not passed the quality gate (docs/ASSET_QUALITY.md)") };
        if e.status != Status::Approved {
            bail!("asset '{id}' was rejected at review: {}", e.notes);
        }
        let files = asset_files(Path::new(&e.source))?;
        if fingerprint(&files) != e.fingerprint && hash_files(&files)? != e.hash {
            bail!("asset '{id}' changed after approval: it must go through the quality gate again");
        }
        Ok(e)
    }

    /// Review mode (the visual review of a level happens in the game): load an asset that has
    /// passed its automated checks on its current files but is not approved yet. Approved assets
    /// load normally. Anything without a passing report for its current files is refused.
    pub fn load_for_review(&self, r: &mut Renderer, id: &str) -> Result<Model> {
        if self.verify(id).is_ok() {
            return self.load_model(r, id);
        }
        let rep = read_report(id)?;
        if !rep.passed {
            bail!("'{id}' failed its automated checks; it cannot be shown even for review");
        }
        let files = asset_files(Path::new(&rep.source))?;
        if hash_files(&files)? != rep.hash {
            bail!("'{id}' changed since it was checked; run assetcheck check again");
        }
        eprintln!("REVIEW MODE: '{id}' is not approved yet (shown for its visual review only)");
        let opts = LoadOptions { foliage: rep.category == Category::Foliage };
        akari::gltf_load::load(r, Path::new(&rep.source), opts)
    }

    /// Load an approved asset (the only way the game loads 3D models).
    pub fn load_model(&self, r: &mut Renderer, id: &str) -> Result<Model> {
        let e = self.verify(id)?;
        let opts = LoadOptions { foliage: e.category == Category::Foliage };
        akari::gltf_load::load(r, Path::new(&e.source), opts)
    }
}
