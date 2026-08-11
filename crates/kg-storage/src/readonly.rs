//! Adapter-neutral fail-closed Cypher policy for `GraphStore::query_readonly`.

use crate::{GraphResult, GraphStoreError};

pub fn validate_readonly_cypher(cypher: &str) -> GraphResult<()> {
    let tokens = cypher_tokens(cypher)?;
    const FORBIDDEN: &[&str] = &[
        "CREATE", "MERGE", "DELETE", "DETACH", "SET", "REMOVE", "DROP", "CALL", "LOAD", "FOREACH",
        "USE",
    ];
    if let Some(token) = tokens
        .iter()
        .find(|token| FORBIDDEN.contains(&token.as_str()))
    {
        return Err(GraphStoreError::Query(format!(
            "read-only query rejected mutating or administrative Cypher token {token}"
        )));
    }
    if !tokens
        .iter()
        .any(|token| matches!(token.as_str(), "MATCH" | "RETURN" | "WITH" | "UNWIND"))
    {
        return Err(GraphStoreError::Query(
            "read-only query requires an explicit read clause".into(),
        ));
    }
    Ok(())
}

fn cypher_tokens(cypher: &str) -> GraphResult<Vec<String>> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum State {
        Code,
        Single,
        Double,
        Backtick,
        LineComment,
        BlockComment,
    }
    let chars = cypher.chars().collect::<Vec<_>>();
    let mut state = State::Code;
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        let next = chars.get(index + 1).copied();
        match state {
            State::Code => match (ch, next) {
                ('/', Some('/')) => {
                    finish_token(&mut current, &mut tokens);
                    state = State::LineComment;
                    index += 1;
                }
                ('/', Some('*')) => {
                    finish_token(&mut current, &mut tokens);
                    state = State::BlockComment;
                    index += 1;
                }
                ('\'', _) => {
                    finish_token(&mut current, &mut tokens);
                    state = State::Single;
                }
                ('"', _) => {
                    finish_token(&mut current, &mut tokens);
                    state = State::Double;
                }
                ('`', _) => {
                    finish_token(&mut current, &mut tokens);
                    state = State::Backtick;
                }
                _ if ch.is_ascii_alphanumeric() || ch == '_' => {
                    current.push(ch.to_ascii_uppercase())
                }
                _ => finish_token(&mut current, &mut tokens),
            },
            State::Single => {
                if ch == '\\' {
                    index += usize::from(next.is_some());
                } else if ch == '\'' {
                    state = State::Code;
                }
            }
            State::Double => {
                if ch == '\\' {
                    index += usize::from(next.is_some());
                } else if ch == '"' {
                    state = State::Code;
                }
            }
            State::Backtick => {
                if ch == '`' {
                    state = State::Code;
                }
            }
            State::LineComment => {
                if ch == '\n' {
                    state = State::Code;
                }
            }
            State::BlockComment => {
                if ch == '*' && next == Some('/') {
                    state = State::Code;
                    index += 1;
                }
            }
        }
        index += 1;
    }
    finish_token(&mut current, &mut tokens);
    if !matches!(state, State::Code | State::LineComment) {
        return Err(GraphStoreError::Query(
            "read-only query rejected unterminated quote or block comment".into(),
        ));
    }
    Ok(tokens)
}

fn finish_token(current: &mut String, tokens: &mut Vec<String>) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_current_read_corpus_and_quoted_mutation_words() {
        for query in [
            "MATCH (n) RETURN n",
            "RETURN 1",
            "WITH 1 AS x RETURN x",
            "UNWIND [1,2] AS x RETURN x",
            "MATCH (n) WHERE n.text = 'CREATE SET DELETE' RETURN n.text",
            "MATCH (n) RETURN n.`set` // DELETE",
            "/* CREATE */ MATCH (n) RETURN count(n)",
        ] {
            assert!(validate_readonly_cypher(query).is_ok(), "rejected {query}");
        }
    }

    #[test]
    fn rejects_mutations_admin_and_unterminated_input() {
        for query in [
            "CREATE (n)",
            "MATCH (n) SET n.x = 1 RETURN n",
            "MATCH (n) DETACH DELETE n",
            "MERGE (n:Paper {id: 'x'}) RETURN n",
            "CALL db.labels()",
            "LOAD CSV FROM 'x' AS row RETURN row",
            "MATCH (n) REMOVE n.x RETURN n",
            "MATCH (n) RETURN n /*",
        ] {
            assert!(validate_readonly_cypher(query).is_err(), "accepted {query}");
        }
    }
}
