//! # DAKE Library
//!
//! This crate provides the core modules for the DAKE distributed build system.
//! It exposes public modules for interacting with the daemon, fetching data,
//! and managing processes, while keeping internal components encapsulated.

pub mod caller;
pub mod daemon;
pub mod fetch;
pub mod network;
pub mod process_id;

mod constants;
mod env_variables;
mod lexer;
mod macros;
mod makefile;
mod utils;
