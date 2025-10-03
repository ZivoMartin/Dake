use crate::lexer::{
    directive::DIRECTIVE_PREFIX,
    target_label::TargetLabel,
    tokens::{Line, Token},
};
use anyhow::{Context, Result};
use log::warn;
use std::{fs::File, io::Read, path::Path};

const DEFAULT_PATH_CANDIDATES: [&str; 3] = ["Makefile", "makefile", "GNUMakefile"];
const NO_MAKEFILE_FOUND: &str = "dake: *** No targets specified and no makefile found.  Stop.";

pub type LexingOutput = Vec<Token>;

pub fn lex(s: String) -> Result<LexingOutput> {
    const FORBIDDEN_RIGHT_PREFIX: [&str; 1] = ["="];

    fn generate_lines(s: &str) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut lines_iter = s.lines();

        while let Some(line) = lines_iter.next() {
            if line.starts_with(DIRECTIVE_PREFIX) {
                lines.push(Line::Directive(line[2..].to_string()));
                continue;
            }

            let (mut line, _) = line
                .split_once('#')
                .map(|(l, _)| (l.to_string(), ()))
                .unwrap_or((line.to_string(), ()));

            fn push_line(lines: &mut Vec<Line>, line: &str) {
                if line.is_empty() {
                    return;
                }
                let line = match line.split_once(':') {
                    Some((left, right)) => {
                        if FORBIDDEN_RIGHT_PREFIX.iter().any(|s| right.starts_with(s)) {
                            Line::RawLine(format!("{line}\n"))
                        } else {
                            Line::ColonLine(left.to_string(), format!("{right}\n"))
                        }
                    }
                    None => Line::RawLine(format!("{line}\n")),
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

    fn lines_to_tokens(lines: Vec<Line>) -> Result<Vec<Token>> {
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

                    while let Some(open) = rest.find('[') {
                        if let Some(close) = rest[open..].find(']') {
                            let prefix = rest[..open].trim();
                            let inside = &rest[open + 1..open + close].trim();

                            if !prefix.is_empty() {
                                left_parsed =
                                    Some((inside.to_string(), prefix.parse::<TargetLabel>()?));
                            }

                            rest = &rest[open + close + 1..];
                        } else {
                            warn!("Unmatched parenthesis in target: {}", left);
                            break;
                        }
                    }

                    let token = match left_parsed {
                        Some((target, label)) => Token::Target {
                            target: target.to_string(),
                            label: Some(label),
                            command: right,
                        },
                        None => Token::Target {
                            target: left.to_string(),
                            label: None,
                            command: right,
                        },
                    };
                    tokens.push(token);
                }
                Some(Line::RawLine(line)) => {
                    warn!("Unexpected RawLine after processing: {}", line);
                }
                Some(Line::Directive(dir)) => tokens.push(Token::Directive(dir.parse()?)),
                None => break,
            }
        }
        Ok(tokens)
    }

    let lines = generate_lines(&s);
    let tokens = lines_to_tokens(lines.clone())?;
    Ok(tokens)
}

pub fn lex_from_path(path: &str) -> Result<LexingOutput> {
    let mut f = File::open(&path).context(format!("When opening the file {path}."))?;
    let mut content = String::new();
    f.read_to_string(&mut content)
        .context(format!("When reading the file {path}."))?;
    lex(content)
}

pub fn guess_path_and_lex() -> Result<LexingOutput> {
    let path = DEFAULT_PATH_CANDIDATES
        .iter()
        .find(|path| Path::new(path).try_exists().unwrap_or(false))
        .context(NO_MAKEFILE_FOUND)?;
    lex_from_path(path)
}
