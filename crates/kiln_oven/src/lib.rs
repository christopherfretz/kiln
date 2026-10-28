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

mod arith;
mod compile;
mod failure;
mod kbc;
mod mem;
mod memo;
mod notes;
pub mod report;
mod softfloat;
mod stats;
mod ty;
mod value;
mod vm;

use std::time::Instant;

use kiln_ast::consts::ConstStore;
use kiln_ast::types::RecordLayout;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, StmtId};
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
    /// template arguments, immediate invocations and `if constexpr`).
    pub fn new(kind: EvalKind, loc: Loc) -> Self {
        let enum_range_check =
            matches!(kind, EvalKind::TemplateArg | EvalKind::Consteval | EvalKind::IfConstexpr);
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

/// A class's virtual-function table (M3; unused in M2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VTableLayout {
    /// The final overrider of each slot.
    pub slots: Vec<DeclId>,
}

/// What the Oven needs from sema. The Oven never holds a borrow of the
/// AST across calls: sema may mutate it (instantiation on demand,
/// interning constants) between and during evaluations.
pub trait SemaBridge {
    /// The AST (re-borrowed per use, never held).
    fn ast(&self) -> &Ast;
    /// The body of function `f`'s definition, instantiating it if needed.
    fn ensure_body(&mut self, f: DeclId) -> Option<StmtId>;
    /// The layout of a complete record (via `kiln_abi`).
    fn ensure_layout(&mut self, record: DeclId) -> &RecordLayout;
    /// The vtable of a dynamic class (M3).
    fn vtable(&mut self, record: DeclId) -> &VTableLayout;
    /// The constant store results are frozen into.
    fn consts(&mut self) -> &mut ConstStore;
    /// Emits a diagnostic (unused by M2 evaluation, which returns notes).
    fn diag(&mut self, d: Diag);
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
    pub(crate) globals: Vec<Global>,
    pub(crate) global_map: FxHashMap<DeclId, u32>,
    /// Globals resolved to an unknown value because they had no
    /// initializer yet: re-resolved by the next evaluation.
    pub(crate) provisional: Vec<u32>,
    /// String literal objects of evaluated `StringLiteral` exprs, one per
    /// expression ([lex.string]/9: distinct evaluations need not share).
    pub(crate) lit_exprs: FxHashMap<ExprId, u32>,
    pub(crate) thawed: FxHashMap<ConstValueId, u64>,
    pub(crate) mem: Memory,
    pub(crate) regs: Vec<u128>,
    pub(crate) stack: Vec<Frame>,
    pub(crate) memo: Memo,
    pub(crate) stats: EvalStats,
    pub(crate) run: vm::RunState,
    pub(crate) scratch: value::Scratch,
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
            globals: Vec::new(),
            global_map: FxHashMap::default(),
            provisional: Vec::new(),
            lit_exprs: FxHashMap::default(),
            thawed: FxHashMap::default(),
            mem: Memory::new(budget.bytes),
            regs: vec![0; 256],
            stack: Vec::with_capacity(64),
            memo: Memo::new(budget.memo_entries),
            stats: EvalStats::default(),
            run: vm::RunState::default(),
            scratch: value::Scratch::default(),
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
        let start = Instant::now();
        let r = self.eval_top(bridge, e, ctx.object, false, ctx.enum_range_check);
        let wall_us = start.elapsed().as_micros() as u64;
        let c = self.run.counters;
        self.stats.push(EvalRecord {
            loc: ctx.loc,
            kind: ctx.kind,
            steps: c.steps,
            bytes: c.bytes,
            peak_bytes: c.peak,
            calls: c.calls,
            allocs: 0,
            wall_us,
            ok: r.is_ok(),
        });
        r
    }

    /// Folds `e` if it is a constant expression; never reports anything
    /// and is not recorded in the statistics.
    pub fn try_fold(&mut self, bridge: &mut dyn SemaBridge, e: ExprId) -> Option<ConstValueId> {
        self.eval_top(bridge, e, None, true, false).ok()
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
        self.eval_top(bridge, init, Some(var), true, false).is_ok()
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
            + cap(&self.globals)
            + map(&self.global_map)
            + cap(&self.provisional)
            + map(&self.lit_exprs)
            + map(&self.thawed)
            + self.mem.footprint()
            + cap(&self.regs)
            + cap(&self.stack)
            + self.memo.footprint()
            + self.run.footprint()
            + self.scratch.footprint()
    }

    /// The disassembly of everything compiled so far (debugging aid).
    pub fn disassemble(&self) -> String {
        kbc::disassemble(&self.code, 0, self.code.len())
    }

    fn eval_top(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: ExprId,
        object: Option<DeclId>,
        silent: bool,
        enum_check: bool,
    ) -> Result<ConstValueId, EvalFailure> {
        for g in self.provisional.drain(..) {
            self.globals[g as usize].state = compile::GState::Unresolved;
        }
        self.run.silent = silent;
        self.run.enum_check = enum_check;
        self.run.counters = vm::Counters::default();
        self.mem.allocated = 0;
        self.mem.peak = 0;
        let r = self.eval_inner(bridge, e, object);
        self.run.counters.bytes = self.mem.allocated;
        self.run.counters.peak = self.mem.peak;
        self.mem.reset();
        self.stack.clear();
        r
    }
}
