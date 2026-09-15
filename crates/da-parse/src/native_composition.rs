//! Pure, bounded composition of the native scholarly shadow components.
//!
//! This module is intentionally not a parser.  It accepts the closed outcomes
//! produced by the date, name, affiliation, and citation leaves and either
//! returns an honest candidate or a closed rejection.  Header and reference
//! composition are separate entry points so a caller can select Java fallback
//! independently for either lane.
use crate::{
    AffiliationSpan, CitationField, NativeAffiliationAuthorEvidence, NativeAffiliationLink,
    NativeAffiliationLinkOutcome, NativeAffiliationOutcome, NativeAffiliationParser,
    NativeCitationAnchor, NativeCitationAnchorOutcome, NativeCitationAnchorScanner,
    NativeCitationOutcome, NativeCitationParser, NativeDateOutcome, NativeDateParser,
    NativePersonalNameOutcome, NativePersonalNameParser, NativeReference,
    NativeReferenceListOutcome, NativeReferenceSegmenter, PersonalNameContext, RecognizedDate,
    RecognizedPersonalName,
};
use serde::{Deserialize, Serialize};

pub const NATIVE_COMPOSITION_SCHEMA_VERSION: &str = "da-parse.native-composition/v1";
pub const NATIVE_COMPOSITION_MAX_AUTHORS: usize = 4096;
pub const NATIVE_COMPOSITION_MAX_AFFILIATIONS: usize = 4096;
pub const NATIVE_COMPOSITION_MAX_REFERENCES: usize = 4096;
pub const NATIVE_COMPOSITION_MAX_FIELDS: usize = 32768;
pub const NATIVE_COMPOSITION_MAX_ANCHORS: usize = 16384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionSpan {
    pub start: usize,
    pub end: usize,
}
impl CompositionSpan {
    pub const fn valid(self) -> bool {
        self.start < self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderAuthorInput {
    pub ordinal: usize,
    pub span: CompositionSpan,
    pub name: NativePersonalNameOutcome,
    pub affiliations: NativeAffiliationLinkOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderCompositionInput {
    pub date: NativeDateOutcome,
    pub authors: Vec<HeaderAuthorInput>,
    pub affiliation_components: Vec<NativeAffiliationOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderCandidate {
    pub date: RecognizedDate,
    pub authors: Vec<HeaderAuthor>,
    /// Projected native CRF fields; legacy composition keeps this empty.
    /// `serde(default)` keeps candidates serialized before this field existed
    /// readable instead of failing on a missing key.
    #[serde(default)]
    pub fields: crate::NativeHeaderFields,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderAuthor {
    pub ordinal: usize,
    pub span: CompositionSpan,
    pub name: RecognizedPersonalName,
    pub affiliations: Vec<NativeAffiliationLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentReceipt {
    pub component: CompositionComponent,
    pub disposition: ComponentDisposition,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionComponent {
    Date,
    Names,
    Affiliations,
    CitationFields,
    References,
    Anchors,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentDisposition {
    Accepted,
    Empty,
    Rejected,
    Truncated,
    Oversized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderCompositionCandidate {
    pub candidate: HeaderCandidate,
    pub receipts: Vec<ComponentReceipt>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceCompositionInput {
    pub references: NativeReferenceListOutcome,
    pub fields: Vec<NativeCitationOutcome>,
    pub anchors: NativeCitationAnchorOutcome,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceCandidate {
    pub references: Vec<NativeReference>,
    pub fields: Vec<CitationField>,
    pub anchors: Vec<NativeCitationAnchor>,
    pub adjacent_pairs: Vec<(usize, usize)>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceCompositionCandidate {
    pub candidate: ReferenceCandidate,
    pub receipts: Vec<ComponentReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionError {
    RejectedComponent {
        component: CompositionComponent,
    },
    EmptyComponent {
        component: CompositionComponent,
    },
    OversizedComponent {
        component: CompositionComponent,
    },
    TruncatedComponent {
        component: CompositionComponent,
    },
    InvalidSpan {
        component: CompositionComponent,
        ordinal: usize,
    },
    ReorderedOrdinal {
        component: CompositionComponent,
        previous: usize,
        current: usize,
    },
    DuplicateOrdinal {
        component: CompositionComponent,
        ordinal: usize,
    },
    OverlappingSpan {
        component: CompositionComponent,
        previous: usize,
        current: usize,
    },
    ForeignAffiliationLink {
        author: usize,
        key: String,
    },
    InvalidAnchorPair {
        left: usize,
        right: usize,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum HeaderCompositionOutcome {
    Accepted {
        composition: HeaderCompositionCandidate,
    },
    Rejected {
        error: CompositionError,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ReferenceCompositionOutcome {
    Accepted {
        composition: ReferenceCompositionCandidate,
    },
    Rejected {
        error: CompositionError,
    },
}

/// Ordered W2 header sequence: date, then names, then affiliations, then compose.
/// A failed earlier atom must not invent a HeaderCandidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderSequenceInput<'a> {
    pub date_text: &'a str,
    pub name_texts: Vec<&'a str>,
    pub affiliation_texts: Vec<&'a str>,
}

pub fn run_header_sequence(input: HeaderSequenceInput<'_>) -> HeaderCompositionOutcome {
    let date = NativeDateParser::new().parse(input.date_text);
    match &date {
        NativeDateOutcome::Recognized { .. } => {}
        NativeDateOutcome::Empty => {
            return reject(CompositionError::EmptyComponent {
                component: CompositionComponent::Date,
            });
        }
        NativeDateOutcome::Oversized => {
            return reject(CompositionError::OversizedComponent {
                component: CompositionComponent::Date,
            });
        }
        _ => {
            return reject(CompositionError::RejectedComponent {
                component: CompositionComponent::Date,
            });
        }
    }
    if input.name_texts.is_empty() {
        return reject(CompositionError::EmptyComponent {
            component: CompositionComponent::Names,
        });
    }
    let mut authors = Vec::with_capacity(input.name_texts.len());
    let mut cursor = 0usize;
    for (ordinal, name_text) in input.name_texts.iter().enumerate() {
        let name = NativePersonalNameParser::new().parse(PersonalNameContext::Header, name_text);
        match &name {
            NativePersonalNameOutcome::Recognized { .. } => {}
            NativePersonalNameOutcome::Empty => {
                return reject(CompositionError::EmptyComponent {
                    component: CompositionComponent::Names,
                });
            }
            NativePersonalNameOutcome::Oversized => {
                return reject(CompositionError::OversizedComponent {
                    component: CompositionComponent::Names,
                });
            }
            _ => {
                return reject(CompositionError::RejectedComponent {
                    component: CompositionComponent::Names,
                });
            }
        }
        let start = cursor;
        let end = start + name_text.len().max(1);
        cursor = end + 1;
        authors.push(HeaderAuthorInput {
            ordinal,
            span: CompositionSpan { start, end },
            name,
            affiliations: NativeAffiliationLinkOutcome::Linked {
                author: NativeAffiliationAuthorEvidence {
                    ordinal,
                    span: AffiliationSpan { start: 0, end: 1 },
                },
                links: vec![],
            },
        });
    }
    let affiliation_components = input
        .affiliation_texts
        .iter()
        .map(|text| NativeAffiliationParser::new().parse(text))
        .collect();
    compose_header(HeaderCompositionInput {
        date,
        authors,
        affiliation_components,
    })
}

/// Compose the legacy header and attach already-projected native CRF fields.
///
/// This additive entry point preserves `compose_header` and
/// `run_header_sequence` for existing callers while allowing the native header
/// decoder to carry title, email, and `<pubnum>`-mapped ids into composition.
pub fn compose_header_with_fields(
    input: HeaderCompositionInput,
    fields: crate::NativeHeaderFields,
) -> HeaderCompositionOutcome {
    match compose_header(input) {
        HeaderCompositionOutcome::Accepted { mut composition } => {
            composition.candidate.fields = fields;
            HeaderCompositionOutcome::Accepted { composition }
        }
        rejected => rejected,
    }
}

/// Sequence entry point that attaches already-projected native CRF fields.
///
/// `run_header_sequence` keeps its exact signature and behaviour for existing
/// callers; this variant adds the same fields attach as
/// [`compose_header_with_fields`] on the date → names → affiliations path. A
/// rejected sequence stays rejected and carries no fields at all — attaching
/// them to a rejection would be invention.
pub fn run_header_sequence_with_fields(
    input: HeaderSequenceInput<'_>,
    fields: crate::NativeHeaderFields,
) -> HeaderCompositionOutcome {
    match run_header_sequence(input) {
        HeaderCompositionOutcome::Accepted { mut composition } => {
            composition.candidate.fields = fields;
            HeaderCompositionOutcome::Accepted { composition }
        }
        rejected => rejected,
    }
}

pub fn compose_header(input: HeaderCompositionInput) -> HeaderCompositionOutcome {
    if input.authors.len() > NATIVE_COMPOSITION_MAX_AUTHORS
        || input.affiliation_components.len() > NATIVE_COMPOSITION_MAX_AFFILIATIONS
    {
        return reject(CompositionError::OversizedComponent {
            component: CompositionComponent::Names,
        });
    }
    let date = match input.date {
        NativeDateOutcome::Recognized { date } => date,
        NativeDateOutcome::Empty => {
            return reject(CompositionError::EmptyComponent {
                component: CompositionComponent::Date,
            });
        }
        NativeDateOutcome::Oversized => {
            return reject(CompositionError::OversizedComponent {
                component: CompositionComponent::Date,
            });
        }
        _ => {
            return reject(CompositionError::RejectedComponent {
                component: CompositionComponent::Date,
            });
        }
    };
    if input.authors.is_empty() {
        return reject(CompositionError::EmptyComponent {
            component: CompositionComponent::Names,
        });
    }
    for outcome in &input.affiliation_components {
        match outcome {
            NativeAffiliationOutcome::Recognized { components } => {
                if components
                    .iter()
                    .any(|component| component.span.start >= component.span.end)
                {
                    return reject(CompositionError::InvalidSpan {
                        component: CompositionComponent::Affiliations,
                        ordinal: 0,
                    });
                }
            }
            NativeAffiliationOutcome::Empty => {
                return reject(CompositionError::EmptyComponent {
                    component: CompositionComponent::Affiliations,
                });
            }
            _ => {
                return reject(CompositionError::RejectedComponent {
                    component: CompositionComponent::Affiliations,
                });
            }
        }
    }
    let mut authors = Vec::with_capacity(input.authors.len());
    let mut previous: Option<usize> = None;
    let mut previous_span: Option<CompositionSpan> = None;
    for item in input.authors {
        if !item.span.valid() {
            return reject(CompositionError::InvalidSpan {
                component: CompositionComponent::Names,
                ordinal: item.ordinal,
            });
        }
        if let Some(previous_span) = previous_span
            && item.span.start < previous_span.end
        {
            return reject(CompositionError::OverlappingSpan {
                component: CompositionComponent::Names,
                previous: previous.unwrap_or(item.ordinal),
                current: item.ordinal,
            });
        }
        if let Some(p) = previous {
            if item.ordinal == p {
                return reject(CompositionError::DuplicateOrdinal {
                    component: CompositionComponent::Names,
                    ordinal: item.ordinal,
                });
            }
            if item.ordinal < p {
                return reject(CompositionError::ReorderedOrdinal {
                    component: CompositionComponent::Names,
                    previous: p,
                    current: item.ordinal,
                });
            }
        }
        previous = Some(item.ordinal);
        previous_span = Some(item.span);
        let name = match item.name {
            NativePersonalNameOutcome::Recognized { name } => name,
            NativePersonalNameOutcome::Empty => {
                return reject(CompositionError::EmptyComponent {
                    component: CompositionComponent::Names,
                });
            }
            _ => {
                return reject(CompositionError::RejectedComponent {
                    component: CompositionComponent::Names,
                });
            }
        };
        let affiliations = match item.affiliations {
            NativeAffiliationLinkOutcome::Linked { author, links } => {
                if !author.span.start.lt(&author.span.end) {
                    return reject(CompositionError::InvalidSpan {
                        component: CompositionComponent::Affiliations,
                        ordinal: item.ordinal,
                    });
                }
                if author.ordinal != item.ordinal {
                    return reject(CompositionError::ForeignAffiliationLink {
                        author: item.ordinal,
                        key: "author_ordinal".into(),
                    });
                }
                links
            }
            NativeAffiliationLinkOutcome::Error { error: _ } => {
                return reject(CompositionError::RejectedComponent {
                    component: CompositionComponent::Affiliations,
                });
            }
        };
        authors.push(HeaderAuthor {
            ordinal: item.ordinal,
            span: item.span,
            name,
            affiliations,
        });
    }
    HeaderCompositionOutcome::Accepted {
        composition: HeaderCompositionCandidate {
            candidate: HeaderCandidate {
                date,
                authors,
                fields: crate::NativeHeaderFields::default(),
            },
            receipts: vec![
                receipt(CompositionComponent::Date, ComponentDisposition::Accepted),
                receipt(CompositionComponent::Names, ComponentDisposition::Accepted),
                receipt(
                    CompositionComponent::Affiliations,
                    ComponentDisposition::Accepted,
                ),
            ],
        },
    }
}

/// Ordered W3 bibliography sequence: segment, then fields, then compose.
/// A lost or empty middle reference must not invent a ReferenceCandidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceSequenceInput<'a> {
    pub entries: Vec<&'a str>,
    pub body_text: Option<&'a str>,
}

pub fn run_reference_sequence(input: ReferenceSequenceInput<'_>) -> ReferenceCompositionOutcome {
    let references = NativeReferenceSegmenter::new().segment(&input.entries);
    match &references {
        NativeReferenceListOutcome::Recognized { references } => {
            if references.len() != input.entries.len() {
                return reject_ref(CompositionError::RejectedComponent {
                    component: CompositionComponent::References,
                });
            }
        }
        NativeReferenceListOutcome::Empty => {
            return reject_ref(CompositionError::EmptyComponent {
                component: CompositionComponent::References,
            });
        }
        NativeReferenceListOutcome::Oversized => {
            return reject_ref(CompositionError::OversizedComponent {
                component: CompositionComponent::References,
            });
        }
        NativeReferenceListOutcome::Truncated => {
            return reject_ref(CompositionError::TruncatedComponent {
                component: CompositionComponent::References,
            });
        }
        NativeReferenceListOutcome::Malformed { .. } => {
            return reject_ref(CompositionError::RejectedComponent {
                component: CompositionComponent::References,
            });
        }
    }
    let fields = input
        .entries
        .iter()
        .map(|entry| NativeCitationParser::new().parse(entry))
        .collect();
    let anchors = match input.body_text {
        Some(body) => NativeCitationAnchorScanner::new().scan(body),
        None => NativeCitationAnchorOutcome::Empty,
    };
    compose_references(ReferenceCompositionInput {
        references,
        fields,
        anchors,
    })
}

pub fn compose_references(input: ReferenceCompositionInput) -> ReferenceCompositionOutcome {
    let references = match input.references {
        NativeReferenceListOutcome::Recognized { references } => references,
        NativeReferenceListOutcome::Empty => {
            return reject_ref(CompositionError::EmptyComponent {
                component: CompositionComponent::References,
            });
        }
        NativeReferenceListOutcome::Oversized => {
            return reject_ref(CompositionError::OversizedComponent {
                component: CompositionComponent::References,
            });
        }
        NativeReferenceListOutcome::Truncated => {
            return reject_ref(CompositionError::TruncatedComponent {
                component: CompositionComponent::References,
            });
        }
        NativeReferenceListOutcome::Malformed { .. } => {
            return reject_ref(CompositionError::RejectedComponent {
                component: CompositionComponent::References,
            });
        }
    };
    if references.len() > NATIVE_COMPOSITION_MAX_REFERENCES {
        return reject_ref(CompositionError::OversizedComponent {
            component: CompositionComponent::References,
        });
    }
    let mut previous: Option<usize> = None;
    for reference in &references {
        if !reference.span.is_valid_for(reference.source.len()) {
            return reject_ref(CompositionError::InvalidSpan {
                component: CompositionComponent::References,
                ordinal: reference.ordinal,
            });
        }
        if let Some(p) = previous {
            if reference.ordinal == p {
                return reject_ref(CompositionError::DuplicateOrdinal {
                    component: CompositionComponent::References,
                    ordinal: reference.ordinal,
                });
            }
            if reference.ordinal < p {
                return reject_ref(CompositionError::ReorderedOrdinal {
                    component: CompositionComponent::References,
                    previous: p,
                    current: reference.ordinal,
                });
            }
        }
        previous = Some(reference.ordinal);
    }
    let mut fields = Vec::new();
    if input.fields.len() > NATIVE_COMPOSITION_MAX_FIELDS {
        return reject_ref(CompositionError::OversizedComponent {
            component: CompositionComponent::CitationFields,
        });
    }
    let fields_present = !input.fields.is_empty();
    for outcome in input.fields {
        match outcome {
            NativeCitationOutcome::Recognized { fields: values } => fields.extend(values),
            NativeCitationOutcome::Empty => {}
            _ => {
                return reject_ref(CompositionError::RejectedComponent {
                    component: CompositionComponent::CitationFields,
                });
            }
        }
    }
    let (anchors, adjacent_pairs) = match input.anchors {
        NativeCitationAnchorOutcome::Recognized {
            anchors,
            adjacent_pairs,
        } => (anchors, adjacent_pairs),
        NativeCitationAnchorOutcome::Empty => (Vec::new(), Vec::new()),
        NativeCitationAnchorOutcome::Oversized => {
            return reject_ref(CompositionError::OversizedComponent {
                component: CompositionComponent::Anchors,
            });
        }
        NativeCitationAnchorOutcome::Truncated => {
            return reject_ref(CompositionError::TruncatedComponent {
                component: CompositionComponent::Anchors,
            });
        }
        NativeCitationAnchorOutcome::Malformed { .. } => {
            return reject_ref(CompositionError::RejectedComponent {
                component: CompositionComponent::Anchors,
            });
        }
    };
    if anchors.len() > NATIVE_COMPOSITION_MAX_ANCHORS {
        return reject_ref(CompositionError::OversizedComponent {
            component: CompositionComponent::Anchors,
        });
    }
    let anchors_present = !anchors.is_empty();
    for &(left, right) in &adjacent_pairs {
        if right != left + 1 || right >= anchors.len() {
            return reject_ref(CompositionError::InvalidAnchorPair { left, right });
        }
    }
    ReferenceCompositionOutcome::Accepted {
        composition: ReferenceCompositionCandidate {
            candidate: ReferenceCandidate {
                references,
                fields,
                anchors,
                adjacent_pairs,
            },
            receipts: vec![
                receipt(
                    CompositionComponent::References,
                    ComponentDisposition::Accepted,
                ),
                receipt(
                    CompositionComponent::CitationFields,
                    if fields_present {
                        ComponentDisposition::Accepted
                    } else {
                        ComponentDisposition::Empty
                    },
                ),
                receipt(
                    CompositionComponent::Anchors,
                    if anchors_present {
                        ComponentDisposition::Accepted
                    } else {
                        ComponentDisposition::Empty
                    },
                ),
            ],
        },
    }
}

fn receipt(component: CompositionComponent, disposition: ComponentDisposition) -> ComponentReceipt {
    ComponentReceipt {
        component,
        disposition,
    }
}
fn reject(error: CompositionError) -> HeaderCompositionOutcome {
    HeaderCompositionOutcome::Rejected { error }
}
fn reject_ref(error: CompositionError) -> ReferenceCompositionOutcome {
    ReferenceCompositionOutcome::Rejected { error }
}
