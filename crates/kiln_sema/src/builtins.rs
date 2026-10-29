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
        let attrs = [
            self.ast.attrs.push(implicit(AttrKind::BUILTIN, Loc::INVALID, id)),
            self.ast.attrs.push(implicit(AttrKind::NOTHROW, loc, 0)),
        ];
        let _ = self.builder().attach_attrs(f, &attrs);
        Some(f)
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
