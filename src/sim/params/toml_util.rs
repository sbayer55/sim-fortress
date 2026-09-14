//! TOML deep-merge and comment-attachment helpers.


/// Recursively merge `overlay` onto `base`: tables deep-merge, anything else is
/// replaced by the overlay value.
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
        (_, overlay) => overlay,
    }
}

/// Attach a `# comment` before the leaf key at `path` in a `toml_edit` document.
pub(super) fn attach_comment(doc: &mut toml_edit::DocumentMut, path: &str, comment: &str) {
    let parts: Vec<&str> = path.split('.').collect();
    let (tables, leaf) = parts.split_at(parts.len().saturating_sub(1));
    let Some(leaf) = leaf.first() else { return };
    let mut table = doc.as_table_mut();
    for part in tables {
        match table.get_mut(part).and_then(|i| i.as_table_mut()) {
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
