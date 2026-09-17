//! Headless AI flags (ai-requirements R10): `--chronicle` and `--design-species`.
//!
//! Each reads the `[ai]` table from `ui.toml` only, calls the gateway
//! synchronously, and fails with one line rather than falling back, because
//! batch output must be what the flag promised or nothing. Without the `ai`
//! feature every flag reports that it is not compiled.

#[cfg(not(feature = "ai"))]
use std::io;

#[cfg(not(feature = "ai"))]
use crate::sim::Params;

#[cfg(not(feature = "ai"))]
fn not_compiled() -> io::Error {
    io::Error::other(super::AiError::NotCompiled.to_string())
}

/// `--chronicle`: narrate a headless run season by season into `chronicle.md`.
#[cfg(not(feature = "ai"))]
pub fn chronicle(_seed: u64, _ticks: u64, _params: &Params, _out: Option<&str>) -> io::Result<()> {
    Err(not_compiled())
}

/// `--design-species PROMPT`: write a validated `[[species]]` overlay file.
#[cfg(not(feature = "ai"))]
pub fn design_species(_prompt: &str, _out: Option<&str>, _params: &Params) -> io::Result<()> {
    Err(not_compiled())
}

#[cfg(feature = "ai")]
pub use real::{chronicle, design_species};

#[cfg(feature = "ai")]
mod real {
    use std::fmt::Write as _;
    use std::io;

    use crate::ai::client::Client;
    use crate::ai::prompt::{chronicle as chron_prompt, designer};
    use crate::ai::{Ai, AiError, Feature};
    use crate::sim::chronicle::{self, ChronicleEntry, Source};
    use crate::sim::{Params, Season, Sim};

    fn fail(e: impl std::fmt::Display) -> io::Error {
        io::Error::other(e.to_string())
    }

    /// The `[ai]` table from ui.toml, checked for this feature, with the
    /// gateway probed once so an unreachable URL fails before any stepping.
    fn start(feature: Feature) -> io::Result<Ai> {
        let cfg = crate::ui::config::load_ai();
        if !cfg.enabled {
            return Err(fail(AiError::Off));
        }
        if !cfg.feature_on(feature.key()) {
            return Err(fail(AiError::NoModel(feature.key())));
        }
        Client::new(&cfg).probe().map_err(fail)?;
        Ok(Ai::start(&cfg))
    }

    /// One blocking chronicle call for the season that just ended.
    fn narrate(ai: &Ai, sim: &Sim, year: u32, season: Season, previous: Option<&str>) -> io::Result<ChronicleEntry> {
        let input = chron_prompt::from_sim(sim, year, season, previous);
        let id = ai.request(Feature::Chronicle, chron_prompt::messages(&input), None, false).map_err(fail)?;
        let text = ai.wait(id).map_err(fail)?;
        Ok(ChronicleEntry { year, season, text: text.trim().to_string(), source: Source::Model })
    }

    /// `--chronicle`: step `ticks`, narrate every completed season, and write
    /// Markdown to `out` (default `chronicle.md` in the cwd).
    pub fn chronicle(seed: u64, ticks: u64, params: &Params, out: Option<&str>) -> io::Result<()> {
        let ai = start(Feature::Chronicle)?;
        let mut sim = Sim::new(seed, params.clone());
        let mut md = format!("# Chronicle of seed {seed}\n\n");
        let mut previous: Option<String> = None;
        let mut current = (sim.time.year(), sim.time.season());
        for _ in 0..ticks {
            sim.step();
            let now = (sim.time.year(), sim.time.season());
            if now != current {
                let entry = narrate(&ai, &sim, current.0, current.1, previous.as_deref())?;
                let tally = chronicle::template_entry(&sim, current.0, current.1);
                let _ = write!(md, "## Year {}, {}\n\n{}\n\n*{}*\n\n", entry.year, entry.season.name(), entry.text, tally.text.replace('\n', " "));
                previous = Some(entry.text.clone());
                sim.chronicle.push(entry);
                current = now;
            }
        }
        let path = out.unwrap_or("chronicle.md");
        std::fs::write(path, md)?;
        println!("chronicle: {} seasons written to {path}", sim.chronicle.len());
        Ok(())
    }

    /// `--design-species PROMPT [--out FILE]`: one request, at most one
    /// correction round, then a validated overlay file (default
    /// `species-<name>.toml` in the cwd).
    pub fn design_species(prompt_text: &str, out: Option<&str>, params: &Params) -> io::Result<()> {
        let ai = start(Feature::Designer)?;
        let mut previous: Option<(String, String)> = None;
        for attempt in 0..2 {
            let prev = previous.as_ref().map(|(r, e)| (r.as_str(), e.as_str()));
            let msgs = designer::messages(prompt_text, &params.species, prev);
            let id = ai.request(Feature::Designer, msgs, Some(designer::SCHEMA.to_string()), false).map_err(fail)?;
            let reply = ai.wait(id).map_err(fail)?;
            let checked = designer::overlay_from_reply(&reply).and_then(|o| designer::validated(params, &o.toml).map(|p| (o, p)));
            match checked {
                Ok((overlay, merged)) => {
                    let default_name = format!("species-{}.toml", overlay.name);
                    let path = out.unwrap_or(&default_name);
                    let text = format!("# Written by --design-species from: {prompt_text:?}\n# Pass it back with --params, or edit it.\n{}", overlay.toml);
                    std::fs::write(path, text)?;
                    println!("species overlay written to {path}");
                    for line in designer::summary(&merged, &overlay.name) {
                        println!("  {line}");
                    }
                    return Ok(());
                }
                Err(e) if attempt == 0 => {
                    eprintln!("first reply rejected ({e}); asking once more");
                    previous = Some((reply, e));
                }
                Err(e) => return Err(fail(AiError::Invalid(e))),
            }
        }
        Err(fail(AiError::Invalid("no usable reply".into())))
    }
}
