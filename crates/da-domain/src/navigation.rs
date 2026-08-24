//! Navigation stage (M451 phase-2 port of scripts/m451_navigation_build.py).
//!
//! PageIndex semantics over the context projection: path_id grammar,
//! furniture roles (DATA detectors), prev/next walk, tree_cost search
//! model. Pure logic, no IO (da-domain); the CLI owns the bounded
//! filesystem write-back per the governor/application boundary.
//!
//! DATA contracts: furniture_detectors.yaml, navigation_quality_policy.yaml.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One node of context/navigation.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavNode {
    pub section_id: String,
    pub path_id: String,
    pub depth: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_section_id: Option<String>,
    pub children_section_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_section_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_section_id: Option<String>,
    pub title: String,
    pub char_count: u64,
    pub role: String,
    pub role_confidence: String,
}

/// Input projection node (subset of DocumentContextProjection sections).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectionSection {
    pub section_id: String,
    pub path_id: String,
    #[serde(default)]
    pub order: u64,
    #[serde(default)]
    pub level: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub char_count: u64,
}

/// One furniture detector from the DATA registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FurnitureDetector {
    pub id: String,
    pub role: String,
    pub confidence: String,
    pub title_pattern: String,
}

#[derive(Debug, thiserror::Error)]
pub enum NavigationError {
    #[error("furniture detectors: YAML parse failed: {0}")]
    DetectorsParse(String),
    #[error("furniture detectors: pattern {id:?} invalid: {err}")]
    BadPattern { id: String, err: String },
}

/// Parsed furniture detector registry (title regexes precompiled).
#[derive(Debug, Clone)]
pub struct FurnitureDetectors {
    pub compiled: Vec<(String, String, String, Regex)>, // id, role, confidence, regex
}

impl FurnitureDetectors {
    pub fn from_yaml_str(yaml: &str) -> Result<Self, NavigationError> {
        let v: serde_yaml::Value = serde_yaml::from_str(yaml)
            .map_err(|e| NavigationError::DetectorsParse(e.to_string()))?;
        let mut compiled = Vec::new();
        if let Some(list) = v.get("detectors").and_then(|d| d.as_sequence()) {
            for d in list {
                let id = d.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let role = d.get("role").and_then(|x| x.as_str()).unwrap_or("body").to_string();
                let conf = d
                    .get("confidence")
                    .and_then(|x| x.as_str())
                    .unwrap_or("heuristic")
                    .to_string();
                if let Some(pat) = d.get("title_pattern").and_then(|x| x.as_str()) {
                    let re = Regex::new(pat).map_err(|e| NavigationError::BadPattern {
                        id: id.clone(),
                        err: e.to_string(),
                    })?;
                    compiled.push((id, role, conf, re));
                }
            }
        }
        Ok(Self { compiled })
    }

    /// Classify a node title (first matching detector wins; DATA order).
    pub fn classify(&self, title: &str, path_id: &str) -> (String, String) {
        let t = title.trim();
        for (_, role, conf, re) in &self.compiled {
            if re.is_match(t) {
                return (role.clone(), conf.clone());
            }
        }
        if path_id.ends_with('.') {
            return ("header_metadata".into(), "deterministic".into());
        }
        ("body".into(), "heuristic".into())
    }
}

/// parent_path grammar: '1.2.3'->'1.2'; '1'->None; '1.'->None (header node).
pub fn parent_path(path_id: &str) -> Option<&str> {
    let p = path_id.trim();
    if p.ends_with('.') || !p.contains('.') {
        return None;
    }
    p.rsplit_once('.').map(|(parent, _)| parent)
}

/// Depth from the grammar: '1'->1, '1.2'->2; '1.'->0 (header metadata).
pub fn grammar_depth(path_id: &str) -> usize {
    let p = path_id.trim();
    if p.ends_with('.') {
        return 0;
    }
    if p.is_empty() {
        return 0;
    }
    p.split('.').count()
}

/// Build the navigation tree from projection sections (document order).
pub fn build_navigation(
    sections: &[ProjectionSection],
    detectors: &FurnitureDetectors,
) -> Vec<NavNode> {
    let mut sorted: Vec<&ProjectionSection> = sections.iter().collect();
    sorted.sort_by_key(|s| s.order);

    let by_path: BTreeMap<&str, &ProjectionSection> = sorted
        .iter()
        .map(|s| (s.path_id.as_str(), *s))
        .collect();

    let mut nodes: Vec<NavNode> = sorted
        .iter()
        .map(|s| {
            let (role, conf) = detectors.classify(&s.title, &s.path_id);
            let parent = parent_path(&s.path_id)
                .and_then(|pp| by_path.get(pp).copied());
            NavNode {
                section_id: s.section_id.clone(),
                path_id: s.path_id.clone(),
                depth: grammar_depth(&s.path_id),
                parent_section_id: parent.map(|p| p.section_id.clone()),
                children_section_ids: Vec::new(),
                prev_section_id: None,
                next_section_id: None,
                title: s.title.clone(),
                char_count: s.char_count,
                role,
                role_confidence: conf,
            }
        })
        .collect();

    // children in document order
    let index_by_id: BTreeMap<String, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.section_id.clone(), i))
        .collect();
    let parents: Vec<Option<String>> = nodes.iter().map(|n| n.parent_section_id.clone()).collect();
    for (i, p) in parents.iter().enumerate() {
        if let Some(pid) = p {
            let child_id = nodes[i].section_id.clone();
            if let Some(&pi) = index_by_id.get(pid) {
                nodes[pi].children_section_ids.push(child_id);
            }
        }
    }
    // prev/next in document order
    for i in 0..nodes.len().saturating_sub(1) {
        let (a, b) = {
            let (left, right) = nodes.split_at_mut(i + 1);
            (&mut left[i], &mut right[0])
        };
        a.next_section_id = Some(b.section_id.clone());
        b.prev_section_id = Some(a.section_id.clone());
    }
    nodes
}

/// tree_cost search model (Vectify adaptation): S=char_count, R=routing.
pub fn tree_cost(nodes: &[NavNode], routing_cost: u64) -> u64 {
    use std::collections::BTreeMap as Map;
    let by_id: Map<&str, &NavNode> = nodes.iter().map(|n| (n.section_id.as_str(), n)).collect();
    let mut children: Map<&str, Vec<&str>> = Map::new();
    for n in nodes {
        if let Some(p) = &n.parent_section_id {
            children.entry(p.as_str()).or_default().push(n.section_id.as_str());
        }
    }
    fn cost(id: &str, d: u64, r: u64, by: &Map<&str, &NavNode>, ch: &Map<&str, Vec<&str>>) -> u64 {
        let node = match by.get(id) {
            Some(n) => *n,
            None => return 0,
        };
        match ch.get(id) {
            None => d * r + node.char_count,
            Some(kids) if kids.is_empty() => d * r + node.char_count,
            Some(kids) => {
                let child_sum: u64 = kids
                    .iter()
                    .filter_map(|k| by.get(k).map(|n| n.char_count))
                    .sum();
                let residual = node.char_count.saturating_sub(child_sum);
                let mut branches: Vec<u64> =
                    kids.iter().map(|k| cost(k, d + 1, r, by, ch)).collect();
                if residual > 0 {
                    branches.push((d + 1) * r + residual);
                }
                branches.into_iter().max().unwrap_or(d * r + node.char_count)
            }
        }
    }
    nodes
        .iter()
        .filter(|n| n.parent_section_id.is_none())
        .map(|n| cost(&n.section_id, 1, routing_cost, &by_id, &children))
        .max()
        .unwrap_or(0)
}

/// Navigation quality policy thresholds (DATA).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NavigationQualityPolicy {
    #[serde(default = "default_routing")]
    pub routing_cost: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warn_above: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alarm_above: Option<u64>,
}
fn default_routing() -> u64 {
    250
}

/// Verdict for a "higher-is-bad" signal value.
pub fn verdict_above(value: u64, warn: Option<u64>, alarm: Option<u64>) -> &'static str {
    if alarm.map(|a| value > a).unwrap_or(false) {
        "alarm"
    } else if warn.map(|w| value > w).unwrap_or(false) {
        "warn"
    } else {
        "ok"
    }
}

/// Aggregated stats for the manifest artifact entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavigationStats {
    pub nodes: usize,
    pub roots: usize,
    pub max_depth: usize,
    pub path_dupes: usize,
    pub furniture: usize,
    pub furniture_share: f64,
    pub tree_cost: u64,
}

pub fn stats(nodes: &[NavNode], tree_cost: u64) -> NavigationStats {
    let furniture = nodes.iter().filter(|n| n.role == "furniture").count();
    NavigationStats {
        nodes: nodes.len(),
        roots: nodes.iter().filter(|n| n.parent_section_id.is_none()).count(),
        max_depth: nodes.iter().map(|n| n.depth).max().unwrap_or(0),
        path_dupes: nodes.len() - nodes.iter().map(|n| n.path_id.as_str()).collect::<std::collections::BTreeSet<_>>().len(),
        furniture,
        furniture_share: (furniture as f64) / nodes.len().max(1) as f64,
        tree_cost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DETECTORS: &str = "detectors:\n  - {id: under_review, role: furniture, confidence: deterministic, title_pattern: '^Under review\\.?$'}\n  - {id: fig, role: furniture, confidence: deterministic, title_pattern: '^(Fig(ure)?\\.?\\s*\\d+)'}\n";

    fn det() -> FurnitureDetectors {
        FurnitureDetectors::from_yaml_str(DETECTORS).unwrap()
    }

    fn sec(id: &str, path: &str, order: u64, title: &str, chars: u64) -> ProjectionSection {
        ProjectionSection {
            section_id: id.into(),
            path_id: path.into(),
            order,
            level: 1,
            title: title.into(),
            char_count: chars,
        }
    }

    #[test]
    fn parent_path_grammar() {
        assert_eq!(parent_path("1.2.3"), Some("1.2"));
        assert_eq!(parent_path("1.2"), Some("1"));
        assert_eq!(parent_path("1"), None);
        assert_eq!(parent_path("1."), None);
    }

    #[test]
    fn depth_grammar() {
        assert_eq!(grammar_depth("1"), 1);
        assert_eq!(grammar_depth("1.2"), 2);
        assert_eq!(grammar_depth("1."), 0);
    }

    #[test]
    fn furniture_classification() {
        let d = det();
        assert_eq!(d.classify("Under review.", "1"), ("furniture".into(), "deterministic".into()));
        assert_eq!(d.classify("Figure 3 .", "2"), ("furniture".into(), "deterministic".into()));
        assert_eq!(d.classify("Introduction", "1"), ("body".into(), "heuristic".into()));
        assert_eq!(d.classify("anything", "2."), ("header_metadata".into(), "deterministic".into()));
    }

    #[test]
    fn builds_tree_with_prev_next_symmetry() {
        let d = det();
        let secs = vec![
            sec("a", "1", 0, "Introduction", 100),
            sec("b", "1.1", 1, "Setup", 200),
            sec("c", "2", 2, "Results", 300),
        ];
        let nav = build_navigation(&secs, &d);
        assert_eq!(nav.len(), 3);
        let by: BTreeMap<&str, &NavNode> = nav.iter().map(|n| (n.section_id.as_str(), n)).collect();
        assert_eq!(by["b"].parent_section_id.as_deref(), Some("a"));
        assert_eq!(by["a"].children_section_ids, vec!["b".to_string()]);
        assert_eq!(by["a"].next_section_id.as_deref(), Some("b"));
        assert_eq!(by["b"].prev_section_id.as_deref(), Some("a"));
        assert_eq!(by["b"].next_section_id.as_deref(), Some("c"));
        assert!(by["c"].parent_section_id.is_none());
    }

    #[test]
    fn tree_cost_frontier_model() {
        let d = det();
        // root 1000 with two children 400+400: cost = max(1*R+200?, 2*R+400)
        let secs = vec![
            sec("r", "1", 0, "Root", 1000),
            sec("c1", "1.1", 1, "A", 400),
            sec("c2", "1.2", 2, "B", 400),
        ];
        let nav = build_navigation(&secs, &d);
        let c = tree_cost(&nav, 250);
        // branches: residual (1*250+200)=450 vs children (2*250+400)=900
        assert_eq!(c, 900);
    }

    #[test]
    fn stats_count_furniture_and_dupes() {
        let d = det();
        let secs = vec![
            sec("a", "1", 0, "Introduction", 100),
            sec("b", "1", 1, "Under review.", 10),
            sec("c", "2", 2, "Results", 300),
        ];
        let nav = build_navigation(&secs, &d);
        let st = stats(&nav, 500);
        assert_eq!(st.nodes, 3);
        assert_eq!(st.furniture, 1);
        assert!((st.furniture_share - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(st.path_dupes, 1);
    }
}
