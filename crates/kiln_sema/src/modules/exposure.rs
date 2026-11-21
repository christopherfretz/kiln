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
//! - a use in a default member initializer, the error at its class
//!   ([`Sema::module_field_init_exposures`]), and in the body of a lambda
//!   whose closure type is the type of the variable it initializes, the
//!   error at the lambda (WP-1684);
//! - an odr-use of a TU-local constant in those places
//!   ([`Sema::module_note_odr_use`]; /15.4 ignores only the other
//!   references to it);
//! - the declared type of a function (return and parameter types), of a
//!   variable, or of a non-static data member (the error at its class),
//!   and a default argument (a member function's at its end,
//!   [`Sema::module_check_delayed_default_arg`]), when written
//!   ([`Sema::module_check_decl_exposure`]); also in a local class of an
//!   inline function ([`Sema::local_class_checked`], WP-1814);
//! - a use of an instantiated inline function of a template of the
//!   purview whose instantiated body names a TU-local entity, which makes
//!   it TU-local (/16.5): held as [`PendingExposure::spec`], decided at
//!   the end of the unit ([`Sema::module_resolve_deferred_exposures`]).
//!
//! A TU-local entity is a function, variable, class, enumeration or
//! template with internal linkage, or a class template specialization
//! with a TU-local type argument ([basic.link]/16.1, 16.4). Not checked
//! (GCC 16.1 accepts each): an unevaluated operand, a discarded
//! statement, a reference to a variable that might be usable in constant
//! expressions that is not an odr-use (/15.4), a function or variable
//! declared outside the purview (GCC 16.1 warns,
//! `-Wexpose-global-module-tu-local`), an instantiation of a template
//! declared outside the purview, a friend declaration (/15.3).
//!
//! Not exposures, with clang 22's warning (WP-1814): an enumerator of a
//! TU-local enumeration (not a TU-local entity, /16.1); a use in the body
//! of a member function of a local class that is not inline (ignored,
//! /15.1); a use of a specialization of a function template that is not
//! inline whose definition names a TU-local entity.
//!
//! The initializer of a variable is not an exposure (/15.1.2; /17 makes
//! only a constexpr variable's TU-local value one, which
//! [`Sema::module_check_constexpr_exposure`] checks). For a use in the
//! initializer of an inline variable that is not constexpr, also in a
//! lambda there (whose closure type is not the variable's type), except
//! an odr-use of a constant, kiln gives clang 22's warning at the use, "TU local
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
    /// The closure type of the outermost lambda the use is in, if any.
    lambda: Option<DeclId>,
    /// An odr-use of a TU-local variable usable in constant expressions
    /// ([`Sema::module_note_odr_use`]).
    odr_const: bool,
    /// `entity` is an instantiation of a template of this unit's purview,
    /// TU-local only if its instantiated declaration is an exposure
    /// ([basic.link]/16.5): decided at the end of the unit
    /// ([`Sema::module_resolve_deferred_exposures`]).
    spec: bool,
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
        if !self.exposure_use_point() {
            return;
        }
        let in_inst = !self.inst.stack.frames.is_empty();
        let Some((owner, at, lambda)) = self.exposure_site() else {
            if !in_inst {
                self.note_pattern_use(d);
                self.local_member_body_warning(d, loc);
            }
            return;
        };
        if self.is_tu_local_enumerator(d) {
            // Not a TU-local entity (/16.1 lists types, functions,
            // variables and templates); clang 22 warns, except in a
            // default member initializer.
            if !in_inst && self.kind_of(owner) != DeclKind::CXXRecord {
                let at = self.modules.qualified_use_begin.unwrap_or(loc);
                self.warn_tu_local_exposed(d, at);
            }
            return;
        }
        if self.modules.exposed.contains(&owner) {
            return;
        }
        let tu_local = self.is_tu_local_entity(d);
        if tu_local || self.is_purview_instantiation(d) {
            let p = PendingExposure {
                owner,
                at,
                entity: d,
                loc,
                lambda,
                odr_const: false,
                spec: !tu_local,
            };
            self.modules.pending_exposures.push(p);
        }
    }

    /// Whether a use now may be held: not in an unevaluated operand or a
    /// discarded statement, and either a module interface's purview or an
    /// instantiation in one ([`Sema::inst_exposure_owner`]).
    fn exposure_use_point(&self) -> bool {
        let here = if self.inst.stack.frames.is_empty() {
            !self.modules.frames.is_empty() && self.module_exposure_purview()
        } else {
            self.exposure_main_unit()
        };
        here && self.unevaluated == 0 && !self.in_discarded_statement()
    }

    /// Whether the main unit is a module interface unit or module
    /// partition, whose instantiations [`Sema::inst_exposure_owner`]
    /// follows.
    fn exposure_main_unit(&self) -> bool {
        self.ast.modules.active
            && self.ast.modules.units.first().is_some_and(|u| {
                u.kind.is_interface()
                    || u.kind == kiln_ast::modules::UnitKind::PartitionImplementation
            })
    }

    /// The declaration a use now belongs to ([`Sema::exposure_owner`]),
    /// or in an instantiation the instantiated function
    /// ([`Sema::inst_exposure_owner`]).
    fn exposure_site(&self) -> Option<(DeclId, Loc, Option<DeclId>)> {
        if self.inst.stack.frames.is_empty() {
            self.exposure_owner()
        } else {
            self.inst_exposure_owner()
        }
    }

    /// In an instantiation: the innermost instantiated function that is
    /// not a lambda's call operator, when it is instantiated from a
    /// template of the main unit's purview (the closure type of the
    /// outermost lambda between, if any). Its held uses are recorded at
    /// its end ([`Sema::module_flush_exposures`]): [basic.link]/16.5 makes
    /// an instantiation whose instantiated declaration is an exposure
    /// TU-local.
    fn inst_exposure_owner(&self) -> Option<(DeclId, Loc, Option<DeclId>)> {
        let mut c = self.ctx;
        let mut lambda = None;
        for _ in 0..MAX_WALK {
            let k = self.kind_of(c);
            if k.is_function() {
                let parent = self.ast.decls.get(c).ctx()?;
                if self.is_lambda_call_op(c) {
                    lambda = Some(parent);
                    c = parent;
                    continue;
                }
                return self
                    .is_purview_instantiation(c)
                    .then(|| (c, self.ast.decls.get(c).loc(), lambda));
            }
            match k {
                DeclKind::CXXRecord if self.ast.closure_lambda(c).is_some() => {}
                DeclKind::LinkageSpec | DeclKind::Export => {}
                _ => return None,
            }
            c = self.ast.decls.get(c).ctx()?;
        }
        None
    }

    /// Whether `f` is a function instantiated (implicitly or by an
    /// explicit instantiation) from a template of the main unit's purview,
    /// or a member of an instantiated class of one, and not TU-local by
    /// its name: whether it is TU-local depends on its instantiated
    /// definition ([basic.link]/16.5). A template of the global module
    /// fragment, a header unit or an imported module is not followed.
    fn is_purview_instantiation(&self, f: DeclId) -> bool {
        if !self.kind_of(f).is_function() {
            return false;
        }
        let Some(pattern) = self.ast.instantiated_from(f) else { return false };
        let explicit = self
            .ast
            .spec_info(f)
            .map(|(_, i)| i.kind)
            .or_else(|| self.ast.member_spec_info(f).map(|(_, m)| m.kind));
        let o = self.ast.modules.owner(pattern);
        explicit != Some(kiln_ast::templates::SpecKind::ExplicitSpecialization)
            && o.unit == 0
            && o.purview
            && !self.is_tu_local_entity(f)
    }

    /// Whether `d` is an enumerator of a TU-local enumeration of the
    /// purview.
    fn is_tu_local_enumerator(&self, d: DeclId) -> bool {
        self.kind_of(d) == DeclKind::EnumConstant
            && self.ast.decls.get(d).ctx().is_some_and(|e| {
                self.kind_of(e) == DeclKind::Enum
                    && self.ast.modules.purview_module(e).is_some()
                    && self.is_tu_local_entity(e)
            })
    }

    /// clang 22's warning "TU local entity 'e' is exposed"
    /// (`-WTU-local-entity-exposure`, from `-fclang-compat=22`) at a use
    /// of `e` that is not an exposure by the text; not in a template
    /// definition (clang 22 warns there only when it instantiates it).
    fn warn_tu_local_exposed(&mut self, e: DeclId, loc: Loc) {
        if self.clang_compat < 22 || self.in_template() {
            return;
        }
        let msg =
            self.diag(DiagId::sema_module_tu_local_exposed, loc).arg(self.name_for_diagnostic(e));
        self.emit(msg);
    }

    /// The innermost function around the current point that is not a
    /// lambda's call operator, through closure types, linkage
    /// specifications and export declarations only.
    fn innermost_function(&self) -> Option<DeclId> {
        let mut c = self.ctx;
        for _ in 0..MAX_WALK {
            let k = self.kind_of(c);
            let through = if k.is_function() {
                self.is_lambda_call_op(c)
            } else {
                matches!(k, DeclKind::LinkageSpec | DeclKind::Export)
                    || k == DeclKind::CXXRecord && self.ast.closure_lambda(c).is_some()
            };
            if !through {
                return k.is_function().then_some(c);
            }
            c = self.ast.decls.get(c).ctx()?;
        }
        None
    }

    /// A use of `d` in the definition of a template's function that is not
    /// inline (whose body /15.1 ignores): recorded for clang 22's warning
    /// at a use of a specialization ([`Sema::module_resolve_deferred_exposures`]).
    fn note_pattern_use(&mut self, d: DeclId) {
        if !self.in_template() {
            return;
        }
        let Some(c) = self.innermost_function() else { return };
        if !self.is_inline_fn(c)
            && self.ast.modules.purview_module(c).is_some()
            && self.is_tu_local_entity(d)
        {
            let first = self.ast.first(c);
            self.modules.inst_tu_local.entry(first).or_insert(d);
        }
    }

    /// A use of `d` at `loc` in the body of a member function of a local
    /// class of an inline function that is not inline itself (a class
    /// attached to a named module, [class.mfct]/1): /15.1 ignores that
    /// body, so it is not an exposure; clang 22 warns for a TU-local
    /// entity or an enumerator of a TU-local enumeration (WP-1814).
    fn local_member_body_warning(&mut self, d: DeclId, loc: Loc) {
        let Some(c) = self.innermost_function() else { return };
        if self.is_inline_fn(c) {
            return;
        }
        let Some(r) = self.ast.decls.get(c).ctx() else { return };
        if self.local_class_checked(r) && self.is_tu_local_entity(d) {
            self.warn_tu_local_exposed(d, loc);
        } else if self.local_class_checked(r) && self.is_tu_local_enumerator(d) {
            let at = self.modules.qualified_use_begin.unwrap_or(loc);
            self.warn_tu_local_exposed(d, at);
        }
    }

    /// Whether `r` is a local class (not a closure type) whose innermost
    /// enclosing function that is not a lambda's call operator is inline
    /// and not TU-local: its declarations are part of that function's
    /// body, which [basic.link]/15.1 does not ignore (WP-1814).
    fn local_class_checked(&self, r: DeclId) -> bool {
        if self.kind_of(r) != DeclKind::CXXRecord || self.ast.closure_lambda(r).is_some() {
            return false;
        }
        let mut c = self.ast.decls.get(r).ctx();
        for _ in 0..MAX_WALK {
            let Some(x) = c else { return false };
            let k = self.kind_of(x);
            if k.is_function() && !self.is_lambda_call_op(x) {
                return self.is_inline_fn(x) && !self.is_tu_local_entity(x);
            }
            if matches!(k, DeclKind::Namespace | DeclKind::TranslationUnit) {
                return false;
            }
            c = self.ast.decls.get(x).ctx();
        }
        false
    }

    /// An odr-use at `loc` of `d`, found at the end of its
    /// full-expression: a variable usable in constant expressions with
    /// internal linkage is TU-local and only its uses that are not
    /// odr-uses are ignored ([basic.link]/15.4), so `return &K;` and a
    /// reference bound to `K` in an inline function make it an exposure
    /// (GCC 16.1 rejects; clang 22 accepts). Held as
    /// [`Sema::module_note_use`] holds a use; true if it was.
    pub(crate) fn module_note_odr_use(&mut self, d: DeclId, loc: Loc) -> bool {
        if !self.exposure_use_point() || !self.is_tu_local_constant(d) {
            return false;
        }
        let Some((owner, at, lambda)) = self.exposure_site() else { return false };
        if self.modules.exposed.contains(&owner) {
            return false;
        }
        let p = PendingExposure { owner, at, entity: d, loc, lambda, odr_const: true, spec: false };
        self.modules.pending_exposures.push(p);
        true
    }

    /// Whether `d` is a variable of the purview usable in constant
    /// expressions with internal linkage (TU-local, but named without an
    /// odr-use only a value: [basic.link]/15.4).
    fn is_tu_local_constant(&self, d: DeclId) -> bool {
        self.kind_of(d) == DeclKind::Var
            && self.ast.decls.var(d).is_some()
            && self.might_be_constant_var(d)
            && self.ast.modules.purview_module(d).is_some()
            && self.has_internal_linkage(d)
    }

    /// The end of full-expression `e`: a held use within it is dropped
    /// when `e` folds to a constant that is not a TU-local value (GCC
    /// 16.1 checks the folded body: `helper;`, `(void)&hidden;`,
    /// `0 ? helper() : 1` and `&hidden != nullptr` leave no name behind).
    /// A use in the body of a lambda the full-expression contains whole
    /// is not within it: the closure type keeps the body.
    pub(crate) fn module_full_expr_exposures(&mut self, e: ExprId) {
        if self.modules.pending_exposures.is_empty() {
            return;
        }
        let (b, end) = (self.ast.begin_loc(e).to_raw(), self.ast.end_loc(e).to_raw());
        let inside: Vec<bool> = self
            .modules
            .pending_exposures
            .iter()
            .map(|p| {
                let within = |l: Loc| (b..=end).contains(&l.to_raw());
                within(p.loc) && !p.lambda.is_some_and(|l| within(self.ast.decls.get(l).loc()))
            })
            .collect();
        if !inside.contains(&true) {
            return;
        }
        let pending = &self.modules.pending_exposures;
        if pending.iter().zip(&inside).any(|(p, &w)| w && self.warn_only(p)) {
            // clang 22 warns for each use in an inline variable's
            // initializer, folded or not.
            return;
        }
        let mut value = None;
        let folded = if self.ast.types.is_void(self.ast.exprs.get(e).ty()) {
            self.void_expr_folds(e)
        } else if self.ast.exprs.get(e).value_category().is_glvalue() {
            // A glvalue full-expression (a reference bound to `K`) keeps
            // its odr-uses whatever its value (WP-1684).
            false
        } else {
            value = self.fold_rvalue(e).filter(|&v| self.tu_local_referent(v).is_none());
            value.is_some()
        };
        if folded {
            // A value pointing to an instantiation keeps its use: whether
            // it is TU-local is decided later.
            let kept: Vec<bool> = self
                .modules
                .pending_exposures
                .iter()
                .zip(&inside)
                .map(|(p, &w)| {
                    !w || p.spec && value.is_some_and(|v| self.value_points_to(v, p.entity))
                })
                .collect();
            let mut i = 0;
            self.modules.pending_exposures.retain(|_| {
                i += 1;
                kept[i - 1]
            });
        }
    }

    /// Whether constant `cv` is or holds a pointer to function `f`.
    fn value_points_to(&self, cv: kiln_ast::ids::ConstValueId, f: DeclId) -> bool {
        use kiln_ast::consts::{ConstValue, PtrBase, Scalar};
        let f = self.ast.first(f);
        let mut work = vec![cv];
        let mut seen = kiln_source::fx::FxHashSet::default();
        while let Some(c) = work.pop() {
            if seen.len() > MAX_WALK || !seen.insert(c) {
                continue;
            }
            match self.ast.consts.get(c) {
                ConstValue::Scalar(Scalar::Pointer { base, .. }) => match base {
                    PtrBase::Decl(d) | PtrBase::Function(d) if self.ast.first(d) == f => {
                        return true;
                    }
                    PtrBase::Const(c) => work.push(c),
                    _ => {}
                },
                ConstValue::Aggregate(img) => work.extend(img.relocs.iter().map(|r| r.value)),
                ConstValue::Scalar(_) => {}
            }
        }
        false
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

    /// Whether held use `p` is in an inline variable's initializer
    /// outside any lambda: a warning, not an exposure (see the module
    /// docs).
    fn warn_only(&self, p: &PendingExposure) -> bool {
        self.kind_of(p.owner) == DeclKind::Var && p.lambda.is_none()
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

    /// The end of declaration `owner` (a function definition, a
    /// variable's initializer, a default member initializer of class
    /// `owner`): its first held use is an exposure. For a variable, a use
    /// in the lambda whose closure type is the variable's type is one,
    /// reported at the lambda (the closure type is reachable and its
    /// function call operator is inline; GCC 16.1 rejects, clang 22 warns
    /// in an inline variable); any other use in an inline variable's
    /// initializer is clang 22's warning, except an odr-use of a constant
    /// (clang 22 is silent).
    pub(crate) fn module_flush_exposures(&mut self, owner: DeclId) {
        if self.modules.pending_exposures.is_empty() {
            return;
        }
        let (mine, specs): (Vec<PendingExposure>, Vec<PendingExposure>) = self
            .modules
            .pending_exposures
            .iter()
            .copied()
            .filter(|p| p.owner == owner)
            .partition(|p| !p.spec);
        self.modules.pending_exposures.retain(|p| p.owner != owner);
        if !self.inst.stack.frames.is_empty() {
            self.record_inst_exposures(owner, &mine, &specs);
            return;
        }
        if self.kind_of(owner) == DeclKind::Var {
            let closure = self
                .ast
                .decls
                .get(owner)
                .ty()
                .and_then(|t| self.ast.types.as_record(t))
                .filter(|&r| self.ast.closure_lambda(r).is_some())
                .map(|r| self.ast.first(r));
            let in_closure =
                |s: &Self, p: &PendingExposure| p.lambda.map(|l| s.ast.first(l)) == closure;
            if closure.is_some()
                && let Some(p) = mine.iter().find(|p| in_closure(self, p))
            {
                if !self.modules.exposed.contains(&owner) {
                    self.report_exposure(owner, p.at, p.entity);
                }
                return;
            }
            let warn = self
                .ast
                .decls
                .var(owner)
                .is_some_and(|x| x.spec.is_inline() && !x.spec.is_constexpr())
                && self.clang_compat >= 22;
            for p in specs {
                let w = closure.is_none() || !in_closure(self, &p);
                if !w || warn {
                    self.modules.deferred_exposures.push((p, w));
                }
            }
            if warn {
                for p in mine.into_iter().filter(|p| !p.odr_const) {
                    let msg = self
                        .diag(DiagId::sema_module_tu_local_exposed, p.loc)
                        .arg(self.name_for_diagnostic(p.entity));
                    self.emit(msg);
                }
            }
            return;
        }
        if self.modules.exposed.contains(&owner) {
            return;
        }
        if let Some(p) = mine.first() {
            self.report_exposure(owner, p.at, p.entity);
        } else {
            self.modules.deferred_exposures.extend(specs.into_iter().map(|p| (p, false)));
        }
    }

    /// The end of instantiated function `f` ([`Sema::inst_exposure_owner`]),
    /// when it is inline: records the first TU-local entity its body
    /// names and the instantiations of the purview's templates it names
    /// (TU-local in turn when their instantiated definitions are
    /// exposures).
    fn record_inst_exposures(
        &mut self,
        f: DeclId,
        mine: &[PendingExposure],
        specs: &[PendingExposure],
    ) {
        let first = self.ast.first(f);
        if !self.is_inline_fn(first) {
            // /15.1 ignores its body.
            return;
        }
        if let Some(p) = mine.first() {
            self.modules.inst_tu_local.entry(first).or_insert(p.entity);
        }
        if !specs.is_empty() {
            let uses: Vec<DeclId> = specs.iter().map(|p| self.ast.first(p.entity)).collect();
            let v = self.modules.inst_spec_uses.entry(first).or_default();
            for u in uses {
                if u != first && !v.contains(&u) {
                    v.push(u);
                }
            }
        }
    }

    /// The end of the translation unit, after its instantiations: each
    /// held use of an instantiation ([`PendingExposure::spec`]) whose
    /// instantiated definition is an exposure, which makes it TU-local
    /// ([basic.link]/16.5), is an exposure of the declaration it is in (or
    /// clang 22's warning, in an inline variable's initializer). An
    /// instantiated function that is not inline has its body ignored
    /// (/15.1): it is not TU-local, and a use of a function template
    /// specialization whose body names a TU-local entity gets clang 22's
    /// warning (WP-1814).
    pub(crate) fn module_resolve_deferred_exposures(&mut self) {
        let mut deferred = std::mem::take(&mut self.modules.deferred_exposures);
        // A use in a template's definition makes that template an
        // exposure, and so its specializations: again until none is new.
        for _ in 0..MAX_WALK {
            let before = self.modules.exposed.len();
            deferred.retain(|&(p, warn)| {
                if !self.inst_is_tu_local(self.ast.first(p.entity)) {
                    return true;
                }
                if warn {
                    self.warn_tu_local_exposed(p.entity, p.loc);
                } else if !self.modules.exposed.contains(&p.owner) {
                    self.report_exposure(p.owner, p.at, p.entity);
                }
                false
            });
            if self.modules.exposed.len() == before {
                break;
            }
        }
        for (p, _) in deferred {
            let s = self.ast.first(p.entity);
            if !self.is_inline_fn(s) && self.is_function_spec(s) && self.inst_names_tu_local(s) {
                self.warn_tu_local_exposed(p.entity, p.loc);
            }
        }
    }

    /// Whether instantiation `s` (a first declaration) is TU-local by
    /// [basic.link]/16.5: it is inline and its body names a TU-local
    /// entity, or an instantiation that is TU-local so.
    fn inst_is_tu_local(&self, s: DeclId) -> bool {
        let mut seen = kiln_source::fx::FxHashSet::default();
        let mut work = vec![s];
        while let Some(x) = work.pop() {
            if seen.len() > 100_000 || !seen.insert(x) || !self.is_inline_fn(x) {
                continue;
            }
            if self.inst_names_tu_local(x) {
                return true;
            }
            if let Some(v) = self.modules.inst_spec_uses.get(&x) {
                work.extend(v.iter().copied());
            }
        }
        false
    }

    /// Whether the body of instantiation `s` names a TU-local entity: a
    /// use found while instantiating it (an inline one), or one its pattern
    /// names where it is written (a template that is an exposure, or a use
    /// in the body of one that is not inline, [`Sema::note_pattern_use`],
    /// which is what clang 22 warns for).
    fn inst_names_tu_local(&self, s: DeclId) -> bool {
        self.modules.inst_tu_local.contains_key(&s)
            || self.ast.instantiated_from(s).is_some_and(|p| {
                self.ast.redecls_of(p).any(|r| {
                    self.modules.exposed.contains(&r) || self.modules.inst_tu_local.contains_key(&r)
                })
            })
    }

    /// The declaration a use at the current point belongs to, when the
    /// use is in a part [basic.link]/15 considers, where it is reported,
    /// and the closure type of the outermost lambda it is in: the
    /// innermost enclosing function that is not a lambda's call
    /// operator, if it is inline (at the outermost lambda between, if
    /// any); else, outside any function, the class whose default member
    /// initializer is being parsed (WP-1684: at the class, or at the
    /// lambda); else the variable being initialized when it is inline and
    /// not constexpr, or when the use is in a lambda there (at the lambda;
    /// [`Sema::module_flush_exposures`]).
    fn exposure_owner(&self) -> Option<(DeclId, Loc, Option<DeclId>)> {
        let mut c = self.ctx;
        let mut lambda = None;
        for _ in 0..MAX_WALK {
            let k = self.kind_of(c);
            if k.is_function() {
                let parent = self.ast.decls.get(c).ctx()?;
                if self.is_lambda_call_op(c) {
                    lambda = Some(parent);
                    c = parent;
                    continue;
                }
                if !self.is_inline_fn(c) || self.is_tu_local_entity(c) {
                    return None;
                }
                return Some((c, self.lambda_or(lambda, c), lambda));
            }
            match k {
                DeclKind::CXXRecord if self.ast.closure_lambda(c).is_some() => {}
                DeclKind::LinkageSpec | DeclKind::Export => {}
                _ => break,
            }
            c = self.ast.decls.get(c).ctx()?;
        }
        if let Some(&(field, _)) = self.nsdmi_this.last()
            && self.ast.decls.get(field).ctx() == Some(c)
        {
            let owner = self.nsdmi_exposure_owner(field)?;
            return Some((owner, self.lambda_or(lambda, owner), lambda));
        }
        let v = self.depr_init?;
        let inline =
            self.ast.decls.var(v).is_some_and(|x| x.spec.is_inline() && !x.spec.is_constexpr());
        if !(inline || lambda.is_some() && self.ast.decls.var(v).is_some())
            || self.is_tu_local_entity(v)
        {
            return None;
        }
        Some((v, self.lambda_or(lambda, v), lambda))
    }

    /// Where a use in declaration `d` is reported: at the outermost
    /// lambda it is in (`lambda`, a closure type), else at `d`.
    fn lambda_or(&self, lambda: Option<DeclId>, d: DeclId) -> Loc {
        self.ast.decls.get(lambda.unwrap_or(d)).loc()
    }

    /// Whether function `f` is inline (written `inline`, `constexpr` or
    /// `consteval`, or implicitly).
    fn is_inline_fn(&self, f: DeclId) -> bool {
        let s = self.fn_spec(f);
        s.is_inline() || s.implicit_inline() || s.constexpr_kind() != ConstexprKind::None
    }

    /// The class a default member initializer of `field` belongs to for
    /// exposures (WP-1684): its class, or for a member of an anonymous
    /// struct or union the named class around it, unless TU-local; a
    /// local class only in an inline function that is not TU-local (a
    /// non-inline function's body is ignored, [basic.link]/15.1.1).
    fn nsdmi_exposure_owner(&self, field: DeclId) -> Option<DeclId> {
        let mut r = self.ast.decls.get(field).ctx()?;
        for _ in 0..MAX_WALK {
            if !self
                .ast
                .decls
                .record(r)
                .is_some_and(|x| x.defn.contains(kiln_ast::decl::DefinitionBits::ANONYMOUS))
            {
                break;
            }
            r = self.ast.decls.get(r).ctx()?;
        }
        if self.kind_of(r) != DeclKind::CXXRecord
            || self.ast.closure_lambda(r).is_some()
            || self.is_tu_local_entity(r)
        {
            return None;
        }
        let mut c = self.ast.decls.get(r).ctx()?;
        for _ in 0..MAX_WALK {
            let k = self.kind_of(c);
            if k.is_function() {
                return (!self.is_lambda_call_op(c)
                    && self.is_inline_fn(c)
                    && !self.is_tu_local_entity(c))
                .then_some(r);
            }
            if matches!(k, DeclKind::Namespace | DeclKind::TranslationUnit) {
                return Some(r);
            }
            c = self.ast.decls.get(c).ctx()?;
        }
        None
    }

    /// The end of the default member initializer of `field`: its held
    /// uses are an exposure of its class (WP-1684; GCC 16.1 rejects,
    /// clang 22 accepts).
    pub(crate) fn module_field_init_exposures(&mut self, field: DeclId) {
        if self.modules.pending_exposures.is_empty() {
            return;
        }
        if let Some(owner) = self.nsdmi_exposure_owner(field) {
            self.module_flush_exposures(owner);
        }
    }

    /// The end of the default argument of parameter `p` of a member
    /// function, parsed in the complete-class context: one naming a
    /// TU-local entity makes the function an exposure, as a default
    /// argument parsed in place does ([`Sema::module_check_decl_exposure`];
    /// WP-1684).
    pub(crate) fn module_check_delayed_default_arg(&mut self, p: DeclId) {
        if !self.inst.stack.frames.is_empty() || !self.module_exposure_purview() {
            return;
        }
        let Some(f) = self.ast.decls.get(p).ctx() else { return };
        let Some(class) = self.ast.decls.get(f).ctx() else { return };
        if !self.kind_of(f).is_function()
            || self.kind_of(class) != DeclKind::CXXRecord
            || self.in_function_scope(class)
            || self.modules.exposed.contains(&f)
            || self.is_tu_local_entity(class)
        {
            return;
        }
        if let Some(e) = self.default_arg_tu_local(p) {
            self.report_exposure(f, self.ast.decls.get(f).loc(), e);
        }
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
        if self.in_function_scope(ctx) && !self.local_class_checked(ctx)
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
            && !self.local_class_checked(r)
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

    /// The TU-local entity a default argument of function `f` names, if
    /// any ([`Sema::default_arg_tu_local`]).
    fn default_args_tu_local(&self, f: DeclId) -> Option<DeclId> {
        let params = self.ast.decls.function(f)?.params;
        self.ast.lists.iter::<DeclId>(params).find_map(|p| self.default_arg_tu_local(p))
    }

    /// The TU-local entity the default argument of parameter `p` names,
    /// if any: a TU-local function or variable it names (in an
    /// unevaluated operand too), a TU-local constant it odr-uses
    /// ([basic.link]/15.4 ignores the other references to one), or a
    /// TU-local type written in it (a `sizeof`/`alignof` operand, an
    /// explicit type conversion: WP-1684; GCC 16.1 rejects each).
    fn default_arg_tu_local(&self, p: DeclId) -> Option<DeclId> {
        let e = self.ast.decls.param(p).and_then(|x| x.default_arg)?;
        let mut stack = self.take_walk_stack(e);
        let mut found = None;
        let mut n = 0usize;
        while let Some(x) = stack.pop() {
            n += 1;
            if n > 100_000 {
                break;
            }
            let node = self.ast.exprs.get(x);
            found = match node.kind() {
                ExprKind::DeclRefExpr => {
                    let r = self.ast.exprs.decl(x);
                    (self.is_tu_local_entity(r)
                        || self.is_tu_local_constant(r)
                            && self.ast.exprs.non_odr_use(x) == kiln_ast::expr::NonOdrUse::None)
                        .then_some(r)
                }
                ExprKind::UnaryExprOrTypeTraitExpr => {
                    self.ast.exprs.trait_type_arg(x).and_then(|t| self.type_tu_local(t, 0))
                }
                ExprKind::CStyleCastExpr
                | ExprKind::CXXStaticCastExpr
                | ExprKind::CXXFunctionalCastExpr
                | ExprKind::CXXConstCastExpr
                | ExprKind::CXXReinterpretCastExpr
                | ExprKind::CXXDynamicCastExpr
                | ExprKind::CXXTemporaryObjectExpr => {
                    let t = self.ast.exprs.cast_written_type(x).unwrap_or_else(|| node.ty());
                    self.type_tu_local(t, 0)
                }
                _ => None,
            };
            if found.is_some() {
                break;
            }
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        self.put_walk_stack(stack);
        found
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
