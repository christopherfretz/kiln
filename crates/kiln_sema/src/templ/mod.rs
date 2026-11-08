//! Templates (spec/sema-templates.md). WP-069 lands the parse-time half:
//! template heads and parameters, attaching a head to the declaration it
//! introduces, the template-aware name classification and the dependent
//! expression nodes ([`actions`]). Instantiation, substitution, deduction
//! and satisfaction are WP-070..074. WP-070a lands the engine core:
//! argument checking ([`args`]), the substituter ([`subst`]), class
//! template instantiation ([`class`]), lazy member instantiation
//! ([`pending`]), the instantiation stack ([`stack`]) and the report
//! ([`report`]); deduction ([`deduce`]), partial ordering ([`order`]),
//! pack expansion and satisfaction still report
//! `sema_unimplemented_template_feature`.

pub mod actions;
pub mod args;
pub mod auto;
pub mod builtins;
pub mod class;
pub mod concepts;
pub mod ctad;
pub(crate) mod ctad_site;
pub mod deduce;
pub mod deduce_call;
#[cfg(feature = "op-stats")]
pub(crate) mod deduce_stats;
mod default_args;
mod equiv;
pub(crate) mod explicit_memo;
pub mod funcs;
pub mod member;
pub mod nttp;
pub mod order;
pub mod pack;
pub mod pending;
pub mod report;
pub mod spec;
pub mod stack;
pub mod subst;
mod ttarg;
mod ttarg22;
pub mod ttpkind;
pub mod var;
pub(crate) mod written;

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::ids::{ArgListId, TemplateParamListId};
use kiln_ast::names::DeclName;
use kiln_ast::templates::{SpecKind, TtpKind, is_template_kind};
use kiln_ast::types::{ArgView, TemplateArg, TypeView};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType};
use kiln_diag::{DiagId, FixIt};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::declspec::ParamInfo;
use crate::scope::{Ns, Scope, ScopeKind};

/// A type-constraint `C<A...>` of a placeholder: the concept and the
/// arguments after the constrained type.
pub type TypeConstraint = (DeclId, Vec<TemplateArg>);

/// A template parameter list being parsed or waiting for its
/// declaration (one per `ScopeKind::TemplateParams` scope, plus invented
/// heads of abbreviated function templates).
#[derive(Clone, Debug)]
pub(crate) struct HeadBuild {
    template_loc: Loc,
    l_angle: Loc,
    depth: u16,
    params: Vec<DeclId>,
    /// The finished list ([`Sema::act_template_head`]).
    list: Option<TemplateParamListId>,
    /// Consumed by a declaration (or matched to a qualifier).
    taken: bool,
    /// Holds only invented parameters (`auto:N`, [dcl.fct]/22) and has no
    /// scope.
    invented: bool,
}

/// What a class of an out-of-line declaration's qualifier expects of
/// the template headers ([`Sema::scope_slots`]).
#[derive(Clone, Copy, Debug)]
enum ScopeSlot {
    /// A class template's or partial specialization's parameter list.
    Params(TemplateParamListId),
    /// `template<>`, for this implicitly instantiated specialization.
    Empty(DeclId),
}

impl Sema<'_, '_> {
    /// Takes the innermost head for the declaration about to be created
    /// (a generic lambda's call operator, whose head is always its own).
    pub(crate) fn take_innermost_head(&mut self) {
        if let Some(h) = self.templ.heads.last_mut() {
            h.taken = true;
            self.templ.cur = h.list;
        }
    }
}

/// The template-declaration state the first declarator of a group
/// consumes: which heads it took and the pending `template<>` /
/// `[extern] template` / `template<> template<...>` markers
/// ([`Sema::save_template_decl_state`]).
#[derive(Clone, Debug, Default)]
pub struct TemplateDeclState {
    taken: Vec<bool>,
    explicit_spec: Option<Loc>,
    explicit_inst: Option<(Option<Loc>, Loc)>,
    member_spec: Option<Loc>,
}

impl Sema<'_, '_> {
    /// Before the first declarator of a template declaration, explicit
    /// specialization or explicit instantiation: the state its later
    /// declarators get back ([`Sema::restore_template_decl_state`]).
    pub fn save_template_decl_state(&self) -> TemplateDeclState {
        TemplateDeclState {
            taken: self.templ.heads.iter().map(|h| h.taken).collect(),
            explicit_spec: self.templ.explicit_spec,
            explicit_inst: self.templ.explicit_inst,
            member_spec: self.templ.member_spec,
        }
    }

    /// [temp.pre]/5: after "a template declaration can only declare a
    /// single entity", clang (`ParseDeclGroup`) still declares each later
    /// declarator of the group with the same template parameter lists
    /// (or as the same kind of specialization or instantiation): the
    /// heads the previous declarator took are open again.
    pub fn restore_template_decl_state(&mut self, s: &TemplateDeclState) {
        for (h, &t) in self.templ.heads.iter_mut().zip(&s.taken) {
            h.taken = t;
        }
        self.templ.explicit_spec = s.explicit_spec;
        self.templ.explicit_inst = s.explicit_inst;
        self.templ.member_spec = s.member_spec;
        self.templ.cur = None;
        self.templ.head_mismatch = false;
    }
}

impl HeadBuild {
    /// A written head whose list starts at `l_angle` (a lambda's
    /// *template-parameter-list*, which has no `template` keyword).
    pub(crate) fn explicit(l_angle: Loc, depth: u16) -> Self {
        HeadBuild {
            template_loc: Loc::INVALID,
            l_angle,
            depth,
            params: Vec::new(),
            list: None,
            taken: false,
            invented: false,
        }
    }
}

/// The template state of [`Sema`] (spec/sema-templates.md §11.2; the
/// engine's `InstState` is WP-070's).
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent parse-state flags")]
pub struct TemplState {
    pub(crate) heads: Vec<HeadBuild>,
    /// The open expansion statements' synthetic levels (innermost last):
    /// the function whose body holds each, and the dependent depth
    /// outside it ([`Sema::in_template_outside_expansions`]).
    pub(crate) expansions_open: Vec<(Option<DeclId>, u32)>,
    /// A declarator-id's nested-name-specifier is being parsed (clang's
    /// `EnteringContext`): `X<P...>::` names the current instantiation
    /// even outside `X`'s definition ([temp.mem]/1, out-of-line members).
    pub(crate) entering_context: bool,
    /// The last [`Sema::take_template_head`] diagnosed a head matching an
    /// implicitly instantiated class of the qualifier
    /// ([`DiagId::sema_template_params_for_non_template`]): the
    /// declaration is invalid.
    pub(crate) head_mismatch: bool,
    /// Where each partial specialization declared before its definition
    /// was defined: the redefinition note points there (kiln reuses the
    /// first declaration, clang adds a redeclaration).
    pub(crate) partial_def_loc: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// The head consumed by the declaration being built: its pattern is
    /// wrapped in a template decl by [`Sema::finish_template_pattern`].
    pub(crate) cur: Option<TemplateParamListId>,
    /// The number of invented parameters of the innermost abbreviated
    /// function template (`auto:N` names, [dcl.fct]/22).
    pub(crate) invented: u16,
    /// The declaration contexts saved by open requires-expression bodies.
    pub(crate) req_ctx: Vec<DeclId>,
    /// Where each template parameter's default argument starts (the
    /// redefinition and missing-default diagnostics point there).
    pub(crate) default_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// The element types of each expanded non-type parameter pack (a
    /// member template's `template<Ts... a>` once `Ts` is known,
    /// [temp.variadic]/5; §3.2), as a list of type arguments.
    pub(crate) expanded_packs: kiln_source::fx::FxHashMap<DeclId, ArgListId>,
    /// The template head just taken declares a member template of a
    /// local class ([temp.mem]/2), already diagnosed: the static data
    /// member check stays quiet (clang drops the declaration first).
    pub(crate) local_template: bool,
    /// The type of the expanded-pack element being converted (read by
    /// `convert_nttp_arg` in place of the parameter's declared type).
    pub(crate) nttp_elem: Option<QualType>,
    /// Where each argument of the template-id being parsed starts (set by
    /// the parser right before `act_template_id`; consumed there).
    pub(crate) arg_locs: Vec<Loc>,
    /// The `>` of that template-id, keyed by its first argument's location
    /// (read by the first argument check after the parse; the "while
    /// checking a default template argument used here" note points there).
    pub(crate) r_angle: Option<(Option<Loc>, Loc)>,
    /// `template<>` was just parsed: the next class template-id declared
    /// is an explicit specialization ([temp.expl.spec]; WP-070b).
    pub(crate) explicit_spec: Option<Loc>,
    /// `[extern] template` without `<` was just parsed (the `extern`
    /// and `template` locations): the next class template-id declared
    /// is explicitly instantiated ([temp.explicit]).
    pub(crate) explicit_inst: Option<(Option<Loc>, Loc)>,
    /// The `template<>` of `template<> template<class U> struct
    /// A<int>::B ...` ([temp.expl.spec]/16): the declaration under the
    /// inner head specializes member template `B` of the implicitly
    /// instantiated `A<int>`, which the outer `template<>` matched.
    pub(crate) member_spec: Option<Loc>,
    /// The `template` keywords of the leading `template<>` headers of the
    /// namespace-scope declaration being parsed, not yet matched to its
    /// qualifier ([`Sema::check_extra_spec_headers`]).
    pub(crate) spec_headers: Vec<Loc>,
    /// The instantiation-stack depth at which `explicit_spec` /
    /// `explicit_inst` was set: a declarator met while a class named in
    /// the declaration's qualifier is instantiated is not the explicit
    /// specialization (WP-106).
    pub(crate) explicit_depth: usize,
    /// The declarator handed to `act_on_declarator` next is followed by a
    /// function body (set by the parser; WP-106 diagnostics).
    pub(crate) fn_def_next: bool,
    /// Partial ordering results of partial specializations, by index
    /// pair: whether the first is at least as specialized as the second
    /// ([temp.spec.partial.order], §4.4).
    pub(crate) partial_order: kiln_source::fx::FxHashMap<(u32, u32), bool>,
    /// The first error substituting into a function template
    /// specialization's signature, by (invalid) specialization: the
    /// substitution-failure note of a later deduction that finds it
    /// memoized (§6.1 step 4).
    pub(crate) fn_spec_failures: kiln_source::fx::FxHashMap<DeclId, kiln_diag::Diag>,
    /// Templated functions made invalid by their body (the constexpr
    /// variable rule, `constexpr_body.rs`), not by their declaration: a
    /// deduction still runs, and its specialization is a substitution
    /// failure (clang marks only the `FunctionDecl` invalid, not its
    /// template, and an instantiation of it is invalid too). WP-414.
    pub(crate) body_invalid: kiln_source::fx::FxHashSet<DeclId>,
    /// Function template specializations whose explicit-specifier is
    /// not substituted yet: deduction checks the associated constraints
    /// first ([`Sema::get_function_spec_signature`], §6.1).
    pub(crate) explicit_pending: kiln_source::fx::FxHashSet<DeclId>,
    /// The warnings a SFINAE context held back while a function template
    /// specialization's signature was substituted, by specialization:
    /// reported at its first use (clang's `SuppressedDiagnostics`,
    /// emitted by `DiagnoseUseOfDecl`; §5.7).
    pub(crate) suppressed_diags: kiln_source::fx::FxHashMap<DeclId, Vec<kiln_diag::Diag>>,
    /// How many of a specialization's `suppressed_diags` lead the list as
    /// the forming deduction's own (its explicit-argument and default
    /// phases), ahead of the signature substitution's: a use that finds
    /// no match hands back only the signature's (WP-444).
    pub(crate) deduction_diags: kiln_source::fx::FxHashMap<DeclId, usize>,
    /// clang 19: implicit variable template specializations whose type
    /// was "deduced" as its own placeholder (`template<class T> auto v =
    /// v<T>;`, [`Sema::undeduced_var_spec_use_19`]); rejected at the end
    /// of a translation unit with no other error.
    pub(crate) self_typed_var_specs: Vec<DeclId>,
    /// The point of instantiation of the specialization the next call
    /// deduction forms, when it is not the deduction's location: a
    /// qualified callee's name (clang deduces at the callee's begin and
    /// marks the specialization referenced at its name).
    pub(crate) deduce_poi: Option<Loc>,
    /// The function parameters the last call deduction found
    /// non-dependent once its explicit template arguments were
    /// substituted, as a bit set by index (CWG1391: their conversions
    /// are checked before the specialization is formed, so a failure
    /// names the template; spec/sema-overload.md §10.4).
    pub(crate) explicit_nondep: u64,
    /// Pack elements produced by expansions (§11.3 `templ.pack_elements`).
    pub pack_elements: u64,
    /// The `...` of each fold-expression (clang `CXXFoldExpr::EllipsisLoc`;
    /// the AST node stores only the parentheses).
    pub(crate) fold_ellipsis: kiln_source::fx::FxHashMap<ExprId, Loc>,
    /// The dependent-scope marker of dependent `decltype(e)::` qualifiers
    /// (an unlinked alias of `<dependent type>`, created on first use;
    /// [`Sema::act_on_nested_name_decltype`]).
    pub(crate) decltype_scope: Option<DeclId>,
    /// A declarator qualifier `X<args>::` of class template `X` naming
    /// neither the template nor one of its partial specializations
    /// (`template<class T> void P<T*>::f()` with no `P<T*>`): the
    /// template and the specifier, reported by the declaration
    /// ([`Sema::diagnose_qualified_declaration`]).
    pub(crate) unmatched_qualifier: Option<(DeclId, kiln_ast::ids::NnsId)>,
    /// The object scope of a nested-name-specifier after `.`/`->`
    /// ([basic.lookup.qual.general]/2, WP-121): its first component is
    /// looked up in the object's class before the expression's context.
    /// Set by [`Sema::begin_member_qualifier`], taken by the first
    /// component.
    pub(crate) member_scope: Option<MemberScope>,
    /// The implicit deduction guides of each class template (first
    /// declaration), formed on first use ([over.match.class.deduct]/1).
    pub(crate) ctad_guides: kiln_source::fx::FxHashMap<DeclId, Vec<DeclId>>,
    /// The template-name location of each placeholder for a deduced
    /// class type, by the location of its type-specifier (the
    /// `while building implicit deduction guide` note's point).
    pub(crate) ctad_name_locs: kiln_source::fx::FxHashMap<Loc, Loc>,
    /// The type-specifier location of each variable declared with a
    /// placeholder for a deduced class type.
    pub(crate) ctad_var_type_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// The type-specifier location of the variable whose type is being
    /// deduced now; taken by [`Sema::deduce_ctad`].
    pub(crate) ctad_site: Option<Loc>,
    /// What each implicit guide among them (template decl) was formed
    /// from ([over.match.class.deduct]/1, [over.match.best.general]/2.12-13).
    pub(crate) ctad_origin: kiln_source::fx::FxHashMap<DeclId, ctad::GuideOrigin>,
    /// Each guide formed for an alias template from a guide of the
    /// template it aliases ([over.match.class.deduct]/3, P1814): the
    /// alias template and its defining-type-id.
    pub(crate) ctad_alias: kiln_source::fx::FxHashMap<DeclId, (DeclId, QualType)>,
    /// Each guide formed from a guide of a base class template whose
    /// constructors the class template inherits ([over.match.class.deduct]/1,
    /// P2582), and each alias template guide formed from one.
    pub(crate) ctad_inherited: kiln_source::fx::FxHashMap<DeclId, ctad::InheritedGuide>,
    /// The guide formed for (alias template or inheriting class template,
    /// source guide), if any.
    pub(crate) ctad_alias_cache: kiln_source::fx::FxHashMap<(DeclId, DeclId), Option<DeclId>>,
    /// Each template template parameter written without `class` or
    /// `typename` at clang compatibility 22 (spec/sema-templates.md):
    /// it names no template, so every argument is rejected.
    pub(crate) keyless_ttps: kiln_source::fx::FxHashSet<DeclId>,
    /// The `using` keyword of each alias declaration in a template:
    /// where clang declares the guides formed for an alias template.
    pub(crate) alias_using_loc: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// Where each dependent type and nested-name-specifier component was
    /// written, sorted (§5.8; [`Sema::written_loc`]).
    pub(crate) written_locs: kiln_source::fx::FxHashMap<written::WrittenKey, Vec<Loc>>,
    /// The `->` of each function declared with a dependent trailing
    /// return type: its dependent names are found after it (§5.8), not
    /// among the parameters' (`auto f(int (*p)(T::type)) -> T::type`).
    pub(crate) trailing_arrow: kiln_source::fx::FxHashMap<DeclId, Loc>,
}

/// The class an object expression names for the lookup of the first
/// component of a member access's qualifier.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MemberScope {
    /// The object's (complete) class.
    Class(DeclId),
    /// A type-dependent object: a name not found in the context is
    /// a dependent member of the object's type.
    Dependent,
}

/// The function whose body was current when a default argument's
/// conversion began, and the member definitions (with the location of
/// their use) the conversion named.
pub(crate) type DefaultArgUses = (Option<DeclId>, Vec<(DeclId, Loc)>);

/// The instantiation engine's state (spec/sema-templates.md §11.2).
#[derive(Debug, Default)]
pub struct InstState {
    /// The active instantiations, innermost last (§10.1).
    pub(crate) stack: stack::InstantiationStack,
    /// `-ftemplate-depth=` / `-ftemplate-backtrace-limit=`.
    pub limits: stack::InstLimits,
    /// Member definitions whose instantiation was required at a point
    /// where it could not run (§4.4), drained at end of TU.
    pub(crate) pending: std::collections::VecDeque<pending::PendingInst>,
    /// One entry per active SFINAE context: the errors it captured (§5.7).
    pub(crate) sfinae: std::cell::RefCell<Vec<stack::SfinaeState>>,
    /// Errors reported by function definition instantiations so far
    /// (nested ones counted once): a statement whose substitution only
    /// triggered a failing instantiation of another body is not itself
    /// in error (clang: that body is invalid; a call naming it is a
    /// `RecoveryExpr`).
    pub(crate) body_errors: u32,
    /// Alias template specializations already substituted (§4.6).
    pub(crate) alias_cache: kiln_source::fx::FxHashMap<(DeclId, ArgListId), QualType>,
    /// The instantiation report (§11.3).
    pub report: report::InstReport,
    /// A fatal error was emitted (the depth limit, the error limit, a
    /// missing `#include`, `-Wfatal-errors`): no further instantiation
    /// (clang `InstantiatingTemplate` is invalid once
    /// `hasFatalErrorOccurred`; WP-826). Set by
    /// [`Sema::instantiation_stopped`].
    pub(crate) fatal: bool,
    /// The class template specializations whose partial specialization
    /// is being selected (§4.4): a nested request for one of them finds
    /// it incomplete (clang `isAlreadyInstantiating`).
    pub(crate) selecting: Vec<DeclId>,
    /// The members already queued in `pending` (no duplicates).
    pub(crate) queued: kiln_source::fx::FxHashSet<DeclId>,
    /// While the end-of-TU drain instantiates one pending definition:
    /// the stack depth it started at, and the last frame popped back to
    /// that depth (the definition's own), which the drain keeps active
    /// while the instantiations it required run (§4.4).
    pub(crate) drain_capture: Option<(usize, Option<stack::InstFrame>)>,
    /// While a default argument is converted to its parameter's type:
    /// the function whose body was current then, and the member
    /// definitions the conversion would have queued for instantiation
    /// (`Sema::defer_default_arg_uses`).
    pub(crate) default_arg_uses: Option<DefaultArgUses>,
    /// A default argument's deferred instantiations, keyed by the stored
    /// default argument: queued where a call uses it.
    pub(crate) deferred_default_uses: kiln_source::fx::FxHashMap<ExprId, Vec<(DeclId, Loc)>>,
    /// Inline static data members of specializations whose initializer
    /// is not instantiated yet ([temp.inst]/4: on first reference).
    pub(crate) deferred_inits: kiln_source::fx::FxHashSet<DeclId>,
    /// Fields of class template specializations whose default member
    /// initializer is not instantiated yet, with the class and the
    /// pattern initializer ([temp.inst]/3: when a constructor or an
    /// aggregate initialization uses it; clang
    /// `InstantiateInClassInitializer`).
    pub(crate) deferred_nsdmis: kiln_source::fx::FxHashMap<DeclId, (DeclId, ExprId)>,
    /// The substitution state of each class owning a
    /// `deferred_nsdmis` field: its arguments and context pairs.
    pub(crate) nsdmi_ctx:
        kiln_source::fx::FxHashMap<DeclId, (args::MultiLevelArgs, Vec<(DeclId, DeclId)>)>,
    /// Non-inline static data members of specializations whose
    /// definition (the pattern's out-of-line one, [class.static.data]/3)
    /// is not instantiated yet ([temp.inst]/4: when odr-used or needed
    /// for constant evaluation).
    pub(crate) member_defs: kiln_source::fx::FxHashSet<DeclId>,
    /// The first reference to each variable template specialization
    /// whose pattern had no definition there (clang's point of
    /// instantiation), and whether it was odr-used since: an explicit
    /// instantiation definition still undefined at the end of the TU is
    /// then reported there (clang queues the use, §4.5).
    pub(crate) undefined_var_uses: kiln_source::fx::FxHashMap<DeclId, (Loc, bool)>,
    /// Variables that may be usable in constant expressions, odr-used
    /// while their definition could not be instantiated, queued only for
    /// the end of the TU's point of instantiation ([temp.point]/7): one
    /// still undefined there is not reported (clang never queues them,
    /// spec/sema-templates.md §4.6).
    pub(crate) quiet_pending: kiln_source::fx::FxHashSet<DeclId>,
    /// The end-of-TU drain has started: an instantiation from here on is
    /// handed to codegen in `end_of_tu_defs`.
    pub(crate) at_end_of_tu: bool,
    /// Declaring such a member: its `constexpr` does not imply `inline`
    /// until the initializer is instantiated (as clang).
    pub(crate) deferring_init: bool,
    /// The specialization just diagnosed as an implicit instantiation of
    /// an undefined template: the caller's own incomplete-type error for
    /// it is suppressed, as clang's `RequireCompleteType` reports only
    /// the instantiation failure.
    pub(crate) undefined_diagnosed: std::cell::Cell<Option<DeclId>>,
    /// Drop the next error passed to `emit` (set by `incomplete_note`).
    pub(crate) suppress_next_error: std::cell::Cell<bool>,
    /// Deducing the candidates of an explicit specialization or
    /// instantiation at clang 22 (§6.9): an access error inside a SFINAE
    /// context is no substitution failure, only the reason shown if the
    /// deduction fails otherwise (clang 21+ `SuppressAccessChecks`).
    pub(crate) suppress_access: std::cell::Cell<bool>,
    /// The template-id whose arguments `check_template_args` converts:
    /// its template name, the point of a `PriorArgSubstitution` frame.
    pub(crate) template_id_loc: Option<Loc>,
    /// The mem-initializers of constructor definitions in a template, as
    /// parsed, and their `:` (clang keeps the pattern's unresolved
    /// `CXXCtorInitializer`s for `InstantiateMemInitializers`).
    pub(crate) pattern_mem_inits:
        kiln_source::fx::FxHashMap<DeclId, (Vec<crate::class::MemInitInfo>, Loc)>,
    /// The constructor definitions in a template one of whose
    /// mem-initializers failed its definition-time check (diagnosed):
    /// their instantiations add no implicit initializers.
    pub(crate) failed_mem_inits: kiln_source::fx::FxHashSet<DeclId>,
    /// The location of the innermost running constant evaluation: the
    /// point of instantiation of a definition `ensure_body` instantiates
    /// (§4.11).
    pub(crate) eval_loc: Option<Loc>,
    /// The thread's stack guard (P4 (c)): the one installed when the
    /// state was created.
    pub(crate) guard: crate::stack_guard::StackGuard,
    /// The guard tripped: its fatal error was emitted, and every later
    /// check fails so the recursion unwinds.
    pub(crate) stack_exhausted: std::cell::Cell<bool>,
    /// Concepts and constraints (§8): normal forms, satisfaction caches.
    pub concepts: concepts::ConceptState,
    /// [temp.inst]/12: default arguments of instantiated functions not
    /// instantiated yet, by parameter: the pattern function and its
    /// parameter whose default argument is substituted at the first use
    /// (`default_args.rs`).
    pub(crate) lazy_defaults: kiln_source::fx::FxHashMap<DeclId, (DeclId, DeclId)>,
    /// The `vector_size` / `ext_vector_type` attribute location of a
    /// dependent vector type, by its size operand: clang diagnoses a
    /// substitution at the attribute, which the type does not keep.
    pub(crate) vector_attr_locs: kiln_source::fx::FxHashMap<ExprId, Loc>,
    /// The qualifier locations of each nullability-attributed dependent
    /// type, in parse order: clang diagnoses one that substitution makes
    /// a non-pointer at the qualifier, which the interned type does not
    /// keep (`subst/types.rs` picks the one nearest the declaration).
    pub(crate) nullability_locs: kiln_source::fx::FxHashMap<QualType, Vec<Loc>>,
}

impl Sema<'_, '_> {
    /// Reports a template feature that is designed but not implemented
    /// yet ("kiln: not yet implemented: {feature}"); never panics.
    pub fn templ_unimplemented(&self, feature: &str, loc: Loc) {
        let d = self.diag(DiagId::sema_unimplemented_template_feature, loc).arg(feature);
        self.emit(d);
    }

    /// Sets whether a declarator-id's nested-name-specifier is being
    /// parsed (clang's `EnteringContext`); returns the previous value.
    pub fn set_entering_context(&mut self, on: bool) -> bool {
        std::mem::replace(&mut self.templ.entering_context, on)
    }

    /// Records where each argument of the next template-id starts (the
    /// kind and conversion diagnostics point there, as clang's do) and
    /// where its `>` is.
    pub fn set_template_arg_locs(&mut self, locs: Vec<Loc>, r_angle: Loc) {
        self.templ.r_angle = Some((locs.first().copied(), r_angle));
        self.templ.arg_locs = locs;
    }

    /// The depth the next template parameter list gets: the number of
    /// enclosing non-empty lists (spec/sema-templates.md §2.1).
    pub fn template_depth(&self) -> u16 {
        self.templ.heads.len() as u16
    }

    /// True inside a template parameter scope (a dependent context,
    /// [temp.dep]).
    pub fn in_template(&self) -> bool {
        self.ast.templates.dependent_depth > 0
    }

    /// `template <` ([temp.pre]/1): opens the parameter scope; returns the
    /// depth of the parameters. Paired with
    /// [`Sema::act_on_finish_template_decl`].
    pub fn act_on_template_params_start(&mut self, template_loc: Loc, l_angle: Loc) -> u16 {
        let depth = self.template_depth();
        if self.in_local_scope() {
            self.error(DiagId::sema_template_outside_namespace_or_class, template_loc);
        }
        self.templ.heads.push(HeadBuild {
            template_loc,
            l_angle,
            depth,
            params: Vec::new(),
            list: None,
            taken: false,
            invented: false,
        });
        self.scopes.push(Scope::new(ScopeKind::TemplateParams, None));
        self.ast.templates.dependent_depth += 1;
        depth
    }

    /// Opens the synthetic template level of an expansion statement's
    /// for-range-declaration and compound-statement ([stmt.expand]/1:
    /// they are instantiated once per element, P1306R5): a head with no
    /// parameters and no scope, taken (no declaration consumes it), and a
    /// dependent context, so the declarations inside are patterns.
    /// Returns its depth. Paired with [`Sema::pop_expansion_level`].
    pub(crate) fn push_expansion_level(&mut self, loc: Loc) -> u16 {
        let depth = self.template_depth();
        self.templ.heads.push(HeadBuild {
            template_loc: loc,
            l_angle: loc,
            depth,
            params: Vec::new(),
            list: None,
            taken: true,
            invented: false,
        });
        let f = self.func.as_ref().map(|f| f.decl);
        self.templ.expansions_open.push((f, self.ast.templates.dependent_depth));
        self.ast.templates.dependent_depth += 1;
        depth
    }

    /// The body of the current function is in a template other than the
    /// synthetic levels of its own expansion statements: the levels of
    /// expansion statements in a non-template function are expanded
    /// before its body ends, so a `return` in one deduces the placeholder
    /// return type from its expansions ([dcl.spec.auto.general]/12).
    pub(crate) fn in_template_outside_expansions(&self) -> bool {
        let f = self.func.as_ref().map(|f| f.decl);
        match self.templ.expansions_open.iter().find(|e| e.0 == f && f.is_some()) {
            Some(&(_, outside)) => outside > 0,
            None => self.in_template(),
        }
    }

    /// Closes the level [`Sema::push_expansion_level`] opened.
    pub(crate) fn pop_expansion_level(&mut self) {
        self.templ.heads.pop();
        self.templ.expansions_open.pop();
        self.ast.templates.dependent_depth = self.ast.templates.dependent_depth.saturating_sub(1);
    }

    /// Leaves the template parameter scope of the innermost head.
    pub fn act_on_finish_template_decl(&mut self) {
        if let Some(i) = self.scopes.iter().rposition(|s| s.kind == ScopeKind::TemplateParams) {
            self.scopes.truncate(i);
        }
        // The invented head of an out-of-line member's abbreviated
        // function template sits above the class template's head
        // ([`Sema::act_on_invented_template_param`]); it ends first.
        while self.templ.heads.last().is_some_and(|h| h.invented) {
            self.templ.heads.pop();
            self.ast.templates.dependent_depth =
                self.ast.templates.dependent_depth.saturating_sub(1);
        }
        self.templ.heads.pop();
        self.ast.templates.dependent_depth = self.ast.templates.dependent_depth.saturating_sub(1);
    }

    fn param_head(&self, name: Option<(Symbol, Loc)>, loc: Loc) -> DeclHead {
        let dname = name.map_or(DeclName::EMPTY, |n| DeclName::ident(n.0));
        DeclHead::new(self.ctx, dname, name.map_or(loc, |n| n.1))
    }

    /// Records parameter `p` of the innermost head. Its name becomes
    /// visible at [`Sema::act_on_template_param_end`], after its default
    /// argument.
    fn push_template_param(&mut self, p: DeclId) {
        let Some(h) = self.templ.heads.last_mut() else { return };
        h.params.push(p);
    }

    /// The end of template parameter `p`, after its default argument if
    /// any: [basic.scope.pdecl]/9, the locus of a template parameter is
    /// immediately after its complete template-parameter, so its own
    /// default argument (`template<class...> class _And = _And`, libc++
    /// `tuple`) names the enclosing scope's entity, not the parameter.
    /// Makes its name visible; diagnoses a name declared twice in one
    /// list ([temp.local]/6).
    pub fn act_on_template_param_end(&mut self, p: DeclId) {
        let d = *self.ast.decls.get(p);
        let (Some(n), loc) = (d.name().as_ident(), d.loc()) else { return };
        let Some(s) = self.scopes.iter_mut().rev().find(|s| s.kind == ScopeKind::TemplateParams)
        else {
            return;
        };
        if let Some(prev) = s.find(n, Ns::Ordinary) {
            s.locals.push((n, p, Ns::Ordinary));
            let msg = self
                .diag(DiagId::sema_template_param_shadow, loc)
                .arg(format!("'{}'", self.ast.symbols().as_str(n)));
            let msg = self.note_prev_loc(msg, prev);
            self.emit(msg);
            return;
        }
        s.locals.push((n, p, Ns::Ordinary));
    }

    /// [temp.local]/6: a declaration of `name` (not a template
    /// parameter) whose ordinary lookup finds a template parameter
    /// redeclares it (clang's `DiagnoseTemplateParameterShadow`).
    pub(crate) fn check_template_param_shadow(&self, name: Symbol, loc: Loc) {
        if !self.in_template() {
            return;
        }
        let found = self.lookup_name(name, crate::lookup::LookupKind::Ordinary);
        let [p] = found.decls[..] else { return };
        if !kiln_ast::templates::is_template_parm_kind(self.kind_of(p)) {
            return;
        }
        let msg = self
            .diag(DiagId::sema_template_param_shadow, loc)
            .arg(format!("'{}'", self.ast.symbols().as_str(name)));
        let msg = self.note_prev_loc(msg, p);
        self.emit(msg);
    }

    fn note_prev_loc(&self, msg: kiln_diag::Diag, prev: DeclId) -> kiln_diag::Diag {
        let l = self.ast.decls.get(prev).loc();
        msg.note(DiagId::note_template_param_here, kiln_source::Span::empty(l))
    }

    fn next_param_index(&self) -> u16 {
        self.templ.heads.last().map_or(0, |h| h.params.len() as u16)
    }

    fn cur_depth(&self) -> u16 {
        self.templ.heads.last().map_or(0, |h| h.depth)
    }

    /// `class T`, `typename... Ts`, `typename = void`
    /// ([temp.param]/2): a type parameter.
    pub fn act_on_type_template_param(
        &mut self,
        typename_kw: bool,
        kw_loc: Loc,
        pack: bool,
        name: Option<(Symbol, Loc)>,
        at: Loc,
    ) -> DeclId {
        let h = self.param_head(name, at);
        let (depth, index) = (self.cur_depth(), self.next_param_index());
        let p = self.builder().template_type_parm(h, depth, index, pack, typename_kw);
        // Its first token, the key or the type-constraint (clang's
        // `getBeginLoc`; redeclaration diagnostics point there).
        if kw_loc != h.loc {
            let _ = self.ast.decls.cold.begin_loc.insert(p, kw_loc);
        }
        self.push_template_param(p);
        p
    }

    /// `C T`, `C<int>... Ts` ([temp.param]/4): a type parameter whose
    /// type-constraint `concept<args>` becomes its immediately-declared
    /// constraint `concept<T, args...>`. clang prints it with `class`.
    pub fn act_on_constrained_template_param(
        &mut self,
        concept: DeclId,
        cargs: &[TemplateArg],
        loc: Loc,
        pack: bool,
        (name, at): (Option<(Symbol, Loc)>, Loc),
    ) -> DeclId {
        let p = self.act_on_type_template_param(false, loc, pack, name, at);
        let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        let mut c = self.immediately_declared_constraint(concept, ty, cargs, loc);
        if pack {
            c = self.pack_constraint(c, loc);
        }
        self.builder().set_parm_constraint(p, c);
        p
    }

    /// The immediately-declared constraint of a constrained pack is the
    /// fold `(C<Ts> && ...)` ([temp.param]/4; clang's
    /// `formImmediatelyDeclaredConstraint`).
    pub(crate) fn pack_constraint(&mut self, c: ExprId, loc: Loc) -> ExprId {
        self.builder().fold_expr(Some(c), kiln_ast::expr::BinaryOp::LAnd, None, loc, loc)
    }

    /// `concept<ty, extra...>` as a `ConceptSpecializationExpr`
    /// ([temp.param]/4, [dcl.type.auto.deduct]).
    pub(crate) fn immediately_declared_constraint(
        &mut self,
        concept: DeclId,
        ty: QualType,
        extra: &[TemplateArg],
        loc: Loc,
    ) -> ExprId {
        let mut args = Vec::with_capacity(extra.len() + 1);
        args.push(TemplateArg::ty(ty));
        args.extend_from_slice(extra);
        let l = self.ast.types.arg_list(&args);
        self.builder().concept_spec_expr(concept, l, QualType::BOOL, loc)
    }

    /// A non-type template parameter ([temp.param]/2) declared like a
    /// function parameter `p`; `pack` for `T... N`.
    pub fn act_on_non_type_template_param(&mut self, p: &ParamInfo, pack: bool) -> DeclId {
        // [dcl.attr.annotation]/1: not on a template parameter.
        self.reject_annotations_in(&p.attrs, Some("a template parameter"));
        let h = self.param_head(p.name, p.loc);
        let (depth, index) = (self.cur_depth(), self.next_param_index());
        // `int... N` keeps the type `int` (as clang); only a type with
        // unexpanded packs (`Ts... Vs`) becomes a pack expansion.
        let mut ty = p.ty;
        if pack
            && self.ast.types.flags(ty.ty()).contains(kiln_ast::types::TypeFlags::UNEXPANDED_PACK)
        {
            ty = self.ast.types.pack_expansion(ty, None);
        }
        let d = self.builder().non_type_template_parm(h, ty, depth, index, pack);
        // [temp.variadic]/12: `template<Ts N>` (clang reports at the
        // parameter's name).
        let unexpanded = !pack && {
            let at = p.name.map_or(p.loc, |n| n.1);
            self.diagnose_unexpanded_declarator(ty, "non-type template parameter type", at)
        };
        self.declarator_type_invalid = false;
        if unexpanded || !self.check_nttp_type(ty, p.loc) {
            self.ast.decls.set_flags(d, DeclFlags::INVALID);
        } else {
            self.attach_nttp_type_constraint(d, p.ty, p.type_loc, pack);
        }
        self.push_template_param(d);
        d
    }

    /// [temp.param]/5: a non-type template parameter `d` whose type `ty`
    /// contains a placeholder with a type-constraint `C<args>` (`C auto
    /// V`, `C auto&... Vs`) introduces the immediately-declared
    /// constraint `C<decltype(V), args...>` (folded over a pack), one of
    /// the template's associated constraints ([temp.constr.decl]/3.1).
    /// As clang's `AttachTypeConstraint`, the constrained type is
    /// `decltype(V)`, the invented type when the type is the placeholder
    /// `C auto` itself. Another placeholder (`C auto&`, `C auto*`,
    /// `C decltype(auto)`) is checked when the argument deduces it
    /// ([dcl.type.auto.deduct]/4), as clang does (it rejects the first
    /// two "not supported yet").
    fn attach_nttp_type_constraint(&mut self, d: DeclId, ty: QualType, at: Loc, pack: bool) {
        let TypeView::Auto {
            keyword: kiln_ast::types::AutoKeyword::Auto,
            concept: Some(c),
            args,
            ..
        } = self.ast.types.canonical_view(ty)
        else {
            return;
        };
        let loc = self.ast.decls.get(d).loc();
        let at = if at.is_valid() { at } else { loc };
        let r = self.build_decl_ref(d, loc);
        let Some(t) = self.act_on_decltype(r, loc, loc) else { return };
        let extra = args.map(|a| self.ast.types.args(a).to_vec()).unwrap_or_default();
        let mut e = self.immediately_declared_constraint(c, t, &extra, at);
        if pack {
            e = self.pack_constraint(e, at);
        }
        self.builder().set_parm_constraint(d, e);
    }

    /// Starts the parameter list of a template template parameter
    /// (`template<class> class TT`): its parameters are one level deeper.
    pub fn act_on_template_template_params_start(&mut self, template_loc: Loc, l_angle: Loc) {
        let depth = self.template_depth();
        self.templ.heads.push(HeadBuild {
            template_loc,
            l_angle,
            depth,
            params: Vec::new(),
            list: None,
            taken: true,
            invented: false,
        });
        self.scopes.push(Scope::new(ScopeKind::TemplateParams, None));
        self.ast.templates.dependent_depth += 1;
    }

    /// `template<params> class TT` ([temp.param]/2), after
    /// [`Sema::act_on_template_template_params_start`] and the inner
    /// [`Sema::act_template_head`]: closes the inner list and declares
    /// the parameter in the outer one.
    pub fn act_on_template_template_param(
        &mut self,
        params: TemplateParamListId,
        typename_kw: bool,
        pack: bool,
        name: Option<(Symbol, Loc)>,
        at: Loc,
    ) -> DeclId {
        self.act_on_finish_template_decl();
        // [temp.param]/1: a template-head's parameter list is not empty
        // (clang err_template_template_parm_no_parms, at the name).
        let l = *self.ast.templates.param_list(params);
        if self.ast.lists.iter::<DeclId>(l.params).next().is_none() {
            let d = self.diag(DiagId::sema_template_template_param_no_params, at);
            self.emit(d);
        }
        let h = self.param_head(name, at);
        let (depth, index) = (self.cur_depth(), self.next_param_index());
        let p = self.builder().template_template_parm(h, params, depth, index, pack, typename_kw);
        self.push_template_param(p);
        p
    }

    /// Template template parameter `p` was written without `class` or
    /// `typename` (already diagnosed by the parser). At clang
    /// compatibility 22 it then names no template: its uses are not
    /// template names and every argument for it is rejected (clang 22's
    /// recovery); clang 19 recovers it as a type template.
    pub fn act_on_keyless_template_template_param(&mut self, p: DeclId) {
        if self.clang_compat >= 22 {
            let _ = self.templ.keyless_ttps.insert(p);
        }
    }

    /// A template template parameter's default (at `at`) names variable
    /// template or concept `a` ([temp.arg.template]/1): at clang
    /// compatibility 22 it is a template argument of the wrong kind, the
    /// parameter gets no default (clang 19 reports the parser's "must be a
    /// class template" instead).
    pub fn act_on_wrong_kind_ttp_default(&mut self, a: DeclId, at: Loc) {
        self.reject_keyless_ttp_arg(a, at);
    }

    /// clang 22's rejection of template `a` as the argument (at `at`)
    /// of a key-less template template parameter
    /// ([`Sema::act_on_keyless_template_template_param`]).
    pub(crate) fn reject_keyless_ttp_arg(&mut self, a: DeclId, at: Loc) {
        self.reject_ttp_arg_kind(TtpKind::Type, a, at);
    }

    /// clang 22's rejection of template `a` (at `at`) as the argument of
    /// a template template parameter of kind `want` whose kind differs
    /// ([temp.arg.template]/1, /3): "template argument does not refer to
    /// a {class or alias template|variable template|concept}, or template
    /// template parameter", with a note at `a`.
    pub(crate) fn reject_ttp_arg_kind(&mut self, want: TtpKind, a: DeclId, at: Loc) {
        let param_sel = match want {
            TtpKind::Type => 0u32,
            TtpKind::Variable => 1,
            TtpKind::Concept => 2,
        };
        let (sel, nloc) = match self.kind_of(a) {
            DeclKind::VarTemplate => (1u32, self.ast.decls.get(a).loc()),
            DeclKind::Concept => (2, self.ast.decls.get(a).loc()),
            DeclKind::TemplateTemplateParm => match self.ttp_kind(a) {
                TtpKind::Variable => (1, self.ast.decls.get(a).loc()),
                TtpKind::Concept => (2, self.ast.decls.get(a).loc()),
                TtpKind::Type => (0, self.ast.decls.get(a).loc()),
            },
            DeclKind::TypeAliasTemplate => {
                let pat = self.ast.template_info(a).and_then(|i| i.pattern);
                let l = pat.and_then(|p| self.templ.alias_using_loc.get(&p).copied());
                (0, l.unwrap_or_else(|| self.ast.decls.get(a).loc()))
            }
            _ => (0, self.ast.decls.get(a).loc()),
        };
        let note = self
            .diag(DiagId::note_template_arg_refers_to_template, nloc)
            .arg(sel)
            .arg(self.decl_name(a));
        let d = self
            .diag(DiagId::sema_template_arg_not_valid_template, at)
            .arg(param_sel)
            .with_note(note);
        self.emit(d);
    }

    /// The default argument `= arg` of parameter `p`
    /// ([temp.param]/10-12); `eq` is the `=`, `loc` the argument.
    pub fn act_on_template_param_default(
        &mut self,
        p: DeclId,
        arg: TemplateArg,
        eq: Loc,
        loc: Loc,
    ) {
        use crate::templ::pack::PackOperand;
        let chunk = std::mem::replace(&mut self.pending_chunk_loc, Loc::INVALID);
        // An invalid type-id is no default (clang's `ParseTypeParameter`
        // passes the invalid `TypeResult` on, which `ActOnTypeParameter`
        // ignores): `class U = R<T>*` with `R<T>` a reference.
        if let ArgView::Type(t) = arg.view()
            && self.is_error_type(t)
        {
            return;
        }
        if self.templ.keyless_ttps.contains(&p) {
            // Rejected and dropped (clang 22).
            if let ArgView::Template(n) = arg.view()
                && let Some(a) = self.ast.types.template_decl(n)
            {
                self.reject_keyless_ttp_arg(a, loc);
            }
            return;
        }
        // A default of the wrong kind ([temp.arg.template]/1, P2841R7) is
        // rejected and dropped (clang 22).
        if let ArgView::Template(n) = arg.view()
            && let Some(a) = self.arg_template_decl(n)
            && let Some(k) = self.template_arg_kind(a)
            && k != self.ttp_kind(p)
        {
            let want = self.ttp_kind(p);
            self.reject_ttp_arg_kind(want, a, loc);
            return;
        }
        if self.ast.template_parm(p).is_some_and(|x| x.pack) {
            // [temp.param]/11: "template parameter pack cannot have a
            // default argument", at the `=` (clang's
            // `ActOn*TemplateParameter`); the default is dropped.
            self.error(DiagId::sema_template_param_pack_default_arg, eq);
            return;
        }
        // [temp.variadic]/12: `class U = Ts` is reported at the
        // parameter's name, `int N = sizeof(Ts)` at the argument (clang's
        // `ActOn{Type,NonType}TemplateParameter`); the default is dropped.
        let op = match arg.view() {
            ArgView::Type(t) => Some((PackOperand::Type(t), self.ast.decls.get(p).loc())),
            ArgView::Expr { expr, .. } => Some((PackOperand::Expr(expr), self.begin_loc(expr))),
            _ => None,
        };
        // In a generic lambda's template-head an enclosing scope's packs
        // make the lambda-expression contain an unexpanded pack
        // ([temp.variadic]/5): `[]<class U = Ts>() {}...` expands `Ts`.
        if let Some((op, at)) = op {
            let unexpanded = if self.lambdas.is_empty() {
                self.diagnose_unexpanded_pack(op, "default argument", at)
            } else {
                self.inst.stack.is_empty()
                    && self.diagnose_lambda_local_packs(op, "default argument", at)
            };
            if unexpanded {
                return;
            }
        }
        if self.builder().set_template_default(p, arg).is_err() {
            self.error(DiagId::sema_template_param_default_redefined, loc);
            return;
        }
        let _ = self.templ.default_locs.insert(p, loc);
        if chunk.is_valid() && matches!(arg.view(), ArgView::Type(_)) {
            self.inner_chunk_locs.insert(p, chunk);
        }
    }

    /// `>` and the optional requires-clause of the innermost head
    /// ([temp.pre]/1): builds its `TemplateParameterList`, checking the
    /// pack and default-argument ordering rules ([temp.param]/14).
    ///
    /// Deviation from the spec's `act_template_head(params, requires,
    /// depth)`: the parameters were declared one by one through the
    /// `act_on_*_template_param` actions (they must be visible to the
    /// later parameters' defaults), so the head takes them from the
    /// innermost open list.
    pub fn act_template_head(
        &mut self,
        requires: Option<ExprId>,
        r_angle: Loc,
    ) -> TemplateParamListId {
        let Some(h) = self.templ.heads.last().cloned() else {
            return self.builder().template_param_list(
                Loc::INVALID,
                Loc::INVALID,
                r_angle,
                0,
                &[],
                requires,
            );
        };
        let l = self.builder().template_param_list(
            h.template_loc,
            h.l_angle,
            r_angle,
            h.depth,
            &h.params,
            requires,
        );
        if let Some(h) = self.templ.heads.last_mut() {
            h.list = Some(l);
        }
        l
    }

    /// clang's `CheckTemplateParameterList` for the template `kind` whose
    /// head is `list`, redeclaring the template `prev` if any:
    /// - [temp.param]/12: a default argument is not redefined by a
    ///   redeclaration; one given earlier is inherited;
    /// - [temp.param]/14: in a class, variable or alias template, every
    ///   parameter after one with a (possibly inherited) default has one or
    ///   is a pack, and a pack is the last parameter; in a concept
    ///   (P2841R7, `kind` = `Concept`), only the pack rule applies.
    pub(crate) fn check_template_param_list(
        &mut self,
        kind: DeclKind,
        list: TemplateParamListId,
        prev: Option<DeclId>,
    ) {
        let params: Vec<DeclId> = {
            let l = *self.ast.templates.param_list(list);
            self.ast.lists.iter::<DeclId>(l.params).collect()
        };
        // The parameter lists of every earlier declaration: a default
        // given by any of them is inherited by `prev` (clang's
        // `OldParam->hasDefaultArgument()` sees an inherited one).
        let olds: Vec<Vec<DeclId>> = match prev {
            Some(t) => match self.pattern_of(t) {
                Some(pat) => self
                    .ast
                    .redecls_of(pat)
                    .filter_map(|x| self.ast.described_template(x))
                    .map(|x| self.template_params(x))
                    .collect(),
                None => vec![self.template_params(t)],
            },
            None => Vec::new(),
        };
        let ordered = matches!(
            kind,
            DeclKind::ClassTemplate | DeclKind::VarTemplate | DeclKind::TypeAliasTemplate
        );
        let mut prev_default: Option<Loc> = None;
        for (i, &p) in params.iter().enumerate() {
            let new_loc = self.templ.default_locs.get(&p).copied();
            let old_loc = olds
                .iter()
                .find_map(|l| l.get(i).and_then(|&o| self.templ.default_locs.get(&o).copied()));
            let pack = self.ast.template_parm(p).is_some_and(|x| x.pack);
            match (new_loc, old_loc) {
                (Some(n), Some(o)) => {
                    let d = self.diag(DiagId::sema_template_param_default_redefined, n).note(
                        DiagId::note_template_param_prev_default,
                        kiln_source::Span::empty(o),
                    );
                    self.emit(d);
                    prev_default = Some(n);
                }
                (Some(l), None) | (None, Some(l)) => prev_default = Some(l),
                (None, None) => {
                    if ordered
                        && !pack
                        && let Some(l) = prev_default
                    {
                        let loc = self.ast.decls.get(p).loc();
                        let d = self.diag(DiagId::sema_template_param_missing_default, loc).note(
                            DiagId::note_template_param_has_default,
                            kiln_source::Span::empty(l),
                        );
                        self.emit(d);
                    }
                }
            }
            // [temp.param]/14: only a template parameter pack; a non-type
            // parameter that is a pack expansion of an enclosing level's
            // pack (`template<Ts... a, int n>` in a member template) is
            // not one (clang's `isPackExpansion`).
            // A concept's head obeys the pack rule but not the default
            // one (clang's `CheckTemplateParameterList` with
            // `TPC_Other`, which clang 19 and 22 apply to concepts).
            let pack_last = ordered || kind == DeclKind::Concept;
            if pack_last && pack && i + 1 != params.len() && !self.is_nttp_pack_expansion(p) {
                let loc = self.ast.decls.get(p).loc();
                self.error(DiagId::sema_template_param_pack_not_last, loc);
            }
        }
    }

    /// Whether `p` is a non-type template parameter whose type is a pack
    /// expansion (`Ts... a`, [temp.param]/17) rather than a template
    /// parameter pack of a plain type (`int... a`).
    pub(crate) fn is_nttp_pack_expansion(&self, p: DeclId) -> bool {
        self.kind_of(p) == DeclKind::NonTypeTemplateParm
            && self.ast.decls.get(p).ty().is_some_and(|t| pack::type_pattern(self, t).is_some())
    }

    /// The innermost untaken head the declaration being declared consumes
    /// ([temp.pre]/1), after matching the outer heads to the class
    /// templates named by its qualifier (`template<class T> void
    /// A<T>::f()`, [temp.mem]/1; `match_template_lists_to_scope`). Sets
    /// [`TemplState::cur`].
    pub(crate) fn take_template_head(
        &mut self,
        qualifier: Option<DeclId>,
    ) -> Option<TemplateParamListId> {
        self.templ.cur = None;
        self.templ.head_mismatch = false;
        self.templ.local_template = false;
        // The untaken heads directly around this declaration. A
        // declaration in a lambda body takes none of the heads outside
        // the lambda: they await the declaration whose declarator holds
        // the lambda (`template<class T> auto f() -> decltype([] { int
        // t; return t; }())`, [temp.pre]/2).
        let floor = self.lambda_head_floor();
        let mut open = Vec::new();
        for (i, h) in self.templ.heads.iter().enumerate().rev() {
            if h.taken || i < floor {
                break;
            }
            open.push(i);
        }
        open.reverse();
        let written = open.iter().filter(|&&i| !self.templ.heads[i].invented).count();
        let extra_diagnosed = self.check_extra_spec_headers(qualifier, written);
        if open.is_empty() {
            return None;
        }
        let check_empty = self.checks_member_spec_headers();
        let mut it = open.iter().copied().peekable();
        for slot in self.scope_slots(qualifier) {
            match slot {
                ScopeSlot::Params(expected) => {
                    let Some(i) = it.next() else { break };
                    self.templ.heads[i].taken = true;
                    // [temp.mem]/1: each head equivalent to its class
                    // template's (clang's `TemplateParameterListsAreEqual`
                    // in `MatchTemplateParametersToScopeSpecifier`).
                    if let Some(list) = self.templ.heads[i].list
                        && !self.templ.heads[i].invented
                    {
                        let _ = self.check_template_heads_match(list, expected);
                    }
                }
                ScopeSlot::Empty(class) if check_empty => {
                    let Some(&i) = it.peek() else { break };
                    if self.templ.heads[i].invented {
                        break;
                    }
                    // [temp.expl.spec]/15: only `template<>` matches an
                    // implicitly instantiated class (clang's
                    // `err_template_param_list_matches_nontemplate`).
                    let loc = self.templ.heads[i].template_loc;
                    let d = self
                        .diag(DiagId::sema_template_params_for_non_template, loc)
                        .arg(self.ty_arg(self.tag_type(class)));
                    self.emit(d);
                    for &j in &open {
                        self.templ.heads[j].taken = true;
                    }
                    self.templ.head_mismatch = true;
                    return None;
                }
                ScopeSlot::Empty(_) => {}
            }
        }
        let rest: Vec<usize> = it.collect();
        let &innermost = rest.last()?;
        if rest.len() > 1 && !extra_diagnosed {
            let loc = self.templ.heads[rest[0]].template_loc;
            self.error(DiagId::sema_extraneous_template_params, loc);
        }
        for &i in &rest {
            self.templ.heads[i].taken = true;
        }
        let list = self.templ.heads[innermost].list;
        self.templ.cur = list;
        list
    }

    /// Forgets the pending `template<>` headers after a qualifier that
    /// named nothing (diagnosed): they cannot be matched to its classes,
    /// so none is reported extraneous (clang's invalid `CXXScopeSpec`).
    pub fn discard_spec_headers(&mut self) {
        self.templ.spec_headers.clear();
    }

    /// Matches the pending `template<>` headers ([`TemplState::spec_headers`],
    /// taken) and the `written` non-empty heads after them to the classes
    /// of `qualifier`, outermost first, as clang's
    /// `MatchTemplateParametersToScopeSpecifier` does: an implicitly
    /// instantiated specialization takes a `template<>`, a class template
    /// (or partial specialization) a non-empty head, and an explicit
    /// specialization none, its members being declared as those of an
    /// ordinary class ([temp.expl.spec]/5). The last header is the
    /// declaration's own; any other left over is extraneous
    /// ([temp.expl.spec]/17): when all of them are `template<>`, clang 22's
    /// "extraneous template parameter list in template specialization"
    /// (`-Wextraneous-template-head`, an error by default; clang 19 a plain
    /// warning without a group), else the error clang also gives for
    /// extra non-empty heads (the declaration is then dropped), both at
    /// the first extra header's `template`.
    /// A `template<>` left over when the qualifier names (or is nested in)
    /// an explicit specialization adds "'template<>' header not required
    /// for explicitly-specialized class ..." at that specialization.
    /// Returns whether it diagnosed; only the case where an extra header
    /// is a `template<>` is handled here (extra non-empty heads alone are
    /// [`Self::take_template_head`]'s), and a `template<>` meeting a class
    /// template, or a non-empty head an implicit specialization, is left
    /// to the other checks.
    pub(crate) fn check_extra_spec_headers(
        &mut self,
        qualifier: Option<DeclId>,
        written: usize,
    ) -> bool {
        if self.inst.stack.len() != self.templ.explicit_depth {
            return false;
        }
        let empties = std::mem::take(&mut self.templ.spec_headers);
        let n = empties.len();
        if n == 0 {
            return false;
        }
        let total = n + written;
        let mut consumed = 0;
        for slot in self.scope_slots(qualifier) {
            if consumed == total {
                break;
            }
            let empty = consumed < n;
            match slot {
                ScopeSlot::Empty(_) if !empty => return false,
                ScopeSlot::Params(_) if empty => return false,
                _ => consumed += 1,
            }
        }
        // Extra headers: `consumed .. total - 1`; only those with a
        // `template<>` among them (`consumed < n`) are handled here.
        if consumed + 1 >= total || consumed >= n {
            return false;
        }
        let all_empty = total - 1 <= n;
        let loc = empties[consumed];
        let id = if !all_empty {
            // clang marks the declaration invalid (dropped here, as for a
            // head matching an implicit specialization).
            self.templ.head_mismatch = true;
            DiagId::sema_extraneous_template_params
        } else if self.clang_compat >= 22 {
            DiagId::sema_template_spec_extra_headers
        } else {
            DiagId::sema_template_spec_extra_headers_clang19
        };
        let mut d = self.diag(id, loc);
        if let Some(q) = qualifier
            && let Some(spec) = self.explicit_spec_in_scope(q)
        {
            let note = self
                .diag(
                    DiagId::note_explicit_template_spec_does_not_need_header,
                    self.ast.decls.get(spec).loc(),
                )
                .arg(format!("'{}'", self.ty_canonical_str(self.tag_type(q))));
            d = d.with_note(note);
        }
        self.emit(d);
        true
    }

    /// The explicitly specialized class `c` is, or is nested in, if any
    /// (clang's `ExplicitSpecLoc` walk in
    /// `MatchTemplateParametersToScopeSpecifier`): a class template
    /// specialization declared with `template<>`, or a member class of an
    /// implicit specialization explicitly specialized.
    fn explicit_spec_in_scope(&self, mut c: DeclId) -> Option<DeclId> {
        for _ in 0..1024 {
            if self.kind_of(c) == DeclKind::CXXRecord {
                let first = self.ast.first(c);
                if self.ast.described_template(first).is_none()
                    && self.ast.partial_spec_info(first).is_none()
                {
                    if let Some((_, info)) = self.ast.spec_info(first) {
                        if info.kind == SpecKind::ExplicitSpecialization {
                            return Some(c);
                        }
                    } else if self
                        .ast
                        .member_spec_info(first)
                        .is_some_and(|(_, m)| m.kind == SpecKind::ExplicitSpecialization)
                    {
                        return Some(c);
                    }
                }
            }
            c = self.ast.decls.get(c).ctx()?;
        }
        None
    }

    /// [temp.mem]/2: a local class shall not have member templates
    /// (clang's `CheckTemplateDeclScope`, at the `template` keyword of the
    /// head just taken). A friend template's scope is the enclosing
    /// function ([temp.pre]/2: "templates can only be declared in
    /// namespace or class scope"); it is dropped (true). A member
    /// template stays declared, the static data member check quiet. An
    /// abbreviated function template (`void f(auto)`, no written head)
    /// is diagnosed at `begin`, where its declaration starts (clang 22;
    /// clang 19 gives no location).
    pub(crate) fn check_template_in_local_class(&mut self, friend: bool, begin: Loc) -> bool {
        let Some(list) = self.templ.cur else { return false };
        if self.kind_of(self.ctx) != DeclKind::CXXRecord {
            return false;
        }
        let class = self.ast.definition(self.ctx).unwrap_or(self.ctx);
        if !self.is_local_class(class) {
            return false;
        }
        let invented = self.templ.heads.iter().any(|h| h.invented && h.list == Some(list));
        let loc = if invented && begin != Loc::INVALID {
            begin
        } else {
            self.ast.templates.param_list(list).template_loc
        };
        if friend {
            self.error(DiagId::sema_template_outside_namespace_or_class, loc);
            self.templ.cur = None;
            return true;
        }
        self.error(DiagId::sema_template_decl_in_local, loc);
        self.templ.local_template = true;
        false
    }

    /// How many template parameter lists the qualifier of an out-of-line
    /// declaration accounts for: one per class template (pattern) or
    /// class template partial specialization on its context chain that is not already open (clang's
    /// `MatchTemplateParametersToScopeSpecifier`, [temp.mem]/1). Explicit
    /// specializations (`template<>`) are not matched yet (WP-070a).
    pub fn match_template_lists_to_scope(&self, qualifier: Option<DeclId>) -> usize {
        self.scope_slots(qualifier).iter().filter(|s| matches!(s, ScopeSlot::Params(_))).count()
    }

    /// The template headers the classes of qualifier `qualifier` expect,
    /// outermost first (clang's `MatchTemplateParametersToScopeSpecifier`):
    /// a class template (pattern) or partial specialization not already
    /// open expects its parameter list, an implicitly instantiated (or
    /// explicitly instantiated) specialization `template<>`. The walk
    /// stops at an explicit specialization, whose members are declared
    /// as those of an ordinary class ([temp.expl.spec]/4).
    fn scope_slots(&self, qualifier: Option<DeclId>) -> Vec<ScopeSlot> {
        let Some(mut c) = qualifier else { return Vec::new() };
        let mut slots = Vec::new();
        for _ in 0..1024 {
            if self.kind_of(c) == DeclKind::CXXRecord
                && !self.defining.iter().any(|&d| self.ast.first(d) == self.ast.first(c))
            {
                let first = self.ast.first(c);
                let list = match self.ast.described_template(first) {
                    Some(t) => self.ast.template_info(t).map(|i| i.params),
                    None => self.ast.partial_spec_info(first).map(|(_, p)| p.params),
                };
                if let Some(list) = list {
                    slots.push(ScopeSlot::Params(list));
                } else if let Some((_, info)) = self.ast.spec_info(first) {
                    if info.kind == SpecKind::ExplicitSpecialization {
                        break;
                    }
                    slots.push(ScopeSlot::Empty(c));
                } else if self
                    .ast
                    .member_spec_info(first)
                    .is_some_and(|(_, m)| m.kind == SpecKind::ExplicitSpecialization)
                {
                    // A member class specialization (`template<> struct
                    // O<long>::I { ... };`) is an ordinary class too.
                    break;
                }
            }
            match self.ast.decls.get(c).ctx() {
                Some(p) => c = p,
                None => break,
            }
        }
        slots.reverse();
        slots
    }

    /// Whether the declaration being declared matches `template<>`
    /// headers to the implicitly instantiated classes of its qualifier:
    /// one at namespace scope, not after `template<>` or an explicit
    /// instantiation's `template` (those forms are matched elsewhere).
    fn checks_member_spec_headers(&self) -> bool {
        self.templ.explicit_spec.is_none()
            && self.templ.explicit_inst.is_none()
            && self.templ.member_spec.is_none()
            && matches!(self.kind_of(self.ctx), DeclKind::Namespace | DeclKind::TranslationUnit)
    }

    /// `void A<int>::f() {}` at namespace scope: a member of an implicitly
    /// instantiated specialization is explicitly specialized only after
    /// `template<>` ([temp.expl.spec]/1; clang's
    /// `err_template_spec_needs_header` from
    /// `MatchTemplateParametersToScopeSpecifier`). A head left for that
    /// class is diagnosed by [`Self::take_template_head`] instead.
    pub(crate) fn check_member_spec_header(
        &mut self,
        qualifier: Option<DeclId>,
        name_loc: Loc,
        start: Loc,
    ) {
        if !self.checks_member_spec_headers() {
            return;
        }
        let mut heads =
            self.templ.heads.iter().rev().take_while(|h| !h.taken).filter(|h| !h.invented).count();
        for slot in self.scope_slots(qualifier) {
            match slot {
                ScopeSlot::Params(_) => heads = heads.saturating_sub(1),
                ScopeSlot::Empty(_) if heads > 0 => return,
                ScopeSlot::Empty(_) => {
                    let d = self
                        .diag(DiagId::sema_template_spec_needs_header, name_loc)
                        .fixit(FixIt::insert(start, "template<> "));
                    self.emit(d);
                    return;
                }
            }
        }
    }

    /// "previous definition is here" on partial specialization `p`'s
    /// definition ([`TemplState::partial_def_loc`]).
    pub(crate) fn note_prev_partial_def(&self, msg: kiln_diag::Diag, p: DeclId) -> kiln_diag::Diag {
        let loc = match self.templ.partial_def_loc.get(&p) {
            Some(&l) => l,
            None => self.ast.decls.get(self.ast.definition(p).unwrap_or(p)).loc(),
        };
        msg.with_note(self.diag(DiagId::note_previous_definition, loc))
    }

    /// Called right before the builder creates the declaration a head
    /// consumed: the next declaration becomes the template's pattern.
    pub(crate) fn mark_pattern_next(&mut self) {
        if self.templ.cur.is_some() {
            self.ast.templates.pattern_next = true;
        }
    }

    /// Wraps `pattern` in its template decl when the declaration consumed
    /// a head ([`Sema::take_template_head`]); returns the template decl.
    pub(crate) fn finish_template_pattern(&mut self, pattern: DeclId) -> Option<DeclId> {
        let list = self.templ.cur.take()?;
        let kind = match self.kind_of(pattern) {
            DeclKind::CXXRecord => DeclKind::ClassTemplate,
            DeclKind::Var => DeclKind::VarTemplate,
            DeclKind::TypeAlias => DeclKind::TypeAliasTemplate,
            k if k.payload() == kiln_ast::decl::PayloadClass::Function => {
                DeclKind::FunctionTemplate
            }
            _ => {
                let loc = self.ast.decls.get(pattern).loc();
                self.templ_unimplemented("this kind of template declaration", loc);
                return None;
            }
        };
        let prev = self.ast.prev(pattern).and_then(|p| self.ast.described_template(p));
        // [temp.over.link]/6: a class or variable template is redeclared
        // with an equivalent head (alias templates are checked by
        // `act_on_typedef`, function templates with other heads are
        // overloads).
        // A friend class template in a dependent context (`template<C U>
        // friend class X;` in a class template) is compared with neither
        // side here: its head is deeper than the namespace scope
        // template's and may name the enclosing parameters (clang's
        // `CheckClassTemplate`). A friend redeclaration is compared when
        // the enclosing class is instantiated (`check_friend_class_head`).
        let dependent_friend = kind == DeclKind::ClassTemplate
            && self.ast.decls.get(pattern).misc().friend() != kiln_ast::decl::FriendKind::None
            && self.ast.templates.param_list(list).depth > 0;
        let old = prev
            .filter(|_| !dependent_friend)
            .and_then(|t| self.head_comparable_decl(t))
            .and_then(|t| self.ast.template_info(t))
            .map(|i| i.params);
        let heads_match = match old {
            Some(old) if matches!(kind, DeclKind::ClassTemplate | DeclKind::VarTemplate) => {
                self.check_template_heads_match(list, old)
            }
            _ => true,
        };
        if heads_match {
            self.check_template_param_list(kind, list, prev);
        }
        let t = self.builder().template_decl(kind, pattern, list, prev);
        // clang `ActOnFunctionDeclarator`: an invalid function makes its
        // template invalid, which lookup then ignores beside other
        // declarations (`LookupResult::resolveKind`, WP-308).
        if kind == DeclKind::FunctionTemplate && self.ast.decls.get(pattern).has(DeclFlags::INVALID)
        {
            self.ast.decls.set_flags(t, DeclFlags::INVALID);
        }
        if prev.is_some() {
            self.merge_template_defaults(t);
        }
        if let (Some(old), true) = (old, heads_match) {
            self.record_inherited_defaults(list, old);
        }
        Some(t)
    }

    /// Records on the AST the default arguments the parameters of head
    /// `new` inherit from the previous declaration's head `old`
    /// ([temp.param]/12; clang `CheckTemplateParameterList`'s
    /// `setInheritedDefaultArgument`): a parameter with no default of its
    /// own inherits its counterpart's, owned by the parameter that wrote
    /// it, which `-ast-dump` prints as `inherited from`. Only earlier
    /// declarations pass defaults on; [`Self::merge_template_defaults`]
    /// keeps the union that instantiation reads.
    fn record_inherited_defaults(&mut self, new: TemplateParamListId, old: TemplateParamListId) {
        let params = |l: TemplateParamListId| -> Vec<DeclId> {
            let list = self.ast.templates.param_list(l);
            self.ast.lists.iter::<DeclId>(list.params).collect()
        };
        let (new, old) = (params(new), params(old));
        let tt = &self.ast.templates;
        let owners: Vec<(DeclId, DeclId)> = new
            .iter()
            .zip(&old)
            .filter(|&(&q, _)| tt.defaults.get(q).is_none())
            .filter_map(|(&q, &o)| {
                let owner =
                    if tt.defaults.get(o).is_some() { o } else { *tt.inherited_defaults.get(o)? };
                Some((q, owner))
            })
            .collect();
        for (q, owner) in owners {
            let _ = self.builder().set_inherited_template_default(q, owner);
        }
    }

    /// Shares the default template arguments across the declarations of
    /// template `t` ([temp.param]/12: the defaults available are the
    /// union of those of all declarations), recording each one for the
    /// parameters at its position that have none of their own.
    fn merge_template_defaults(&mut self, t: DeclId) {
        let Some(p) = self.pattern_of(t) else { return };
        let lists: Vec<Vec<DeclId>> = self
            .ast
            .redecls_of(p)
            .filter_map(|x| self.ast.described_template(x))
            .map(|x| self.template_params(x))
            .collect();
        let n = lists.iter().map(Vec::len).max().unwrap_or(0);
        for i in 0..n {
            // Inherited ones too.
            let any = |q: &DeclId| self.template_default(*q);
            let Some(a) = lists.iter().find_map(|l| l.get(i).and_then(any)) else { continue };
            for l in &lists {
                if let Some(&q) = l.get(i)
                    && self.template_default(q).is_none()
                {
                    self.inherited_defaults.insert(q, a);
                }
            }
        }
    }

    /// The declaration of template `t` whose parameters stand for the
    /// template: its definition's, else the first one that is not a
    /// friend declaration, else the first. A friend redeclaration nested
    /// in a class template has deeper parameters (libc++
    /// `template <class _CharT, class _Traits, class _Alloc> friend class
    /// basic_string;` in `__wrap_iter`, which may come first), and can have
    /// no default arguments ([temp.param]/12).
    pub(crate) fn head_template(&self, t: DeclId) -> DeclId {
        let first = self.ast.first(t);
        let Some(p) = self.pattern_of(t) else { return first };
        if let Some(d) = self.ast.definition(p).and_then(|d| self.ast.described_template(d)) {
            return d;
        }
        let mut head = first;
        for x in self.ast.redecls_of(p) {
            if self.ast.decls.get(x).misc().friend() == kiln_ast::decl::FriendKind::None
                && let Some(d) = self.ast.described_template(x)
            {
                head = d;
            }
        }
        head
    }

    /// The latest declaration of template `t` that is not a friend
    /// declaration, else the first: the one name lookup finds (a friend
    /// declaration is invisible to it, [namespace.memdef]/3), whose head
    /// clang's notes and constraint checks of a template-id name (its
    /// "template is declared here", `[with C = char]`). Its parameters
    /// are at the template's own depth, as [`Self::head_template`]'s.
    pub(crate) fn visible_template(&self, t: DeclId) -> DeclId {
        let first = self.ast.first(t);
        let Some(p) = self.pattern_of(t) else { return first };
        self.ast
            .redecls_of(p)
            .filter(|&x| self.ast.decls.get(x).misc().friend() == kiln_ast::decl::FriendKind::None)
            .find_map(|x| self.ast.described_template(x))
            .unwrap_or(first)
    }

    /// The template decl `d` (a template, or the pattern of one) is part
    /// of, if any.
    pub fn template_of(&self, d: DeclId) -> Option<DeclId> {
        if is_template_kind(self.kind_of(d)) { Some(d) } else { self.ast.described_template(d) }
    }

    /// The most recent pattern of template `t`.
    pub(crate) fn pattern_of(&self, t: DeclId) -> Option<DeclId> {
        self.ast.template_info(t).and_then(|i| i.pattern)
    }

    /// Re-enters the template parameter scopes of `f`'s template (and of
    /// the class templates around it that are not open) for a body parsed
    /// after its declaration (a member function template of a class,
    /// [class.mem]/7). A member class template partial specialization's
    /// own parameters are re-entered the same way ([temp.spec.partial.member],
    /// [temp.local]/8: they are in scope in its member bodies). Returns
    /// how many heads were pushed.
    pub fn act_on_reenter_template_scopes(&mut self, f: DeclId) -> usize {
        let params_id = match self.ast.described_template(f) {
            Some(t) => match self.ast.template_info(t) {
                Some(info) => info.params,
                None => return 0,
            },
            None => match self.ast.partial_spec_info(f) {
                Some((_, ps)) => ps.params,
                None => return 0,
            },
        };
        let list = *self.ast.templates.param_list(params_id);
        if self.templ.heads.iter().any(|h| h.list == Some(params_id)) {
            return 0;
        }
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
        self.templ.heads.push(HeadBuild {
            template_loc: list.template_loc,
            l_angle: list.l_angle,
            depth: list.depth,
            params: params.clone(),
            list: Some(params_id),
            taken: true,
            invented: false,
        });
        let mut s = Scope::new(ScopeKind::TemplateParams, None);
        for p in params {
            if let Some(n) = self.ast.decls.get(p).name().as_ident() {
                s.locals.push((n, p, Ns::Ordinary));
            }
        }
        // Below the scopes of the function's own definition.
        self.scopes.push(s);
        self.ast.templates.dependent_depth += 1;
        1
    }

    /// Leaves the `n` template parameter scopes entered by
    /// [`Sema::act_on_reenter_template_scopes`].
    pub fn act_on_exit_reentered_template_scopes(&mut self, n: usize) {
        for _ in 0..n {
            if self.scopes.last().is_some_and(|s| s.kind == ScopeKind::TemplateParams) {
                self.scopes.pop();
            }
            self.templ.heads.pop();
            self.ast.templates.dependent_depth =
                self.ast.templates.dependent_depth.saturating_sub(1);
        }
    }

    /// The invented type parameter `auto:N` of a parameter whose type
    /// contains `auto` ([dcl.fct]/22): added to the innermost open head,
    /// or to a new invented head one level deeper when the function has
    /// none. Returns its type; `constraint` is the type-constraint `C` of
    /// `C auto` with its extra arguments. `qualifier` is the function
    /// declarator's: the heads its class templates account for
    /// (`template<class T> void A<T>::f(C auto)`, [temp.mem]/1) are not
    /// the function's own, which is then invented ([dcl.fct]/22).
    pub fn act_on_invented_template_param(
        &mut self,
        loc: Loc,
        pack: bool,
        constraint: Option<(DeclId, Vec<TemplateArg>, Loc)>,
        param_name: Option<Symbol>,
        qualifier: Option<DeclId>,
    ) -> QualType {
        // An explicit head is open once its list is built; an invented one
        // collects every `auto` parameter of the declarator.
        let scope_heads = || {
            let open = self
                .templ
                .heads
                .iter()
                .rev()
                .take_while(|h| !h.taken)
                .filter(|h| !h.invented)
                .count();
            open <= self.match_template_lists_to_scope(qualifier)
        };
        // A generic lambda's first `auto` parameter opens the lambda's
        // own head even when the enclosing one is still open (a lambda
        // in a concept's constraint-expression, [temp.constr.atomic]/3).
        let needs_new = self.in_headless_lambda_declarator()
            || self.templ.heads.last().is_none_or(|h| {
                h.taken
                    || (!h.invented && (h.list.is_none() || qualifier.is_some() && scope_heads()))
            });
        if needs_new {
            let depth = self.template_depth();
            self.templ.heads.push(HeadBuild {
                template_loc: loc,
                l_angle: loc,
                depth,
                params: Vec::new(),
                list: None,
                taken: false,
                invented: true,
            });
            self.ast.templates.dependent_depth += 1;
            self.templ.invented = 0;
            // The parameters before this one were declared outside the
            // template the declarator now turns out to be ([dcl.fct]/22).
            if let Some(f) = self.prototype_params.last() {
                let before: Vec<DeclId> = f.params.iter().map(|&(_, d)| d).collect();
                for d in before {
                    self.ast.decls.mark_dependent_param(d);
                }
            }
        }
        self.templ.invented += 1;
        let (depth, index) = (self.cur_depth(), self.next_param_index());
        // clang's names: `x:auto` for parameter `x`, else `auto:N` with
        // `N` the 1-based position in the list.
        let name = match param_name {
            Some(s) => {
                let text = format!("{}:auto", self.ast.symbols().as_str(s));
                self.intern(&text)
            }
            None => self.intern(&format!("auto:{}", u32::from(index) + 1)),
        };
        let h = DeclHead::new(self.ctx, DeclName::ident(name), loc).flags(DeclFlags::IMPLICIT);
        let p = self.builder().template_type_parm(h, depth, index, pack, false);
        if let Some(head) = self.templ.heads.last_mut() {
            head.params.push(p);
        }
        let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        if let Some((c, extra, cloc)) = constraint {
            let cloc = if cloc.is_valid() { cloc } else { loc };
            let mut e = self.immediately_declared_constraint(c, ty, &extra, cloc);
            if pack {
                e = self.pack_constraint(e, loc);
            }
            self.builder().set_parm_constraint(p, e);
        }
        ty
    }

    /// clang's `err_auto_not_allowed` for a parameter's placeholder at
    /// `loc`, with the position selector `site` that
    /// [`Sema::param_placeholder_site`] gave ([dcl.fct]/22,
    /// [dcl.spec.auto.general]/2-3). Returns `int`, the type clang
    /// recovers with.
    pub fn act_on_misplaced_param_placeholder(
        &mut self,
        loc: Loc,
        decltype_auto: bool,
        site: u8,
    ) -> QualType {
        use kiln_ast::types::AutoKeyword;
        let kw = if decltype_auto { AutoKeyword::DecltypeAuto } else { AutoKeyword::Auto };
        self.auto_not_allowed(loc, kw, site);
        QualType::INT
    }

    /// The undeduced placeholder of `t`, a parameter's trailing return
    /// type (under pointer and reference declarators and further function
    /// return types, `auto (*)(int) -> auto`, as
    /// [`Sema::replace_placeholder`] rewrites it): whether it is
    /// `decltype(auto)`, and its type-constraint with the extra arguments.
    pub fn trailing_return_placeholder(
        &self,
        t: QualType,
    ) -> Option<(bool, Option<TypeConstraint>)> {
        let leaf = self.contained_placeholder(t)?;
        let TypeView::Auto { keyword, concept, args, .. } = self.ast.types.canonical_view(leaf)
        else {
            return None;
        };
        let extra = args.map(|a| self.ast.types.args(a).to_vec()).unwrap_or_default();
        Some((keyword == kiln_ast::types::AutoKeyword::DecltypeAuto, concept.map(|c| (c, extra))))
    }

    /// `t` with its trailing-return placeholder replaced by the invented
    /// template parameter type `with` (clang invents one for an `auto` in
    /// a parameter's trailing return type, `void f(auto (*p)() -> auto)`).
    pub fn replace_trailing_return_placeholder(&mut self, t: QualType, with: QualType) -> QualType {
        self.replace_placeholder(t, with)
    }

    /// Before a function declarator's declaration is created: finishes
    /// the head an abbreviated function template's invented parameters
    /// were added to ([dcl.fct]/22). An explicit head gets a new list
    /// with the invented parameters appended.
    pub(crate) fn finish_invented_head(&mut self) {
        let Some(h) = self.templ.heads.last() else { return };
        if h.taken || self.templ.invented == 0 {
            return;
        }
        let h = h.clone();
        let requires = h.list.and_then(|l| self.ast.templates.param_list(l).requires);
        let r = h.list.map_or(h.l_angle, |l| self.ast.templates.param_list(l).r_angle);
        let l = self.builder().template_param_list(
            h.template_loc,
            h.l_angle,
            r,
            h.depth,
            &h.params,
            requires,
        );
        if let Some(x) = self.templ.heads.last_mut() {
            x.list = Some(l);
        }
        self.templ.invented = 0;
    }

    /// After a declaration with invented parameters only (no written
    /// head) is complete: drops its invented head.
    pub fn act_on_end_abbreviated(&mut self) {
        // Also the end of a declaration: uses of deleted functions that
        // no full-expression reported (spec/sema-class.md §4.3).
        if self.special.has_pending() {
            self.flush_pending_deleted();
        }
        // Only a head the declaration opened: in a member declaration of
        // a local class of a generic lambda, the invented head below the
        // class's is the lambda's, open until the lambda ends.
        let floor = self.classes.last().map_or(0, |c| c.heads);
        if self.templ.heads.len() > floor && self.templ.heads.last().is_some_and(|h| h.invented) {
            self.templ.heads.pop();
            self.ast.templates.dependent_depth =
                self.ast.templates.dependent_depth.saturating_sub(1);
        }
        self.templ.invented = 0;
    }

    /// Attaches the trailing requires-clause `r` to function `f`
    /// ([dcl.decl.general]/4); only a templated function may have one
    /// ([dcl.decl.general]/5), and a virtual function may not
    /// ([class.virtual]/6). Both errors point at the clause's first
    /// token; the clause is attached anyway and still checked, as in
    /// clang.
    pub fn act_on_trailing_requires(&mut self, f: DeclId, r: ExprId) {
        if self.kind_of(f).payload() != kiln_ast::decl::PayloadClass::Function {
            let loc = self.ast.exprs.get(r).loc();
            self.error(DiagId::sema_trailing_requires_non_function, loc);
            return;
        }
        // An instantiated member keeps its pattern's clause; an invalid
        // clause was diagnosed already (clang drops it in the parser).
        if !self.in_template() && self.inst.stack.is_empty() && !self.is_err_expr(r) {
            let loc = self.begin_loc(r);
            self.error(DiagId::sema_trailing_requires_non_template, loc);
            let _ = self.inst.concepts.untemplated_requires.insert(f);
        }
        // clang `CheckFunctionDeclaration` (`err_constrained_virtual_method`):
        // declared `virtual`, overriding a virtual function, or redeclaring
        // a virtual function; again in each instantiation.
        if crate::class::is_virtual(&self.ast, f) && !self.is_err_expr(r) {
            let loc = self.begin_loc(r);
            let d = self.diag(DiagId::sema_virtual_requires_clause, loc).per_instantiation();
            self.emit(d);
        }
        let _ = self.ast.templates.trailing_requires.insert(f, r);
    }

    /// The default argument recorded for template parameter `p`.
    pub fn template_default(&self, p: DeclId) -> Option<TemplateArg> {
        self.ast
            .templates
            .defaults
            .get(p)
            .copied()
            .or_else(|| self.inherited_defaults.get(&p).copied())
    }

    /// A declarator-id that is a template-id `tmpl<args>` naming the
    /// function or variable template `tmpl` (`f<T*>`, `v<int>`) after a
    /// template head or `template<>` ([temp.expl.spec],
    /// [temp.spec.partial]). A variable template's is returned for the
    /// declarator ([`Declarator::spec_id`]); a function template partial
    /// specialization is an error ([temp.spec.partial.general]/1 covers
    /// only class and variable templates; clang's
    /// `err_function_template_partial_spec`: `None`, and the declarator
    /// is dropped); an explicit one is returned for the declarator too
    /// (§6.9, `act_on_function_spec_declarator`). The argument
    /// locations are the ones the parser recorded; `r_angle` is the
    /// closing `>`.
    ///
    /// [`Declarator::spec_id`]: crate::declspec::Declarator::spec_id
    pub fn act_on_specialization_declarator_id(
        &mut self,
        tmpl: DeclId,
        args: &[TemplateArg],
        loc: Loc,
        r_angle: Loc,
    ) -> Option<spec::SpecDeclaratorId> {
        let arg_locs = std::mem::take(&mut self.templ.arg_locs);
        let explicit = !self.has_pending_head();
        match (self.kind_of(tmpl), explicit) {
            (DeclKind::FunctionTemplate, false) => {
                self.error(DiagId::sema_function_partial_spec, loc);
                None
            }
            (_, _) => Some(spec::SpecDeclaratorId {
                template: self.ast.first(tmpl),
                args: args.to_vec(),
                arg_locs,
                loc,
                range: kiln_source::Span::new(loc, r_angle.with_offset(1)),
            }),
        }
    }

    /// The function template `operator op` that redeclaration lookup in
    /// `qualifier` (else the current scope) finds, for the declarator-id
    /// `operator op<args>` of an explicit specialization.
    pub fn operator_function_template(
        &self,
        op: kiln_ast::names::OverloadedOperator,
        qualifier: Option<DeclId>,
    ) -> Option<DeclId> {
        self.prev_candidates_dn(DeclName::operator(op), qualifier)
            .into_iter()
            .find(|&c| self.kind_of(c) == DeclKind::FunctionTemplate)
    }

    /// True between `template<>` and the declaration it introduces.
    pub fn has_explicit_spec_head(&self) -> bool {
        self.templ.explicit_spec.is_some()
    }

    /// The parser is about to hand sema a function declarator followed
    /// by its body (`true`), or is done with it (`false`).
    pub fn act_function_def_next(&mut self, def: bool) {
        self.templ.fn_def_next = def;
    }

    /// Whether `[extern] template` heads the declaration being parsed.
    pub fn has_explicit_inst_head(&self) -> bool {
        self.templ.explicit_inst.is_some()
    }

    /// True while parsing between `template<...>` and the declaration it
    /// heads (the next declaration consumes it).
    pub fn has_pending_head(&self) -> bool {
        self.templ.heads.last().is_some_and(|h| !h.taken)
    }
}
