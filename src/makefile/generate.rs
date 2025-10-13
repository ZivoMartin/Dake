//! # Remote Makefile Set Generator
//!
//! This module implements the logic to build a [`RemoteMakefileSet`] from a
//! stream of parsed [`Token`]s produced by the lexer.
//!
//! Responsibilities:
//! - Generate local and remote makefiles from tokens.
//! - Handle `Target` rules by rewriting them into either a fetch command or a
//!   direct target rule, depending on the associated [`TargetLabel`].
//! - Handle `Directive`s such as [`RootDef`] to register root paths.
//! - Ensure that each IP involved in the distributed build has its own
//!   [`RemoteMakefile`].
//!
//! The first makefile is considered the "primary" one, while additional
//! makefiles are stored separately.

use crate::{
    lexer::{Directive, TargetLabel, Token},
    makefile::{RemoteMakefile, RemoteMakefileSet},
};
use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};
use tracing::{info, warn};

impl RemoteMakefileSet {
    /// Generates a new [`RemoteMakefileSet`] from a set of tokens.
    ///
    /// # Arguments
    /// * `tokens` - The list of tokens parsed from a Makefile.
    /// * `caller_dir` - The local working directory of the caller.
    /// * `caller_sock` - The socket of the caller.
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
    pub fn generate(tokens: Vec<Token>, caller_dir: PathBuf, caller_sock: SocketAddr) -> Self {
        info!(
            "RemoteMakefileSet: Starting generation with {} tokens",
            tokens.len()
        );

        let mut full_fetch_makefile = String::new();
        let mut saw_ips = HashSet::from([caller_sock.ip()]);
        let mut makefiles = vec![RemoteMakefile::new(String::new(), caller_sock)];
        let mut root_path_set = HashMap::from([(caller_sock.ip(), caller_dir.clone())]);

        // Utility closure to construct fetch commands
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
                    let label = label.unwrap_or_else(|| TargetLabel::new(caller_sock, None));
                    info!(
                        "RemoteMakefileSet: Processing target '{}' for label {:?}",
                        target, label
                    );

                    // Add a new makefile for this IP if not already seen
                    if saw_ips.insert(label.ip()) {
                        info!(
                            "RemoteMakefileSet: Adding new RemoteMakefile for ip {}",
                            label.ip()
                        );
                        makefiles.push(RemoteMakefile::new(full_fetch_makefile.clone(), label.sock))
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
                    Directive::RootDef { ip, path } => {
                        info!(
                            "RemoteMakefileSet: Registered RootDef ip={}, path={:?}",
                            ip, path
                        );
                        root_path_set.insert(ip, path);
                    }
                },
            }
        }

        // Build RemoteMakefileSet from results
        let mut iter = makefiles.into_iter();
        match iter.next() {
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
        }
    }
}
