//! clang `Sema::checkTypeSupport`'s `long double` rule (WP-1804): on
//! x86-64 without `x87` (`-mno-x87`, `-mgeneral-regs-only`) the target
//! has no `long double` (clang `X86TargetInfo::handleTargetFeatures`
//! clears `HasLongDouble`), and each use of a declaration whose type is,
//! takes or returns `long double`, each definition of such a function and
//! each `long double` operand of a built-in binary operator is the error
//! `err_target_unsupported_type` (spec/sema.md "Target type support").

use kiln_ast::decl::DeclKind;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// x86-64: whether module feature `name` is off ([`Sema::x86_features`];
    /// the default `x86-64` map has `x87`, `mmx` and `sse` on).
    pub(crate) fn x86_feature_off(&self, name: &str) -> bool {
        self.abi.triple.arch == kiln_abi::target::Arch::X86_64
            && self.x86_features.iter().rev().find_map(|e| match e.split_at_checked(1) {
                Some((sign, f)) if f == name => Some(sign == "-"),
                _ => None,
            }) == Some(true)
    }

    /// How many `long double` types clang's `CheckType` finds in `q`: `q`
    /// itself, then a function type's parameters and its return type.
    fn long_double_parts(&self, q: QualType) -> usize {
        let t = &self.ast.types;
        let is_ld = |x: QualType| t.canonical(x).without_quals() == QualType::LONGDOUBLE;
        let mut n = usize::from(is_ld(q));
        if let TypeView::Function(p) = t.canonical_view(q) {
            n += t.params(&p).iter().filter(|&&x| is_ld(x)).count();
            n += usize::from(is_ld(p.ret()));
        }
        n
    }

    /// clang skips the check in a trivial copy or move constructor or
    /// assignment operator ("memcpy operations for structs containing a
    /// member with unsupported type are ok").
    fn in_trivial_copy(&self) -> bool {
        use crate::class::SpecialKind as K;
        let c = self.ctx;
        let Some(k) = self.special_kind_of(c) else { return false };
        let Some(r) = self.ast.decls.get(c).ctx() else { return false };
        matches!(k, K::CopyCtor | K::MoveCtor | K::CopyAssign | K::MoveAssign)
            && self.has_trivial(r, k)
    }

    /// clang `checkTypeSupport(q, loc, d)` for `long double`: one error per
    /// `long double` part of `q` at `loc`, naming `d` (with a note at it)
    /// or `expression`. Returns whether any was given. A declaration
    /// reported once is not reported again (clang makes it invalid, and an
    /// invalid declaration's uses raise no more errors).
    pub(crate) fn check_long_double(&mut self, q: QualType, loc: Loc, d: Option<DeclId>) -> bool {
        if !self.x86_feature_off("x87") || self.unevaluated > 0 || self.in_trivial_copy() {
            return false;
        }
        let n = self.long_double_parts(q);
        if n == 0 {
            return false;
        }
        let name = d.map(|d| format!("'{}'", self.name_for_diagnostic(d)));
        let triple = self.abi.triple.normalized();
        for _ in 0..n {
            let what = name.clone().unwrap_or_else(|| "expression".to_owned());
            let mut msg = self
                .diag(DiagId::sema_target_unsupported_type, loc)
                .arg(what)
                .arg("'long double'".to_owned())
                .arg(triple.clone());
            if let (Some(d), Some(name)) = (d, &name) {
                let at = self.ast.decls.get(d).loc();
                let note = self.diag(DiagId::note_entity_defined_here, at).arg(name.clone());
                msg = msg.with_note(note);
            }
            self.emit(msg.allow_duplicate());
        }
        if let Some(d) = d {
            let _ = self.long_double_reported.insert(d);
        }
        true
    }

    /// clang `DiagnoseUseOfDecl` -> `checkTypeSupport` for a use of `d` at
    /// `loc`. Returns true when the use is an error (reported now or
    /// before: clang's `CheckDeclInExpr` rejects the invalid declaration
    /// silently).
    pub(crate) fn check_long_double_use(&mut self, d: DeclId, loc: Loc) -> bool {
        if self.long_double_reported.contains(&d) {
            return self.x86_feature_off("x87") && self.unevaluated == 0;
        }
        let n = self.ast.decls.get(d);
        if matches!(n.kind(), DeclKind::Namespace | DeclKind::Typedef | DeclKind::TypeAlias) {
            return false;
        }
        match n.ty() {
            Some(t) => self.check_long_double(t, loc, Some(d)),
            None => false,
        }
    }

    /// The operand type-support checks of a builtin binary operator:
    /// SVE sizeless operands, then `long double` support.
    pub(crate) fn check_type_support_operands(&mut self, ops: [ExprId; 2], loc: Loc) {
        self.check_sve_operands(ops, loc);
        self.check_long_double_operands(ops, loc);
    }

    /// clang `CreateBuiltinBinOp`'s `checkTypeSupport` of each operand at
    /// the operator, with no declaration.
    pub(crate) fn check_long_double_operands(&mut self, ops: [ExprId; 2], loc: Loc) {
        for e in ops {
            if !self.is_err_expr(e) {
                let t = self.ety(e);
                self.check_long_double(t, loc, None);
            }
        }
    }
}
