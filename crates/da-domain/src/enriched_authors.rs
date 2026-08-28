//! Enriched authors binding (M451 phase-2 port #7): positional TEI<->OpenAlex
//! author binding with a surname check. Pure logic over INJECTED work
//! observations (the reference fetcher's cache) + parsed TEI.

use serde::{Deserialize, Serialize};

/// TEI header author (from persName).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeiAuthor {
    pub full: String,
    pub surname: String,
    /// internal id basis: norm(initials+surname) — v7 vocabulary
    pub author_key: String,
}

/// One bound author row (enriched/authors.json entry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnrichedAuthor {
    pub position: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openalex_a_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orcid: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub institutions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tei_full_name: Option<String>,
    /// position_surname_confirmed | position_surname_mismatch | openalex_only
    pub binding: String,
}

/// Normalize for name comparison: NFKD-lowercase alnum only.
pub fn norm_name(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            let c = c.to_ascii_lowercase();
            if c.is_ascii_alphanumeric() {
                Some(c)
            } else {
                None
            }
        })
        .collect()
}

/// Parse TEI header authors (forename/surname parts) in document order.
pub fn parse_tei_authors(tei: &str) -> Vec<TeiAuthor> {
    use regex::Regex;
    let file_re = Regex::new(r"(?s)<fileDesc>(.*?)</fileDesc>").expect("static");
    let pers_re = Regex::new(r"(?s)<persName>(.*?)</persName>").expect("static");
    let fore_re = Regex::new(r"<forename[^>]*>([^<]+)</forename>").expect("static");
    let sur_re = Regex::new(r"<surname[^>]*>([^<]+)</surname>").expect("static");
    let mut out = Vec::new();
    if let Some(f) = file_re.captures(tei) {
        for p in pers_re.captures_iter(&f[1]) {
            let fores: Vec<String> = fore_re
                .captures_iter(&p[1])
                .map(|c| c[1].trim().to_string())
                .collect();
            let surs: Vec<String> = sur_re
                .captures_iter(&p[1])
                .map(|c| c[1].trim().to_string())
                .collect();
            let Some(last) = surs.last() else { continue };
            let initials: String = fores
                .iter()
                .filter(|f| !f.is_empty())
                .filter_map(|f| f.chars().next().map(|c| c.to_string()))
                .collect();
            let surname = last.clone();
            let full = [fores.join(" "), surname.clone()]
                .join(" ")
                .trim()
                .to_string();
            out.push(TeiAuthor {
                author_key: norm_name(&format!("{initials}{surname}")),
                full,
                surname,
            });
        }
    }
    out
}

/// One OpenAlex authorship observation (slim cache shape).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkAuthorship {
    #[serde(default)]
    pub a_id: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub orcid: Option<String>,
    #[serde(default)]
    pub institutions: Vec<String>,
}

/// Positional binding with surname check (python reference semantics):
/// same index -> confirmed if norm(surname) is contained in the display
/// name; mismatch otherwise; extra OpenAlex rows -> openalex_only.
pub fn bind_authors(tei: &[TeiAuthor], oa: &[WorkAuthorship]) -> Vec<EnrichedAuthor> {
    let n = tei.len().max(oa.len());
    (0..n)
        .map(|i| {
            let t = tei.get(i);
            let o = oa.get(i);
            match (t, o) {
                (Some(t), Some(o)) => {
                    let confirmed = o
                        .display_name
                        .as_deref()
                        .map(|dn| {
                            norm_name(&t.surname).is_empty()
                                || norm_name(dn).contains(&norm_name(&t.surname))
                        })
                        .unwrap_or(false);
                    EnrichedAuthor {
                        position: i,
                        openalex_a_id: o.a_id.clone(),
                        display_name: o.display_name.clone(),
                        orcid: o.orcid.clone(),
                        institutions: o.institutions.clone(),
                        author_key: Some(t.author_key.clone()),
                        tei_full_name: Some(t.full.clone()),
                        binding: if confirmed {
                            "position_surname_confirmed".into()
                        } else {
                            "position_surname_mismatch".into()
                        },
                    }
                }
                (Some(t), None) => EnrichedAuthor {
                    position: i,
                    openalex_a_id: None,
                    display_name: None,
                    orcid: None,
                    institutions: vec![],
                    author_key: Some(t.author_key.clone()),
                    tei_full_name: Some(t.full.clone()),
                    binding: "tei_only".into(),
                },
                (None, Some(o)) => EnrichedAuthor {
                    position: i,
                    openalex_a_id: o.a_id.clone(),
                    display_name: o.display_name.clone(),
                    orcid: o.orcid.clone(),
                    institutions: o.institutions.clone(),
                    author_key: None,
                    tei_full_name: None,
                    binding: "openalex_only".into(),
                },
                (None, None) => unreachable!(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEI: &str = r#"<fileDesc><author><persName><forename>Madhu</forename><forename>S.</forename><surname>Advani</surname></persName></author><persName><forename>Jin</forename><forename>Hwa</forename><surname>Lee</surname></persName></fileDesc>"#;

    #[test]
    fn parses_tei_authors_with_keys() {
        let a = parse_tei_authors(TEI);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].full, "Madhu S. Advani");
        assert_eq!(a[0].author_key, "msadvani");
        assert_eq!(a[1].author_key, "jhlee");
    }

    #[test]
    fn positional_binding_confirm_mismatch_only() {
        let tei = parse_tei_authors(TEI);
        let oa = vec![
            WorkAuthorship {
                a_id: Some("A1".into()),
                display_name: Some("Madhu S. Advani".into()),
                orcid: Some("0-1".into()),
                institutions: vec![],
            },
            WorkAuthorship {
                a_id: Some("A2".into()),
                display_name: Some("Completely Different".into()),
                orcid: None,
                institutions: vec!["Duke".into()],
            },
            WorkAuthorship {
                a_id: Some("A3".into()),
                display_name: Some("Extra Person".into()),
                orcid: None,
                institutions: vec![],
            },
        ];
        let b = bind_authors(&tei, &oa);
        assert_eq!(b.len(), 3);
        assert_eq!(b[0].binding, "position_surname_confirmed");
        assert_eq!(b[1].binding, "position_surname_mismatch");
        assert_eq!(b[2].binding, "openalex_only");
        assert_eq!(b[1].author_key.as_deref(), Some("jhlee"));
    }
}
