use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use log::warn;

use crate::makefile::RemoteMakefileSet;
use crate::{
    lexer::{Directive, TargetLabel, Token},
    makefile::RemoteMakefile,
};

impl RemoteMakefileSet {
    pub fn generate(tokens: Vec<Token>, caller_dir: PathBuf, caller_sock: SocketAddr) -> Self {
        let mut full_fetch_makefile = String::new();
        let mut saw_ips = HashSet::from([caller_sock.ip()]);
        let mut makefiles = vec![RemoteMakefile::new(String::new(), caller_sock)];
        let mut root_path_set = HashMap::from([(caller_sock.ip(), caller_dir.clone())]);

        let get_fetch_command = |root_path_set: &HashMap<IpAddr, PathBuf>,
                                 label: TargetLabel,
                                 target: String|
         -> String {
            let path = match label.path {
                Some(path) => format!("--labeled-path {}", path.display()),
                None => match root_path_set.get(&label.ip()) {
                    Some(path) => format!("--labeled-path {}", path.display()),
                    None => String::new(),
                },
            };
            format!(
                "target/debug/dake fetch \"{}\" {} {path} \"{target}\"\n",
                caller_dir.display(),
                label.sock
            )
        };

        for token in tokens.into_iter() {
            match token {
                Token::RawText(text) => makefiles
                    .iter_mut()
                    .for_each(|m: &mut RemoteMakefile| m.push_content(&text)),
                Token::Target {
                    target,
                    label,
                    command,
                } => {
                    let label = label.unwrap_or_else(|| TargetLabel::new(caller_sock, None));

                    if saw_ips.insert(label.ip()) {
                        makefiles.push(RemoteMakefile::new(full_fetch_makefile.clone(), label.sock))
                    }

                    let fetch_command =
                        get_fetch_command(&root_path_set, label.clone(), target.clone());

                    let fetch = format!("{target}:\n\t{fetch_command}\n");
                    let default = format!("{target}:{command}");

                    full_fetch_makefile += &fetch;
                    for m in makefiles.iter_mut() {
                        m.push_content(if m.ip() == label.ip() {
                            &default
                        } else {
                            &fetch
                        })
                    }
                }
                Token::Directive(dir) => match dir {
                    Directive::RootDef { ip, path } => {
                        root_path_set.insert(ip, path);
                    }
                },
            }
        }
        let mut iter = makefiles.into_iter();
        match iter.next() {
            Some(first) => {
                let rest: Vec<_> = iter.collect();
                RemoteMakefileSet::new(rest, first.drop_makefile())
            }
            None => {
                warn!("makefiles array should not be empty at this point.");
                RemoteMakefileSet::new(Vec::new(), String::new())
            }
        }
    }
}
