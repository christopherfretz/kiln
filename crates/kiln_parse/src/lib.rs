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
mod new_delete;
mod parser;
mod pragma;
mod stmt;
mod tentative;
pub mod tokens;

use std::sync::Arc;

use kiln_ast::Ast;
use kiln_ast::types::TypeContext;
use kiln_diag::DiagEngine;
use kiln_lex::Std;
use kiln_lex::literal::{EncodingParams, LiteralOptions};
use kiln_pp::{PpOptions, Preprocessor};
use kiln_sema::target::Target;
use kiln_sema::{Budget, Oven, Sema};
use kiln_source::symbol::Interner;
use kiln_source::{FileId, SourceManager};

pub use parser::{PExpr, Parser};
pub use tentative::TypeIdContext;
pub use tokens::{Event, TokenSource};

/// Options for [`parse_file`], [`parse_tu`] and [`parse_preprocessed`].
#[derive(Clone, Copy, Debug)]
pub struct ParseOptions {
    /// The language standard.
    pub std: Std,
    /// Keep every token of the translation unit instead of a sliding
    /// window (the pre-WP-055 whole-file buffer; for measurements only).
    pub keep_tokens: bool,
    /// C++ exceptions are enabled (`-f[no-]exceptions`): `throw` is
    /// an error otherwise ([except], clang's `err_exceptions_disabled`).
    pub exceptions: bool,
    /// Run-time type information is enabled (`-f[no-]rtti`): `typeid`
    /// is an error otherwise (clang's `err_no_typeid_with_fno_rtti`).
    pub rtti: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        ParseOptions { std: Std::Cxx26, keep_tokens: false, exceptions: true, rtti: true }
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
/// reporting to `diags`, and returns its AST: [`parse_tu`] with the
/// default [`ConstexprLimits`].
///
/// `interner` must outlive `diags` (the preprocessor and the AST share
/// it).
#[must_use]
pub fn parse_file<'sm>(
    sm: &'sm SourceManager,
    interner: &'sm Arc<Interner>,
    file: FileId,
    diags: &DiagEngine<'sm>,
    target: Target,
    opts: &ParseOptions,
) -> Ast {
    parse_tu(sm, interner, file, diags, target, opts, ConstexprLimits::default()).ast
}

/// [`parse_file`] with explicit constant-evaluation limits, also returning
/// the TU's Oven. The file is preprocessed with [`PpOptions::new`] (no
/// predefined macros, no include paths): a driver builds its own
/// [`Preprocessor`] and calls [`parse_preprocessed`].
#[must_use]
pub fn parse_tu<'sm>(
    sm: &'sm SourceManager,
    interner: &'sm Arc<Interner>,
    file: FileId,
    diags: &DiagEngine<'sm>,
    target: Target,
    opts: &ParseOptions,
    limits: ConstexprLimits,
) -> ParsedTu {
    let mut pp = Preprocessor::new(sm, diags, interner, PpOptions::new(opts.std));
    pp.enter_main_file(file);
    parse_preprocessed(pp, Arc::clone(interner), diags, target, opts, limits).0
}

/// Parses and analyses the translation unit `pp` produces (its main file
/// entered), reporting to `diags` and evaluating constant expressions
/// under `limits`. `interner` must be the preprocessor's. Returns the
/// parsed TU and the preprocessor (for its macro table and dependency
/// callbacks).
///
/// # Panics
///
/// If `interner` is not `pp`'s interner.
#[must_use]
pub fn parse_preprocessed<'a, 'sm>(
    pp: Preprocessor<'sm, 'a>,
    interner: Arc<Interner>,
    diags: &'a DiagEngine<'sm>,
    target: Target,
    opts: &ParseOptions,
    limits: ConstexprLimits,
) -> (ParsedTu, Preprocessor<'sm, 'a>) {
    assert!(
        std::ptr::eq(Arc::as_ptr(&interner), std::ptr::from_ref(pp.interner())),
        "the AST and the preprocessor share an interner"
    );
    let ast = Ast::new(interner, TypeContext::new(target.types.clone()));
    let lit_opts = LiteralOptions::new(opts.std, encoding_params(&target));
    let mut s = Sema::for_target(ast, diags, target);
    s.set_constexpr_limits(limits.budget, limits.backtrace_limit);
    s.exceptions = opts.exceptions;
    s.rtti = opts.rtti;
    let mut src = TokenSource::new(pp, diags);
    src.keep_all(opts.keep_tokens);
    let mut p = Parser::new(s, src, lit_opts);
    p.parse_translation_unit();
    let (s, src) = p.finish();
    let (ast, oven) = s.into_parts();
    (ParsedTu { ast, oven }, src.into_preprocessor())
}
