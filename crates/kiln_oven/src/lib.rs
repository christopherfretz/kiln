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
mod compile;
mod complex;
mod failure;
mod fltstr;
mod kbc;
mod mem;
mod memo;
mod memptr;
mod notes;
pub mod report;
mod softfloat;
mod stats;
mod ty;
mod value;
mod vm;

use std::time::Instant;

pub use kiln_ast::class::{Subobject, SubobjectId};
use kiln_ast::consts::ConstStore;
use kiln_ast::types::RecordLayout;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, QualType, ReflectionId, StmtId};
use kiln_diag::Diag;
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

pub use failure::{EvalFailure, FailKind, Note};
pub use stats::{EvalRecord, EvalStats, EvalTotals};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        EvalContext { kind, loc, object: None, enum_range_check }
    }

    /// The initializer of the `constexpr` variable `var` (enumeration
    /// ranges are checked; clear [`EvalContext::enum_range_check`] for a
    /// `constinit` variable that is not `constexpr`).
    pub fn init(var: DeclId, loc: Loc) -> Self {
        EvalContext { kind: EvalKind::Init, loc, object: Some(var), enum_range_check: true }
    }

    /// A converted constant expression ([expr.const]/12) that is not a
    /// template argument (a `case` label, an enumerator of an enumeration
    /// with a fixed underlying type): reported as [`EvalKind::Other`],
    /// with enumeration ranges checked.
    pub fn converted(loc: Loc) -> Self {
        EvalContext { kind: EvalKind::Other, loc, object: None, enum_range_check: true }
    }

    /// `self` with [`EvalContext::enum_range_check`] set to `on`.
    #[must_use]
    pub fn with_enum_range_check(mut self, on: bool) -> Self {
        self.enum_range_check = on;
        self
    }
}

/// A reflection intrinsic the Oven evaluates natively (spec/oven.md
/// "Reflection runtime"; WP-267 spike, `-freflection`): a call of a
/// function the bridge recognizes by its declaration
/// ([`SemaBridge::meta_intrinsic`]) compiles to `Op::Builtin` (`bi::META`)
/// and runs through the `reflect_*`/`define_member` bridge methods. Every
/// one is impure: a call that reached one is never memoized.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MetaOp {
    /// `std::meta::type_of(r)`: the type of the entity `r` reflects.
    TypeOf,
    /// `std::meta::name_of(r)`: its name, as a `const char *` into a
    /// frozen string literal object (the spike's stand-in for
    /// `std::string_view`).
    NameOf,
    /// `std::meta::__kiln_members_count(r)`: the number of members of the
    /// class `r` reflects (completing it on demand).
    MembersCount,
    /// `std::meta::__kiln_member_at(r, i)`: its `i`th member, in
    /// declaration order.
    MemberAt,
    /// `__kiln_define_member(cls, type, name)`: adds a non-static data
    /// member to the class under definition (the spike's experiment).
    DefineMember,
}

impl MetaOp {
    const ALL: [MetaOp; 5] = [
        MetaOp::TypeOf,
        MetaOp::NameOf,
        MetaOp::MembersCount,
        MetaOp::MemberAt,
        MetaOp::DefineMember,
    ];

    /// The op's code in the instruction's `sub` field.
    pub(crate) fn code(self) -> u32 {
        self as u32
    }

    /// The op of `code`.
    pub(crate) fn from_code(code: u32) -> Option<MetaOp> {
        Self::ALL.get(code as usize).copied()
    }
}

/// What the Oven needs from sema. The Oven never holds a borrow of the
/// AST across calls: sema may mutate it (instantiation on demand,
/// interning constants) between and during evaluations.
pub trait SemaBridge {
    /// The AST (re-borrowed per use, never held).
    fn ast(&self) -> &Ast;
    /// The body of function `f`'s definition, instantiating it if needed;
    /// for an implicit or defaulted special member this defines it on
    /// demand (sema's `define_implicit_*`, spec/sema-class.md §4.5).
    /// Called while an evaluation runs (the first call of `f`); sema may
    /// evaluate nested constant expressions with `oven` meanwhile
    /// (R-OVEN-1: spec/oven.md "Nested evaluation").
    fn ensure_body(&mut self, f: DeclId, oven: &mut Oven) -> Option<StmtId>;
    /// Whether the translation unit is C++26 or later (the rules that
    /// depend on the language mode: P2747 placement new outside namespace
    /// std). The default is C++26.
    fn cxx26(&self) -> bool {
        true
    }
    /// The layout of a complete record (via `kiln_abi`).
    fn ensure_layout(&mut self, record: DeclId) -> &RecordLayout;
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

    // Reflection (WP-267 spike, `-freflection`; spec/oven.md "Reflection
    // runtime"). A failure is the notes explaining it (the first is the
    // reason, reported at its location); no notes is a silent failure.
    // The defaults (a bridge without reflection) recognize nothing.

    /// The reflection intrinsic `f` is, if any (consulted when a direct
    /// call of `f` is compiled).
    fn meta_intrinsic(&self, f: DeclId) -> Option<MetaOp> {
        let _ = f;
        None
    }

    /// `type_of(r)` for the call at `loc`.
    ///
    /// # Errors
    /// If `r` does not reflect an entity with a type.
    fn reflect_type_of(&mut self, r: ReflectionId, loc: Loc) -> Result<ReflectionId, Vec<Note>> {
        let _ = (r, loc);
        Err(Vec::new())
    }

    /// `name_of(r)`: the frozen `const char *` value pointing to a string
    /// literal object holding the name.
    ///
    /// # Errors
    /// If `r` reflects nothing with a name.
    fn reflect_name(&mut self, r: ReflectionId, loc: Loc) -> Result<ConstValueId, Vec<Note>> {
        let _ = (r, loc);
        Err(Vec::new())
    }

    /// The members of the class `r` reflects, in declaration order,
    /// completing it first if it is an implicitly instantiable
    /// specialization (with `oven` lent back to sema, as `ensure_body`).
    ///
    /// # Errors
    /// If `r` does not reflect a complete class type.
    fn reflect_members(
        &mut self,
        r: ReflectionId,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<Vec<ReflectionId>, Vec<Note>> {
        let _ = (r, loc, oven);
        Err(Vec::new())
    }

    /// Adds the non-static data member `name` of type `ty` to the class
    /// `cls` (which must be the class under definition) and returns its
    /// reflection: the AST MUTATES while this evaluation runs. Only in a
    /// manifestly constant-evaluated context (never in a speculative
    /// fold). `oven` is lent back to sema for nested evaluations.
    ///
    /// # Errors
    /// If the context does not permit it, an operand is not what is
    /// required, or `name` is already a member.
    fn define_member(
        &mut self,
        cls: ReflectionId,
        ty: ReflectionId,
        name: &str,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<ReflectionId, Vec<Note>> {
        let _ = (cls, ty, name, loc, oven);
        Err(Vec::new())
    }
}

/// The constant evaluator of one translation unit.
#[derive(Debug)]
pub struct Oven {
    pub(crate) budget: Budget,
    /// Call frames shown in a failure's backtrace (0: all).
    pub(crate) backtrace_limit: usize,
    pub(crate) types: Types,
    pub(crate) code: Vec<u32>,
    pub(crate) sites: Vec<Site>,
    pub(crate) step_locs: Vec<Loc>,
    pub(crate) origins: Vec<mem::Origin>,
    pub(crate) traps: Vec<TrapInfo>,
    pub(crate) funcs: Vec<Func>,
    pub(crate) func_map: FxHashMap<DeclId, u32>,
    pub(crate) thunks: FxHashMap<ExprId, u32>,
    /// Thunks compiled inside a default argument use (the key's second
    /// half, [`Oven::fold_immediate_in`]): their source-location builtins
    /// take that use's values.
    pub(crate) thunks_at: FxHashMap<(ExprId, ExprId), u32>,
    /// The default argument use the next thunk is compiled in.
    pub(crate) srcloc_root: Option<ExprId>,
    pub(crate) globals: Vec<Global>,
    pub(crate) global_map: FxHashMap<DeclId, u32>,
    /// Globals resolved to an unknown value because they had no
    /// initializer yet: re-resolved by the next evaluation.
    pub(crate) provisional: Vec<u32>,
    /// Globals whose resolution depends on the lax mode
    /// ([`value::VarInfo::lax`]), and the mode they were resolved in.
    pub(crate) lax_globals: Vec<u32>,
    pub(crate) lax_resolved: bool,
    /// String literal objects of evaluated `StringLiteral` exprs, one per
    /// expression ([lex.string]/9: distinct evaluations need not share).
    pub(crate) lit_exprs: FxHashMap<ExprId, u32>,
    pub(crate) thawed: FxHashMap<ConstValueId, u64>,
    /// The static objects of thawed lifetime-extended temporaries.
    pub(crate) lets: FxHashMap<DeclId, u64>,
    /// The static temporaries a frozen result points to, with their
    /// objects: frozen into `LetData.value` once the result is.
    pub(crate) let_pending: Vec<(DeclId, u64)>,
    pub(crate) mem: Memory,
    pub(crate) regs: Vec<u128>,
    pub(crate) stack: Vec<Frame>,
    /// Registered destructions, innermost last (`Op::PushCleanup`).
    pub(crate) cleanups: Vec<vm::Cleanup>,
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
}

/// The inclusive cost of the nested evaluations run so far by the running
/// context (subtracted from its own record).
#[derive(Clone, Copy, Debug, Default)]
struct Inner {
    steps: u64,
    frozen: u64,
    wall_us: u64,
}

impl Oven {
    /// A new evaluator with the given limits.
    pub fn new(budget: Budget) -> Self {
        Oven {
            budget,
            backtrace_limit: notes::BACKTRACE_LIMIT,
            types: Types::default(),
            code: Vec::new(),
            sites: Vec::new(),
            step_locs: Vec::new(),
            origins: Vec::new(),
            traps: Vec::new(),
            funcs: Vec::new(),
            func_map: FxHashMap::default(),
            thunks: FxHashMap::default(),
            thunks_at: FxHashMap::default(),
            srcloc_root: None,
            globals: Vec::new(),
            global_map: FxHashMap::default(),
            provisional: Vec::new(),
            lax_globals: Vec::new(),
            lax_resolved: false,
            lit_exprs: FxHashMap::default(),
            thawed: FxHashMap::default(),
            lets: FxHashMap::default(),
            let_pending: Vec::new(),
            mem: Memory::new(budget.bytes),
            regs: vec![0; 256],
            stack: Vec::with_capacity(64),
            cleanups: Vec::new(),
            memo: Memo::new(budget.memo_entries),
            stats: EvalStats::default(),
            run: vm::RunState::default(),
            scratch: value::Scratch::default(),
            mp: memptr::MemPtrs::default(),
            virt: vm::Virt::default(),
            ctx_depth: 0,
            inner: Inner::default(),
        }
    }

    /// Interns an allocation origin for `Alloc` instructions.
    pub(crate) fn push_origin(&mut self, o: mem::Origin) -> u32 {
        let i = self.origins.len() as u32;
        self.origins.push(o);
        i
    }

    /// Sets how many call frames a failure's backtrace shows
    /// (`-fconstexpr-backtrace-limit=`; clang's default is 10, 0 shows
    /// all). Elided frames are summarized in one "skipping" note.
    pub fn set_backtrace_limit(&mut self, n: usize) {
        self.backtrace_limit = n;
    }

    /// The backtrace limit ([`Oven::set_backtrace_limit`]).
    pub fn backtrace_limit(&self) -> usize {
        self.backtrace_limit
    }

    /// The limits.
    pub fn budget(&self) -> Budget {
        self.budget
    }

    /// Evaluates the constant expression `e` (a manifestly
    /// constant-evaluated context; recorded in [`Oven::stats`]).
    pub fn evaluate(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: ExprId,
        ctx: EvalContext,
    ) -> Result<ConstValueId, EvalFailure> {
        let (r, c) = self.eval_ctx(bridge, e, ctx.object, false, ctx.enum_range_check, true);
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
            wall_us: c.wall_us,
            ok: r.is_ok(),
        });
        r
    }

    /// Folds `e` if it is a constant expression; never reports anything
    /// and is not recorded in the statistics. The evaluation is a trial,
    /// not manifestly constant-evaluated: `__builtin_is_constant_evaluated()`
    /// yields false.
    pub fn try_fold(&mut self, bridge: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId> {
        self.eval_ctx(bridge, e, None, true, false, false).0.ok()
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
        let r = self.eval_ctx(bridge, e, None, true, false, false).0.ok();
        self.run.lax = false;
        r
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
        let r = self.eval_ctx(bridge, e, None, true, false, true).0.ok();
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
        self.eval_ctx(bridge, e, Some(var), true, false, true).0.ok()
    }

    /// Whether `var`'s initializer is a constant initializer
    /// ([basic.start.static]/2): it evaluates successfully as a constant
    /// expression. Not recorded in the statistics.
    pub fn is_constant_initialized(&mut self, bridge: &mut dyn SemaBridge, var: DeclId) -> bool {
        use kiln_ast::decl::VarEvalState;
        let ast = bridge.ast();
        let Some(data) = ast.decls.var(var) else { return false };
        match data.eval.state() {
            VarEvalState::Value(_) => return true,
            VarEvalState::NotConstant => return false,
            _ => {}
        }
        let Some(init) = data.init else { return false };
        self.eval_ctx(bridge, init, Some(var), true, false, true).0.is_ok()
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
        self.begin_outermost(true, false, const_context);
        let r = self.destroy_var(bridge, var, value);
        self.end_outermost();
        r
    }

    /// Per-evaluation records and totals.
    pub fn stats(&self) -> &EvalStats {
        &self.stats
    }

    /// Bytes of heap the evaluator holds (capacities of its code, abstract
    /// memory, registers, stacks, caches and scratch buffers). Compiling
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
            + map(&self.thunks)
            + map(&self.thunks_at)
            + cap(&self.globals)
            + map(&self.global_map)
            + cap(&self.provisional)
            + cap(&self.lax_globals)
            + map(&self.lit_exprs)
            + map(&self.thawed)
            + map(&self.lets)
            + cap(&self.let_pending)
            + self.mem.footprint()
            + cap(&self.regs)
            + cap(&self.stack)
            + cap(&self.cleanups)
            + self.memo.footprint()
            + self.run.footprint()
            + self.scratch.footprint()
            + self.mp.footprint()
            + self.virt.footprint()
    }

    /// The disassembly of everything compiled so far (debugging aid).
    pub fn disassemble(&self) -> String {
        kbc::disassemble(&self.code, 0, self.code.len())
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
        let start = Instant::now();
        self.types.refresh(bridge);
        let frozen0 = bridge.consts().content_bytes();
        let inner0 = std::mem::take(&mut self.inner);
        let (r, c) = if self.ctx_depth == 0 {
            self.eval_outermost(bridge, e, object, silent, enum_check, const_context)
        } else {
            self.eval_nested(bridge, e, object, silent, enum_check, const_context)
        };
        let wall_us = start.elapsed().as_micros() as u64;
        let frozen = bridge.consts().content_bytes().saturating_sub(frozen0);
        let inner = std::mem::replace(&mut self.inner, inner0);
        if self.ctx_depth > 0 {
            self.inner.steps += c.steps;
            self.inner.frozen += frozen;
            self.inner.wall_us += wall_us;
        }
        let cost = Cost {
            steps: c.steps.saturating_sub(inner.steps),
            bytes: c.bytes,
            peak: c.peak,
            arena: c.arena,
            calls: c.calls,
            allocs: c.allocs,
            frozen: frozen.saturating_sub(inner.frozen),
            wall_us: wall_us.saturating_sub(inner.wall_us),
        };
        (r, cost)
    }

    /// The outermost context: owns the frame and heap spaces, reset at its
    /// end.
    fn eval_outermost(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: ExprId,
        object: Option<DeclId>,
        silent: bool,
        enum_check: bool,
        const_context: bool,
    ) -> (Result<ConstValueId, EvalFailure>, vm::Counters) {
        self.begin_outermost(silent, enum_check, const_context);
        let r = self.eval_inner(bridge, e, object, 0);
        let c = self.end_outermost();
        (r, c)
    }

    /// Starts the outermost context: globals resolved provisionally (or
    /// for the other strictness) are forgotten, the counters reset.
    fn begin_outermost(&mut self, silent: bool, enum_check: bool, const_context: bool) {
        for g in self.provisional.drain(..) {
            self.globals[g as usize].state = compile::GState::Unresolved;
        }
        if self.lax_resolved != self.run.lax {
            self.lax_resolved = self.run.lax;
            for g in self.lax_globals.drain(..) {
                self.globals[g as usize].state = compile::GState::Unresolved;
            }
        }
        self.run.silent = silent;
        self.run.enum_check = enum_check;
        self.run.const_context = const_context;
        self.run.counters = vm::Counters::default();
        self.run.nesting = 0;
        self.run.depth_base = 0;
        self.mem.allocated = 0;
        self.mem.peak = 0;
        self.mem.arena_peak = 0;
        self.mem.heap_allocs = 0;
        self.mem.ctx = 0;
        self.ctx_depth = 1;
    }

    /// Ends the outermost context: its cost; the frame and heap spaces
    /// are reset.
    fn end_outermost(&mut self) -> vm::Counters {
        self.ctx_depth = 0;
        let mut c = self.run.counters;
        c.bytes = self.mem.allocated;
        c.peak = self.mem.peak;
        c.arena = self.mem.arena_peak;
        c.allocs = self.mem.heap_allocs;
        self.stats.note_static_peak(self.mem.static_peak());
        self.mem.reset();
        self.stack.clear();
        self.cleanups.clear();
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
        e: ExprId,
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
                let loc = bridge.ast().exprs.get(e).loc();
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
            self.run.counters,
            self.run.object,
            self.run.maxd,
            self.run.depth_base,
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
        let (live0, arena0, steps0) =
            (self.mem.live_bytes(), self.mem.arena(), self.run.counters.steps);
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
        self.run.counters = vm::Counters { steps: steps0, ..vm::Counters::default() };
        self.run.maxd = 0;
        self.run.depth_base = depth_base;
        self.mem.allocated = 0;
        self.mem.peak = live0;
        self.mem.arena_peak = arena0;
        self.mem.heap_allocs = 0;
        self.mem.ctx = ctx;
        let r = self.eval_inner(bridge, e, object, fb);
        let steps1 = self.run.counters.steps;
        let c = vm::Counters {
            steps: steps1 - steps0,
            bytes: self.mem.allocated,
            peak: self.mem.peak.saturating_sub(live0),
            calls: self.run.counters.calls,
            arena: self.mem.arena_peak.saturating_sub(arena0),
            allocs: self.mem.heap_allocs,
        };
        self.mem.free_heap_of(ctx, &self.types);
        self.mem.release(mark, &self.types);
        self.stack.truncate(sp);
        self.ctx_depth -= 1;
        let counters;
        (
            self.run.silent,
            self.run.enum_check,
            self.run.const_context,
            counters,
            self.run.object,
            self.run.maxd,
            self.run.depth_base,
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
    frozen: u64,
    wall_us: u64,
}
