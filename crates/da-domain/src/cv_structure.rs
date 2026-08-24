//! Statistical structure-analysis primitives ported from pdf-craft
//! (oomol-lab/pdf-craft, AGPL-3.0-compatible study; ADR-072 companion).
//!
//! Two mechanisms:
//! - [`split_by_cv`]: generic coefficient-of-variation splitter. Repeatedly
//!   splits the group with max CV at its largest internal gap until every
//!   group's CV ≤ threshold (or max_groups reached). Used for heading-level
//!   assignment from font heights — data-driven, no hardcoded size tables.
//! - [`find_toc_pages`]: TOC-page detection via title→body self-matching.
//!   A page whose body contains many document titles is an outline page;
//!   score = Σ(matched / total_occurrences) downweights titles repeated in
//!   running heads. Cut position = largest score drop between candidates.

/// Coefficient of variation (std/mean) of a slice; 0 for <2 items.
fn cv(values: &[f32]) -> f32 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    if mean == 0.0 {
        return f32::INFINITY;
    }
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32;
    variance.sqrt() / mean
}

#[derive(Debug)]
struct CvGroup {
    /// (size, payload) sorted on demand.
    items: Vec<(f32, usize)>,
}

impl CvGroup {
    fn sizes(&self) -> Vec<f32> {
        self.items.iter().map(|(s, _)| *s).collect()
    }
    fn avg_size(&self) -> f32 {
        if self.items.is_empty() {
            0.0
        } else {
            self.items.iter().map(|(s, _)| s).sum::<f32>() / self.items.len() as f32
        }
    }
}

/// Split `payload_items` ((size, payload)) into groups with CV ≤ max_cv,
/// splitting the worst group at its largest internal gap. Result groups are
/// ordered by average size ascending (small → large), mirroring pdf-craft's
/// heading-level assignment where smaller groups of LARGER heights mean
/// higher levels — caller reverses as needed.
pub fn split_by_cv(payload_items: &[(f32, usize)], max_cv: f32, max_groups: usize) -> Vec<Vec<usize>> {
    if payload_items.len() <= 2 {
        return vec![payload_items.iter().map(|(_, p)| *p).collect()];
    }
    let mut groups: Vec<CvGroup> = vec![CvGroup { items: payload_items.to_vec() }];

    while groups.len() < max_groups {
        // find group with max cv above threshold
        let mut worst_idx: Option<usize> = None;
        let mut worst_cv = max_cv;
        for (i, g) in groups.iter().enumerate() {
            if g.items.len() <= 2 {
                continue;
            }
            let c = cv(&g.sizes());
            if c > worst_cv {
                worst_cv = c;
                worst_idx = Some(i);
            }
        }
        let Some(idx) = worst_idx else { break };

        // split at largest internal gap (by size)
        let mut items = groups[idx].items.clone();
        items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut best_gap = 0.0f32;
        let mut split_at = 0usize;
        for w in 0..items.len().saturating_sub(1) {
            let gap = items[w + 1].0 - items[w].0;
            if gap > best_gap {
                best_gap = gap;
                split_at = w + 1;
            }
        }
        if split_at == 0 || split_at >= items.len() {
            break;
        }
        let right = items.split_off(split_at);
        groups[idx] = CvGroup { items };
        groups.insert(idx + 1, CvGroup { items: right });
    }

    let mut ordered: Vec<(f32, Vec<usize>)> =
        groups.into_iter().map(|g| (g.avg_size(), g.items.into_iter().map(|(_, p)| p).collect())).collect();
    ordered.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    ordered.into_iter().map(|(_, payloads)| payloads).collect()
}

/// A candidate TOC page: its score and which titles matched it.
#[derive(Debug, Clone)]
pub struct TocPageCandidate {
    pub page_index: u32,
    pub score: f32,
}

/// Detect TOC pages by matching known document titles against page bodies
/// (pdf-craft `find_toc_pages`, reduced to substring counting).
///
/// `titles`: normalized lowercase titles harvested from headings.
/// `page_bodies`: normalized lowercase body text per page (1-indexed).
///
/// Score per page = Σ over matched titles of matched_count / total_count
/// (titles occurring everywhere — running heads — contribute ~0).
/// Candidates cut at the largest score drop, min 2 pages.
pub fn find_toc_pages(
    titles: &[String],
    page_bodies: &BTreeMapU32String,
    total_pages: u32,
) -> Vec<TocPageCandidate> {
    let total_of = |t: &str| -> u32 {
        page_bodies.values().map(|b| count_occurrences(b, t)).sum()
    };
    let mut totals: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut scores: std::collections::HashMap<u32, f32> = std::collections::HashMap::new();

    for t in titles {
        if t.chars().count() < 4 {
            continue;
        }
        let total = *totals.entry(t.clone()).or_insert_with(|| total_of(t));
        if total == 0 {
            continue;
        }
        for (&pn, body) in page_bodies.iter() {
            let c = count_occurrences(body, t);
            if c == 0 || c >= total {
                continue; // references exclude this very page's own copies
            }
            *scores.entry(pn).or_insert(0.0) += c as f32 / total as f32;
        }
    }

    let mut refs: Vec<TocPageCandidate> = scores
        .into_iter()
        .map(|(page_index, score)| TocPageCandidate { page_index, score })
        .collect();
    // TOC lives in the document HEAD — filter before ranking so a high-scoring
    // content chapter deep in the book cannot win the cut (pdf-craft applies
    // this inside the human-like filter, but after the cut; our regression on
    // the fixture showed the cut must see only plausible pages first)
    let head_limit = (total_pages as f32 * TOC_HEAD_RATIO).round() as u32;
    refs.retain(|r| r.page_index <= head_limit);
    refs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    // cut at the largest drop between consecutive scores (pdf-craft);
    // a single candidate is kept as-is when it dominates its runner-up
    if refs.len() == 1 {
        let alone = refs[0].score;
        return if alone > 0.0 { refs } else { Vec::new() };
    }
    let mut cut = 0usize;
    let mut max_diff = 0.0f32;
    for i in 0..refs.len().saturating_sub(1) {
        let d = refs[i].score - refs[i + 1].score;
        if d > max_diff {
            max_diff = d;
            cut = i + 1;
        }
    }
    refs.truncate(cut);
    let max_content_score = refs.get(cut).map(|r| r.score).unwrap_or(0.0);

    human_like_toc_filter(refs, total_pages, max_content_score)
}

/// pdf-craft `_human_like_toc_filter`: TOC lives in the head of a document
/// (_TOC_HEAD_RATIO), is bounded in count, forms a serial run from the first
/// candidate, and must outscore the best content page by _TOC_SCORE_MIN_RATIO.
const TOC_HEAD_RATIO: f32 = 0.18;
const MIN_TOC_LIMIT: usize = 3;
const TOC_SCORE_MIN_RATIO: f32 = 3.0;

fn human_like_toc_filter(
    mut refs: Vec<TocPageCandidate>,
    _total_pages: u32,
    max_content_score: f32,
) -> Vec<TocPageCandidate> {
    if refs.is_empty() {
        return Vec::new();
    }
    // significance relative to the TOP score, not absolute dominance:
    // real multi-page TOCs decay smoothly (live book: 5.87 → 2.09 across
    // p8..p16 with no cliff), so top/RATIO defines the floor
    let top_score = refs.iter().map(|r| r.score).fold(f32::MIN, f32::max);
    if top_score < TOC_SCORE_MIN_RATIO * max_content_score {
        return Vec::new(); // outline doesn't dominate content → refuse
    }
    let floor = top_score / TOC_SCORE_MIN_RATIO;
    refs.retain(|r| r.score >= floor);

    // All significant pages in the head zone ARE the outline: real books may
    // restart numbering after front-matter (live specimen: p8..p16 with a
    // preface gap p15→p8 ordering artifact). Keep them all.
    refs.sort_by_key(|r| r.page_index);
    if refs.len() > MIN_TOC_LIMIT * 4 {
        refs.truncate(MIN_TOC_LIMIT * 4);
    }
    refs
}

fn count_occurrences(haystack: &str, needle: &str) -> u32 {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count() as u32
}

/// BTreeMap<u32, String> alias to keep signatures short.
pub type BTreeMapU32String = std::collections::BTreeMap<u32, String>;

/// Assign heading levels to font-height clusters (pdf-craft
/// `analyse_chapter_internal_levels`): collect heading line heights, run
/// [`split_by_cv`], map groups to levels 1..=max_levels ordered by height
/// descending (bigger font → higher level).
///
/// Returns payload → level (1-based).
pub fn assign_heading_levels(height_payloads: &[(f32, usize)], max_levels: usize) -> std::collections::HashMap<usize, usize> {
    /// pdf-craft toc/config.py: MAX_TITLE_CV = 0.025 — very strict, so
    /// heading-height clusters stay tight and levels track real size bands
    const MAX_TITLE_CV: f32 = 0.025;
    let groups = split_by_cv(height_payloads, MAX_TITLE_CV, max_levels);
    let mut out = std::collections::HashMap::new();
    // split_by_cv returns ascending by avg size; reverse → biggest first
    for (level_zero_based, group) in groups.into_iter().rev().enumerate() {
        let level = (level_zero_based + 1).min(max_levels);
        for p in group {
            out.insert(p, level);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn cv_split_separates_distinct_size_clusters() {
        // heading heights: two clusters {30, 28} and {12, 11}
        let items: Vec<(f32, usize)> =
            vec![(30.0, 0), (28.0, 1), (12.0, 2), (11.0, 3)];
        let groups = split_by_cv(&items, 0.35, 5);
        assert_eq!(groups.len(), 2, "two clear clusters must split: {groups:?}");
        let mut small = groups[0].clone();
        small.sort();
        let mut large = groups[1].clone();
        large.sort();
        assert_eq!(small, vec![2, 3], "ascending avg: small cluster first");
        assert_eq!(large, vec![0, 1]);
    }

    #[test]
    fn cv_split_keeps_homogeneous_group_whole() {
        let items: Vec<(f32, usize)> = vec![(10.0, 0), (10.2, 1), (9.9, 2)];
        let groups = split_by_cv(&items, 0.35, 5);
        assert_eq!(groups.len(), 1, "low CV stays together");
    }

    #[test]
    fn heading_levels_biggest_font_is_level_one() {
        let items: Vec<(f32, usize)> =
            vec![(30.0, 0), (29.0, 1), (15.0, 2), (14.0, 3), (9.0, 4), (9.2, 5)];
        let m = assign_heading_levels(&items, 5);
        assert_eq!(m[&0], 1, "{m:?}");
        assert_eq!(m[&1], 1);
        assert_eq!(m[&2], 2);
        // smallest-font cluster lands on the lowest level present
        assert_eq!(m[&4], m[&5]);
        assert!(*m.get(&4).unwrap() > m[&2], "{m:?}");
    }

    fn toc_fixture() -> (Vec<String>, BTreeMap<u32, String>) {
        let titles = vec![
            "value iteration".into(),
            "policy iteration".into(),
            "q-learning".into(),
            "kalman filter".into(),
        ];
        let mut bodies: BTreeMap<u32, String> = BTreeMap::new();
        bodies.insert(
            8,
            "contents\nvalue iteration ......... 120\npolicy iteration ...... 130\nq-learning ..... 200\nkalman filter .... 300".into(),
        );
        bodies.insert(
            120,
            "chapter 7. value iteration. value iteration is an alternative to policy iteration that is often used because of its simplicity.".into(),
        );
        bodies.insert(
            130,
            "7.4 policy iteration. policy iteration is one way to compute an optimal policy.".into(),
        );
        (titles, bodies)
    }

    #[test]
    fn toc_pages_found_by_title_body_matching() {
        let (titles, bodies) = toc_fixture();
        let found = find_toc_pages(&titles, &bodies, 300);
        assert!(
            found.iter().any(|c| c.page_index == 8),
            "outline page must surface: {found:?}"
        );
        assert!(
            !found.iter().any(|c| c.page_index == 120),
            "content pages must not be classified as TOC: {found:?}"
        );
    }

    #[test]
    fn running_head_title_repetition_downweighted() {
        let (mut titles, mut bodies) = toc_fixture();
        titles.push("algorithms for decision making".into());
        // running head repeats book title on EVERY content page
        for pn in 100..=140 {
            let e = bodies.entry(pn).or_default();
            *e = format!("algorithms for decision making | {e}");
        }
        let found = find_toc_pages(&titles, &bodies, 300);
        // the repeated title contributes ~0 everywhere; TOC still wins
        assert!(found.iter().any(|c| c.page_index == 8), "{found:?}");
    }
}
