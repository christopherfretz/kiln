//! The Oven: constant evaluator (bytecode compiler, VM, abstract memory model, `std::meta` runtime).
//!
//! See `docs/ARCHITECTURE.md` and `spec/oven.md` for the contract
//! this crate implements. M2 covers the C-like subset: integers of every
//! width, floating types, enums, pointers, arrays, structs and unions
//! (with bit-fields), all C control flow, calls and recursion.
//!
//! The pipeline: [`Oven::evaluate`] compiles the expression to a KBC thunk
//! (`compile`), the functions it calls are compiled lazily on first call,
//! the VM (`vm`) runs it against the abstract memory (`mem`), and the
//! result is frozen into the AST's `ConstStore` (`value`). Everything the
//! Oven builds (types, code, thawed constants, the memo cache) is kept for
//! the translation unit; the frame memory is reset after each evaluation.
#![forbid(unsafe_code)]
#![allow(
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::unreadable_literal,
    clippy::match_same_arms,
    clippy::cast_precision_loss,
    clippy::cast_lossless,
    clippy::needless_pass_by_value,
    clippy::items_after_statements,
    clippy::wildcard_imports,
    clippy::enum_glob_use,
    clippy::float_cmp,
    clippy::single_match_else,
    clippy::unused_self,
    reason = "an interpreter: wide matches over opcodes and kinds, bit-level arithmetic"
)]

pub mod apfloat;
mod arith;
mod bigint;
mod compile;
mod complex;
mod failure;
mod frame;
mod hooks;
mod kbc;
mod leaf;
mod mem;
mod memo;
mod memptr;
mod meta;
mod notes;
pub mod profile;
mod reclaim;
pub mod report;
mod softfloat;
mod stats;
#[cfg(feature = "op-stats")]
pub mod trial_stats;
mod ty;
mod value;
mod vm;

use std::time::Instant;

pub use kiln_ast::class::{Subobject, SubobjectId};
use kiln_ast::consts::ConstStore;
use kiln_ast::types::RecordLayout;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType, StmtId};
use kiln_diag::Diag;
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

pub use failure::{EvalFailure, FailKind, Note, Overflow};
pub use frame::{FrameEval, FrameObject};
pub use meta::materialize::{ExcCtor, MetaThrow, Shape};
pub use meta::member_cache::{
    CachedList, MemberCache, MemberEntry, MemberList, filter_members, member_kind,
};
pub use meta::{
    AccessOp, AnnotOp, DefineOp, Dms, ExtOp, MAX_META_ARGS, MemberOp, MetaCtx, MetaError,
    MetaErrorKind, MetaOp, MetaOut, OptionsPlan, OvenFault, Purity, QueryOp, RawOptions, TemplOp,
    TraitOp, ValueOp, info_arg,
};
pub use notes::FrameMark;
pub use stats::{EvalRecord, EvalStats, EvalTotals, JobTotals};

/// Whether this build checks every trial-fold walk against the full path
/// (the `leaf-check` feature; then the walk saves no thunk compile).
#[doc(hidden)]
pub const LEAF_CHECK: bool = cfg!(feature = "leaf-check");
pub use value::automatic_referent;

use compile::{Func, Global, Site, TrapInfo};
use mem::Memory;
use memo::Memo;
use ty::Types;
use vm::Frame;

/// Resource limits of one top-level evaluation (spec/oven.md "Limits").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// Statements executed (clang's `-fconstexpr-steps`, default 1048576).
    pub steps: u64,
    /// Nested calls (clang's `-fconstexpr-depth`, default 512).
    pub depth: u32,
    /// Live abstract-memory bytes (`-fconstexpr-memory`, a kiln extension).
    pub bytes: u64,
    /// Entries of the memoization cache (0 disables it).
    pub memo_entries: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Budget { steps: 1_048_576, depth: 512, bytes: 256 << 20, memo_entries: 4096 }
    }
}

/// Why an expression is being evaluated (the `kind` of a report line).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EvalKind {
    /// The condition of a `static_assert`.
    StaticAssert,
    /// The initializer of a `constexpr`/`constinit` variable or a constant
    /// initialization.
    Init,
    /// A template argument.
    TemplateArg,
    /// An immediate invocation.
    Consteval,
    /// The condition of an `if constexpr`.
    IfConstexpr,
    /// An atomic constraint of a concept or `requires` clause
    /// ([temp.constr.atomic]/3).
    Constraint,
    /// Array bounds, enumerator values, `case` labels, bit-field widths, ...
    Other,
    /// The evaluation of a consteval block ([dcl.pre], P3289R1): the one
    /// plainly constant-evaluated context (spec/consteval-blocks.md §3).
    ConstevalBlock,
}

impl EvalKind {
    /// The report spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            EvalKind::StaticAssert => "static_assert",
            EvalKind::Init => "init",
            EvalKind::TemplateArg => "template_arg",
            EvalKind::Consteval => "consteval",
            EvalKind::IfConstexpr => "if_constexpr",
            EvalKind::Constraint => "constraint",
            EvalKind::Other => "other",
            EvalKind::ConstevalBlock => "consteval_block",
        }
    }
}

/// The context of an evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvalContext {
    /// What is evaluated (for the report).
    pub kind: EvalKind,
    /// The construct's location (for the report).
    pub loc: Loc,
    /// The variable whose initializer this is, if any: names the result
    /// object in notes and guards against self-reference.
    pub object: Option<DeclId>,
    /// Whether a cast of an out-of-range value to an unscoped enumeration
    /// without a fixed underlying type is reported (as the
    /// `-Wenum-constexpr-conversion` error, through
    /// [`SemaBridge::diag`]; the evaluation goes on with the value).
    /// clang 19 checks only where it evaluates a constant expression
    /// proper: a `constexpr` variable's initializer, a converted constant
    /// expression (`case` labels, enumerators of a fixed-type enumeration,
    /// template arguments), immediate invocations and `if constexpr`
    /// conditions — not integral constant expressions (`static_assert`,
    /// array bounds, bit-field widths) or other initializers.
    pub enum_range_check: bool,
    /// Whether the evaluation is plainly constant-evaluated: the
    /// corresponding expression of a consteval-block-declaration
    /// ([expr.const.core]/2.32, P3289R1), the only context in which an
    /// injected declaration may be produced (spec/consteval-blocks.md §3).
    /// Not inherited by nested evaluations ([`Oven::plain`]).
    pub plain: bool,
}

impl EvalContext {
    /// A context of `kind` at `loc` (enumeration ranges are checked for
    /// template arguments, immediate invocations, `if constexpr` and
    /// atomic constraints).
    pub fn new(kind: EvalKind, loc: Loc) -> Self {
        let enum_range_check = matches!(
            kind,
            EvalKind::TemplateArg
                | EvalKind::Consteval
                | EvalKind::IfConstexpr
                | EvalKind::Constraint
        );
        EvalContext { kind, loc, object: None, enum_range_check, plain: false }
    }

    /// The initializer of the `constexpr` variable `var` (enumeration
    /// ranges are checked; clear [`EvalContext::enum_range_check`] for a
    /// `constinit` variable that is not `constexpr`).
    pub fn init(var: DeclId, loc: Loc) -> Self {
        EvalContext {
            kind: EvalKind::Init,
            loc,
            object: Some(var),
            enum_range_check: true,
            plain: false,
        }
    }

    /// A converted constant expression ([expr.const]/12) that is not a
    /// template argument (a `case` label, an enumerator of an enumeration
    /// with a fixed underlying type): reported as [`EvalKind::Other`],
    /// with enumeration ranges checked.
    pub fn converted(loc: Loc) -> Self {
        EvalContext {
            kind: EvalKind::Other,
            loc,
            object: None,
            enum_range_check: true,
            plain: false,
        }
    }

    /// The evaluation of the consteval block at `loc`: plainly
    /// constant-evaluated ([`EvalContext::plain`]).
    pub fn consteval_block(loc: Loc) -> Self {
        EvalContext {
            kind: EvalKind::ConstevalBlock,
            loc,
            object: None,
            enum_range_check: true,
            plain: true,
        }
    }

    /// `self` with [`EvalContext::enum_range_check`] set to `on`.
    #[must_use]
    pub fn with_enum_range_check(mut self, on: bool) -> Self {
        self.enum_range_check = on;
        self
    }
}

/// What the Oven needs from sema. The Oven never holds a borrow of the
/// AST across calls: sema may mutate it (instantiation on demand,
/// interning constants) between and during evaluations.
pub trait SemaBridge {
    /// The AST (re-borrowed per use, never held).
    fn ast(&self) -> &Ast;
    /// The translation unit's sources, for the `F:L:C` of an unnamed
    /// class a note names (`(anonymous union at F:L:C)`, WP-773). The
    /// default has none, and the location is left out.
    fn sources(&self) -> Option<&kiln_source::SourceManager> {
        None
    }
    /// The body of function `f`'s definition, instantiating it if needed;
    /// for an implicit or defaulted special member this defines it on
    /// demand (sema's `define_implicit_*`, spec/sema-class.md §4.5).
    /// Called while an evaluation runs (the first call of `f`); sema may
    /// evaluate nested constant expressions with `oven` meanwhile
    /// (R-OVEN-1: spec/oven.md "Nested evaluation").
    fn ensure_body(&mut self, f: DeclId, oven: &mut Oven) -> Option<StmtId>;
    /// The translation unit's language standard as its year (1998, 2003,
    /// 2011, ..., 2026): the rules that depend on the language mode
    /// (P2747 placement new outside namespace std in C++26; P2280
    /// unknown references from C++23). The default is C++26.
    fn cxx_year(&self) -> u32 {
        2026
    }
    /// Whether function `f` is non-throwing ([except.spec]/2), resolving
    /// its exception specification if needed (which may evaluate nested
    /// constant expressions with `oven`). An exception leaving it ends a
    /// constant evaluation (P3068). The default is potentially-throwing.
    fn is_nothrow(&mut self, f: DeclId, oven: &mut Oven) -> bool {
        let _ = (f, oven);
        false
    }
    /// Whether the translation unit is C (WP-1311: a C library builtin's
    /// call is then evaluated without the C++11 "non-constexpr function"
    /// note, clang `VisitBuiltinCallExpr`'s `CPlusPlus11` test).
    fn is_c(&self) -> bool {
        false
    }
    /// Whether the translation unit is C++26 or later.
    fn cxx26(&self) -> bool {
        self.cxx_year() >= 2026
    }
    /// Whether the translation unit is C++23 or later.
    fn cxx23(&self) -> bool {
        self.cxx_year() >= 2023
    }
    /// The layout of a complete record (via `kiln_abi`); `None` while the
    /// class is incomplete (no definition, or still being defined).
    fn ensure_layout(&mut self, record: DeclId) -> Option<&RecordLayout>;
    /// `(size, align)` in bytes of a type the Oven has no value model
    /// for (a vector type: spec/oven.md "Interface"), via `kiln_abi`, so
    /// `sizeof`/`alignof` agree with layout. The default is unknown
    /// (`(0, 1)`).
    fn opaque_size_align(&mut self, ty: QualType) -> (u64, u32) {
        let _ = ty;
        (0, 1)
    }
    /// The alignment in bytes of variable `d`'s object (clang
    /// `ASTContext::getDeclAlign`: its `alignas`/`aligned`, else its
    /// type's, raised by the target's large-array rule), for the
    /// `__builtin_assume_aligned` check. The default 0 means the type's
    /// alignment.
    fn decl_align(&mut self, d: DeclId) -> u64 {
        let _ = d;
        0
    }
    /// `alignof` of an expression naming declaration `d` (a variable or
    /// data member: [expr.alignof] as clang's `GetAlignOfExpr`, which is
    /// `ASTContext::getDeclAlign(d, /*ForAlignof=*/true)`, so `alignas`
    /// on the declaration counts). The default 0 means the type's
    /// alignment.
    fn alignof_decl(&mut self, d: DeclId) -> u64 {
        let _ = d;
        0
    }
    /// The final overrider of virtual function `method` called on the
    /// subobject `at` of an object whose most-derived class is
    /// `most_derived`, with the subobject the overrider is a member of
    /// ([class.virtual]/2; spec/sema-class.md §3.5). `None` when there is
    /// no unique final overrider (diagnosed by sema at the class's `}`).
    fn final_overrider(
        &mut self,
        most_derived: DeclId,
        at: SubobjectId,
        method: DeclId,
    ) -> Option<(DeclId, SubobjectId)>;
    /// The base-class subobject graph of complete class `record`, indexed
    /// by [`SubobjectId`] (spec/sema-class.md §3.5): the currency of the
    /// Oven's base-class pointer paths.
    fn subobjects(&mut self, record: DeclId) -> &[Subobject];
    /// The constant store results are frozen into.
    fn consts(&mut self) -> &mut ConstStore;
    /// Emits a diagnostic (unused by M2 evaluation, which returns notes).
    fn diag(&mut self, d: Diag);
    /// Records the constant value of lifetime-extended temporary `t` (a
    /// `LifetimeExtendedTemporary` with static storage duration, frozen
    /// with its extending variable's initializer: `LetData.value`,
    /// write-once — a second write of the same temporary is ignored). A
    /// bridge without a mutable AST (tests) may ignore it: reads of the
    /// temporary from later evaluations then see an external object.
    fn set_temporary_value(&mut self, t: DeclId, v: ConstValueId) {
        let _ = (t, v);
    }
    /// The type `t&` (interned in the bridge's AST): the type of the
    /// address a glvalue immediate invocation's value is frozen as
    /// ([`Oven::evaluate`], WP-640). A bridge without a mutable AST
    /// (tests) may return `None`: such an invocation is then frozen as
    /// the value it designates.
    fn lvalue_ref_to(&mut self, t: QualType) -> Option<QualType> {
        let _ = t;
        None
    }

    // Reflection (`-freflection`; spec/reflection.md §3). The defaults (a
    // bridge without reflection) recognize nothing.

    /// The reflection intrinsic `f` is, if any (consulted when a direct
    /// call of `f` is compiled).
    fn meta_intrinsic(&self, f: DeclId) -> Option<MetaOp> {
        let _ = f;
        None
    }

    /// Runs intrinsic `op` on `args` (one value per declared parameter:
    /// an `info`'s [`kiln_ast::ReflectionId`] bits, a scalar's value, the address
    /// of anything else) and writes the result to `out`. `oven` is the
    /// running evaluator: it reads operands from its memory
    /// ([`Oven::meta_c_bytes`]) and is lent back to sema for anything
    /// that evaluates. A `Mutating` op is never called in a trial.
    ///
    /// # Errors
    /// The notes of a failure that makes the evaluation not a constant
    /// expression (none: a silent failure), an unimplemented op, or an
    /// evaluator fault.
    fn meta(
        &mut self,
        op: MetaOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let _ = (op, args, out, ctx, oven);
        Err(MetaError::silent())
    }

    /// The [`Shape`] the class-typed result (`param: None`) or parameter
    /// `param` (of class type `q`) of intrinsic `op` is materialized by
    /// (spec/reflection.md §3.5), consulted once when a call of `op` is
    /// compiled; `Ok(None)` passes a parameter by address.
    ///
    /// # Errors
    /// Why `q` cannot be the op's result or parameter (a note at the
    /// call site).
    fn meta_shape(
        &mut self,
        op: MetaOp,
        param: Option<usize>,
        q: QualType,
    ) -> Result<Option<Shape>, String> {
        let _ = (op, param, q);
        Ok(None)
    }

    /// The `std::meta::exception` constructor a failing intrinsic throws
    /// with (spec/reflection.md §8.2), when the class is declared;
    /// consulted when a call of an intrinsic is compiled.
    fn meta_exception(&mut self) -> Option<ExcCtor> {
        None
    }

    /// The exception intrinsic `callee` (op `op`), failing at `loc` with
    /// reason `what`, throws; `None` fails the evaluation instead (the
    /// reason as its note).
    fn meta_throw(
        &mut self,
        op: MetaOp,
        callee: DeclId,
        what: &str,
        loc: Loc,
    ) -> Option<MetaThrow> {
        let _ = (op, callee, what, loc);
        None
    }
}

/// What one evaluation context runs.
#[derive(Clone, Copy, Debug)]
enum Job {
    /// The value of an expression.
    Expr(ExprId),
    /// The characters of a `static_assert` message ([`Oven::evaluate_chars`]).
    Chars { size: ExprId, data: ExprId },
    /// A condition with arguments substituted for parameters
    /// ([`Oven::evaluate_with_params`]; the bindings in `subst_binds`):
    /// `at` locates it.
    Subst { cond: Option<ExprId>, at: ExprId },
    /// A call of function `f` without arguments at `at`
    /// ([`Oven::evaluate_call`]).
    Call { f: DeclId, at: Loc },
    /// The object size of pointer `p` for type `ty`
    /// ([`Oven::fold_object_size`]).
    ObjectSize { p: ExprId, ty: u32 },
    /// A full-expression of a `constexpr` function body in a
    /// potential-constant frame ([`Oven::frame_local`],
    /// [`Oven::evaluate_in_frame`]; the frame in `frame`).
    Frame(ExprId),
}

impl Job {
    /// The location of a depth-limit failure.
    fn loc(self, ast: &Ast) -> Loc {
        match self {
            Job::Expr(e)
            | Job::Chars { size: e, .. }
            | Job::Subst { at: e, .. }
            | Job::ObjectSize { p: e, .. }
            | Job::Frame(e) => ast.exprs.get(e).loc(),
            Job::Call { at, .. } => at,
        }
    }
}

/// The [`Oven::ctor_steps`] tag of a virtual-base stage (WP-465).
pub(crate) const CTOR_STEP_VBASE: u32 = 1 << 31;
/// The [`Oven::ctor_steps`] tag of an element of the member being
/// list-initialized: the low bits are the member's entry, [`CtorStep::off`]
/// and [`CtorStep::len`] the element's bytes (WP-465).
pub(crate) const CTOR_STEP_ELEM: u32 = 1 << 30;
/// With [`CTOR_STEP_ELEM`]: the element is of class type, its subobjects
/// are left to the frames above.
pub(crate) const CTOR_STEP_RECORD: u32 = 1 << 29;

/// One construction step of a constructor ([`Oven::ctor_steps`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct CtorStep {
    /// The first instruction of the step.
    pub(crate) pc: u32,
    /// The entry being initialized, with its `CTOR_STEP_*` tags.
    pub(crate) cur: u32,
    /// An element step's offset from `this`.
    pub(crate) off: u32,
    /// An element step's size.
    pub(crate) len: u32,
}

/// The constant evaluator of one translation unit.
#[derive(Debug)]
pub struct Oven {
    pub(crate) budget: Budget,
    /// Call frames shown in a failure's backtrace (0: all).
    pub(crate) backtrace_limit: usize,
    /// The clang release whose note wording failures follow
    /// (`-fclang-compat=`; [`Oven::set_clang_compat`]).
    pub(crate) clang_compat: u32,
    /// The translation unit's language standard as its year
    /// ([`Oven::set_cxx_year`]).
    cxx_year: u32,
    /// The next outermost or nested evaluation reports
    /// `-Wconstant-evaluated` though silent ([`Oven::warn_constant_evaluated`]).
    cev_warn_next: bool,
    /// The next outermost evaluation checks for undefined behaviour
    /// ([`Oven::fold_for_overflow`]).
    ub_check_next: bool,
    /// The next outermost evaluation ignores every contract assertion
    /// (`vm::RunState::contracts_off`).
    contracts_off_next: bool,
    /// The outermost evaluation met a contract violation
    /// ([basic.contract.eval]/7; [`Oven::check_contract_init`]).
    pub(crate) contract_violated: bool,
    /// The next outermost evaluation is [`Oven::evaluate_call`]'s or
    /// [`Oven::evaluate_potential`]'s, with the number of calls it may
    /// make (`vm::RunState::potential`).
    potential_next: Option<u32>,
    /// The next outermost or nested evaluation is plainly
    /// constant-evaluated ([`EvalContext::plain`], [`Oven::plain`]).
    plain_next: bool,
    /// The next evaluation is an immediate invocation
    /// ([`EvalKind::Consteval`]): a `void` result is a value
    /// (`value::eval_at`).
    pub(crate) void_next: bool,
    /// The next evaluation is a glvalue immediate invocation: its value
    /// is the address of the object or function it designates, as a
    /// reference's initializer (`value::eval_at`; WP-640).
    pub(crate) addr_next: bool,
    /// The next evaluation is [`Oven::evaluate_potential`]'s: its result
    /// is not checked (clang's `Expr::isPotentialConstantExpr` evaluates
    /// the body, never `CheckConstantExpression`), so a result that
    /// fails the check is quietly unknown (`value::eval_at`; WP-1163).
    pub(crate) unchecked_next: bool,
    pub(crate) types: Types,
    pub(crate) code: Vec<u32>,
    /// `code.len()` just after its trailing fetch-window padding, while
    /// that padding is the last thing in `code` (`kbc::pad`/`unpad`).
    pub(crate) padded: usize,
    pub(crate) sites: Vec<Site>,
    pub(crate) step_locs: Vec<Loc>,
    pub(crate) origins: Vec<mem::Origin>,
    pub(crate) traps: Vec<TrapInfo>,
    pub(crate) funcs: Vec<Func>,
    pub(crate) func_map: FxHashMap<DeclId, u32>,
    /// The base-object variants of constructors and destructors of
    /// classes with virtual bases ([`Oven::func_index_base`]).
    pub(crate) base_funcs: FxHashMap<DeclId, u32>,
    /// The construction progress of each compiled constructor (by
    /// function index) that initializes subobjects: `(pc, i)` pairs in
    /// code order, from `pc` on entries `..i` of the class's
    /// `fields` have begun their lifetime, entry `i` is being
    /// initialized (`u32::MAX`: all have begun). In a complete-object
    /// constructor, `CTOR_STEP_VBASE | j` first: virtual base `j` (entry
    /// `vbase_start + j`) is being initialized, the earlier ones have
    /// begun, no non-virtual entry has. Read only by
    /// `__builtin_is_within_lifetime` (spec/oven.md "Builtins"). While a
    /// member is list-initialized, `CTOR_STEP_ELEM` steps name the element
    /// being initialized: the bytes before it have begun, it and those
    /// after it have not, an aggregate holding it has.
    pub(crate) ctor_steps: FxHashMap<u32, Box<[CtorStep]>>,
    /// The variables and expressions (temporaries, `new` expressions)
    /// whose objects are volatile as a whole: the origins of the frame
    /// and heap allocations [`mem::flags::VOLATILE`] marks without a volatile
    /// member on the path (`Oven::volatile_path`, WP-1019).
    pub(crate) volatile_decls: FxHashSet<DeclId>,
    pub(crate) volatile_exprs: FxHashSet<ExprId>,
    /// Compiled top-level thunks, by expression, the object it
    /// initializes and whether it yields a glvalue's address
    /// ([`Oven::compile_thunk`]).
    pub(crate) thunks: FxHashMap<(ExprId, Option<DeclId>, bool), u32>,
    /// Thunks compiled inside a default argument use (the key's second
    /// half, [`Oven::fold_immediate_in`]): their source-location builtins
    /// take that use's values.
    pub(crate) thunks_at: FxHashMap<(ExprId, ExprId, bool), u32>,
    /// The default argument use the next thunk is compiled in.
    pub(crate) srcloc_root: Option<ExprId>,
    /// The thunk compiled last, for a failed trial fold to reclaim
    /// (`reclaim.rs`).
    pub(crate) last_thunk: Option<reclaim::LastThunk>,
    pub(crate) globals: Vec<Global>,
    /// The global bound to the running initializer's object under
    /// construction, with its state before (`Oven::bind_object`, WP-1100).
    pub(crate) bound_obj: Option<(u32, compile::GState)>,
    pub(crate) global_map: FxHashMap<DeclId, u32>,
    /// Globals resolved to an unknown value because they had no
    /// initializer yet: re-resolved by the next evaluation.
    pub(crate) provisional: Vec<u32>,
    /// The object each provisional global was last resolved to, with its
    /// type: reused while the variable still has no initializer, so a
    /// provisional global costs one static slot per translation unit, not
    /// one per evaluation (spec/oven.md "Unknown references").
    pub(crate) provisional_objs: FxHashMap<u32, (ty::OTy, u64)>,
    /// Floating arithmetic that yields a NaN or divides by zero goes on
    /// with the `APFloat` result ([`Oven::set_fold_float_ub`]).
    pub(crate) fold_float_ub: bool,
    /// A fold's pointer-to-integer cast of an initializer's whole value
    /// keeps the address ([`Oven::set_fold_addr_int`]).
    pub(crate) addr_int: bool,
    /// A fold may leave allocations undeallocated
    /// ([`Oven::set_fold_leaks`]).
    pub(crate) fold_leaks: bool,
    /// The fold is a C static initializer's, which clang evaluates in a
    /// constant context ([`Oven::set_fold_c_static`]).
    pub(crate) fold_c_static: bool,
    /// The displacement (`Op::AddrInt`) of each address an integer object
    /// holds by space and absolute byte, when not 0
    /// ([`crate::kbc::addr_int_disp`]; read by the freeze). Cleared by
    /// [`Oven::set_fold_c_static`].
    pub(crate) addr_int_disps: FxHashMap<(mem::Sp, u64), i64>,
    /// The addresses C static initializers compute that a pointer's
    /// encoding cannot hold, as a base pointer (0: an integer) and a
    /// byte displacement: `(void *)-1`, `(char *)f + 1`, `a + 128` past
    /// `a`'s end (C23 6.6p9, clang's `inttoptr` and `gep i8`). A pointer
    /// to one is [`mem::synth_ptr`]; only [`Oven::set_fold_c_static`]
    /// makes them.
    pub(crate) synth: std::cell::RefCell<Vec<(u64, i64)>>,
    /// The label address differences C static initializers compute
    /// (`&&a - &&b`), as the two `AddrLabelExpr`s; an integer holds one as
    /// the address [`mem::label_diff_ptr`] (`addr_int::LABEL_DIFF`).
    pub(crate) label_diffs: Vec<(ExprId, ExprId)>,
    /// The integer objects of the current C static initializer's fold
    /// that hold such a difference, by space and absolute byte, with
    /// its displacement: their bytes stay uninitialized (so the fold
    /// cannot read them) and the freeze relocates them. Cleared by
    /// [`Oven::set_fold_c_static`].
    pub(crate) label_diff_cells: FxHashMap<(mem::Sp, u64), (u32, i64)>,
    /// String literal objects of evaluated `StringLiteral` exprs, one per
    /// expression ([lex.string]/9: distinct evaluations need not share).
    pub(crate) lit_exprs: FxHashMap<ExprId, u32>,
    pub(crate) thawed: FxHashMap<ConstValueId, u64>,
    /// The static objects of thawed lifetime-extended temporaries.
    pub(crate) lets: FxHashMap<DeclId, u64>,
    /// The objects of unknown value that reference variables not usable
    /// in constant expressions bind to (P2280, C++23 [expr.const]/10:
    /// "an unspecified object of the referenced type").
    pub(crate) unknowns: FxHashMap<DeclId, u64>,
    /// The static temporaries a frozen result points to, with their
    /// objects: frozen into `LetData.value` once the result is.
    pub(crate) let_pending: Vec<(DeclId, u64)>,
    /// The characters [`Oven::evaluate_chars`] read (`bi::MSG_CHARS`).
    pub(crate) msg_chars: Vec<u8>,
    /// The parameter bindings of a [`Job::Subst`].
    pub(crate) subst_binds: Vec<(DeclId, ExprId)>,
    /// The potential-constant frame jobs (`frame.rs`, WP-1150).
    pub(crate) frame: frame::FrameState,
    pub(crate) mem: Memory,
    /// Remembered `union_path` answers (WP-760, `vm/umemo.rs`).
    pub(crate) umemo: vm::umemo::UnionMemo,
    pub(crate) regs: Vec<u128>,
    pub(crate) stack: Vec<Frame>,
    /// Registered destructions, innermost last (`Op::PushCleanup`).
    pub(crate) cleanups: Vec<vm::Cleanup>,
    /// In-flight and handled exceptions (P3068, `vm/exc.rs`).
    pub(crate) exc: vm::exc::Excs,
    pub(crate) memo: Memo,
    pub(crate) stats: EvalStats,
    pub(crate) run: vm::RunState,
    pub(crate) scratch: value::Scratch,
    /// The interned pointer-to-member values.
    pub(crate) mp: memptr::MemPtrs,
    /// Class subobject graphs and resolved virtual calls (WP-077).
    pub(crate) virt: vm::Virt,
    /// Evaluation contexts in progress (R-OVEN-1: 0 between evaluations,
    /// above 1 while a bridge callback evaluates a nested one).
    pub(crate) ctx_depth: u32,
    /// What the nested contexts of the running one cost, inclusively.
    inner: Inner,
    /// Reflection call sites and materialization state (spec/reflection.md §3.5).
    pub(crate) meta: meta::materialize::MetaState,
    /// Interned growth already attributed to an intrinsic call
    /// (`refl_bytes`, `meta/intern.rs`).
    pub(crate) refl_seen: meta::intern::ReflSeen,
    /// The data member descriptions of the translation unit (§6.1).
    pub(crate) dms: meta::define::DmsTable,
    /// The instrumentation hooks' switches (`hooks.rs`, spec/oven.md
    /// "Trace hook").
    pub(crate) hooks: hooks::Hooks,
}

/// The inclusive cost of the nested evaluations run so far by the running
/// context (subtracted from its own record).
#[derive(Clone, Copy, Debug, Default)]
struct Inner {
    steps: u64,
    frozen: u64,
    memo_hits: u64,
    wall_us: u64,
}

impl Oven {
    /// A new evaluator with the given limits.
    pub fn new(budget: Budget) -> Self {
        Oven {
            budget,
            backtrace_limit: notes::BACKTRACE_LIMIT,
            clang_compat: 22,
            cxx_year: 2026,
            cev_warn_next: false,
            ub_check_next: false,
            contracts_off_next: false,
            contract_violated: false,
            potential_next: None,
            plain_next: false,
            void_next: false,
            addr_next: false,
            unchecked_next: false,
            types: Types::default(),
            code: Vec::new(),
            padded: 0,
            sites: Vec::new(),
            step_locs: Vec::new(),
            origins: Vec::new(),
            traps: Vec::new(),
            funcs: Vec::new(),
            func_map: FxHashMap::default(),
            base_funcs: FxHashMap::default(),
            ctor_steps: FxHashMap::default(),
            volatile_decls: FxHashSet::default(),
            volatile_exprs: FxHashSet::default(),
            thunks: FxHashMap::default(),
            thunks_at: FxHashMap::default(),
            srcloc_root: None,
            last_thunk: None,
            globals: Vec::new(),
            bound_obj: None,
            global_map: FxHashMap::default(),
            provisional: Vec::new(),
            provisional_objs: FxHashMap::default(),
            fold_float_ub: false,
            addr_int: false,
            fold_leaks: false,
            fold_c_static: false,
            addr_int_disps: FxHashMap::default(),
            synth: std::cell::RefCell::default(),
            label_diffs: Vec::new(),
            label_diff_cells: FxHashMap::default(),
            lit_exprs: FxHashMap::default(),
            thawed: FxHashMap::default(),
            lets: FxHashMap::default(),
            unknowns: FxHashMap::default(),
            let_pending: Vec::new(),
            msg_chars: Vec::new(),
            subst_binds: Vec::new(),
            frame: frame::FrameState::default(),
            mem: Memory::new(budget.bytes),
            umemo: vm::umemo::UnionMemo::default(),
            regs: vec![0; 256],
            stack: Vec::with_capacity(64),
            cleanups: Vec::new(),
            exc: vm::exc::Excs::default(),
            memo: Memo::new(budget.memo_entries),
            stats: EvalStats::default(),
            run: vm::RunState::default(),
            scratch: value::Scratch::default(),
            mp: memptr::MemPtrs::default(),
            virt: vm::Virt::default(),
            ctx_depth: 0,
            inner: Inner::default(),
            meta: meta::materialize::MetaState::default(),
            refl_seen: meta::intern::ReflSeen::default(),
            dms: meta::define::DmsTable::default(),
            hooks: hooks::Hooks::default(),
        }
    }

    /// Interns an allocation origin for `Alloc` instructions.
    pub(crate) fn push_origin(&mut self, o: mem::Origin) -> u32 {
        let i = self.origins.len() as u32;
        self.origins.push(o);
        i
    }

    /// Lets floating arithmetic that yields a NaN or divides by zero
    /// ([expr.pre]/4, [expr.mul]/4: not a constant expression) go on with
    /// the IEEE result, as clang's codegen folds an initializer or operand
    /// (`ConstantEmitter`: such a result is undefined behaviour the
    /// folding evaluation modes keep evaluating past). A call that did is
    /// never memoized. Codegen sets it; sema sets it only for the C check
    /// that a static initializer is constant (which reads success alone).
    pub fn set_fold_float_ub(&mut self, on: bool) {
        self.fold_float_ub = on;
    }

    /// Lets a fold outside a constant context ([`Oven::fold_rvalue`])
    /// of a variable's initializer keep the address a pointer-to-integer
    /// cast of the whole value (or of a scalar member's) converts, as
    /// clang's `ConstantEmitter` does (`static intptr_t i = (intptr_t)&g;`,
    /// C23 6.6p9): the integer freezes as a [`kiln_ast::consts::Scalar::Pointer`]
    /// of integer type, which codegen emits as `ptrtoint`. Codegen sets it
    /// around its initializer folds; sema sets it only for the C check that
    /// a static initializer is constant, which reads success and never the
    /// value, so no integer constant expression sema reads holds an address
    /// (spec/oven.md "Address constants").
    pub fn set_fold_addr_int(&mut self, on: bool) {
        self.addr_int = on;
    }

    /// Lets a fold outside a constant context ([`Oven::fold_rvalue`])
    /// succeed with allocations still live at its end, as long as its
    /// value does not point at one: clang's `CheckMemoryLeaks` is only a
    /// core-constant-expression note there (`CCEDiag`), so its codegen
    /// folds `(bool)new int(4)` to `true` (`ConstantFoldsToSimpleInteger`,
    /// coverage `ConditionFoldsToBool`; WP-1108). The allocations are
    /// reclaimed with the context, as any other. A constant expression
    /// ([expr.const]/5.20) and every evaluation sema runs keep the leak
    /// check. Codegen sets it; sema sets it only for the C check that a
    /// static initializer is constant (which reads success alone).
    pub fn set_fold_leaks(&mut self, on: bool) {
        self.fold_leaks = on;
    }

    /// Marks the folds that follow as a C static initializer's. Every C
    /// file-scope or `static` variable's initializer must be a constant
    /// expression (C23 6.7.11p4), and clang evaluates it in a constant
    /// context (`VarDecl::hasConstantInitialization` is always true in C),
    /// so `__builtin_constant_p` of a non-constant argument there folds
    /// to 0 instead of failing the lax fold (GCC agrees). Codegen sets it
    /// around a C translation unit's initializer folds and sema around
    /// its C static-initializer check; C++ never sets it.
    pub fn set_fold_c_static(&mut self, on: bool) {
        self.fold_c_static = on;
        // Per-object state of the previous fold's addresses an integer
        // holds; a later fold must not read it back.
        if !self.label_diff_cells.is_empty() {
            self.label_diff_cells.clear();
        }
        if !self.addr_int_disps.is_empty() {
            self.addr_int_disps.clear();
        }
        self.label_diffs.clear();
    }

    /// Sets how many call frames a failure's backtrace shows
    /// (`-fconstexpr-backtrace-limit=`; clang's default is 10, 0 shows
    /// all). Elided frames are summarized in one "skipping" note.
    pub fn set_backtrace_limit(&mut self, n: usize) {
        self.backtrace_limit = n;
    }

    /// Sets the clang release (major version, 19 or 22) whose note wording
    /// a failure's notes follow (`-fclang-compat=`, spec/oven.md
    /// "Diagnostics"). Default 22, as the driver.
    pub fn set_clang_compat(&mut self, level: u32) {
        self.clang_compat = level;
    }

    /// The compatibility level ([`Oven::set_clang_compat`]).
    pub fn clang_compat(&self) -> u32 {
        self.clang_compat
    }

    /// Records the translation unit's language standard as its year
    /// (sema's [`SemaBridge::cxx_year`]), for a bridge that does not know
    /// it: codegen's folds share the TU's bytecode, which bakes in the
    /// mode-dependent rules (the pre-C++26 `void *` cast), so they must
    /// see the same year (spec/oven.md "Casts from void *"). Default 2026.
    pub fn set_cxx_year(&mut self, year: u32) {
        self.cxx_year = year;
    }

    /// The year [`Oven::set_cxx_year`] recorded.
    pub fn cxx_year(&self) -> u32 {
        self.cxx_year
    }

    /// The backtrace limit ([`Oven::set_backtrace_limit`]).
    pub fn backtrace_limit(&self) -> usize {
        self.backtrace_limit
    }

    /// Whether the running evaluation is plainly constant-evaluated
    /// (spec/consteval-blocks.md §3): its context asked for it
    /// ([`EvalContext::plain`]) and it is not a fold or a speculative
    /// trial (`__builtin_constant_p`'s operand, [`Oven::try_fold`],
    /// codegen's folds). The gate of every injecting operation.
    pub fn plain(&self) -> bool {
        self.run.plain && !self.run.fold && !self.mem.in_trial()
    }

    /// The limits.
    pub fn budget(&self) -> Budget {
        self.budget
    }

    /// Marks the running evaluation impure (not memoized) from the
    /// bridge: a `Stable` reflection op whose answer for these operands
    /// can still change later in the TU, as a `PointDependent` op's can
    /// (spec/reflection.md §3.3; `type_of` of a variable whose type is
    /// still an array of unknown bound). O(1).
    pub fn mark_impure(&mut self) {
        self.run.impure += 1;
    }

    /// Opens a transactional trial (spec/oven.md "Trial evaluation",
    /// `TrialMode::Transactional`) around bridge work that may evaluate
    /// nested constants (`std::meta::can_substitute`): every effect is
    /// allowed; [`Oven::trial_end`] keeps or undoes them. O(1).
    pub fn trial_begin(&mut self) {
        self.mem.checkpoint(mem::TrialMode::Transactional);
    }

    /// Closes the innermost trial [`Oven::trial_begin`] opened: `keep`
    /// commits its effects, else they are rolled back (statics and
    /// interned values made since are kept).
    pub fn trial_end(&mut self, keep: bool) {
        if keep {
            self.mem.commit();
        } else {
            self.mem.rollback(&self.types);
        }
    }

    /// Evaluates the constant expression `e` (a manifestly
    /// constant-evaluated context; recorded in [`Oven::stats`]).
    pub fn evaluate(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: ExprId,
        ctx: EvalContext,
    ) -> Result<ConstValueId, EvalFailure> {
        let hook = self.hook_eval_begin(&ctx);
        self.plain_next = ctx.plain;
        self.void_next = ctx.kind == EvalKind::Consteval;
        // A glvalue immediate invocation ([expr.const]/16) is a constant
        // expression whose result is the entity it designates
        // ([expr.const]/22): its value is that address, not the
        // designated object's value (which need not be readable: a
        // non-const variable's; WP-640).
        self.addr_next = self.void_next && is_glvalue(bridge.ast(), e);
        let (r, c) = self.eval_ctx(bridge, e, ctx.object, false, ctx.enum_range_check, true);
        self.plain_next = false;
        self.void_next = false;
        self.addr_next = false;
        if let Some(m) = hook {
            self.hook_eval_end(bridge, m, Some(e), &ctx, &c, r.is_ok());
        }
        self.stats.push(EvalRecord {
            loc: ctx.loc,
            kind: ctx.kind,
            steps: c.steps,
            bytes: c.bytes,
            peak_bytes: c.peak,
            calls: c.calls,
            allocs: c.allocs,
            peak_arena: c.arena,
            frozen_bytes: c.frozen,
            static_bytes: self.mem.static_top(),
            memo_hits: c.memo_hits,
            refl_bytes: c.refl,
            reclaimed_bytes: c.reclaimed,
            wall_us: c.wall_us,
            ok: r.is_ok(),
        });
        r
    }

    /// Evaluates the character range of a `static_assert` message
    /// ([dcl.pre]/11, P2741; clang `Expr::EvaluateCharRangeAsString`):
    /// `size` (of type `std::size_t`) and `data` (of type `const char *`),
    /// then the `size` characters `data` points to, in one manifestly
    /// constant-evaluated context whose temporaries live until the last
    /// character is read. Recorded in [`Oven::stats`] as `ctx.kind`.
    ///
    /// Returns the characters read (those before a failing read: clang
    /// prints them) and the failure, with its notes, if any of these is
    /// not a constant expression (a read outside the object `data`
    /// points into).
    pub fn evaluate_chars(
        &mut self,
        bridge: &mut dyn SemaBridge,
        size: ExprId,
        data: ExprId,
        ctx: EvalContext,
    ) -> (Vec<u8>, Result<(), EvalFailure>) {
        let hook = self.hook_eval_begin(&ctx);
        let job = Job::Chars { size, data };
        let (r, c) = self.eval_job(bridge, job, None, false, false, true);
        if let Some(m) = hook {
            self.hook_eval_end(bridge, m, None, &ctx, &c, r.is_ok());
        }
        self.stats.push(EvalRecord {
            loc: ctx.loc,
            kind: ctx.kind,
            steps: c.steps,
            bytes: c.bytes,
            peak_bytes: c.peak,
            calls: c.calls,
            allocs: c.allocs,
            peak_arena: c.arena,
            frozen_bytes: c.frozen,
            static_bytes: self.mem.static_top(),
            memo_hits: c.memo_hits,
            refl_bytes: c.refl,
            reclaimed_bytes: c.reclaimed,
            wall_us: c.wall_us,
            ok: r.is_ok(),
        });
        let chars = std::mem::take(&mut self.msg_chars);
        (chars, r.map(|_| ()))
    }

    /// Evaluates `cond` (`None`: `true`) as a constant expression with
    /// each parameter `p` of `binds` initialized from its argument `a`
    /// first, in one scope whose objects are destroyed after the
    /// condition (clang `Expr::EvaluateWithSubstitution`, for the
    /// condition of an `enable_if` attribute; spec/oven.md "Evaluation entry
    /// points"). A
    /// parameter not in `binds` has no value: reading it is not a
    /// constant expression. Silent, manifestly constant-evaluated, not
    /// recorded in the statistics; `at` locates the evaluation.
    pub fn evaluate_with_params(
        &mut self,
        bridge: &mut dyn SemaBridge,
        cond: Option<ExprId>,
        binds: &[(DeclId, ExprId)],
        at: ExprId,
    ) -> Option<ConstValueId> {
        let saved = std::mem::replace(&mut self.subst_binds, binds.to_vec());
        let job = Job::Subst { cond, at };
        let r = self.eval_job(bridge, job, None, true, false, true).0.ok();
        self.subst_binds = saved;
        r
    }

    /// Evaluates a call of function `f` without arguments at `at`, with
    /// its notes, manifestly constant-evaluated (as clang's
    /// `Expr::isPotentialConstantExpr` runs the body of a `constexpr`
    /// function without parameters; spec/oven.md "Evaluation entry
    /// points"): the failure if it is not a constant expression, its last
    /// note the call's "in call to". A function with parameters, or
    /// returning a reference or an aggregate, is not evaluated (`Ok`).
    /// Not recorded in the statistics; outside an outermost evaluation,
    /// `Ok`.
    pub fn evaluate_call(
        &mut self,
        bridge: &mut dyn SemaBridge,
        f: DeclId,
        at: Loc,
    ) -> Result<(), EvalFailure> {
        if self.ctx_depth != 0 {
            return Ok(());
        }
        self.potential_next = Some(1);
        let r = self.eval_job(bridge, Job::Call { f, at }, None, false, false, true).0;
        self.potential_next = None;
        match r {
            Err(e) if e.notes.is_empty() => Ok(()),
            r => r.map(|_| ()),
        }
    }

    /// `e`, a subexpression of a `constexpr` function's body that names
    /// none of its parameters or local variables, evaluated as clang's
    /// `Expr::isPotentialConstantExpr` evaluates it
    /// (`checkingPotentialConstantExpression`: no "always true"
    /// `is_constant_evaluated` warning; a call of a `constexpr` function
    /// fails without a note, as clang's `CheckCallLimit` makes none; a
    /// `void` expression has a value). WP-873.
    pub fn evaluate_potential(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: ExprId,
    ) -> Result<ConstValueId, EvalFailure> {
        if self.ctx_depth != 0 {
            return Err(EvalFailure::silent(FailKind::NotConstant));
        }
        self.potential_next = Some(0);
        self.void_next = true;
        self.unchecked_next = true;
        let r = self.eval_ctx(bridge, e, None, false, false, true).0;
        self.potential_next = None;
        self.void_next = false;
        self.unchecked_next = false;
        r
    }

    /// Folds `e` if it is a constant expression; never reports anything
    /// and is not recorded in the statistics. The evaluation is a trial,
    /// not manifestly constant-evaluated: `__builtin_is_constant_evaluated()`
    /// yields false.
    pub fn try_fold(&mut self, bridge: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId> {
        #[cfg(feature = "op-stats")]
        let trial = trial_stats::begin(bridge.ast(), Some(e), self.memo.hits);
        let r = match self.leaf_first(bridge, e) {
            Some(r) => r,
            None => self.trial_eval(bridge, e),
        };
        #[cfg(feature = "op-stats")]
        trial_stats::end(trial, "try_fold", r.is_some(), self.memo.hits);
        r
    }

    /// As [`Oven::try_fold`], laxly (clang `Expr::EvaluateAsRValue`, as
    /// codegen's `tryEmitAsConstant` and a folded array bound use it): a
    /// `const` non-volatile variable with an initializer is read even if
    /// it is not usable in constant expressions (a `const double`, a
    /// `const _Atomic(int)`; [expr.const]/4 rejects such a read, clang
    /// only notes it). Strict outside an outermost evaluation.
    pub fn fold_rvalue(&mut self, bridge: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId> {
        if self.ctx_depth != 0 {
            return self.try_fold(bridge, e);
        }
        self.run.lax = true;
        self.mem.lax = true;
        #[cfg(feature = "op-stats")]
        let trial = trial_stats::begin(bridge.ast(), Some(e), self.memo.hits);
        let r = match self.leaf_first(bridge, e) {
            Some(r) => r,
            None => self.trial_eval(bridge, e),
        };
        #[cfg(feature = "op-stats")]
        trial_stats::end(trial, "fold_rvalue", r.is_some(), self.memo.hits);
        self.run.lax = false;
        self.mem.lax = false;
        r
    }

    /// clang `Expr::tryEvaluateObjectSize` (WP-808): the object size of
    /// pointer expression `p` for type `ty` (0-3) as
    /// `__builtin_object_size(p, ty)` folds it (codegen's
    /// `evaluateOrEmitBuiltinObjectSize` of a `pass_object_size`
    /// argument), without the builtin's side-effect default: `None` when
    /// the pointer does not fold to a known object. A silent trial, lax as
    /// [`Oven::fold_rvalue`].
    pub fn fold_object_size(
        &mut self,
        bridge: &mut dyn SemaBridge,
        p: ExprId,
        ty: u32,
    ) -> Option<u64> {
        let outer = self.ctx_depth == 0;
        if outer {
            self.run.lax = true;
            self.mem.lax = true;
        }
        let mark = self.trial_mark();
        let job = Job::ObjectSize { p, ty };
        let r = self.eval_job(bridge, job, None, true, false, false).0.ok();
        if outer {
            self.run.lax = false;
            self.mem.lax = false;
        }
        let n = r.and_then(|v| match bridge.ast().consts.scalar(v) {
            Some(kiln_ast::consts::Scalar::Int { bits, .. }) => bits.to_u128(),
            _ => None,
        });
        if n.is_none() {
            self.reclaim_failed(mark);
        }
        n.and_then(|n| u64::try_from(n).ok())
    }

    /// clang's `Expr::EvaluateForOverflow` of `e` (`EM_IgnoreSideEffects`
    /// with `CheckingForUndefinedBehavior`), as [`Oven::fold_rvalue`]:
    /// a signed integer `+`, `-`, `*` (compound assignments included),
    /// unary `-`, `++` or `--` whose result is outside its type does not
    /// fail the evaluation, which goes on with the value wrapped to the
    /// type; each one is returned, in evaluation order, with the value.
    /// Calls are entered, so an overflow in a `constexpr` function's body
    /// is found. Nested evaluations, global initializations and
    /// `__builtin_constant_p` check nothing. Silent and not recorded.
    /// Outside an outermost evaluation, a plain [`Oven::try_fold`].
    pub fn fold_for_overflow(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: ExprId,
    ) -> (Option<ConstValueId>, Vec<Overflow>) {
        if self.ctx_depth != 0 {
            return (self.try_fold(bridge, e), Vec::new());
        }
        self.run.lax = true;
        self.mem.lax = true;
        self.ub_check_next = true;
        #[cfg(feature = "op-stats")]
        let trial = trial_stats::begin(bridge.ast(), Some(e), self.memo.hits);
        let mark = self.trial_mark();
        let r = self.eval_ctx(bridge, e, None, true, false, false).0.ok();
        #[cfg(feature = "op-stats")]
        trial_stats::end(trial, "fold_for_overflow", r.is_some(), self.memo.hits);
        self.ub_check_next = false;
        self.run.lax = false;
        self.mem.lax = false;
        self.run.ub_check = false;
        let found = std::mem::take(&mut self.run.overflows)
            .into_iter()
            .map(|(pc, value)| {
                let site = self.site(pc as usize);
                Overflow { loc: site.loc, e: site.e, value }
            })
            .collect();
        if r.is_none() {
            self.reclaim_failed(mark);
        }
        (r, found)
    }

    /// Evaluates immediate invocation `e` ([expr.const]/16; manifestly
    /// constant-evaluated, silent, not recorded) as it appears inside the
    /// default argument used by `CXXDefaultArgExpr` `at`: its
    /// source-location builtins take the values of that use
    /// ([support.srcloc.cons]/2; clang rebuilds such a default argument at
    /// each use and evaluates its invocations there).
    pub fn fold_immediate_in(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: ExprId,
        at: Option<ExprId>,
    ) -> Option<ConstValueId> {
        let saved = std::mem::replace(&mut self.srcloc_root, at);
        self.addr_next = is_glvalue(bridge.ast(), e);
        let r = self.eval_ctx(bridge, e, None, true, false, true).0.ok();
        self.addr_next = false;
        self.srcloc_root = saved;
        r
    }

    /// Folds the initializer `e` of variable `var` as a constant
    /// initializer ([basic.start.static]/2, manifestly constant-evaluated:
    /// `__builtin_is_constant_evaluated()` yields true); `None` if it is
    /// not a constant expression. Silent and not recorded in the
    /// statistics, like [`Oven::try_fold`].
    pub fn fold_constant_init(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: ExprId,
        var: DeclId,
    ) -> Option<ConstValueId> {
        #[cfg(feature = "op-stats")]
        let trial = trial_stats::begin(bridge.ast(), Some(e), self.memo.hits);
        let r = self.eval_ctx(bridge, e, Some(var), true, false, true).0.ok();
        #[cfg(feature = "op-stats")]
        trial_stats::end(trial, "fold_constant_init", r.is_some(), self.memo.hits);
        r
    }

    /// clang's `-Wconstant-evaluated` of a variable's initializer `e`
    /// that is not checked as `constexpr` or `constinit`: clang's
    /// `CheckCompleteVariableDeclaration` evaluates the initializer of
    /// every variable with static storage, or that might be usable in
    /// constant expressions, as a constant initializer, and
    /// `__builtin_is_constant_evaluated()` (or `std::is_constant_evaluated()`)
    /// called directly by it warns that it is always true there, even when
    /// the initialization then fails. Otherwise silent; not recorded.
    pub fn warn_constant_evaluated(&mut self, bridge: &mut dyn SemaBridge, e: ExprId, var: DeclId) {
        self.cev_warn_next = true;
        let _ = self.eval_ctx(bridge, e, Some(var), true, false, true);
        self.cev_warn_next = false;
    }

    /// The contract assertions of the initialization `e` of variable
    /// `var` that is not checked as `constexpr` or `constinit`
    /// ([expr.const.init]/4.1, spec/contracts.md §6.4): whether it is
    /// constant-initialized is decided with every contract assertion
    /// ignored; if it is, the initialization is manifestly
    /// constant-evaluated with the translation unit's semantic, so a
    /// violation under a terminating semantic is an error (GCC's wording,
    /// at the assertion, with the call stack) and one under observe a
    /// warning. Sema calls it only when the translation unit has contract
    /// assertions; an initialization that violates none costs one silent
    /// evaluation. Not recorded.
    pub fn check_contract_init(&mut self, bridge: &mut dyn SemaBridge, e: ExprId, var: DeclId) {
        if self.ctx_depth != 0 {
            return;
        }
        let _ = self.eval_ctx(bridge, e, Some(var), true, false, true);
        if !std::mem::take(&mut self.contract_violated) {
            return;
        }
        self.contracts_off_next = true;
        let constant = self.eval_ctx(bridge, e, Some(var), true, false, true).0.is_ok();
        self.contracts_off_next = false;
        if !constant {
            // Dynamic initialization: the assertions are checked at run
            // time.
            return;
        }
        let (r, _) = self.eval_ctx(bridge, e, Some(var), false, false, true);
        let violated = std::mem::take(&mut self.contract_violated);
        let Err(f) = r else { return };
        if f.notes.is_empty() {
            return;
        }
        let mut notes = f.notes.into_iter();
        // A violation's note is GCC's message; otherwise a predicate
        // failed to be a constant (the rest of the evaluation is one,
        // as the decision above found), noted by its own reason.
        let (id, loc, first) = if violated {
            let n = notes.next().map(|n| n.loc).unwrap_or_default();
            (kiln_diag::DiagId::oven_contract_predicate_false, n, None)
        } else {
            let n = notes.next();
            let loc = n.as_ref().map(|n| n.loc).unwrap_or_default();
            (kiln_diag::DiagId::oven_contract_condition_not_constant, loc, n)
        };
        let mut d = Diag::new(id, kiln_source::Span::empty(loc)).allow_duplicate();
        for n in first.into_iter().chain(notes) {
            d = d.note(n.msg, kiln_source::Span::empty(n.loc));
        }
        bridge.diag(d);
    }

    /// Whether `var`'s initializer is a constant initializer
    /// ([basic.start.static]/2): it evaluates successfully as a constant
    /// expression. Not recorded in the statistics. An initializer in the
    /// trial-fold walk's domain is answered by the walk (WP-663,
    /// [`Oven::leaf_init`]).
    pub fn is_constant_initialized(&mut self, bridge: &mut dyn SemaBridge, var: DeclId) -> bool {
        self.constant_initialized(bridge, var, true)
    }

    /// [`Oven::is_constant_initialized`] by the full path only (test-only).
    #[doc(hidden)]
    pub fn is_constant_initialized_full(
        &mut self,
        bridge: &mut dyn SemaBridge,
        var: DeclId,
    ) -> bool {
        self.constant_initialized(bridge, var, false)
    }

    /// [`Oven::is_constant_initialized`], with or without its fast path.
    fn constant_initialized(
        &mut self,
        bridge: &mut dyn SemaBridge,
        var: DeclId,
        fast: bool,
    ) -> bool {
        use kiln_ast::decl::VarEvalState;
        let ast = bridge.ast();
        let Some(data) = ast.decls.var(var) else { return false };
        match data.eval.state() {
            VarEvalState::Value(_) | VarEvalState::NotConstant => {
                #[cfg(feature = "op-stats")]
                trial_stats::no_eval();
                return matches!(data.eval.state(), VarEvalState::Value(_));
            }
            _ => {}
        }
        let Some(init) = data.init else { return false };
        #[cfg(feature = "op-stats")]
        let trial = trial_stats::begin(ast, Some(init), self.memo.hits);
        let r = match fast.then(|| self.leaf_init(bridge, var, init)).flatten() {
            Some(r) => r,
            None => self.eval_ctx(bridge, init, Some(var), true, false, true).0.is_ok(),
        };
        #[cfg(feature = "op-stats")]
        trial_stats::end(trial, "is_constant_initialized", r, self.memo.hits);
        r
    }

    /// Whether variable `var` has constant destruction ([expr.const]/7):
    /// destroying it at the end of its lifetime is a constant expression
    /// (clang `VarDecl::evaluateDestruction`, which
    /// `Sema::FinalizeVarWithDestructor` runs for every variable whose
    /// destructor is non-trivial and `constexpr`; such a variable needs no
    /// destructor call, spec/codegen.md). `value` is its initializer's
    /// value, if that is constant; `const_context` whether it is
    /// constant-initialized (the destruction is then manifestly
    /// constant-evaluated). Only the object of a `const` variable is
    /// readable and writable, holding `value` or indeterminate; its
    /// `mutable` members, and every subobject of any other variable, are
    /// outside their lifetime ([expr.const]/5.9, /5.16). Silent and not
    /// recorded in the statistics; `false` inside a running evaluation.
    pub fn has_constant_destruction(
        &mut self,
        bridge: &mut dyn SemaBridge,
        var: DeclId,
        value: Option<ConstValueId>,
        const_context: bool,
    ) -> bool {
        if self.ctx_depth != 0 {
            return false;
        }
        self.types.refresh(bridge);
        self.mp_refresh(bridge);
        self.begin_outermost(true, false, const_context);
        // clang requires no notes at all (`VarDecl::evaluateDestruction`).
        self.run.fold = false;
        let r = self.destroy_var(bridge, var, value);
        self.end_outermost();
        r
    }

    /// The global entries (kind and resolution), the provisional ones and
    /// their reusable objects, and the static allocations, as text: the
    /// fast path's equivalence tests compare it (test-only).
    #[doc(hidden)]
    pub fn globals_digest(&self) -> String {
        let mut objs: Vec<_> = self.provisional_objs.iter().collect();
        objs.sort_unstable_by_key(|x| *x.0);
        let statics: Vec<_> =
            self.mem.statics.iter().map(|a| (a.base, a.size, a.flags, a.origin)).collect();
        format!(
            "{:?} {:?} {objs:?} {statics:?}",
            self.globals.iter().map(|g| (g.kind, g.state)).collect::<Vec<_>>(),
            self.provisional
        )
    }

    /// Per-evaluation records and totals.
    pub fn stats(&self) -> &EvalStats {
        &self.stats
    }

    /// Records `n` splices whose operand sema read as a frozen reflection
    /// without an evaluation (the report's `splice_fast` total;
    /// spec/oven.md "Reporting protocol").
    pub fn note_splice_fast(&mut self, n: u64) {
        self.stats.note_splice_fast(n);
    }

    /// Bytes of heap the evaluator holds (capacities of its code, abstract
    /// memory, registers, stacks, caches, scratch buffers and type table). Compiling
    /// new code grows it; re-evaluating compiled code must not
    /// (spec/oven.md "Memory": the frame arena is reset, not freed).
    pub fn footprint(&self) -> usize {
        fn cap<T>(v: &Vec<T>) -> usize {
            v.capacity() * size_of::<T>()
        }
        fn map<K, V>(m: &FxHashMap<K, V>) -> usize {
            m.capacity() * (size_of::<(K, V)>() + 1)
        }
        cap(&self.code)
            + cap(&self.sites)
            + cap(&self.step_locs)
            + cap(&self.origins)
            + cap(&self.traps)
            + cap(&self.funcs)
            + map(&self.func_map)
            + map(&self.base_funcs)
            + map(&self.ctor_steps)
            + self.ctor_steps.values().map(|s| size_of_val(&**s)).sum::<usize>()
            + map(&self.thunks)
            + map(&self.thunks_at)
            + cap(&self.globals)
            + map(&self.global_map)
            + cap(&self.provisional)
            + map(&self.provisional_objs)
            + map(&self.lit_exprs)
            + map(&self.thawed)
            + map(&self.lets)
            + map(&self.unknowns)
            + cap(&self.let_pending)
            + cap(&self.msg_chars)
            + self.mem.footprint()
            + cap(&self.regs)
            + cap(&self.stack)
            + cap(&self.cleanups)
            + self.exc.footprint()
            + self.memo.footprint()
            + self.run.footprint()
            + self.scratch.footprint()
            + self.dms.footprint()
            + self.mp.footprint()
            + self.virt.footprint()
            + self.types.footprint()
    }

    /// [`Oven::footprint`] as accounting rows (`-fkiln-memory-report`,
    /// spec/oven.md "Memory report"): `oven.code` (bytecode words),
    /// `oven.sites` (sites, step locations, origins, traps; count =
    /// sites), `oven.funcs` (compiled functions and thunks, with their
    /// maps and construction steps; count = functions), `oven.thunks`
    /// (the thunk caches; count = cached thunks), `oven.globals` (globals
    /// and the per-variable and per-literal maps; count = globals),
    /// `oven.heap-peak` (the translation unit's evaluation-memory
    /// high-water mark, `Memory::heap_peak`; count = bytes),
    /// `oven.static-peak` (the static space's; count = bytes),
    /// `oven.mem.capacity` (abstract memory's buffers and tables as held;
    /// count = static space bytes; a `.capacity` row, outside the total),
    /// `oven.memo` and `oven.other` (registers, stacks, scratch, type
    /// table). The reserved bytes of every row but the two peaks sum to
    /// [`Oven::footprint`].
    pub fn memory(&self) -> kiln_ast::mem::MemReport {
        use kiln_ast::mem::{MemReport, MemRow};
        fn cap<T>(v: &Vec<T>) -> u64 {
            (v.capacity() * size_of::<T>()) as u64
        }
        fn len<T>(v: &[T]) -> u64 {
            size_of_val(v) as u64
        }
        fn map<K, V>(m: &FxHashMap<K, V>) -> u64 {
            (m.capacity() * (size_of::<(K, V)>() + 1)) as u64
        }
        fn mlen<K, V>(m: &FxHashMap<K, V>) -> u64 {
            (m.len() * (size_of::<(K, V)>() + 1)) as u64
        }
        let steps: u64 = self.ctor_steps.values().map(|s| size_of_val(&**s) as u64).sum();
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("oven.code", &self.code));
        r.push(MemRow::new(
            "oven.sites",
            self.sites.len() as u64,
            len(&self.sites) + len(&self.step_locs) + len(&self.origins) + len(&self.traps),
            cap(&self.sites) + cap(&self.step_locs) + cap(&self.origins) + cap(&self.traps),
        ));
        r.push(MemRow::new(
            "oven.funcs",
            self.funcs.len() as u64,
            len(&self.funcs)
                + mlen(&self.func_map)
                + mlen(&self.base_funcs)
                + mlen(&self.ctor_steps)
                + steps,
            cap(&self.funcs)
                + map(&self.func_map)
                + map(&self.base_funcs)
                + map(&self.ctor_steps)
                + steps,
        ));
        r.push(MemRow::new(
            "oven.thunks",
            (self.thunks.len() + self.thunks_at.len()) as u64,
            mlen(&self.thunks) + mlen(&self.thunks_at),
            map(&self.thunks) + map(&self.thunks_at),
        ));
        let globals_live = len(&self.globals)
            + mlen(&self.global_map)
            + len(&self.provisional)
            + mlen(&self.provisional_objs)
            + mlen(&self.lit_exprs)
            + mlen(&self.thawed)
            + mlen(&self.lets)
            + mlen(&self.unknowns)
            + len(&self.let_pending);
        let globals_cap = cap(&self.globals)
            + map(&self.global_map)
            + cap(&self.provisional)
            + map(&self.provisional_objs)
            + map(&self.lit_exprs)
            + map(&self.thawed)
            + map(&self.lets)
            + map(&self.unknowns)
            + cap(&self.let_pending);
        r.push(MemRow::new("oven.globals", self.globals.len() as u64, globals_live, globals_cap));
        // Abstract memory enters the total by its exact high-water marks,
        // not by its buffers' capacity, which doubles as it grows and so
        // moves in steps unrelated to the peak (WP-1315).
        let hp = self.mem.heap_peak();
        r.push(MemRow::new("oven.heap-peak", hp, hp, hp));
        let sp = self.mem.static_peak().max(self.mem.static_top());
        r.push(MemRow::new("oven.static-peak", sp, sp, sp));
        let m = self.mem.footprint() as u64;
        r.push(MemRow::new("oven.mem.capacity", self.mem.static_top(), m, m));
        let m = self.memo.footprint() as u64;
        r.push(MemRow::new("oven.memo", 0, m, m));
        let other =
            (cap(&self.msg_chars) + cap(&self.regs) + cap(&self.stack) + cap(&self.cleanups))
                as usize
                + self.exc.footprint()
                + self.run.footprint()
                + self.scratch.footprint()
                + self.dms.footprint()
                + self.mp.footprint()
                + self.virt.footprint()
                + self.types.footprint();
        r.push(MemRow::new("oven.other", 0, other as u64, other as u64));
        r
    }

    /// The disassembly of everything compiled so far (debugging aid).
    pub fn disassemble(&self) -> String {
        kbc::disassemble(&self.code, 0, self.code.len())
    }

    /// The silent evaluation of a trial fold of `e` (after the leaf fast
    /// path declined): a failed outermost one gives its fresh thunk's code
    /// back (`reclaim.rs`).
    fn trial_eval(&mut self, bridge: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId> {
        let mark = self.trial_mark();
        let r = self.eval_ctx(bridge, e, None, true, false, false).0.ok();
        if r.is_none() {
            self.reclaim_failed(mark);
        }
        r
    }

    /// Runs one evaluation context: the outermost one, or one nested in a
    /// bridge callback of the running one (R-OVEN-1). The cost is the
    /// context's own: what its nested contexts cost is excluded (they have
    /// records of their own), though the step budget is shared.
    fn eval_ctx(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: ExprId,
        object: Option<DeclId>,
        silent: bool,
        enum_check: bool,
        const_context: bool,
    ) -> (Result<ConstValueId, EvalFailure>, Cost) {
        self.eval_job(bridge, Job::Expr(e), object, silent, enum_check, const_context)
    }

    /// [`Oven::eval_ctx`] of any job.
    fn eval_job(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: Job,
        object: Option<DeclId>,
        silent: bool,
        enum_check: bool,
        const_context: bool,
    ) -> (Result<ConstValueId, EvalFailure>, Cost) {
        // The ICE breadcrumb (spec/driver.md "Internal compiler errors").
        let _phase =
            kiln_source::activity::enter(kiln_source::activity::Phase::Oven, e.loc(bridge.ast()));
        let start = Instant::now();
        self.types.refresh(bridge);
        self.mp_refresh(bridge);
        let frozen0 = bridge.consts().content_bytes();
        let hits0 = self.memo.hits;
        let inner0 = std::mem::take(&mut self.inner);
        let (r, c) = if self.ctx_depth == 0 {
            self.eval_outermost(bridge, e, object, silent, enum_check, const_context)
        } else {
            self.eval_nested(bridge, e, object, silent, enum_check, const_context)
        };
        let wall_us = start.elapsed().as_micros() as u64;
        let frozen = bridge.consts().content_bytes().saturating_sub(frozen0);
        let memo_hits = self.memo.hits.saturating_sub(hits0);
        let inner = std::mem::replace(&mut self.inner, inner0);
        if self.ctx_depth > 0 {
            self.inner.steps += c.steps;
            self.inner.frozen += frozen;
            self.inner.memo_hits += memo_hits;
            self.inner.wall_us += wall_us;
        }
        let cost = Cost {
            steps: c.steps.saturating_sub(inner.steps),
            bytes: c.bytes,
            peak: c.peak,
            arena: c.arena,
            calls: c.calls,
            allocs: c.allocs,
            refl: c.refl,
            reclaimed: c.reclaimed,
            frozen: frozen.saturating_sub(inner.frozen),
            memo_hits: memo_hits.saturating_sub(inner.memo_hits),
            wall_us: wall_us.saturating_sub(inner.wall_us),
        };
        // Every context, recorded or not (`-fconstexpr-stats`' trials).
        self.stats.note_job(cost.steps, cost.wall_us, r.is_ok());
        (r, cost)
    }

    /// Runs `job` with registers from `fb`.
    fn run_job(
        &mut self,
        b: &mut dyn SemaBridge,
        job: Job,
        object: Option<DeclId>,
        fb: u32,
    ) -> Result<ConstValueId, EvalFailure> {
        self.unbound(|o| match job {
            Job::Expr(e) => o.eval_inner(b, e, object, fb),
            Job::Chars { size, data } => o.eval_chars(b, size, data, fb),
            Job::Subst { cond, at } => o.eval_subst(b, cond, at, fb),
            Job::Call { f, at } => o.eval_call(b, f, at, fb),
            Job::ObjectSize { p, ty } => o.eval_object_size(b, p, ty, fb),
            Job::Frame(e) => o.eval_frame(b, e, fb),
        })
    }

    /// The outermost context: owns the frame and heap spaces, reset at its
    /// end.
    fn eval_outermost(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: Job,
        object: Option<DeclId>,
        silent: bool,
        enum_check: bool,
        const_context: bool,
    ) -> (Result<ConstValueId, EvalFailure>, vm::Counters) {
        self.begin_outermost(silent, enum_check, const_context);
        let r = self.run_job(bridge, e, object, 0);
        let c = self.end_outermost();
        (r, c)
    }

    /// Starts the outermost context: globals resolved provisionally are
    /// forgotten, the counters reset.
    fn begin_outermost(&mut self, silent: bool, enum_check: bool, const_context: bool) {
        for g in self.provisional.drain(..) {
            self.globals[g as usize].state = compile::GState::Unresolved;
        }
        self.run.silent = silent;
        self.run.enum_check = enum_check;
        self.run.const_context = const_context;
        self.run.fold = silent && !const_context;
        self.run.cev_warn = std::mem::take(&mut self.cev_warn_next);
        self.run.ub_check = std::mem::take(&mut self.ub_check_next);
        self.run.potential = std::mem::take(&mut self.potential_next);
        self.run.contracts_off = std::mem::take(&mut self.contracts_off_next);
        self.contract_violated = false;
        self.run.overflows.clear();
        self.run.plain = std::mem::take(&mut self.plain_next);
        self.run.counters = vm::Counters::default();
        self.run.nesting = 0;
        self.run.depth_base = 0;
        self.mem.allocated = 0;
        self.mem.peak = 0;
        self.mem.fold_peak();
        self.mem.arena_peak = 0;
        self.mem.heap_allocs = 0;
        self.mem.ctx = 0;
        self.ctx_depth = 1;
    }

    /// Ends the outermost context: its cost; the frame and heap spaces
    /// are reset.
    fn end_outermost(&mut self) -> vm::Counters {
        self.ctx_depth = 0;
        self.run.plain = false;
        let mut c = self.run.counters;
        c.bytes = self.mem.allocated;
        c.peak = self.mem.peak;
        c.arena = self.mem.arena_peak;
        c.allocs = self.mem.heap_allocs;
        c.reclaimed = self.mem.allocated.saturating_sub(self.mem.held());
        self.stats.note_static_peak(self.mem.static_peak());
        self.mem.reset();
        self.stack.clear();
        self.cleanups.clear();
        // The next evaluation's objects reuse this one's encodings.
        self.run.end_param = 0;
        self.exc.clear();
        c
    }

    /// A context nested in a bridge callback of the running one: runs on
    /// the same spaces above everything live, and frees what it allocated
    /// (frame objects by mark, heap allocations by context) at its end.
    /// Its counters are its own; the step budget continues the enclosing
    /// one's, and its calls count from the depth that started it.
    fn eval_nested(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: Job,
        object: Option<DeclId>,
        silent: bool,
        enum_check: bool,
        const_context: bool,
    ) -> (Result<ConstValueId, EvalFailure>, vm::Counters) {
        // The enclosing calls ran an evaluation a cached result would skip.
        self.run.impure += 1;
        let depth_base = self.active_calls() + 1;
        if depth_base >= self.budget.depth {
            let notes = if silent {
                Vec::new()
            } else {
                let loc = e.loc(bridge.ast());
                vec![Note::new(
                    loc,
                    format!(
                        "constexpr evaluation exceeded maximum depth of {} calls",
                        self.budget.depth
                    ),
                )]
            };
            let f = EvalFailure { kind: FailKind::DepthLimit, notes };
            return (Err(f), vm::Counters::default());
        }
        let run = (
            self.run.silent,
            self.run.enum_check,
            self.run.const_context,
            self.run.fold,
            self.run.cev_warn,
            self.run.plain,
            self.run.counters,
            self.run.object,
            self.run.maxd,
            self.run.depth_base,
            self.run.ub_check,
        );
        let mem = (
            self.mem.allocated,
            self.mem.peak,
            self.mem.arena_peak,
            self.mem.heap_allocs,
            self.mem.ctx,
            self.mem.cur_frame,
        );
        let sp = self.stack.len();
        let mark = self.mem.mark();
        let (live0, arena0, steps0, held0) =
            (self.mem.live_bytes(), self.mem.arena(), self.run.counters.steps, self.mem.held());
        // Registers above every live frame's.
        let fb = self
            .stack
            .iter()
            .map(|f| f.fb + self.funcs[f.func as usize].nregs + 1)
            .max()
            .unwrap_or(0);
        let ctx = self.ctx_depth;
        self.ctx_depth += 1;
        self.run.silent = silent;
        self.run.enum_check = enum_check;
        self.run.const_context = const_context;
        self.run.fold = silent && !const_context;
        self.run.cev_warn = std::mem::take(&mut self.cev_warn_next);
        self.run.plain = std::mem::take(&mut self.plain_next);
        self.run.ub_check = false;
        self.run.counters = vm::Counters { steps: steps0, ..vm::Counters::default() };
        self.run.maxd = 0;
        self.run.depth_base = depth_base;
        self.mem.allocated = 0;
        self.mem.peak = live0;
        self.mem.arena_peak = arena0;
        self.mem.heap_allocs = 0;
        self.mem.ctx = ctx;
        let r = self.run_job(bridge, e, object, fb);
        let steps1 = self.run.counters.steps;
        let c = vm::Counters {
            steps: steps1 - steps0,
            bytes: self.mem.allocated,
            peak: self.mem.peak.saturating_sub(live0),
            calls: self.run.counters.calls,
            arena: self.mem.arena_peak.saturating_sub(arena0),
            allocs: self.mem.heap_allocs,
            refl: self.run.counters.refl,
            reclaimed: self.mem.allocated.saturating_sub(self.mem.held().saturating_sub(held0)),
        };
        // The nested context's peak is its own (exclusive) and is restored
        // away below; the translation unit's high-water mark keeps it.
        self.mem.fold_peak();
        self.mem.free_heap_of(ctx, &self.types);
        self.mem.release(mark, &self.types);
        self.stack.truncate(sp);
        self.ctx_depth -= 1;
        let counters;
        (
            self.run.silent,
            self.run.enum_check,
            self.run.const_context,
            self.run.fold,
            self.run.cev_warn,
            self.run.plain,
            counters,
            self.run.object,
            self.run.maxd,
            self.run.depth_base,
            self.run.ub_check,
        ) = run;
        self.run.counters = vm::Counters { steps: steps1, ..counters };
        (
            self.mem.allocated,
            self.mem.peak,
            self.mem.arena_peak,
            self.mem.heap_allocs,
            self.mem.ctx,
            self.mem.cur_frame,
        ) = mem;
        (r, c)
    }
}

/// What one evaluation context cost on its own.
#[derive(Clone, Copy, Debug, Default)]
struct Cost {
    steps: u64,
    bytes: u64,
    peak: u64,
    arena: u64,
    calls: u64,
    allocs: u64,
    refl: u64,
    reclaimed: u64,
    frozen: u64,
    memo_hits: u64,
    wall_us: u64,
}

/// Whether `e` is a glvalue ([basic.lval]/1).
fn is_glvalue(ast: &Ast, e: ExprId) -> bool {
    ast.exprs.get(e).value_category().is_glvalue()
}
