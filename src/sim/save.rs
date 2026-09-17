//! Save / load (C6 FR1/FR2): a binary `SIMF` file with a versioned header and a
//! postcard-encoded `Sim`. The spatial index is rebuilt on load, never serialised.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::sim::species::Kind;
use crate::sim::stats::census;
use crate::sim::Sim;

/// File magic: `b"SIMF"`.
pub const MAGIC: [u8; 4] = *b"SIMF";
/// Current on-disk format version.
///
/// A newer version is rejected (FR1), and so is an older one: C8 widened the
/// genome and C5 `FR5b` added the wary state, so pre-wary files cannot be
/// read. Version 5 added `world.age` to the serialised parameters
/// (erosion-based world generation) and made the species roster configurable:
/// every per-species table became a `Vec` and the header carries the roster
/// labels (a version-4 header still decodes so old files stay listable).
/// Version 6 added the per-cell temperature and the world's prevailing wind
/// (climate physics); version 7 added the `Marsh` terrain and the marsh step
/// cost parameter (wetlands and riparian corridors); version 8 added the
/// per-cell biome and the per-cell region map (biomes as regions); version 9
/// came with river morphology (width tiers, deltas, washes and falls); version
/// 10 widened the genome to twelve traits (Mutability) and added the
/// per-creature `sterile` flag.
pub const VERSION: u16 = 10;
/// Padding code used to fill a title-screen terrain strip out to 120 columns.
pub const BLANK_TERRAIN: u8 = u8::MAX;

/// One roster entry as the header records it, so the Load list can label the
/// counts without decoding the `Sim`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderSpecies {
    pub plural: String,
    pub kind: Kind,
    pub glyph: char,
    pub color: [u8; 3],
}

/// The header that precedes the `Sim` payload (FR1).
///
/// `counts` are the per-species living counts in roster order, labelled by
/// `species`; `strip_rows` are 4 × 120 terrain codes for the S00 decorative
/// strips (world rows at H × {0.45, 0.475, 0.75, 0.775}).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveHeader {
    pub seed: u64,
    pub tick: u64,
    pub season_days: u32,
    pub start_hour: u32,
    pub saved_at_unix: u64,
    pub world_name: String,
    pub counts: Vec<u32>,
    pub species: Vec<HeaderSpecies>,
    pub strip_rows: [Vec<u8>; 4],
}

impl SaveHeader {
    /// `(prey, predators)` living counts.
    pub fn kind_totals(&self) -> (u32, u32) {
        let mut prey = 0;
        let mut pred = 0;
        for (s, &n) in self.species.iter().zip(&self.counts) {
            match s.kind {
                Kind::Prey => prey += n,
                Kind::Predator => pred += n,
            }
        }
        (prey, pred)
    }
}

/// The version-4 header (six fixed species), kept so old files stay listable.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct HeaderV4 {
    seed: u64,
    tick: u64,
    season_days: u32,
    start_hour: u32,
    saved_at_unix: u64,
    world_name: String,
    counts: [u32; 6],
    strip_rows: [Vec<u8>; 4],
}

impl HeaderV4 {
    fn upgrade(self) -> SaveHeader {
        let labels = [
            ("Voles", Kind::Prey, 'v', [188, 156, 116]),
            ("Hares", Kind::Prey, 'h', [228, 220, 196]),
            ("Deer", Kind::Prey, 'd', [214, 160, 92]),
            ("Foxes", Kind::Predator, 'f', [246, 128, 42]),
            ("Wolves", Kind::Predator, 'w', [224, 66, 66]),
            ("Lynxes", Kind::Predator, 'l', [236, 110, 150]),
        ];
        SaveHeader {
            seed: self.seed,
            tick: self.tick,
            season_days: self.season_days,
            start_hour: self.start_hour,
            saved_at_unix: self.saved_at_unix,
            world_name: self.world_name,
            counts: self.counts.to_vec(),
            species: labels.iter().map(|(p, k, g, c)| HeaderSpecies { plural: (*p).to_string(), kind: *k, glyph: *g, color: *c }).collect(),
            strip_rows: self.strip_rows,
        }
    }
}

/// A fully loaded save: header plus the reconstructed simulation.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub header: SaveHeader,
    pub sim: Sim,
}

/// One save file in a `Load World` list.
#[derive(Clone, Debug)]
pub struct SaveEntry {
    pub path: PathBuf,
    pub header: SaveHeader,
    /// On-disk format version, so the list can flag a file `load` will reject.
    pub version: u16,
}

#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    Postcard(postcard::Error),
    /// `save is from another version (N ≠ M)`: the format is never kept compatible (C7 FR11).
    VersionMismatch { found: u16, supported: u16 },
    /// A save written before the current format (C8): the genome gained two traits,
    /// so the creature records cannot be decoded. There is no migration path.
    OlderVersion { found: u16, supported: u16 },
    BadMagic,
    Truncated,
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Postcard(e) => write!(f, "{e}"),
            Self::VersionMismatch { found, supported } => {
                write!(f, "save is from another version ({found} ≠ {supported})")
            }
            Self::OlderVersion { found, supported } => write!(
                f,
                "save is from an older version ({found}, now {supported}): the genome format changed, so it cannot be loaded"
            ),
            Self::BadMagic => write!(f, "not a Sim Fortress save (bad magic)"),
            Self::Truncated => write!(f, "save file is truncated"),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<postcard::Error> for SaveError {
    fn from(e: postcard::Error) -> Self {
        Self::Postcard(e)
    }
}

/// Lowercase the world name, collapsing every run of non-`[a-z0-9]` to one `-`
/// (FR1). Falls back to `world` when nothing survives.
pub fn slug(world_name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in world_name.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            last_dash = false;
        } else if !out.is_empty() && !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "world".to_string()
    } else {
        out
    }
}

pub fn save_filename(world_name: &str, tick: u64) -> String {
    format!("{}-{tick}.simf", slug(world_name))
}

pub fn autosave_filename(world_name: &str) -> String {
    format!("{}-autosave.simf", slug(world_name))
}

/// `saves/` relative to the cwd, overridable by `--saves-dir` (FR1).
pub fn saves_dir(override_dir: Option<&Path>) -> PathBuf {
    match override_dir {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from("saves"),
    }
}

/// Autosave fires at the day boundary when `autosave_days > 0` and the 0-based
/// day index is a positive multiple of `autosave_days` (FR1).
pub fn autosave_due(day_index: u64, autosave_days: u32) -> bool {
    autosave_days > 0 && day_index > 0 && day_index % u64::from(autosave_days) == 0
}

/// Per-species living counts, roster order.
pub fn counts(sim: &Sim) -> Vec<u32> {
    census(&sim.creatures, sim.params.species.len()).population
}

/// The four 120-column terrain strips for the S00 title screen (FR1).
pub fn strip_rows(sim: &Sim) -> [Vec<u8>; 4] {
    let h = sim.world.height();
    let w = sim.world.width();
    let fractions = [0.45f64, 0.475, 0.75, 0.775];
    let mut out: [Vec<u8>; 4] = std::array::from_fn(|_| vec![BLANK_TERRAIN; 120]);
    let center = w.min(120);
    let left_pad = (120 - center).div_euclid(2);
    let start_col = (w - center).div_euclid(2);
    for (i, frac) in fractions.iter().enumerate() {
        let row = (crate::cast!((crate::cast!(h => f64) * frac).round() => usize)).min(h.saturating_sub(1));
        for j in 0..center {
            out[i][left_pad + j] = crate::cast!(sim.world.cell(start_col + j, row).terrain => u8);
        }
    }
    out
}

/// Build the header for the current state of `sim`.
pub fn build_header(sim: &Sim, world_name: &str) -> SaveHeader {
    SaveHeader {
        seed: sim.seed,
        tick: sim.time.tick,
        season_days: sim.params.time.season_days,
        start_hour: sim.params.time.start_hour,
        saved_at_unix: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        world_name: world_name.to_string(),
        counts: counts(sim),
        species: sim.params.species.0.iter().map(|s| HeaderSpecies { plural: s.plural.clone(), kind: s.kind, glyph: s.glyph, color: s.color }).collect(),
        strip_rows: strip_rows(sim),
    }
}

fn encode(header: &SaveHeader, sim: &Sim) -> Result<Vec<u8>, SaveError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    let header_bytes = postcard::to_allocvec(header)?;
    bytes.extend_from_slice(&(crate::cast!(header_bytes.len() => u32)).to_le_bytes());
    bytes.extend_from_slice(&header_bytes);
    let sim_bytes = postcard::to_allocvec(sim)?;
    bytes.extend_from_slice(&sim_bytes);
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> Result<(SaveHeader, Sim), SaveError> {
    if bytes.len() < 10 {
        return Err(SaveError::Truncated);
    }
    if bytes[0..4] != MAGIC {
        return Err(SaveError::BadMagic);
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version < VERSION {
        return Err(SaveError::OlderVersion { found: version, supported: VERSION });
    }
    if version != VERSION {
        return Err(SaveError::VersionMismatch { found: version, supported: VERSION });
    }
    let header_len = crate::cast!(u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) => usize);
    if bytes.len() < 10 + header_len {
        return Err(SaveError::Truncated);
    }
    let header: SaveHeader = postcard::from_bytes(&bytes[10..10 + header_len])?;
    let mut sim: Sim = postcard::from_bytes(&bytes[10 + header_len..])?;
    sim.rebuild_spatial();
    // Group statistics are not serialised: rebuild them from the loaded positions.
    sim.refresh_group_stats();
    Ok((header, sim))
}

/// Write a timestamped save `<slug>-<tick>.simf` and return its path.
pub fn save(sim: &Sim, world_name: &str, dir: &Path) -> Result<PathBuf, SaveError> {
    std::fs::create_dir_all(dir)?;
    let header = build_header(sim, world_name);
    let bytes = encode(&header, sim)?;
    let path = dir.join(save_filename(world_name, sim.time.tick));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// Write (or overwrite) `<slug>-autosave.simf` and return its path.
pub fn autosave(sim: &Sim, world_name: &str, dir: &Path) -> Result<PathBuf, SaveError> {
    std::fs::create_dir_all(dir)?;
    let header = build_header(sim, world_name);
    let bytes = encode(&header, sim)?;
    let path = dir.join(autosave_filename(world_name));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// Load a save (header + sim, spatial index rebuilt).
pub fn load(path: &Path) -> Result<Loaded, SaveError> {
    let bytes = std::fs::read(path)?;
    let (header, sim) = decode(&bytes)?;
    Ok(Loaded { header, sim })
}

/// Read only the header (the `header_only_read` acceptance path).
///
/// Deliberately version-agnostic: the header record is the same in every format
/// version, so the Load list can list old files and let `load` produce the
/// friendly `OlderVersion` error.
pub fn read_header(path: &Path) -> Result<SaveHeader, SaveError> {
    let bytes = std::fs::read(path)?;
    Ok(read_header_bytes(&bytes)?.0)
}

/// Parse the header and report the format version beside it.
pub fn read_header_bytes(bytes: &[u8]) -> Result<(SaveHeader, u16), SaveError> {
    if bytes.len() < 10 {
        return Err(SaveError::Truncated);
    }
    if bytes[0..4] != MAGIC {
        return Err(SaveError::BadMagic);
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    let header_len = crate::cast!(u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) => usize);
    if bytes.len() < 10 + header_len {
        return Err(SaveError::Truncated);
    }
    let header_bytes = &bytes[10..10 + header_len];
    let header: SaveHeader = if version < 5 {
        postcard::from_bytes::<HeaderV4>(header_bytes)?.upgrade()
    } else {
        postcard::from_bytes(header_bytes)?
    };
    Ok((header, version))
}

/// All `*.simf` saves in `dir`, newest first (FR3). Unreadable files are skipped.
pub fn list_saves(dir: &Path) -> Vec<SaveEntry> {
    let mut out: Vec<SaveEntry> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().and_then(|s| s.to_str()) != Some("simf") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        if let Ok((header, version)) = read_header_bytes(&bytes) {
            out.push(SaveEntry { path, header, version });
        }
    }
    out.sort_by(|a, b| {
        b.header
            .saved_at_unix
            .cmp(&a.header.saved_at_unix)
            .then_with(|| b.header.tick.cmp(&a.header.tick))
            .then_with(|| a.path.cmp(&b.path))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Params;

    fn tmpdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("simf-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn filename_slug() {
        assert_eq!(slug("The Valley of Sunfall"), "the-valley-of-sunfall");
        assert_eq!(slug("  A--B__C  "), "a-b-c");
        assert_eq!(slug("!!!   "), "world");
        assert_eq!(save_filename("My World", 42), "my-world-42.simf");
        assert_eq!(autosave_filename("My World"), "my-world-autosave.simf");
    }

    #[test]
    fn header_only_read() {
        let dir = tmpdir("header");
        let sim = Sim::new(7, Params::default());
        let path = save(&sim, "Header Test", &dir).unwrap();
        let h = read_header(&path).unwrap();
        assert_eq!(h.world_name, "Header Test");
        assert_eq!(h.seed, 7);
        assert_eq!(h.tick, 0);
        assert_eq!(h.season_days, sim.params.time.season_days);
        assert_eq!(h.counts, counts(&sim));
        assert_eq!(h.strip_rows, strip_rows(&sim));
    }

    #[test]
    fn version_mismatch_rejected() {
        let dir = tmpdir("version");
        let sim = Sim::new(7, Params::default());
        let header = build_header(&sim, "v");
        let mut bytes = encode(&header, &sim).unwrap();
        // Bump the version field to VERSION + 1.
        bytes[4..6].copy_from_slice(&(VERSION + 1).to_le_bytes());
        let path = dir.join("future.simf");
        std::fs::write(&path, &bytes).unwrap();
        match load(&path) {
            Err(SaveError::VersionMismatch { found, supported }) => {
                assert_eq!(found, VERSION + 1);
                assert_eq!(supported, VERSION);
            }
            other => panic!("expected VersionMismatch, got {other:?}"),
        }
    }

    #[test]
    fn older_version_rejected() {
        let dir = tmpdir("older-version");
        let sim = Sim::new(7, Params::default());
        let header = build_header(&sim, "v");
        // A genuine version-4 file: the fixed six-species header in front of the
        // current payload (only the header is ever decoded from an old file).
        let v4 = HeaderV4 {
            seed: header.seed,
            tick: header.tick,
            season_days: header.season_days,
            start_hour: header.start_hour,
            saved_at_unix: header.saved_at_unix,
            world_name: header.world_name.clone(),
            counts: [1, 2, 3, 4, 5, 6],
            strip_rows: header.strip_rows,
        };
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MAGIC);
        // Tagged as version 4, the last format with the fixed-six header
        // (`read_header` only takes the legacy path below version 5).
        let v4_version: u16 = 4;
        bytes.extend_from_slice(&v4_version.to_le_bytes());
        let header_bytes = postcard::to_allocvec(&v4).unwrap();
        bytes.extend_from_slice(&(crate::cast!(header_bytes.len() => u32)).to_le_bytes());
        bytes.extend_from_slice(&header_bytes);
        bytes.extend_from_slice(&postcard::to_allocvec(&sim).unwrap());
        let path = dir.join("older.simf");
        std::fs::write(&path, &bytes).unwrap();
        match load(&path) {
            Err(SaveError::OlderVersion { found, supported }) => {
                assert_eq!(found, v4_version);
                assert_eq!(supported, VERSION);
            }
            other => panic!("expected OlderVersion, got {other:?}"),
        }
        // The message names the version pair rather than a decode failure.
        let msg = SaveError::OlderVersion { found: 1, supported: VERSION }.to_string();
        assert!(msg.contains("older version"), "{msg}");
        // The header still parses, so the Load list can show the file and let the
        // player try it.
        let entries = list_saves(&dir);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].version, v4_version);
        let old = read_header(&path).unwrap();
        assert_eq!(old.counts, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(old.species.len(), 6, "the v4 header is labelled with the fixed six species");
        assert_eq!(old.species[0].plural, "Voles");
        assert_eq!(old.kind_totals(), (6, 15));
    }

    #[test]
    fn round_trip_checksum_3_seeds() {
        const N: u64 = 10_000;
        let dir = tmpdir("roundtrip");
        for seed in 1..=3u64 {
            let mut sim = Sim::new(seed, Params::default());
            for _ in 0..N {
                sim.step();
            }
            let path = save(&sim, "Round Trip", &dir).unwrap();
            let mut loaded = load(&path).unwrap().sim;
            // `load(save(sim))` then N ticks equals `sim` then N ticks.
            for _ in 0..N {
                sim.step();
                loaded.step();
            }
            assert_eq!(loaded.checksum(), sim.checksum(), "seed {seed} round-trip checksum");
        }
    }

    #[test]
    fn autosave_interval() {
        assert!(!autosave_due(0, 0));
        assert!(!autosave_due(0, 7));
        assert!(!autosave_due(5, 7));
        assert!(autosave_due(7, 7));
        assert!(autosave_due(14, 7));
        assert!(!autosave_due(8, 7));
        // off when 0
        assert!(!autosave_due(7, 0));
    }

    #[test]
    fn strip_rows_padded_to_120() {
        let sim = Sim::new(7, Params::default());
        let rows = strip_rows(&sim);
        assert_eq!(rows.len(), 4);
        for r in &rows {
            assert_eq!(r.len(), 120);
            assert!(r.iter().all(|&c| c <= 9 || c == BLANK_TERRAIN));
        }
        // Default world is 150 wide: no blank padding (centre 120 of 150).
        assert!(rows[0].iter().all(|&c| c != BLANK_TERRAIN));
    }
}
