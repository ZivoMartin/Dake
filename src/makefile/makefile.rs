use std::{collections::HashSet, net::SocketAddr, path::PathBuf};

use crate::{lexer::Token, target_label::TargetLabel};
use derive_getters::Getters;
use log::warn;
use serde::{Deserialize, Serialize};

fn get_fetch_command(label: TargetLabel, target: String) -> String {
    let path = match label.path {
        Some(path) => format!("--path {}", path.display()),
        None => String::new(),
    };
    format!(
        "target/debug/dake fetch \"{}\" {path} \"{target}\"\n",
        label.sock
    )
}

#[derive(Getters, Clone, Serialize, Deserialize)]
pub struct RemoteMakefile {
    makefile: String,
    sock: SocketAddr,
}

impl RemoteMakefile {
    pub fn new(makefile: String, sock: SocketAddr) -> Self {
        RemoteMakefile { makefile, sock }
    }

    pub fn set_sock(&mut self, sock: SocketAddr) {
        self.sock = sock
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

    pub fn drop_makefiles(self) -> Vec<RemoteMakefile> {
        self.remote_makefiles
    }

    pub fn generate(tokens: Vec<Token>, caller_dir: PathBuf, caller_sock: SocketAddr) -> Self {
        let mut full_fetch_makefile = String::new();
        let mut saw_ips = HashSet::from([caller_sock.ip()]);
        let mut makefiles = vec![RemoteMakefile::new(String::new(), caller_sock)];
        let default_label = TargetLabel::new(caller_sock, Some(caller_dir));

        for token in tokens.into_iter() {
            match token {
                Token::RawText(text) => makefiles
                    .iter_mut()
                    .for_each(|m: &mut RemoteMakefile| m.makefile.push_str(&text)),
                Token::Target {
                    target,
                    label,
                    command,
                } => {
                    let label = label.unwrap_or_else(|| default_label.clone());

                    if saw_ips.insert(label.ip()) {
                        makefiles.push(RemoteMakefile::new(full_fetch_makefile.clone(), label.sock))
                    }

                    let fetch_command = get_fetch_command(label.clone(), target.clone());
                    let fetch = format!("{target}:\n\t{fetch_command}\n");
                    let default = format!("{target}:{command}");

                    full_fetch_makefile += &fetch;
                    for m in makefiles.iter_mut() {
                        m.makefile.push_str(if m.sock.ip() == label.ip() {
                            &default
                        } else {
                            &fetch
                        })
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
