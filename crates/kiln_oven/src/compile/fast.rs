//! Library fast paths (WP-1864; spec/oven.md "Library fast paths"): calls
//! of standard-library plumbing the Oven evaluates without running the
//! library's code.
//!
//! - A call of a wrapper of the evaluation context (libstdc++'s
//!   `std::__is_constant_evaluated` and `std::is_constant_evaluated`,
//!   `if consteval { return true; } else { return false; }`; libc++'s
//!   `std::__libcpp_is_constant_evaluated`, `return
//!   __builtin_is_constant_evaluated();`) is the context itself, read as
//!   `if consteval` reads it ([`Compiler::ice_wrapper`]): no call, no
//!   frame, no step, no depth, no memo lookup, as clang's `isStdBuiltin`
//!   set is no call.
//! - A call of `std::char_traits<C>::compare`, `length` or `find` for a
//!   character type `C` ([`Compiler::traits_fast`]) is preceded by a
//!   `Builtin` `bi::TRAITS` that evaluates the whole operation natively
//!   (`vm/fast.rs`) and skips the call when it may; when it may not (an
//!   invalid read, a pointer past its array, a budget it would cross), it
//!   declines and the call runs, so every diagnostic is the library
//!   code's.

use kiln_ast::DeclId;
use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::types::BuiltinType;
use kiln_ast::{Ast, ExprId, StmtId};

use super::new_delete::{in_std, is_named, is_std_named};
use super::{Compiler, skip_parens};
use crate::kbc::bi;
use crate::ty::OTy;

/// The statement a compound statement of one statement holds (or `s`).
fn only(ast: &Ast, s: StmtId) -> Option<StmtId> {
    if ast.stmts.get(s).kind() != StmtKind::CompoundStmt {
        return Some(s);
    }
    match ast.lists.get(ast.stmts.compound_body(s))[..] {
        [x] => Some(StmtId::new(x)),
        _ => None,
    }
}

/// `e` without parentheses and no-op or boolean-preserving implicit
/// conversions.
fn bare(ast: &Ast, mut e: ExprId) -> ExprId {
    loop {
        e = skip_parens(ast, e);
        let n = ast.exprs.get(e);
        if n.kind() == ExprKind::ImplicitCastExpr
            && matches!(ast.exprs.cast_kind(e), CastKind::NoOp | CastKind::LValueToRValue)
        {
            e = ast.exprs.operand(e);
            continue;
        }
        return e;
    }
}

/// The `bool` literal statement `s` returns (`return true;`, alone or in
/// braces).
fn returns_bool(ast: &Ast, s: StmtId) -> Option<bool> {
    let r = only(ast, s)?;
    if ast.stmts.get(r).kind() != StmtKind::ReturnStmt {
        return None;
    }
    let v = bare(ast, ast.stmts.return_value(r)?);
    (ast.exprs.get(v).kind() == ExprKind::CXXBoolLiteralExpr).then(|| ast.exprs.bool_value(v))
}

impl Compiler<'_> {
    /// Whether `d` is a library wrapper of the evaluation context
    /// (spec/oven.md "Library fast paths"): a function of namespace `std`
    /// without parameters returning `bool`, whose definition is one
    /// `if consteval` (or `if !consteval`) returning a different `bool`
    /// literal from each branch, or one `return
    /// __builtin_is_constant_evaluated();` (not in `std::is_constant_evaluated`
    /// itself, whose builtin call warns, `-Wconstant-evaluated`). `Some(neg)`:
    /// the call's value is the context, negated when `neg`.
    pub(super) fn ice_wrapper(&self, d: DeclId) -> Option<bool> {
        let ast = self.ast();
        if !in_std(ast, d) {
            return None;
        }
        let def = ast.definition(d)?;
        let f = ast.decls.function(def)?;
        if !ast.lists.get(f.params).is_empty() {
            return None;
        }
        let ret = match ast.types.canonical_view(ast.decls.get(def).ty()?) {
            kiln_ast::types::TypeView::Function(p) => p.ret(),
            _ => return None,
        };
        if ast.types.builtin(ret) != Some(BuiltinType::Bool) {
            return None;
        }
        let s = only(ast, f.body?)?;
        match ast.stmts.get(s).kind() {
            StmtKind::IfStmt => {
                let kind = ast.stmts.if_kind(s);
                let x = ast.stmts.if_data(s);
                if !matches!(kind, IfKind::Consteval | IfKind::NotConsteval)
                    || x.init.is_some()
                    || x.cond_var.is_some()
                {
                    return None;
                }
                let (t, e) = (returns_bool(ast, x.then?)?, returns_bool(ast, x.els?)?);
                // `if consteval`: `t` in a constant context, `e` outside.
                (t != e).then_some(t == (kind == IfKind::NotConsteval))
            }
            StmtKind::ReturnStmt => {
                let v = bare(ast, ast.stmts.return_value(s)?);
                if is_named(ast, d, "is_constant_evaluated")
                    || ast.exprs.get(v).kind() != ExprKind::CallExpr
                    || !ast.lists.get(ast.exprs.call_args(v)).is_empty()
                {
                    return None;
                }
                let mut c = skip_parens(ast, ast.exprs.callee(v));
                while ast.exprs.get(c).kind().is_cast() {
                    c = skip_parens(ast, ast.exprs.operand(c));
                }
                (ast.exprs.get(c).kind() == ExprKind::DeclRefExpr
                    && ast.decls.builtin_id(ast.exprs.decl(c))
                        == Some(kiln_ast::decl::builtin::IS_CONSTANT_EVALUATED))
                .then_some(false)
            }
            _ => None,
        }
    }

    /// The `bi::TRAITS` sub and element type of a call of `d` with
    /// `nargs` arguments and no object argument, when `d` is
    /// `std::char_traits<C>::compare(const C*, const C*, size_t)`,
    /// `length(const C*)` or `find(const C*, size_t, const C&)` for a
    /// character type `C` (`char`, `wchar_t`, `char8_t`, `char16_t`,
    /// `char32_t`): the standard fixes what each computes
    /// ([char.traits.require]), so the library's definition need not be
    /// read.
    pub(super) fn traits_fast(&mut self, d: DeclId, nargs: usize) -> Option<(u32, OTy)> {
        let ast = self.ast();
        let rec = ast.decls.get(d).ctx()?;
        if !is_std_named(ast, rec, "char_traits") {
            return None;
        }
        let kind = [
            ("compare", bi::TR_COMPARE, 3),
            ("length", bi::TR_LENGTH, 1),
            ("find", bi::TR_FIND, 3),
        ]
        .into_iter()
        .find(|&(n, _, k)| k == nargs && is_named(ast, d, n))
        .map(|(_, k, _)| k)?;
        let f = ast.decls.function(ast.first(d))?;
        let params = ast.lists.get(f.params);
        if params.len() != nargs {
            return None;
        }
        let p0 = ast.decls.get(DeclId::new(params[0])).ty()?;
        let elem = ast.types.pointee(p0).filter(|_| !ast.types.is_reference(p0))?;
        let unsigned = match ast.types.builtin(elem)? {
            BuiltinType::Char | BuiltinType::Char8 => bi::TR_UNSIGNED,
            BuiltinType::WChar | BuiltinType::Char16 | BuiltinType::Char32 => 0,
            _ => return None,
        };
        let et = self.ty(elem.without_quals());
        Some((bi::TRAITS | (kind | unsigned) << 8, et))
    }
}
