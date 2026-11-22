//! Conditional inclusion ([cpp.cond]): the `#if` expression hook.
//!
//! The core parses the directive structure (`#if` .. `#endif` nesting,
//! skipping of excluded groups) itself. The *value* of a `#if` / `#elif`
//! expression comes from an [`IfEvaluator`]; the evaluator kiln installs
//! is [`crate::IfEval`] ([cpp.cond]/12 `intmax_t`/`uintmax_t` arithmetic,
//! `__has_*`, clang's diagnostics).

use kiln_diag::{Diag, DiagId};
use kiln_lex::{Interner, Std, Symbol, Token};
use kiln_source::SourceManager;

/// What an [`IfEvaluator`] can do with the directive it evaluates.
///
/// Tokens end with `Eod` (repeated if read again). The core discards
/// whatever the evaluator leaves unread on the line.
pub trait IfContext {
    /// The next token, macro-expanded ([cpp.cond]/13). Builtins such as
    /// `__has_include(...)` are already replaced by their values.
    fn next(&mut self) -> Token;
    /// The next token without macro expansion (for the operand of
    /// `defined`, [cpp.cond]/10).
    fn next_unexpanded(&mut self) -> Token;
    /// `defined name` (marks the macro used).
    fn is_defined(&mut self, name: Symbol) -> bool;
    /// The spelling of a token (cleaned of line splices).
    fn spelling(&self, tok: &Token) -> String;
    /// The interner the token symbols belong to.
    fn interner(&self) -> &Interner;
    /// The source manager the token spans belong to.
    fn source_manager(&self) -> &SourceManager;
    /// Emits a diagnostic.
    fn emit(&mut self, diag: Diag);
    /// True if `id` would be reported (to skip building a diagnostic).
    fn is_enabled(&self, id: DiagId) -> bool;
    /// The language standard.
    fn std(&self) -> Std;
    /// Whether a `u8` literal is a `char8_t` (`-f[no-]char8_t`).
    fn char8(&self) -> bool {
        self.std() >= Std::Cxx20
    }
    /// The C standard when the input is C (`None`: C++). C has no
    /// operator names and, before C23, no `true`/`false`.
    fn c(&self) -> Option<kiln_lex::CStd> {
        None
    }
    /// The `-Wundef-prefix=` prefixes.
    fn undef_prefixes(&self) -> &[String] {
        &[]
    }
    /// Whether literals decode with clang 19's escape rules
    /// ([`crate::PpOptions::clang19_escapes`]).
    fn clang19_escapes(&self) -> bool {
        false
    }
}

/// Evaluates the controlling expression of `#if` / `#elif`.
pub trait IfEvaluator {
    /// Reads the expression from `cx` and returns whether the group is
    /// included. Errors are reported through `cx`; an erroneous
    /// expression evaluates to `false`.
    fn eval(&mut self, cx: &mut dyn IfContext) -> bool;

    /// Evaluates an integer constant expression in preprocessor arithmetic
    /// for an `#embed` / `__has_embed` `limit(...)` / `clang::offset(...)`
    /// parameter (C23 6.10.4.2). The tokens end with `Eod` at the closing
    /// parenthesis. Returns `None` after reporting an error. The default
    /// is [`crate::IfEval`]'s for [`IfEvaluator::nested`]'s target.
    fn eval_value(&mut self, cx: &mut dyn IfContext) -> Option<i64> {
        self.nested().eval_value(cx)
    }

    /// The evaluator for an expression read while this one is running: a
    /// `__has_embed(... limit(e))` parameter inside a `#if` ([cpp.cond]/5,
    /// C23 6.10.1) is a full constant expression of its own, evaluated as
    /// clang's `PPExpressions` evaluates it. The default is
    /// [`crate::IfEval::default`]; [`crate::IfEval`] returns itself, so
    /// character literals keep the target's signedness.
    fn nested(&self) -> crate::IfEval {
        crate::IfEval::default()
    }
}
