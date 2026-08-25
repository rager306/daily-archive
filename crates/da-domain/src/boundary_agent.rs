//! Section-boundary FSM agent helper (ADR-073 wave, dogfood-driven).
//!
//! Deterministic FSM over ambiguous section-heading anchors. No ML: the
//! agent CLASSIFIES anchor candidates with evidence-based rules, records
//! every decision as a low-confidence pair for the critic (data/agents/
//! critic contract), and escalates only genuine ties.
//!
//! States: Scan → CandidateFound → Classify{prose_after | table_cell |
//! code_line} → Decide{accept | reject_to_prev} → (tie) Escalate.
//!
//! Training-data path (SONA W1 record-only): every decision lands in
//! artifacts/pipeline_cache/boundary_decisions.jsonl {features, decision,
//! audit_verdict} — when a labelled corpus exists, a learned classifier can
//! replace the rule vote WITHOUT changing the FSM shape.

use serde::Serialize;

/// One candidate anchor position on a shared page.
#[derive(Debug, Clone, PartialEq)]
pub struct AnchorCandidate {
    /// byte offset of the candidate heading in page text
    pub offset: usize,
    /// normalized title text at that offset
    pub matched_text: String,
    /// text immediately after the candidate heading (same line window)
    pub following_text: String,
    /// previous non-empty line before the candidate
    pub preceding_text: String,
}

/// Evidence features extracted from page context (the "observation" vector).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BoundaryFeatures {
    /// chars of prose right after the heading (>60 ⇒ real section body)
    pub following_prose_chars: usize,
    /// heading is preceded by a pure section-number line ("7.4", "F.10")
    pub number_line_precedes: bool,
    /// candidate sits inside a table band (surrounded by short numeric/cell lines)
    pub table_band_around: bool,
    /// candidate is inside a 7–8.2pt code-font zone proxy (short lines,
    /// no sentence punctuation, high symbol density)
    pub code_zone_around: bool,
    /// TOC bookmark page matches this page exactly
    pub toc_page_exact: bool,
}

/// Decision of the deterministic classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Decision {
    /// real section heading — split here
    Accept,
    /// table cell / code token / running text mention — keep in prev section
    RejectToPrev,
    /// genuinely ambiguous → critic/human escalation queue
    Escalate,
}

/// A recorded decision (training-row shape; SONA W1 record-only).
#[derive(Debug, Clone, Serialize)]
pub struct BoundaryDecision {
    pub paper_id: String,
    pub page: u32,
    pub title: String,
    pub features: BoundaryFeatures,
    pub decision: Decision,
}

/// Classify one candidate from its features. Rules are ordered by evidence
/// strength; ties escalate instead of guessing.
pub fn classify(f: &BoundaryFeatures) -> Decision {
    // Strongest positive: numbered section line above + any body follows.
    // Books number real headings ("17.3"); numbered cells inside tables do
    // not occur in our corpus.
    if f.number_line_precedes && f.following_prose_chars >= 8 {
        return Decision::Accept;
    }
    // Table band: cells around, short continuation — reject.
    if f.table_band_around && f.following_prose_chars < 40 {
        return Decision::RejectToPrev;
    }
    // Code zone: symbol-dense short lines — reject.
    if f.code_zone_around {
        return Decision::RejectToPrev;
    }
    // Prose follows but no number line — still likely a heading in books
    // without numbering discipline.
    if f.following_prose_chars >= 60 && !f.code_zone_around {
        return Decision::Accept;
    }
    Decision::Escalate
}

/// Extract features for a candidate found by `find_heading_anchor`.
pub fn extract_features(
    page_lines: &[&str],
    start_line: usize,
    window: usize,
    toc_page: u32,
    page_no: u32,
) -> BoundaryFeatures {
    let following: String = page_lines
        .get(start_line + window..)
        .map(|ls| ls.join(" "))
        .unwrap_or_default();
    let preceding: String = page_lines
        .get(start_line.saturating_sub(2)..start_line)
        .map(|ls| ls.join(" "))
        .unwrap_or_default();
    let number_precedes = preceding.trim().lines().next_back().is_some_and(|l| {
        let t = l.trim();
        !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ' ')
    });

    // table-band heuristic: ≥3 of 6 surrounding lines are short numeric/cell-ish
    let around: Vec<&str> = page_lines
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let d = (*i as i64) - (start_line as i64);
            (-3..=3).contains(&d)
        })
        .map(|(_, l)| l.trim())
        .collect();
    let cellish = around
        .iter()
        .filter(|l| {
            // em-dash ячейка таблицы выплат — тоже признак таблицы
            let bare: String =
                l.replace(['\u{2014}', '-', '|'], "").trim().to_string();
            if bare.is_empty() {
                return !l.trim().is_empty();
            }
            (!bare.is_empty() && bare.len() <= 12 && !bare.ends_with('.'))
                || bare.parse::<f64>().is_ok()
        })
        .count();

    // code-zone proxy: many lines end with bare identifiers / no periods
    let codeish = around
        .iter()
        .filter(|l| {
            !l.contains('.') && !l.ends_with(',') && (l.ends_with(')') || l.ends_with("end"))
        })
        .count();

    BoundaryFeatures {
        following_prose_chars: following.chars().take(200).count(),
        number_line_precedes: number_precedes,
        table_band_around: cellish >= 3,
        code_zone_around: codeish >= 3,
        toc_page_exact: toc_page == page_no,
    }
}

/// In-memory decision log (persisted by CLI into jsonl).
#[derive(Debug, Default)]
pub struct BoundaryDecisionLog {
    pub decisions: Vec<BoundaryDecision>,
}

impl BoundaryDecisionLog {
    pub fn push(&mut self, d: BoundaryDecision) {
        self.decisions.push(d);
    }

    pub fn to_jsonl(&self) -> String {
        self.decisions
            .iter()
            .map(|d| serde_json::to_string(d).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feats(prose: usize, num: bool, table: bool, code: bool, exact: bool) -> BoundaryFeatures {
        BoundaryFeatures {
            following_prose_chars: prose,
            number_line_precedes: num,
            table_band_around: table,
            code_zone_around: code,
            toc_page_exact: exact,
        }
    }

    #[test]
    fn real_heading_accepted() {
        assert_eq!(
            classify(&feats(120, true, false, false, true)),
            Decision::Accept
        );
    }

    #[test]
    fn table_cell_rejected() {
        // specimen: 'Hex world' inside Table F.1 — cells around, no prose
        assert_eq!(classify(&feats(5, false, true, false, true)), Decision::RejectToPrev);
    }

    #[test]
    fn code_token_rejected() {
        // specimen: 'label'/'executes' inside Julia blocks
        assert_eq!(classify(&feats(8, false, false, true, true)), Decision::RejectToPrev);
    }

    #[test]
    fn unnumbered_long_prose_heading_accepted() {
        assert_eq!(classify(&feats(80, false, false, false, true)), Decision::Accept);
    }

    #[test]
    fn tie_escalates() {
        // prose present but table band also detected — conflicting evidence
        assert_eq!(classify(&feats(50, false, true, false, true)), Decision::Escalate);
    }

    #[test]
    fn decision_log_jsonl_roundtrip_shape() {
        let mut log = BoundaryDecisionLog::default();
        log.push(BoundaryDecision {
            paper_id: "bk".into(),
            page: 631,
            title: "Hex World".into(),
            features: feats(5, false, true, false, true),
            decision: Decision::RejectToPrev,
        });
        let line = log.to_jsonl();
        assert!(line.contains("\"decision\":\"RejectToPrev\""));
        assert!(line.contains("\"table_band_around\":true"));
    }
}
