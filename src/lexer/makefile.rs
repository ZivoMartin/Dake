use crate::lexer::tokens::{Line, Token};
use derive_getters::Getters;

#[derive(Debug, Getters)]
pub struct Makefile {
    raw: String,
    lines: Vec<Line>,
    tokens: Vec<Token>,
}

impl Makefile {
    pub fn new(raw: String, lines: Vec<Line>, tokens: Vec<Token>) -> Self {
        Makefile { raw, lines, tokens }
    }
}
