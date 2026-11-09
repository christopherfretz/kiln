//! clang's `require_constant_initialization` attribute (WP-954; spec/sema.md
//! §4.5): `__attribute__((require_constant_initialization))` /
//! `[[clang::require_constant_initialization]]`, the attribute form of
//! clang's `ConstInitAttr` that predates `constinit` ([dcl.constinit]).
//!
//! It applies to a variable with static or thread storage duration (clang
//! `SubjectList<[GlobalVar], ErrorDiag>`); its initializing declaration
//! has a constant initializer ([basic.start.static]/2), else clang's
//! `variable does not have a constant initializer` with `required by
//! 'require_constant_initialization' attribute here` and the evaluation's
//! notes (`CheckCompleteVariableDeclaration`). Unlike `constinit`, it may
//! be added after the first declaration, but not after the initializing
//! one: that is a warning and the attribute is dropped
//! (`mergeDeclAttributes`).

use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{CastKind, ExprKind};
use kiln_ast::{DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_oven::EvalContext;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::class::SpecialKind;

impl Sema<'_, '_> {
    /// Whether variable `v` has static or thread storage duration (clang
    /// `VarDecl::hasGlobalStorage`).
    pub(crate) fn has_global_storage(&self, v: DeclId) -> bool {
        self.ast.decls.var(v).is_some()
            && self.kind_of(v) != DeclKind::ParmVar
            && !self.has_local_storage(v)
    }

    /// The subject and redeclaration checks of a `require_constant_initialization`
    /// attribute `node` on declaration `d` of kind `kind`; false drops it.
    pub(crate) fn check_require_constant_init_attr(
        &mut self,
        d: DeclId,
        kind: DeclKind,
        node: AttrNode,
    ) -> bool {
        if !matches!(kind, DeclKind::Var | DeclKind::VarTemplateSpecialization)
            || !self.has_global_storage(d)
        {
            let shown = self.attr_shown(&node, "require_constant_initialization");
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type_error, node.loc)
                .arg(format!("'{shown}'"))
                .arg("global variables");
            self.emit(msg);
            return false;
        }
        // clang `mergeDeclAttributes`: the first time the attribute is
        // seen, a previous initializing declaration makes it too late.
        let earlier: Vec<DeclId> = self.ast.redecls_of(d).filter(|&r| r != d).take(4096).collect();
        if earlier.is_empty()
            || earlier.iter().any(|&r| {
                self.find_attr(r, AttrKind::REQUIRE_CONSTANT_INIT).is_some()
                    || self.constinit_locs.contains_key(&r)
            })
        {
            return true;
        }
        // clang `VarDecl::getInitializingDeclaration`: the one with an
        // initializer, else the definition.
        let with_init = |s: &Self, r: DeclId| s.ast.decls.var(r).is_some_and(|x| x.init.is_some());
        let Some(init) = earlier
            .iter()
            .copied()
            .find(|&r| with_init(self, r))
            .or_else(|| earlier.iter().copied().find(|&r| self.is_var_definition(r)))
        else {
            return true;
        };
        let note = self
            .diag(DiagId::note_require_constant_init_missing_here, self.ast.decls.get(init).loc());
        let msg =
            self.diag(DiagId::sema_require_const_init_added_too_late, node.loc).with_note(note);
        self.emit(msg);
        false
    }

    /// Where the `require_constant_initialization` attribute governing
    /// variable `v` was written: on a declaration of its entity, or of the
    /// pattern it was instantiated from.
    pub(crate) fn require_constant_init_of(&self, v: DeclId) -> Option<Loc> {
        let mut x = v;
        for _ in 0..64 {
            if let Some(a) = self.find_attr(self.ast.latest(x), AttrKind::REQUIRE_CONSTANT_INIT) {
                return Some(self.ast.attrs.get(a).loc);
            }
            x = self.ast.instantiated_from(x)?;
        }
        None
    }

    /// The initializing declaration of `require_constant_initialization`
    /// variable `v` has a constant initializer ([basic.start.static]/2;
    /// clang `CheckCompleteVariableDeclaration`, the `ConstInitAttr`
    /// branch): evaluated as for `constinit`, the error at the variable
    /// (no lone note is folded), then the attribute's note, then the
    /// evaluation's. Before C++11 clang checks the initializer
    /// syntactically and gives one `subexpression not valid` note; kiln
    /// evaluates it and points that note where the evaluation failed.
    /// Whether `v` has the attribute and a constant initializer.
    pub(crate) fn check_require_constant_init(&mut self, v: DeclId) -> bool {
        let Some(at) = self.require_constant_init_of(v) else { return false };
        let vn = *self.ast.decls.get(v);
        if vn.has(DeclFlags::INVALID)
            || !self.has_global_storage(v)
            || self.ast.decls.var(v).is_some_and(|x| x.spec.is_constexpr())
        {
            return false;
        }
        let Some(e) = self.ast.decls.var(v).and_then(|x| x.init) else { return false };
        if self.has_errors_in(e)
            || self.expr_is_value_dependent(e)
            || vn.ty().is_some_and(|t| self.is_dependent_type(t))
        {
            return false;
        }
        let loc = vn.loc();
        let note = self.diag(DiagId::note_require_constant_init_here, at);
        let msg = self.diag(DiagId::sema_constinit_non_constant, loc).with_note(note);
        if self.std < kiln_lex::Std::Cxx11 {
            let is_ref = vn.ty().is_some_and(|t| self.ast.types.is_reference(t));
            let full_ok = self.eval_const(e, EvalContext::init(v, loc)).is_ok();
            let Some(c) = self.cxx03_culprit(e, is_ref, full_ok, 0) else { return true };
            let msg = msg.note("subexpression not valid in a constant expression", Span::empty(c));
            self.emit(msg);
            return false;
        }
        let Err(f) = self.eval_const(e, EvalContext::init(v, loc)) else { return true };
        if f.kind == kiln_oven::FailKind::Destruction {
            return true;
        }
        let msg = Self::with_failure_notes(msg, f);
        self.emit(msg);
        false
    }

    /// clang `Expr::isConstantInitializer`, the C++03 check of a constant
    /// initializer: the location of the subexpression that is not one
    /// (its `getExprLoc`), or None. Aggregates, trivial default and copy
    /// construction and the no-op conversions are walked; any other
    /// expression is constant if it folds (`for_ref`: if the whole
    /// reference binding evaluated, `full_ok`).
    fn cxx03_culprit(
        &mut self,
        e: ExprId,
        for_ref: bool,
        full_ok: bool,
        depth: u32,
    ) -> Option<Loc> {
        if depth > 256 {
            return None;
        }
        let n = *self.ast.exprs.get(e);
        let kind = n.kind();
        if matches!(kind, ExprKind::ExprWithCleanups | ExprKind::ConstantExpr) {
            return self.cxx03_culprit(self.ast.exprs.operand(e), for_ref, full_ok, depth + 1);
        }
        if kind == ExprKind::MaterializeTemporaryExpr {
            return self.cxx03_culprit(self.ast.exprs.operand(e), false, full_ok, depth + 1);
        }
        if for_ref {
            return (!full_ok).then(|| self.culprit_loc(e));
        }
        let walk = |s: &mut Self, x: ExprId| s.cxx03_culprit(x, false, full_ok, depth + 1);
        match kind {
            ExprKind::ParenExpr | ExprKind::CXXBindTemporaryExpr => {
                return walk(self, self.ast.exprs.operand(e));
            }
            ExprKind::ImplicitValueInitExpr => return None,
            ExprKind::InitListExpr => {
                let list = self.ast.exprs.init_list_inits(e);
                let inits: Vec<ExprId> = self.ast.lists.iter::<ExprId>(list).collect();
                for x in inits {
                    if let Some(c) = walk(self, x) {
                        return Some(c);
                    }
                }
                return None;
            }
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                let ctor = self.ast.exprs.constructor(e);
                let args: Vec<ExprId> =
                    self.ast.lists.iter::<ExprId>(self.ast.exprs.construct_args(e)).collect();
                if self.is_trivial_ctor(ctor) {
                    return match args[..] {
                        [] => None,
                        [a] => walk(self, a),
                        _ => Some(self.culprit_loc(e)),
                    };
                }
            }
            k if k.is_cast()
                && matches!(
                    self.ast.exprs.cast_kind(e),
                    CastKind::NoOp
                        | CastKind::LValueToRValue
                        | CastKind::ConstructorConversion
                        | CastKind::NullToPointer
                ) =>
            {
                return walk(self, self.ast.exprs.operand(e));
            }
            _ => {}
        }
        if self.try_fold(e).is_some() { None } else { Some(self.culprit_loc(e)) }
    }

    /// Whether constructor `c` is a trivial default, copy or move
    /// constructor of a class with a trivial destructor.
    fn is_trivial_ctor(&self, c: DeclId) -> bool {
        let Some(k) = self.special_kind_of(c) else { return false };
        let Some(r) = self.ast.decls.get(c).ctx() else { return false };
        matches!(k, SpecialKind::DefaultCtor | SpecialKind::CopyCtor | SpecialKind::MoveCtor)
            && self.has_trivial(r, k)
            && self.has_trivial(r, SpecialKind::Dtor)
    }
}
