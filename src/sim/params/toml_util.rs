//! TOML deep-merge and comment-attachment helpers.

/// Recursively merge `overlay` onto `base`: tables deep-merge, arrays of named
/// tables (`[[species]]`, `[[disease.pathogens]]`) merge element-wise by their
/// `name` (a known name deep-merges in place, a new name appends), and anything
/// else is replaced by the overlay value.
pub(super) fn deep_merge(base: toml::Value, overlay: toml::Value) -> toml::Value {
    match (base, overlay) {
        (toml::Value::Table(mut b), toml::Value::Table(o)) => {
            for (k, v) in o {
                match b.remove(&k) {
                    Some(existing) => b.insert(k, deep_merge(existing, v)),
                    None => b.insert(k, v),
                };
            }
            toml::Value::Table(b)
        }
        (toml::Value::Array(b), toml::Value::Array(o)) if is_named_array(&b) && is_named_array(&o) => {
            toml::Value::Array(merge_named(b, o))
        }
        (_, overlay) => overlay,
    }
}

/// Every element is a table with a string `name`.
fn is_named_array(items: &[toml::Value]) -> bool {
    !items.is_empty() && items.iter().all(|v| name_of(v).is_some())
}

fn name_of(v: &toml::Value) -> Option<&str> {
    v.as_table().and_then(|t| t.get("name")).and_then(toml::Value::as_str)
}

fn merge_named(mut base: Vec<toml::Value>, overlay: Vec<toml::Value>) -> Vec<toml::Value> {
    for item in overlay {
        let pos = name_of(&item).and_then(|n| base.iter().position(|b| name_of(b) == Some(n)));
        match pos {
            Some(i) => {
                let existing = std::mem::replace(&mut base[i], toml::Value::Boolean(false));
                base[i] = deep_merge(existing, item);
            }
            None => base.push(item),
        }
    }
    base
}

/// Attach a `# comment` before the leaf key at `path` in a `toml_edit` document.
/// A path segment that names an array of tables descends into its first table.
pub(super) fn attach_comment(doc: &mut toml_edit::DocumentMut, path: &str, comment: &str) {
    let parts: Vec<&str> = path.split('.').collect();
    let (tables, leaf) = parts.split_at(parts.len().saturating_sub(1));
    let Some(leaf) = leaf.first() else { return };
    let mut table = doc.as_table_mut();
    for part in tables {
        let Some(item) = table.get_mut(part) else { return };
        if item.is_array_of_tables() {
            match item.as_array_of_tables_mut().and_then(|a| a.iter_mut().next()) {
                Some(t) => table = t,
                None => return,
            }
            continue;
        }
        match item.as_table_mut() {
            Some(t) => table = t,
            None => return,
        }
    }
    let decor = format!("# {comment}\n");
    if let Some(item) = table.get_mut(leaf) {
        if let Some(t) = item.as_table_mut() {
            t.decor_mut().set_prefix(decor);
            return;
        }
        // An array of tables (`[[disease.pathogens]]`): comment the first table,
        // never the key, or the comment ends up inside the header brackets.
        if let Some(arr) = item.as_array_of_tables_mut() {
            if let Some(first) = arr.iter_mut().next() {
                first.decor_mut().set_prefix(decor);
            }
            return;
        }
    }
    if let Some(mut key) = table.key_mut(leaf) {
        key.leaf_decor_mut().set_prefix(decor);
    }
}

#[cfg(test)]
mod tests {
    use super::super::Params;

    #[test]
    fn overlay_appends_species_by_name() {
        let mut p = Params::default();
        p.apply_overlay("[[species]]\nname = \"boar\"\nplural = \"Boars\"\nglyph = \"b\"\n").unwrap();
        assert_eq!(p.species.len(), 7);
        assert_eq!(p.species.0[6].name, "boar");
        // The six originals are untouched and keep their positions.
        assert_eq!(p.species.0[..6], Params::default().species.0[..]);
    }

    #[test]
    fn overlay_edits_species_by_name() {
        let mut p = Params::default();
        p.apply_overlay("[[species]]\nname = \"vole\"\ninitial_count = 400\n").unwrap();
        assert_eq!(p.species.len(), 6);
        assert_eq!(p.species.0[0].initial_count, 400);
        assert_eq!(p.species.0[0].plural, "Voles");
    }

    #[test]
    fn pathogen_overlay_merges_by_name() {
        let mut p = Params::default();
        p.apply_overlay("[[disease.pathogens]]\nname = \"Greyfever\"\nseverity = 0.1\n").unwrap();
        assert_eq!(p.disease.pathogens.len(), 3);
        assert!((p.disease.pathogens[0].severity - 0.1).abs() < 1e-6);
        assert_eq!(p.disease.pathogens[0].hosts.len(), 3);
    }

    #[test]
    fn unnamed_arrays_still_replace() {
        let mut p = Params::default();
        p.apply_overlay("[genetics]\nbreeding_seasons = [\"spring\"]\n").unwrap();
        assert_eq!(p.genetics.breeding_seasons.len(), 1);
    }
}
