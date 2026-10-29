//! Builtin functions (spec/sema.md §5.4): clang's `LazilyCreateBuiltin`.
//!
//! A builtin function is declared the first time an unqualified lookup of
//! its name finds nothing: an implicit `extern "C"` `Function` decl in the
//! translation unit (inside an implicit `LinkageSpec`, as clang dumps it)
//! carrying its builtin id in the `builtin_id` cold data, an implicit
//! `BuiltinAttr` and, for the `noexcept` ones, an implicit `NoThrowAttr`.
//! Its name has the placeholder type `<builtin fn type>`; a call converts
//! it with `BuiltinFnToFnPtr` ([`Sema::act_on_call`]), and any other use is
//! an error.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{DeclFlags, DeclKind, FnSpec, FnStorage, LinkageLang, builtin};
use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::names::DeclName;
use kiln_ast::types::{ExcSpec, FuncInfo};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::conv;

impl Sema<'_, '_> {
    /// Declares the builtin function `name` (first used at `loc`) if it
    /// is one kiln knows.
    pub(crate) fn lazily_create_builtin(&mut self, name: Symbol, loc: Loc) -> Option<DeclId> {
        let id = builtin::by_name(self.ast.symbols().as_str(name))?;
        let ty = match id {
            builtin::IS_CONSTANT_EVALUATED => self.ast.types.function(
                QualType::BOOL,
                &[],
                FuncInfo::new(),
                ExcSpec::BasicNoexcept,
            ),
            builtin::ADDRESSOF => {
                let vref = self.ast.types.lvalue_ref_to(QualType::VOID);
                let vp = self.ast.types.pointer_to(QualType::VOID);
                self.ast.types.function(vp, &[vref], FuncInfo::new(), ExcSpec::BasicNoexcept)
            }
            _ => return None,
        };
        let tu = self.ast.tu;
        let lh = DeclHead::new(tu, DeclName::EMPTY, loc).flags(DeclFlags::IMPLICIT);
        let ls = self.builder().linkage_spec_head(lh, LinkageLang::C);
        let h = DeclHead::new(ls, DeclName::ident(name), loc).flags(DeclFlags::IMPLICIT);
        let spec = FnSpec::new().with_storage(FnStorage::Extern);
        let f = self.builder().function(h, DeclKind::Function, ty, spec, None);
        let _ = self.ast.decls.cold.builtin_id.insert(f, id);
        let implicit =
            |kind, loc, a| AttrNode { kind, syntax: AttrSyntax::Gnu, flags: 1, loc, a, b: 0 };
        let mut attrs = vec![
            self.ast.attrs.push(implicit(AttrKind::BUILTIN, Loc::INVALID, id)),
            self.ast.attrs.push(implicit(AttrKind::NOTHROW, loc, 0)),
        ];
        if id == builtin::ADDRESSOF {
            // clang: an implicit `[[clang::lifetimebound]]` `void &`
            // parameter and `__attribute__((const))`.
            let pty = self.ast.types.lvalue_ref_to(QualType::VOID);
            let ph = DeclHead::new(f, DeclName::EMPTY, Loc::INVALID).flags(DeclFlags::IMPLICIT);
            let p = self.builder().param_head(ph, pty, 0, 0);
            let lb = self.ast.attrs.push(implicit(AttrKind::LIFETIMEBOUND, loc, 0));
            let _ = self.builder().attach_attrs(p, &[lb]);
            let _ = self.builder().set_params(f, &[p]);
            attrs.push(self.ast.attrs.push(implicit(AttrKind::CONST, loc, 0)));
        }
        let _ = self.builder().attach_attrs(f, &attrs);
        Some(f)
    }

    /// A call of `__builtin_addressof` (clang's custom type-checking,
    /// `SemaBuiltinAddressof`): exactly one argument, which must be an
    /// lvalue that is not a bit-field ([specialized.addressof]); the type
    /// is a pointer to the argument's type and the argument is not
    /// converted. `None` after a diagnostic.
    pub(crate) fn builtin_addressof_call(
        &mut self,
        callee: ExprId,
        b: DeclId,
        args: &[ExprId],
        braced: bool,
        loc: Loc,
        rparen: Loc,
    ) -> Option<ExprId> {
        let at = self.begin_loc(callee);
        match args {
            [] => {
                let msg = self.diag(DiagId::sema_call_too_few_args, rparen).arg(1).arg(0);
                self.emit(msg);
                return None;
            }
            [_] => {}
            [_, extra, ..] => {
                let msg = self
                    .diag(DiagId::sema_call_too_many_args, self.begin_loc(*extra))
                    .arg(1)
                    .arg(args.len());
                self.emit(msg);
                return None;
            }
        }
        if braced {
            self.unsupported("a braced argument to __builtin_addressof", loc);
            return None;
        }
        let ty = self.check_addressable(args[0], at)?;
        let f = self.builtin_fn_to_ptr(callee, b);
        let ty = self.ast.types.pointer_to(ty);
        Some(self.builder().call(
            f,
            &[args[0]],
            ty,
            kiln_ast::expr::ValueCategory::PRValue,
            loc,
            rparen,
            kiln_ast::expr::ExprFlags::NONE,
        ))
    }

    /// The builtin function `callee` (through parentheses) names, if it
    /// names one.
    pub(crate) fn builtin_callee(&self, callee: ExprId) -> Option<DeclId> {
        let x = self.ignore_parens(callee);
        let n = self.ast.exprs.get(x);
        if n.kind() != ExprKind::DeclRefExpr {
            return None;
        }
        let d = self.ast.exprs.decl(x);
        self.ast.decls.builtin_id(d).map(|_| d)
    }

    /// The callee of a call to the builtin `d`: its name converted to a
    /// pointer to its function type (clang's `BuiltinFnToFnPtr`).
    pub(crate) fn builtin_fn_to_ptr(&mut self, callee: ExprId, d: DeclId) -> ExprId {
        let fty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let p = self.ast.types.pointer_to(fty);
        conv::raw_cast(
            self,
            CastKind::BuiltinFnToFnPtr,
            callee,
            p,
            kiln_ast::expr::ValueCategory::PRValue,
            None,
        )
    }

    /// Whether `e` is a builtin function's name used other than as a
    /// callee; reports `builtin functions must be directly called`.
    pub(crate) fn check_builtin_fn_use(&mut self, e: ExprId) -> bool {
        if self.ast.exprs.get(e).ty() == QualType::BOUND_MEMBER {
            // [expr.ref]/7.3.2: a bound member function can only be called.
            self.diagnose_bound_member(e);
            return true;
        }
        if self.ast.exprs.get(e).ty() != QualType::BUILTIN_FN {
            return false;
        }
        let msg = self.diag(DiagId::sema_builtin_fn_use, self.eloc(e));
        self.emit(msg);
        true
    }
}

impl Sema<'_, '_> {
    /// `__builtin_bit_cast(ty, e)` at `loc` ([bit.cast]; clang
    /// `BuildBuiltinBitCastExpr` and `CastOperation::CheckBuiltinBitCast`):
    /// both types complete, a prvalue operand materialized, equal sizes,
    /// and both types trivially copyable. The result is a prvalue of `ty`
    /// (cv-unqualified for a non-class type) with cast kind
    /// `LValueToRValueBitCast`. Evaluating it is an Oven stub
    /// (spec/sema-templates.md §9.5).
    pub fn act_on_builtin_bit_cast(
        &mut self,
        ty: QualType,
        e: ExprId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        use kiln_ast::expr::{ExprFlags, StorageDuration, ValueCategory};
        let _ = rparen;
        if self.is_err_expr(e) || self.is_error_type(ty) {
            return self.recover(self.non_reference(ty), &[e], loc);
        }
        if self.ast.types.is_dependent(ty) || self.expr_is_type_dependent(e) {
            let to = self.non_reference(ty);
            return self.builder().cast(
                ExprKind::BuiltinBitCastExpr,
                CastKind::Dependent,
                e,
                to,
                ValueCategory::PRValue,
                None,
                loc,
                ExprFlags::NONE,
            );
        }
        let dest = self.non_reference(ty);
        if !self.require_complete(dest, loc) {
            let msg = self.diag(DiagId::sema_cast_to_incomplete, loc).arg(self.ty_arg(dest));
            self.emit(self.incomplete_note(msg, dest));
            return self.recover_err(&[e], loc);
        }
        let src = self.ety(e);
        if !self.require_complete(src, loc) {
            let msg = self.diag(DiagId::sema_incomplete_type_required, loc).arg(self.ty_arg(src));
            self.emit(self.incomplete_note(msg, src));
            return self.recover_err(&[e], loc);
        }
        let mut sub = e;
        if self.ast.exprs.get(e).value_category() == ValueCategory::PRValue {
            // clang's `CreateMaterializeTemporaryExpr` (its full-expression
            // needs cleanups, without side effects).
            sub = self.builder().materialize_temporary(
                e,
                src,
                false,
                StorageDuration::FullExpression,
                None,
            );
            self.cleanup.needs = true;
        }
        // A reference destination has the size of a pointer (clang's
        // `getTypeSizeInChars` of a reference type).
        let size_of = |s: &mut Self, t: QualType| {
            let t = if s.ast.types.is_reference(t) { s.ast.types.pointer_to(t) } else { t };
            crate::target::size_align(&s.ast, t, &s.abi).map_or(0, |x| x.0)
        };
        let (ss, ds) = (size_of(self, src), size_of(self, ty));
        if ss != ds {
            let msg = self.diag(DiagId::sema_bit_cast_size_mismatch, loc).arg(ss).arg(ds);
            self.emit(msg);
            return self.recover_err(&[sub], loc);
        }
        for (t, which) in [(ty, "1"), (src, "0")] {
            if self.ast.types.is_reference(t) || !self.is_trivially_copyable_type(t) {
                let msg = self.diag(DiagId::sema_bit_cast_not_trivially_copyable, loc).arg(which);
                self.emit(msg);
                return self.recover_err(&[sub], loc);
            }
        }
        let to = if self.ast.types.as_record(dest).is_some() || self.ast.types.is_array(dest) {
            dest
        } else {
            conv::strip_cv(self, dest)
        };
        self.builder().cast(
            ExprKind::BuiltinBitCastExpr,
            CastKind::LValueToRValueBitCast,
            sub,
            to,
            ValueCategory::PRValue,
            None,
            loc,
            ExprFlags::NONE,
        )
    }
}
