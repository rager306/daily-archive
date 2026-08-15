//! Navigation property invariants for the ordered-section tree model
//! (M398 S01 T02).
//!
//! Exercises the pure [`compute_section_navigation_edges`] helper — no graph
//! IO — against:
//!
//! 1. Deterministic fixtures: deep trees with descendant interruption, legacy
//!    flat sections, and GROBID restart suffixes (`~N`).
//! 2. Proptest-generated ordered forests up to five levels. The generator
//!    mimics the GROBID parser: `n`-attribute path components with `~N`
//!    collision suffixes on restart, so the unique-path-suffix case is
//!    exercised on every run.
//!
//! Asserted invariants: same-parent sibling edges (direct even when
//! descendants intervene), unique successor/predecessor, acyclic complete
//! next_sibling chains, correct first_child and parent identity, and restart
//! paths never treated as dotted parents.

use std::collections::{HashMap, HashSet};

use da_application::ingest::{NavEdge, NavRelation, compute_section_navigation_edges};
use da_ports::parser::Section;
use proptest::prelude::*;
use proptest::strategy::{BoxedStrategy, Strategy};

// ─── Helpers ─────────────────────────────────────────────────────────

fn mk_sec(
    title: &str,
    path: Option<&str>,
    sid: Option<&str>,
    parent: Option<&str>,
    order: Option<u32>,
) -> Section {
    Section {
        title: title.into(),
        text: "text".into(),
        level: path.map(|p| p.split('.').count() as u32).unwrap_or(0),
        path_id: path.map(String::from),
        section_id: sid.map(String::from),
        parent_section_id: parent.map(String::from),
        order_in_parent: order,
    }
}

fn id_map(pairs: &[(&str, u64)]) -> HashMap<String, u64> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
}

fn edge_set(edges: &[NavEdge]) -> HashSet<(u64, u64, NavRelation)> {
    edges.iter().map(|e| (e.from, e.to, e.relation)).collect()
}

/// Parent path of a dotted path_id: everything before the last '.'.
/// Mirrors the model's canonical parent identity (dot-free `~N` restart
/// suffixes never introduce a hierarchy level).
fn parent_of_path(path: &str) -> Option<&str> {
    path.rfind('.').map(|pos| &path[..pos])
}

// ─── Deterministic fixtures ──────────────────────────────────────────

#[test]
fn flat_sections_without_paths_produce_no_navigation_edges() {
    // Legacy flat-section parser output: no path_id / section_id populated.
    let sections = vec![
        mk_sec("Intro", None, None, None, None),
        mk_sec("Methods", None, None, None, None),
    ];
    let map = HashMap::new();
    let edges = compute_section_navigation_edges(&sections, &map);
    assert!(
        edges.is_empty(),
        "legacy flat sections must emit zero navigation edges"
    );
}

#[test]
fn deep_tree_descendant_interruption_emits_direct_sibling_edges() {
    // Deep-tree regression fixture (M398 S01 T01, now via the pure model):
    // siblings whose direct successor is hidden behind descendants.
    //   1 (root)
    //   1.1 | 1.1.1 (descends between 1.1 and its sibling 1.2)
    //   1.2 | 1.2.1 (descends between 1.2 and its sibling 1.3)
    //   1.3
    //   2 (root) | 2.1
    let sections = vec![
        mk_sec("Intro", Some("1"), Some("sec-1"), None, Some(0)),
        mk_sec(
            "Background",
            Some("1.1"),
            Some("sec-1.1"),
            Some("sec-1"),
            Some(0),
        ),
        mk_sec(
            "Sub",
            Some("1.1.1"),
            Some("sec-1.1.1"),
            Some("sec-1.1"),
            Some(0),
        ),
        mk_sec(
            "Motivation",
            Some("1.2"),
            Some("sec-1.2"),
            Some("sec-1"),
            Some(1),
        ),
        mk_sec(
            "Deep",
            Some("1.2.1"),
            Some("sec-1.2.1"),
            Some("sec-1.2"),
            Some(0),
        ),
        mk_sec(
            "Related",
            Some("1.3"),
            Some("sec-1.3"),
            Some("sec-1"),
            Some(2),
        ),
        mk_sec("Methods", Some("2"), Some("sec-2"), None, Some(1)),
        mk_sec(
            "Model",
            Some("2.1"),
            Some("sec-2.1"),
            Some("sec-2"),
            Some(0),
        ),
    ];
    let map = id_map(&[
        ("sec-1", 0),
        ("sec-1.1", 1),
        ("sec-1.1.1", 2),
        ("sec-1.2", 3),
        ("sec-1.2.1", 4),
        ("sec-1.3", 5),
        ("sec-2", 6),
        ("sec-2.1", 7),
    ]);
    let edges = compute_section_navigation_edges(&sections, &map);
    let expected: HashSet<(u64, u64, NavRelation)> = [
        // root sibling past all of 1's subtree: 1 → 2
        (0, 6, NavRelation::NextSibling),
        // 1 → first child 1.1
        (0, 1, NavRelation::FirstChild),
        // 1.1 → parent 1
        (1, 0, NavRelation::ParentSection),
        // 1.1 → next sibling 1.2, DIRECT past descendant 1.1.1
        (1, 3, NavRelation::NextSibling),
        // 1.1 → first child 1.1.1
        (1, 2, NavRelation::FirstChild),
        // 1.1.1 → parent 1.1
        (2, 1, NavRelation::ParentSection),
        // 1.2 → parent 1
        (3, 0, NavRelation::ParentSection),
        // 1.2 → next sibling 1.3, DIRECT past descendant 1.2.1
        (3, 5, NavRelation::NextSibling),
        // 1.2 → first child 1.2.1
        (3, 4, NavRelation::FirstChild),
        // 1.2.1 → parent 1.2
        (4, 3, NavRelation::ParentSection),
        // 1.3 → parent 1
        (5, 0, NavRelation::ParentSection),
        // 2 → first child 2.1
        (6, 7, NavRelation::FirstChild),
        // 2.1 → parent 2
        (7, 6, NavRelation::ParentSection),
    ]
    .into_iter()
    .collect();
    assert_eq!(edge_set(&edges), expected, "deep-tree direct sibling edges");
    assert_eq!(edges.len(), 13, "expected exactly 13 navigation edges");
}

#[test]
fn grobid_restart_suffix_is_not_a_dotted_parent() {
    // GROBID restart: a second root div n="1" collides → path "1~2"; its
    // child n="1" under parent "1~2" gets path "1~2.1". The `~2` suffix is
    // dot-free, so "1~2" stays a ROOT section and "1~2.1" belongs to parent
    // "1~2" — never to "1" (no dotted-parent misreading).
    let sections = vec![
        mk_sec("1", Some("1"), Some("sec-1"), None, Some(0)),
        mk_sec("1.1", Some("1.1"), Some("sec-1.1"), Some("sec-1"), Some(0)),
        mk_sec("2", Some("2"), Some("sec-2"), None, Some(1)),
        mk_sec("restart", Some("1~2"), Some("sec-1~2"), None, Some(0)),
        mk_sec(
            "restart child",
            Some("1~2.1"),
            Some("sec-1~2.1"),
            Some("sec-1~2"),
            Some(0),
        ),
    ];
    let map = id_map(&[
        ("sec-1", 0),
        ("sec-1.1", 1),
        ("sec-2", 2),
        ("sec-1~2", 3),
        ("sec-1~2.1", 4),
    ]);
    let edges = compute_section_navigation_edges(&sections, &map);
    let expected: HashSet<(u64, u64, NavRelation)> = [
        (0, 2, NavRelation::NextSibling),   // 1 → 2
        (0, 1, NavRelation::FirstChild),    // 1 → 1.1
        (1, 0, NavRelation::ParentSection), // 1.1 → 1
        (2, 3, NavRelation::NextSibling),   // 2 → 1~2 (restart root is a root sibling)
        (3, 4, NavRelation::FirstChild),    // 1~2 → 1~2.1
        (4, 3, NavRelation::ParentSection), // 1~2.1 → 1~2 (NOT → 1)
    ]
    .into_iter()
    .collect();
    assert_eq!(
        edge_set(&edges),
        expected,
        "restart suffix must not create dotted parents"
    );
    // Explicit traps:
    assert!(
        !edges
            .iter()
            .any(|e| e.from == 3 && e.relation == NavRelation::ParentSection),
        "restart root 1~2 must not have a parent_section edge"
    );
    assert!(
        !edges
            .iter()
            .any(|e| e.from == 4 && e.to == 0 && e.relation == NavRelation::ParentSection),
        "1~2.1 must not be parented to 1"
    );
    assert!(
        edges
            .iter()
            .any(|e| e.from == 4 && e.to == 3 && e.relation == NavRelation::ParentSection),
        "1~2.1 must be parented to 1~2"
    );
}

#[test]
fn nested_restart_suffix_shares_parent_with_colliding_sibling() {
    // Under root 1, two divs n="1": the first gets "1.1", the second
    // collides → "1.1~2". Both are siblings under parent "1": "1.1~2" must
    // NOT become a child of "1.1", and "1.1" must NOT gain a first_child.
    let sections = vec![
        mk_sec("1", Some("1"), Some("sec-1"), None, Some(0)),
        mk_sec("a", Some("1.1"), Some("sec-1.1"), Some("sec-1"), Some(0)),
        mk_sec(
            "b",
            Some("1.1~2"),
            Some("sec-1.1~2"),
            Some("sec-1"),
            Some(1),
        ),
    ];
    let map = id_map(&[("sec-1", 0), ("sec-1.1", 1), ("sec-1.1~2", 2)]);
    let edges = compute_section_navigation_edges(&sections, &map);
    let expected: HashSet<(u64, u64, NavRelation)> = [
        (0, 1, NavRelation::FirstChild),    // 1 → 1.1
        (1, 2, NavRelation::NextSibling),   // 1.1 → 1.1~2 (direct)
        (1, 0, NavRelation::ParentSection), // 1.1 → 1
        (2, 0, NavRelation::ParentSection), // 1.1~2 → 1 (sibling, not child of 1.1)
    ]
    .into_iter()
    .collect();
    assert_eq!(
        edge_set(&edges),
        expected,
        "nested restart suffix shares the parent"
    );
    assert!(
        !edges
            .iter()
            .any(|e| e.from == 1 && e.relation == NavRelation::FirstChild),
        "1.1 must not gain a first_child from the restarted sibling"
    );
}

// ─── Proptest: ordered-forest generator (GROBID-faithful) ────────────

/// Maximum tree depth: 5 levels (root = level 1 .. deepest = level 5).
const MAX_LEVELS: u32 = 5;

#[derive(Debug, Clone)]
struct GenNode {
    /// GROBID div `n` attribute (last path component; may repeat → restart).
    n: u32,
    children: Vec<GenNode>,
}

#[derive(Debug, Clone)]
struct ForestNode {
    n: u32,
    children: Vec<usize>,
}

/// Arena of a generated ordered forest. Node index == section index in
/// pre-order serialization; roots are the top-level section nodes.
#[derive(Debug, Clone)]
struct Forest {
    nodes: Vec<ForestNode>,
    roots: Vec<usize>,
}

fn flatten(node: GenNode, arena: &mut Vec<ForestNode>) -> usize {
    let idx = arena.len();
    arena.push(ForestNode {
        n: node.n,
        children: Vec::new(),
    });
    let children: Vec<usize> = node
        .children
        .into_iter()
        .map(|c| flatten(c, arena))
        .collect();
    arena[idx].children = children;
    idx
}

impl Forest {
    /// Serialize the arena to GROBID-style sections in pre-order, returning
    /// the sections plus a `section_id` → arena-index map (the graph nodes).
    /// Duplicate paths get `~N` collision suffixes exactly like the parser.
    fn to_sections(&self) -> (Vec<Section>, HashMap<String, u64>) {
        let mut sections = Vec::new();
        let mut id_map = HashMap::new();
        let mut used: HashSet<String> = HashSet::new();
        for (root_idx, &root) in self.roots.iter().enumerate() {
            self.walk(
                root,
                "",
                root_idx as u32,
                &mut used,
                &mut sections,
                &mut id_map,
            );
        }
        (sections, id_map)
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        &self,
        node: usize,
        parent_path: &str,
        order_in_parent: u32,
        used: &mut HashSet<String>,
        out: &mut Vec<Section>,
        id_map: &mut HashMap<String, u64>,
    ) {
        let component = self.nodes[node].n.to_string();
        let base_path = if parent_path.is_empty() {
            component
        } else {
            format!("{parent_path}.{component}")
        };
        // GROBID collision suffixing on restart (dot-free `~N`).
        let mut path_id = base_path.clone();
        let mut collision = 2u32;
        while used.contains(&path_id) {
            path_id = format!("{base_path}~{collision}");
            collision += 1;
        }
        used.insert(path_id.clone());
        let parent_section_id = if parent_path.is_empty() {
            None
        } else {
            Some(format!("sec-{parent_path}"))
        };
        out.push(Section {
            title: format!("s{path_id}"),
            text: String::new(),
            level: path_id.split('.').count() as u32,
            path_id: Some(path_id.clone()),
            section_id: Some(format!("sec-{path_id}")),
            parent_section_id,
            order_in_parent: Some(order_in_parent),
        });
        id_map.insert(format!("sec-{path_id}"), node as u64);
        for (local_idx, &child) in self.nodes[node].children.iter().enumerate() {
            self.walk(child, &path_id, local_idx as u32, used, out, id_map);
        }
    }

    /// Ground-truth navigation edges derived DIRECTLY from the tree structure
    /// (parent/child/sibling arrays) — independent of dotted-path parsing, so
    /// it is not a common-mode oracle with the model under test.
    fn expected_navigation_edges(&self) -> Vec<NavEdge> {
        let mut edges = Vec::new();
        for (p, node) in self.nodes.iter().enumerate() {
            for (i, &child) in node.children.iter().enumerate() {
                edges.push(NavEdge {
                    from: child as u64,
                    to: p as u64,
                    relation: NavRelation::ParentSection,
                });
                if i == 0 {
                    edges.push(NavEdge {
                        from: p as u64,
                        to: child as u64,
                        relation: NavRelation::FirstChild,
                    });
                } else {
                    let prev = node.children[i - 1];
                    edges.push(NavEdge {
                        from: prev as u64,
                        to: child as u64,
                        relation: NavRelation::NextSibling,
                    });
                }
            }
        }
        for pair in self.roots.windows(2) {
            edges.push(NavEdge {
                from: pair[0] as u64,
                to: pair[1] as u64,
                relation: NavRelation::NextSibling,
            });
        }
        edges
    }

    /// For every node, the last sibling of its sibling group (roots form one
    /// group). Used to assert chain completeness.
    fn last_sibling_by_group(&self) -> HashMap<usize, usize> {
        let mut m = HashMap::new();
        if let Some(&last) = self.roots.last() {
            for &r in &self.roots {
                m.insert(r, last);
            }
        }
        for node in &self.nodes {
            if let Some(&last) = node.children.last() {
                for &c in &node.children {
                    m.insert(c, last);
                }
            }
        }
        m
    }
}

fn gen_node(levels_left: u32) -> BoxedStrategy<GenNode> {
    if levels_left == 0 {
        (1u32..=3)
            .prop_map(|n| GenNode {
                n,
                children: Vec::new(),
            })
            .boxed()
    } else {
        (
            1u32..=3,
            proptest::collection::vec(gen_node(levels_left - 1), 0..=3),
        )
            .prop_map(|(n, children)| GenNode { n, children })
            .boxed()
    }
}

/// Every non-leaf has at least one child, so descendant interruption of
/// sibling pairs is guaranteed at every depth.
fn deep_node(levels_left: u32) -> BoxedStrategy<GenNode> {
    if levels_left == 0 {
        (1u32..=3)
            .prop_map(|n| GenNode {
                n,
                children: Vec::new(),
            })
            .boxed()
    } else {
        (
            1u32..=3,
            proptest::collection::vec(deep_node(levels_left - 1), 1..=3),
        )
            .prop_map(|(n, children)| GenNode { n, children })
            .boxed()
    }
}

fn forest_from_roots(roots: Vec<GenNode>) -> Forest {
    let mut arena = Vec::new();
    let root_idxs: Vec<usize> = roots.into_iter().map(|r| flatten(r, &mut arena)).collect();
    Forest {
        nodes: arena,
        roots: root_idxs,
    }
}

fn forest_strategy() -> BoxedStrategy<Forest> {
    proptest::collection::vec(gen_node(MAX_LEVELS - 1), 1..=5)
        .prop_map(forest_from_roots)
        .boxed()
}

fn deep_forest_strategy() -> BoxedStrategy<Forest> {
    proptest::collection::vec(deep_node(MAX_LEVELS - 1), 1..=3)
        .prop_map(forest_from_roots)
        .boxed()
}

// ─── Property invariants ─────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The computed navigation edges exactly match the ground truth derived
    /// from the generated tree structure: same-parent sibling edges (direct
    /// even when descendants intervene), first_child and parent identity.
    #[test]
    fn navigation_edges_match_ordered_tree_oracle(forest in forest_strategy()) {
        let (sections, id_map) = forest.to_sections();
        prop_assert!(!sections.is_empty(), "forest must serialize to sections");
        let edges = compute_section_navigation_edges(&sections, &id_map);
        prop_assert_eq!(
            edge_set(&edges),
            edge_set(&forest.expected_navigation_edges()),
            "navigation edges must match the ordered-tree ground truth"
        );
    }

    /// Every node has at most one outgoing and one incoming next_sibling edge,
    /// and no self-loops.
    #[test]
    fn sibling_edges_have_unique_successor_and_predecessor(forest in forest_strategy()) {
        let (sections, id_map) = forest.to_sections();
        let edges = compute_section_navigation_edges(&sections, &id_map);
        let n = sections.len();
        let mut succ = vec![None; n];
        let mut pred = vec![None; n];
        for e in &edges {
            prop_assert!(e.from != e.to, "self-loop navigation edge");
            if e.relation == NavRelation::NextSibling {
                let (f, t) = (e.from as usize, e.to as usize);
                prop_assert!(succ[f].is_none(), "node {f} has more than one successor");
                succ[f] = Some(t);
                prop_assert!(pred[t].is_none(), "node {t} has more than one predecessor");
                pred[t] = Some(f);
            }
        }
    }

    /// next_sibling chains are acyclic and complete: following successors
    /// from any node terminates at exactly the oracle's last sibling of that
    /// node's sibling group.
    #[test]
    fn next_sibling_chains_are_acyclic_and_complete(forest in forest_strategy()) {
        let (sections, id_map) = forest.to_sections();
        let edges = compute_section_navigation_edges(&sections, &id_map);
        let n = sections.len();
        let mut succ = vec![None; n];
        for e in &edges {
            if e.relation == NavRelation::NextSibling {
                succ[e.from as usize] = Some(e.to as usize);
            }
        }
        let last_by_group = forest.last_sibling_by_group();
        for start in 0..n {
            let mut cur = start;
            let mut steps = 0usize;
            while let Some(nxt) = succ[cur] {
                cur = nxt;
                steps += 1;
                prop_assert!(steps <= n, "next_sibling cycle detected from node {start}");
            }
            let &last = last_by_group.get(&start).expect("every node belongs to a sibling group");
            if last == start {
                prop_assert!(
                    succ[start].is_none(),
                    "last sibling {start} must not have a successor"
                );
            } else {
                prop_assert_eq!(
                    cur,
                    last,
                    "chain from {} must end at the group's last sibling {}",
                    start,
                    last
                );
            }
        }
    }

    /// In deep trees (every non-leaf has children), each sibling pair is
    /// connected by a DIRECT next_sibling edge even though descendants sit
    /// between the siblings in document order.
    #[test]
    fn deep_tree_sibling_edges_are_direct_despite_descendants(forest in deep_forest_strategy()) {
        let (sections, id_map) = forest.to_sections();
        let edges = compute_section_navigation_edges(&sections, &id_map);
        let next: HashSet<(u64, u64)> = edges
            .iter()
            .filter(|e| e.relation == NavRelation::NextSibling)
            .map(|e| (e.from, e.to))
            .collect();
        for node in &forest.nodes {
            for pair in node.children.windows(2) {
                let (a, b) = (pair[0] as u64, pair[1] as u64);
                prop_assert!(
                    next.contains(&(a, b)),
                    "sibling {a} must point DIRECTLY to sibling {b} even when descendants intervene"
                );
            }
        }
        for pair in forest.roots.windows(2) {
            let (a, b) = (pair[0] as u64, pair[1] as u64);
            prop_assert!(
                next.contains(&(a, b)),
                "root {a} must point DIRECTLY to root {b}"
            );
        }
    }

    /// Parent identity always comes from the last '.' prefix of the full
    /// path — including GROBID restart suffixes (`~N`): a restart root has no
    /// parent_section edge, and a restart child is parented to the suffixed
    /// base path, never to a phantom dotted parent.
    #[test]
    fn parent_identity_uses_last_dot_prefix_including_restart_suffixes(forest in forest_strategy()) {
        let (sections, id_map) = forest.to_sections();
        let edges = compute_section_navigation_edges(&sections, &id_map);
        for (i, s) in sections.iter().enumerate() {
            let Some(path) = s.path_id.as_deref() else {
                continue;
            };
            let expected_parent: Vec<u64> = parent_of_path(path)
                .and_then(|p| id_map.get(&format!("sec-{p}")).copied())
                .into_iter()
                .collect();
            let parent_edges: Vec<u64> = edges
                .iter()
                .filter(|e| e.from == i as u64 && e.relation == NavRelation::ParentSection)
                .map(|e| e.to)
                .collect();
            prop_assert_eq!(
                parent_edges,
                expected_parent,
                "section {}: parent identity must be the last '.' prefix, never a dotted parent",
                path
            );
        }
    }
}
