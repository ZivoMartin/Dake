use std::collections::HashSet;

use crate::lexer::tokens::{Line, Token};
use derive_getters::Getters;
use log::warn;

fn get_fetch_command(ip: String, target: String) -> String {
    format!("target/debug/dake fetch \"{ip}\" \"{target}\"\n")
}

#[derive(Debug, Getters)]
pub struct Makefile {
    raw: String,
    lines: Vec<Line>,
    tokens: Vec<Token>,
}

#[derive(Getters, Debug, Clone)]
pub struct RemoteMakefile {
    ip: String,
    makefile: String,
}

impl RemoteMakefile {
    fn new(ip: String, makefile: String) -> Self {
        Self { ip, makefile }
    }
}

#[derive(Getters)]
pub struct RemoteMakefileSet {
    remote_makefiles: Vec<RemoteMakefile>,
    my_makefile: String,
}

impl RemoteMakefileSet {
    pub fn new(remote_makefiles: Vec<RemoteMakefile>, my_makefile: String) -> Self {
        Self {
            remote_makefiles,
            my_makefile,
        }
    }
}

impl Makefile {
    pub fn new(raw: String, lines: Vec<Line>, tokens: Vec<Token>) -> Self {
        Makefile { raw, lines, tokens }
    }

    pub fn generate(&self, caller_ip: String) -> RemoteMakefileSet {
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
        let mut iter = makefiles.into_iter();
        match iter.next() {
            Some(first) => {
                let rest: Vec<_> = iter.collect();
                RemoteMakefileSet::new(rest, first.makefile)
            }
            None => {
                warn!("makefiles array should not be empty at this point.");
                RemoteMakefileSet::new(Vec::new(), String::new())
            }
        }
    }
}
