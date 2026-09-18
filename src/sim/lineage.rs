//! Lineage store (C4 FR6/FR8): one node per creature ever born (founders
//! included), bounded by generation pruning, plus the S08 tree builder.
//!
//! Only ordered collections are used so iteration is deterministic.

use std::collections::{BTreeMap, BTreeSet};

pub mod lifelog;
pub use lifelog::{LifeLog, LifeRecord, LIFE_WINDOW_DAYS};

use serde::{Deserialize, Serialize};

use crate::sim::creatures::{Cause, Creature, CreatureId, CreatureStore, Mutation, NameId, Sex};
use crate::sim::params::Roster;
use crate::sim::species::{Genome, SpeciesId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineageNode {
    pub id: CreatureId,
    pub name: NameId,
    pub tag: String,
    pub species: SpeciesId,
    pub sex: Sex,
    pub generation: u32,
    /// Negative for founders.
    pub born_day: i32,
    pub died_day: Option<u32>,
    pub genome: Genome,
    pub mutations: Vec<Mutation>,
    pub parents: Option<(CreatureId, CreatureId)>,
    /// Children in birth (id) order.
    pub children: Vec<CreatureId>,
    /// Carries a notable mutation, has ≥ 10 offspring, or survived ≥ 2 infections (C7).
    pub notable: bool,
    // ---- C7
    pub cause: Option<Cause>,
    /// The outbreak that killed it (absolute index into `Sim.disease.outbreaks`).
    pub outbreak: Option<u16>,
    pub infections_survived: u8,
}

impl LineageNode {
    pub const fn alive(&self) -> bool {
        self.died_day.is_none()
    }

    pub fn name_str<'r>(&self, roster: &'r Roster) -> &'r str {
        roster.name_for(self.species, self.name)
    }

    pub fn mother(&self) -> Option<CreatureId> {
        self.parents.map(|p| p.0)
    }

    pub fn father(&self) -> Option<CreatureId> {
        self.parents.map(|p| p.1)
    }
}

/// One drawn line of the S08 tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeItem {
    Node { id: CreatureId, depth: usize, prefix: String },
    /// `… and N more` for a truncated branch.
    More { depth: usize, prefix: String, count: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    pub root: CreatureId,
    pub focus: CreatureId,
    pub items: Vec<TreeItem>,
    /// Number of `Node` items (≤ `lineage_rows_max`).
    pub node_count: usize,
}

impl Tree {
    /// Ids of the drawn nodes in display order.
    pub fn node_ids(&self) -> Vec<CreatureId> {
        self.items
            .iter()
            .filter_map(|it| match it {
                TreeItem::Node { id, .. } => Some(*id),
                TreeItem::More { .. } => None,
            })
            .collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lineage {
    nodes: BTreeMap<CreatureId, LineageNode>,
    /// Deaths of the last 240 days with their outcome counters (S15, save 15).
    /// Unlike `nodes` it is never pruned by generation, only by age.
    lives: LifeLog,
}

impl Lineage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: CreatureId) -> Option<&LineageNode> {
        self.nodes.get(&id)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn nodes(&self) -> impl Iterator<Item = &LineageNode> {
        self.nodes.values()
    }

    /// Deaths of the last `LIFE_WINDOW_DAYS` days, oldest first.
    pub const fn lives(&self) -> &LifeLog {
        &self.lives
    }

    /// Log a finished life (called from `behavior::kill`).
    pub fn record_life(&mut self, rec: LifeRecord) {
        self.lives.push(rec);
    }

    /// Record a creature (founder or newborn). Links it into its parents'
    /// `children` lists and flags notability (FR6).
    pub fn record(&mut self, c: &Creature, roster: &Roster, mutation_notable: f32) {
        let notable = c.mutations.iter().any(|m| m.delta.abs() >= mutation_notable);
        self.nodes.insert(
            c.id,
            LineageNode {
                id: c.id,
                name: c.name,
                tag: c.tag(roster),
                species: c.species,
                sex: c.sex,
                generation: c.generation,
                born_day: c.born_day,
                died_day: None,
                genome: c.genome,
                mutations: c.mutations.clone(),
                parents: c.parents,
                children: Vec::new(),
                notable,
                cause: None,
                outbreak: None,
                infections_survived: 0,
            },
        );
        if let Some((m, f)) = c.parents {
            for p in <[CreatureId; 2]>::from((m, f)) {
                let Some(n) = self.nodes.get_mut(&p) else {
                    continue;
                };
                if !n.children.contains(&c.id) {
                    n.children.push(c.id);
                }
                if n.children.len() >= 10 {
                    n.notable = true;
                }
            }
        }
    }

    pub fn record_death(&mut self, id: CreatureId, day: u32, cause: Cause, outbreak: Option<u16>, survived: u8) {
        if let Some(n) = self.nodes.get_mut(&id) {
            n.died_day = Some(day);
            n.cause = Some(cause);
            n.outbreak = outbreak;
            n.infections_survived = survived;
            if survived >= 2 {
                n.notable = true;
            }
        }
    }

    /// Push the parents of `id` that exist and were not seen before, mother
    /// first, onto `out` and the next frontier.
    fn push_parents(
        &self,
        id: CreatureId,
        seen: &mut BTreeSet<CreatureId>,
        out: &mut Vec<CreatureId>,
        next: &mut Vec<CreatureId>,
    ) {
        let Some(n) = self.nodes.get(&id) else { return };
        let Some((m, fa)) = n.parents else { return };
        for p in <[CreatureId; 2]>::from((m, fa)) {
            if self.nodes.contains_key(&p) && seen.insert(p) {
                out.push(p);
                next.push(p);
            }
        }
    }

    /// Ancestors up to `depth` generations above `id` (breadth-first, mother
    /// before father, nearest first). Missing (pruned) ancestors are skipped.
    pub fn ancestors(&self, id: CreatureId, depth: u32) -> Vec<CreatureId> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut frontier = vec![id];
        for _ in 0..depth {
            let mut next = Vec::new();
            for f in frontier {
                self.push_parents(f, &mut seen, &mut out, &mut next);
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
        out
    }

    /// Push each unseen child of `id` onto `out` and the next frontier, in the
    /// stored child order. Returns false once `cap` is reached.
    fn push_unseen_children(
        &self,
        id: CreatureId,
        cap: usize,
        seen: &mut BTreeSet<CreatureId>,
        out: &mut Vec<CreatureId>,
        next: &mut Vec<CreatureId>,
    ) -> bool {
        let Some(n) = self.nodes.get(&id) else {
            return true;
        };
        for &ch in &n.children {
            if out.len() >= cap {
                return false;
            }
            if self.nodes.contains_key(&ch) && seen.insert(ch) {
                out.push(ch);
                next.push(ch);
            }
        }
        true
    }

    /// Descendants up to `max_depth` generations below `id`, breadth-first,
    /// at most `cap` ids.
    pub fn descendants(&self, id: CreatureId, max_depth: u32, cap: usize) -> Vec<CreatureId> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut frontier = vec![id];
        for _ in 0..max_depth {
            let mut next = Vec::new();
            for f in frontier {
                if !self.push_unseen_children(f, cap, &mut seen, &mut out, &mut next) {
                    return out;
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
        out
    }

    /// Living descendants (any depth, up to `cap` scanned) and how many of them
    /// are notable.
    pub fn living_descendants(&self, id: CreatureId, cap: usize) -> (usize, usize) {
        let mut living = 0;
        let mut notable = 0;
        for d in self.descendants(id, u32::MAX, cap) {
            if let Some(n) = self.nodes.get(&d) {
                if n.alive() {
                    living += 1;
                }
                if n.notable {
                    notable += 1;
                }
            }
        }
        (living, notable)
    }

    /// The root `up` generations above `focus` following the mother link (fewer
    /// when the chain ends early).
    pub fn root_of(&self, focus: CreatureId, up: u32) -> CreatureId {
        let mut cur = focus;
        for _ in 0..up {
            match self.nodes.get(&cur).and_then(LineageNode::mother) {
                Some(m) if self.nodes.contains_key(&m) => cur = m,
                _ => break,
            }
        }
        cur
    }

    /// Delete dead nodes with `generation < species_max_gen − keep` that are not
    /// ancestors of a living creature (FR6). Returns the number removed.
    pub fn prune(&mut self, species_max_gen: &[u32], keep: u32, store: &CreatureStore) -> usize {
        // Mark every ancestor of a living creature.
        let mut marked: BTreeSet<CreatureId> = BTreeSet::new();
        let mut stack: Vec<CreatureId> = store.living().map(|c| c.id).collect();
        while let Some(id) = stack.pop() {
            let Some(n) = self.nodes.get(&id) else {
                continue;
            };
            let Some((m, f)) = n.parents else {
                continue;
            };
            for p in <[CreatureId; 2]>::from((m, f)) {
                if marked.insert(p) {
                    stack.push(p);
                }
            }
        }
        let before = self.nodes.len();
        self.nodes.retain(|id, n| {
            if n.alive() || marked.contains(id) {
                return true;
            }
            let cutoff = species_max_gen[n.species.index()].saturating_sub(keep);
            n.generation >= cutoff
        });
        // Drop dangling child links.
        let ids: BTreeSet<CreatureId> = self.nodes.keys().copied().collect();
        for n in self.nodes.values_mut() {
            n.children.retain(|c| ids.contains(c));
        }
        before - self.nodes.len()
    }

    /// One breadth-first level of the S08 tree fill: every included id's tree
    /// children up to `max_gen`, keeping at most `rows_max` included nodes.
    fn expand_tree_level<F>(
        &self,
        frontier: Vec<CreatureId>,
        included: &mut BTreeSet<CreatureId>,
        max_gen: u32,
        rows_max: usize,
        tree_children: &F,
    ) -> Vec<CreatureId>
    where
        F: Fn(CreatureId) -> Vec<CreatureId>,
    {
        let mut next = Vec::new();
        for id in frontier {
            if !included.contains(&id) {
                continue;
            }
            for ch in tree_children(id) {
                let gen = self.nodes.get(&ch).map_or(u32::MAX, |n| n.generation);
                if gen > max_gen {
                    continue;
                }
                if included.contains(&ch) {
                    next.push(ch);
                } else if included.len() < rows_max && self.nodes.contains_key(&ch) {
                    included.insert(ch);
                    next.push(ch);
                }
            }
        }
        next
    }

    /// The S08 tree (FR8): always the root → focus mother chain, the focus's
    /// siblings, children and grandchildren; the remaining budget up to
    /// `rows_max` nodes is filled breadth-first from the root down to
    /// `focus.generation + 2`, with `… and N more` per truncated branch.
    pub fn tree(&self, focus: CreatureId, up: u32, rows_max: usize) -> Option<Tree> {
        let focus_node = self.nodes.get(&focus)?;
        let root = self.root_of(focus, up);
        let rows_max = rows_max.max(1);

        // Display parent: the mother, except that the focus's own children hang
        // under the focus even when it is the father.
        let display_parent = |child: &LineageNode| -> Option<CreatureId> {
            match child.parents {
                Some((m, f)) if f == focus => Some(if m == focus { m } else { f }),
                Some((m, _)) => Some(m),
                None => None,
            }
        };
        let tree_children = |id: CreatureId| -> Vec<CreatureId> {
            let mut v: Vec<CreatureId> = self
                .nodes
                .get(&id)
                .map(|n| {
                    n.children
                        .iter()
                        .copied()
                        .filter(|c| self.nodes.get(c).and_then(&display_parent) == Some(id))
                        .collect()
                })
                .unwrap_or_default();
            v.sort_unstable();
            v.dedup();
            v
        };

        let mut included: BTreeSet<CreatureId> = BTreeSet::new();
        let add = |included: &mut BTreeSet<CreatureId>, id: CreatureId| -> bool {
            if included.len() >= rows_max || !self.nodes.contains_key(&id) {
                return false;
            }
            included.insert(id);
            true
        };

        // 1. Root → focus chain (mother links).
        let mut chain = vec![focus];
        let mut cur = focus;
        while cur != root {
            match self.nodes.get(&cur).and_then(LineageNode::mother) {
                Some(m) if self.nodes.contains_key(&m) => {
                    chain.push(m);
                    cur = m;
                }
                _ => break,
            }
        }
        for id in chain.iter().rev() {
            add(&mut included, *id);
        }
        // 2. Siblings (other tree children of the focus's display parent).
        if let Some(p) = display_parent(focus_node) {
            if included.contains(&p) {
                for s in tree_children(p) {
                    add(&mut included, s);
                }
            }
        }
        // 3. Children and grandchildren.
        let kids = tree_children(focus);
        for &k in &kids {
            add(&mut included, k);
        }
        for &k in &kids {
            if included.contains(&k) {
                for g in tree_children(k) {
                    add(&mut included, g);
                }
            }
        }
        // 4. Breadth-first fill from the root down to focus.generation + 2.
        let max_gen = focus_node.generation.saturating_add(2);
        let mut frontier = vec![root];
        while !frontier.is_empty() && included.len() < rows_max {
            frontier = self.expand_tree_level(frontier, &mut included, max_gen, rows_max, &tree_children);
        }

        // Render: depth-first from the root over the included nodes.
        let mut items = Vec::new();
        let mut node_count = 0;
        walk(root, &included, &tree_children, 0, true, &mut Vec::new(), &mut items, &mut node_count);
        Some(Tree { root, focus, items, node_count })
    }
}

#[allow(clippy::too_many_arguments)]
fn walk(
    id: CreatureId,
    included: &BTreeSet<CreatureId>,
    tree_children: &dyn Fn(CreatureId) -> Vec<CreatureId>,
    depth: usize,
    is_last: bool,
    lasts: &mut Vec<bool>,
    items: &mut Vec<TreeItem>,
    node_count: &mut usize,
) {
    let mut prefix = String::new();
    for &l in lasts.iter() {
        prefix.push_str(if l { "    " } else { "│   " });
    }
    if depth > 0 {
        prefix.push_str(if is_last { "└── " } else { "├── " });
    }
    items.push(TreeItem::Node { id, depth, prefix });
    *node_count += 1;

    let all = tree_children(id);
    let shown: Vec<CreatureId> = all.iter().copied().filter(|c| included.contains(c)).collect();
    let hidden = all.len() - shown.len();
    if depth > 0 {
        lasts.push(is_last);
    }
    for (i, &c) in shown.iter().enumerate() {
        let last = i + 1 == shown.len() && hidden == 0;
        walk(c, included, tree_children, depth + 1, last, lasts, items, node_count);
    }
    if hidden > 0 {
        let mut p = String::new();
        for &l in lasts.iter() {
            p.push_str(if l { "    " } else { "│   " });
        }
        p.push_str("└── ");
        items.push(TreeItem::More { depth: depth + 1, prefix: p, count: hidden });
    }
    if depth > 0 {
        lasts.pop();
    }
}
