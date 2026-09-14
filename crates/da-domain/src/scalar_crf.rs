//! Pure reader for the text model format emitted by Wapiti.
//! The module deliberately accepts bytes only: filesystem access belongs to adapters.

use std::collections::BTreeMap;
use thiserror::Error;

pub const MAX_CRF_MODEL_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_CRF_PATTERNS: usize = 4096;
pub const MAX_CRF_LABELS: usize = 4096;
pub const MAX_CRF_OBSERVATIONS: usize = 2_000_000;
pub const MAX_CRF_WEIGHTS: usize = 10_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrfPattern {
    pub source: String,
    pub kind: char,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrfModel {
    pub model_type: u32,
    pub labels: Vec<String>,
    pub observations: Vec<String>,
    pub patterns: Vec<CrfPattern>,
    pub u_offsets: Vec<Option<usize>>,
    pub b_offsets: Vec<Option<usize>>,
    pub weights: BTreeMap<usize, f64>,
    pub nftr: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CrfModelError {
    #[error("malformed model header")]
    MalformedHeader,
    #[error("bad netstring")]
    BadNetstring,
    #[error("truncated model")]
    Truncated,
    #[error("unknown pattern type: {0}")]
    UnknownPatternType(char),
    #[error("model count mismatch")]
    CountMismatch,
    #[error("weight index out of range: {index} >= {nftr}")]
    IndexOutOfRange { index: usize, nftr: usize },
    #[error("weight is not finite")]
    NonFiniteWeight,
    #[error("integer overflow while reading model")]
    Overflow,
    #[error("model resource limit exceeded: {resource} ({actual} > {maximum})")]
    LimitsExceeded {
        resource: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("malformed weight")]
    MalformedWeight,
}

impl CrfModel {
    pub fn parse(bytes: &[u8]) -> Result<Self, CrfModelError> {
        if bytes.len() > MAX_CRF_MODEL_BYTES {
            return Err(CrfModelError::LimitsExceeded {
                resource: "bytes",
                actual: bytes.len(),
                maximum: MAX_CRF_MODEL_BYTES,
            });
        }
        let mut input = Cursor::new(bytes);
        let header = input.line()?;
        let (model_type, nact) = parse_header(header)?;
        let reader = input.line()?;
        let (npat, _ntoks, _autouni) = parse_reader_header(reader)?;
        limit(npat, MAX_CRF_PATTERNS, "patterns")?;
        let mut patterns = Vec::with_capacity(npat);
        for _ in 0..npat {
            let source = input.netstring()?;
            let kind = source
                .chars()
                .next()
                .ok_or(CrfModelError::UnknownPatternType('\0'))?;
            if !matches!(kind, 'u' | 'b' | '*') {
                return Err(CrfModelError::UnknownPatternType(kind));
            }
            patterns.push(CrfPattern { source, kind });
        }
        let label_header = input.line()?;
        let nlabels = parse_quark_header(label_header)?;
        limit(nlabels, MAX_CRF_LABELS, "labels")?;
        let mut labels = Vec::with_capacity(nlabels);
        for _ in 0..nlabels {
            labels.push(input.netstring()?);
        }
        let obs_header = input.line()?;
        let nobs = parse_quark_header(obs_header)?;
        limit(nobs, MAX_CRF_OBSERVATIONS, "observations")?;
        let mut observations = Vec::with_capacity(nobs);
        for _ in 0..nobs {
            observations.push(input.netstring()?);
        }
        if nlabels == 0 {
            return Err(CrfModelError::CountMismatch);
        }

        let mut u_offsets = Vec::with_capacity(nobs);
        let mut b_offsets = Vec::with_capacity(nobs);
        let mut nftr = 0usize;
        for observation in &observations {
            let kind = observation
                .chars()
                .next()
                .ok_or(CrfModelError::CountMismatch)?;
            let u = matches!(kind, 'u' | '*');
            let b = matches!(kind, 'b' | '*');
            if !u && !b {
                return Err(CrfModelError::UnknownPatternType(kind));
            }
            let uoff = if u {
                let at = nftr;
                nftr = nftr.checked_add(nlabels).ok_or(CrfModelError::Overflow)?;
                Some(at)
            } else {
                None
            };
            let boff = if b {
                let size = nlabels
                    .checked_mul(nlabels)
                    .ok_or(CrfModelError::Overflow)?;
                let at = nftr;
                nftr = nftr.checked_add(size).ok_or(CrfModelError::Overflow)?;
                Some(at)
            } else {
                None
            };
            u_offsets.push(uoff);
            b_offsets.push(boff);
        }
        let mut weights = BTreeMap::new();
        for _ in 0..nact {
            let line = input.line()?;
            let (index, value) = parse_weight(line)?;
            if index >= nftr {
                return Err(CrfModelError::IndexOutOfRange { index, nftr });
            }
            if !value.is_finite() {
                return Err(CrfModelError::NonFiniteWeight);
            }
            weights.insert(index, value);
        }
        if !input.remaining() {
            return Err(CrfModelError::CountMismatch);
        }
        Ok(Self {
            model_type,
            labels,
            observations,
            patterns,
            u_offsets,
            b_offsets,
            weights,
            nftr,
        })
    }
}

fn limit(actual: usize, maximum: usize, resource: &'static str) -> Result<(), CrfModelError> {
    if actual > maximum {
        Err(CrfModelError::LimitsExceeded {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn parse_header(line: &[u8]) -> Result<(u32, usize), CrfModelError> {
    let s = std::str::from_utf8(line).map_err(|_| CrfModelError::MalformedHeader)?;
    let mut p = s.split('#');
    if p.next() != Some("") || p.next() != Some("mdl") {
        return Err(CrfModelError::MalformedHeader);
    }
    let ty = p
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or(CrfModelError::MalformedHeader)?;
    let nact = p
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or(CrfModelError::MalformedHeader)?;
    if p.next().is_some() || ty != 2 {
        return Err(CrfModelError::MalformedHeader);
    }
    limit(nact, MAX_CRF_WEIGHTS, "weights")?;
    Ok((ty, nact))
}

fn parse_reader_header(line: &[u8]) -> Result<(usize, usize, usize), CrfModelError> {
    let s = std::str::from_utf8(line).map_err(|_| CrfModelError::MalformedHeader)?;
    let body = s
        .strip_prefix("#rdr#")
        .ok_or(CrfModelError::MalformedHeader)?;
    let mut p = body.split('/');
    let a = p
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or(CrfModelError::MalformedHeader)?;
    let b = p
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or(CrfModelError::MalformedHeader)?;
    let c = p.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    if p.next().is_some() {
        return Err(CrfModelError::MalformedHeader);
    }
    Ok((a, b, c))
}
fn parse_quark_header(line: &[u8]) -> Result<usize, CrfModelError> {
    let s = std::str::from_utf8(line).map_err(|_| CrfModelError::MalformedHeader)?;
    s.strip_prefix("#qrk#")
        .and_then(|v| v.parse().ok())
        .ok_or(CrfModelError::MalformedHeader)
}
fn parse_weight(line: &[u8]) -> Result<(usize, f64), CrfModelError> {
    let equals = line
        .iter()
        .position(|byte| *byte == b'=')
        .ok_or(CrfModelError::MalformedWeight)?;
    let index = std::str::from_utf8(&line[..equals])
        .ok()
        .and_then(|v| v.parse().ok())
        .ok_or(CrfModelError::MalformedWeight)?;
    Ok((index, parse_hex_float(&line[equals + 1..])?))
}

fn parse_hex_float(bytes: &[u8]) -> Result<f64, CrfModelError> {
    let s = std::str::from_utf8(bytes).map_err(|_| CrfModelError::MalformedWeight)?;
    let (negative, body) = s.strip_prefix('-').map_or((false, s), |v| (true, v));
    let body = body
        .strip_prefix("0x")
        .or_else(|| body.strip_prefix("0X"))
        .ok_or(CrfModelError::MalformedWeight)?;
    let exponent_at = body
        .bytes()
        .position(|byte| byte == b'p' || byte == b'P')
        .ok_or(CrfModelError::MalformedWeight)?;
    let (mantissa, exponent) = body.split_at(exponent_at);
    let exponent = &exponent[1..];
    let exponent: i32 = exponent
        .parse()
        .map_err(|_| CrfModelError::MalformedWeight)?;
    let (whole, fraction) = mantissa
        .split_once('.')
        .map_or((mantissa, ""), |(a, b)| (a, b));
    if whole.is_empty() && fraction.is_empty() {
        return Err(CrfModelError::MalformedWeight);
    }
    let mut value = 0.0f64;
    for c in whole.bytes().chain(fraction.bytes()) {
        let d = char::from(c)
            .to_digit(16)
            .ok_or(CrfModelError::MalformedWeight)?;
        value = value * 16.0 + d as f64;
    }
    let shift = exponent
        .checked_sub(
            (fraction.len() as i32)
                .checked_mul(4)
                .ok_or(CrfModelError::Overflow)?,
        )
        .ok_or(CrfModelError::Overflow)?;
    value *= 2f64.powi(shift);
    if negative {
        value = -value;
    }
    if !value.is_finite() {
        return Err(CrfModelError::NonFiniteWeight);
    }
    Ok(value)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn line(&mut self) -> Result<&'a [u8], CrfModelError> {
        let start = self.at;
        let end = self.bytes[start..]
            .iter()
            .position(|b| *b == b'\n')
            .ok_or(CrfModelError::Truncated)?
            + start;
        self.at = end + 1;
        let line = &self.bytes[start..end];
        if line.contains(&b'\r') {
            return Err(CrfModelError::BadNetstring);
        }
        Ok(line)
    }
    fn netstring(&mut self) -> Result<String, CrfModelError> {
        let start = self.at;
        let colon = self.bytes[start..]
            .iter()
            .position(|b| *b == b':')
            .ok_or(CrfModelError::BadNetstring)?
            + start;
        let len: usize = std::str::from_utf8(&self.bytes[start..colon])
            .ok()
            .and_then(|v| v.parse().ok())
            .ok_or(CrfModelError::BadNetstring)?;
        let payload_start = colon + 1;
        let payload_end = payload_start
            .checked_add(len)
            .ok_or(CrfModelError::Overflow)?;
        if payload_end + 2 > self.bytes.len() {
            return Err(CrfModelError::Truncated);
        }
        if self.bytes[payload_end] != b','
            || self.bytes[payload_end + 1] != b'\n'
            || self.bytes[payload_start..payload_end].contains(&b'\r')
        {
            return Err(CrfModelError::BadNetstring);
        }
        self.at = payload_end + 2;
        String::from_utf8(self.bytes[payload_start..payload_end].to_vec())
            .map_err(|_| CrfModelError::BadNetstring)
    }
    fn remaining(&self) -> bool {
        self.at == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(weights: &str) -> Vec<u8> {
        format!(
            "#mdl#2#{}\n#rdr#1/0\n7:u00:foo,\n#qrk#2\n1:A,\n1:B,\n#qrk#1\n5:u:foo,\n{}",
            weights.lines().count(),
            weights
        )
        .into_bytes()
    }
    #[test]
    fn reads_netstrings_and_sparse_offsets() {
        let m = CrfModel::parse(&model("0=0x1p+0\n")).unwrap();
        assert_eq!(m.labels, ["A", "B"]);
        assert_eq!(m.nftr, 2);
        assert_eq!(m.weights[&0], 1.0);
    }
    #[test]
    fn parses_c99_hex_float() {
        let m = CrfModel::parse(&model("0=-0x1.8p+1\n")).unwrap();
        assert_eq!(m.weights[&0], -3.0);
    }
    #[test]
    fn rejects_bad_netstring_crlf_and_out_of_range() {
        let mut bytes = model("");
        let first_newline = bytes.iter().position(|b| *b == b'\n').unwrap();
        bytes[first_newline] = b'\r';
        assert!(matches!(
            CrfModel::parse(&bytes),
            Err(CrfModelError::BadNetstring)
        ));
        assert!(matches!(
            CrfModel::parse(&model("2=0x1p+0\n")),
            Err(CrfModelError::IndexOutOfRange { .. })
        ));
    }
    #[test]
    fn rejects_unknown_pattern_and_weight_limit() {
        let mut bytes = model("");
        let needle = b"u00:foo";
        let at = bytes
            .windows(needle.len())
            .position(|w| w == needle)
            .unwrap();
        bytes[at] = b'x';
        assert!(matches!(
            CrfModel::parse(&bytes),
            Err(CrfModelError::UnknownPatternType('x'))
        ));
        let huge = format!(
            "#mdl#2#{}\n#rdr#1/0\n7:u00:foo,\n#qrk#2\n1:A,\n1:B,\n#qrk#1\n5:u:foo,\n",
            MAX_CRF_WEIGHTS + 1
        );
        assert!(matches!(
            CrfModel::parse(huge.as_bytes()),
            Err(CrfModelError::LimitsExceeded {
                resource: "weights",
                ..
            })
        ));
    }
}
