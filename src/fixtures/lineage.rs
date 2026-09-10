//! Family tree for the hero wolf, Ashfang.

#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub tag: String,
    pub generation: u32,
    pub born_year: u32,
    pub died_year: Option<u32>,
    pub mutations: Vec<String>,
    pub children: Vec<usize>,
    pub notable: bool,
}

pub struct Lineage {
    pub nodes: Vec<Node>,
    pub root: usize,
    pub focus: usize,
}

pub fn generate() -> Lineage {
    let mut nodes = Vec::new();
    let mut add = |name: &str, tag: &str, gen: u32, born: u32, died: Option<u32>, muts: &[&str], notable: bool| {
        nodes.push(Node {
            name: name.into(),
            tag: tag.into(),
            generation: gen,
            born_year: born,
            died_year: died,
            mutations: muts.iter().map(|s| s.to_string()).collect(),
            children: Vec::new(),
            notable,
        });
        nodes.len() - 1
    };
    let fenrir = add("Fenrir", "w#003", 17, 1, Some(5), &[], false);
    let rime = add("Rime", "w#006", 18, 2, Some(6), &["Size +0.11"], false);
    let howl = add("Howl", "w#009", 18, 2, Some(7), &[], false);
    let greymaw = add("Greymaw", "w#017", 20, 4, Some(9), &["Aggression +0.09"], true);
    let sable = add("Sable", "w#021", 21, 5, Some(10), &["Sense +0.05"], false);
    let scorch = add("Scorch", "w#019", 20, 4, Some(8), &[], false);
    let umber = add("Umber", "w#024", 21, 5, None, &[], false);
    let ashfang = add("Ashfang", "w#042", 23, 8, None, &["Camouflage -0.07"], true);
    let flint = add("Flint", "w#044", 23, 8, Some(11), &[], false);
    let cinder = add("Cinder", "w#051", 24, 9, None, &["Speed +0.04"], false);
    let ember = add("Ember", "w#053", 24, 9, None, &[], false);
    let rook = add("Rook", "w#058", 24, 10, None, &["Metabolism -0.06"], true);
    let shade = add("Shade", "w#060", 24, 10, Some(11), &[], false);
    let vex = add("Vex", "w#071", 25, 11, None, &[], false);
    let snarl = add("Snarl", "w#072", 25, 11, None, &["Aggression +0.05"], false);
    let dusk = add("Dusk", "w#075", 25, 12, None, &[], false);
    let talon = add("Talon", "w#076", 25, 12, None, &["Sense +0.08"], true);
    let gloam = add("Gloam", "w#079", 25, 12, None, &[], false);
    let brindle = add("Brindle", "w#080", 25, 12, None, &[], false);

    let link = |nodes: &mut Vec<Node>, p: usize, cs: &[usize]| nodes[p].children.extend_from_slice(cs);
    link(&mut nodes, fenrir, &[rime, howl]);
    link(&mut nodes, rime, &[greymaw, scorch]);
    link(&mut nodes, howl, &[sable]);
    link(&mut nodes, greymaw, &[ashfang, flint]);
    link(&mut nodes, scorch, &[umber]);
    link(&mut nodes, ashfang, &[cinder, ember, rook, shade]);
    link(&mut nodes, cinder, &[vex, snarl]);
    link(&mut nodes, rook, &[dusk, talon, gloam]);
    link(&mut nodes, ember, &[brindle]);
    let _ = sable;
    Lineage { nodes, root: fenrir, focus: ashfang }
}
