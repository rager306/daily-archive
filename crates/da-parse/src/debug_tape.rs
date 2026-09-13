//! Parser for the GROBID model-debug dump (M506/S01/T03/T04).
//!
//! The debug dump is a `text/plain` answer in which each model invocation is
//! delimited by a header line. Two shapes exist in the 0.9.1 output:
//!
//! ```text
//! === model: segmentation ===
//! === model: date (occurrence 1 of 46) ===
//! ```
//!
//! A model invoked once carries no occurrence suffix; a model invoked per
//! candidate carries an explicit `occurrence N of M`. That distinction is the
//! whole point of parsing rather than counting: an earlier plan step assumed a
//! dump section maps one-to-one onto a document unit, but a single request can
//! invoke the same model dozens of times, so coverage accounting must be able
//! to say "the `date` model ran 46 times" instead of "there were 46 models".
//!
//! This module is a leaf: pure parsing over bytes, no I/O, no transport, no
//! policy. It deliberately does NOT treat a missing section as evidence that a
//! model did not run — the dump is a best-effort signal, so absence is reported
//! as absence of *evidence*, never as absence of *computation*.

/// One model invocation found in a debug dump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSection {
    /// Model name exactly as printed by GROBID (e.g. `segmentation`,
    /// `affiliation-address`, `date`).
    pub model: String,
    /// 1-based occurrence index when the header carried one, else `None`.
    pub occurrence: Option<u32>,
    /// Total occurrences when the header carried one, else `None`.
    pub occurrence_total: Option<u32>,
    /// Byte offset of the header line's start.
    pub start: usize,
    /// Byte offset just past this section's last byte (start of the next
    /// header, or end of the dump).
    pub end: usize,
}

impl DebugSection {
    /// The body of this section, excluding its header line.
    pub fn body<'a>(&self, dump: &'a str) -> &'a str {
        dump[self.start..self.end]
            .split_once('\n')
            .map(|(_, body)| body)
            .unwrap_or("")
    }
}

fn parse_header(line: &str) -> Option<(String, Option<u32>, Option<u32>)> {
    let inner = line.strip_prefix("=== model: ")?.strip_suffix(" ===")?;
    match inner.split_once(" (occurrence ") {
        // `name (occurrence N of M)`
        Some((name, rest)) => {
            let (n, m) = rest.strip_suffix(')')?.split_once(" of ")?;
            let occurrence = n.trim().parse::<u32>().ok()?;
            let total = m.trim().parse::<u32>().ok()?;
            let name = name.trim();
            // A blank name would record an anonymous invocation, inflating
            // coverage with a model nobody can attribute evidence to.
            if name.is_empty() {
                return None;
            }
            Some((name.to_string(), Some(occurrence), Some(total)))
        }
        // `name`
        None => {
            let name = inner.trim();
            if name.is_empty() {
                return None;
            }
            Some((name.to_string(), None, None))
        }
    }
}

/// Parse every model section header in one debug dump.
///
/// Offsets are byte offsets into `dump`, so a caller can slice bodies without
/// copying the whole dump around. Lines that merely resemble a header (for
/// example inside a model's own output) are only recognised when the full
/// `=== model: ... ===` shape matches.
pub fn parse_debug_sections(dump: &str) -> Vec<DebugSection> {
    let mut headers: Vec<(usize, String, Option<u32>, Option<u32>)> = Vec::new();
    let mut offset = 0usize;
    for line in dump.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
        if let Some((model, occurrence, total)) = parse_header(trimmed) {
            headers.push((offset, model, occurrence, total));
        }
        offset += line.len();
    }

    let dump_len = dump.len();
    headers
        .iter()
        .enumerate()
        .map(|(index, (start, model, occurrence, total))| DebugSection {
            model: model.clone(),
            occurrence: *occurrence,
            occurrence_total: *total,
            start: *start,
            end: headers
                .get(index + 1)
                .map(|(next_start, ..)| *next_start)
                .unwrap_or(dump_len),
        })
        .collect()
}

/// Per-model invocation counts, in first-appearance order.
///
/// This is the coverage view: it answers "which models appear in this dump and
/// how many times each ran", which is what a tape needs before it can claim a
/// model was actually exercised.
pub fn model_invocation_counts(dump: &str) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for section in parse_debug_sections(dump) {
        match counts.iter_mut().find(|(name, _)| *name == section.model) {
            Some((_, count)) => *count += 1,
            None => counts.push((section.model, 1)),
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::{model_invocation_counts, parse_debug_sections};

    const DUMP: &str = "\
=== model: segmentation ===
tok1\tA\t<header>
tok2\tB\t<body>
=== model: date (occurrence 1 of 2) ===
1\t<date>
=== model: date (occurrence 2 of 2) ===
2\t<date>
";

    #[test]
    fn parses_both_header_shapes_and_keeps_occurrence_identity() {
        let sections = parse_debug_sections(DUMP);
        assert_eq!(sections.len(), 3);

        assert_eq!(sections[0].model, "segmentation");
        assert_eq!(sections[0].occurrence, None);
        assert_eq!(sections[0].occurrence_total, None);

        assert_eq!(sections[1].model, "date");
        assert_eq!(sections[1].occurrence, Some(1));
        assert_eq!(sections[1].occurrence_total, Some(2));

        assert_eq!(sections[2].model, "date");
        assert_eq!(sections[2].occurrence, Some(2));
        assert_eq!(sections[2].occurrence_total, Some(2));
    }

    #[test]
    fn section_bodies_round_trip_without_copying_the_dump() {
        let sections = parse_debug_sections(DUMP);
        assert_eq!(
            sections[0].body(DUMP),
            "tok1\tA\t<header>\ntok2\tB\t<body>\n"
        );
        assert_eq!(sections[1].body(DUMP), "1\t<date>\n");
        // The last section ends at the end of the dump.
        assert_eq!(sections[2].end, DUMP.len());
    }

    #[test]
    fn repeated_invocations_are_counted_per_model_not_per_section() {
        // The trap this guards: three sections but only two distinct models,
        // and `date` ran twice inside one request.
        assert_eq!(
            model_invocation_counts(DUMP),
            vec![("segmentation".to_string(), 1), ("date".to_string(), 2)]
        );
    }

    #[test]
    fn a_dump_without_any_section_yields_no_coverage_and_does_not_panic() {
        assert!(parse_debug_sections("").is_empty());
        assert!(parse_debug_sections("plain text with no headers\n").is_empty());
        assert!(model_invocation_counts("just a feature vector\n").is_empty());
    }

    #[test]
    fn near_miss_header_shapes_are_not_treated_as_sections() {
        // Missing the closing marker, wrong prefix, or an unparsable
        // occurrence must not be counted as a model invocation — a false
        // positive here would inflate coverage.
        for near_miss in [
            "=== model: segmentation ==\n",
            "== model: segmentation ===\n",
            "=== model: date (occurrence one of two) ===\n",
            "=== model: date (occurrence 1 of) ===\n",
            "=== model:  ===\n",
        ] {
            assert!(
                parse_debug_sections(near_miss).is_empty(),
                "near miss must not parse: {near_miss:?}"
            );
        }
    }

    #[test]
    fn blank_model_names_are_rejected_rather_than_recorded() {
        assert!(parse_debug_sections("=== model:    ===\n").is_empty());
    }
}
