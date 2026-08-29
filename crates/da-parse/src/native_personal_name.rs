//! Pure, bounded, redacting personal-name parser for shadow comparison.
use serde::{Deserialize, Serialize};

pub const NATIVE_PERSONAL_NAME_PARSER_ID: &str = "da-parse.native_personal_name";
pub const NATIVE_PERSONAL_NAME_PARSER_VERSION: &str = "1.0.0";
pub const NATIVE_PERSONAL_NAME_OUTCOME_SCHEMA_VERSION: &str =
    "da-parse.native-personal-name.outcome/v1";
pub const NATIVE_PERSONAL_NAME_INPUT_BYTE_CAP: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonalNameContext {
    Header,
    Citation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecognizedPersonalName {
    pub given_name: Option<String>,
    pub middle_names: Vec<String>,
    pub surname: String,
    pub recomposed_surname: String,
    pub surname_particles: Vec<String>,
    pub suffix: Option<String>,
    pub initials: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativePersonalNameMalformed {
    ControlCharacter,
    NonAsciiScript,
    UnexpectedCharacter,
    RepeatedSeparator,
    EmptyComponent,
    GrammarMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativePersonalNameUnsupported {
    EtAl,
    UnsupportedPunctuation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativePersonalNameAmbiguous {
    SingleToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NativePersonalNameOutcome {
    Recognized {
        name: RecognizedPersonalName,
    },
    Empty,
    Malformed {
        reason: NativePersonalNameMalformed,
    },
    Unsupported {
        reason: NativePersonalNameUnsupported,
    },
    Ambiguous {
        reason: NativePersonalNameAmbiguous,
    },
    Oversized,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativePersonalNameParser;

impl NativePersonalNameParser {
    pub const fn new() -> Self {
        Self
    }

    pub fn parse(&self, context: PersonalNameContext, input: &str) -> NativePersonalNameOutcome {
        if input.len() > NATIVE_PERSONAL_NAME_INPUT_BYTE_CAP {
            return NativePersonalNameOutcome::Oversized;
        }
        if input.is_empty() || input.chars().all(char::is_whitespace) {
            return NativePersonalNameOutcome::Empty;
        }
        if input.chars().any(char::is_control) {
            return malformed(NativePersonalNameMalformed::ControlCharacter);
        }
        if input.contains("et al") || input.contains("Et al") || input.contains("ET AL") {
            return NativePersonalNameOutcome::Unsupported {
                reason: NativePersonalNameUnsupported::EtAl,
            };
        }
        if input
            .chars()
            .any(|c| c.is_whitespace() && !c.is_ascii_whitespace())
        {
            // Unicode spacing is valid and deliberately normalized only as a separator.
        }
        if input.split_whitespace().count() < 2 {
            return NativePersonalNameOutcome::Ambiguous {
                reason: NativePersonalNameAmbiguous::SingleToken,
            };
        }
        if input
            .chars()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|pair| pair[0].is_whitespace() && pair[1].is_whitespace())
        {
            return malformed(NativePersonalNameMalformed::RepeatedSeparator);
        }
        let tokens = match tokenize(input) {
            Ok(tokens) => tokens,
            Err(TokenizeError::Malformed(reason)) => return malformed(reason),
            Err(TokenizeError::Unsupported(reason)) => {
                return NativePersonalNameOutcome::Unsupported { reason };
            }
        };
        let comma = tokens.iter().position(|t| t == ",");
        let words: Vec<String> = tokens;
        if words.len() < 2 {
            return malformed(NativePersonalNameMalformed::GrammarMismatch);
        }
        let ParsedNameParts {
            given,
            middle,
            surname_words,
            suffix,
            first_initials,
        } = match (context, comma) {
            (PersonalNameContext::Citation, Some(_)) => parse_citation(words),
            (PersonalNameContext::Header, Some(_)) => {
                return malformed(NativePersonalNameMalformed::UnexpectedCharacter);
            }
            (_, None) => parse_header(words),
        };
        let surname = surname_words.join(" ");
        if surname.is_empty() {
            return malformed(NativePersonalNameMalformed::EmptyComponent);
        }
        let surname_particles = surname_words[..surname_words.len().saturating_sub(1)]
            .iter()
            .filter(|w| is_particle(w))
            .cloned()
            .collect::<Vec<_>>();
        let mut initials = first_initials;
        for word in &middle {
            initials.extend(initials_of(word));
        }
        if let Some(ref g) = given
            && is_initial(g)
        {
            initials.extend(initials_of(g));
        }
        NativePersonalNameOutcome::Recognized {
            name: RecognizedPersonalName {
                given_name: given,
                middle_names: middle,
                surname: surname.clone(),
                recomposed_surname: surname,
                surname_particles,
                suffix,
                initials,
            },
        }
    }
}

fn malformed(reason: NativePersonalNameMalformed) -> NativePersonalNameOutcome {
    NativePersonalNameOutcome::Malformed { reason }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenizeError {
    Malformed(NativePersonalNameMalformed),
    Unsupported(NativePersonalNameUnsupported),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedNameParts {
    given: Option<String>,
    middle: Vec<String>,
    surname_words: Vec<String>,
    suffix: Option<String>,
    first_initials: Vec<String>,
}

fn tokenize(input: &str) -> Result<Vec<String>, TokenizeError> {
    let mut out = Vec::new();
    let mut current = String::new();
    for c in input.chars() {
        if c.is_whitespace() || c == ',' {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            if c == ',' {
                out.push(",".into());
            }
            continue;
        }
        if matches!(c, ';' | ':' | '(' | ')' | '[' | ']' | '"') {
            return Err(TokenizeError::Unsupported(
                NativePersonalNameUnsupported::UnsupportedPunctuation,
            ));
        }
        if c.is_ascii_digit() {
            return Err(TokenizeError::Malformed(
                NativePersonalNameMalformed::UnexpectedCharacter,
            ));
        }
        let allowed_mark = ('\u{0300}'..='\u{036f}').contains(&c);
        let allowed_punctuation = c.is_ascii_punctuation() || matches!(c, '’');
        if !c.is_alphabetic() && !allowed_mark && !allowed_punctuation {
            return Err(TokenizeError::Malformed(
                NativePersonalNameMalformed::NonAsciiScript,
            ));
        }
        current.push(c);
    }
    if !current.is_empty() {
        out.push(current);
    }
    if out.iter().any(|token: &String| token == ",")
        && out.iter().filter(|token| token.as_str() == ",").count() != 1
    {
        return Err(TokenizeError::Malformed(
            NativePersonalNameMalformed::RepeatedSeparator,
        ));
    }
    if out
        .iter()
        .any(|token: &String| token.trim_matches('.').is_empty())
    {
        return Err(TokenizeError::Malformed(
            NativePersonalNameMalformed::EmptyComponent,
        ));
    }
    Ok(out)
}

fn parse_header(mut words: Vec<String>) -> ParsedNameParts {
    let suffix = words.last().filter(|w| is_suffix(w)).cloned();
    if suffix.is_some() {
        words.pop();
    }
    let surname_start = words
        .iter()
        .rposition(|w| !is_particle(w))
        .unwrap_or(words.len().saturating_sub(1));
    let surname_start =
        surname_start.saturating_sub(leading_particles_before(surname_start, &words));
    let surname_words = words.split_off(surname_start);
    let mut front = words;
    let given = front.first().map(|w| {
        if is_initial(w) {
            w.trim_matches('.').to_string()
        } else {
            w.clone()
        }
    });
    let middle = if front.len() > 1 {
        front.drain(1..).map(normalize_middle).collect()
    } else {
        Vec::new()
    };
    ParsedNameParts {
        given,
        middle,
        surname_words,
        suffix,
        first_initials: Vec::new(),
    }
}

fn parse_citation(mut words: Vec<String>) -> ParsedNameParts {
    let surname_end = words
        .iter()
        .position(|w| is_given_separator(w))
        .unwrap_or(0);
    let given_words = words.split_off(surname_end + 1);
    let surname_words = words[..surname_end].to_vec();
    let mut given_words = given_words;
    let given = given_words.pop().map(|w| w.trim_matches('.').to_string());
    let middle = given_words
        .into_iter()
        .map(|w| w.trim_matches('.').to_string())
        .collect();
    ParsedNameParts {
        given,
        middle,
        surname_words,
        suffix: None,
        first_initials: Vec::new(),
    }
}

fn normalize_middle(word: String) -> String {
    if word
        .trim_matches('.')
        .chars()
        .filter(|c| c.is_alphabetic())
        .count()
        == 1
    {
        word.trim_matches('.').to_string()
    } else {
        word
    }
}

fn is_given_separator(w: &str) -> bool {
    w == ","
}
fn leading_particles_before(index: usize, words: &[String]) -> usize {
    let mut n = 0;
    let mut i = index;
    while i > 0 && is_particle(&words[i - 1]) {
        n += 1;
        i -= 1;
    }
    n
}
fn is_particle(w: &str) -> bool {
    matches!(
        w.trim_matches('.').to_ascii_lowercase().as_str(),
        "van" | "der" | "de" | "la" | "von" | "den" | "di" | "da"
    )
}
fn is_suffix(w: &str) -> bool {
    matches!(
        w.trim_matches('.').to_ascii_lowercase().as_str(),
        "jr" | "sr" | "ii" | "iii" | "iv"
    )
}
fn is_initial(w: &str) -> bool {
    w.trim_matches('.')
        .chars()
        .filter(|c| c.is_alphabetic())
        .count()
        == 1
        || w.contains('-')
}
fn initials_of(w: &str) -> Vec<String> {
    w.trim_matches('.')
        .split('-')
        .filter(|p| !p.is_empty())
        .map(|p| p.chars().filter(|c| c.is_alphabetic()).take(1).collect())
        .collect()
}
