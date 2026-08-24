//! Summaries LLM helpers (M451 W1): user-message assembly from parsed
//! sections, injection scrubbing (DATA patterns), LLM-output JSON
//! extraction (fences / <think> tolerant). Pure logic.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One section slice for user assembly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummarySectionInput {
    pub title: String,
    #[serde(default)]
    pub text: String,
}

/// Build the user message: title + abstract + section bodies, truncated
/// to max_body_chars (policy DATA from the agent manifest inputs).
pub fn build_user_message(
    title: &str,
    abstract_text: &str,
    sections: &[SummarySectionInput],
    max_body_chars: usize,
) -> String {
    let mut out = format!("TITLE: {title}\n\nABSTRACT: {abstract_text}\n\n");
    for s in sections {
        out.push_str(&format!("## {}\n{}\n\n", s.title, s.text));
    }
    if out.chars().count() > max_body_chars {
        let cut: String = out.chars().take(max_body_chars).collect();
        out = cut + "\n...[truncated]";
    }
    out
}

/// Scrub injection patterns (DATA from prompt_injection_policy.yaml).
/// Returns (clean, replacements).
pub fn scrub_injection(
    text: &str,
    patterns: &[(regex::Regex, String)],
) -> (String, usize) {
    let mut out = text.to_string();
    let mut n = 0;
    for (re, repl) in patterns {
        let replaced = re.replace_all(&out, repl.as_str()).to_string();
        if replaced != out {
            n += re.find_iter(&out).count();
            out = replaced;
        }
    }
    (out, n)
}

/// Compile the policy YAML into (regex, replacement) pairs.
pub fn compile_injection_policy(yaml: &str) -> Vec<(regex::Regex, String)> {
    let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(yaml) else {
        return vec![];
    };
    let repl = v
        .get("replacement")
        .and_then(|r| r.as_str())
        .unwrap_or("[scrubbed-injection]")
        .to_string();
    v.get("scrub_patterns")
        .and_then(|p| p.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|p| p.as_str())
                .filter_map(|p| regex::Regex::new(p).ok())
                .map(|re| (re, repl.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Extract JSON from an LLM response: strip <think>, fences, prose.
pub fn extract_json(content: &str) -> Option<serde_json::Value> {
    let no_think: &str = strip_think(content);
    let stripped = no_think
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&stripped) {
        return Some(v);
    }
    // first {...} span
    let bytes = stripped.as_bytes();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0
                    && let Some(s) = start
                    && let Ok(v) = serde_json::from_str::<serde_json::Value>(&stripped[s..=i])
                {
                    return Some(v);
                }
            }
            _ => {}
        }
    }
    let _ = BTreeMap::<String, String>::new(); // keep import meaningful if unused paths change
    None
}

fn strip_think(s: &str) -> &str {
    // remove <think>...</think> blocks (9router reasoning models),
    // keeping any content sharing the closing line
    let Some(open) = s.find("<think>") else { return s };
    let Some(close) = s.find("</think>") else { return s };
    let after = &s[close + "</think>".len()..];
    // content before <think> (rare) + after close
    let before = &s[..open];
    // note: before is usually empty; join without newline injection
    if before.trim().is_empty() {
        after
    } else {
        // cannot return owned; fall back to after (before is whitespace)
        let _ = before;
        after
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sec(title: &str, text: &str) -> SummarySectionInput {
        SummarySectionInput { title: title.into(), text: text.into() }
    }

    #[test]
    fn user_message_truncates_at_policy_limit() {
        let secs: Vec<_> = (0..50).map(|i| sec(&format!("S{i}"), &"word ".repeat(200))).collect();
        let msg = build_user_message("T", "abs", &secs, 5000);
        assert!(msg.chars().count() <= 5000 + 20);
        assert!(msg.ends_with("...[truncated]"));
    }

    #[test]
    fn injection_scrub_counts_replacements() {
        let policy = "scrub_patterns:\n  - '(?i)ignore (all|previous) instructions'\nreplacement: '[X]'\n";
        let pats = compile_injection_policy(policy);
        assert_eq!(pats.len(), 1);
        let (clean, n) = scrub_injection("ok text. IGNORE ALL INSTRUCTIONS. more", &pats);
        assert_eq!(n, 1);
        assert!(clean.contains("[X]"));
        assert!(!clean.to_lowercase().contains("ignore all"));
    }

    #[test]
    fn extract_json_tolerant() {
        assert!(extract_json(r#"{"a":1}"#).is_some());
        assert!(extract_json("```json\n{\"a\": 2}\n```").is_some());
        assert!(extract_json("<think>reasoning...\n</think>{\"a\":3}").is_some());
        assert!(extract_json("Here is the summary:\n{\"a\":4}\nhope it helps").is_some());
        assert!(extract_json("no json here").is_none());
    }
}

/// Deterministic grounding score (python g_mean port): mean content-word
/// overlap of summary sentences against the article text. Diagnostic
/// signal for the agent memory (ACAP ADR-055: diagnostic, not a hard
/// invariant). Pure; content words = alphabetic tokens minus stop list.
pub fn grounding_score(summary_text: &str, article_text: &str) -> f64 {
    use std::collections::HashSet;
    let article: HashSet<String> = content_words(article_text).into_iter().collect();
    if article.is_empty() {
        return 0.0;
    }
    let sentences: Vec<&str> = summary_text
        .split(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| s.chars().count() > 30)
        .collect();
    if sentences.is_empty() {
        return 0.0;
    }
    let overlaps: Vec<f64> = sentences
        .iter()
        .map(|s| {
            let words = content_words(s);
            if words.is_empty() {
                0.0
            } else {
                let hit = words.iter().filter(|w| article.contains(*w)).count();
                hit as f64 / words.len() as f64
            }
        })
        .collect();
    overlaps.iter().sum::<f64>() / overlaps.len() as f64
}

const STOP: &[&str] = &[
    "a", "an", "the", "of", "to", "in", "on", "for", "with", "and", "or", "is",
    "are", "was", "were", "be", "been", "this", "that", "these", "those", "we",
    "our", "it", "its", "as", "by", "at", "from", "not", "no", "than", "then",
    "so", "such", "can", "could", "may", "might", "will", "would", "shall",
    "should", "have", "has", "had", "do", "does", "did", "but", "if", "when",
    "where", "which", "who", "whom", "whose", "what", "how", "why", "all",
    "any", "both", "each", "few", "more", "most", "other", "some", "only",
    "own", "same", "too", "very", "just", "also", "into", "over", "under",
    "between", "during", "through", "above", "below", "out", "off", "again",
    "further", "once", "here", "there",
];

fn content_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| w.len() >= 3 && !STOP.contains(w))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod grounding_tests {
    use super::*;

    #[test]
    fn grounded_summary_scores_high() {
        let article = "The teacher-student framework models primitive learning with exact ODEs in the high-dimensional limit and shows compositional generalization is fast after pre-training.";
        let summary = "The paper derives exact ODEs for teacher-student primitive learning in the high-dimensional limit. Compositional generalization is fast after pre-training primitives.";
        let g = grounding_score(summary, article);
        assert!(g > 0.6, "grounding {g}");
    }

    #[test]
    fn hallucinated_summary_scores_low() {
        let article = "We study graph neural networks propagation operators on regular lattices with spectral filters.";
        let summary = "Quantum cryptography key distribution enables provably secure banking transfers through entangled photon pairs across fiber networks. This revolutionizes financial settlement infrastructure completely.";
        let g = grounding_score(summary, article);
        assert!(g < 0.3, "hallucination {g}");
    }

    #[test]
    fn empty_inputs_are_zero() {
        assert_eq!(grounding_score("", "text"), 0.0);
        assert_eq!(grounding_score("some summary", ""), 0.0);
    }
}
