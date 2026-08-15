//! Section navigation validator (M398 S01 T03).
//!
//! Deterministic validator over parsed [`da_ports::parser::Section`] trees and
//! the emitted navigation relations ([`crate::ingest::NavEdge`]) computed by
//! the ordered-tree model. It is pure (no graph IO) and reports **stable
//! diagnostics** with stable codes and section identity:
//!
//! | Code | Meaning |
//! |------|---------|
//! | `NAV-DUP-PATH` | two sections share the same structural `path_id` |
//! | `NAV-DUP-SECTION-ID` | two sections share the same `section_id` |
//! | `NAV-ORPHAN-PARENT` | parent reference (declared or dotted-derived) resolves to no section |
//! | `NAV-PARENT-ID-MISMATCH` | declared `parent_section_id` disagrees with the dotted parent path |
//! | `NAV-DEPTH-MISMATCH` | `level` disagrees with the dotted path depth |
//! | `NAV-BROKEN-PARENT-LINK` | missing/wrong/spurious `parent_section` or `first_child` edge |
//! | `NAV-MISSING-SIBLING` | a non-last sibling lacks its direct `next_sibling` edge |
//! | `NAV-CROSS-PARENT-SIBLING` | a `next_sibling` edge jumps across parent groups |
//! | `NAV-MULTI-SUCCESSOR` | a node has more than one outgoing `next_sibling` |
//! | `NAV-MULTI-FIRST-CHILD` | a node has more than one outgoing/incoming `first_child` |
//! | `NAV-MULTI-PARENT` | a node emits more than one outgoing `parent_section` (child → parent) |
//! | `NAV-UNKNOWN-NODE` | an edge endpoint (or a section) resolves to no graph node |
//! | `NAV-CYCLE` | a cycle in `next_sibling`, `parent_section`, or `first_child` edges |
//!
//! The validator lives in da-application next to Section consumption on
//! purpose (ADR-053 / M398 S01 T03): parser types stay in da-ports and are
//! not moved into domain or kg crates.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::ingest::{NavEdge, NavRelation};
use da_ports::parser::Section;

/// Stable diagnostic codes. The `as_str()` form (`NAV-*`) is the stable wire
/// contract consumed by tests and (T04) the Governor navigation report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NavDiagCode {
    DupPath,
    DupSectionId,
    OrphanParent,
    ParentIdMismatch,
    DepthMismatch,
    BrokenParentLink,
    MissingSibling,
    CrossParentSibling,
    MultiSuccessor,
    MultiFirstChild,
    MultiParent,
    UnknownNode,
    Cycle,
}

impl NavDiagCode {
    /// Stable machine-readable code (never renumbered; extend, don't rename).
    pub const fn as_str(self) -> &'static str {
        match self {
            NavDiagCode::DupPath => "NAV-DUP-PATH",
            NavDiagCode::DupSectionId => "NAV-DUP-SECTION-ID",
            NavDiagCode::OrphanParent => "NAV-ORPHAN-PARENT",
            NavDiagCode::ParentIdMismatch => "NAV-PARENT-ID-MISMATCH",
            NavDiagCode::DepthMismatch => "NAV-DEPTH-MISMATCH",
            NavDiagCode::BrokenParentLink => "NAV-BROKEN-PARENT-LINK",
            NavDiagCode::MissingSibling => "NAV-MISSING-SIBLING",
            NavDiagCode::CrossParentSibling => "NAV-CROSS-PARENT-SIBLING",
            NavDiagCode::MultiSuccessor => "NAV-MULTI-SUCCESSOR",
            NavDiagCode::MultiFirstChild => "NAV-MULTI-FIRST-CHILD",
            NavDiagCode::MultiParent => "NAV-MULTI-PARENT",
            NavDiagCode::UnknownNode => "NAV-UNKNOWN-NODE",
            NavDiagCode::Cycle => "NAV-CYCLE",
        }
    }
}

/// One deterministic validation finding, carrying stable code + section
/// identity (path_id / section_id / graph node id) + a stable human message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavDiagnostic {
    /// Stable code, e.g. `"NAV-MISSING-SIBLING"`.
    pub code: String,
    /// Structural path of the primary offending section, when applicable.
    pub path_id: Option<String>,
    /// `section_id` of the primary offending section, when applicable.
    pub section_id: Option<String>,
    /// Graph node id of the primary offending section, when applicable.
    pub node_id: Option<u64>,
    /// Deterministic human-readable detail (stable across runs).
    pub message: String,
}

impl NavDiagnostic {
    fn new(
        code: NavDiagCode,
        path_id: Option<String>,
        section_id: Option<String>,
        node_id: Option<u64>,
        message: impl Into<String>,
    ) -> Self {
        NavDiagnostic {
            code: code.as_str().to_string(),
            path_id,
            section_id,
            node_id,
            message: message.into(),
        }
    }
}

/// Aggregate, serializable validation result (the Governor-consumable
/// contract shape; T04 renders fixtures against it).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NavigationReport {
    /// `true` when the tree and its navigation edges are fully consistent.
    pub ok: bool,
    pub section_count: usize,
    pub edge_count: usize,
    pub diagnostic_count: usize,
    /// code → count, keyed by stable `NAV-*` codes (sorted).
    pub counts: BTreeMap<String, usize>,
    pub diagnostics: Vec<NavDiagnostic>,
}

/// Parent path of a dotted path_id (everything before the last '.').
/// Dot-free GROBID restart suffixes (`~N`) never introduce a level.
fn dotted_parent(path: &str) -> Option<&str> {
    path.rfind('.').map(|pos| &path[..pos])
}

/// Number of dotted path components ("1~2" → 1, "1~2.1" → 2, "3.1.2" → 3).
fn dotted_depth(path: &str) -> u32 {
    path.split('.').count() as u32
}

/// Whether `level` is valid for `path`:
///
/// - multi-component paths: `level` must equal the dotted depth (both parser
///   paths agree: `walk_divs` and flat `<head n="N.M">` set component count);
/// - single numeric components: flat legacy TEI keeps the old behavior where
///   `<head n="2">` → `level = 2` (not 1), so both `1` and `n` are accepted;
/// - everything else (incl. restart suffixes like `1~2`): must equal 1.
fn level_matches_dotted_depth(path: &str, level: u32) -> bool {
    let dd = dotted_depth(path);
    if dd == 1 && path.chars().all(|c| c.is_ascii_digit()) {
        let n = path.parse::<u32>().unwrap_or(1);
        level == 1 || level == n
    } else {
        level == dd
    }
}

/// Deterministic validator over parsed sections and emitted navigation edges.
///
/// `sections` are the parsed document sections (with `path_id`,
/// `parent_section_id`, `order_in_parent`, `section_id`), `section_id_map`
/// maps `section_id` → graph node id, and `edges` are the navigation edges
/// emitted for the tree (`next_sibling`, `first_child`, `parent_section`).
///
/// The expected structure is derived from the dotted path tree (the same
/// canonical parent identity the model uses), so a clean tree validates to an
/// empty diagnostic list. Diagnostics are sorted by (code, path, id) so the
/// output is byte-stable across runs.
pub fn validate_section_navigation(
    sections: &[Section],
    section_id_map: &HashMap<String, u64>,
    edges: &[NavEdge],
) -> Vec<NavDiagnostic> {
    let mut diags: Vec<NavDiagnostic> = Vec::new();

    // ── Index sections (document order preserved) ────────────────────────
    let mut by_path: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut by_sid: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, s) in sections.iter().enumerate() {
        if let Some(p) = s.path_id.as_deref() {
            by_path.entry(p).or_default().push(i);
        }
        if let Some(sid) = s.section_id.as_deref()
            && by_sid.insert(sid, i).is_some()
        {
            diags.push(NavDiagnostic::new(
                NavDiagCode::DupSectionId,
                s.path_id.clone(),
                Some(sid.to_string()),
                section_id_map.get(sid).copied(),
                format!(
                    "section_id '{sid}' is claimed by more than one section (duplicate stable id)"
                ),
            ));
        }
    }

    // node id → section index, section_id → node id (only for known sections).
    let mut node_to_section: HashMap<u64, usize> = HashMap::new();
    for (sid, &node) in section_id_map {
        if let Some(&i) = by_sid.get(sid.as_str()) {
            node_to_section.insert(node, i);
        }
    }

    // ── Pass 1: section-level structure checks ───────────────────────────
    for s in sections.iter() {
        let path = s.path_id.as_deref();
        let sid = s.section_id.as_deref();

        // Identity resolution: a section with a section_id must map to a node.
        if let Some(sid) = sid
            && !section_id_map.contains_key(sid)
        {
            diags.push(NavDiagnostic::new(
                NavDiagCode::UnknownNode,
                path.map(String::from),
                Some(sid.to_string()),
                None,
                format!(
                    "section '{sid}' has a section_id but no graph node in the id map (identity resolution failure)"
                ),
            ));
            continue;
        }
        let Some(path) = path else {
            // Legacy flat section without a path: nothing structural to check.
            continue;
        };
        let Some(sid) = sid else {
            continue;
        };
        let node = section_id_map[sid];

        // Duplicate structural paths.
        if by_path.get(path).is_some_and(|v| v.len() > 1) {
            let offenders: Vec<&str> = by_path[path]
                .iter()
                .filter_map(|&j| sections[j].section_id.as_deref())
                .collect();
            diags.push(NavDiagnostic::new(
                NavDiagCode::DupPath,
                Some(path.to_string()),
                Some(sid.to_string()),
                Some(node),
                format!(
                    "path '{path}' is shared by {} sections: {}",
                    offenders.len(),
                    offenders.join(", ")
                ),
            ));
        }

        // Dotted depth mismatch.
        if !level_matches_dotted_depth(path, s.level) {
            diags.push(NavDiagnostic::new(
                NavDiagCode::DepthMismatch,
                Some(path.to_string()),
                Some(sid.to_string()),
                Some(node),
                format!(
                    "level {} disagrees with dotted path depth {} for path '{path}'",
                    s.level,
                    dotted_depth(path)
                ),
            ));
        }

        // Parent consistency: the effective parent reference is the declared
        // `parent_section_id` when present, else the dotted-derived
        // `sec-<parent_path>`. An unresolvable reference is an orphan parent;
        // a declared reference that disagrees with the dotted parent is a
        // parent-id mismatch.
        let parent_path = dotted_parent(path);
        let derived_parent_sid = parent_path.map(|pp| format!("sec-{pp}"));
        let declared = s.parent_section_id.as_deref();
        let parent_ref: Option<String> = match declared {
            Some(d) => Some(d.to_string()),
            None => derived_parent_sid.clone(),
        };
        if let Some(pr) = &parent_ref
            && !by_sid.contains_key(pr.as_str())
        {
            diags.push(NavDiagnostic::new(
                NavDiagCode::OrphanParent,
                Some(path.to_string()),
                Some(sid.to_string()),
                Some(node),
                format!(
                    "parent reference '{pr}' of '{path}' has no section in the tree (orphan parent)"
                ),
            ));
        }
        match (declared, &derived_parent_sid) {
            (Some(d), Some(exp)) if d != exp => diags.push(NavDiagnostic::new(
                NavDiagCode::ParentIdMismatch,
                Some(path.to_string()),
                Some(sid.to_string()),
                Some(node),
                format!(
                    "declared parent_section_id '{d}' disagrees with dotted parent '{exp}' for '{path}'"
                ),
            )),
            (Some(d), None) => diags.push(NavDiagnostic::new(
                NavDiagCode::ParentIdMismatch,
                Some(path.to_string()),
                Some(sid.to_string()),
                Some(node),
                format!(
                    "root section '{path}' declares parent_section_id '{d}' (roots have no parent)"
                ),
            )),
            (None, Some(exp)) => diags.push(NavDiagnostic::new(
                NavDiagCode::ParentIdMismatch,
                Some(path.to_string()),
                Some(sid.to_string()),
                Some(node),
                format!(
                    "section '{path}' has dotted parent '{exp}' but declares no parent_section_id"
                ),
            )),
            _ => {}
        }
    }

    // ── Pass 2: edge-level sanity and illegal degrees ───────────────────
    let node_name = |n: u64| -> String {
        node_to_section
            .get(&n)
            .and_then(|&i| sections[i].path_id.as_deref())
            .map(|p| format!("'{p}' (node {n})"))
            .unwrap_or_else(|| format!("node {n}"))
    };

    let mut succ_out: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut first_out: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut parent_out: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut first_in: HashMap<u64, Vec<u64>> = HashMap::new();

    for e in edges {
        let from_known = node_to_section.contains_key(&e.from);
        let to_known = node_to_section.contains_key(&e.to);
        if !from_known || !to_known {
            diags.push(NavDiagnostic::new(
                NavDiagCode::UnknownNode,
                None,
                None,
                Some(e.from),
                format!(
                    "navigation edge {:?} references unknown node(s): {}",
                    e.relation,
                    match (from_known, to_known) {
                        (false, false) =>
                            format!("from {} and to {}", node_name(e.from), node_name(e.to)),
                        (false, true) => format!("from {}", node_name(e.from)),
                        (true, false) => format!("to {}", node_name(e.to)),
                        (true, true) => unreachable!(
                            "unknown-node branch requires at least one unknown endpoint"
                        ),
                    }
                ),
            ));
            continue;
        }
        match e.relation {
            NavRelation::NextSibling => succ_out.entry(e.from).or_default().push(e.to),
            NavRelation::FirstChild => {
                first_out.entry(e.from).or_default().push(e.to);
                first_in.entry(e.to).or_default().push(e.from);
            }
            NavRelation::ParentSection => {
                parent_out.entry(e.from).or_default().push(e.to);
            }
        }
    }

    // Illegal outgoing degrees.
    for (node, targets) in succ_out.iter() {
        if targets.len() > 1 {
            diags.push(NavDiagnostic::new(
                NavDiagCode::MultiSuccessor,
                path_of(node, &node_to_section, sections),
                sid_of(node, &node_to_section, sections),
                Some(*node),
                format!(
                    "{} has {} next_sibling successors: {} (more than one successor)",
                    node_name(*node),
                    targets.len(),
                    targets
                        .iter()
                        .map(|t| node_name(*t))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
    }
    for (node, targets) in first_out.iter() {
        if targets.len() > 1 {
            diags.push(NavDiagnostic::new(
                NavDiagCode::MultiFirstChild,
                path_of(node, &node_to_section, sections),
                sid_of(node, &node_to_section, sections),
                Some(*node),
                format!(
                    "{} has {} first_child targets: {} (more than one first child)",
                    node_name(*node),
                    targets.len(),
                    targets
                        .iter()
                        .map(|t| node_name(*t))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
    }
    for (node, parents) in parent_out.iter() {
        if parents.len() > 1 {
            diags.push(NavDiagnostic::new(
                NavDiagCode::MultiParent,
                path_of(node, &node_to_section, sections),
                sid_of(node, &node_to_section, sections),
                Some(*node),
                format!(
                    "{} has {} parent_section targets: {} (more than one parent)",
                    node_name(*node),
                    parents.len(),
                    parents
                        .iter()
                        .map(|p| node_name(*p))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
    }
    for (node, parents) in first_in.iter() {
        if parents.len() > 1 {
            diags.push(NavDiagnostic::new(
                NavDiagCode::MultiFirstChild,
                path_of(node, &node_to_section, sections),
                sid_of(node, &node_to_section, sections),
                Some(*node),
                format!(
                    "{} is first_child of {} parents: {} (more than one first-child claim)",
                    node_name(*node),
                    parents.len(),
                    parents
                        .iter()
                        .map(|p| node_name(*p))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
    }

    // ── Pass 3: expected structure vs emitted edges ──────────────────────
    // Sibling groups: sections grouped by dotted parent (None = roots), in
    // document order — exactly how the model resolves siblings.
    let mut groups: BTreeMap<Option<String>, Vec<usize>> = BTreeMap::new();
    for (i, s) in sections.iter().enumerate() {
        let Some(path) = s.path_id.as_deref() else {
            continue;
        };
        let Some(sid) = s.section_id.as_deref() else {
            continue;
        };
        if !section_id_map.contains_key(sid) {
            continue; // cannot participate in edges (already reported)
        }
        groups
            .entry(dotted_parent(path).map(String::from))
            .or_default()
            .push(i);
    }

    // Expected sibling links: every non-last member of a group must have a
    // direct next_sibling edge to the next member (document order).
    for members in groups.values() {
        for pair in members.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let a_node = section_id_map[sections[a].section_id.as_deref().unwrap()];
            let b_node = section_id_map[sections[b].section_id.as_deref().unwrap()];
            let actual: Vec<u64> = succ_out.get(&a_node).cloned().unwrap_or_default();
            if actual.is_empty() {
                diags.push(NavDiagnostic::new(
                    NavDiagCode::MissingSibling,
                    sections[a].path_id.clone(),
                    sections[a].section_id.clone(),
                    Some(a_node),
                    format!(
                        "{} is a non-last sibling but has no next_sibling edge to {}",
                        node_name(a_node),
                        node_name(b_node)
                    ),
                ));
            } else if actual != vec![b_node] {
                diags.push(NavDiagnostic::new(
                    NavDiagCode::MissingSibling,
                    sections[a].path_id.clone(),
                    sections[a].section_id.clone(),
                    Some(a_node),
                    format!(
                        "{} next_sibling target(s) {:?} do not match the expected direct successor {}",
                        node_name(a_node),
                        actual.iter().map(|t| node_name(*t)).collect::<Vec<_>>(),
                        node_name(b_node)
                    ),
                ));
            }
        }
    }

    // Expected parent_section and first_child links.
    let mut expected_children: HashMap<u64, Vec<u64>> = HashMap::new(); // parent node → child nodes (doc order)
    for s in sections.iter() {
        let Some(path) = s.path_id.as_deref() else {
            continue;
        };
        let Some(pp) = dotted_parent(path) else {
            continue;
        };
        let Some(&parent_idx) = by_path.get(pp).and_then(|v| v.first()) else {
            continue; // orphan parent already reported in pass 1
        };
        let parent_section = &sections[parent_idx];
        let Some(parent_sid) = parent_section.section_id.as_deref() else {
            continue;
        };
        let Some(&parent_node) = section_id_map.get(parent_sid) else {
            continue;
        };
        let Some(child_sid) = s.section_id.as_deref() else {
            continue;
        };
        let Some(&child_node) = section_id_map.get(child_sid) else {
            continue;
        };

        // parent_section edge: child → parent.
        let out_parents: Vec<u64> = parent_out.get(&child_node).cloned().unwrap_or_default();
        if out_parents.is_empty() {
            diags.push(NavDiagnostic::new(
                NavDiagCode::BrokenParentLink,
                s.path_id.clone(),
                s.section_id.clone(),
                Some(child_node),
                format!(
                    "{} has parent {} but no parent_section edge to it",
                    node_name(child_node),
                    node_name(parent_node)
                ),
            ));
        } else if out_parents != vec![parent_node] {
            diags.push(NavDiagnostic::new(
                NavDiagCode::BrokenParentLink,
                s.path_id.clone(),
                s.section_id.clone(),
                Some(child_node),
                format!(
                    "{} parent_section target(s) {:?} do not match the expected parent {}",
                    node_name(child_node),
                    out_parents
                        .iter()
                        .map(|t| node_name(*t))
                        .collect::<Vec<_>>(),
                    node_name(parent_node)
                ),
            ));
        }

        // Record child for the parent's first_child expectation (doc order).
        expected_children
            .entry(parent_node)
            .or_default()
            .push(child_node);
    }

    // first_child: every parent with children must point to its first child.
    for (parent_node, children) in &expected_children {
        let first = children[0];
        let actual: Vec<u64> = first_out.get(parent_node).cloned().unwrap_or_default();
        if actual.is_empty() {
            diags.push(NavDiagnostic::new(
                NavDiagCode::BrokenParentLink,
                path_of(parent_node, &node_to_section, sections),
                sid_of(parent_node, &node_to_section, sections),
                Some(*parent_node),
                format!(
                    "{} has {} child/children but no first_child edge to {}",
                    node_name(*parent_node),
                    children.len(),
                    node_name(first)
                ),
            ));
        } else if actual != vec![first] {
            diags.push(NavDiagnostic::new(
                NavDiagCode::BrokenParentLink,
                path_of(parent_node, &node_to_section, sections),
                sid_of(parent_node, &node_to_section, sections),
                Some(*parent_node),
                format!(
                    "{} first_child target(s) {:?} do not match the expected first child {}",
                    node_name(*parent_node),
                    actual.iter().map(|t| node_name(*t)).collect::<Vec<_>>(),
                    node_name(first)
                ),
            ));
        }
    }

    // Spurious edges: a node with NO children must not emit first_child; a
    // root (dotted parent None) must not emit parent_section.
    let parents_with_children: HashSet<u64> = expected_children.keys().copied().collect();
    for (node, targets) in first_out.iter() {
        if !parents_with_children.contains(node) {
            diags.push(NavDiagnostic::new(
                NavDiagCode::BrokenParentLink,
                path_of(node, &node_to_section, sections),
                sid_of(node, &node_to_section, sections),
                Some(*node),
                format!(
                    "{} emits first_child to {} but has no children in the tree (spurious first_child)",
                    node_name(*node),
                    targets.iter().map(|t| node_name(*t)).collect::<Vec<_>>().join(", ")
                ),
            ));
        }
    }
    let root_nodes: HashSet<u64> = sections
        .iter()
        .filter_map(|s| {
            let path = s.path_id.as_deref()?;
            if dotted_parent(path).is_some() {
                return None;
            }
            s.section_id
                .as_deref()
                .and_then(|sid| section_id_map.get(sid).copied())
        })
        .collect();
    for (node, targets) in parent_out.iter() {
        if root_nodes.contains(node) {
            diags.push(NavDiagnostic::new(
                NavDiagCode::BrokenParentLink,
                path_of(node, &node_to_section, sections),
                sid_of(node, &node_to_section, sections),
                Some(*node),
                format!(
                    "root {} emits parent_section to {} (roots have no parent)",
                    node_name(*node),
                    targets
                        .iter()
                        .map(|t| node_name(*t))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
    }

    // ── Pass 4: cross-parent siblings ────────────────────────────────────
    // Parent key of a node = dotted parent path of its section (None = root).
    // Any next_sibling edge whose endpoints belong to different parent groups
    // (including root vs child) is cross-parent.
    let parent_key_of_node = |n: u64| -> Option<String> {
        node_to_section
            .get(&n)
            .and_then(|&i| sections[i].path_id.as_deref())
            .and_then(dotted_parent)
            .map(String::from)
    };
    for e in edges {
        if e.relation != NavRelation::NextSibling {
            continue;
        }
        let fk = parent_key_of_node(e.from);
        let tk = parent_key_of_node(e.to);
        if fk != tk {
            diags.push(NavDiagnostic::new(
                NavDiagCode::CrossParentSibling,
                path_of(&e.from, &node_to_section, sections),
                sid_of(&e.from, &node_to_section, sections),
                Some(e.from),
                format!(
                    "next_sibling edge {} → {} crosses parent groups ('{:?}' vs '{:?}')",
                    node_name(e.from),
                    node_name(e.to),
                    fk,
                    tk
                ),
            ));
        }
    }

    // ── Pass 5: cycles per relation ──────────────────────────────────────
    for (relation, adj) in [
        (NavRelation::NextSibling, &succ_out),
        (NavRelation::ParentSection, &parent_out),
        (NavRelation::FirstChild, &first_out),
    ] {
        for cycle in find_cycles(adj) {
            let rendered: Vec<String> = cycle.iter().map(|n| node_name(*n)).collect();
            diags.push(NavDiagnostic::new(
                NavDiagCode::Cycle,
                cycle
                    .first()
                    .and_then(|n| node_to_section.get(n))
                    .and_then(|&i| sections[i].path_id.clone()),
                cycle
                    .first()
                    .and_then(|n| node_to_section.get(n))
                    .and_then(|&i| sections[i].section_id.clone()),
                cycle.first().copied(),
                format!("{:?} cycle detected: {}", relation, rendered.join(" → ")),
            ));
        }
    }

    // ── Deterministic ordering ───────────────────────────────────────────
    diags.sort_by(|a, b| {
        (&a.code, &a.path_id, &a.section_id, &a.node_id, &a.message).cmp(&(
            &b.code,
            &b.path_id,
            &b.section_id,
            &b.node_id,
            &b.message,
        ))
    });
    diags
}

/// Aggregate a validation result into the serializable report contract.
pub fn render_navigation_report(
    sections: &[Section],
    section_id_map: &HashMap<String, u64>,
    edges: &[NavEdge],
) -> NavigationReport {
    let diagnostics = validate_section_navigation(sections, section_id_map, edges);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for d in &diagnostics {
        *counts.entry(d.code.clone()).or_insert(0) += 1;
    }
    NavigationReport {
        ok: diagnostics.is_empty(),
        section_count: sections.len(),
        edge_count: edges.len(),
        diagnostic_count: diagnostics.len(),
        counts,
        diagnostics,
    }
}

// ─── Internal helpers ─────────────────────────────────────────────────

fn path_of(
    node: &u64,
    node_to_section: &HashMap<u64, usize>,
    sections: &[Section],
) -> Option<String> {
    node_to_section
        .get(node)
        .and_then(|i| sections[*i].path_id.clone())
}

fn sid_of(
    node: &u64,
    node_to_section: &HashMap<u64, usize>,
    sections: &[Section],
) -> Option<String> {
    node_to_section
        .get(node)
        .and_then(|i| sections[*i].section_id.clone())
}

/// Deterministic cycle detection on a directed graph (iterative DFS).
/// Returns distinct cycles (deduped by sorted node set), each as the node
/// sequence in traversal order. Node iteration and neighbor lists are sorted,
/// so the result is stable across runs.
fn find_cycles(adj: &HashMap<u64, Vec<u64>>) -> Vec<Vec<u64>> {
    // 0 = white (unseen), 1 = gray (on DFS path), 2 = black (done).
    let mut color: HashMap<u64, u8> = HashMap::new();
    let mut cycles: Vec<Vec<u64>> = Vec::new();
    let mut seen: HashSet<Vec<u64>> = HashSet::new();

    let mut nodes: Vec<u64> = adj.keys().copied().collect();
    nodes.sort_unstable();

    for &start in &nodes {
        if color.get(&start) == Some(&2) {
            continue;
        }
        let mut stack: Vec<(u64, usize)> = vec![(start, 0)];
        let mut path: Vec<u64> = vec![start];
        color.insert(start, 1);
        while let Some(&(node, idx)) = stack.last() {
            let neighbors = match adj.get(&node) {
                Some(n) => n,
                None => {
                    stack.pop();
                    path.pop();
                    color.insert(node, 2);
                    continue;
                }
            };
            if idx >= neighbors.len() {
                stack.pop();
                path.pop();
                color.insert(node, 2);
                continue;
            }
            let next = neighbors[idx];
            stack.last_mut().expect("stack non-empty").1 += 1;
            match color.get(&next) {
                Some(1) => {
                    // Back edge → cycle from `next` to the current path end.
                    if let Some(pos) = path.iter().position(|&x| x == next) {
                        let mut cyc: Vec<u64> = path[pos..].to_vec();
                        cyc.push(next);
                        let mut key = cyc.clone();
                        key.sort_unstable();
                        if seen.insert(key) {
                            cycles.push(cyc);
                        }
                    }
                }
                Some(2) => {}
                _ => {
                    color.insert(next, 1);
                    path.push(next);
                    stack.push((next, 0));
                }
            }
        }
    }
    cycles
}
