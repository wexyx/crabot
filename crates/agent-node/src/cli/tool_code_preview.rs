use serde_json::{Map, Value};

/// A conservative display-only scan of literal tools.name({...}) calls. Never
/// evaluates code or guesses the values of variables, templates or expressions.
pub(super) fn call(code: &str) -> Option<(String, Value)> {
    if code.len() > 65536 {
        return None;
    }
    let tokens = tokens(code);
    for (at, window) in tokens.windows(6).enumerate() {
        if window[0] != Token::Word("tools".into())
            || window[1] != Token::Mark('.')
            || window[3] != Token::Mark('(')
            || window[4] != Token::Mark('{')
        {
            continue;
        }
        let Token::Word(name) = &window[2] else {
            continue;
        };
        let name = name.strip_prefix("crabot_tool_").unwrap_or(name);
        if name.is_empty() || name.len() > 64 {
            continue;
        }
        let mut input = Map::new();
        let mut depth = 1;
        for i in at + 5..tokens.len() {
            match &tokens[i] {
                Token::Mark('{') | Token::Mark('[') | Token::Mark('(') => depth += 1,
                Token::Mark('}') | Token::Mark(']') | Token::Mark(')') => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Token::Word(key) | Token::Text(key) if depth == 1 => {
                    if !["target", "query", "id", "name", "command", "cmd", "action"]
                        .contains(&key.as_str())
                    {
                        continue;
                    }
                    if tokens.get(i + 1) != Some(&Token::Mark(':')) {
                        continue;
                    }
                    if let (Some(Token::Text(text)), Some(Token::Mark(',' | '}'))) =
                        (tokens.get(i + 2), tokens.get(i + 3))
                    {
                        input.insert(key.clone(), Value::String(text.clone()));
                    }
                }
                _ => (),
            }
        }
        return Some((name.into(), Value::Object(input)));
    }
    None
}

#[derive(PartialEq)]
enum Token {
    Word(String),
    Text(String),
    Mark(char),
}

fn tokens(code: &str) -> Vec<Token> {
    let mut chars = code.chars().peekable();
    let mut result = vec![];
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            for c in chars.by_ref() {
                if c == '\n' {
                    break;
                }
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(c) = chars.next() {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else if matches!(c, '\'' | '"' | '`') {
            let mut text = String::new();
            let mut valid = c != '`';
            let mut closed = false;
            while let Some(next) = chars.next() {
                if next == c {
                    closed = true;
                    break;
                }
                if next == '\\' {
                    match chars.next() {
                        Some('n') => text.push('\n'),
                        Some('r') => text.push('\r'),
                        Some('t') => text.push('\t'),
                        Some(c @ ('\\' | '\'' | '"' | '/')) => text.push(c),
                        _ => valid = false,
                    }
                } else {
                    text.push(next);
                }
            }
            result.push(if valid && closed {
                Token::Text(text)
            } else {
                Token::Mark('?')
            });
        } else if c.is_ascii_alphabetic() || matches!(c, '_' | '$') {
            let mut word = String::from(c);
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$'))
            {
                word.push(chars.next().unwrap());
            }
            result.push(Token::Word(word));
        } else {
            result.push(Token::Mark(c));
        }
    }
    result
}
