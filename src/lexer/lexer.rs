use crate::lexer::{
    tokens::{Line, Token},
    Makefile,
};
use anyhow::{Context, Result};
use log::warn;
use std::{fs::File, io::Read, path::Path};

const DEFAULT_PATH_CANDIDATES: [&str; 3] = ["Makefile", "makefile", "GNUMakefile"];
const NO_MAKEFILE_FOUND: &str = "dake: *** No targets specified and no makefile found.  Stop.";

pub fn lex(s: String) -> Result<Makefile> {
    fn generate_lines(s: &str) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut lines_iter = s.lines();

        while let Some(line) = lines_iter.next() {
            let (mut line, _) = line
                .split_once('#')
                .map(|(l, _)| (l.to_string(), ()))
                .unwrap_or((line.to_string(), ()));

            fn push_line(lines: &mut Vec<Line>, line: &str) {
                let line = line.trim();
                if line.is_empty() {
                    return;
                }
                let line = match line.split_once(':') {
                    Some((left, right)) => Line::ColonLine(left.to_string(), right.to_string()),
                    None => Line::RawLine(line.to_string()),
                };
                lines.push(line);
            }

            // Handle continuations with "\"
            while line.ends_with('\\') {
                if let Some(next_line) = lines_iter.next() {
                    line.pop(); // remove the backslash
                    line.push_str(next_line);
                } else {
                    push_line(&mut lines, &line);
                    break;
                }
            }

            push_line(&mut lines, &line);
        }
        lines
    }

    fn lines_to_tokens(lines: Vec<Line>) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut lines_iter = lines.into_iter().peekable();

        loop {
            // Gather consecutive RawLines as one RawText
            let mut dummy_text = String::new();
            while let Some(Line::RawLine(line)) = lines_iter.peek() {
                dummy_text.push_str(line);
                lines_iter.next();
            }
            if !dummy_text.is_empty() {
                tokens.push(Token::RawText(dummy_text));
            }

            match lines_iter.next() {
                Some(Line::ColonLine(left, mut right)) => {
                    // Peek next line for inline continuation
                    if let Some(Line::RawLine(extra)) = lines_iter.peek() {
                        right.push_str(extra);
                        lines_iter.next();
                    }

                    let mut rest = left.trim();
                    let mut left_parsed = None;

                    while let Some(open) = rest.find('(') {
                        if let Some(close) = rest[open..].find(')') {
                            let prefix = rest[..open].trim();
                            let inside = &rest[open + 1..open + close].trim();

                            if !prefix.is_empty() {
                                left_parsed = Some((inside.to_string(), prefix.to_string()));
                            }

                            rest = &rest[open + close + 1..];
                        } else {
                            warn!("Unmatched parenthesis in target: {}", left);
                            break;
                        }
                    }

                    let token = match left_parsed {
                        Some((ip, name)) => Token::Target {
                            name: name.to_string(),
                            ip: Some(ip),
                            command: right,
                        },
                        None => Token::Target {
                            name: left.to_string(),
                            ip: None,
                            command: right,
                        },
                    };
                    tokens.push(token);
                }
                Some(Line::RawLine(line)) => {
                    warn!("Unexpected RawLine after processing: {}", line);
                }
                None => break,
            }
        }
        tokens
    }

    let lines = generate_lines(&s);
    let tokens = lines_to_tokens(lines.clone());
    Ok(Makefile::new(s, lines, tokens))
}

pub fn lex_from_path(path: String) -> Result<Makefile> {
    let mut f = File::open(&path).context(format!("When opening the file {path}."))?;
    let mut content = String::new();
    f.read_to_string(&mut content)
        .context(format!("When reading the file {path}."))?;
    lex(content)
}

pub fn guess_path_and_lex() -> Result<Makefile> {
    let path = DEFAULT_PATH_CANDIDATES
        .iter()
        .find(|path| Path::new(path).try_exists().unwrap_or(false))
        .context(NO_MAKEFILE_FOUND)?;
    lex_from_path(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokens::{Line, Token};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_empty_string() {
        let mf = lex(String::new()).unwrap();
        assert!(mf.tokens().is_empty());
        assert!(mf.lines().is_empty());
    }

    #[test]
    fn test_comment_only() {
        let mf = lex("# just a comment".into()).unwrap();
        assert!(mf.tokens().is_empty());
        assert!(mf.lines().is_empty());
    }

    #[test]
    fn test_raw_line() {
        let mf = lex("hello world".into()).unwrap();
        assert_eq!(mf.lines(), &[Line::RawLine("hello world".into())]);
        assert_eq!(mf.tokens(), &[Token::RawText("hello world".into())]);
    }

    #[test]
    fn test_simple_target() {
        let mf = lex("foo: bar".into()).unwrap();
        assert_eq!(mf.lines(), &[Line::ColonLine("foo".into(), " bar".into())]);
        match &mf.tokens()[0] {
            Token::Target { name, ip, command } => {
                assert_eq!(name, "foo");
                assert!(ip.is_none());
                assert_eq!(command, " bar");
            }
            _ => panic!("Expected target token"),
        }
    }

    #[test]
    fn test_line_continuation() {
        let mf = lex("foo: bar \\\n baz".into()).unwrap();
        match &mf.tokens()[0] {
            Token::Target { name, command, .. } => {
                assert_eq!(name, "foo");
                assert!(command.contains("bar"));
                assert!(command.contains("baz"));
            }
            _ => panic!("Expected target token"),
        }
    }

    #[test]
    fn test_parentheses_ip_parsing() {
        let mf = lex("foo(ip1): echo hi".into()).unwrap();
        match &mf.tokens()[0] {
            Token::Target { name, ip, command } => {
                assert_eq!(name, "foo");
                assert_eq!(ip.as_deref(), Some("ip1"));
                assert_eq!(command, " echo hi");
            }
            _ => panic!("Expected target token"),
        }
    }

    #[test]
    fn test_unmatched_parenthesis_logs_error() {
        // Should not panic, but should still produce a Token
        let mf = lex("foo(bar: baz".into()).unwrap();
        assert_eq!(mf.tokens().len(), 1);
    }

    #[test]
    fn test_multiple_tokens() {
        let input = r#"
hello
foo: bar
world
"#;
        let mf = lex(input.into()).unwrap();
        assert!(mf.tokens().iter().any(|t| matches!(t, Token::RawText(_))));
        assert!(mf
            .tokens()
            .iter()
            .any(|t| matches!(t, Token::Target { .. })));
    }

    #[test]
    fn test_guess_path_and_lex() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Makefile");
        fs::write(&path, "foo: bar").unwrap();

        // Temporarily change CWD
        let old_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let mf = guess_path_and_lex().unwrap();
        assert_eq!(mf.tokens().len(), 1);

        // Restore
        std::env::set_current_dir(old_cwd).unwrap();
    }
}
