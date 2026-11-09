//! `[[clang::musttail]]` on a return statement (clang
//! `Sema::checkAndRewriteMustTailAttr` / `checkMustTailAttr`, `AttrDocs`
//! "musttail"; WP-809): the returned call must be lowered as a guaranteed
//! tail call, so caller and callee need matching signatures, calling
//! conventions and `this`-ness, and nothing may run after the call (no
//! non-trivial temporaries; the scopes left are checked by the jump-scope
//! pass, `jump_scope.rs`). A failed check drops the attribute.

use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::decl::{DeclKind, FnStorage};
use kiln_ast::expr::{ExprDeps, ExprKind, UnaryOp};
use kiln_ast::types::{FuncProto, TypeView};
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

/// clang's `FuncType::MemberType` (the select of
/// `err_musttail_member_mismatch`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Member {
    None = 0,
    Static = 1,
    NonStatic = 2,
    MemPtr = 3,
}

/// One side of the call (clang `FuncType`).
#[derive(Clone, Copy)]
struct Side {
    member: Member,
    /// The class of the implicit object (`None`: no `this`).
    this: Option<QualType>,
    proto: FuncProto,
}

impl Sema<'_, '_> {
    /// clang `checkAndRewriteMustTailAttr` for the `musttail` attribute
    /// `node` on return statement `ret`: false (after the diagnostics)
    /// when the attribute must be dropped. Skipped in a dependent context
    /// and for an instantiation-dependent operand (checked again at
    /// instantiation).
    /// The decl-specifier start of `d` (clang's `getBeginLoc`), where the
    /// callee notes point.
    fn musttail_begin_loc(&self, d: DeclId) -> Loc {
        self.ast.decls.cold.begin_loc.get(d).copied().unwrap_or(self.ast.decls.get(d).loc())
    }

    /// Whether the `musttail` on `ret` is valid; false after emitting the
    /// error (clang drops the attributed statement then).
    // One step per clang check, in clang's order (the first failing one
    // is the diagnostic).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn check_musttail(&mut self, ret: StmtId, node: &AttrNode) -> bool {
        let Some(caller) = self.func.as_ref().map(|f| f.decl) else { return true };
        let e = self.ast.stmts.return_value(ret);
        if self.in_template()
            || self.is_dependent_ctx(caller)
            || e.is_some_and(|e| self.ast.exprs.get(e).deps().contains(ExprDeps::INSTANTIATION))
        {
            return true;
        }
        let st_loc = self.ast.stmts.get(ret).loc();
        let name = self.attr_shown(node, "musttail");
        let required = |s: &Self| s.diag(DiagId::note_tail_call_required, node.loc).arg(&name);
        let Some(ce) = e.and_then(|e| self.musttail_call_of(e)) else {
            let msg = self.diag(DiagId::sema_musttail_needs_call, st_loc).arg(&name);
            self.emit(msg);
            return false;
        };
        let e = e.expect("a call was found");
        if self.ast.exprs.get(e).kind() == ExprKind::ExprWithCleanups
            && self.ast.exprs.cleanups_have_side_effects(e)
        {
            self.error(DiagId::sema_musttail_needs_trivial_args, st_loc);
            return false;
        }
        // The caller (clang: a `FunctionDecl` current context; kiln has no
        // blocks or Objective-C methods).
        let Some(src_side) = self.musttail_method_side(caller, false, st_loc, node, &name) else {
            return false;
        };
        let callee_decl = self.musttail_callee_decl(ce);
        let callee_loc = callee_decl.map_or(st_loc, |d| self.musttail_begin_loc(d));
        let exprs = &self.ast.exprs;
        let callee_expr = self.musttail_ignore_parens(exprs.callee(ce));
        let dst_side = if let Some(d) = callee_decl.filter(|&d| self.is_method_kind(d)) {
            let Some(s) = self.musttail_method_side(d, true, st_loc, node, &name) else {
                return false;
            };
            s
        } else if exprs.get(callee_expr).kind() == ExprKind::BinaryOperator
            && exprs.binary_op(callee_expr).is_ptr_mem()
        {
            let [_, rhs] = exprs.operands(callee_expr);
            let t = exprs.get(rhs).ty();
            let TypeView::MemberPointer { pointee, class } = self.ast.types.canonical_view(t)
            else {
                return true;
            };
            let Some(&proto) = self.ast.types.proto(pointee) else { return true };
            Side { member: Member::MemPtr, this: Some(QualType::new(class, 0)), proto }
        } else if exprs.get(callee_expr).kind() == ExprKind::CXXPseudoDestructorExpr {
            let msg = self
                .diag(DiagId::sema_musttail_structors_forbidden, st_loc)
                .arg(1)
                .arg(1)
                .with_note(required(self));
            self.emit(msg);
            return false;
        } else {
            let t = exprs.get(callee_expr).ty();
            let f = self.ast.types.pointee(t).unwrap_or(t);
            let Some(&proto) = self.ast.types.proto(f) else { return true };
            Side { member: Member::None, this: None, proto }
        };
        let named = callee_decl.map(|d| format!(" '{}'", self.decl_name(d)));
        let named_arg = named.clone().unwrap_or_default();
        let (cc_a, cc_b) = (src_side.proto.info().cc(), dst_side.proto.info().cc());
        if normalize_cc(cc_a) != normalize_cc(cc_b) {
            let msg = self
                .diag(DiagId::sema_musttail_callconv_mismatch, st_loc)
                .arg(&named_arg)
                .with_note(
                    self.diag(DiagId::note_musttail_callconv_mismatch, callee_loc)
                        .arg(cc_name(cc_a))
                        .arg(cc_name(cc_b)),
                )
                .with_note(required(self));
            self.emit(msg);
            return false;
        }
        if src_side.proto.variadic() || dst_side.proto.variadic() {
            let msg = self.diag(DiagId::sema_musttail_no_variadic, st_loc).arg(&name);
            self.emit(msg);
            return false;
        }
        if callee_decl.is_some_and(|d| self.has_cxx11_noreturn(d)) {
            let msg = self.diag(DiagId::sema_musttail_no_return, st_loc).arg(&name);
            self.emit(msg);
            return false;
        }
        if src_side.this.is_some() != dst_side.this.is_some() {
            let mut msg = self
                .diag(DiagId::sema_musttail_member_mismatch, st_loc)
                .arg(src_side.member as u32)
                .arg(dst_side.member as u32)
                .arg(&named_arg);
            if let Some(d) = callee_decl {
                let n = format!("'{}'", self.decl_name(d));
                msg = msg.with_note(self.diag(DiagId::note_entity_declared_at, callee_loc).arg(n));
            }
            let msg = msg.with_note(required(self));
            self.emit(msg);
            return false;
        }
        if let Some(note) = self.musttail_types_mismatch(&src_side, &dst_side, callee_loc) {
            let msg = self
                .diag(DiagId::sema_musttail_mismatch, st_loc)
                .arg(&named_arg)
                .with_note(note)
                .with_note(required(self));
            self.emit(msg);
            return false;
        }
        true
    }

    /// The call a `musttail` return value is (clang
    /// `IgnoreParenImplicitAsWritten`: parentheses, implicit casts, full
    /// expressions, materialized and bound temporaries).
    fn musttail_call_of(&self, mut e: ExprId) -> Option<ExprId> {
        let exprs = &self.ast.exprs;
        for _ in 0..256 {
            match exprs.get(e).kind() {
                ExprKind::ParenExpr
                | ExprKind::ImplicitCastExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::ConstantExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr => e = exprs.operand(e),
                ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXOperatorCallExpr => {
                    return Some(e);
                }
                _ => return None,
            }
        }
        None
    }

    fn musttail_ignore_parens(&self, mut e: ExprId) -> ExprId {
        for _ in 0..256 {
            if self.ast.exprs.get(e).kind() != ExprKind::ParenExpr {
                break;
            }
            e = self.ast.exprs.operand(e);
        }
        e
    }

    fn musttail_ignore_paren_imp_casts(&self, mut e: ExprId) -> ExprId {
        for _ in 0..256 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ParenExpr | ExprKind::ImplicitCastExpr => e = self.ast.exprs.operand(e),
                _ => break,
            }
        }
        e
    }

    /// clang `Expr::getReferencedDeclOfCallee` of call `ce`.
    fn musttail_callee_decl(&self, ce: ExprId) -> Option<DeclId> {
        let exprs = &self.ast.exprs;
        let mut e = self.musttail_ignore_paren_imp_casts(exprs.callee(ce));
        for _ in 0..256 {
            match exprs.get(e).kind() {
                ExprKind::SubstNonTypeTemplateParmExpr => {
                    e = self.musttail_ignore_paren_imp_casts(exprs.subst_replacement(e));
                }
                ExprKind::BinaryOperator if exprs.binary_op(e).is_ptr_mem() => {
                    e = self.musttail_ignore_paren_imp_casts(exprs.operands(e)[1]);
                }
                ExprKind::UnaryOperator
                    if matches!(
                        exprs.unary_op(e),
                        UnaryOp::Deref | UnaryOp::AddrOf | UnaryOp::Plus
                    ) =>
                {
                    e = self.musttail_ignore_paren_imp_casts(exprs.operand(e));
                }
                ExprKind::DeclRefExpr | ExprKind::MemberExpr => return Some(exprs.decl(e)),
                _ => return None,
            }
        }
        None
    }

    pub(crate) fn is_method_kind(&self, d: DeclId) -> bool {
        matches!(
            self.ast.decls.get(d).kind(),
            DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        )
    }

    /// clang's `GetMethodType` lambda (and the plain-function case) for
    /// function `f`: `None` after the diagnostics for a constructor or
    /// destructor.
    fn musttail_method_side(
        &mut self,
        f: DeclId,
        is_callee: bool,
        st_loc: Loc,
        node: &AttrNode,
        name: &str,
    ) -> Option<Side> {
        let kind = self.ast.decls.get(f).kind();
        if matches!(kind, DeclKind::CXXConstructor | DeclKind::CXXDestructor) {
            let dtor = u32::from(kind == DeclKind::CXXDestructor);
            let mut msg = self
                .diag(DiagId::sema_musttail_structors_forbidden, st_loc)
                .arg(u32::from(is_callee))
                .arg(dtor);
            if is_callee {
                let loc = self.musttail_begin_loc(f);
                msg = msg
                    .with_note(self.diag(DiagId::note_musttail_structors_forbidden, loc).arg(dtor));
            }
            let msg = msg.with_note(self.diag(DiagId::note_tail_call_required, node.loc).arg(name));
            self.emit(msg);
            return None;
        }
        let fty = self.ast.decls.get(f).ty()?;
        let proto = *self.ast.types.proto(fty)?;
        if !self.is_method_kind(f) {
            return Some(Side { member: Member::None, this: None, proto });
        }
        let is_static =
            self.ast.decls.function(f).is_some_and(|x| x.spec.storage() == FnStorage::Static);
        if is_static {
            return Some(Side { member: Member::Static, this: None, proto });
        }
        let class = self.ast.decls.get(f).ctx()?;
        let this = self.this_class_type(class);
        Some(Side { member: Member::NonStatic, this: Some(this), proto })
    }

    /// `[[noreturn]]` (not the GNU spelling) on a declaration of `d`
    /// (clang `hasAttr<CXX11NoReturnAttr>`).
    fn has_cxx11_noreturn(&self, d: DeclId) -> bool {
        let ast = &self.ast;
        ast.redecls_of(d).any(|r| {
            ast.lists.iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r)).any(|a| {
                let n = ast.attrs.get(a);
                n.kind == AttrKind::NORETURN && !n.is_gnu_spelling()
            })
        })
    }

    /// clang `ASTContext::hasSimilarType`: equal after removing the cv
    /// qualifiers at every level of a pointer / member pointer / array
    /// chain.
    fn musttail_similar(&self, mut a: QualType, mut b: QualType) -> bool {
        let t = &self.ast.types;
        for _ in 0..256 {
            if t.same_unqualified(a, b) {
                return true;
            }
            match (t.canonical_view(a), t.canonical_view(b)) {
                (TypeView::Pointer(x), TypeView::Pointer(y))
                | (TypeView::IncompleteArray(x), TypeView::IncompleteArray(y)) => (a, b) = (x, y),
                (
                    TypeView::MemberPointer { pointee: x, class: c },
                    TypeView::MemberPointer { pointee: y, class: d },
                ) if c == d => (a, b) = (x, y),
                (TypeView::ConstantArray(x), TypeView::ConstantArray(y)) if x.size == y.size => {
                    (a, b) = (x.elem, y.elem);
                }
                _ => return false,
            }
        }
        false
    }

    /// clang's `CheckTypesMatch`: the `note_musttail_mismatch` note at
    /// `loc` for the first difference, if any (types printed without
    /// their top-level qualifiers).
    fn musttail_types_mismatch(
        &mut self,
        src: &Side,
        dst: &Side,
        loc: Loc,
    ) -> Option<kiln_diag::Diag> {
        let found = self.musttail_first_mismatch(src, dst)?;
        let (id, a, b, ord) = match found {
            Mismatch::Arity(na, nb) => {
                return Some(self.diag(DiagId::note_musttail_mismatch_arity, loc).arg(na).arg(nb));
            }
            Mismatch::Types(id, a, b, ord) => (id, a, b, ord),
        };
        let a = self.ast.types.unqualified(a);
        let b = self.ast.types.unqualified(b);
        let (x, y) = self.ty_arg_pair(a, b);
        let msg = self.diag(id, loc);
        let msg = match ord {
            Some(i) => msg.arg(crate::expr::ordinal(i)),
            None => msg,
        };
        Some(msg.arg(x).arg(y))
    }

    fn musttail_first_mismatch(&self, src: &Side, dst: &Side) -> Option<Mismatch> {
        if let (Some(a), Some(b)) = (src.this, dst.this)
            && !self.musttail_similar(a, b)
        {
            return Some(Mismatch::Types(DiagId::note_musttail_mismatch_class, a, b, None));
        }
        let (ra, rb) = (src.proto.ret(), dst.proto.ret());
        if !self.musttail_similar(ra, rb) {
            return Some(Mismatch::Types(DiagId::note_musttail_mismatch_return, ra, rb, None));
        }
        let (na, nb) = (src.proto.num_params(), dst.proto.num_params());
        if na != nb {
            return Some(Mismatch::Arity(na, nb));
        }
        let t = &self.ast.types;
        let pa = t.params(&src.proto);
        let pb = t.params(&dst.proto);
        pb.iter().zip(pa).enumerate().find(|&(_, (&a, &b))| !self.musttail_similar(a, b)).map(
            |(i, (&a, &b))| {
                Mismatch::Types(DiagId::note_musttail_mismatch_param, a, b, Some(i + 1))
            },
        )
    }
}

/// The first difference `CheckTypesMatch` finds.
enum Mismatch {
    Arity(u32, u32),
    /// The note, the two types, the parameter's ordinal.
    Types(DiagId, QualType, QualType, Option<usize>),
}

fn normalize_cc(cc: kiln_ast::types::CallConv) -> kiln_ast::types::CallConv {
    use kiln_ast::types::CallConv;
    if cc == CallConv::C { CallConv::Default } else { cc }
}

/// clang `FunctionType::getNameForCallConv`.
fn cc_name(cc: kiln_ast::types::CallConv) -> &'static str {
    use kiln_ast::types::CallConv;
    match cc {
        CallConv::C | CallConv::Default => "cdecl",
        cc => cc.attribute().unwrap_or("cdecl"),
    }
}
