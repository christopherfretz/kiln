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
mod annot;
mod arm_builtins;
mod asm;
mod attr_cond;
pub mod attr_triage;
mod bit_int;
mod bitfield_constant;
mod builtins;
mod cconv;
pub mod cfg;
pub mod class;
mod class_b_attrs;
mod cleanup_attr;
mod complex;
mod consteval;
mod consteval_block;
mod consteval_only;
mod constexpr_body;
pub mod constfold;
mod contracts;
pub mod conv;
mod coroutine;
mod ctor_attr;
pub mod decl;
mod decl_attrs;
pub mod declspec;
mod deduce;
mod deprecated;
mod diagnose_if;
mod empty_body;
mod enable_if;
mod entity;
pub mod except;
mod expansion;
pub mod expr;
mod extern_c;
mod fallthrough;
mod flex_array;
mod fmv;
mod for_loop_analysis;
mod format_string;
mod forms;
mod gnu;
mod gnu_null;
mod gnu_typeof;
mod impcast;
mod implicit_fallthrough;
mod infinite_recursion;
pub mod init;
mod init_dtor;
mod init_priority;
mod int_in_bool;
mod int_overflow;
mod internal_linkage;
mod jump_scope;
mod kind_conflict;
pub mod lambda;
pub mod literal;
mod literal_op;
mod local_friend;
pub mod lookup;
mod loop_hint;
mod macro_fixit;
mod meta;
mod missing_noreturn;
pub mod modules;
mod ms_struct;
mod musttail;
pub mod names;
pub mod new_delete;
mod nonnull;
mod null_compare;
mod null_deref;
mod nullability;
mod oven_bridge;
pub mod overload;
mod packed;
mod predefined;
mod qualified_decl;
mod range_for;
mod reflect;
mod reflect_limit;
mod reflect_operand;
mod rtti;
pub mod scope;
mod section;
mod selectany;
mod self_ref;
mod side_effects;
mod splice;
mod splice_local;
mod splice_object;
pub mod stack_guard;
mod static_tls;
pub mod stmt;
mod struct_bind;
mod sve;
mod switch_conv;
mod switch_enum;
pub mod target;
pub mod templ;
mod template_id_typo;
mod tls_model;
mod transparent_union;
mod trivial_abi;
mod try_stmt;
mod type_diff;
mod undefined_used;
mod uninit;
mod unreachable_code;
mod unused_file_scope;
mod unused_var;
pub mod using;
mod vector;
mod vector_ops;
mod vexing;
mod visibility;

use kiln_abi::Target;
use kiln_ast::types::{PrintOptions, TargetTypes, aka, print_type, vector_diag_arg};
use kiln_ast::{Ast, AstBuilder, DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_source::{Loc, Span};

pub use class::{BaseSpecInfo, MemInitId, MemInitInfo};
pub use contracts::{ContractDeclKind, ContractScope};
pub use decl::{NameClass, TagUse};
pub use declspec::{
    BindingId, Chunk, DeclSite, DeclSpec, Declarator, DeclaratorContext, Decomposition, DefaultArg,
    NullSpec, ParamInfo, Sign, SpecialName, StorageSpec, TypeSpecKind, Width,
};
pub use empty_body::EmptyBodyStmt;
pub use expansion::{ExpansionHead, ExpansionInit};
pub use gnu::{OffsetofComponent, StmtExprScope};
pub use implicit_fallthrough::AttrMacro;
pub use init::{Designator, InitItem, InitList, InitValue, Initializer};
pub use kiln_oven::{Budget, EvalContext, EvalFailure, EvalKind, Oven};
pub use range_for::{RangeCollect, RangeForHead, RangeInit};
pub use scope::ScopeKind;
pub use template_id_typo::TemplateIdTypo;
pub use vector_ops::LaxVectorConversions;

/// A delayed-typo diagnostic held for the end of its full-expression
/// ([`Sema::begin_delayed_typos`]): its sequence number and the recovery
/// expression that stands for the `TypoExpr`.
#[derive(Debug)]
struct HeldTypo {
    seq: u32,
    expr: ExprId,
    diag: Diag,
    /// A speculative correction: `expr` is the corrected expression.
    spec: Option<Box<TypoSpec>>,
}

/// clang 19's typo correction of a template-id, tried ahead of its
/// correction point ([`Sema::speculate_template_id_typo`], WP-399): the
/// expression goes on with the corrected name while the diagnostics wait
/// ([`kiln_diag::DiagEngine::hold`]), and the correction stands if no
/// error came of an expression built on it.
#[derive(Debug)]
struct TypoSpec {
    /// The first held diagnostic issued after the correction.
    start: usize,
    /// Where the template-id's name is.
    loc: Loc,
    /// The diagnostic of the correction ("did you mean").
    suggest: Diag,
    /// What a failed speculation reports instead of the uncorrected
    /// diagnostic: the correction clang 19 goes on to (a variable the
    /// rejected function tied with, or the next distance's one value).
    fallback: Option<Diag>,
    /// The speculation is on a function another candidate at its
    /// distance ties with: valid, it makes the correction ambiguous
    /// (clang's `AmbiguousTypoExprs`) and nothing is corrected.
    ambiguous: bool,
    /// The tied value an `ambiguous` function speculation competes with,
    /// as the type and category of the expression it rebuilds into:
    /// an initializer it cannot initialize settles the tie for the
    /// function ([`Sema::settle_typo_tie`]).
    tie: Option<(QualType, kiln_ast::expr::ValueCategory)>,
    /// The full-expression warnings about an expression built on the
    /// correction, with their order ([`Sema::emit_after_typos`]): clang
    /// issues them after it corrects the `TypoExpr`.
    after: Vec<(u32, Diag)>,
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
    /// The class layouts computed for the deallocation-function lookup of
    /// virtual destructors ([`Sema::complete_class_align`]), kept between
    /// classes so a class lattice is laid out once rather than once per
    /// class (WP-450). It holds complete classes only (a class still being
    /// defined is laid out over it but not kept, WP-428), and a complete
    /// class's layout never changes within the TU.
    pub(crate) layout_cache: kiln_abi::LayoutCache,
    /// The `#pragma pack` directives seen, in order (location and the
    /// spelling of the argument list).
    pub pragma_packs: Vec<(Loc, String)>,
    /// The named sections seen, each with the first declaration placed in
    /// it and its flags (clang `ASTContext::SectionInfos`; [`section`]).
    pub(crate) sections: kiln_source::fx::FxHashMap<Box<[u8]>, (DeclId, u8)>,
    /// The known `no_sanitize("...")` arguments clang does not allow on a
    /// global variable, by attribute: warned about once the subject is
    /// known to be one (WP-675; `class_b_attrs`).
    pub(crate) no_sanitize_not_global: kiln_source::fx::FxHashMap<kiln_ast::AttrId, Vec<String>>,
    /// The attribute-token as written (scope, name) of each attribute
    /// spelled with a scope or a `__`-affixed name, by its location; how
    /// diagnostics name it ([`Sema::attr_shown`]).
    pub(crate) attr_spellings: kiln_source::fx::FxHashMap<
        Loc,
        (Option<kiln_source::symbol::Symbol>, kiln_source::symbol::Symbol),
    >,
    /// The builtin trait engine's quiet mode, used by the
    /// [meta.reflection.traits] functions (spec/reflection-traits.md).
    pub(crate) traits: templ::builtins::TraitState,
    /// C++ exceptions are enabled (`-f[no-]exceptions`; default on).
    pub exceptions: bool,
    /// The `throw`/`try` locations already diagnosed as used with
    /// exceptions disabled: a template's is reported at its first
    /// instantiation only (WP-607, [`Sema::check_exceptions_enabled`]).
    pub(crate) exc_disabled_seen: kiln_source::fx::FxHashSet<Loc>,
    /// The template patterns' non-dependent throw-expressions whose
    /// exceptions-disabled error waits for an instantiation: one the
    /// substitution shares instead of rebuilding is reported by
    /// [`Sema::note_shared_throws`].
    pub(crate) exc_disabled_pattern: kiln_source::fx::FxHashSet<ExprId>,
    /// Run-time type information is enabled (`-f[no-]rtti`; default on):
    /// `typeid` is an error otherwise.
    pub rtti: bool,
    /// `-freflection` (P2996 spike, WP-267; default off): `^^`, type
    /// splices and the reflection intrinsics (spec/reflection.md).
    pub reflection: bool,
    /// [`Sema::is_consteval_only_type`] of the complete classes seen
    /// (keyed by definition).
    pub(crate) consteval_only_records: kiln_source::fx::FxHashMap<DeclId, bool>,
    /// The consteval-only leaves of template definitions outside every
    /// immediate function context: checked again in each instantiation
    /// ([`Sema::note_inst_consteval_only`]).
    pub(crate) consteval_only_pattern: kiln_source::fx::FxHashSet<ExprId>,
    /// Depth of expressions being rebuilt by substitution (their
    /// consteval-only leaves follow the pattern's, not the builders').
    pub(crate) subst_expr_depth: u32,
    /// A template-id is being formed by substituting into a dependent
    /// one: a class template specialization it creates records all its
    /// arguments as written, as GCC's (spec/reflection.md §3.9).
    pub(crate) subst_template_id: bool,
    /// `-fcoro-aligned-allocation` (default off): a coroutine frame is
    /// allocated and freed with the `std::align_val_t` forms of
    /// `operator new`/`operator delete` when available (P2014R0 option 2,
    /// as clang; spec/sema.md "Coroutines").
    pub coro_aligned_allocation: bool,
    /// `-flax-vector-conversions=` (default `all`, as clang; spec/types.md
    /// `Vector`).
    pub lax_vector_conversions: LaxVectorConversions,
    /// The temporaries of a for-range-initializer are extended to the
    /// whole loop (C++23, P2718R0; spec/sema-class.md §5.4).
    pub range_lifetime_extension: bool,
    /// A GNU dialect (`-std=gnu++NN`): some extension warnings are
    /// off by default (clang's `*_in_gnu_mode` diagnostics, WP-183).
    pub gnu: bool,
    /// The clang major `-fclang-compat` names (19 or 22; default 22, as the driver): the
    /// few spellings that differ between the two releases follow it, e.g.
    /// a closure type in `__PRETTY_FUNCTION__` (spec/sema.md
    /// "Predefined identifiers").
    pub clang_compat: u32,
    /// `-fcf-protection=branch` (or `full`) is on: `nocf_check` applies
    /// (clang `LangOptions::CFProtectionBranch`; WP-813).
    pub cf_protection_branch: bool,
    /// The `AArch64` `sve` target feature is on (`-march=armv8-a+sve`,
    /// `-target-feature +sve`): the SVE vector types are usable in
    /// expressions and as automatic variables (spec/sema.md "SVE").
    pub sve: bool,
    /// `-msve-vector-bits=N` (clang's `VScaleMin` = `VScaleMax` = N/128):
    /// the one size `arm_sve_vector_bits` accepts (spec/sema.md "SVE").
    pub sve_vector_bits: Option<u32>,
    /// The begin of the nested-name-specifier whose component the parser
    /// is acting on (set around each `act_on_nested_name_*` call): clang
    /// 22's point of instantiation for a class or enumeration completed
    /// as a qualifier (`Sema::nns_poi`).
    pub nns_start: Option<Loc>,
    /// GCC's implicit library builtins (`-stdlib=libstdc++`, WP-380): a
    /// call of a recognized C library `ceil` / `floor` / `trunc` /
    /// `round` / `fabs` / `fmin` / `fmax` / `copysign` function folds in
    /// constant evaluation as its `__builtin_` twin, as GCC folds
    /// `::ceil` (spec/sema.md §5.4).
    pub gnu_lib_builtins: bool,
    /// `-fno-builtin` / `-ffreestanding` (clang `LangOptions::NoBuiltin`,
    /// WP-808): no library function is recognized as a builtin
    /// ([`Sema::lib_builtin_info`]), so none gets the implicit attributes
    /// or lowers as its `__builtin_` twin.
    pub no_builtin: bool,
    /// The `-fno-builtin-<name>` names, sorted (clang
    /// `LangOptions::NoBuiltinFuncs`): each is not recognized as a
    /// library builtin; `std-<name>` names a `std` one (WP-808).
    pub no_builtin_fns: Vec<String>,
    /// `-f[no-]sized-deallocation` (clang `LangOptions::SizedDeallocation`,
    /// default on; WP-808): off, the sized global `operator delete` and
    /// `operator delete[]` are not declared implicitly, and a global
    /// deallocation lookup never prefers a sized one
    /// ([`Sema::usual_delete`]).
    pub sized_deallocation: bool,
    /// `-f[no-]char8_t` (clang `LangOptions::Char8`): `None` is the
    /// default, C++20 and later; see [`Sema::char8`].
    pub char8: Option<bool>,
    /// Whether a name is one of clang's library builtins at the clang 22
    /// level or not (clang `Builtin::Context::isBuiltinFunc`'s table, set
    /// by the parser from the preprocessor's): the names a `no_builtin`
    /// attribute accepts (WP-808).
    pub lib_builtin_name: fn(&str, bool) -> bool,
    /// The functions whose `no_builtin` attribute waits for their
    /// definition (clang checks the declarator's definition kind), with the
    /// attribute's location: a `{` body clears one, `= default` / `=
    /// delete` reports it, the end of the unit reports the rest as
    /// declarations (WP-808).
    pub(crate) no_builtin_pending: Vec<(DeclId, Loc)>,
    /// `-f[no-]math-errno` (clang `LangOptions::MathErrno`, the Linux
    /// default on): without it the errno-setting math library functions
    /// are implicitly `const` (spec/sema.md §5.4, WP-806).
    pub math_errno: bool,
    /// `-fkiln-dump-cfg`: build the graph of every analysed body and
    /// append its dump to [`Sema::cfg_out`] (spec/sema.md §7.5.1).
    pub cfg_dump: bool,
    /// `-fkiln-cfg-stats`: build the graph of every analysed body and
    /// append a line with its size and build time to [`Sema::cfg_out`]
    /// (spec/sema.md §7.5.1).
    pub cfg_stats: bool,
    /// Every call candidate's deduction runs in full: the pre-filters
    /// that return a deduction's early failure without it
    /// (`deduce_call_prefail`, spec/sema.md §6.1) are off. A test hook
    /// (WP-657): the results are the same either way.
    #[doc(hidden)]
    pub full_deduction: bool,
    /// Every function redeclaration search scans its whole lookup entry:
    /// the redeclaration index (`overload/redecl_index.rs`, spec/sema.md
    /// §3) is off. A test hook (WP-658): the results are the same
    /// either way.
    #[doc(hidden)]
    pub full_redecl_scan: bool,
    /// The redeclaration index (WP-658).
    pub(crate) redecl_index: overload::redecl_index::RedeclIndex,
    /// The explicit-argument substitution memo of deduction step 1
    /// (WP-661; off under [`Sema::full_deduction`]).
    pub(crate) explicit_memo: templ::explicit_memo::ExplicitMemo,
    /// GCC's `-fpermissive` (default off): the C++26 arithmetic between
    /// different enumerations or an enumeration and a floating type
    /// (P2864, ill-formed) is diagnosed with the C++20/23 deprecation
    /// warning, so GCC-built code still compiles (spec/driver.md).
    pub permissive: bool,
    /// The dumps of `-fkiln-dump-cfg` and lines of `-fkiln-cfg-stats`.
    pub cfg_out: String,
    /// `-fkiln-cfg-max-elems`: a body whose graph would hold more
    /// elements gets none; its flow-sensitive warnings are skipped with
    /// `-Rkiln-analysis-skipped` (spec/sema.md §7.5).
    pub cfg_max_elems: usize,
    /// The graph of the body being finished, its vectors reused from
    /// body to body.
    pub(crate) cfg_body: cfg::BodyCfg,
    /// The uninitialized-values analysis' reused buffers (spec/sema.md §7.11).
    pub(crate) uninit_scratch: uninit::Scratch,
    /// The functions (first declarations) clang 22 infers `noreturn`
    /// (`InferredNoReturnAttr`, [`Sema::infer_noreturn`]).
    pub(crate) inferred_noreturn: kiln_source::fx::FxHashSet<DeclId>,
    /// The classes (first declarations) clang infers `[[gsl::Pointer]]`
    /// on from a `std` container's `iterator` typedef naming them
    /// (`Sema::inferGslPointerAttribute`, spec/sema-class.md §5.6).
    pub(crate) gsl_inferred_pointers: kiln_source::fx::FxHashSet<DeclId>,
    /// Set while a typedef is instantiated: its gsl inference runs only
    /// for a pattern naming a dependent type (clang
    /// `TemplateDeclInstantiator::InstantiateTypedefNameDecl`).
    pub(crate) in_typedef_instantiation: bool,
    /// The `#pragma pack` state (clang's `AlignPackStack`): the current
    /// maximum field alignment in bytes (0: none) and the pushed
    /// `(label, value)` entries. Applied to each class definition as it
    /// begins (`record_pack`).
    pub(crate) pack: (u16, Vec<(Option<String>, u16)>),
    /// `#pragma ms_struct on` is in effect (clang `MSStructPragmaOn`):
    /// each class definition that begins gets an implicit `ms_struct`.
    pub(crate) ms_struct_pragma: bool,
    /// The FP pragma state (clang's `CurFPFeatureOverrides` and
    /// `FpPragmaStack`): the override in effect, relative to the command
    /// line, and the `float_control(push)` entries.
    pub(crate) fp_pragma: (kiln_ast::fp::FpOverride, Vec<kiln_ast::fp::FpOverride>),
    pub(crate) scopes: Vec<scope::Scope>,
    /// The current semantic declaration context.
    pub(crate) ctx: DeclId,
    pub(crate) func: Option<stmt::FnState>,
    /// Depth of unevaluated operands (`sizeof`, `alignof`).
    pub(crate) unevaluated: u32,
    /// The id-expression being built is a qualified name that is the
    /// whole operand of unary `&` ([`Sema::act_on_id_expression_in`]).
    pub(crate) address_operand: bool,
    /// [`stmt::FnState::volatile_assigns`] outside a function body (a
    /// namespace-scope initializer, a default member initializer).
    pub(crate) volatile_assigns: Vec<ExprId>,
    /// The operands of the `decltype`s being parsed, innermost last: the
    /// unevaluated depth of each and the calls with an incomplete class
    /// prvalue result it deferred ([dcl.type.decltype]/2).
    pub(crate) decltype_calls: Vec<(u32, Vec<deduce::DeferredCall>)>,
    /// The `}` of the innermost braced aggregate list being checked: a
    /// brace-elided sublist's element destructor checks are located
    /// there (`init_dtor.rs`).
    /// The unrecoverable error count when the outermost braced list
    /// being checked started: clang's `InitListChecker::hadError`, which
    /// suppresses "excess elements" once an element failed (WP-420).
    pub(crate) agg_error_mark: Option<u32>,
    /// An initializer clause was left over in the outermost braced list
    /// being checked: the initialization failed (clang's `hadError`).
    pub(crate) agg_excess: std::cell::Cell<bool>,
    pub(crate) agg_list_end: Option<Loc>,
    /// Depth of immediate function contexts ([expr.const]/16) entered
    /// by `if consteval`.
    pub(crate) immediate: u32,
    /// Depth of [`Sema::act_on_constant_full_expr`]: the full-expression
    /// ending is a manifestly constant-evaluated constant-expression (no
    /// runtime-behavior warnings, clang's `ConstantEvaluated` context).
    pub(crate) constant_full_expr: u32,
    /// Inside the full-expressions of an initializer dropped after its
    /// diagnostic: an unexpanded pack in them is not reported again
    /// (clang's `AddInitializerToDecl` returns for an invalid variable;
    /// likewise a for-range-initializer, `ActOnCXXForRangeStmt`).
    pub(crate) discarding_init: bool,
    /// The language standard (set by the parser; spellings in notes).
    pub(crate) std: kiln_lex::Std,
    /// `DeclRefExpr`s of the current full-expression that are potential
    /// odr-uses ([basic.def.odr]/4): resolved at the end of the
    /// full-expression (spec/sema.md §5.2).
    pub(crate) pending_odr: Vec<ExprId>,
    /// `DeclRefExpr`s of the current full-expression, in a dependent
    /// context inside a lambda, naming an enclosing local that might be
    /// usable in constant expressions: the potential captures decided at
    /// the end of the full-expression (clang
    /// `LambdaScopeInfo::PotentialCaptures`; spec/sema-lambda.md §3).
    pub(crate) potential_caps: Vec<ExprId>,
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
    /// Set while a member access resolved in a template's pattern is
    /// formed again for an instantiation; taken by the next object
    /// completeness check ([`Sema::member_object`]).
    pub(crate) pattern_member_access: bool,
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
    /// The `#pragma GCC visibility` stack (clang `VisContext`): a pushed
    /// visibility (0 default, 1 hidden, 2 protected) and the pragma's
    /// location, or `None` for a namespace with a visibility attribute,
    /// which suspends the pragma in its body (spec/sema.md §4.5).
    pub(crate) vis_stack: Vec<Option<(u32, Loc)>>,
    /// The open namespace definitions that pushed a `None` entry.
    pub(crate) vis_namespaces: Vec<DeclId>,
    /// The names of `#pragma weak` pragmas that preceded any declaration
    /// of them, with the name's location (clang
    /// `WeakUndeclaredIdentifiers`).
    pub(crate) weak_pending: Vec<(kiln_source::symbol::Symbol, Loc)>,
    /// Functions declared with a parameter that needed the missing-
    /// `typename` recovery: no qualified redeclaration matches them
    /// (spec/sema-templates.md, clang's invalid declaration).
    pub(crate) missing_typename_fns: kiln_source::fx::FxHashSet<DeclId>,
    /// The latest namespace-scope declaration of each function or
    /// variable with C language linkage, by name ([dcl.link]/6; clang's
    /// extern "C" context, `extern_c.rs`).
    pub(crate) extern_c: kiln_source::fx::FxHashMap<kiln_source::symbol::Symbol, DeclId>,
    /// The one-character `>` tokens the parser split off a `>>` or `>>=`
    /// closing a template argument list ([temp.names]/4), for
    /// `token_end` (clang measures the split token).
    pub(crate) split_greaters: kiln_source::fx::FxHashSet<Loc>,
    /// The constant evaluator of the translation unit (spec/sema.md §8).
    /// Taken out for the duration of an evaluation, so the bridge can
    /// reach the rest of `Sema` (`ensure_body` instantiates through it,
    /// spec/sema-templates.md §4.11); `None` only while one runs. Boxed,
    /// so taking it out and putting it back moves a pointer, not the
    /// evaluator (WP-645).
    pub(crate) oven: Option<Box<Oven>>,
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
    /// Where a function declared `noinline`, `always_inline` or
    /// `flatten` begins (its leading attributes included; clang
    /// `Decl::getBeginLoc`): the note of a statement attribute that
    /// overrides it points there (WP-813).
    pub(crate) inline_fn_begin_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// Where the outermost `&`/`&&` of a declaration of dependent
    /// reference type was written (`T &m;`): a substitution that forms
    /// a reference to `void` is diagnosed there (clang's
    /// `ReferenceTypeLoc`; [dcl.ref]/1).
    pub(crate) ref_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// Where the innermost `*` or `[` of a declaration of dependent type
    /// was written, when it applies to the decl-specifiers (`T *m;`,
    /// `T const *volatile *m;`'s first `*`, `T a[3];`, a type parameter's
    /// `= T *`): a substitution that forms a pointer to (or an array of)
    /// references is diagnosed there, as clang's `TreeTransform` rebuilds
    /// the type at its `TypeLoc` sigil ([dcl.ref]/5, [dcl.array]/1).
    pub(crate) inner_chunk_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// [`Sema::inner_chunk_locs`] of the type parameter default or
    /// alias-declaration type just parsed, adopted by
    /// `act_on_template_param_default` / `act_on_alias_decl`.
    pub(crate) pending_chunk_loc: Loc,
    /// Where a `constexpr` special member function's declaration
    /// begins, when not at its name: a constexpr defaulted special member
    /// of a class with virtual bases is diagnosed there before C++26
    /// (clang `getBeginLoc`; [dcl.fct.def.default]/3, P3533R2).
    pub(crate) constexpr_begin_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
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
    /// Functions defined in a class whose body is saved for the end of
    /// the outermost class ([class.mem.general]/7,
    /// `act_on_delayed_body`): a friend template of a class template
    /// specialization instantiated before the body is parsed (a use of
    /// the specialization in a body) is still instantiated from that
    /// definition (clang's `willHaveBody`).
    pub(crate) delayed_bodies: kiln_source::fx::FxHashSet<DeclId>,
    /// While a friend function template of a class template is
    /// redeclared (`friend_function_template`): its pattern's
    /// constraints (type-constraints, requires-clauses) mapped to their
    /// substitution with the specialization's levels, the friend's own
    /// parameters at their retained depth, which redeclaration matching
    /// compares instead ([temp.over.link]/6; clang's
    /// `AreConstraintExpressionsEqual`, WP-446).
    pub(crate) friend_cmp_constraints: kiln_source::fx::FxHashMap<ExprId, ExprId>,
    /// While a friend of a class template is redeclared: a friend
    /// function template one of whose constraints names an enclosing
    /// template's parameter, or a non-template friend with a
    /// requires-clause, declares a function (template) of its own, never
    /// one of another scope ([temp.friend]/9; clang's
    /// `FriendConstraintsDependOnEnclosingTemplate`, WP-726, WP-779).
    pub(crate) friend_template_apart: bool,
    /// Where `constinit` was written on each variable declaration that
    /// has it ([dcl.constinit]; WP-159).
    pub(crate) constinit_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// Whether each reference variable named so far, whose initializer
    /// is not a plain static lvalue, is constant-initialized (so usable
    /// in constant expressions, [expr.const]/4; WP-231): evaluated once.
    pub(crate) ref_usable: kiln_source::fx::FxHashMap<DeclId, bool>,
    /// The automatic variable a variable's constant value refers to
    /// (P2686R5; [`kiln_oven::automatic_referent`]), by the variable,
    /// once its value is known: a use outside that variable's function
    /// odr-uses it (`Sema::representable_here`).
    pub(crate) auto_referent: kiln_source::fx::FxHashMap<DeclId, Option<DeclId>>,
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
    /// The declarator whose type is being formed declares a non-static
    /// data member: a variable length array bound that does not fold is
    /// clang's `err_typecheck_field_variable_size` there
    /// ([`Sema::type_for_declarator_at`]).
    pub(crate) declarator_field: bool,
    /// The diagnostic an alias-declaration's type-id left for its name: a
    /// variable length array bound at namespace or class scope folded
    /// (`-Wgnu-folding-constant`) or rejected (clang
    /// `CheckTypedefForVariablyModifiedType` on the `TypeAliasDecl`).
    pub(crate) pending_alias_vla: Option<DiagId>,
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
    /// Splices whose operand was read as a frozen constant without an
    /// Oven evaluation (spec/reflection.md §5, the splice fast path).
    pub(crate) splice_fast: u64,
    /// The `typeid` expression operands being formed, innermost last:
    /// the open lambda count and the unevaluated depth inside each
    /// ([`Sema::enter_typeid_operand`]; [expr.prim.splice]/2.1.4
    /// disregards the effect of an enclosing `typeid`).
    pub(crate) typeid_operands: Vec<(usize, u32)>,
    /// The `std::meta::substitute` memo (spec/reflection.md §3.3).
    pub(crate) meta_subst: meta::SubstMemo,
    /// The placeholder [`Sema::lend_oven`] leaves in a bridge callback's
    /// Oven slot while the running Oven is lent, kept for the next loan
    /// (a loan allocates no Oven of its own).
    pub(crate) spare_oven: Option<Box<Oven>>,
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
    /// The always-non-null comparison warnings of a template definition,
    /// by the operand they point at: an instantiation sharing the
    /// non-dependent operand issues them again, as clang's rebuilt
    /// comparison does (spec/sema.md §5.26).
    pub(crate) null_compare_rewarn: kiln_source::fx::FxHashMap<ExprId, (usize, Diag)>,
    /// The non-dependent variadic arguments of a template definition's
    /// calls that the instantiation must diagnose (a `void` or non-trivial
    /// class argument), with the callee kind: clang checks them only in
    /// the instantiation, which shares the call (spec/sema.md §5.19).
    pub(crate) vararg_recheck: kiln_source::fx::FxHashMap<ExprId, u8>,
    /// The non-dependent calls of a template definition to a function
    /// with an argument-dependent `diagnose_if`, by their first converted
    /// argument: the function, the converted arguments and where to
    /// diagnose. clang checks them only in the instantiation, which shares
    /// the call (spec/sema.md "The `diagnose_if` attribute").
    pub(crate) diagnose_if_recheck: kiln_source::fx::FxHashMap<ExprId, (DeclId, Vec<ExprId>, Loc)>,
    /// Whether the translation unit has a `diagnose_if` attribute: until
    /// it does, calls and uses skip looking for one.
    pub(crate) any_diagnose_if: bool,
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
    /// The class stacks set aside by each enclosing instantiation
    /// context, outermost first: a class still being defined there keeps
    /// its friends for access checks made inside the instantiation
    /// (spec/sema-class.md §8.3).
    pub(crate) suspended_classes: Vec<Vec<class::ClassBuild>>,
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
    /// A C-style or functional cast is converting a pointer to a base
    /// class: the base may be inaccessible ([expr.cast]/4, clang's
    /// `IgnoreBaseAccess`).
    pub(crate) cast_ignores_base_access: bool,
    /// Access checks in a class template pattern (or partial
    /// specialization) that passed only because the class may instantiate
    /// to a befriended specialization, keyed by the pattern's first
    /// declaration; re-run in each instantiation (clang's dependent access
    /// diagnostics; spec/sema-templates.md §4.8).
    pub(crate) dependent_access: kiln_source::fx::FxHashMap<DeclId, Vec<class::AccessCheck>>,
    /// Set by the access check when friendship came only from such a
    /// befriended specialization.
    pub(crate) speculative_friend: std::cell::Cell<bool>,
    /// Namespace `std::meta` (primary context) once found: the associated
    /// namespace of `std::meta::info` for ADL (`overload/adl.rs`).
    pub(crate) std_meta_ns: std::cell::Cell<Option<DeclId>>,
    /// Access errors reported, as (member, location, innermost context),
    /// so a replayed check does not repeat one its instantiation redid.
    pub(crate) access_failed: kiln_source::fx::FxHashSet<(DeclId, Loc, DeclId)>,
    /// True while [`Sema::replay_dependent_access`] runs.
    pub(crate) replaying_access: bool,
    /// The entity a failed initialization names in place of "a variable"
    /// (clang `InitializedEntity::EK_Temporary` "a value" for `T{x}`,
    /// `EK_New` "a new value"): a `sema_init_conversion_failed` select.
    pub(crate) init_entity: Option<&'static str>,
    /// The warning of the last usual arithmetic conversions (C++20's
    /// deprecated enumeration operands), for the caller to report at the
    /// operator ([`Sema::report_arith_warning`]).
    pub(crate) arith_warning: Option<conv::ConvError>,
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
    /// The final overriders in each used vtable, by class (first
    /// declaration; `class/virtual/used.rs`, WP-414): each use of a
    /// vtable re-marks its bases' overriders, so a lattice computed them
    /// once per use and class.
    pub(crate) vtable_finals: kiln_source::fx::FxHashMap<DeclId, std::rc::Rc<[DeclId]>>,
    /// Module classes whose vtables are defined here even when their key
    /// function is defined in another unit (clang `DefinitionRequired`;
    /// spec/modules.md §7).
    pub(crate) module_vtables: kiln_source::fx::FxHashSet<DeclId>,
    /// The depth of `synth` whose notes were last printed (clang
    /// `LastEmittedCodeSynthesisContextDepth`).
    pub(crate) synth_noted: std::cell::Cell<usize>,
    /// The reused stack of the expression walks ([`Sema::take_walk_stack`]):
    /// a walk takes it, cleared, and puts it back, so a full-expression
    /// check allocates no stack per call (WP-645). A nested walk finds it
    /// empty and starts its own.
    pub(crate) walk_stack: std::cell::RefCell<Vec<ExprId>>,
    /// [`Sema::walk_stack`] for the implicit-conversion walk, whose work
    /// items carry the context location.
    pub(crate) conv_walk_stack: Vec<(ExprId, Loc)>,
    /// The reused buffers of [`Sema::exprs_equivalent`] (WP-645).
    pub(crate) equiv_scratch: std::cell::RefCell<kiln_ast::types::EquivScratch>,
    /// The reused argument and option buffers of call-candidate
    /// deductions, one entry per nesting level (WP-657).
    pub(crate) cand_scratch: Vec<(Vec<overload::ArgInfo>, Vec<overload::IcsOptions>)>,
    /// The reused template-parameter buffers of deductions, one entry per
    /// nesting level (WP-657).
    pub(crate) deduce_scratch: Vec<Vec<DeclId>>,
    /// Delegating constructors and their targets, checked for cycles at
    /// the end of the translation unit ([class.base.init]/6).
    pub(crate) delegating: Vec<(DeclId, DeclId, Loc)>,
    /// Functions and variables odr-used while undefined, as first
    /// declarations with their first such use, in use order (clang
    /// `UndefinedButUsed`), checked at the end of the translation unit
    /// (`undefined_used.rs`).
    pub(crate) undefined_used: Vec<(DeclId, Loc)>,
    /// The first declarations in [`Sema::undefined_used`].
    pub(crate) undefined_used_seen: kiln_source::fx::FxHashSet<DeclId>,
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
    /// How many [`TypoSpec`]s are open (each holds the engine once).
    open_specs: u32,
    /// While specs are open: each recovery expression built, with the
    /// held-diagnostic count then (an error is the failure of the first
    /// recovery expression built after it).
    spec_failures: Vec<(ExprId, usize)>,
    /// While specs are open: the held diagnostics that are typo
    /// corrections themselves, never a failure of a speculation.
    spec_exempt: kiln_source::fx::FxHashSet<usize>,
    /// The scope depth at the point of instantiation (clang numbers the
    /// implicit `__begin`/`__end` variables of an instantiated dependent
    /// range-based `for` from `getCurScope()`); set by the outermost
    /// `in_inst_context`.
    pub(crate) range_for_outer_depth: Option<usize>,
    /// The location of a condition that is clang 19's speculative typo
    /// correction: clang converts the uncorrected `TypoExpr` to `bool`
    /// (a no-op) and so never checks the corrected expression's
    /// conversion ([`Sema::act_on_condition`]).
    pub(crate) cond_typo_at: Loc,
    /// [`Sema::note_condition_typo`]'s location, taken by the next
    /// [`Sema::act_on_condition`].
    pending_cond_typo: Loc,
    /// The expressions that stand for clang's `ExprError` (a name lookup
    /// found nothing): an initializer holding one is not performed
    /// ([`Sema::is_expr_error`], clang's `ActOnInitializerError`).
    expr_errors: kiln_source::fx::FxHashSet<ExprId>,
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
    /// Function template specializations and members of class template
    /// specializations whose dependent annotations are substituted when
    /// needed (spec/annotations.md §5): the specialization maps to its
    /// pattern.
    pub(crate) uninstantiated_annots: kiln_source::fx::FxHashMap<DeclId, DeclId>,
    /// Set while the parser reads a base-specifier's
    /// attribute-specifier-seq: an annotation operand naming a pack is
    /// kept, since the base-specifier may turn out to be a pack expansion
    /// whose pattern includes it ([temp.variadic]/5.4; WP-867). The
    /// parser checks it once the `...` is known
    /// ([`Sema::check_base_annotation_packs`]).
    pub(crate) base_annotation_packs: bool,
    /// The annotation each annotation attribute already produced: one
    /// written on a declaration's leading attribute-specifier-seq
    /// appertains to every declarator's entity and is one annotation
    /// ([dcl.attr.annotation]/3; `[[=1]] int x, y;`).
    pub(crate) annot_of_attr: kiln_source::fx::FxHashMap<kiln_ast::AttrId, kiln_ast::annot::AnnId>,
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
    /// Set by the parser for the class declaration after an extraneous
    /// `template<>` (`template<> struct S;`): clang's
    /// `err_template_tag_noparams`, emitted by [`Sema::act_on_tag`] (after
    /// the class-specifier's own errors, as clang's `ActOnTag`). One
    /// naming a non-template class is also clang's "specialization of
    /// member does not specialize an instantiated member".
    pub extraneous_tag_head: Option<Diag>,
    /// Set when the last declarator's type was diagnosed invalid in a
    /// way clang records on its `Declarator` (an unexpanded pack,
    /// [temp.variadic]/12): clang's `ParseDeclGroup` reuses the
    /// declarator, so the flag sticks and the group's later declarators
    /// are parsed but not declared ([`Sema::take_declarator_type_invalid`]).
    pub(crate) declarator_type_invalid: bool,
    /// The "restrict requires a pointer or reference" the last
    /// decl-specifier-seq's type reported (its `__restrict` and type),
    /// repeated for each later declarator of its group as clang forms
    /// every declarator's type anew ([`Sema::take_declspec_restrict_error`]).
    pub(crate) declspec_restrict_error: Option<(Loc, QualType)>,
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
    /// The unnamed namespace of each namespace scope, keyed by the
    /// scope's primary redeclaration context ([namespace.unnamed]/1;
    /// clang's `getAnonymousNamespace` on the original namespace), so an
    /// unnamed namespace in a reopened namespace or inside `extern "C++"`
    /// extends the first one.
    pub(crate) anon_namespaces: kiln_source::fx::FxHashMap<DeclId, DeclId>,
    /// The `this` type of the default member initializers being parsed,
    /// innermost last, with their fields (`C*`, spec/sema-class.md §2.5).
    pub(crate) nsdmi_this: Vec<(DeclId, QualType)>,
    /// The contract assertions whose predicates are being analysed,
    /// innermost last (spec/contracts.md §5).
    pub(crate) contract_ctx: Vec<contracts::ContractCtx>,
    /// A contract assertion was seen: initializations are checked for
    /// contract violations ([`Sema::check_contract_init`]).
    pub(crate) has_contracts: bool,
    /// The parameters already diagnosed by the postcondition const rule
    /// ([dcl.contract.func]/7): each once, and the parameter stays
    /// valid, so the body's uses of it evaluate (spec/contracts.md §9).
    pub(crate) post_param_diagnosed: kiln_source::fx::FxHashSet<DeclId>,
    /// The locations of the template-pattern parameters of non-dependent
    /// type diagnosed by that rule at definition time, which their
    /// instantiations do not report again.
    pub(crate) post_param_pattern_locs: kiln_source::fx::FxHashSet<Loc>,
    /// A non-dependent friend function of a templated class -> the
    /// namespace-scope function it will redeclare once instantiated
    /// (`pattern_friend_prev`): its seq's const rule on postcondition
    /// parameters is checked on the pattern ([dcl.contract.func]/7).
    pub(crate) friend_pattern_prev: kiln_source::fx::FxHashMap<DeclId, DeclId>,
    /// The static data member whose out-of-line initializer is being
    /// analysed: its class is the access context ([class.access]/7).
    pub(crate) initializing: Option<DeclId>,
    /// The `enable_if` conditions of the pattern of the member function
    /// being instantiated by `Substituter::function_decl`, for the
    /// redeclaration check of the declaration it makes (whose attributes
    /// are instantiated after it is declared; clang instantiates them
    /// before `CheckFunctionDeclaration`). Taken by
    /// [`Sema::new_fn_enable_if`].
    pub(crate) inst_enable_if: Option<kiln_ast::ids::ListId>,
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
    /// The frozen strings and source locations of the `std::meta` names
    /// family, per reflection (`meta/names.rs`).
    pub(crate) meta_names: meta::names::NameCache,
    /// The member lists and access scope of the `std::meta` member
    /// queries (`meta/member.rs`; spec/reflection-members.md §2).
    pub(crate) meta_members: meta::member::MemberState,
    /// The lambdas whose bodies are being analysed, innermost last
    /// (spec/sema-lambda.md; WP-075).
    pub(crate) lambdas: Vec<lambda::LambdaScope>,
    /// How many of [`Sema::lambdas`] enclose the point of instantiation
    /// rather than the code being instantiated ([`Sema::in_inst_context`]
    /// sets the template heads aside, so those lambdas' head counts no
    /// longer index them).
    pub(crate) lambda_base: usize,
    /// The packs of enclosing scopes a finished lambda's body names
    /// outside an expansion ([`lambda::LambdaScope`]'s `packs`): what
    /// the lambda-expression's "contains an unexpanded parameter pack"
    /// diagnostic names.
    pub(crate) lambda_packs: kiln_source::fx::FxHashMap<kiln_ast::ids::LambdaId, Vec<DeclId>>,
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
    /// The decl-specifier nullability markers awaiting their declarator
    /// and the qualifier each stands for (`nullability.rs`).
    pub(crate) declspec_nulls: Vec<(QualType, NullSpec)>,
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
            layout_cache: kiln_abi::LayoutCache::default(),
            pragma_packs: Vec::new(),
            sections: kiln_source::fx::FxHashMap::default(),
            no_sanitize_not_global: kiln_source::fx::FxHashMap::default(),
            attr_spellings: kiln_source::fx::FxHashMap::default(),
            traits: templ::builtins::TraitState::default(),
            exceptions: true,
            exc_disabled_seen: kiln_source::fx::FxHashSet::default(),
            exc_disabled_pattern: kiln_source::fx::FxHashSet::default(),
            rtti: true,
            reflection: false,
            consteval_only_records: kiln_source::fx::FxHashMap::default(),
            consteval_only_pattern: kiln_source::fx::FxHashSet::default(),
            subst_expr_depth: 0,
            subst_template_id: false,
            coro_aligned_allocation: false,
            lax_vector_conversions: LaxVectorConversions::All,
            range_lifetime_extension: false,
            gnu: false,
            clang_compat: 22,
            cf_protection_branch: false,
            sve: false,
            sve_vector_bits: None,
            nns_start: None,
            gnu_lib_builtins: false,
            no_builtin: false,
            sized_deallocation: true,
            char8: None,
            no_builtin_fns: Vec::new(),
            lib_builtin_name: |n, _| kiln_ast::decl::builtin::lib_by_name(n).is_some(),
            no_builtin_pending: Vec::new(),
            math_errno: true,
            cfg_dump: false,
            cfg_stats: false,
            full_deduction: false,
            full_redecl_scan: false,
            redecl_index: overload::redecl_index::RedeclIndex::default(),
            explicit_memo: templ::explicit_memo::ExplicitMemo::default(),
            permissive: false,
            cfg_out: String::new(),
            cfg_max_elems: cfg::DEFAULT_MAX_ELEMS,
            cfg_body: cfg::BodyCfg::default(),
            uninit_scratch: uninit::Scratch::default(),
            inferred_noreturn: kiln_source::fx::FxHashSet::default(),
            gsl_inferred_pointers: kiln_source::fx::FxHashSet::default(),
            in_typedef_instantiation: false,
            pack: (0, Vec::new()),
            ms_struct_pragma: false,
            fp_pragma: (kiln_ast::fp::FpOverride::NONE, Vec::new()),
            scopes: vec![scope::Scope {
                first_decl: 0,
                ..scope::Scope::new(ScopeKind::TranslationUnit, Some(tu))
            }],
            ctx: tu,
            func: None,
            unevaluated: 0,
            address_operand: false,
            volatile_assigns: Vec::new(),
            decltype_calls: Vec::new(),
            agg_list_end: None,
            agg_error_mark: None,
            agg_excess: std::cell::Cell::new(false),
            immediate: 0,
            constant_full_expr: 0,
            discarding_init: false,
            std: kiln_lex::Std::Cxx26,
            pending_odr: Vec::new(),
            potential_caps: Vec::new(),
            misaligned: Vec::new(),
            imm: consteval::ImmState::default(),
            defining: Vec::new(),
            pattern_member_access: false,
            invalid_defining: kiln_source::fx::FxHashSet::default(),
            enums: Vec::new(),
            local_externs: Vec::new(),
            dropped_qualified_locals: kiln_source::fx::FxHashSet::default(),
            vis_stack: Vec::new(),
            vis_namespaces: Vec::new(),
            weak_pending: Vec::new(),
            missing_typename_fns: kiln_source::fx::FxHashSet::default(),
            extern_c: kiln_source::fx::FxHashMap::default(),
            split_greaters: kiln_source::fx::FxHashSet::default(),
            oven: Some(Box::new(Oven::new(Budget::default()))),
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
            inline_fn_begin_locs: kiln_source::fx::FxHashMap::default(),
            ref_locs: kiln_source::fx::FxHashMap::default(),
            inner_chunk_locs: kiln_source::fx::FxHashMap::default(),
            pending_chunk_loc: Loc::INVALID,
            constexpr_begin_locs: kiln_source::fx::FxHashMap::default(),
            friend_qualifiers: kiln_source::fx::FxHashMap::default(),
            virt_spec_locs: kiln_source::fx::FxHashMap::default(),
            instantiating_friend_template: false,
            delayed_bodies: kiln_source::fx::FxHashSet::default(),
            friend_cmp_constraints: kiln_source::fx::FxHashMap::default(),
            friend_template_apart: false,
            constinit_locs: kiln_source::fx::FxHashMap::default(),
            ref_usable: kiln_source::fx::FxHashMap::default(),
            auto_referent: kiln_source::fx::FxHashMap::default(),
            empty_loop_bodies: kiln_source::fx::FxHashMap::default(),
            group_placeholders: kiln_source::fx::FxHashMap::default(),
            deferring_call_ref: false,
            declarator_int_recovery: false,
            declarator_field: false,
            pending_alias_vla: None,
            reverse_using_shadows: false,
            va_list_type: None,
            ext_vector_typedefs: Vec::new(),
            splice_fast: 0,
            typeid_operands: Vec::new(),
            meta_subst: meta::SubstMemo::default(),
            spare_oven: None,
            ovl: overload::OverloadScratch::default(),
            trailing_notes: std::cell::RefCell::new(Vec::new()),
            designator_rewarn: kiln_source::fx::FxHashMap::default(),
            null_deref_rewarn: kiln_source::fx::FxHashMap::default(),
            null_compare_rewarn: kiln_source::fx::FxHashMap::default(),
            vararg_recheck: kiln_source::fx::FxHashMap::default(),
            any_diagnose_if: false,
            diagnose_if_recheck: kiln_source::fx::FxHashMap::default(),
            null_deref_seen: kiln_source::fx::FxHashSet::default(),
            designator_pending: None,
            deferred_call_refs: Vec::new(),
            classes: Vec::new(),
            suspended_classes: Vec::new(),
            implicit_declared: kiln_source::fx::FxHashMap::default(),
            special: class::SpecialState::default(),
            delayed_access: Vec::new(),
            delayed_access_depth: Vec::new(),
            delayed_deprecations: Vec::new(),
            depr_points: Vec::new(),
            depr_init: None,
            exempt_next_delay: false,
            explicit_conv: false,
            cast_ignores_base_access: false,
            dependent_access: kiln_source::fx::FxHashMap::default(),
            speculative_friend: std::cell::Cell::new(false),
            std_meta_ns: std::cell::Cell::new(None),
            access_failed: kiln_source::fx::FxHashSet::default(),
            replaying_access: false,
            init_entity: None,
            arith_warning: None,
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
            vtable_finals: kiln_source::fx::FxHashMap::default(),
            module_vtables: kiln_source::fx::FxHashSet::default(),
            synth_noted: std::cell::Cell::new(0),
            walk_stack: std::cell::RefCell::new(Vec::new()),
            conv_walk_stack: Vec::new(),
            equiv_scratch: std::cell::RefCell::default(),
            cand_scratch: Vec::new(),
            deduce_scratch: Vec::new(),
            delegating: Vec::new(),
            undefined_used: Vec::new(),
            undefined_used_seen: kiln_source::fx::FxHashSet::default(),
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
            open_specs: 0,
            spec_failures: Vec::new(),
            spec_exempt: kiln_source::fx::FxHashSet::default(),
            range_for_outer_depth: None,
            cond_typo_at: Loc::INVALID,
            pending_cond_typo: Loc::INVALID,
            expr_errors: kiln_source::fx::FxHashSet::default(),
            delete_cxx26: true,
            unparsed_exc: kiln_source::fx::FxHashSet::default(),
            delayed_exc_equiv: Vec::new(),
            unparsed_default_args: kiln_source::fx::FxHashMap::default(),
            uninstantiated_exc: kiln_source::fx::FxHashMap::default(),
            uninstantiated_annots: kiln_source::fx::FxHashMap::default(),
            base_annotation_packs: false,
            annot_of_attr: kiln_source::fx::FxHashMap::default(),
            fn_saved: Vec::new(),
            member_refs: kiln_source::fx::FxHashMap::default(),
            friend_tag: None,
            extraneous_tag_head: None,
            declarator_type_invalid: false,
            lambda_packs: kiln_source::fx::FxHashMap::default(),
            declspec_restrict_error: None,
            friend_class_template: None,
            inherited_defaults: kiln_source::fx::FxHashMap::default(),
            inherited_aligned: kiln_source::fx::FxHashMap::default(),
            inh_ctors: kiln_source::fx::FxHashMap::default(),
            inh_ctor_base: kiln_source::fx::FxHashMap::default(),
            anon_namespaces: kiln_source::fx::FxHashMap::default(),
            friend_specs: kiln_source::fx::FxHashMap::default(),
            nsdmi_this: Vec::new(),
            contract_ctx: Vec::new(),
            has_contracts: false,
            post_param_diagnosed: kiln_source::fx::FxHashSet::default(),
            post_param_pattern_locs: kiln_source::fx::FxHashSet::default(),
            friend_pattern_prev: kiln_source::fx::FxHashMap::default(),
            initializing: None,
            inst_enable_if: None,
            templ: templ::TemplState::default(),
            inst: templ::InstState::default(),
            abstract_noted: kiln_source::fx::FxHashSet::default(),
            srcloc: builtins::SrcLocCache::default(),
            meta_names: meta::names::NameCache::default(),
            meta_members: meta::member::MemberState::default(),
            lambdas: Vec::new(),
            lambda_base: 0,
            lambda_mangle: lambda::ManglingState::default(),
            size_t_sugar: size_t,
            signed_size_t_sugar: ptrdiff_t,
            ptrdiff_t_sugar: ptrdiff_t,
            modules: modules::ModSema::default(),
            declspec_nulls: Vec::new(),
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
                if self.inst.suppress_access.get() && is_access_error(id) {
                    if let Some(s) = self.inst.sfinae.borrow_mut().last_mut()
                        && s.first.is_none()
                    {
                        s.first = Some(d);
                    }
                    return;
                }
                // A warning that is an error only through `-Werror` is
                // still a warning here (clang's SFINAE response follows
                // the diagnostic's class, not its mapped severity).
                let failure = (sev >= kiln_diag::Severity::Error
                    && (id.class() != kiln_diag::DiagClass::Warning || id.is_default_error()))
                    || is_sfinae_failure(id);
                if failure && let Some(s) = self.inst.sfinae.borrow_mut().last_mut() {
                    s.errors += 1;
                    if s.first.is_none() {
                        s.first = Some(d);
                    }
                } else if !failure && self.diags.would_emit(&d) {
                    // A warning waits for the use of what the substitution
                    // forms (§5.7); clang 22 keeps the context notes with it
                    // (clang 19 none).
                    if self.clang_compat >= 22 {
                        d.prepend_notes(self.inst_context_notes());
                    }
                    if let Some(s) = self.inst.sfinae.borrow_mut().last_mut() {
                        s.warnings.push(d);
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
            let held = self.take_held_typos(mark);
            self.emit_typos(held, false);
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
        let held = self.take_held_typos(mark);
        self.emit_typos(held, false);
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
        self.emit_typos(held, true);
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
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        if found.is_empty() {
            return false;
        }
        let (now, keep): (Vec<HeldTypo>, Vec<HeldTypo>) = std::mem::take(&mut self.delayed_typos)
            .into_iter()
            .partition(|t| found.contains(&t.expr));
        self.delayed_typos = keep;
        self.emit_typos(now, false);
        true
    }

    /// Reports the delayed typos `batch` at their correction point, in
    /// order (into [`Sema::emit_dropped_typos`]'s list if `dropped`).
    /// The speculative corrections among them ([`TypoSpec`]) stand
    /// together or not at all, as clang's `CorrectDelayedTyposInExpr`
    /// rebuilds the whole expression: one that an error came of
    /// ([`Sema::spec_failed_errors`]) and has no `fallback`, or one that
    /// succeeded but is `ambiguous`, makes them all report the typo
    /// uncorrected; a failed one with a `fallback` reports that instead.
    /// The errors are dropped either way (clang never built the failing
    /// expression), and an uncorrected typo's expression becomes an
    /// expression error ([`Sema::holds_uncorrected_typo`]). A dropped
    /// expression is never corrected.
    fn emit_typos(&mut self, batch: Vec<HeldTypo>, dropped: bool) {
        let specs = batch.iter().filter(|t| t.spec.is_some()).count();
        // Per speculation: whether an error came of it.
        let mut failures = Vec::with_capacity(batch.len());
        if specs > 0 {
            for t in &batch {
                let errors = match &t.spec {
                    Some(s) => self.spec_failed_errors(s.loc, s.start),
                    None => Vec::new(),
                };
                failures.push(!errors.is_empty());
                for i in errors {
                    self.diags.drop_held(i);
                }
            }
        } else {
            failures.resize(batch.len(), false);
        }
        // The speculations stand together: one failing with nowhere to
        // go, or one valid but ambiguous, leaves the whole batch
        // uncorrected.
        let failed = dropped
            || batch.iter().zip(&failures).any(|(t, &f)| {
                t.spec.as_ref().is_some_and(|s| if f { s.fallback.is_none() } else { s.ambiguous })
            });
        let mut after = Vec::new();
        for (t, f) in batch.into_iter().zip(failures) {
            if failed && t.spec.is_some() {
                // clang's uncorrected `TypoExpr` is an `ExprError`.
                self.expr_errors.insert(t.expr);
            }
            let d = match t.spec {
                Some(s) if !failed && !f => {
                    after.extend(s.after);
                    s.suggest
                }
                Some(s) if !failed => s.fallback.unwrap_or(t.diag),
                _ => t.diag,
            };
            if dropped {
                self.dropped_typos.push(d);
            } else {
                if self.open_specs > 0 {
                    self.spec_exempt.insert(self.diags.held_len());
                }
                self.emit(d);
            }
        }
        after.sort_by_key(|&(n, _)| n);
        for (_, d) in after {
            self.emit(d);
        }
        for _ in 0..specs {
            self.open_specs = self.open_specs.saturating_sub(1);
            self.diags.release();
        }
        if self.open_specs == 0 {
            self.spec_failures.clear();
            self.spec_exempt.clear();
        }
    }

    /// The held errors issued since `start` that come of an expression
    /// built on the speculative correction at `loc`: not the parser's,
    /// not a typo's, and either the failure of a recovery expression over
    /// the correction (an expression at `loc`: the reference, or what it
    /// was converted to) or of no recovery expression at all (a failed
    /// initialization).
    fn spec_failed_errors(&self, loc: Loc, start: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for (i, id) in self.diags.held_errors_from(start) {
            if id.name().starts_with("parse_") || self.spec_exempt.contains(&i) {
                continue;
            }
            let node = self.spec_failures.iter().find(|&&(_, n)| n > i).map(|&(e, _)| e);
            if node.is_none_or(|n| self.expr_contains_at(n, loc)) {
                out.push(i);
            }
        }
        out
    }

    /// Whether `e` holds a delayed typo (clang's `TypoExpr`, which is
    /// type-dependent: clang 19 diagnoses an unused result before it
    /// corrects the typos, so a corrected `val<int>;` is not "unused").
    pub(crate) fn holds_typo_expr(&self, e: ExprId) -> bool {
        if self.typo_exprs.is_empty() {
            return false;
        }
        let mut stack = vec![e];
        let mut budget = 1usize << 16;
        while let Some(x) = stack.pop() {
            if self.typo_exprs.contains(&x) {
                return true;
            }
            if budget == 0 {
                return false;
            }
            budget -= 1;
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        false
    }

    /// Whether `e` or an expression under it is at `loc` (a bounded walk).
    fn expr_contains_at(&self, e: ExprId, loc: Loc) -> bool {
        let mut stack = vec![e];
        let mut budget = 1usize << 16;
        while let Some(x) = stack.pop() {
            if self.ast.exprs.get(x).loc() == loc {
                return true;
            }
            if budget == 0 {
                return false;
            }
            budget -= 1;
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        false
    }

    /// Opens the speculative correction `u` of a delayed typo whose
    /// uncorrected diagnostic is `plain` and corrected one `suggest`
    /// ([`TypoSpec`]): the engine holds every diagnostic until its
    /// correction point.
    pub(crate) fn hold_typo_spec(
        &mut self,
        plain: Diag,
        suggest: Diag,
        u: ExprId,
        loc: Loc,
        fallback: Option<Diag>,
        ambiguous: bool,
    ) {
        let seq = self.typo_count;
        self.typo_count = self.typo_count.wrapping_add(1);
        self.typo_exprs.insert(u);
        if self.open_specs == 0 {
            self.spec_failures.clear();
            self.spec_exempt.clear();
        }
        self.open_specs += 1;
        self.diags.hold();
        let start = self.diags.held_len();
        let spec = Some(Box::new(TypoSpec {
            start,
            loc,
            suggest,
            fallback,
            ambiguous,
            tie: None,
            after: Vec::new(),
        }));
        self.delayed_typos.push(HeldTypo { seq, expr: u, diag: plain, spec });
    }

    /// Records the tied value of the speculation [`Sema::hold_typo_spec`]
    /// just opened at `loc` ([`TypoSpec::tie`]).
    pub(crate) fn set_typo_tie(
        &mut self,
        loc: Loc,
        tie: Option<(QualType, kiln_ast::expr::ValueCategory)>,
    ) {
        if let Some(s) = self.delayed_typos.last_mut().and_then(|h| h.spec.as_mut())
            && s.loc == loc
        {
            s.tie = tie;
        }
    }

    /// clang 19 rebuilds an initializer `= e` once per candidate of a
    /// speculative correction (`TransformTypos`): when `e` (parentheses
    /// aside) is a function speculation tied with a value
    /// ([`TypoSpec::tie`]) and no implicit conversion takes the value to
    /// `to`, the function is the one valid candidate and the correction
    /// is no longer ambiguous (`int (*p)(int) = valc<int>;` with
    /// `int vala; int valb(int);` is `valb`).
    pub(crate) fn settle_typo_tie(&mut self, e: ExprId, to: QualType) {
        if self.open_specs == 0 {
            return;
        }
        let at = self.expr_loc(self.ignore_parens(e));
        let Some(i) = self.delayed_typos.iter().rposition(|h| {
            h.spec.as_ref().is_some_and(|s| s.loc == at && s.ambiguous && s.tie.is_some())
        }) else {
            return;
        };
        let Some((ty, vk)) = self.delayed_typos[i].spec.as_ref().and_then(|s| s.tie) else {
            return;
        };
        let arg = overload::ArgInfo::of_type(ty, vk);
        let ics = self.try_implicit_conversion(&arg, to, overload::IcsOptions::NONE);
        if ics.is_bad()
            && let Some(s) = self.delayed_typos[i].spec.as_mut()
        {
            s.ambiguous = false;
        }
    }

    /// Called before the compat-19 correction scope of condition `e`
    /// ends: whether `e` (parentheses aside) is a speculative typo
    /// correction, which the next [`Sema::act_on_condition`] then does
    /// not check for `-Wpointer-bool-conversion` (clang 19 converts the
    /// `TypoExpr`, not the corrected expression).
    pub fn note_condition_typo(&mut self, e: ExprId) {
        let at = self.expr_loc(self.ignore_parens(e));
        let open = at.is_valid()
            && self.delayed_typos.iter().any(|h| h.spec.as_ref().is_some_and(|s| s.loc == at));
        self.pending_cond_typo = if open { at } else { Loc::INVALID };
    }

    /// Whether `e` stands for clang's `ExprError` rather than a
    /// `RecoveryExpr`: the name it refers to was not found (an undeclared
    /// identifier, `S::nom`), so clang's parser sees an invalid
    /// expression (WP-408).
    pub fn is_expr_error(&self, e: ExprId) -> bool {
        self.expr_errors.contains(&e)
    }

    /// Whether `e` holds a speculative typo correction that did not
    /// stand ([`TypoSpec`]): clang's full-expression is an `ExprError`.
    pub fn holds_uncorrected_typo(&self, e: ExprId) -> bool {
        if self.expr_errors.is_empty() || self.typo_exprs.is_empty() {
            return false;
        }
        let mut stack = vec![e];
        let mut budget = 1usize << 16;
        while let Some(x) = stack.pop() {
            if self.expr_errors.contains(&x) && self.typo_exprs.contains(&x) {
                return true;
            }
            if budget == 0 {
                return false;
            }
            budget -= 1;
            self.ast.exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        false
    }

    /// Records `e` as an `ExprError` ([`Sema::is_expr_error`]).
    pub(crate) fn mark_expr_error(&mut self, e: ExprId) -> ExprId {
        self.expr_errors.insert(e);
        e
    }

    /// Takes [`Sema::note_condition_typo`]'s mark if it is for `at`.
    pub(crate) fn take_condition_typo(&mut self, at: Loc) -> bool {
        std::mem::replace(&mut self.pending_cond_typo, Loc::INVALID) == at && at.is_valid()
    }

    /// Emits the full-expression warning `d` about `e`, or, if `e` is
    /// built on an open speculative correction ([`TypoSpec`]), keeps it
    /// for after the correction's diagnostic (dropped if the correction
    /// fails): clang 19 analyzes the full-expression once it has
    /// corrected its `TypoExpr`s.
    pub(crate) fn emit_after_typos(&mut self, e: ExprId, d: Diag) {
        if self.open_specs > 0 {
            let at = (0..self.delayed_typos.len()).rev().find(|&i| {
                self.delayed_typos[i].spec.as_ref().is_some_and(|s| self.expr_contains_at(e, s.loc))
            });
            if let Some(i) = at {
                let n = self.typo_count;
                self.typo_count = self.typo_count.wrapping_add(1);
                if let Some(s) = self.delayed_typos[i].spec.as_mut() {
                    s.after.push((n, d));
                }
                return;
            }
        }
        self.emit(d);
    }

    /// Records the recovery expression `e` just built while a
    /// speculative correction is open ([`Sema::spec_failed_errors`]).
    pub(crate) fn note_spec_recovery(&mut self, e: ExprId) {
        if self.open_specs > 0 {
            let n = self.diags.held_len();
            self.spec_failures.push((e, n));
        }
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
        // A speculative correction left open (no correction point
        // reached) reports uncorrected.
        let (open, keep): (Vec<HeldTypo>, Vec<HeldTypo>) =
            std::mem::take(&mut self.delayed_typos).into_iter().partition(|t| t.spec.is_some());
        self.delayed_typos = keep;
        self.emit_typos(open, true);
        while self.open_specs > 0 {
            self.open_specs -= 1;
            self.diags.release();
        }
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
            self.delayed_typos.push(HeldTypo { seq, expr: e, diag: d, spec: None });
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
        if let Some(v) = vector_diag_arg(&self.ast.types, &names, q, self.clang_compat >= 22) {
            return v;
        }
        let s = print_type(&self.ast.types, &names, q, PrintOptions::SUGARED);
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
/// An access-control error ([class.access]) that clang's
/// `SuppressAccessChecks` discards.
fn is_access_error(id: DiagId) -> bool {
    matches!(
        id,
        DiagId::sema_access_base_conv
            | DiagId::sema_access_base_ctor
            | DiagId::sema_access_ctor
            | DiagId::sema_access_dtor
            | DiagId::sema_access_field_ctor
            | DiagId::sema_access_member
    )
}

/// clang's `SFINAEFailure` warnings: a substitution failure whatever the
/// mapped severity, even under `-Wno-…` or `#pragma … ignored` (narrowing:
/// [dcl.init.list]/3, [temp.deduct.general]/8).
fn is_sfinae_failure(id: DiagId) -> bool {
    matches!(
        id,
        DiagId::sema_cce_narrowing_value
            | DiagId::sema_designated_init_reordered
            | DiagId::sema_init_list_constant_narrowing
            | DiagId::sema_init_list_type_narrowing
            | DiagId::sema_init_list_variable_narrowing
            | DiagId::sema_initializer_overrides
            | DiagId::sema_initializer_union_overrides
    )
}
