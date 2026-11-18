//! Exposures by naming a TU-local entity ([basic.link]/14-17;
//! spec/modules.md §5 "Exposures"; WP-1678). The constexpr-value kind is
//! [`Sema::module_check_constexpr_exposure`].
//!
//! In the purview of a module interface unit or module partition (not
//! its private module fragment, not a header unit), a declaration of an
//! entity that is not TU-local is ill-formed when it names a TU-local
//! one outside the parts /15 ignores. kiln checks, at most once per
//! declaration, the subset GCC 16.1 rejects (clang 22 only warns,
//! `-WTU-local-entity-exposure`):
//! - a use ([`Sema::diagnose_deprecated`], clang's `DiagnoseUseOfDecl`)
//!   in the body of an inline function (written `inline`, `constexpr` or
//!   `consteval`, or a member defined in a class attached to the global
//!   module), also in a lambda there: the error is at the function (the
//!   lambda);
//! - the declared type of a function (return and parameter types), of a
//!   variable, or of a non-static data member (the error at its class),
//!   and a default argument, when written ([`Sema::module_check_decl_exposure`]).
//!
//! A TU-local entity is a function, variable, class, enumeration or
//! template with internal linkage, or a class template specialization
//! with a TU-local type argument ([basic.link]/16.1, 16.4). Not checked
//! (GCC 16.1 accepts each): an unevaluated operand, a discarded
//! statement, an enumerator, a variable that might be usable in constant
//! expressions (/15.4 when it is not odr-used), a function or variable
//! declared outside the purview (GCC 16.1 warns,
//! `-Wexpose-global-module-tu-local`), a template instantiation, a
//! friend declaration (/15.3).
//!
//! The initializer of a variable is not an exposure (/15.1.2; /17 makes
//! only a constexpr variable's TU-local value one, which
//! [`Sema::module_check_constexpr_exposure`] checks). For a use in the
//! initializer of an inline variable that is not constexpr, also in a
//! lambda there, kiln gives clang 22's warning at the use, "TU local
//! entity 'hidden' is exposed" (`-WTU-local-entity-exposure`, on by
//! default; not before `-fclang-compat=22`); GCC 16.1 rejects one
//! constant-initialized to a TU-local value.

use kiln_ast::decl::{ConstexprKind, DeclKind};
use kiln_ast::expr::{ExprKind, UnaryOp};
use kiln_ast::types::{ArgView, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::Phase;
use crate::Sema;

/// How many links a walk follows at most.
const MAX_WALK: usize = 1024;

/// A use [`Sema::module_note_use`] holds.
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
pub(crate) struct PendingExposure {
    /// The declaration it would make an exposure.
    owner: DeclId,
    /// Where that is reported.
    at: Loc,
    /// The TU-local entity named.
    entity: DeclId,
    /// The use.
    loc: Loc,
}

impl Sema<'_, '_> {
    /// Whether a declaration parsed now is checked for exposures: the
    /// purview of a module interface unit or module partition, outside
    /// its private module fragment ([basic.link]/17). False at once in a
    /// unit that is not a module unit.
    pub(crate) fn module_exposure_purview(&self) -> bool {
        let Some(f) = self.modules.frames.last() else { return false };
        f.phase == Phase::Purview
            && self.ast.modules.units.get(usize::from(f.unit)).is_some_and(|u| {
                u.kind.is_interface()
                    || u.kind == kiln_ast::modules::UnitKind::PartitionImplementation
            })
    }

    /// A use of `d` at `loc` (see the module docs) in a part of a
    /// declaration [basic.link]/15 considers: held until its
    /// full-expression ends ([`Sema::module_full_expr_exposures`]) or,
    /// outside one, until the declaration's end
    /// ([`Sema::module_flush_exposures`]).
    pub(crate) fn module_note_use(&mut self, d: DeclId, loc: Loc) {
        if self.modules.frames.is_empty()
            || self.unevaluated > 0
            || !self.inst.stack.frames.is_empty()
            || !self.module_exposure_purview()
            || self.in_discarded_statement()
        {
            return;
        }
        let Some((owner, at)) = self.exposure_owner() else { return };
        if !self.modules.exposed.contains(&owner) && self.is_tu_local_entity(d) {
            self.modules.pending_exposures.push(PendingExposure { owner, at, entity: d, loc });
        }
    }

    /// The end of full-expression `e`: a held use within it is dropped
    /// when `e` folds to a constant that is not a TU-local value (GCC
    /// 16.1 checks the folded body: `helper;`, `(void)&hidden;`,
    /// `0 ? helper() : 1` and `&hidden != nullptr` leave no name behind).
    pub(crate) fn module_full_expr_exposures(&mut self, e: ExprId) {
        if self.modules.pending_exposures.is_empty() {
            return;
        }
        let (b, end) = (self.ast.begin_loc(e).to_raw(), self.ast.end_loc(e).to_raw());
        let within = |p: &PendingExposure| (b..=end).contains(&p.loc.to_raw());
        if !self.modules.pending_exposures.iter().any(within) {
            return;
        }
        if self.modules.pending_exposures.iter().any(|p| within(p) && self.warn_only(p)) {
            // clang 22 warns for each use in an inline variable's
            // initializer, folded or not.
            return;
        }
        let folded = if self.ast.types.is_void(self.ast.exprs.get(e).ty()) {
            self.void_expr_folds(e)
        } else {
            self.fold_rvalue(e).is_some_and(|v| self.tu_local_referent(v).is_none())
        };
        if folded {
            self.modules.pending_exposures.retain(|p| !within(p));
        }
    }

    /// The end of expression statement `e` (a discarded-value
    /// expression): a held use within it is dropped when `e` is only a
    /// name or its address ([`Sema::module_full_expr_exposures`]).
    pub(crate) fn module_discarded_exposures(&mut self, e: ExprId) {
        if self.modules.pending_exposures.is_empty() || !self.void_expr_folds(e) {
            return;
        }
        let (b, end) = (self.ast.begin_loc(e).to_raw(), self.ast.end_loc(e).to_raw());
        let kept: Vec<bool> = self
            .modules
            .pending_exposures
            .iter()
            .map(|p| self.warn_only(p) || !(b..=end).contains(&p.loc.to_raw()))
            .collect();
        let mut i = 0;
        self.modules.pending_exposures.retain(|_| {
            i += 1;
            kept[i - 1]
        });
    }

    /// Whether held use `p` is in an inline variable's initializer: a
    /// warning, not an exposure (see the module docs).
    fn warn_only(&self, p: &PendingExposure) -> bool {
        self.kind_of(p.owner) == DeclKind::Var
    }

    /// Whether void expression `e` has no effect GCC 16.1 keeps: a name,
    /// its address, or a cast of either to `void`.
    fn void_expr_folds(&self, e: ExprId) -> bool {
        let mut x = e;
        for _ in 0..MAX_WALK {
            match self.ast.exprs.get(x).kind() {
                ExprKind::CStyleCastExpr
                | ExprKind::CXXStaticCastExpr
                | ExprKind::CXXFunctionalCastExpr
                | ExprKind::ImplicitCastExpr
                | ExprKind::ParenExpr
                | ExprKind::UnaryOperator => {
                    if self.ast.exprs.get(x).kind() == ExprKind::UnaryOperator
                        && self.ast.exprs.unary_op(x) != UnaryOp::AddrOf
                    {
                        return false;
                    }
                    let kids = self.ast.exprs.children(x, &self.ast.lists);
                    let [k] = kids.as_slice() else { return false };
                    x = *k;
                }
                ExprKind::DeclRefExpr => return true,
                _ => return false,
            }
        }
        false
    }

    /// The end of declaration `owner` (a function definition, an inline
    /// variable's initializer): its first held use is an exposure.
    pub(crate) fn module_flush_exposures(&mut self, owner: DeclId) {
        if self.modules.pending_exposures.is_empty() {
            return;
        }
        let mine: Vec<PendingExposure> =
            self.modules.pending_exposures.iter().copied().filter(|p| p.owner == owner).collect();
        self.modules.pending_exposures.retain(|p| p.owner != owner);
        if self.kind_of(owner) == DeclKind::Var {
            if self.clang_compat >= 22 {
                for p in mine {
                    let msg = self
                        .diag(DiagId::sema_module_tu_local_exposed, p.loc)
                        .arg(self.name_for_diagnostic(p.entity));
                    self.emit(msg);
                }
            }
            return;
        }
        if let Some(p) = mine.first()
            && !self.modules.exposed.contains(&owner)
        {
            self.report_exposure(owner, p.at, p.entity);
        }
    }

    /// The declaration a use at the current point belongs to, when the
    /// use is in a part [basic.link]/15 considers, and where it is
    /// reported: the innermost enclosing function that is not a lambda's
    /// call operator, if it is inline (at the outermost lambda between,
    /// if any); else, outside any function, the inline variable (not
    /// constexpr) being initialized, also from a lambda there.
    fn exposure_owner(&self) -> Option<(DeclId, Loc)> {
        let mut c = self.ctx;
        let mut lambda_at = None;
        for _ in 0..MAX_WALK {
            let k = self.kind_of(c);
            if k.is_function() {
                let parent = self.ast.decls.get(c).ctx()?;
                if self.is_lambda_call_op(c) {
                    lambda_at = Some(self.ast.decls.get(parent).loc());
                    c = parent;
                    continue;
                }
                let s = self.fn_spec(c);
                let inline = s.is_inline()
                    || s.implicit_inline()
                    || s.constexpr_kind() != ConstexprKind::None;
                if !inline || self.is_tu_local_entity(c) {
                    return None;
                }
                return Some((c, lambda_at.unwrap_or_else(|| self.ast.decls.get(c).loc())));
            }
            match k {
                DeclKind::CXXRecord if self.ast.closure_lambda(c).is_some() => {}
                DeclKind::LinkageSpec | DeclKind::Export => {}
                _ => break,
            }
            c = self.ast.decls.get(c).ctx()?;
        }
        let v = self.depr_init?;
        let inline =
            self.ast.decls.var(v).is_some_and(|x| x.spec.is_inline() && !x.spec.is_constexpr());
        if !inline || self.is_tu_local_entity(v) {
            return None;
        }
        Some((v, self.ast.decls.get(v).loc()))
    }

    /// [basic.link]/16 for what kiln checks (see the module docs).
    fn is_tu_local_entity(&self, d: DeclId) -> bool {
        let k = self.kind_of(d);
        match k {
            DeclKind::Var | DeclKind::VarTemplate => {
                if self.ast.decls.var(d).is_some() && self.might_be_constant_var(d) {
                    return false;
                }
            }
            DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization => {
                if self.has_internal_linkage(d) {
                    return true;
                }
                // [basic.link]/16.4: a specialization with a TU-local
                // type argument.
                return self.ast.spec_info(d).is_some_and(|(_, i)| {
                    let args = self.ast.types.canonical_args(i.args);
                    self.ast.types.args(args).to_vec().into_iter().any(|a| match a.view() {
                        ArgView::Type(t) => self.type_tu_local(t, 0).is_some(),
                        _ => false,
                    })
                });
            }
            DeclKind::Enum | DeclKind::ClassTemplate => {
                return self.has_internal_linkage(d);
            }
            k if k.is_function() => {}
            DeclKind::FunctionTemplate => {}
            _ => return false,
        }
        // A function or variable of the global module fragment: GCC 16.1
        // only warns (`-Wexpose-global-module-tu-local`).
        self.ast.modules.purview_module(d).is_some() && self.has_internal_linkage(d)
    }

    /// The TU-local class or enumeration type `ty` is built from, if any
    /// (through pointers, references, arrays, function types, member
    /// pointers and template arguments).
    fn type_tu_local(&self, ty: QualType, depth: usize) -> Option<DeclId> {
        if depth > 64 || self.is_error_type(ty) {
            return None;
        }
        let t = &self.ast.types;
        match t.canonical_view(ty) {
            TypeView::Pointer(p)
            | TypeView::LValueRef(p)
            | TypeView::RValueRef(p)
            | TypeView::IncompleteArray(p)
            | TypeView::Complex(p)
            | TypeView::Atomic(p) => self.type_tu_local(p, depth + 1),
            TypeView::ConstantArray(a) => self.type_tu_local(a.elem, depth + 1),
            TypeView::MemberPointer { pointee, class } => self
                .type_tu_local(QualType::unqual(class), depth + 1)
                .or_else(|| self.type_tu_local(pointee, depth + 1)),
            TypeView::Function(p) => std::iter::once(p.ret())
                .chain(t.params(&p).iter().copied())
                .find_map(|x| self.type_tu_local(x, depth + 1)),
            TypeView::Record(d) | TypeView::Enum(d) => self.is_tu_local_entity(d).then_some(d),
            _ => None,
        }
    }

    /// After `act_on_declarator` declared `d`: the declared type of a
    /// function, variable or non-static data member, and a function's
    /// default arguments (see the module docs). `friend` declarations are
    /// ignored ([basic.link]/15.3).
    pub(crate) fn module_check_decl_exposure(&mut self, d: DeclId, friend: bool) {
        if friend || !self.inst.stack.frames.is_empty() || !self.module_exposure_purview() {
            return;
        }
        let k = self.kind_of(d);
        let Some(ctx) = self.ast.decls.get(d).ctx() else { return };
        let owner = match k {
            DeclKind::Field if self.kind_of(ctx) == DeclKind::CXXRecord => ctx,
            DeclKind::Var => d,
            k if k.is_function() => d,
            _ => return,
        };
        if self.in_function_scope(ctx)
            || self.modules.exposed.contains(&owner)
            || self.is_tu_local_entity(owner)
        {
            return;
        }
        let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let mut found = self.type_tu_local(ty, 0);
        if found.is_none() && k.is_function() {
            found = self.default_args_tu_local(d);
        }
        if let Some(e) = found {
            self.report_exposure(owner, self.ast.decls.get(owner).loc(), e);
        }
    }

    /// Whether context `c` is a function or lies in one (a local class,
    /// a closure).
    fn in_function_scope(&self, c: DeclId) -> bool {
        let mut c = Some(c);
        for _ in 0..MAX_WALK {
            let Some(x) = c else { return false };
            let k = self.kind_of(x);
            if k.is_function() {
                return true;
            }
            if matches!(k, DeclKind::Namespace | DeclKind::TranslationUnit) {
                return false;
            }
            c = self.ast.decls.get(x).ctx();
        }
        false
    }

    /// The end of the definition of class `r`: a base class that is
    /// TU-local makes it an exposure.
    pub(crate) fn module_check_class_exposure(&mut self, r: DeclId) {
        if !self.inst.stack.frames.is_empty() || !self.module_exposure_purview() {
            return;
        }
        if self.ast.decls.get(r).ctx().is_none_or(|c| self.in_function_scope(c))
            || self.modules.exposed.contains(&r)
            || self.is_tu_local_entity(r)
        {
            return;
        }
        let bases: Vec<QualType> = self.ast.decls.bases(r, &self.ast.lists).map(|b| b.ty).collect();
        if let Some(e) = bases.into_iter().find_map(|t| self.type_tu_local(t, 0)) {
            self.report_exposure(r, self.ast.decls.get(r).loc(), e);
        }
    }

    /// The TU-local function or variable a default argument of function
    /// `f` names, if any.
    fn default_args_tu_local(&self, f: DeclId) -> Option<DeclId> {
        let params = self.ast.decls.function(f)?.params;
        for p in self.ast.lists.iter::<DeclId>(params) {
            let Some(e) = self.ast.decls.param(p).and_then(|x| x.default_arg) else { continue };
            let mut stack = self.take_walk_stack(e);
            let mut found = None;
            let mut n = 0usize;
            while let Some(x) = stack.pop() {
                n += 1;
                if n > 100_000 {
                    break;
                }
                let node = self.ast.exprs.get(x);
                if node.kind() == ExprKind::DeclRefExpr {
                    let r = self.ast.exprs.decl(x);
                    if self.is_tu_local_entity(r) {
                        found = Some(r);
                        break;
                    }
                }
                self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
            }
            self.put_walk_stack(stack);
            if found.is_some() {
                return found;
            }
        }
        None
    }

    /// "declaration of 'owner' exposes TU-local entity 'e'" at `at`, once
    /// per declaration.
    fn report_exposure(&mut self, owner: DeclId, at: Loc, e: DeclId) {
        self.modules.exposed.insert(owner);
        let msg = self
            .diag(DiagId::sema_module_exposure, at)
            .arg(self.name_for_diagnostic(owner))
            .arg(self.name_for_diagnostic(e));
        self.emit(msg);
    }
}
