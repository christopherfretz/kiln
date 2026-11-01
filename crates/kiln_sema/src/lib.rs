//! Semantic analysis: lookup, types, templates, overloads, conversions, initialization, reflection.
//!
//! See `docs/ARCHITECTURE.md` and `spec/sema.md` for the contract
//! this crate implements.
//!
//! The parser (`kiln_parse`) drives a [`Sema`] through `act_on_*` entry
//! points, clang's Parser/Sema split: the parser owns the grammar and the
//! token stream, `Sema` owns scopes, lookup, declarations, types and the
//! typed AST it builds through [`kiln_ast::AstBuilder`]. Conversions live
//! in [`conv`] (`spec/sema-conv.md`) and literal typing in [`literal`].
#![forbid(unsafe_code)]
#![allow(
    clippy::many_single_char_names,
    reason = "sema code uses the standard's short names (e, t, d, l, r)"
)]

mod asm;
mod builtins;
pub mod class;
mod consteval;
pub mod constfold;
pub mod conv;
pub mod decl;
pub mod declspec;
mod deduce;
pub mod except;
pub mod expr;
mod forms;
mod gnu;
pub mod init;
mod jump_scope;
pub mod lambda;
pub mod literal;
mod literal_op;
pub mod lookup;
pub mod names;
pub mod new_delete;
mod oven_bridge;
pub mod overload;
mod predefined;
mod range_for;
mod rtti;
pub mod scope;
mod self_ref;
mod side_effects;
pub mod stack_guard;
pub mod stmt;
mod struct_bind;
mod sve;
pub mod target;
pub mod templ;
mod try_stmt;
mod type_diff;
pub mod using;
mod vector;
mod vector_ops;

use kiln_abi::Target;
use kiln_ast::types::{PrintOptions, TargetTypes, aka, print_type};
use kiln_ast::{Ast, AstBuilder, DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_source::{Loc, Span};

pub use class::{BaseSpecInfo, MemInitId, MemInitInfo};
pub use decl::{NameClass, TagUse};
pub use declspec::{
    BindingId, Chunk, DeclSpec, Declarator, DeclaratorContext, Decomposition, DefaultArg,
    ParamInfo, Sign, SpecialName, StorageSpec, TypeSpecKind, Width,
};
pub use gnu::{OffsetofComponent, StmtExprScope};
pub use init::{Designator, InitItem, InitList, InitValue, Initializer};
pub use kiln_oven::{Budget, EvalContext, EvalFailure, EvalKind, Oven};
pub use range_for::{RangeCollect, RangeForHead, RangeInit};
pub use scope::ScopeKind;

/// The semantic-analysis state of one translation unit (spec/sema.md §1).
///
/// `'a` is the borrow of the diagnostics engine and `'sm` the source
/// manager it reports against (two lifetimes: `DiagEngine` is invariant
/// in `'sm` and has drop glue, spec/sema.md §1).
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools, reason = "independent language switches set by the driver")]
pub struct Sema<'a, 'sm> {
    /// The AST being built.
    pub ast: Ast,
    /// Where diagnostics go.
    pub diags: &'a DiagEngine<'sm>,
    /// The target's builtin type properties (the conversion machinery's
    /// view of the target, spec/sema-conv.md).
    pub target: TargetTypes,
    /// The full target description (layout for `sizeof`/`alignof`,
    /// spec/abi.md).
    pub abi: Target,
    /// The `#pragma pack` directives seen, in order (location and the
    /// spelling of the argument list).
    pub pragma_packs: Vec<(Loc, String)>,
    /// C++ exceptions are enabled (`-f[no-]exceptions`; default on).
    pub exceptions: bool,
    /// Run-time type information is enabled (`-f[no-]rtti`; default on):
    /// `typeid` is an error otherwise.
    pub rtti: bool,
    /// The temporaries of a for-range-initializer are extended to the
    /// whole loop (C++23, P2718R0; spec/sema-class.md §5.4).
    pub range_lifetime_extension: bool,
    /// The `#pragma pack` state (clang's `AlignPackStack`): the current
    /// maximum field alignment in bytes (0: none) and the pushed
    /// `(label, value)` entries. Applied to each class definition as it
    /// begins (`record_pack`).
    pub(crate) pack: (u16, Vec<(Option<String>, u16)>),
    pub(crate) scopes: Vec<scope::Scope>,
    /// The current semantic declaration context.
    pub(crate) ctx: DeclId,
    pub(crate) func: Option<stmt::FnState>,
    /// Depth of unevaluated operands (`sizeof`, `alignof`).
    pub(crate) unevaluated: u32,
    /// The operands of the `decltype`s being parsed, innermost last: the
    /// unevaluated depth of each and the calls with an incomplete class
    /// prvalue result it deferred ([dcl.type.decltype]/2).
    pub(crate) decltype_calls: Vec<(u32, Vec<deduce::DeferredCall>)>,
    /// Depth of immediate function contexts ([expr.const]/16) entered
    /// by `if consteval`.
    pub(crate) immediate: u32,
    /// `DeclRefExpr`s of the current full-expression that are potential
    /// odr-uses ([basic.def.odr]/4): resolved at the end of the
    /// full-expression (spec/sema.md §5.2).
    pub(crate) pending_odr: Vec<ExprId>,
    /// Immediate invocations and references to immediate functions of
    /// the current full-expression, and why escalated functions are
    /// immediate (spec/sema.md §5.10).
    pub(crate) imm: consteval::ImmState,
    /// Classes whose definitions are being processed (incomplete until
    /// their closing brace).
    pub(crate) defining: Vec<DeclId>,
    /// Enumerations whose definitions are being processed.
    pub(crate) enums: Vec<decl::EnumBuild>,
    /// Block-scope function and `extern` variable declarations, by
    /// enclosing namespace and name: a later namespace-scope declaration
    /// redeclares them ([basic.link]/6, clang's local extern decls).
    pub(crate) local_externs: Vec<(DeclId, kiln_source::symbol::Symbol, DeclId)>,
    /// The constant evaluator of the translation unit (spec/sema.md §8).
    /// Taken out for the duration of an evaluation, so the bridge can
    /// reach the rest of `Sema` (`ensure_body` instantiates through it,
    /// spec/sema-templates.md §4.11); `None` only while one runs.
    pub(crate) oven: Option<Oven>,
    /// The implicitly declared global allocation functions.
    pub(crate) global_ops: new_delete::GlobalOps,
    /// Braced-init-list call arguments awaiting their parameter types
    /// (placeholders of [`Sema::act_on_braced_arg`]).
    pub(crate) braced_args: Vec<(ExprId, InitList)>,
    /// The parameters declared so far in each enclosing
    /// parameter-declaration-clause being parsed
    /// ([`Sema::act_on_start_prototype`]).
    pub(crate) prototype_params: Vec<decl::PrototypeFrame>,
    /// The contexts of the qualified declarator-ids being parsed
    /// ([`Sema::act_on_enter_declarator_scope`]), the scope depth each
    /// one's scopes end at, and the context to restore.
    pub(crate) declarator_records: Vec<(DeclId, usize, DeclId)>,
    /// Parameters created by [`Sema::act_on_prototype_param`] that no
    /// function has adopted yet ([`Sema::create_params`]).
    pub(crate) unowned_params: std::collections::HashSet<DeclId>,
    /// The declared type (`cv auto`, `auto &`, `auto &&`) of each
    /// decomposition declared in a template, which instantiation
    /// deduces anew (the pattern's own may be deduced, spec/sema.md).
    pub(crate) decomp_written: std::collections::HashMap<DeclId, QualType>,
    /// The declared (placeholder) return type of each function whose
    /// return type was deduced, by first declaration: redeclarations are
    /// matched against it ([dcl.spec.auto.general]/13).
    pub(crate) declared_returns: kiln_source::fx::FxHashMap<DeclId, QualType>,
    /// The type-specifier location of each handler's exception
    /// declaration (clang `getTypeSpecStartLoc`), where the handler-order
    /// warnings point ([except.handle]/4, WP-100).
    pub(crate) handler_type_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// Where `constinit` was written on each variable declaration that
    /// has it ([dcl.constinit]; WP-159).
    pub(crate) constinit_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// The declared placeholder type and decl-specifiers' type location
    /// of each variable of a multi-declarator group in a template
    /// pattern: instantiation checks the group's deductions again
    /// ([dcl.spec.auto.general]/8).
    pub(crate) group_placeholders: kiln_source::fx::FxHashMap<DeclId, (QualType, Loc)>,
    /// Set while [`Sema::act_on_call_name`] builds an unqualified callee:
    /// a function it names is not marked referenced until the call is
    /// known to be non-dependent ([temp.dep.candidate]), and ADL applies
    /// (spec/sema-overload.md §2.2).
    pub(crate) deferring_call_ref: bool,
    /// Replaying a non-dependent class-scope using-declaration of a
    /// pattern: clang walks the pattern's shadow list, which is newest
    /// first, so the instantiation's shadows come in reverse (WP-159).
    pub(crate) reverse_using_shadows: bool,
    /// The type `__builtin_va_list` names (its typedef sugar; the `A` of
    /// a builtin signature), once declared.
    pub(crate) va_list_type: Option<QualType>,
    /// Overload resolution's pooled candidate sets and counters
    /// (spec/sema-overload.md §1.4).
    pub(crate) ovl: overload::OverloadScratch,
    /// Notes the next error takes (the user-declared candidates of an
    /// operator whose built-in fallback fails, spec/sema-overload.md
    /// §6.2): set and cleared around the fallback.
    pub(crate) trailing_notes: std::cell::RefCell<Vec<Diag>>,
    /// Callees whose function is not marked referenced yet (see
    /// `deferring_call_ref`), resolved by [`Sema::act_on_call`].
    pub(crate) deferred_call_refs: Vec<(ExprId, DeclId)>,
    /// Classes being defined, innermost last (spec/sema-class.md §8.3).
    pub(crate) classes: Vec<class::ClassBuild>,
    /// Which implicit special members each class has declared
    /// (`SpecialKind::bit`s; spec/sema-class.md §4.1).
    pub(crate) implicit_declared: kiln_source::fx::FxHashMap<DeclId, u8>,
    /// Special-member state (spec/sema-class.md §8.3).
    pub(crate) special: class::SpecialState,
    /// Access checks delayed until their declaration exists
    /// (spec/sema-class.md §2.4).
    pub(crate) delayed_access: Vec<class::AccessCheck>,
    /// `delayed_access.len()` at each open delay point, innermost last.
    pub(crate) delayed_access_depth: Vec<usize>,
    /// Access checks in a class template pattern (or partial
    /// specialization) that passed only because the class may instantiate
    /// to a befriended specialization, keyed by the pattern's first
    /// declaration; re-run in each instantiation (clang's dependent access
    /// diagnostics; spec/sema-templates.md §4.8).
    pub(crate) dependent_access: kiln_source::fx::FxHashMap<DeclId, Vec<class::AccessCheck>>,
    /// Set by the access check when friendship came only from such a
    /// befriended specialization.
    pub(crate) speculative_friend: std::cell::Cell<bool>,
    /// Access errors reported, as (member, location, innermost context),
    /// so a replayed check does not repeat one its instantiation redid.
    pub(crate) access_failed: kiln_source::fx::FxHashSet<(DeclId, Loc, DeclId)>,
    /// True while [`Sema::replay_dependent_access`] runs.
    pub(crate) replaying_access: bool,
    /// The current full-expression's cleanup state (spec/sema-class.md
    /// §5.3).
    pub(crate) cleanup: class::CleanupState,
    /// The cleanup states of the enclosing full-expressions around
    /// unevaluated operands, innermost last.
    pub(crate) cleanup_saved: Vec<class::CleanupState>,
    /// Implicit special members being defined (spec/sema-class.md §4.5),
    /// innermost last: diagnostics emitted meanwhile get the note `in
    /// implicit ... for 'T' first required here`.
    pub(crate) synth: Vec<class::SynthContext>,
    /// Classes whose vtables are used ([basic.def.odr]/8; clang
    /// `VTableUses`), with the use: their virtual members are marked
    /// referenced at the end of the TU (spec/sema-class.md §3.5).
    pub(crate) vtable_uses: Vec<(DeclId, Loc)>,
    /// The classes ever pushed on `vtable_uses` (first declarations).
    pub(crate) vtables_used: kiln_source::fx::FxHashSet<DeclId>,
    /// The depth of `synth` whose notes were last printed (clang
    /// `LastEmittedCodeSynthesisContextDepth`).
    pub(crate) synth_noted: std::cell::Cell<usize>,
    /// The internal `__builtin_memcpy` declaration (spec/sema-class.md
    /// §4.5), once declared.
    pub(crate) memcpy_decl: Option<DeclId>,
    /// Delegating constructors and their targets, checked for cycles at
    /// the end of the translation unit ([class.base.init]/6).
    pub(crate) delegating: Vec<(DeclId, DeclId, Loc)>,
    /// The subobject the next constructor call initializes, set by a
    /// mem-initializer and consumed by the outermost constructor call it
    /// builds (clang's `InitializedEntity` for bases, members and
    /// delegation; spec/sema-class.md §4.9).
    pub(crate) ctor_entity: Option<(class::InitEntity, kiln_ast::expr::ConstructionKind)>,
    /// The type a failed constructor call names instead of the class:
    /// the reference a list-initialized temporary binds to (`'const E &'`,
    /// clang's entity type). Set around that construction only.
    pub(crate) ctor_diag_ty: Option<QualType>,
    /// The mem-initializers of the constructor definition being parsed
    /// (spec/sema-class.md §4.9).
    pub(crate) mem_inits: class::MemInitState,
    /// Materialized temporaries collected for P2718 range-for lifetime
    /// extension (spec/sema-class.md §5.4).
    pub(crate) extend_collector: Option<Vec<ExprId>>,
    /// Diagnostics held back while a range-based `for` step runs, so
    /// clang's trailing notes (`when looking up 'begin' function...`)
    /// can be attached to the last of them (`range_for.rs`).
    pub(crate) diag_capture: std::cell::RefCell<Option<Vec<Diag>>>,
    /// The scope depth at the point of instantiation (clang numbers the
    /// implicit `__begin`/`__end` variables of an instantiated dependent
    /// range-based `for` from `getCurScope()`); set by the outermost
    /// `in_inst_context`.
    pub(crate) range_for_outer_depth: Option<usize>,
    /// The C++26 rule for deleting a pointer to an incomplete type
    /// (P3144), as the parser last passed it to
    /// [`Sema::act_on_cxx_delete`]; reused when a dependent
    /// delete-expression is instantiated.
    pub(crate) delete_cxx26: bool,
    /// Member functions whose *noexcept-specifier* awaits replay
    /// (`ExcSpec::Deferred`, spec/sema-class.md §2.7).
    pub(crate) unparsed_exc: kiln_source::fx::FxHashSet<DeclId>,
    /// Members of class template specializations whose dependent
    /// *noexcept-specifier* is instantiated when needed ([temp.inst]/14,
    /// clang's `EST_Uninstantiated`): the first declaration maps to its
    /// pattern; its type carries `ExcSpec::Deferred(pattern)`.
    pub(crate) uninstantiated_exc: kiln_source::fx::FxHashMap<DeclId, DeclId>,
    /// The enclosing function's state and the scopes pushed for a member
    /// function definition, saved by `act_on_start_function_def` while a
    /// (local class's or out-of-line) member function body is analysed.
    pub(crate) fn_saved: Vec<class::SavedFn>,
    /// Names of non-static members whose meaning depends on the operand
    /// they appear in (spec/sema-class.md §2.5/§2.6): `&C::m` forms a
    /// pointer to member, anything else needs an object.
    pub(crate) member_refs: kiln_source::fx::FxHashMap<ExprId, class::MemberRef>,
    /// The class the last `friend class X;` declared ([`TagUse::Friend`]),
    /// taken by [`Sema::act_on_friend`] as the `FriendDecl`'s owned tag.
    pub(crate) friend_tag: Option<DeclId>,
    /// The class template the last `template<..> friend class X;`
    /// befriended ([temp.friend]/1), taken by [`Sema::act_on_friend`].
    pub(crate) friend_class_template: Option<DeclId>,
    /// Default template arguments a template parameter inherits from
    /// another declaration of its template ([temp.param]/12), keyed by
    /// the parameter that has none of its own.
    /// The friend function template specializations of class template
    /// patterns (`friend bool operator==<>(const X&, const X&);`,
    /// [temp.friend]/1.3), keyed by their `FriendDecl`: matched against
    /// their templates per specialization.
    pub(crate) friend_specs: kiln_source::fx::FxHashMap<DeclId, class::friend::FriendSpec>,
    /// The member aliases standing for a dependent `using typename
    /// B<T>::name;` ([`Sema::act_on_using_typename_declarator`]): an
    /// instantiation redoes the using-declarator.
    pub(crate) using_typename: kiln_source::fx::FxHashSet<DeclId>,
    pub(crate) inherited_defaults: kiln_source::fx::FxHashMap<DeclId, kiln_ast::types::TemplateArg>,
    /// The inheriting constructors declared, by (derived class, base
    /// constructor) (WP-114, clang `findInheritingConstructor`'s lookup).
    pub(crate) inh_ctors: kiln_source::fx::FxHashMap<(DeclId, DeclId), DeclId>,
    /// The base constructor of each inheriting constructor.
    pub(crate) inh_ctor_base: kiln_source::fx::FxHashMap<DeclId, DeclId>,
    /// The `this` type of the default member initializers being parsed,
    /// innermost last, with their fields (`C*`, spec/sema-class.md §2.5).
    pub(crate) nsdmi_this: Vec<(DeclId, QualType)>,
    /// The static data member whose out-of-line initializer is being
    /// analysed: its class is the access context ([class.access]/7).
    pub(crate) initializing: Option<DeclId>,
    /// Template heads and parse-time template state (WP-069,
    /// spec/sema-templates.md §11.2).
    pub(crate) templ: templ::TemplState,
    /// The instantiation engine (spec/sema-templates.md §11.2; WP-070a).
    pub inst: templ::InstState,
    /// The abstract classes whose unimplemented pure virtual functions
    /// were already noted (clang notes them on the first abstract-type
    /// diagnostic per class only; WP-078a).
    pub(crate) abstract_noted: kiln_source::fx::FxHashSet<DeclId>,
    /// State of the source-location builtins (`builtins.rs`).
    pub(crate) srcloc: builtins::SrcLocCache,
    /// The lambdas whose bodies are being analysed, innermost last
    /// (spec/sema-lambda.md; WP-075).
    pub(crate) lambdas: Vec<lambda::LambdaScope>,
    /// The mangling-number state of closure types (spec/sema-lambda.md).
    pub(crate) lambda_mangle: lambda::ManglingState,
}

impl<'a, 'sm> Sema<'a, 'sm> {
    /// A `Sema` over `ast` reporting to `diags`, for the target whose
    /// builtin types are `target` (the layout target is picked from its
    /// `long double` format, [`target::target_for_types`]).
    pub fn new(ast: Ast, diags: &'a DiagEngine<'sm>, target: TargetTypes) -> Self {
        let abi = target::target_for_types(&target);
        Self::with_abi(ast, diags, abi, target)
    }

    /// A `Sema` over `ast` reporting to `diags` for `abi`.
    pub fn for_target(ast: Ast, diags: &'a DiagEngine<'sm>, abi: Target) -> Self {
        let types = abi.types.clone();
        Self::with_abi(ast, diags, abi, types)
    }

    fn with_abi(
        mut ast: Ast,
        diags: &'a DiagEngine<'sm>,
        abi: Target,
        target: TargetTypes,
    ) -> Self {
        let tu = ast.tu;
        templ::builtins::declare_builtin_templates(&mut ast);
        builtins::declare_int128_typedefs(&mut ast, &target);
        let mut s = Sema {
            ast,
            diags,
            target,
            abi,
            pragma_packs: Vec::new(),
            exceptions: true,
            rtti: true,
            range_lifetime_extension: false,
            pack: (0, Vec::new()),
            scopes: vec![scope::Scope {
                first_decl: 0,
                ..scope::Scope::new(ScopeKind::TranslationUnit, Some(tu))
            }],
            ctx: tu,
            func: None,
            unevaluated: 0,
            decltype_calls: Vec::new(),
            immediate: 0,
            pending_odr: Vec::new(),
            imm: consteval::ImmState::default(),
            defining: Vec::new(),
            enums: Vec::new(),
            local_externs: Vec::new(),
            oven: Some(Oven::new(Budget::default())),
            global_ops: new_delete::GlobalOps::default(),
            braced_args: Vec::new(),
            prototype_params: Vec::new(),
            declarator_records: Vec::new(),
            unowned_params: std::collections::HashSet::new(),
            decomp_written: std::collections::HashMap::new(),
            declared_returns: kiln_source::fx::FxHashMap::default(),
            handler_type_locs: kiln_source::fx::FxHashMap::default(),
            constinit_locs: kiln_source::fx::FxHashMap::default(),
            group_placeholders: kiln_source::fx::FxHashMap::default(),
            deferring_call_ref: false,
            reverse_using_shadows: false,
            va_list_type: None,
            ovl: overload::OverloadScratch::default(),
            trailing_notes: std::cell::RefCell::new(Vec::new()),
            deferred_call_refs: Vec::new(),
            classes: Vec::new(),
            implicit_declared: kiln_source::fx::FxHashMap::default(),
            special: class::SpecialState::default(),
            delayed_access: Vec::new(),
            delayed_access_depth: Vec::new(),
            dependent_access: kiln_source::fx::FxHashMap::default(),
            speculative_friend: std::cell::Cell::new(false),
            access_failed: kiln_source::fx::FxHashSet::default(),
            replaying_access: false,
            cleanup: class::CleanupState::default(),
            cleanup_saved: Vec::new(),
            synth: Vec::new(),
            vtable_uses: Vec::new(),
            vtables_used: kiln_source::fx::FxHashSet::default(),
            synth_noted: std::cell::Cell::new(0),
            memcpy_decl: None,
            delegating: Vec::new(),
            ctor_entity: None,
            ctor_diag_ty: None,
            mem_inits: class::MemInitState::default(),
            extend_collector: None,
            diag_capture: std::cell::RefCell::new(None),
            range_for_outer_depth: None,
            delete_cxx26: true,
            unparsed_exc: kiln_source::fx::FxHashSet::default(),
            uninstantiated_exc: kiln_source::fx::FxHashMap::default(),
            fn_saved: Vec::new(),
            member_refs: kiln_source::fx::FxHashMap::default(),
            friend_tag: None,
            friend_class_template: None,
            inherited_defaults: kiln_source::fx::FxHashMap::default(),
            inh_ctors: kiln_source::fx::FxHashMap::default(),
            inh_ctor_base: kiln_source::fx::FxHashMap::default(),
            friend_specs: kiln_source::fx::FxHashMap::default(),
            using_typename: kiln_source::fx::FxHashSet::default(),
            nsdmi_this: Vec::new(),
            initializing: None,
            templ: templ::TemplState::default(),
            inst: templ::InstState::default(),
            abstract_noted: kiln_source::fx::FxHashSet::default(),
            srcloc: builtins::SrcLocCache::default(),
            lambdas: Vec::new(),
            lambda_mangle: lambda::ManglingState::default(),
        };
        s.declare_builtin_va_list();
        s.declare_sve_typedefs();
        s
    }

    /// The AST, mutably.
    pub fn ast_mut(&mut self) -> &mut Ast {
        &mut self.ast
    }

    /// A builder over the AST.
    pub fn builder(&mut self) -> AstBuilder<'_> {
        self.ast.builder()
    }

    /// The target's builtin type properties.
    pub fn target(&self) -> &TargetTypes {
        &self.target
    }

    /// The full target description.
    pub fn abi(&self) -> &Target {
        &self.abi
    }

    /// The current semantic declaration context.
    pub fn context(&self) -> DeclId {
        self.ctx
    }

    /// Consumes the `Sema`, returning the AST.
    pub fn into_ast(self) -> Ast {
        self.ast
    }

    /// True if any error has been reported.
    pub fn has_errors(&self) -> bool {
        self.diags.error_count() > 0
    }

    // -- diagnostics helpers ---------------------------------------------

    /// A diagnostic `id` with its caret at `loc`.
    pub fn diag(&self, id: DiagId, loc: Loc) -> Diag {
        Diag::new(id, Span::empty(loc))
    }

    /// Emits `d`. Inside an instantiation (spec/sema-templates.md §10.2)
    /// the context notes are added to a diagnostic that shows, and a
    /// SFINAE context captures errors instead (§5.7). Inside an implicit
    /// special member definition the `in implicit ... first required
    /// here` note (spec/sema-class.md §4.5) goes first, as the innermost
    /// code-synthesis context.
    pub fn emit(&self, mut d: Diag) {
        if let Some(held) = self.diag_capture.borrow_mut().as_mut() {
            held.push(d);
            return;
        }
        if self.inst.suppress_next_error.replace(false) {
            return;
        }
        if !self.trailing_notes.borrow().is_empty()
            && d.id().is_some_and(|id| self.diags.severity(id) >= kiln_diag::Severity::Error)
        {
            for n in self.trailing_notes.take() {
                d = d.with_note(n);
            }
        }
        if !self.inst.stack.is_empty()
            && let Some(id) = d.id()
        {
            // Dedup is per instantiation (spec/diag.md §7.4).
            d = d.context(self.inst.stack.context_key());
            let sev = self.diags.severity(id);
            if self.inst.stack.is_sfinae() {
                if sev >= kiln_diag::Severity::Error
                    && let Some(s) = self.inst.sfinae.borrow_mut().last_mut()
                {
                    s.errors += 1;
                    if s.first.is_none() {
                        s.first = Some(d);
                    }
                }
                return;
            }
            // Only a diagnostic that will print takes the context notes,
            // which print once per depth (§10.2).
            if self.diags.would_emit(&d) {
                d.prepend_notes(self.inst_context_notes());
            }
        }
        let d = self.with_synth_note(d);
        d.emit(self.diags);
    }

    /// Emits a diagnostic with no arguments at `loc`.
    pub fn error(&self, id: DiagId, loc: Loc) {
        self.emit(self.diag(id, loc));
    }

    /// `q` as clang prints a type argument: `'T'`, or `'T' (aka 'int')`
    /// when the canonical spelling differs.
    /// A vector type adds ` (vector of N 'E' values)` (clang's
    /// `ConvertTypeToDiagnosticString`).
    /// clang's `desugarForDiagnostic`
    /// does not desugar `__builtin_va_list`, so a difference only there
    /// gives no `aka`.
    pub fn ty_arg(&self, q: QualType) -> String {
        let names = names::SemaNames::new(self);
        let s = print_type(&self.ast.types, &names, q, PrintOptions::SUGARED);
        if let kiln_ast::types::TypeView::Vector { elem, count, .. } =
            self.ast.types.canonical_view(q)
        {
            // clang's `Desugar` stops before a vector type ("people want
            // their vec4"), and prints the vector details only without
            // an `aka`.
            if let Some(d) = self.vector_aka(q) {
                let a = print_type(&self.ast.types, &names, d, PrintOptions::SUGARED);
                if a != s {
                    return format!("'{s}' (aka '{a}')");
                }
            }
            let top = self.ast.types.desugar_top(q);
            let elem = match self.ast.types.view(top.ty()) {
                kiln_ast::types::TypeView::Vector { elem, .. } => elem,
                _ => elem,
            };
            let e = print_type(&self.ast.types, &names, elem, PrintOptions::SUGARED);
            let v = if count == 1 { "value" } else { "values" };
            return format!("'{s}' (vector of {count} '{e}' {v})");
        }
        let a = aka(&self.ast.types, &names, q).filter(|a| {
            self.va_list_type.is_none_or(|v| {
                let vc = print_type(&self.ast.types, &names, v, PrintOptions::CANONICAL);
                s.replace("__builtin_va_list", &vc) != *a
            })
        });
        match a {
            Some(a) => format!("'{s}' (aka '{a}')"),
            None => format!("'{s}'"),
        }
    }

    /// clang's `Desugar` for a type whose canonical type is a vector: the
    /// type desugared step by step, stopping at the sugar whose
    /// underlying type is the vector itself; `None` when no step counts
    /// for an `aka` (template parameter substitution, `using` and
    /// elaborated names, attributes and deduced types do not).
    fn vector_aka(&self, q: QualType) -> Option<QualType> {
        use kiln_ast::types::TypeView as V;
        let t = &self.ast.types;
        let mut cur = q;
        let mut aka = false;
        for _ in 0..256 {
            let next = t.desugar_once(cur);
            if next == cur {
                break;
            }
            match t.view(cur.ty()) {
                // clang substitutes canonical template arguments.
                V::SubstTemplateTypeParm { .. } => {
                    cur = t.canonical(cur);
                    continue;
                }
                V::Elaborated { .. } | V::Using { .. } | V::Attributed { .. } | V::Auto { .. } => {}
                _ => {
                    if matches!(t.view(next.ty()), V::Vector { .. }) {
                        break;
                    }
                    aka = true;
                }
            }
            cur = next;
        }
        aka.then_some(cur)
    }

    /// `q` printed canonically (fully qualified) without quotes.
    pub fn ty_canonical_str(&self, q: QualType) -> String {
        let names = names::SemaNames::new(self);
        print_type(&self.ast.types, &names, q, PrintOptions::CANONICAL)
    }

    /// `q` printed without quotes.
    pub fn ty_str(&self, q: QualType) -> String {
        let names = names::SemaNames::new(self);
        print_type(&self.ast.types, &names, q, PrintOptions::SUGARED)
    }

    /// `q` printed canonical and fully scoped, without quotes: how clang
    /// prints a declaration's own type (`Context.getTypeDeclType(D)`, a
    /// bare `RecordType`), e.g. `N::X<int>`.
    pub fn decl_ty_str(&self, q: QualType) -> String {
        let names = names::SemaNames::new(self);
        print_type(&self.ast.types, &names, q, PrintOptions::CANONICAL)
    }

    /// The qualified name of `d`, as clang prints a declaration argument.
    pub fn qualified_decl_name(&self, d: DeclId) -> String {
        use kiln_ast::types::TypeNames as _;
        let mut s = String::new();
        names::SemaNames::new(self).for_decls().write_decl_name(d, true, &mut s);
        s
    }

    /// The unqualified name of `d` with its template arguments, as clang
    /// prints a `NamedDecl` argument without the `q` modifier.
    pub fn unqualified_decl_name(&self, d: DeclId) -> String {
        use kiln_ast::types::TypeNames as _;
        let mut s = String::new();
        names::SemaNames::new(self).write_decl_name(d, false, &mut s);
        s
    }

    /// A nested-name-specifier as written (`N::M::`, `::`).
    pub fn nns_str(&self, n: kiln_ast::ids::NnsId) -> String {
        let mut s = String::new();
        kiln_ast::types::write_nns(&self.ast.types, &names::SemaNames::new(self), n, &mut s);
        s
    }

    /// The simple name of `d` (empty when unnamed).
    pub fn decl_name(&self, d: DeclId) -> String {
        if self.ast.decls.decomp_bindings(d).is_some() {
            return self.decomp_name(d);
        }
        self.decl_name_str(self.ast.decls.get(d).name())
    }

    /// `name` as clang prints it in a diagnostic: identifiers, `operator+`,
    /// the class name for constructors, `~X`, `operator int`.
    pub fn decl_name_str(&self, name: kiln_ast::names::DeclName) -> String {
        self.ast.names.simple_spelling(name, self.ast.symbols()).unwrap_or_else(|| {
            let mut s = String::new();
            kiln_ast::dump::AstNames::new(&self.ast).write_decl_name_value(name, &mut s);
            s
        })
    }
}
