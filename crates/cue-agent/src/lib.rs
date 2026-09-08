//! cue-agent: process-isolated, supervised launcher and batch
//! execution engine for headless coding agents.
//!
//! This crate hosts the execution spec model (`spec`) shared by the
//! `cue-agent` binary; supervision and process management land in
//! later phases.

pub mod spec;
