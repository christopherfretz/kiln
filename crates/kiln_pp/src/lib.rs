//! Translation phase 4: the preprocessor ([cpp]).
//!
//! See `docs/ARCHITECTURE.md` and `spec/pp.md` for the contract
//! this crate implements.
//!
//! The core ([`Preprocessor`]) owns directives, conditional skipping, macro
//! expansion, `_Pragma`, the predefined and builtin macros, and the
//! `-E` printer ([`print`]). Everything that depends on the outside world
//! is a hook installed on the preprocessor:
//!
//! - [`IncludeResolver`]: `#include`, `#include_next`, `#embed`,
//!   `__has_include`, `__has_embed` (header search WP-014, `#embed`
//!   resources WP-016);
//! - [`IfEvaluator`]: the value of a `#if` / `#elif` expression (WP-015);
//! - [`FeatureQueries`]: `__has_builtin`, `__has_attribute`,
//!   `__has_feature`, ... (the tables of WP-015);
//! - [`PpCallbacks`]: observers and pragma handlers (`-M` dependency
//!   output with WP-014, `-dD` / `-dI` with WP-017, phase-7 pragmas).
//!
//! `#pragma once` and the include-guard optimization (clang's
//! `MultipleIncludeOpt`) are in the core.
#![forbid(unsafe_code)]

mod builtin;
mod callbacks;
mod cond;
mod diag_map;
mod directive;
mod expand;
mod include;
mod macros;
mod options;
mod pragma;
mod preprocessor;
pub mod print;
mod tok;

pub use builtin::{Builtin, FeatureQueries, NoFeatures, TargetQuery};
pub use callbacks::{
    FileChange, NoCallbacks, PpCallbacks, PragmaInfo, PragmaIntroducer, PragmaMessageKind,
};
pub use cond::{IfContext, IfEvaluator, StubIfEvaluator};
pub use include::{
    EmbedData, EmbedRequest, IncludeKind, IncludeRequest, IncludeResolver, MemoryResolver,
    NoIncludes, ResolvedInclude,
};
pub use macros::{MacroDef, MacroTable};
pub use options::PpOptions;
pub use preprocessor::{Marker, PpItem, Preprocessor};
pub use print::{PrintOptions, print_macros, print_preprocessed};
