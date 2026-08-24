//! TEI figures extraction (M451 phase-2 port #4): untyped `<figure>`
//! elements with xml:id from GROBID TEI — caption (figDesc), label,
//! graphic presence, in-text reference counts. Pure regex parsing kept
//! byte-compatible with the python reference (cross-validated).

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeiFigure {
    pub figure_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    pub ref_count: usize,
    pub has_graphic: bool,
    pub caption_chars: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeiFiguresStats {
    pub count: usize,
    pub with_caption: usize,
    pub with_graphic: usize,
    pub referenced_in_text: usize,
    pub total_in_text_refs: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum TeiFiguresError {
    #[error("tei figures: regex compile failed: {0}")]
    Regex(String),
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn strip_tags(s: &str) -> String {
    let re = Regex::new(r"<[^>]+>").expect("static");
    unescape(&re.replace_all(s, " ")).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse untyped TEI figures + in-text refs.
pub fn parse_tei_figures(tei: &str) -> Result<(Vec<TeiFigure>, TeiFiguresStats), TeiFiguresError> {
    // in-text refs: <ref type="figure" target="#fig_N">
    let refs_re = Regex::new(r##"<ref type="figure" target="#([^"]+)"##).map_err(|e| TeiFiguresError::Regex(e.to_string()))?;
    let mut refs: BTreeMap<String, usize> = BTreeMap::new();
    for cap in refs_re.captures_iter(tei) {
        let fid = cap[1].to_string();
        *refs.entry(fid).or_insert(0) += 1;
    }

    // all figures, then filter typed (tables layer) from the attrs —
    // rust regex has no lookahead, so filtering is post-match
    let fig_re = Regex::new(r##"(?s)<figure([^>]*)>(.*?)</figure>"##)
        .map_err(|e| TeiFiguresError::Regex(e.to_string()))?;
    let id_re = Regex::new(r##"xml:id="([^"]+)""##).map_err(|e| TeiFiguresError::Regex(e.to_string()))?;
    let cap_re = Regex::new(r"(?s)<figDesc>(.*?)</figDesc>").map_err(|e| TeiFiguresError::Regex(e.to_string()))?;
    let label_re = Regex::new(r"<label>([^<]*)</label>").map_err(|e| TeiFiguresError::Regex(e.to_string()))?;

    let mut out = Vec::new();
    for cap in fig_re.captures_iter(tei) {
        let attrs = cap[1].to_string();
        if attrs.contains("type=") {
            continue; // typed=table -> tables layer
        }
        let Some(id_cap) = id_re.captures(&attrs) else { continue };
        let figure_id = id_cap[1].to_string();
        let body = cap[2].to_string();
        let caption = cap_re
            .captures(&body)
            .map(|c| strip_tags(&c[1]));
        let label = label_re
            .captures(&body)
            .map(|c| c[1].trim().to_string())
            .filter(|s| !s.is_empty());
        let ref_count = refs.get(&figure_id).copied().unwrap_or(0);
        out.push(TeiFigure {
            figure_id,
            label,
            caption_chars: caption.as_ref().map(|c| c.chars().count()).unwrap_or(0),
            caption,
            ref_count,
            has_graphic: body.contains("<graphic"),
        });
    }
    out.sort_by(|a, b| a.figure_id.cmp(&b.figure_id));

    let stats = TeiFiguresStats {
        count: out.len(),
        with_caption: out.iter().filter(|f| f.caption.is_some()).count(),
        with_graphic: out.iter().filter(|f| f.has_graphic).count(),
        referenced_in_text: out.iter().filter(|f| f.ref_count > 0).count(),
        total_in_text_refs: refs.values().sum(),
    };
    Ok((out, stats))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEI: &str = r##"<body>
<figure xmlns="http://www.tei-c.org/ns/1.0" xml:id="fig_0"><head>Figure 1.</head><label>1</label><figDesc>Schema of primitives.</figDesc><graphic url="f1.png"/></figure>
<figure xmlns="http://www.tei-c.org/ns/1.0" xml:id="fig_1"><head>Figure 2.</head><label>2</label><figDesc>Learning dynamics &amp; curves.</figDesc></figure>
<figure type="table" xml:id="tab_0"><cell/></figure>
<ref type="figure" target="#fig_0">Fig</ref>
<ref type="figure" target="#fig_0">Fig</ref>
<ref type="figure" target="#fig_1">Fig</ref>
</body>"##;

    #[test]
    fn extracts_untyped_with_captions_refs_and_escaping() {
        let (figs, stats) = parse_tei_figures(TEI).unwrap();
        assert_eq!(figs.len(), 2); // tab_0 excluded
        assert_eq!(figs[0].figure_id, "fig_0");
        assert_eq!(figs[0].caption.as_deref(), Some("Schema of primitives."));
        assert!(figs[0].has_graphic);
        assert_eq!(figs[0].ref_count, 2);
        assert_eq!(figs[1].ref_count, 1);
        assert!(!figs[1].has_graphic);
        assert_eq!(figs[1].caption.as_deref(), Some("Learning dynamics & curves."));
        assert_eq!(stats.count, 2);
        assert_eq!(stats.with_caption, 2);
        assert_eq!(stats.with_graphic, 1);
        assert_eq!(stats.referenced_in_text, 2);
        assert_eq!(stats.total_in_text_refs, 3);
    }

    #[test]
    fn empty_tei_is_honest_zero() {
        let (_, stats) = parse_tei_figures("").unwrap();
        assert_eq!(stats.count, 0);
    }
}
