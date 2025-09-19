mod lexer;
mod makefile;
mod tokens;

pub use lexer::{guess_path_and_lex, lex, lex_from_path};
pub use makefile::{Makefile, RemoteMakefile};
