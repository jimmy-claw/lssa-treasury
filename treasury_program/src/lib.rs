//! The Treasury Program implementation.
//!
//! Demonstrates PDA usage by managing token vaults through chained calls
//! to the Token program.

pub use treasury_core as core;

pub mod create_vault;
pub mod receive;
pub mod send;
