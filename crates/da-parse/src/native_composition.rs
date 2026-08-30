//! Pure, bounded composition of the native scholarly shadow components.
//!
//! This module is intentionally not a parser.  It accepts the closed outcomes
//! produced by the date, name, affiliation, and citation leaves and either
//! returns an honest candidate or a closed rejection.  Header and reference
//! composition are separate entry points so a caller can select Java fallback
//! independently for either lane.
use crate::{
    CitationField, NativeAffiliationLink, NativeAffiliationLinkOutcome, NativeAffiliationOutcome,
    NativeCitationAnchor, NativeCitationAnchorOutcome, NativeCitationOutcome, NativeDateOutcome,
    NativePersonalNameOutcome, NativeReference, NativeReferenceListOutcome, RecognizedDate,
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
            candidate: HeaderCandidate { date, authors },
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
