//! Shared bounded RESP2 wire codec for the read-only graph plane.
//!
//! Both the remote client ([`crate::remote`]) and the read-only server
//! ([`crate::read_only_server`]) speak RESP2 over TCP. This module is the
//! single codec: an incremental frame parser, a client command encoder, and
//! response encoders. Parsing never allocates beyond what the caller feeds
//! it; callers enforce frame-size limits on the buffers they pass to
//! [`parse_resp`] (the server applies `max_frame_bytes`, the client applies
//! `max_response_bytes`).

/// A RESP2 value as decoded from the wire or built for encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RespValue {
    Simple(String),
    Error(String),
    Integer(i64),
    Bulk(String),
    Null,
    Array(Vec<RespValue>),
}

impl RespValue {
    /// Text accessor for server-side dispatch: Bulk and Simple frames carry
    /// command names and string arguments; every other frame kind is not
    /// text.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            RespValue::Simple(text) | RespValue::Bulk(text) => Some(text),
            _ => None,
        }
    }
}

/// Encode a client command as an array of bulk strings (RESP request shape):
/// `*N\r\n$len\r\narg\r\n...`.
pub fn encode_command(args: &[&str]) -> Vec<u8> {
    let mut output = format!("*{}\r\n", args.len()).into_bytes();
    for argument in args {
        output.extend_from_slice(format!("${}\r\n", argument.len()).as_bytes());
        output.extend_from_slice(argument.as_bytes());
        output.extend_from_slice(b"\r\n");
    }
    output
}

/// Incrementally parse exactly one RESP frame from `input`.
///
/// - `Ok(Some((value, consumed)))` — one complete frame ending at `consumed`.
/// - `Ok(None)` — the frame is incomplete; feed more bytes and retry.
/// - `Err(message)` — the bytes so far can never form a valid frame.
pub fn parse_resp(input: &[u8]) -> Result<Option<(RespValue, usize)>, String> {
    parse_at(input, 0)
}

fn parse_at(input: &[u8], position: usize) -> Result<Option<(RespValue, usize)>, String> {
    let Some(prefix) = input.get(position).copied() else {
        return Ok(None);
    };
    match prefix {
        b'+' | b'-' | b':' => {
            let Some((line, next)) = read_line(input, position + 1) else {
                return Ok(None);
            };
            let text = std::str::from_utf8(line)
                .map_err(|error| format!("RESP frame is not UTF-8: {error}"))?;
            let value = match prefix {
                b'+' => RespValue::Simple(text.into()),
                b'-' => RespValue::Error(text.into()),
                b':' => RespValue::Integer(
                    text.parse()
                        .map_err(|error| format!("RESP frame invalid integer {text:?}: {error}"))?,
                ),
                _ => unreachable!(),
            };
            Ok(Some((value, next)))
        }
        b'$' => {
            let Some((line, mut next)) = read_line(input, position + 1) else {
                return Ok(None);
            };
            let length: i64 = std::str::from_utf8(line)
                .map_err(|error| format!("RESP frame bulk length UTF-8: {error}"))?
                .parse()
                .map_err(|error| format!("RESP frame invalid bulk length: {error}"))?;
            if length == -1 {
                return Ok(Some((RespValue::Null, next)));
            }
            if length < -1 {
                return Err(format!("RESP frame invalid bulk length {length}"));
            }
            let length = length as usize;
            let Some(end) = next.checked_add(length) else {
                return Err("RESP frame bulk length overflow".into());
            };
            if input.len() < end + 2 {
                return Ok(None);
            }
            if &input[end..end + 2] != b"\r\n" {
                return Err("RESP frame bulk missing CRLF".into());
            }
            let value = std::str::from_utf8(&input[next..end])
                .map_err(|error| format!("RESP frame bulk is not UTF-8: {error}"))?;
            next = end + 2;
            Ok(Some((RespValue::Bulk(value.into()), next)))
        }
        b'*' => {
            let Some((line, mut next)) = read_line(input, position + 1) else {
                return Ok(None);
            };
            let length: usize = std::str::from_utf8(line)
                .map_err(|error| format!("RESP frame array length UTF-8: {error}"))?
                .parse()
                .map_err(|error| format!("RESP frame invalid array length: {error}"))?;
            let mut values = Vec::with_capacity(length);
            for _ in 0..length {
                let Some((value, after)) = parse_at(input, next)? else {
                    return Ok(None);
                };
                values.push(value);
                next = after;
            }
            Ok(Some((RespValue::Array(values), next)))
        }
        b'_' => {
            if input.get(position..position + 3) == Some(b"_\r\n") {
                Ok(Some((RespValue::Null, position + 3)))
            } else if input.len() < position + 3 {
                Ok(None)
            } else {
                Err("RESP frame invalid null frame".into())
            }
        }
        other => Err(format!("RESP frame unsupported prefix 0x{other:02x}")),
    }
}

fn read_line(input: &[u8], start: usize) -> Option<(&[u8], usize)> {
    input
        .get(start..)?
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|offset| (&input[start..start + offset], start + offset + 2))
}

/// `+text\r\n`
pub fn encode_simple(text: &str) -> Vec<u8> {
    let mut output = Vec::with_capacity(text.len() + 3);
    output.push(b'+');
    output.extend_from_slice(text.as_bytes());
    output.extend_from_slice(b"\r\n");
    output
}

/// `-text\r\n`
pub fn encode_error(text: &str) -> Vec<u8> {
    let mut output = Vec::with_capacity(text.len() + 3);
    output.push(b'-');
    output.extend_from_slice(text.as_bytes());
    output.extend_from_slice(b"\r\n");
    output
}

/// `:number\r\n`
pub fn encode_integer(value: i64) -> Vec<u8> {
    format!(":{value}\r\n").into_bytes()
}

/// `$len\r\ntext\r\n`
pub fn encode_bulk(text: &str) -> Vec<u8> {
    let mut output = Vec::with_capacity(text.len() + 16);
    output.extend_from_slice(format!("${}\r\n", text.len()).as_bytes());
    output.extend_from_slice(text.as_bytes());
    output.extend_from_slice(b"\r\n");
    output
}

/// `_\r\n` — RESP3 null frame (the parser also accepts the RESP2 `$-1`).
pub fn encode_null() -> Vec<u8> {
    b"_\r\n".to_vec()
}

/// `*count\r\n` followed by the concatenated encoded children.
pub fn encode_array(items: &[Vec<u8>]) -> Vec<u8> {
    let mut output = Vec::with_capacity(items.iter().map(Vec::len).sum::<usize>() + 16);
    output.extend_from_slice(format!("*{}\r\n", items.len()).as_bytes());
    for item in items {
        output.extend_from_slice(item);
    }
    output
}

/// Encode any [`RespValue`] back to wire bytes.
pub fn encode_value(value: &RespValue) -> Vec<u8> {
    match value {
        RespValue::Simple(text) => encode_simple(text),
        RespValue::Error(text) => encode_error(text),
        RespValue::Integer(number) => encode_integer(*number),
        RespValue::Bulk(text) => encode_bulk(text),
        RespValue::Null => encode_null(),
        RespValue::Array(items) => {
            encode_array(&items.iter().map(encode_value).collect::<Vec<_>>())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_is_incremental_and_maps_wire_types_without_guessing() {
        let frame = b"*3\r\n*3\r\n$1\r\ns\r\n$1\r\ni\r\n$1\r\nn\r\n*3\r\n$4\r\ntrue\r\n:7\r\n_\r\n*3\r\n$3\r\n1.5\r\n:-2\r\n$-1\r\n";
        assert!(parse_resp(&frame[..10]).unwrap().is_none());
        let (value, consumed) = parse_resp(frame).unwrap().unwrap();
        assert_eq!(consumed, frame.len());
        let RespValue::Array(rows) = value else {
            panic!("expected outer array");
        };
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn command_encode_round_trips_through_parser() {
        let wire = encode_command(&["GRAPH.RO_QUERY", "g", "MATCH (n) RETURN n"]);
        let (value, consumed) = parse_resp(&wire).unwrap().unwrap();
        assert_eq!(consumed, wire.len());
        let RespValue::Array(args) = value else {
            panic!("expected array");
        };
        let texts: Vec<&str> = args.iter().map(|arg| arg.as_str().unwrap()).collect();
        assert_eq!(texts, ["GRAPH.RO_QUERY", "g", "MATCH (n) RETURN n"]);
    }

    #[test]
    fn response_encoders_round_trip() {
        for value in [
            RespValue::Simple("PONG".into()),
            RespValue::Error("ERR nope".into()),
            RespValue::Integer(-7),
            RespValue::Bulk("hello".into()),
            RespValue::Null,
            RespValue::Array(vec![RespValue::Bulk("a".into()), RespValue::Integer(1)]),
        ] {
            let wire = encode_value(&value);
            let (decoded, consumed) = parse_resp(&wire).unwrap().unwrap();
            assert_eq!(consumed, wire.len());
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn malformed_frames_fail_closed() {
        assert!(parse_resp(b"$3\r\nabcXX").is_err());
        assert!(parse_resp(b":not-a-number\r\n").is_err());
        assert!(parse_resp(b"!boom\r\n").is_err());
        assert!(parse_resp(b"*x\r\n").is_err());
        // Incomplete frames ask for more bytes instead of erroring.
        assert!(parse_resp(b"$5\r\nab").unwrap().is_none());
        assert!(parse_resp(b"*2\r\n$1\r\na").unwrap().is_none());
    }

    #[test]
    fn trailing_bytes_are_detected_via_consumed_length() {
        let wire = encode_command(&["PING"]);
        let mut padded = wire.clone();
        padded.extend_from_slice(b"GARBAGE");
        let (_, consumed) = parse_resp(&padded).unwrap().unwrap();
        assert_eq!(consumed, wire.len());
        assert_ne!(consumed, padded.len());
    }

    #[test]
    fn null_accepts_both_wire_forms() {
        assert_eq!(parse_resp(b"_\r\n").unwrap().unwrap().0, RespValue::Null);
        assert_eq!(parse_resp(b"$-1\r\n").unwrap().unwrap().0, RespValue::Null);
    }
}
