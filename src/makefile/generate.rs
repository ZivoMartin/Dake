//! # Remote Makefile Set Generator
//!
//! This module implements the logic to build a [`RemoteMakefileSet`] from a
//! stream of parsed [`Token`]s produced by the lexer.
//!
//! The first makefile is considered the "primary" one, while additional
//! makefiles are stored separately.

use crate::{
    lexer::{Directive, TargetLabel, Token},
    makefile::{RemoteMakefile, RemoteMakefileSet},
    network::DEFAULT_PORT,
    process_id::ProcessId,
};
use anyhow::Result;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    path::PathBuf,
};
use tracing::{info, warn};

impl RemoteMakefileSet {
    /// Generates a new [`RemoteMakefileSet`] from a set of tokens.
    ///
    /// # Behavior
    /// - Raw text (`Token::RawText`) is appended to all makefiles.
    /// - Target rules (`Token::Target`) are rewritten into:
    ///   - A local target rule in the appropriate makefile.
    ///   - A "fetch" rule in other makefiles, instructing them to fetch the
    ///     target from the correct host.
    /// - Directives (`Token::Directive`) register root paths for resolving
    ///   labels.
    ///
    /// # Returns
    /// A new [`RemoteMakefileSet`] containing the distributed makefiles.
    pub fn generate(tokens: Vec<Token>, sock: SocketAddr, pid: ProcessId) -> Result<Self> {
        info!(
            "RemoteMakefileSet: Starting generation with {} tokens",
            tokens.len()
        );

        let path = pid.path().clone();
        let mut full_fetch_makefile = String::new();
        let mut saw_ips = HashSet::from([sock]);
        let mut makefiles = vec![RemoteMakefile::new(String::new(), sock)];
        let mut root_path_set = HashMap::from([(sock, path.clone())]);

        // Utility closure to construct fetch commands
        let get_fetch_command = |root_path_set: &HashMap<SocketAddr, PathBuf>,
                                 label: TargetLabel,
                                 target: String|
         -> String {
            let label_path = match label.path {
                Some(label_path) => format!("--labeled-path {}", label_path.display()),
                None => match root_path_set.get(&label.sock) {
                    Some(label_path) => format!("--labeled-path {}", label_path.display()),
                    None => String::new(),
                },
            };
            format!(
                "dake fetch {process_id} {label_sock} {label_path} \"{target}\"\n",
                process_id = pid,
                label_sock = label.sock
            )
        };

        // Process tokens
        for token in tokens.into_iter() {
            match token {
                Token::RawText(text) => {
                    info!(
                        "RemoteMakefileSet: Appending raw text of length {}",
                        text.len()
                    );
                    makefiles
                        .iter_mut()
                        .for_each(|m: &mut RemoteMakefile| m.push_content(&text))
                }
                Token::Target {
                    target,
                    label,
                    command,
                } => {
                    let label = label.unwrap_or_else(|| TargetLabel::new(sock, None));
                    info!(
                        "RemoteMakefileSet: Processing target '{}' for label {:?}",
                        target, label
                    );

                    // Add a new makefile for this IP if not already seen
                    if saw_ips.insert(label.sock.clone()) {
                        info!(
                            "RemoteMakefileSet: Adding new RemoteMakefile for sock {}",
                            label.sock
                        );
                        makefiles.push(RemoteMakefile::new(
                            full_fetch_makefile.clone(),
                            label.sock.clone(),
                        ))
                    }

                    // Build fetch and default rules
                    let fetch_command =
                        get_fetch_command(&root_path_set, label.clone(), target.clone());

                    let fetch = format!("{target}:\n\t{fetch_command}\n");
                    let default = format!("{target}:{command}");

                    full_fetch_makefile += &fetch;

                    // Distribute rules across makefiles
                    for m in makefiles.iter_mut() {
                        m.push_content(if m.ip() == label.ip() {
                            &default
                        } else {
                            &fetch
                        })
                    }
                }
                Token::Directive(dir) => match dir {
                    Directive::RootDef { ip, path: dir_path } => {
                        info!(
                            "RemoteMakefileSet: Registered RootDef ip={}, path={:?}",
                            ip, dir_path
                        );
                        root_path_set.insert(SocketAddr::new(ip, DEFAULT_PORT), dir_path);
                    }
                },
            }
        }

        // Build RemoteMakefileSet from results
        let mut iter = makefiles.into_iter();
        Ok(match iter.next() {
            Some(first) => {
                let rest: Vec<_> = iter.collect();
                info!(
                    "RemoteMakefileSet: Successfully generated {} remote makefiles",
                    rest.len() + 1
                );
                RemoteMakefileSet::new(rest, first.drop_makefile())
            }
            None => {
                warn!("RemoteMakefileSet: makefiles array should not be empty at this point.");
                RemoteMakefileSet::new(Vec::new(), String::new())
            }
        })
    }
}
