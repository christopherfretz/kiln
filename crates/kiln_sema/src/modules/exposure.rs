//! Exposures by naming a TU-local entity ([basic.link]/14-17;
//! spec/modules.md §5 "Exposures"; WP-1678). The constexpr-value kind is
//! [`Sema::module_check_constexpr_exposure`].
//!
//! In the purview of a module interface unit or module partition (not
//! its private module fragment, not a header unit), a declaration of an
//! entity that is not TU-local is ill-formed when it names a TU-local
//! one outside the parts /15 ignores. kiln checks, at most once per
//! declaration (clang 22 only warns, `-WTU-local-entity-exposure`; GCC
//! 16.1 rejects a subset):
//! - a use ([`Sema::diagnose_deprecated`], clang's `DiagnoseUseOfDecl`)
//!   in the body of an inline function (written `inline`, `constexpr` or
//!   `consteval`, or a member defined in a class attached to the global
//!   module), also in a lambda there: the error is at the function (the
//!   lambda); in an unevaluated operand and a discarded statement too
//!   (WP-1821: they name what they name);
//! - every function of an overload set an id-expression refers to, the
//!   selected one or not, with those argument-dependent lookup adds
//!   ([`Sema::module_note_overload_set`], /14.5; WP-1819);
//! - a use in a default member initializer, the error at its class
//!   ([`Sema::module_field_init_exposures`]), and in the body of a lambda
//!   whose closure type is the type of the variable it initializes, the
//!   error at the lambda (WP-1684);
//! - an odr-use of a TU-local constant in those places
//!   ([`Sema::module_note_odr_use`]; /15.4 ignores only the other
//!   references to it);
//! - the declared type of a function (return and parameter types, a
//!   `noexcept` operand), of a variable, or of a non-static data member
//!   (the error at its class), a name in an unevaluated operand there
//!   ([`Sema::note_declarator_use`], WP-1821), and a default argument (a
//!   member function's at its end,
//!   [`Sema::module_check_delayed_default_arg`]), when written
//!   ([`Sema::module_check_decl_exposure`]); also in a local class of an
//!   inline function (WP-1814);
//! - the declarations of a local class or closure type of a function
//!   that is not inline when its deduced return type names the class
//!   (/15.1 keeps that type): held as [`PendingExposure::escape`] and
//!   decided at the function's end ([`Sema::module_resolve_escapes`],
//!   WP-1820). Any other local class of such a function is part of its
//!   ignored body, whatever its members' specifiers;
//! - a use of an instantiated inline function of a template of the
//!   purview whose instantiated body names a TU-local entity, which makes
//!   it TU-local (/16.5): held as [`PendingExposure::spec`], decided at
//!   the end of the unit ([`Sema::module_resolve_deferred_exposures`]).
//! - the constraint-expression of a concept (a template; neither a
//!   function body nor an initializer), also a lambda there: collected
//!   while it is parsed ([`Sema::module_start_concept`]) and checked at
//!   the concept ([`Sema::module_check_concept_exposure`], WP-1845);
//!
//! A TU-local entity is a function, variable, class, enumeration or
//! template with internal linkage, or a class template specialization
//! with a TU-local type argument ([basic.link]/16.1, 16.4). Not checked:
//! a reference to a variable that might be usable in constant
//! expressions that is not an odr-use (/15.4), a function or variable
//! declared outside the purview (GCC 16.1 warns,
//! `-Wexpose-global-module-tu-local`), an instantiation of a template
//! declared outside the purview, a friend declaration (/15.3), the
//! candidates of an operator expression (not an id-expression). A use is
//! held whatever its full-expression folds to (`helper;`, `0 ? helper()
//! : 1`, `&K != nullptr` name `helper` and odr-use `K`; WP-1847: GCC
//! 16.1 folds these away first).
//!
//! Not exposures, with clang 22's warning (WP-1814, WP-1821): an
//! enumerator of a TU-local enumeration (not a TU-local entity, /16.1);
//! a use in the body of a member function of a local class that is not
//! inline (ignored, /15.1); a use of a specialization of a function
//! template that is not inline whose definition names a TU-local entity,
//! also in an unevaluated operand or a discarded statement; a use of a
//! specialization of an inline template whose body names only those
//! ([`Sema::note_inst_warning`]).
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
use kiln_ast::expr::ExprKind;
use kiln_ast::types::{ArgView, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::Phase;
use crate::Sema;

/// How many links a walk follows at most.
const MAX_WALK: usize = 1024;

/// Where a held use belongs ([`Sema::exposure_site`]): the declaration it
/// would make an exposure, where that is reported, the closure type of
/// the outermost lambda it is in, and its [`PendingExposure::escape`].
type Site = (DeclId, Loc, Option<DeclId>, Option<(DeclId, DeclId)>);

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
    /// `(F, C)`: the use is in local class or closure type `C` of the body
    /// of function `F`, which is not inline and has a deduced return type:
    /// an exposure only if that type names `C` ([`BodyScope::Escape`],
    /// decided at the end of `F`, [`Sema::module_resolve_escapes`]).
    escape: Option<(DeclId, DeclId)>,
}

/// Whether the declarations in a context are part of the body of a
/// function that is not inline, which [basic.link]/15.1 ignores
/// ([`Sema::body_scope`]; WP-1820).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BodyScope {
    /// No function body around them is ignored.
    Considered,
    /// In local class or closure type `C` (the outermost) of the body of
    /// `F`, which is not inline and has a deduced return type: considered
    /// only if that type names `C`, which /15.1 keeps (`(F, C)`).
    Escape(DeclId, DeclId),
    /// In the body of a function that is not inline, or of a TU-local one.
    Ignored,
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
    /// declaration [basic.link]/15 considers: held until the declaration's
    /// end ([`Sema::module_flush_exposures`]), whatever its full-expression
    /// folds to (WP-1847). In a concept's constraint-expression, collected
    /// for the concept ([`Sema::note_concept_use`], WP-1845).
    pub(crate) fn module_note_use(&mut self, d: DeclId, loc: Loc) {
        if !self.exposure_use_point() {
            return;
        }
        let in_inst = !self.inst.stack.frames.is_empty();
        let Some((owner, at, lambda, escape)) = self.exposure_site() else {
            if !in_inst {
                self.note_concept_use(d, loc);
                self.note_pattern_use(d);
                self.local_member_body_warning(d, loc);
                self.note_declarator_use(d, loc);
            }
            return;
        };
        if self.is_tu_local_enumerator(d) {
            // Not a TU-local entity (/16.1 lists types, functions,
            // variables and templates); clang 22 warns, except in a
            // default member initializer, and in an instantiation at a
            // use of the instantiated function (WP-1821).
            if escape.is_some() {
            } else if in_inst || self.in_template() {
                self.note_inst_warning(owner);
            } else if self.kind_of(owner) != DeclKind::CXXRecord {
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
                escape,
            };
            self.modules.pending_exposures.push(p);
        }
    }

    /// A use of `d` at `loc` in the decl-specifiers or a declarator of the
    /// declaration being parsed (its deprecation delay point, outside its
    /// initializer): held for [`Sema::module_check_decl_exposure`], which
    /// takes it. The declaration names `d` there whatever the type it
    /// forms: `decltype(f) *fp;` is an exposure of `f`, an unevaluated
    /// operand of the declarator (the standard's example; WP-1821).
    fn note_declarator_use(&mut self, d: DeclId, loc: Loc) {
        if self.depr_init.is_none()
            && self.depr_points.last() == Some(&self.ctx)
            && !self.kind_of(self.ctx).is_function()
            && self.module_exposure_purview()
            && self.is_tu_local_entity(d)
        {
            self.modules.decl_uses.push((self.depr_points.len(), d, loc));
        }
    }

    /// After `concept name =` ([`Sema::act_on_concept_head`]): in a
    /// module interface's purview, the TU-local entities the
    /// constraint-expression names are collected for
    /// [`Sema::module_check_concept_exposure`] (WP-1845).
    pub(crate) fn module_start_concept(&mut self) {
        self.concept_uses = self.module_exposure_purview().then(Vec::new);
    }

    /// A use of TU-local entity `d` at `loc` in the constraint-expression
    /// of the concept being defined ([`Sema::module_start_concept`]), also
    /// in a lambda there, an unevaluated operand or a requires-expression:
    /// the concept-definition names it ([basic.link]/14).
    fn note_concept_use(&mut self, d: DeclId, loc: Loc) {
        if self.concept_uses.is_none() || !self.is_tu_local_entity(d) {
            return;
        }
        let lambda = self.outermost_lambda();
        if let Some(v) = self.concept_uses.as_mut() {
            v.push((d, loc, lambda));
        }
    }

    /// The closure type of the outermost lambda around the current point,
    /// through closure types, requires-expression bodies, linkage
    /// specifications and export declarations only.
    fn outermost_lambda(&self) -> Option<DeclId> {
        let mut c = self.ctx;
        let mut lambda = None;
        for _ in 0..MAX_WALK {
            let k = self.kind_of(c);
            if k.is_function() {
                if !self.is_lambda_call_op(c) {
                    break;
                }
                lambda = self.ast.decls.get(c).ctx();
            } else if !(matches!(
                k,
                DeclKind::LinkageSpec | DeclKind::Export | DeclKind::RequiresExprBody
            ) || k == DeclKind::CXXRecord && self.ast.closure_lambda(c).is_some())
            {
                break;
            }
            let Some(p) = self.ast.decls.get(c).ctx() else { break };
            c = p;
        }
        lambda
    }

    /// The end of the definition of concept `c`: a TU-local entity its
    /// constraint-expression names makes it an exposure ([basic.link]/14,
    /// /17: a concept is a template, and its constraint-expression is
    /// neither a function body nor an initializer /15 ignores; WP-1845),
    /// reported at its name, or at the outermost lambda the use is in.
    /// GCC 16.1 rejects the same concepts.
    pub(crate) fn module_check_concept_exposure(&mut self, c: DeclId) {
        let Some(uses) = self.concept_uses.take() else { return };
        let Some(&(e, _, lambda)) = uses.first() else { return };
        if self.modules.exposed.contains(&c) || self.is_tu_local_entity(c) {
            return;
        }
        let at = self.lambda_or(lambda, c);
        self.report_exposure(c, at, e);
    }

    /// The constraint-expression of the concept definition being parsed
    /// failed: nothing is checked.
    pub fn act_on_concept_error(&mut self) {
        self.concept_uses = None;
    }

    /// Drops the declarator uses held above deprecation delay level
    /// `level` ([`Sema::note_declarator_use`]), whose declaration is done.
    pub(crate) fn module_drop_declarator_uses(&mut self, level: usize) {
        if !self.modules.decl_uses.is_empty() {
            self.modules.decl_uses.retain(|&(l, ..)| l <= level);
        }
    }

    /// The id-expression at `loc` refers to the set of overloads `decls`
    /// (found by lookup, or added by argument-dependent lookup at a call):
    /// it names each function and function template in the set, whether
    /// or not overload resolution selects it ([basic.link]/14; WP-1819),
    /// so a TU-local one is held as [`Sema::module_note_use`] holds a use
    /// (the selected one is also used). No warning where the text says
    /// there is no exposure: clang 22 looks only at the selected function.
    pub(crate) fn module_note_overload_set(&mut self, decls: &[DeclId], loc: Loc) {
        if !self.ast.modules.active || !self.exposure_use_point() {
            return;
        }
        for &d in decls {
            let mut d = self.ast.strip_shadow(d);
            if self.kind_of(d) == DeclKind::FunctionTemplate {
                // Its linkage is its pattern's.
                d = self.pattern_of(d).unwrap_or(d);
            }
            if !self.kind_of(d).is_function() || !self.is_tu_local_entity(d) {
                continue;
            }
            match self.exposure_site() {
                Some((owner, at, lambda, escape)) => {
                    if self.modules.exposed.contains(&owner) {
                        return;
                    }
                    let p = PendingExposure {
                        owner,
                        at,
                        entity: d,
                        loc,
                        lambda,
                        odr_const: false,
                        spec: false,
                        escape,
                    };
                    self.modules.pending_exposures.push(p);
                }
                // A declarator ([`Sema::note_declarator_use`]); a default
                // argument is also checked by its walk
                // ([`Sema::default_arg_tu_local`]).
                None if self.inst.stack.frames.is_empty() => {
                    self.note_concept_use(d, loc);
                    self.modules.set_uses.push((loc, d));
                    self.note_declarator_use(d, loc);
                }
                None => {}
            }
        }
    }

    /// Whether a use now may be held: a module interface's purview, or
    /// an instantiation in one ([`Sema::inst_exposure_owner`]). An
    /// unevaluated operand and a discarded statement name what they name
    /// ([basic.link]/14 counts every expression a declaration contains:
    /// WP-1821), as the declarator `decltype(f) *fp;` of the standard's
    /// example does.
    fn exposure_use_point(&self) -> bool {
        if self.inst.stack.frames.is_empty() {
            !self.modules.frames.is_empty() && self.module_exposure_purview()
        } else {
            self.exposure_main_unit()
        }
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
    fn exposure_site(&self) -> Option<Site> {
        if self.inst.stack.frames.is_empty() {
            self.exposure_owner()
        } else {
            self.inst_exposure_owner().map(|(o, at, l)| (o, at, l, None))
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
                DeclKind::LinkageSpec | DeclKind::Export | DeclKind::RequiresExprBody => {}
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
                matches!(k, DeclKind::LinkageSpec | DeclKind::Export | DeclKind::RequiresExprBody)
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
        if self.local_class_scope(r) != Some(BodyScope::Considered) {
            return;
        }
        if self.in_template() && (self.is_tu_local_entity(d) || self.is_tu_local_enumerator(d)) {
            // clang 22 warns at a use of the instantiation (WP-1821).
            self.note_inst_warning(c);
        }
        if self.is_tu_local_entity(d) {
            self.warn_tu_local_exposed(d, loc);
        } else if self.is_tu_local_enumerator(d) {
            let at = self.modules.qualified_use_begin.unwrap_or(loc);
            self.warn_tu_local_exposed(d, at);
        }
    }

    /// The [`Sema::body_scope`] of `r` when it is a local class (not a
    /// closure type): its declarations are part of the body of the
    /// function around it (WP-1814, WP-1820).
    fn local_class_scope(&self, r: DeclId) -> Option<BodyScope> {
        if self.kind_of(r) != DeclKind::CXXRecord
            || self.ast.closure_lambda(r).is_some()
            || !self.in_function_scope(r)
        {
            return None;
        }
        Some(self.body_scope(r))
    }

    /// Whether the declarations in `d` (and `d` itself) are part of a
    /// function body [basic.link]/15.1 ignores (WP-1820). A local class or
    /// closure type of a function is not TU-local unless the function is
    /// (/16.1.2), and its declarations are part of that function's body:
    /// considered in an inline function's (WP-1814), ignored with the body
    /// of one that is not inline, whatever their own specifiers (a
    /// constexpr member function's body included), except where the
    /// function's deduced return type, which /15.1 keeps, names the class
    /// ([`BodyScope::Escape`]; GCC 16.1 checks exactly those). Everything
    /// in a TU-local function is TU-local, and ignored.
    fn body_scope(&self, d: DeclId) -> BodyScope {
        let mut below = (self.kind_of(d) == DeclKind::CXXRecord).then_some(d);
        let mut ignoring = None;
        let mut c = self.ast.decls.get(d).ctx();
        for _ in 0..MAX_WALK {
            let Some(x) = c else { break };
            let k = self.kind_of(x);
            if k.is_function() {
                if self.is_tu_local_entity(x) {
                    return BodyScope::Ignored;
                }
                if !self.is_lambda_call_op(x) && !self.is_inline_fn(x) {
                    if ignoring.is_some() {
                        return BodyScope::Ignored;
                    }
                    ignoring = Some((x, below));
                }
            } else if k == DeclKind::CXXRecord {
                below = Some(x);
            } else if matches!(k, DeclKind::Namespace | DeclKind::TranslationUnit) {
                break;
            }
            c = self.ast.decls.get(x).ctx();
        }
        match ignoring {
            None => BodyScope::Considered,
            Some((f, Some(r))) if self.declared_with_placeholder_return(f) => {
                BodyScope::Escape(f, r)
            }
            Some(_) => BodyScope::Ignored,
        }
    }

    /// Whether function `f` was declared with a placeholder return type
    /// (deduced since, or not yet).
    fn declared_with_placeholder_return(&self, f: DeclId) -> bool {
        if self.fn_spec(f).return_deduced() {
            return true;
        }
        let Some(t) = self.ast.decls.get(f).ty() else { return false };
        match self.ast.types.view(t.ty()) {
            TypeView::Function(p) => self.contains_undeduced_auto(p.ret()),
            _ => false,
        }
    }

    /// An exposure of `owner` naming `e`, found where `scope` says: now,
    /// or at the end of the function whose deduced return type decides it
    /// ([`Sema::module_resolve_escapes`]).
    fn report_in_scope(&mut self, owner: DeclId, e: DeclId, scope: BodyScope) {
        let at = self.ast.decls.get(owner).loc();
        match scope {
            BodyScope::Considered => self.report_exposure(owner, at, e),
            BodyScope::Escape(f, c) => self.modules.escapes.push(PendingExposure {
                owner,
                at,
                entity: e,
                loc: at,
                lambda: None,
                odr_const: false,
                spec: false,
                escape: Some((f, c)),
            }),
            BodyScope::Ignored => {}
        }
    }

    /// The end of the definition of function `f`: the held uses in its
    /// local classes and closure types ([`BodyScope::Escape`]) are
    /// exposures when its deduced return type names their class (through
    /// pointers, references, arrays, function types and template
    /// arguments, and the data members' and base classes' types of a class
    /// it names), else part of its body, which /15.1 ignores (WP-1820).
    pub(crate) fn module_resolve_escapes(&mut self, f: DeclId) {
        let pattern = self.escape_pattern(f);
        if self.modules.escapes.is_empty() && pattern.is_none() {
            return;
        }
        let (mine, rest): (Vec<PendingExposure>, Vec<PendingExposure>) =
            std::mem::take(&mut self.modules.escapes)
                .into_iter()
                .partition(|p| p.escape.is_some_and(|(x, _)| x == f));
        self.modules.escapes = rest;
        if self.in_template() {
            // A template's deduced return type is known in its
            // instantiations: its held uses wait for each (WP-1846).
            self.modules.escapes.extend(mine);
            return;
        }
        if mine.is_empty() && pattern.is_none() {
            return;
        }
        let named = self.escaped_local_classes(f);
        if named.is_empty() {
            return;
        }
        if let Some(pf) = pattern {
            self.resolve_instantiated_escapes(f, pf, &named, mine);
            return;
        }
        for p in mine {
            let Some((_, c)) = p.escape else { continue };
            if !named.contains(&c) {
                continue;
            }
            if p.spec {
                let p = PendingExposure { escape: None, ..p };
                self.modules.deferred_exposures.push((p, false));
            } else if !self.modules.exposed.contains(&p.owner) {
                self.report_exposure(p.owner, p.at, p.entity);
            }
        }
    }

    /// For an instantiated function `f` of a template of the purview that
    /// is not inline and was declared with a placeholder return type: the
    /// first declaration of its pattern, whose held uses in local classes
    /// and closure types ([`BodyScope::Escape`]) wait for `f`'s deduced
    /// return type (WP-1846).
    fn escape_pattern(&self, f: DeclId) -> Option<DeclId> {
        if !self.exposure_main_unit() {
            return None;
        }
        let p = self.ast.instantiated_from(f)?;
        (self.is_purview_instantiation(f)
            && !self.is_inline_fn(f)
            && self.declared_with_placeholder_return(p))
        .then(|| self.ast.first(p))
    }

    /// Instantiated function `f` of pattern `pf` (a first declaration)
    /// returns the local classes and closure types `named` (instantiated),
    /// whose declarations [basic.link]/15.1 keeps with the deduced return
    /// type. When they name a TU-local entity, `f`'s declaration is an
    /// exposure, so `f` is TU-local (/15.5) and a use of it is held as a
    /// use of any instantiation of the purview (WP-1846): a use held in
    /// the pattern's classes they are instantiated from, in an
    /// instantiated lambda `mine` holds, or in the body of an instantiated
    /// inline member function of theirs (a name argument-dependent lookup
    /// finds there). A use of an instantiation there makes `f` TU-local
    /// when that one is. GCC 16.1 rejects the instantiated member itself.
    fn resolve_instantiated_escapes(
        &mut self,
        f: DeclId,
        pf: DeclId,
        named: &kiln_source::fx::FxHashSet<DeclId>,
        mine: Vec<PendingExposure>,
    ) {
        let classes: Vec<(DeclId, Loc)> = named
            .iter()
            .map(|&c| {
                let p = self.ast.instantiated_from(c).unwrap_or(c);
                (self.ast.first(p), self.ast.decls.get(c).loc())
            })
            .collect();
        let mut held: Vec<PendingExposure> = self
            .modules
            .escapes
            .iter()
            .copied()
            .filter(|p| {
                p.escape.is_some_and(|(x, c)| {
                    self.ast.first(x) == pf
                        && classes.iter().any(|&(pc, at)| {
                            self.ast.first(c) == pc || self.ast.decls.get(c).loc() == at
                        })
                })
            })
            .collect();
        held.extend(mine.into_iter().filter(|p| p.escape.is_some_and(|(_, c)| named.contains(&c))));
        let first = self.ast.first(f);
        if let Some(p) = held.iter().find(|p| !p.spec) {
            self.modules.escaped_tu_local.entry(first).or_insert(p.entity);
            return;
        }
        // Decided at the end of the unit ([`Sema::inst_is_tu_local`]): the
        // instantiations used there, and the instantiated inline member
        // functions (of nested classes too), whose bodies are recorded as
        // an inline instantiation's.
        let mut uses: Vec<DeclId> =
            held.iter().filter(|p| p.spec).map(|p| self.ast.first(p.entity)).collect();
        let mut work: Vec<DeclId> = named.iter().copied().collect();
        let mut n = 0usize;
        while let Some(r) = work.pop() {
            n += 1;
            if n > MAX_WALK {
                break;
            }
            for m in self.ast.decls.members(r).collect::<Vec<_>>() {
                let k = self.kind_of(m);
                if k == DeclKind::CXXRecord {
                    work.push(m);
                } else if k.is_function() && self.is_inline_fn(m) {
                    uses.push(self.ast.first(m));
                }
            }
        }
        if !uses.is_empty() {
            let v = self.modules.inst_spec_uses.entry(first).or_default();
            for u in uses {
                if u != first && !v.contains(&u) {
                    v.push(u);
                }
            }
        }
    }

    /// The outermost local classes and closure types of function `f` its
    /// return type names ([`Sema::module_resolve_escapes`]).
    fn escaped_local_classes(&self, f: DeclId) -> kiln_source::fx::FxHashSet<DeclId> {
        let mut out = kiln_source::fx::FxHashSet::default();
        let Some(fty) = self.ast.decls.get(f).ty() else { return out };
        let TypeView::Function(p) = self.ast.types.view(fty.ty()) else { return out };
        let mut work = vec![p.ret()];
        let mut seen = kiln_source::fx::FxHashSet::default();
        let mut n = 0usize;
        while let Some(t) = work.pop() {
            n += 1;
            if n > 100_000 || self.is_error_type(t) {
                break;
            }
            let t = self.ast.types.canonical(t);
            if !seen.insert(t.ty()) {
                continue;
            }
            match self.ast.types.canonical_view(t) {
                TypeView::Pointer(x)
                | TypeView::LValueRef(x)
                | TypeView::RValueRef(x)
                | TypeView::IncompleteArray(x)
                | TypeView::Complex(x)
                | TypeView::Atomic(x) => work.push(x),
                TypeView::ConstantArray(a) => work.push(a.elem),
                TypeView::MemberPointer { pointee, class } => {
                    work.push(pointee);
                    work.push(QualType::unqual(class));
                }
                TypeView::Function(q) => {
                    work.push(q.ret());
                    work.extend(self.ast.types.params(&q).iter().copied());
                }
                TypeView::Record(d) => {
                    if let Some((_, i)) = self.ast.spec_info(d) {
                        let args = self.ast.types.canonical_args(i.args);
                        for a in self.ast.types.args(args).to_vec() {
                            if let ArgView::Type(x) = a.view() {
                                work.push(x);
                            }
                        }
                    }
                    let Some(c) = self.outermost_local_of(d, f) else { continue };
                    out.insert(c);
                    // What the class holds is reachable with it.
                    for m in self.ast.decls.members(d).collect::<Vec<_>>() {
                        if self.kind_of(m) == DeclKind::Field
                            && let Some(x) = self.ast.decls.get(m).ty()
                        {
                            work.push(x);
                        }
                    }
                    work.extend(self.ast.decls.bases(d, &self.ast.lists).map(|b| b.ty));
                }
                _ => {}
            }
        }
        out
    }

    /// The outermost class around (or being) `d` whose context is in the
    /// body of function `f`, if `d` is declared there.
    fn outermost_local_of(&self, d: DeclId, f: DeclId) -> Option<DeclId> {
        let mut last = d;
        let mut c = self.ast.decls.get(d).ctx();
        for _ in 0..MAX_WALK {
            let x = c?;
            if x == f {
                return Some(last);
            }
            match self.kind_of(x) {
                DeclKind::Namespace | DeclKind::TranslationUnit => return None,
                DeclKind::CXXRecord => last = x,
                _ => {}
            }
            c = self.ast.decls.get(x).ctx();
        }
        None
    }

    /// An odr-use at `loc` of `d`, found at the end of its
    /// full-expression: a variable usable in constant expressions with
    /// internal linkage is TU-local and only its uses that are not
    /// odr-uses are ignored ([basic.link]/15.4), so `return &K;` and a
    /// reference bound to `K` in an inline function make it an exposure
    /// (GCC 16.1 rejects; clang 22 accepts). Held as
    /// [`Sema::module_note_use`] holds a use.
    pub(crate) fn module_note_odr_use(&mut self, d: DeclId, loc: Loc) {
        if !self.exposure_use_point() || !self.is_tu_local_constant(d) {
            return;
        }
        let Some((owner, at, lambda, escape)) = self.exposure_site() else {
            if self.inst.stack.frames.is_empty() {
                self.note_concept_use(d, loc);
            }
            return;
        };
        if self.modules.exposed.contains(&owner) {
            return;
        }
        let p = PendingExposure {
            owner,
            at,
            entity: d,
            loc,
            lambda,
            odr_const: true,
            spec: false,
            escape,
        };
        self.modules.pending_exposures.push(p);
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
        // The uses in a local class or closure type of a non-inline
        // function's body wait for its deduced return type (WP-1820).
        let (escapes, mine): (Vec<PendingExposure>, Vec<PendingExposure>) =
            mine.into_iter().partition(|p| p.escape.is_some());
        let (spec_escapes, specs): (Vec<PendingExposure>, Vec<PendingExposure>) =
            specs.into_iter().partition(|p| p.escape.is_some());
        self.modules.escapes.extend(escapes);
        self.modules.escapes.extend(spec_escapes);
        if !self.inst.stack.frames.is_empty() {
            let (mine, specs) = self.inst_lambda_escapes(owner, mine, specs);
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

    /// The held uses of instantiated function `f` in a lambda, when `f` is
    /// not inline and was declared with a placeholder return type: held as
    /// in its closure type ([`PendingExposure::escape`]), an exposure if
    /// the deduced return type names that ([`Sema::module_resolve_escapes`],
    /// WP-1846); the others are returned.
    fn inst_lambda_escapes(
        &mut self,
        f: DeclId,
        mine: Vec<PendingExposure>,
        specs: Vec<PendingExposure>,
    ) -> (Vec<PendingExposure>, Vec<PendingExposure>) {
        if self.escape_pattern(f).is_none() {
            return (mine, specs);
        }
        let (lm, mine): (Vec<PendingExposure>, Vec<PendingExposure>) =
            mine.into_iter().partition(|p| p.lambda.is_some());
        let (ls, specs): (Vec<PendingExposure>, Vec<PendingExposure>) =
            specs.into_iter().partition(|p| p.lambda.is_some());
        let held = lm
            .into_iter()
            .chain(ls)
            .map(|p| PendingExposure { escape: p.lambda.map(|l| (f, l)), ..p });
        self.modules.escapes.extend(held);
        (mine, specs)
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
            // /15.1 ignores its body; in a local class of an inline
            // instantiation, clang 22 warns at a use of that (WP-1821).
            if !mine.is_empty() {
                self.note_inst_warning(first);
            }
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

    /// A use in instantiated function `f` that is not an exposure but gets
    /// clang 22's warning at a use of the instantiation (WP-1821): an
    /// enumerator of a TU-local enumeration in an inline one, a TU-local
    /// entity in a member function that is not inline of a local class of
    /// an inline one (that one is warned).
    fn note_inst_warning(&mut self, f: DeclId) {
        let first = self.ast.first(f);
        let at = if self.is_inline_fn(first) {
            Some(first)
        } else {
            self.ast.decls.get(first).ctx().and_then(|r| {
                (self.kind_of(r) == DeclKind::CXXRecord && self.ast.closure_lambda(r).is_none())
                    .then(|| self.innermost_function_of(r))
                    .flatten()
                    .filter(|&x| self.is_inline_fn(x))
                    .map(|x| self.ast.first(x))
            })
        };
        if let Some(x) = at {
            self.modules.inst_warn.insert(x);
        }
    }

    /// Whether a use of instantiation `s` gets clang 22's warning by
    /// [`Sema::note_inst_warning`], noted for it or for its template.
    fn inst_warned(&self, s: DeclId) -> bool {
        let w = &self.modules.inst_warn;
        !w.is_empty()
            && (w.contains(&s)
                || self
                    .ast
                    .instantiated_from(s)
                    .is_some_and(|p| self.ast.redecls_of(p).any(|r| w.contains(&r))))
    }

    /// The innermost function around declaration `d` that is not a
    /// lambda's call operator.
    fn innermost_function_of(&self, d: DeclId) -> Option<DeclId> {
        let mut c = self.ast.decls.get(d).ctx();
        for _ in 0..MAX_WALK {
            let x = c?;
            let k = self.kind_of(x);
            if k.is_function() && !self.is_lambda_call_op(x) {
                return Some(x);
            }
            if matches!(k, DeclKind::Namespace | DeclKind::TranslationUnit) {
                return None;
            }
            c = self.ast.decls.get(x).ctx();
        }
        None
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
            if !self.is_inline_fn(s) && self.is_function_spec(s) && self.inst_names_tu_local(s)
                || self.inst_warned(s)
            {
                self.warn_tu_local_exposed(p.entity, p.loc);
            }
        }
    }

    /// Whether instantiation `s` (a first declaration) is TU-local by
    /// [basic.link]/16.5: it is inline and its body names a TU-local
    /// entity, or its deduced return type names a local class or closure
    /// type whose declarations do ([`Sema::resolve_instantiated_escapes`]),
    /// or it names an instantiation that is TU-local so.
    fn inst_is_tu_local(&self, s: DeclId) -> bool {
        let mut seen = kiln_source::fx::FxHashSet::default();
        let mut work = vec![s];
        while let Some(x) = work.pop() {
            if seen.len() > 100_000 || !seen.insert(x) {
                continue;
            }
            // A function that is not inline is TU-local by the local
            // classes its deduced return type names (WP-1846), and only
            // those make it depend on the instantiations they use.
            if self.modules.escaped_tu_local.contains_key(&x)
                || self.is_inline_fn(x) && self.inst_names_tu_local(x)
            {
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
    fn exposure_owner(&self) -> Option<Site> {
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
                if self.is_tu_local_entity(c) {
                    return None;
                }
                if !self.is_inline_fn(c) {
                    // A lambda in the body of a function that is not
                    // inline: part of that body, unless its closure type
                    // is named by the function's deduced return type
                    // (WP-1820).
                    let closure = lambda?;
                    return match self.body_scope(closure) {
                        BodyScope::Escape(f, r) if f == c => {
                            Some((c, self.lambda_or(lambda, c), lambda, Some((f, r))))
                        }
                        _ => None,
                    };
                }
                let at = self.lambda_or(lambda, c);
                return match self.body_scope(c) {
                    BodyScope::Considered => Some((c, at, lambda, None)),
                    BodyScope::Escape(f, r) => Some((c, at, lambda, Some((f, r)))),
                    BodyScope::Ignored => None,
                };
            }
            match k {
                DeclKind::CXXRecord if self.ast.closure_lambda(c).is_some() => {}
                DeclKind::LinkageSpec | DeclKind::Export | DeclKind::RequiresExprBody => {}
                _ => break,
            }
            c = self.ast.decls.get(c).ctx()?;
        }
        if let Some(&(field, _)) = self.nsdmi_this.last()
            && self.ast.decls.get(field).ctx() == Some(c)
        {
            let (owner, escape) = self.nsdmi_exposure_owner(field)?;
            return Some((owner, self.lambda_or(lambda, owner), lambda, escape));
        }
        let v = self.depr_init?;
        let inline =
            self.ast.decls.var(v).is_some_and(|x| x.spec.is_inline() && !x.spec.is_constexpr());
        if !(inline || lambda.is_some() && self.ast.decls.var(v).is_some())
            || self.is_tu_local_entity(v)
        {
            return None;
        }
        Some((v, self.lambda_or(lambda, v), lambda, None))
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
    /// local class only where [`Sema::body_scope`] considers it (in an
    /// inline function that is not TU-local; a non-inline function's body
    /// is ignored, [basic.link]/15.1, unless its deduced return type
    /// names the class: then with the [`PendingExposure::escape`] pair).
    fn nsdmi_exposure_owner(&self, field: DeclId) -> Option<(DeclId, Option<(DeclId, DeclId)>)> {
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
        match self.body_scope(r) {
            BodyScope::Considered => Some((r, None)),
            BodyScope::Escape(f, c) => Some((r, Some((f, c)))),
            BodyScope::Ignored => None,
        }
    }

    /// The end of the default member initializer of `field`: its held
    /// uses are an exposure of its class (WP-1684; GCC 16.1 rejects,
    /// clang 22 accepts).
    pub(crate) fn module_field_init_exposures(&mut self, field: DeclId) {
        if self.modules.pending_exposures.is_empty() {
            return;
        }
        if let Some((owner, _)) = self.nsdmi_exposure_owner(field) {
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
            || self.modules.exposed.contains(&f)
            || self.is_tu_local_entity(class)
        {
            return;
        }
        // A local class's member as the class's other declarations
        // (WP-1820).
        let scope = match self.decl_context_scope(class) {
            Some(BodyScope::Ignored) | None => return,
            Some(s) => s,
        };
        if let Some(e) = self.default_arg_tu_local(p) {
            self.report_in_scope(f, e, scope);
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
            DeclKind::Enum | DeclKind::ClassTemplate | DeclKind::Concept => {
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
        // The uses in its decl-specifiers and declarator.
        let level = self.depr_points.len();
        let named = if self.modules.decl_uses.is_empty() {
            None
        } else {
            let first = self.modules.decl_uses.iter().find(|&&(l, ..)| l == level).map(|x| x.1);
            self.modules.decl_uses.retain(|&(l, ..)| l != level);
            first
        };
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
        let scope = match self.decl_context_scope(ctx) {
            Some(BodyScope::Ignored) | None => return,
            Some(s) => s,
        };
        if self.modules.exposed.contains(&owner) || self.is_tu_local_entity(owner) {
            return;
        }
        let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let mut found = self.type_tu_local(ty, 0);
        if found.is_none() && k.is_function() {
            found = self.default_args_tu_local(d);
        }
        if let Some(e) = found.or(named) {
            self.report_in_scope(owner, e, scope);
        }
    }

    /// For a declaration in context `ctx`: [`BodyScope::Considered`]
    /// outside any function, the [`Sema::local_class_scope`] of a local
    /// class, `None` in a block scope (a use there is held as the body's,
    /// [`Sema::module_note_use`]).
    fn decl_context_scope(&self, ctx: DeclId) -> Option<BodyScope> {
        if self.in_function_scope(ctx) {
            self.local_class_scope(ctx)
        } else {
            Some(BodyScope::Considered)
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
        let Some(ctx) = self.ast.decls.get(r).ctx() else { return };
        let scope = if self.in_function_scope(ctx) {
            match self.local_class_scope(r) {
                Some(BodyScope::Ignored) | None => return,
                Some(s) => s,
            }
        } else {
            BodyScope::Considered
        };
        if self.modules.exposed.contains(&r) || self.is_tu_local_entity(r) {
            return;
        }
        let bases: Vec<QualType> = self.ast.decls.bases(r, &self.ast.lists).map(|b| b.ty).collect();
        if let Some(e) = bases.into_iter().find_map(|t| self.type_tu_local(t, 0)) {
            self.report_in_scope(r, e, scope);
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
        if found.is_none() && !self.modules.set_uses.is_empty() {
            // A TU-local function in an overload set an id-expression
            // there refers to (WP-1819; [`Sema::module_note_overload_set`]).
            let (b, end) = (self.ast.begin_loc(e).to_raw(), self.ast.end_loc(e).to_raw());
            found = self
                .modules
                .set_uses
                .iter()
                .find(|(l, _)| (b..=end).contains(&l.to_raw()))
                .map(|&(_, d)| d);
        }
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
