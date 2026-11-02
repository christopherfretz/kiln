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

mod absolute_value;
mod align_redecl;
mod asm;
mod bitfield_constant;
mod builtins;
pub mod class;
mod complex;
mod consteval;
pub mod constfold;
pub mod conv;
mod coroutine;
pub mod decl;
mod decl_attrs;
pub mod declspec;
mod deduce;
mod deprecated;
mod empty_body;
pub mod except;
pub mod expr;
mod extern_c;
mod fallthrough;
mod flex_array;
mod for_loop_analysis;
mod format_string;
mod forms;
mod gnu;
mod gnu_null;
mod gnu_typeof;
mod impcast;
mod implicit_fallthrough;
pub mod init;
mod init_dtor;
mod jump_scope;
pub mod lambda;
pub mod literal;
mod literal_op;
mod local_friend;
pub mod lookup;
pub mod modules;
pub mod names;
pub mod new_delete;
mod null_deref;
mod oven_bridge;
pub mod overload;
mod packed;
mod predefined;
mod qualified_decl;
mod range_for;
mod reflect;
mod rtti;
pub mod scope;
mod self_ref;
mod side_effects;
pub mod stack_guard;
pub mod stmt;
mod struct_bind;
mod sve;
mod switch_conv;
mod switch_enum;
pub mod target;
pub mod templ;
mod try_stmt;
mod type_diff;
mod unreachable_code;
mod unused_file_scope;
mod unused_var;
pub mod using;
mod vector;
mod vector_ops;
mod vexing;

use kiln_abi::Target;
use kiln_ast::types::{PrintOptions, TargetTypes, aka, print_type};
use kiln_ast::{Ast, AstBuilder, DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_source::{Loc, Span};

pub use class::{BaseSpecInfo, MemInitId, MemInitInfo};
pub use decl::{NameClass, TagUse};
pub use declspec::{
    BindingId, Chunk, DeclSite, DeclSpec, Declarator, DeclaratorContext, Decomposition, DefaultArg,
    ParamInfo, Sign, SpecialName, StorageSpec, TypeSpecKind, Width,
};
pub use empty_body::EmptyBodyStmt;
pub use gnu::{OffsetofComponent, StmtExprScope};
pub use implicit_fallthrough::AttrMacro;
pub use init::{Designator, InitItem, InitList, InitValue, Initializer};
pub use kiln_oven::{Budget, EvalContext, EvalFailure, EvalKind, Oven};
pub use range_for::{RangeCollect, RangeForHead, RangeInit};
pub use scope::ScopeKind;

/// A delayed-typo diagnostic held for the end of its full-expression
/// ([`Sema::begin_delayed_typos`]): its sequence number and the recovery
/// expression that stands for the `TypoExpr`.
#[derive(Debug)]
struct HeldTypo {
    seq: u32,
    expr: ExprId,
    diag: Diag,
}

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
    /// `-freflection` (P2996 spike, WP-267; default off): `^^`, type
    /// splices and the kiln reflection intrinsics (spec/sema.md
    /// "Reflection (spike, -freflection)").
    pub reflection: bool,
    /// `-fcoro-aligned-allocation` (default off): a coroutine frame is
    /// allocated and freed with the `std::align_val_t` forms of
    /// `operator new`/`operator delete` when available (P2014R0 option 2,
    /// as clang; spec/sema.md "Coroutines").
    pub coro_aligned_allocation: bool,
    /// The temporaries of a for-range-initializer are extended to the
    /// whole loop (C++23, P2718R0; spec/sema-class.md §5.4).
    pub range_lifetime_extension: bool,
    /// A GNU dialect (`-std=gnu++NN`): some extension warnings are
    /// off by default (clang's `*_in_gnu_mode` diagnostics, WP-183).
    pub gnu: bool,
    /// The clang major `-fclang-compat` names (19 or 22; default 19): the
    /// few spellings that differ between the two releases follow it, e.g.
    /// a closure type in `__PRETTY_FUNCTION__` (spec/sema.md
    /// "Predefined identifiers").
    pub clang_compat: u32,
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
    /// The `}` of the innermost braced aggregate list being checked: a
    /// brace-elided sublist's element destructor checks are located
    /// there (`init_dtor.rs`).
    pub(crate) agg_list_end: Option<Loc>,
    /// Depth of immediate function contexts ([expr.const]/16) entered
    /// by `if consteval`.
    pub(crate) immediate: u32,
    /// Depth of [`Sema::act_on_constant_full_expr`]: the full-expression
    /// ending is a manifestly constant-evaluated constant-expression (no
    /// runtime-behavior warnings, clang's `ConstantEvaluated` context).
    pub(crate) constant_full_expr: u32,
    /// The language standard (set by the parser; spellings in notes).
    pub(crate) std: kiln_lex::Std,
    /// `DeclRefExpr`s of the current full-expression that are potential
    /// odr-uses ([basic.def.odr]/4): resolved at the end of the
    /// full-expression (spec/sema.md §5.2).
    pub(crate) pending_odr: Vec<ExprId>,
    /// `&m` of members a `packed` attribute may misalign, reported at the
    /// end of the full-expression (`-Waddress-of-packed-member`).
    pub(crate) misaligned: Vec<packed::Misaligned>,
    /// Immediate invocations and references to immediate functions of
    /// the current full-expression, and why escalated functions are
    /// immediate (spec/sema.md §5.10).
    pub(crate) imm: consteval::ImmState,
    /// Classes whose definitions are being processed (incomplete until
    /// their closing brace).
    pub(crate) defining: Vec<DeclId>,
    /// Of those, the ones made invalid by a member of incomplete type
    /// (clang's `Record->setInvalidDecl()` in `CheckFieldDecl`): a later
    /// use that needs them complete fails quietly (spec/sema.md §4.3).
    pub(crate) invalid_defining: kiln_source::fx::FxHashSet<DeclId>,
    /// Enumerations whose definitions are being processed.
    pub(crate) enums: Vec<decl::EnumBuild>,
    /// Block-scope function and `extern` variable declarations, by
    /// enclosing namespace and name: a later namespace-scope declaration
    /// redeclares them ([basic.link]/6, clang's local extern decls).
    pub(crate) local_externs: Vec<(DeclId, kiln_source::symbol::Symbol, DeclId)>,
    /// Block-scope variables whose qualified declarator-id was rejected
    /// (`static T(::a);`, [`qualified_decl::QualifiedDecl::DropLocal`]):
    /// declared invalid in the block, standing for a namespace member, so
    /// a later block-scope redeclaration sees a previous declaration with
    /// linkage (clang pushes the invalid out-of-line declaration on the
    /// block's scope chain; WP-235).
    pub(crate) dropped_qualified_locals: kiln_source::fx::FxHashSet<DeclId>,
    /// Functions declared with a parameter that needed the missing-
    /// `typename` recovery: no qualified redeclaration matches them
    /// (spec/sema-templates.md, clang's invalid declaration).
    pub(crate) missing_typename_fns: kiln_source::fx::FxHashSet<DeclId>,
    /// The latest namespace-scope declaration of each function or
    /// variable with C language linkage, by name ([dcl.link]/6; clang's
    /// extern "C" context, `extern_c.rs`).
    pub(crate) extern_c: kiln_source::fx::FxHashMap<kiln_source::symbol::Symbol, DeclId>,
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
    /// The requires-expression parameter-declaration-clauses being parsed
    /// ([`Sema::act_on_start_requires_params`]), prototype scopes for
    /// `ParamData::scope_depth` alongside `prototype_params`.
    pub(crate) requires_param_scopes: usize,
    /// The declared type (`cv auto`, `auto &`, `auto &&`) of each
    /// decomposition declared in a template, which instantiation
    /// deduces anew (the pattern's own may be deduced, spec/sema.md).
    pub(crate) decomp_written: std::collections::HashMap<DeclId, QualType>,
    /// The structured binding packs `...xs` declared so far (P1061,
    /// spec/sema.md §4.9).
    pub(crate) binding_packs: std::collections::HashSet<DeclId>,
    /// A binding pack decomposed once its element count is known: the
    /// bindings it expanded into, which replace it in its decomposition's
    /// binding list (instantiation maps the pattern pack to them).
    pub(crate) binding_pack_elems: std::collections::HashMap<DeclId, kiln_ast::ListId>,
    /// clang `RefsMinusAssignments`: per local variable, its references
    /// minus those that are the target of a discarded assignment, and
    /// whether it is a condition variable (clang `isCXXCondDecl`)
    /// (`-Wunused-but-set-variable`, [`Self::diagnose_unused_locals`]).
    pub(crate) refs_minus_assign: std::collections::HashMap<DeclId, (i32, bool)>,
    /// The preprocessor's attribute-macro history and its generation, as
    /// last copied by [`Self::sync_attr_macros`] (the fix-it spelling of
    /// `-Wimplicit-fallthrough`).
    pub(crate) attr_macros: (u32, Vec<AttrMacro>),
    /// The declared (placeholder) return type of each function whose
    /// return type was deduced, by first declaration: redeclarations are
    /// matched against it ([dcl.spec.auto.general]/13).
    pub(crate) declared_returns: kiln_source::fx::FxHashMap<DeclId, QualType>,
    /// The type-specifier location of each handler's exception
    /// declaration (clang `getTypeSpecStartLoc`), where the handler-order
    /// warnings point ([except.handle]/4, WP-100).
    pub(crate) handler_type_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// Where the first type-specifier was written for a declaration of
    /// dependent type, when not at its name: a substituted function
    /// return type (a member function's, or a function type in a
    /// variable's, data member's, parameter's or typedef's declarator)
    /// that is a function or array is diagnosed there (clang's
    /// `FunctionTypeLoc` begin; [dcl.fct]/11, WP-222, WP-241).
    pub(crate) type_spec_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// Where the outermost `&`/`&&` of a declaration of dependent
    /// reference type was written (`T &m;`): a substitution that forms
    /// a reference to `void` is diagnosed there (clang's
    /// `ReferenceTypeLoc`; [dcl.ref]/1).
    pub(crate) ref_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// The namespace named by the qualifier of each friend function (or
    /// function template pattern) declared with a qualified name in a
    /// templated class (`friend void N::f(T);`): its instantiation is
    /// declared with the same qualifier, so it redeclares `N::f`
    /// ([dcl.meaning.general]/2.3, [temp.friend]/1; WP-273).
    pub(crate) friend_qualifiers: kiln_source::fx::FxHashMap<DeclId, DeclId>,
    /// Where `override` / `final` were written on each member function
    /// whose override control is deferred to instantiation (a dependent
    /// class or type; `check_override_control`): the instantiation is
    /// declared with these locations ([class.virtual]/5).
    pub(crate) virt_spec_locs: kiln_source::fx::FxHashMap<DeclId, (Option<Loc>, Option<Loc>)>,
    /// A friend function template of a class template is being
    /// instantiated (`friend_function_template`): a qualified one that
    /// matches no template is not diagnosed (clang).
    pub(crate) instantiating_friend_template: bool,
    /// Where `constinit` was written on each variable declaration that
    /// has it ([dcl.constinit]; WP-159).
    pub(crate) constinit_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// Whether each reference variable named so far, whose initializer
    /// is not a plain static lvalue, is constant-initialized (so usable
    /// in constant expressions, [expr.const]/4; WP-231): evaluated once.
    pub(crate) ref_usable: kiln_source::fx::FxHashMap<DeclId, bool>,
    /// Loops whose body is a same-line null statement, awaiting their
    /// enclosing compound statement (`-Wempty-body`, WP-191).
    pub(crate) empty_loop_bodies:
        kiln_source::fx::FxHashMap<kiln_ast::StmtId, (EmptyBodyStmt, Loc)>,
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
    /// Building the type of a function parameter pack's declarator:
    /// a declarator whose type is in error recovers as clang's
    /// `GetFullTypeForDeclarator` does (an erroneous base or chunk is
    /// `int`, a function returning an array or function returns `int`, a
    /// reference to a reference collapses), so the pack check still
    /// runs on it ([dcl.fct]/16; [`Sema::type_for_declarator_at`]).
    pub(crate) declarator_int_recovery: bool,
    /// Replaying a non-dependent class-scope using-declaration of a
    /// pattern: clang walks the pattern's shadow list, which is newest
    /// first, so the instantiation's shadows come in reverse (WP-159).
    pub(crate) reverse_using_shadows: bool,
    /// The type `__builtin_va_list` names (its typedef sugar; the `A` of
    /// a builtin signature), once declared.
    pub(crate) va_list_type: Option<QualType>,
    /// The typedefs and alias-declarations naming an ext vector type
    /// directly, in declaration order: an element access names its
    /// result through the first that matches (clang `ExtVectorDecls`).
    pub(crate) ext_vector_typedefs: Vec<DeclId>,
    /// Overload resolution's pooled candidate sets and counters
    /// (spec/sema-overload.md §1.4).
    pub(crate) ovl: overload::OverloadScratch,
    /// Notes the next error takes (the user-declared candidates of an
    /// operator whose built-in fallback fails, spec/sema-overload.md
    /// §6.2): set and cleared around the fallback.
    pub(crate) trailing_notes: std::cell::RefCell<Vec<Diag>>,
    /// The designator warnings of a braced list checked in a template
    /// definition, by its semantic `InitListExpr`: an instantiation
    /// shares the non-dependent list but issues them again, as clang's
    /// re-checked list does (spec/sema.md §3.7, WP-196).
    pub(crate) designator_rewarn: kiln_source::fx::FxHashMap<ExprId, Vec<Diag>>,
    /// The `-Wnull-dereference` warnings of a template definition, by
    /// the dereference they point at, numbered in the order they were
    /// checked: an instantiation sharing the non-dependent expression
    /// issues them again, as clang's rebuilt one does (spec/sema.md
    /// §5.15, WP-235).
    pub(crate) null_deref_rewarn: kiln_source::fx::FxHashMap<ExprId, (usize, Diag)>,
    /// The non-dependent variadic arguments of a template definition's
    /// calls that the instantiation must diagnose (a `void` or non-trivial
    /// class argument), with the callee kind: clang checks them only in
    /// the instantiation, which shares the call (spec/sema.md §5.19).
    pub(crate) vararg_recheck: kiln_source::fx::FxHashMap<ExprId, u8>,
    /// The null dereferences warned about per instantiation context
    /// (`queue_null_deref`).
    pub(crate) null_deref_seen: kiln_source::fx::FxHashSet<(ExprId, u64)>,
    /// The warnings of the list being checked in a template definition,
    /// collected for `designator_rewarn`.
    pub(crate) designator_pending: Option<Vec<Diag>>,
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
    /// `delayed_access.len()`, the context and whether the checks are
    /// dropped ([temp.spec.general]/6) at each open delay point,
    /// innermost last.
    pub(crate) delayed_access_depth: Vec<(usize, DeclId, bool)>,
    /// Deprecation warnings held by the open delay points (with the
    /// 1-based depth of the point holding them), emitted when the
    /// declaration they belong to is formed and is not deprecated
    /// (spec/sema.md "Deprecation warnings").
    pub(crate) delayed_deprecations: Vec<(usize, Diag)>,
    /// The open deprecation delay points: the context each was opened in.
    pub(crate) depr_points: Vec<DeclId>,
    /// The declaration whose initializer is being parsed: a use in it is
    /// in that declaration's context for deprecation.
    pub(crate) depr_init: Option<DeclId>,
    /// The next delay point drops its checks (the declaration of an
    /// explicit instantiation or specialization; [`Sema::exempt_next_delayed_access`]).
    pub(crate) exempt_next_delay: bool,
    /// A standard conversion is being applied for an explicit cast (a
    /// complex-to-real conversion is then allowed; clang's
    /// `isCast(CCK)`).
    pub(crate) explicit_conv: bool,
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
    /// The entity a failed initialization names in place of "a variable"
    /// (clang `InitializedEntity::EK_Temporary` "a value" for `T{x}`,
    /// `EK_New` "a new value"): a `sema_init_conversion_failed` select.
    pub(crate) init_entity: Option<&'static str>,
    /// The variable whose initializer is being analysed (the entity of a
    /// top-level initializer list while [`Sema::init_entity`] is `None`;
    /// a flexible array member's initialization, §5.8).
    pub(crate) init_var: Option<DeclId>,
    /// The context access is checked from while a namespace-scope alias
    /// template is substituted in its own context: the use's
    /// (spec/sema-class.md §2.4, clang's `CheckTemplateIdType`).
    pub(crate) access_ctx: Option<DeclId>,
    /// Bumped by every access check that is not trivially satisfied (an
    /// alias template specialization whose substitution made one is not
    /// cached: its result depends on the context).
    pub(crate) access_checks: u32,
    /// The current full-expression's cleanup state (spec/sema-class.md
    /// §5.3).
    pub(crate) cleanup: class::CleanupState,
    /// The cleanup states of the enclosing full-expressions around
    /// unevaluated operands, innermost last.
    pub(crate) cleanup_saved: Vec<class::CleanupState>,
    /// The cleanup state at the start of each constant-expression being
    /// parsed, innermost last ([`Sema::enter_constant_evaluated`]).
    pub(crate) cleanup_marks: Vec<class::CleanupState>,
    /// The enclosing full-expression's pending cleanups at the start of a
    /// constant-expression that is completed later (a template argument
    /// converted while that full-expression is built), keyed by the
    /// constant-expression; restored by
    /// [`Sema::act_on_constant_full_expr`] (spec/sema-class.md §5.3).
    pub(crate) constant_outer_cleanups: kiln_source::fx::FxHashMap<ExprId, class::CleanupState>,
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
    /// The "passing argument to parameter here" note of a non-aggregate
    /// class parameter list-initialized from a braced argument (`f({1})`):
    /// taken by the outermost list constructor call, whose explicit or
    /// deleted constructor error carries it (clang `PrintInitLocationNote`).
    pub(crate) list_param_note: Option<Diag>,
    /// The aggregate temporary (and the reference's location) being
    /// list-initialized for a reference: the first `excess elements`
    /// error inside it takes clang's reference note (spec/sema.md).
    pub(crate) ref_temp_note: std::cell::Cell<Option<(QualType, Loc)>>,
    /// The mem-initializers of the constructor definition being parsed
    /// (spec/sema-class.md §4.9).
    pub(crate) mem_inits: class::MemInitState,
    /// Materialized temporaries collected for P2718 range-for lifetime
    /// extension (spec/sema-class.md §5.4).
    pub(crate) extend_collector: Option<Vec<ExprId>>,
    /// Diagnostics held back while a range-based `for` step runs, so
    /// clang's trailing notes (`when looking up 'begin' function...`)
    /// can be attached to the last of them (`range_for.rs`). The `usize`
    /// is the instantiation-stack length when the capture began: only
    /// diagnostics of the step itself are held, never one from an
    /// instantiation the step started (a SFINAE context traps those,
    /// [temp.deduct.general]/8, also WP-271's converting-constructor
    /// deduction for a rewritten `operator==`; a hard error keeps its
    /// context notes).
    pub(crate) diag_capture: std::cell::RefCell<Option<(usize, Vec<Diag>)>>,
    /// The "no member named" errors of qualified-ids held back until the
    /// end of the enclosing expression statement (clang's delayed typo
    /// correction, `TypoExpr`), while `typo_delay` is non-zero
    /// ([`Sema::begin_delayed_typos`], WP-308), in order.
    delayed_typos: Vec<HeldTypo>,
    /// The nesting depth of [`Sema::begin_delayed_typos`].
    typo_delay: u32,
    /// How many delayed-typo diagnostics were reported or held so far
    /// (also the sequence number of the next one, [`Sema::typo_mark`]).
    typo_count: u32,
    /// The recovery expressions that stand for a delayed typo (clang's
    /// `TypoExpr`s), reported or held ([`Sema::correct_delayed_typos_in`]).
    typo_exprs: kiln_source::fx::FxHashSet<ExprId>,
    /// Delayed typos whose expression the parser dropped without
    /// finishing it ([`Sema::drop_delayed_typos`]): clang never corrects
    /// such a `TypoExpr` and reports it at the end of the translation
    /// unit (WP-330).
    dropped_typos: Vec<Diag>,
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
    /// Redeclarations whose exception specification was not known when
    /// they were declared (a friend's, saved for its complete-class
    /// context or left uninstantiated), paired with the previous
    /// declaration: compared once it is (clang's
    /// `DelayedEquivalentExceptionSpecChecks`, spec/sema.md §4.7).
    pub(crate) delayed_exc_equiv: Vec<(DeclId, DeclId)>,
    /// Parameters whose default argument waits for its complete-class
    /// context, with the argument's location: a use before then is
    /// clang's "use of default argument to function ... that is declared
    /// later in class" ([class.mem.general]/7).
    pub(crate) unparsed_default_args: kiln_source::fx::FxHashMap<DeclId, Loc>,
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
    /// Set when the last declarator's type was diagnosed invalid in a
    /// way clang records on its `Declarator` (an unexpanded pack,
    /// [temp.variadic]/12): clang's `ParseDeclGroup` reuses the
    /// declarator, so the flag sticks and the group's later declarators
    /// are parsed but not declared ([`Sema::take_declarator_type_invalid`]).
    pub(crate) declarator_type_invalid: bool,
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
    pub(crate) inherited_defaults: kiln_source::fx::FxHashMap<DeclId, kiln_ast::types::TemplateArg>,
    /// The `alignas` / `aligned` attributes a variable or class
    /// declaration inherits from its previous declaration (clang
    /// `mergeAlignedAttrs`' inherited clones; [dcl.align]/6), in front of
    /// its own ([`crate::align_redecl`]).
    pub(crate) inherited_aligned: kiln_source::fx::FxHashMap<DeclId, Vec<kiln_ast::AttrId>>,
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
    /// `size_t` as clang 22 spells it: the implicit, unnamed-by-lookup
    /// alias `__size_t` (clang's `PredefinedSugarType`), the type of
    /// `sizeof`/`alignof` and of the allocation functions' size under
    /// `-fclang-compat=22` ([`Sema::size_type`]).
    pub(crate) size_t_sugar: QualType,
    /// clang 22's `__signed_size_t` (a `z` literal) and `__ptrdiff_t`
    /// (pointer subtraction), implicit like [`Sema::size_t_sugar`]
    /// ([`Sema::signed_size_type`], [`Sema::ptrdiff_type`]).
    pub(crate) signed_size_t_sugar: QualType,
    pub(crate) ptrdiff_t_sugar: QualType,
    /// C++20 modules: the units being read (spec/modules.md).
    pub(crate) modules: modules::ModSema,
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

    // One line per `Sema` field; the struct literal grows with every WP and
    // is not control flow (follow-up: derive `Default` for the state groups).
    #[allow(clippy::too_many_lines)]
    fn with_abi(
        mut ast: Ast,
        diags: &'a DiagEngine<'sm>,
        abi: Target,
        target: TargetTypes,
    ) -> Self {
        let tu = ast.tu;
        templ::builtins::declare_builtin_templates(&mut ast);
        builtins::declare_int128_typedefs(&mut ast, &target);
        let size_t = QualType::unqual(target.size_t);
        let ptrdiff_t = QualType::unqual(target.ptrdiff_t);
        let mut s = Sema {
            ast,
            diags,
            target,
            abi,
            pragma_packs: Vec::new(),
            exceptions: true,
            rtti: true,
            reflection: false,
            coro_aligned_allocation: false,
            range_lifetime_extension: false,
            gnu: false,
            clang_compat: 19,
            pack: (0, Vec::new()),
            scopes: vec![scope::Scope {
                first_decl: 0,
                ..scope::Scope::new(ScopeKind::TranslationUnit, Some(tu))
            }],
            ctx: tu,
            func: None,
            unevaluated: 0,
            decltype_calls: Vec::new(),
            agg_list_end: None,
            immediate: 0,
            constant_full_expr: 0,
            std: kiln_lex::Std::Cxx26,
            pending_odr: Vec::new(),
            misaligned: Vec::new(),
            imm: consteval::ImmState::default(),
            defining: Vec::new(),
            invalid_defining: kiln_source::fx::FxHashSet::default(),
            enums: Vec::new(),
            local_externs: Vec::new(),
            dropped_qualified_locals: kiln_source::fx::FxHashSet::default(),
            missing_typename_fns: kiln_source::fx::FxHashSet::default(),
            extern_c: kiln_source::fx::FxHashMap::default(),
            oven: Some(Oven::new(Budget::default())),
            global_ops: new_delete::GlobalOps::default(),
            braced_args: Vec::new(),
            prototype_params: Vec::new(),
            declarator_records: Vec::new(),
            unowned_params: std::collections::HashSet::new(),
            requires_param_scopes: 0,
            decomp_written: std::collections::HashMap::new(),
            binding_packs: std::collections::HashSet::new(),
            binding_pack_elems: std::collections::HashMap::new(),
            refs_minus_assign: std::collections::HashMap::new(),
            attr_macros: (0, Vec::new()),
            declared_returns: kiln_source::fx::FxHashMap::default(),
            handler_type_locs: kiln_source::fx::FxHashMap::default(),
            type_spec_locs: kiln_source::fx::FxHashMap::default(),
            ref_locs: kiln_source::fx::FxHashMap::default(),
            friend_qualifiers: kiln_source::fx::FxHashMap::default(),
            virt_spec_locs: kiln_source::fx::FxHashMap::default(),
            instantiating_friend_template: false,
            constinit_locs: kiln_source::fx::FxHashMap::default(),
            ref_usable: kiln_source::fx::FxHashMap::default(),
            empty_loop_bodies: kiln_source::fx::FxHashMap::default(),
            group_placeholders: kiln_source::fx::FxHashMap::default(),
            deferring_call_ref: false,
            declarator_int_recovery: false,
            reverse_using_shadows: false,
            va_list_type: None,
            ext_vector_typedefs: Vec::new(),
            ovl: overload::OverloadScratch::default(),
            trailing_notes: std::cell::RefCell::new(Vec::new()),
            designator_rewarn: kiln_source::fx::FxHashMap::default(),
            null_deref_rewarn: kiln_source::fx::FxHashMap::default(),
            vararg_recheck: kiln_source::fx::FxHashMap::default(),
            null_deref_seen: kiln_source::fx::FxHashSet::default(),
            designator_pending: None,
            deferred_call_refs: Vec::new(),
            classes: Vec::new(),
            implicit_declared: kiln_source::fx::FxHashMap::default(),
            special: class::SpecialState::default(),
            delayed_access: Vec::new(),
            delayed_access_depth: Vec::new(),
            delayed_deprecations: Vec::new(),
            depr_points: Vec::new(),
            depr_init: None,
            exempt_next_delay: false,
            explicit_conv: false,
            dependent_access: kiln_source::fx::FxHashMap::default(),
            speculative_friend: std::cell::Cell::new(false),
            access_failed: kiln_source::fx::FxHashSet::default(),
            replaying_access: false,
            init_entity: None,
            init_var: None,
            access_ctx: None,
            access_checks: 0,
            cleanup: class::CleanupState::default(),
            cleanup_saved: Vec::new(),
            cleanup_marks: Vec::new(),
            constant_outer_cleanups: kiln_source::fx::FxHashMap::default(),
            synth: Vec::new(),
            vtable_uses: Vec::new(),
            vtables_used: kiln_source::fx::FxHashSet::default(),
            synth_noted: std::cell::Cell::new(0),
            delegating: Vec::new(),
            ctor_entity: None,
            ctor_diag_ty: None,
            list_param_note: None,
            ref_temp_note: std::cell::Cell::new(None),
            mem_inits: class::MemInitState::default(),
            extend_collector: None,
            diag_capture: std::cell::RefCell::new(None),
            delayed_typos: Vec::new(),
            typo_delay: 0,
            typo_count: 0,
            typo_exprs: kiln_source::fx::FxHashSet::default(),
            dropped_typos: Vec::new(),
            range_for_outer_depth: None,
            delete_cxx26: true,
            unparsed_exc: kiln_source::fx::FxHashSet::default(),
            delayed_exc_equiv: Vec::new(),
            unparsed_default_args: kiln_source::fx::FxHashMap::default(),
            uninstantiated_exc: kiln_source::fx::FxHashMap::default(),
            fn_saved: Vec::new(),
            member_refs: kiln_source::fx::FxHashMap::default(),
            friend_tag: None,
            declarator_type_invalid: false,
            friend_class_template: None,
            inherited_defaults: kiln_source::fx::FxHashMap::default(),
            inherited_aligned: kiln_source::fx::FxHashMap::default(),
            inh_ctors: kiln_source::fx::FxHashMap::default(),
            inh_ctor_base: kiln_source::fx::FxHashMap::default(),
            friend_specs: kiln_source::fx::FxHashMap::default(),
            nsdmi_this: Vec::new(),
            initializing: None,
            templ: templ::TemplState::default(),
            inst: templ::InstState::default(),
            abstract_noted: kiln_source::fx::FxHashSet::default(),
            srcloc: builtins::SrcLocCache::default(),
            lambdas: Vec::new(),
            lambda_mangle: lambda::ManglingState::default(),
            size_t_sugar: size_t,
            signed_size_t_sugar: ptrdiff_t,
            ptrdiff_t_sugar: ptrdiff_t,
            modules: modules::ModSema::default(),
        };
        s.declare_size_t_sugar();
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
        // The overload candidates' notes go with the error they explain,
        // also when a range-based `for` holds it back.
        if !self.trailing_notes.borrow().is_empty()
            && d.id()
                .is_some_and(|id| self.diags.severity_at(id, d.loc()) >= kiln_diag::Severity::Error)
        {
            for n in self.trailing_notes.take() {
                d = d.with_note(n);
            }
        }
        if let Some((depth, held)) = self.diag_capture.borrow_mut().as_mut()
            && *depth == self.inst.stack.len()
        {
            held.push(d);
            return;
        }
        if self.inst.suppress_next_error.replace(false) {
            return;
        }
        let mut inst_notes = Vec::new();
        if !self.inst.stack.is_empty()
            && let Some(id) = d.id()
        {
            // Dedup is per instantiation (spec/diag.md §7.4).
            d = d.context(self.inst.stack.context_key());
            let sev = self.diags.severity_at(id, d.loc());
            if self.inst.stack.is_sfinae() {
                if (sev >= kiln_diag::Severity::Error || is_sfinae_failure(id))
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
                inst_notes = self.inst_context_notes();
            }
        }
        // The instantiations begun inside the innermost implicit
        // definition print first, then the definitions, then the rest.
        let synth_notes = self.synth_notes(&d);
        let inner = self
            .synth
            .last()
            .map_or(0, |c| self.inst.stack.len().saturating_sub(c.inst_depth))
            .min(inst_notes.len());
        let outer = inst_notes.split_off(inner);
        inst_notes.extend(synth_notes);
        inst_notes.extend(outer);
        d.prepend_notes(inst_notes);
        d.emit(self.diags);
    }

    /// Starts holding back the diagnostics clang reports through delayed
    /// typo correction (a qualified-id that names no member, `S::nom`)
    /// until the matching [`Sema::end_delayed_typos`]. clang corrects a
    /// `TypoExpr` when the full-expression is finished, after the parser
    /// has checked the `;` of an expression statement, so `S::nom y;`
    /// reports "expected ';' after expression" first (WP-308). Returns
    /// the mark to end with (the sequence number of the next typo);
    /// scopes nest.
    pub fn begin_delayed_typos(&mut self) -> usize {
        self.typo_delay += 1;
        self.typo_count as usize
    }

    /// Ends the scope begun at `mark`. The diagnostics it held stay held
    /// while an enclosing scope is open (the full-expression has not
    /// ended: a subscript inside an expression statement) and are emitted
    /// in order otherwise.
    pub fn end_delayed_typos(&mut self, mark: usize) {
        self.typo_delay = self.typo_delay.saturating_sub(1);
        if self.typo_delay == 0 {
            for t in self.take_held_typos(mark) {
                self.emit(t.diag);
            }
        }
    }

    /// Ends the scope begun at `mark`, emitting what it held now, even
    /// inside an enclosing scope: clang's `ParseExpressionList` corrects
    /// the typos of the expressions it parsed after an error (WP-330).
    pub fn flush_delayed_typos(&mut self, mark: usize) {
        self.typo_delay = self.typo_delay.saturating_sub(1);
        self.flush_typos_from(mark);
    }

    /// Emits now the typos held since `mark` (a [`Sema::typo_mark`]),
    /// without ending a scope.
    pub fn flush_typos_from(&mut self, mark: usize) {
        for t in self.take_held_typos(mark) {
            self.emit(t.diag);
        }
    }

    /// The current typo mark: typos reported from here on have a
    /// sequence number at least this (for [`Sema::flush_typos_from`]).
    pub fn typo_mark(&self) -> usize {
        self.typo_count as usize
    }

    /// Ends the scope begun at `mark`, keeping what it held for the end
    /// of the translation unit ([`Sema::emit_dropped_typos`]): the parser
    /// dropped the expression (a subscript missing its `]`), so clang
    /// never corrects its `TypoExpr` (WP-330).
    pub fn drop_delayed_typos(&mut self, mark: usize) {
        self.typo_delay = self.typo_delay.saturating_sub(1);
        let held = self.take_held_typos(mark);
        self.dropped_typos.extend(held.into_iter().map(|t| t.diag));
    }

    /// clang's `CorrectDelayedTyposInExpr(e)` as the parser calls it on a
    /// subexpression it is about to give up on (the callee of a call
    /// missing its `)`, the base of a subscript, WP-330): the typos held
    /// for the expressions in `e` are reported now. True if `e` holds a
    /// delayed typo, reported earlier or now (clang's `HadDelayedTypo`).
    pub fn correct_delayed_typos_in(&mut self, e: ExprId) -> bool {
        if self.typo_exprs.is_empty() {
            return false;
        }
        let mut found = kiln_source::fx::FxHashSet::default();
        let mut stack = vec![e];
        // A bound on the walk: recovery only, never a semantic result.
        let mut budget = 1usize << 16;
        while let Some(x) = stack.pop() {
            if budget == 0 {
                break;
            }
            budget -= 1;
            if self.typo_exprs.contains(&x) {
                found.insert(x);
            }
            stack.extend(self.ast.exprs.children(x, &self.ast.lists));
        }
        if found.is_empty() {
            return false;
        }
        let (now, keep): (Vec<HeldTypo>, Vec<HeldTypo>) = std::mem::take(&mut self.delayed_typos)
            .into_iter()
            .partition(|t| found.contains(&t.expr));
        self.delayed_typos = keep;
        for t in now {
            self.emit(t.diag);
        }
        true
    }

    /// Starts a new full-expression context inside an expression (a
    /// lambda body, a statement expression): its statements and
    /// declarations end their own full-expressions, so their delayed
    /// typos are not held for the enclosing statement. Returns the depth
    /// to restore with [`Sema::resume_delayed_typos`] (WP-330).
    pub fn suspend_delayed_typos(&mut self) -> u32 {
        std::mem::take(&mut self.typo_delay)
    }

    /// Restores the depth [`Sema::suspend_delayed_typos`] returned.
    pub fn resume_delayed_typos(&mut self, depth: u32) {
        self.typo_delay = depth;
    }

    /// Removes the held typos numbered `mark` or later, in order.
    fn take_held_typos(&mut self, mark: usize) -> Vec<HeldTypo> {
        let i = self.delayed_typos.partition_point(|t| (t.seq as usize) < mark);
        self.delayed_typos.split_off(i)
    }

    /// Emits the delayed typos of dropped expressions, at the end of the
    /// translation unit after the pending instantiations, as clang
    /// (WP-330).
    pub(crate) fn emit_dropped_typos(&mut self) {
        for d in std::mem::take(&mut self.dropped_typos) {
            self.emit(d);
        }
    }

    /// Emits `d` for the typo standing as recovery expression `e`, or
    /// holds it back inside a [`Sema::begin_delayed_typos`] scope
    /// (outside any instantiation, where a SFINAE context must see it).
    pub(crate) fn emit_delayed_typo(&mut self, d: Diag, e: ExprId) {
        let seq = self.typo_count;
        self.typo_count = self.typo_count.wrapping_add(1);
        if self.inst.stack.is_empty() {
            self.typo_exprs.insert(e);
        }
        if self.typo_delay > 0 && self.inst.stack.is_empty() {
            self.delayed_typos.push(HeldTypo { seq, expr: e, diag: d });
        } else {
            self.emit(d);
        }
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
        names::SemaNames::new(self).write_decl_name(d, true, &mut s);
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

/// clang's `SFINAEFailure` warnings: in a SFINAE context they are a
/// substitution failure whatever their severity, even when disabled
/// (`Sema::EmitDiagnostic` consults the SFINAE response before the
/// mapping). The C99-designator extensions are not among them.
fn is_sfinae_failure(id: DiagId) -> bool {
    matches!(
        id,
        DiagId::sema_designated_init_reordered
            | DiagId::sema_initializer_overrides
            | DiagId::sema_initializer_union_overrides
    )
}
