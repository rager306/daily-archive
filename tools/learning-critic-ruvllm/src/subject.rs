//! Redacted structural subject: the single source of truth for the subject
//! grammar crossing the critic boundary.
//!
//! `derive_subject` turns a TEI document into a tag-count profile (no TEI
//! bytes, no text content cross this boundary - the redaction barrier), and
//! [`StructuralProfile::parse`] reads exactly that grammar back. Keeping both
//! sides in one module means the critic can never drift from the producer.

/// Derive the redacted structural profile line for a TEI document.
pub fn derive_subject(tei: &str) -> String {
    let count = |tag: &str| tei.matches(&format!("<{tag}")).count() as u32;
    let p = StructuralProfile {
        sections: count("div"),
        references: count("biblStruct"),
        figures: count("figure"),
        tables: count("table"),
        formulas: count("formula"),
        abstract_present: count("abstract") > 0,
        funding_present: count("funding") > 0,
        notes: count("note"),
        keywords: count("keywords") > 0,
        language_set: tei.contains("xml:lang"),
    };
    p.subject_line()
}

/// Evidence cap when any required marker is short of full credit.
///
/// Lands the calibrated score at most 0.75 (vendor 1.0 * 0.5 + 0.5 * cap),
/// strictly below the 0.8 degraded boundary of the Н-1 acceptance.
pub const DEGRADED_EVIDENCE_CAP: f64 = 0.5;

/// Parsed form of the redacted structural profile (`derive_subject` output).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralProfile {
    pub sections: u32,
    pub references: u32,
    pub figures: u32,
    pub tables: u32,
    pub formulas: u32,
    pub abstract_present: bool,
    pub funding_present: bool,
    pub notes: u32,
    pub keywords: bool,
    pub language_set: bool,
}

impl StructuralProfile {
    /// Parse the exact grammar emitted by `derive_subject`.
    ///
    /// Strict by design: anything else is an unparseable subject and the
    /// critic treats it as malformed instead of guessing.
    pub fn parse(subject: &str) -> Option<Self> {
        let mut fields: Vec<(&str, &str)> = Vec::new();
        for part in subject.split("; ") {
            let (key, value) = part.split_once(": ")?;
            fields.push((key.trim(), value.trim()));
        }
        if fields.len() != 10 {
            return None;
        }
        let get =
            |key: &str| -> Option<&str> { fields.iter().find(|(k, _)| *k == key).map(|(_, v)| *v) };
        let num = |key: &str| -> Option<u32> { get(key)?.parse().ok() };
        let flag = |key: &str| -> Option<bool> {
            match get(key)? {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            }
        };
        Some(Self {
            sections: num("sections")?,
            references: num("references")?,
            figures: num("figures")?,
            tables: num("tables")?,
            formulas: num("formulas")?,
            abstract_present: flag("abstract_present")?,
            funding_present: flag("funding_present")?,
            notes: num("notes")?,
            keywords: flag("keywords")?,
            language_set: flag("language_set")?,
        })
    }

    /// Structural evidence score in [0, 1]: presence of the substantive
    /// markers a real fulltext parse exhibits, not absence of badness.
    ///
    /// Four required markers (graded): structural divisions (`sections`,
    /// full credit at >= 3, partial at 1-2), bibliography (`references`,
    /// full at >= 5, partial at 1-4), an abstract, and header metadata
    /// (`language_set` or `keywords`). Six optional body markers: figures,
    /// tables, formulas, notes, funding, keywords.
    ///
    /// Any required marker short of full credit caps the evidence at
    /// [`DEGRADED_EVIDENCE_CAP`]: for this project a fulltext parse without
    /// a bibliography (or an abstract, or header metadata) is degraded no
    /// matter how rich the rest is - the citation graph is the product.
    /// Grading still separates severity below the cap (all-zero 0.0 vs
    /// bibliography-lost 0.5).
    pub fn evidence(&self) -> f64 {
        let sections = if self.sections >= 3 {
            1.0
        } else if self.sections >= 1 {
            0.4
        } else {
            0.0
        };
        let references = if self.references >= 5 {
            1.0
        } else if self.references >= 1 {
            0.4
        } else {
            0.0
        };
        let metadata = f64::from(self.language_set || self.keywords);
        let required = (sections + references + f64::from(self.abstract_present) + metadata) / 4.0;
        let optional = [
            self.figures > 0,
            self.tables > 0,
            self.formulas > 0,
            self.notes > 0,
            self.funding_present,
            self.keywords,
        ]
        .iter()
        .filter(|present| **present)
        .count();
        let mut evidence = 0.8 * required + 0.2 * (optional as f64) / 6.0;
        if required < 1.0 {
            evidence = evidence.min(DEGRADED_EVIDENCE_CAP);
        }
        evidence
    }

    fn subject_line(self) -> String {
        format!(
            "sections: {}; references: {}; figures: {}; tables: {}; formulas: {}; \
             abstract_present: {}; funding_present: {}; notes: {}; \
             keywords: {}; language_set: {}",
            self.sections,
            self.references,
            self.figures,
            self.tables,
            self.formulas,
            self.abstract_present,
            self.funding_present,
            self.notes,
            self.keywords,
            self.language_set,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEI_FULLTEXT: &str = "<TEI><teiHeader xml:lang=\"en\"><abstract/><keywords/></teiHeader>\
        <text><body><div/><div/><div/><figure/><table/><formula/><note/><note/></body>\
        <back><listBibl><biblStruct/><biblStruct/><biblStruct/><biblStruct/><biblStruct/>\
        <biblStruct/></listBibl></back></text></TEI>";

    #[test]
    fn derive_subject_counts_real_markers() {
        let subject = derive_subject(TEI_FULLTEXT);
        let p = StructuralProfile::parse(&subject).expect("grammar roundtrip");
        assert_eq!(p.sections, 3);
        assert_eq!(p.references, 6);
        assert!(p.abstract_present);
        assert!(p.keywords);
        assert!(p.language_set);
        assert_eq!(p.notes, 2);
    }

    #[test]
    fn derive_subject_of_empty_tei_is_all_zero() {
        let p = StructuralProfile::parse(&derive_subject("")).expect("grammar roundtrip");
        assert_eq!(
            p,
            StructuralProfile {
                sections: 0,
                references: 0,
                figures: 0,
                tables: 0,
                formulas: 0,
                abstract_present: false,
                funding_present: false,
                notes: 0,
                keywords: false,
                language_set: false,
            }
        );
    }

    #[test]
    fn parse_rejects_foreign_subjects() {
        assert!(StructuralProfile::parse("").is_none());
        assert!(StructuralProfile::parse("hello world").is_none());
        assert!(StructuralProfile::parse(
            "sections: 1; references: 2; figures: 3; tables: 4; formulas: 5; \
             abstract_present: maybe; funding_present: false; notes: 7; \
             keywords: false; language_set: false"
        )
        .is_none());
        // missing trailing field
        assert!(StructuralProfile::parse(
            "sections: 1; references: 2; figures: 3; tables: 4; formulas: 5; \
             abstract_present: true; funding_present: false; notes: 7; keywords: false"
        )
        .is_none());
    }
}
