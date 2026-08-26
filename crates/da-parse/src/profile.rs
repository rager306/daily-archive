#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrobidProfile {
    pub consolidate_citations: bool,
    pub consolidate_header: bool,
}

impl GrobidProfile {
    /// Coordinate groups always requested by this adapter. `formula` was
    /// added in M400 S03 T01 so GROBID emits `coords` on `<formula>`
    /// elements; the resulting `configuration_fingerprint` change rotates the
    /// parse-stage cache identity (D204 — never mix profiles/coordinates
    /// under one cache key).
    pub const TEI_COORDINATES: &'static str = "figure,table,note,formula";

    pub const fn rich() -> Self {
        Self {
            consolidate_citations: true,
            consolidate_header: true,
        }
    }

    pub const fn structural() -> Self {
        Self {
            consolidate_citations: false,
            consolidate_header: false,
        }
    }

    /// Stable semantic name for the effective consolidation policy.
    /// Mixed policies are intentionally reported as `custom`; they remain
    /// valid but must never reuse a named profile's cache identity.
    pub fn name(self) -> &'static str {
        match (self.consolidate_header, self.consolidate_citations) {
            (true, true) => "rich",
            (false, false) => "structural",
            _ => "custom",
        }
    }

    /// Exact effective-input identity for cache keys. HTTP timeout is
    /// deliberately excluded: it does not alter successful TEI semantics.
    pub fn configuration_fingerprint(self) -> String {
        format!(
            "profile={};consolidateHeader={};consolidateCitations={};teiCoordinates={}",
            self.name(),
            u8::from(self.consolidate_header),
            u8::from(self.consolidate_citations),
            Self::TEI_COORDINATES
        )
    }

    /// The `teiCoordinates` JSON array sent to GROBID, rendered from
    /// [`Self::TEI_COORDINATES`] so the request body can never drift from the
    /// configuration fingerprint (D204 — M400 S03 T01: `formula` must reach
    /// the REST call, not just the cache key).
    pub fn tei_coordinates_json() -> String {
        let items: Vec<String> = Self::TEI_COORDINATES
            .split(',')
            .map(|c| format!("\"{c}\""))
            .collect();
        format!("[{}]", items.join(","))
    }
}
