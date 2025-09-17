use std::collections::HashSet;

use crate::lexer::tokens::{Line, Token};
use derive_getters::Getters;

fn get_fetch_command(ip: String, target: String) -> String {
    format!("target/debug/dake fetch \"{ip}\" \"{target}\"\n")
}

#[derive(Debug, Getters)]
pub struct Makefile {
    raw: String,
    lines: Vec<Line>,
    tokens: Vec<Token>,
}

#[derive(Getters, Debug)]
pub struct RemoteMakefile {
    ip: String,
    makefile: String,
}

impl RemoteMakefile {
    fn new(ip: String, makefile: String) -> Self {
        Self { ip, makefile }
    }
}

impl Makefile {
    pub fn new(raw: String, lines: Vec<Line>, tokens: Vec<Token>) -> Self {
        Makefile { raw, lines, tokens }
    }

    pub fn generate(&self, caller_ip: String) -> Vec<RemoteMakefile> {
        let mut full_fetch_makefile = String::new();
        let mut saw_ips = HashSet::from([caller_ip.clone()]);
        let mut makefiles = vec![RemoteMakefile::new(caller_ip.clone(), String::new())];

        for token in self.tokens.iter() {
            match token {
                Token::RawText(text) => makefiles
                    .iter_mut()
                    .for_each(|m: &mut RemoteMakefile| m.makefile.push_str(&text)),
                Token::Target {
                    target,
                    ip,
                    command,
                } => {
                    let ip = ip.clone().unwrap_or_else(|| caller_ip.clone());

                    if saw_ips.insert(ip.clone()) {
                        makefiles.push(RemoteMakefile::new(ip.clone(), full_fetch_makefile.clone()))
                    }

                    let fetch_command = get_fetch_command(ip.clone(), target.clone());
                    let fetch = format!("{target}:\n\t{fetch_command}\n");
                    let default = format!("{target}:{command}");

                    full_fetch_makefile += &fetch;
                    for m in makefiles.iter_mut() {
                        m.makefile
                            .push_str(if m.ip == ip { &default } else { &fetch })
                    }
                }
            }
        }
        makefiles
    }
}
