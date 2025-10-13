//! # Lexer
//!
//! This module is responsible for lexing Makefiles into a structured token
//! representation (`Token`).  
//!
//! Responsibilities:
//! - Find and read a Makefile from disk (default candidates: `Makefile`, `makefile`, `GNUMakefile`).
//! - Process Makefile content into lines (`Line`), handling directives, raw text, and target definitions.
//! - Convert lines into tokens (`Token`), associating optional target labels.
//! - Detect common issues such as unmatched brackets or unexpected raw lines.
//!
//! The lexer output is consumed later to build distributed makefile sets.

use crate::lexer::{
    directive::DIRECTIVE_PREFIX,
    target_label::TargetLabel,
    tokens::{Line, Token},
};
use anyhow::{Context, Result};
use std::{fs::File, io::Read, path::Path};
use tracing::{info, warn};

/// Default filenames to try when searching for a Makefile.
const DEFAULT_PATH_CANDIDATES: [&str; 3] = ["Makefile", "makefile", "GNUMakefile"];

/// Error message if no Makefile was found.
const NO_MAKEFILE_FOUND: &str = "dake: *** No targets specified and no makefile found.  Stop.";

/// The result of lexing: a list of tokens.
pub type LexingOutput = Vec<Token>;

/// Lex a string into [`Token`]s.
///
/// # Behavior
/// - Splits into [`Line`]s (directives, raw lines, colon rules).
/// - Groups consecutive raw lines into `RawText`.
/// - Converts colon rules into `Target` tokens, possibly with labels.
/// - Parses directives into `Directive` tokens.
///
/// # Errors
/// Returns an error if directive parsing or target label parsing fails.
pub fn lex(s: String) -> Result<LexingOutput> {
    const FORBIDDEN_RIGHT_PREFIX: [&str; 1] = ["="];

    /// Splits the raw string into [`Line`]s, handling:
    /// - Directives (prefixed with `DIRECTIVE_PREFIX`)
    /// - Comments (ignored)
    /// - Continuations (`\` at end of line)
    fn generate_lines(s: &str) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut lines_iter = s.lines();

        while let Some(line) = lines_iter.next() {
            // Handle directives
            if line.starts_with(DIRECTIVE_PREFIX) {
                lines.push(Line::Directive(line[2..].to_string()));
                continue;
            }

            // Strip comments after `#`
            let (mut line, _) = line
                .split_once('#')
                .map(|(l, _)| (l.to_string(), ()))
                .unwrap_or((line.to_string(), ()));

            /// Pushes a line into the `lines` vector as either a raw or colon line.
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

    /// Converts a sequence of [`Line`]s into [`Token`]s.
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

                    // Parse labels inside [brackets]
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
                            warn!("Lexer: Unmatched bracket in target: {}", left);
                            break;
                        }
                    }

                    let token = match left_parsed {
                        Some((target, label)) => Token::Target {
                            target,
                            label: Some(label),
                            command: right,
                        },
                        None => Token::Target {
                            target: left,
                            label: None,
                            command: right,
                        },
                    };
                    tokens.push(token);
                }
                Some(Line::RawLine(line)) => {
                    warn!("Lexer: Unexpected RawLine after processing: {}", line);
                }
                Some(Line::Directive(dir)) => tokens.push(Token::Directive(dir.parse()?)),
                None => break,
            }
        }
        Ok(tokens)
    }

    let lines = generate_lines(&s);
    info!("Lexer: Generated {} lines", lines.len());

    let tokens = lines_to_tokens(lines.clone())?;
    info!("Lexer: Produced {} tokens", tokens.len());

    Ok(tokens)
}

/// Reads a file from the given path and lexes its contents.
///
/// # Errors
/// Fails if file cannot be opened, read, or lexed.
pub fn lex_from_path(path: &str) -> Result<LexingOutput> {
    let mut f = File::open(&path).context(format!("When opening the file {path}."))?;
    let mut content = String::new();
    f.read_to_string(&mut content)
        .context(format!("When reading the file {path}."))?;
    info!("Lexer: Successfully read file {}", path);

    lex(content)
}

/// Attempts to guess the Makefile path from default candidates and lex it.
///
/// # Errors
/// Fails if no Makefile is found in the current directory.
pub fn guess_path_and_lex() -> Result<LexingOutput> {
    let path = DEFAULT_PATH_CANDIDATES
        .iter()
        .find(|path| Path::new(path).try_exists().unwrap_or(false))
        .context(NO_MAKEFILE_FOUND)?;
    info!("Lexer: Using Makefile at path {}", path);

    lex_from_path(path)
}
