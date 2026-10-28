//! Recursive-descent C++26 parser driving semantic analysis.
//!
//! See `docs/ARCHITECTURE.md` and `spec/parse.md` for the contract
//! this crate implements. The parser owns the grammar and the token
//! stream; [`kiln_sema::Sema`] owns scopes, lookup and the typed AST
//! (clang's Parser/Sema split).
#![forbid(unsafe_code)]

mod attr;
mod decl;
mod expr;
pub mod keywords;
mod lexdiag;
mod parser;
mod stmt;
pub mod tokens;

use std::sync::Arc;

use kiln_ast::Ast;
use kiln_ast::types::TypeContext;
use kiln_diag::DiagEngine;
use kiln_lex::literal::{EncodingParams, LiteralOptions};
use kiln_lex::{LexOptions, Std};
use kiln_sema::target::Target;
use kiln_sema::{Budget, Oven, Sema};
use kiln_source::symbol::Interner;
use kiln_source::{FileId, SourceManager};

pub use lexdiag::lex_diag;
pub use parser::{PExpr, Parser};
use tokens::{Skipped, TokenBuffer};

/// Options for [`parse_file`].
#[derive(Clone, Copy, Debug)]
pub struct ParseOptions {
    /// The language standard.
    pub std: Std,
}

impl Default for ParseOptions {
    fn default() -> Self {
        ParseOptions { std: Std::Cxx26 }
    }
}

/// The literal-encoding parameters of `target`.
fn encoding_params(target: &Target) -> EncodingParams {
    EncodingParams {
        char_signed: target.types.char_signed,
        wchar_bits: 32,
        wchar_signed: target.types.wchar_signed,
    }
}

/// Constant-evaluation settings of a translation unit: the driver's
/// `-fconstexpr-steps=`, `-fconstexpr-depth=`, `-fconstexpr-memory=` and
/// `-fconstexpr-backtrace-limit=` (`spec/oven.md` "Integration").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstexprLimits {
    /// The budget of each top-level evaluation.
    pub budget: Budget,
    /// Call frames a failure's backtrace shows (0: all).
    pub backtrace_limit: usize,
}

impl Default for ConstexprLimits {
    /// clang's defaults (steps 1048576, depth 512, backtrace 10) and
    /// kiln's 256 MiB memory budget.
    fn default() -> Self {
        ConstexprLimits { budget: Budget::default(), backtrace_limit: 10 }
    }
}

/// A parsed translation unit: its AST and the Oven that evaluated its
/// constant expressions (whose [`Oven::stats`] feed
/// `-fconstexpr-report=`).
#[derive(Debug)]
pub struct ParsedTu {
    /// The AST.
    pub ast: Ast,
    /// The TU's constant evaluator.
    pub oven: Oven,
}

/// Parses and analyses the translation unit whose main file is `file`,
/// reporting to `diags`, and returns its AST.
#[must_use]
pub fn parse_file<'sm>(
    sm: &'sm SourceManager,
    file: FileId,
    diags: &DiagEngine<'sm>,
    target: Target,
    opts: &ParseOptions,
) -> Ast {
    parse_tu(sm, file, diags, target, opts, ConstexprLimits::default()).ast
}

/// [`parse_file`] with explicit constant-evaluation limits, also returning
/// the TU's Oven.
#[must_use]
pub fn parse_tu<'sm>(
    sm: &'sm SourceManager,
    file: FileId,
    diags: &DiagEngine<'sm>,
    target: Target,
    opts: &ParseOptions,
    limits: ConstexprLimits,
) -> ParsedTu {
    let interner = Arc::new(Interner::new());
    let lex_opts = LexOptions::new(opts.std);
    let buf = TokenBuffer::lex(sm, &interner, file, lex_opts);
    let ast = Ast::new(interner, TypeContext::new(target.types.clone()));
    let lit_opts = LiteralOptions::new(opts.std, encoding_params(&target));
    let mut s = Sema::for_target(ast, diags, target);
    s.set_constexpr_limits(limits.budget, limits.backtrace_limit);
    for d in &buf.diags {
        if let Some(d) = lex_diag(sm, d) {
            s.emit(d);
        }
    }
    for k in &buf.skipped {
        match k {
            Skipped::UnsupportedPragma(what, loc) => s.act_on_unsupported_pragma(what, *loc),
            Skipped::Directive(name, loc) => {
                s.unsupported(&format!("the '#{name}' directive"), *loc);
            }
        }
    }
    let mut p = Parser::new(s, sm, buf.toks, buf.spellings, lex_opts, lit_opts);
    p.parse_translation_unit();
    let (ast, oven) = p.into_sema().into_parts();
    ParsedTu { ast, oven }
}
