//! Cross-field validation of a parsed `Params`: everything a `[[species]]`
//! overlay can get wrong that serde cannot catch.

use super::Params;
use crate::sim::species::Kind;

impl Params {
    /// Reject rosters the simulation cannot run: the error names the rule.
    pub fn validate(&self) -> Result<(), String> {
        let roster = &self.species;
        if roster.is_empty() {
            return Err("species roster is empty".into());
        }
        if roster.len() > 255 {
            return Err(format!("species roster has {} entries; at most 255", roster.len()));
        }
        for (i, s) in roster.0.iter().enumerate() {
            validate_species(i, s)?;
            if roster.0.iter().take(i).any(|o| o.name == s.name) {
                return Err(format!("species '{}' is defined twice", s.name));
            }
            for (prey, share) in &s.prey_preference {
                let Some(pos) = roster.position(prey) else {
                    return Err(format!("species '{}' prey_preference names unknown species '{prey}'", s.name));
                };
                if roster.0[pos].kind != Kind::Prey {
                    return Err(format!("species '{}' prey_preference names '{prey}', which is not prey", s.name));
                }
                if *share < 0.0 {
                    return Err(format!("species '{}' prey_preference for '{prey}' is negative", s.name));
                }
            }
        }
        self.territory.validate()?;
        for p in &self.disease.pathogens {
            for host in p.hosts.keys() {
                if roster.position(host).is_none() {
                    return Err(format!("pathogen '{}' hosts names unknown species '{host}'", p.name));
                }
            }
        }
        Ok(())
    }
}

fn validate_species(i: usize, s: &super::SpeciesParams) -> Result<(), String> {
    if s.name.is_empty() {
        return Err(format!("species #{i} has no name"));
    }
    if !s.name.chars().all(|c| c.is_ascii_lowercase()) {
        return Err(format!("species '{}': name must be ASCII lowercase letters", s.name));
    }
    if s.plural.is_empty() {
        return Err(format!("species '{}' has no plural", s.name));
    }
    if !s.glyph.is_ascii_lowercase() || !crate::glyphs::is_cp437(s.glyph) {
        return Err(format!("species '{}': glyph {:?} must be an ASCII lowercase letter", s.name, s.glyph));
    }
    for (t, v) in s.base_genome.genome().0.iter().enumerate() {
        if !(0.02..=0.98).contains(v) {
            return Err(format!("species '{}': base_genome trait {t} = {v} is outside 0.02..=0.98", s.name));
        }
    }
    match s.kind {
        Kind::Predator if !s.prey_preference.values().any(|v| *v > 0.0) => {
            return Err(format!("predator '{}' has no prey_preference above 0", s.name));
        }
        Kind::Prey if !s.prey_preference.is_empty() => {
            return Err(format!("prey '{}' must not have a prey_preference", s.name));
        }
        _ => {}
    }
    if s.names.iter().any(String::is_empty) {
        return Err(format!("species '{}': names contains an empty entry", s.name));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlay(s: &str) -> Result<(), String> {
        let mut p = Params::default();
        p.apply_overlay(s)
    }

    #[test]
    fn default_roster_is_valid() {
        Params::default().validate().unwrap();
    }

    #[test]
    fn duplicate_name_rejected() {
        let mut p = Params::default();
        let vole = p.species.0[0].clone();
        p.species.0.push(vole);
        assert!(p.validate().unwrap_err().contains("twice"));
    }

    #[test]
    fn unknown_prey_rejected() {
        let e = overlay("[[species]]\nname = \"wolf\"\n[species.prey_preference]\nboar = 0.3\n").unwrap_err();
        assert!(e.contains("unknown species 'boar'"), "{e}");
    }

    #[test]
    fn predator_as_prey_rejected() {
        let e = overlay("[[species]]\nname = \"wolf\"\n[species.prey_preference]\nfox = 0.3\n").unwrap_err();
        assert!(e.contains("not prey"), "{e}");
    }

    #[test]
    fn bad_glyph_rejected() {
        let e = overlay("[[species]]\nname = \"boar\"\nplural = \"Boars\"\nglyph = \"☃\"\n").unwrap_err();
        assert!(e.contains("glyph"), "{e}");
        let e = overlay("[[species]]\nname = \"boar\"\nplural = \"Boars\"\nglyph = \"B\"\n").unwrap_err();
        assert!(e.contains("glyph"), "{e}");
    }

    #[test]
    fn genome_out_of_range_rejected() {
        let e = overlay("[[species]]\nname = \"vole\"\n[species.base_genome]\nspeed = 1.5\n").unwrap_err();
        assert!(e.contains("0.02..=0.98"), "{e}");
    }

    #[test]
    fn predator_without_prey_rejected() {
        let e = overlay("[[species]]\nname = \"cat\"\nplural = \"Cats\"\nglyph = \"c\"\nkind = \"predator\"\n").unwrap_err();
        assert!(e.contains("no prey_preference"), "{e}");
    }

    #[test]
    fn prey_with_preference_rejected() {
        let e = overlay("[[species]]\nname = \"vole\"\n[species.prey_preference]\nhare = 0.3\n").unwrap_err();
        assert!(e.contains("must not have"), "{e}");
    }

    #[test]
    fn missing_name_and_bad_name_rejected() {
        let e = overlay("[[species]]\nplural = \"Boars\"\nglyph = \"b\"\n").unwrap_err();
        assert!(e.contains("no name"), "{e}");
        let e = overlay("[[species]]\nname = \"Boar\"\nplural = \"Boars\"\nglyph = \"b\"\n").unwrap_err();
        assert!(e.contains("lowercase"), "{e}");
    }

    #[test]
    fn unknown_pathogen_host_rejected() {
        let e = overlay("[[disease.pathogens]]\nname = \"Greyfever\"\n[disease.pathogens.hosts]\nboar = 1.0\n").unwrap_err();
        assert!(e.contains("unknown species 'boar'"), "{e}");
    }

    #[test]
    fn empty_roster_rejected() {
        let mut p = Params::default();
        p.species.0.clear();
        assert!(p.validate().unwrap_err().contains("empty"));
    }

    #[test]
    fn valid_new_species_accepted() {
        let mut p = Params::default();
        p.apply_overlay(
            "[[species]]\nname = \"boar\"\nplural = \"Boars\"\nglyph = \"b\"\ncolor = [120, 90, 60]\ninitial_count = 40\n\
             [[species]]\nname = \"wolf\"\n[species.prey_preference]\nboar = 0.3\n",
        )
        .unwrap();
        assert_eq!(p.species.len(), 7);
        assert_eq!(p.species.0[6].name, "boar");
        let wolf = &p.species.0[4];
        assert!((wolf.prey_preference["boar"] - 0.3).abs() < 1e-6);
        // The wolf keeps its other prey.
        assert!((wolf.prey_preference["deer"] - 0.5).abs() < 1e-6);
    }
}
