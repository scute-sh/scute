//! Cognitive complexity scoring for source code functions.
//!
//! Scores each function based on how hard it is to understand, following
//! [G. Ann Campbell's cognitive complexity spec](https://www.sonarsource.com/docs/CognitiveComplexity.pdf).
//! Six cognitive drivers contribute to the score: flow breaks, nesting,
//! else branches, boolean logic sequences, recursion, and labeled jumps.
//!
//! Nesting is the main multiplier: an `if` inside a `for` inside a `match`
//! costs `1 + depth`, not just 1. Else-if chains are scored flat. Closures
//! inherit the parent's nesting level.
//!
//! # Usage
//!
//! ```no_run
//! use std::path::PathBuf;
//! use scute_core::code_complexity::{self, Definition};
//!
//! let results = code_complexity::check(
//!     &[PathBuf::from("src/")],  // files or directories to check
//!     &Definition::default(),    // warn: 5, fail: 10
//! );
//! ```
//!
//! Each function produces one [`Evaluation`](crate::Evaluation) with per-line
//! [`Evidence`](crate::Evidence) entries explaining what drives the score.

mod check;
mod rules;
mod rust;
mod score;

pub use check::{CHECK_NAME, Definition, check};
