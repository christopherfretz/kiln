//! The snapshot encoding of the [`Oven`] (spec/oven.md "Snapshots",
//! spec/snap.md "Configured roots").
//!
//! The Oven is a configured root: its limits and options come from the
//! consumer's command line, so it is not `Snap`. The consumer builds an
//! Oven exactly as without a snapshot ([`Oven::new`] and the setters) and
//! applies the snapshot to it ([`Oven::snap_restore`]). A snapshot is
//! taken between evaluations only ([`Oven::snap_ready`]): the frame and
//! heap spaces, the call stack and every per-evaluation buffer are then
//! empty, and the state is the compiled code, the type table, the
//! globals with the static memory their values live in, the memo and
//! the reflection tables.
//!
//! `Oven`:
//!
//! | field | class | how |
//! |---|---|---|
//! | `types` | state | as data, its subobject path cache refilled on demand |
//! | `code`, `padded`, `sites`, `step_locs`, `origins`, `traps` | state | the bytecode and its side tables, as data |
//! | `funcs`, `func_map`, `base_funcs`, `ctor_steps` | state | as data: a function compiled before the snapshot is not compiled again |
//! | `volatile_decls`, `volatile_exprs` | state | as data |
//! | `thunks`, `thunks_at`, `last_thunk` | state | as data |
//! | `globals`, `global_map`, `provisional`, `provisional_objs` | state | as data |
//! | `addr_int_disps`, `synth`, `label_diffs`, `label_diff_cells` | state | as data (C static initializer addresses; pointers in memory index `synth` and `label_diffs`) |
//! | `lit_exprs`, `thawed`, `lets`, `unknowns` | state | as data |
//! | `frame` | state + transient | the compiled stubs as data; the running job at rest |
//! | `mem` | state + transient | [`Memory`] (`mem/snap.rs`): the static space as data, the frame and heap spaces at rest |
//! | `exc` | state + transient | the unwind entry and the non-throwing cache as data; no exception in flight or caught |
//! | `memo` | state | as data; its capacity must be the consumer's `Budget::memo_entries` |
//! | `mp`, `virt` | state | as data (interned member pointers; subobject graphs and resolved virtual calls) |
//! | `meta` | state + transient | the compiled intrinsic call sites and `info`'s type as data; the result buffer and a pending throw reset |
//! | `refl_seen` | state | as data (reflection bytes already charged) |
//! | `dms` | state + transient | the data member descriptions, annotation lists and the reader's plan and layouts as data; reused buffers reset |
//! | `budget`, `backtrace_limit` | configuration | `-fconstexpr-steps`, `-fconstexpr-depth`, `-fconstexpr-memory`, the memo size, `-fconstexpr-backtrace-limit` |
//! | `clang_compat`, `cxx_year` | configuration | `-fclang-compat=`, `-std=` (sema also sets both before every evaluation) |
//! | `hooks` | configuration + transient | the trace and profile switches and sink (`-ftime-trace`, `-fconstexpr-stats`, `-fconstexpr-profile`); no call mark open |
//! | `cev_warn_next`, `ub_check_next`, `contracts_off_next`, `potential_next`, `plain_next`, `void_next`, `addr_next`, `unchecked_next` | transient, at rest | set by a caller just before one evaluation: false / `None` |
//! | `fold_float_ub`, `addr_int`, `fold_leaks`, `fold_c_static` | transient, at rest | set by codegen around its folds: false |
//! | `srcloc_root`, `bound_obj`, `let_pending` | transient, at rest | `None`, empty |
//! | `stack`, `cleanups`, `ctx_depth` | transient, at rest | empty, 0: no evaluation runs |
//! | `run` | transient, at rest | no memoizable call pending; its flags and counters are set when an evaluation starts |
//! | `contract_violated`, `msg_chars`, `subst_binds` | transient | the last evaluation's outputs and inputs, reset |
//! | `regs`, `scratch`, `inner` | transient | registers and reused buffers, reset |
//! | `umemo` | rebuilt | the union-path memo, a cache refilled on demand |
//! | `stats` | reset | `-fconstexpr-stats`/`-fconstexpr-report` records: reporting only (the consumer reports its own evaluations) |
//!
//! Validated on read (an error, never a panic or a later out-of-range
//! index): every function entry inside `code`, every function, global
//! and type index a table holds in range, the memo's tables and
//! recency list, the static space's maps, slots and free lists, the
//! pointer marks' tags, and the bytecode (`kbc/verify.rs`, WP-1396):
//! opcodes, instruction boundaries, registers within the owning
//! function's frame, jump targets, and every function, global, type,
//! trap, step-location and origin operand; the `Unwind` entry, the call
//! sites and the non-throwing cache. The AST ids the tables and the code
//! hold are range-checked against the AST after the whole restore
//! (`kiln_ast::Ast::check_ids`; the code's are noted on the reader).
//! Not validated: the pointers stored in static memory and in globals,
//! which are checked at every use as any pointer the evaluator computes
//! (`Memory::alloc_of`: slot, generation, liveness; bounds at access).

use kiln_ast::{ConstValueId, DeclId, ExprId};
use kiln_snap::{Desc, Reader, Schema, SchemaHash, Snap, SnapError, Writer};
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::compile::{FState, Func, GKind, Global, Site, TrapInfo, UNCOMPILED};
use crate::frame::FrameState;
use crate::mem::{self, Memory};
use crate::memo::Memo;
use crate::memptr::MemPtrs;
use crate::meta::define::DmsTable;
use crate::meta::intern::ReflSeen;
use crate::meta::materialize::{MetaState, Recipe};
use crate::reclaim::LastThunk;
use crate::ty::{OTy, Types};
use crate::vm::Virt;
use crate::vm::exc::Excs;
use crate::{CtorStep, Oven};

/// The decoded state fields, validated together before any is assigned.
struct Parts {
    types: Types,
    code: Vec<u32>,
    padded: usize,
    sites: Vec<Site>,
    step_locs: Vec<Loc>,
    origins: Vec<mem::Origin>,
    traps: Vec<TrapInfo>,
    funcs: Vec<Func>,
    func_map: FxHashMap<DeclId, u32>,
    base_funcs: FxHashMap<DeclId, u32>,
    ctor_steps: FxHashMap<u32, Box<[CtorStep]>>,
    volatile_decls: FxHashSet<DeclId>,
    volatile_exprs: FxHashSet<ExprId>,
    thunks: FxHashMap<(ExprId, Option<DeclId>, bool), u32>,
    thunks_at: FxHashMap<(ExprId, ExprId, bool), u32>,
    last_thunk: Option<LastThunk>,
    globals: Vec<Global>,
    global_map: FxHashMap<DeclId, u32>,
    provisional: Vec<u32>,
    provisional_objs: FxHashMap<u32, (OTy, u64)>,
    addr_int_disps: FxHashMap<(mem::Sp, u64), i64>,
    synth: std::cell::RefCell<Vec<(u64, i64)>>,
    label_diffs: Vec<(ExprId, ExprId)>,
    label_diff_cells: FxHashMap<(mem::Sp, u64), (u32, i64)>,
    lit_exprs: FxHashMap<ExprId, u32>,
    thawed: FxHashMap<ConstValueId, u64>,
    lets: FxHashMap<DeclId, u64>,
    unknowns: FxHashMap<DeclId, u64>,
    frame: FrameState,
    mem: Memory,
    exc: Excs,
    memo: Memo,
    mp: MemPtrs,
    virt: Virt,
    meta: MetaState,
    refl_seen: ReflSeen,
    dms: DmsTable,
}

/// Adds `field` to a decoding error's path.
fn at<T>(field: &'static str, r: Result<T, SnapError>) -> Result<T, SnapError> {
    r.map_err(|e| e.within(field))
}

impl Oven {
    /// The quiescence check (spec/oven.md "Snapshots"): `Err` names the
    /// first transient field that is not at rest, as `Oven.field: why`.
    /// A snapshot is taken between evaluations only.
    ///
    /// # Errors
    /// While an evaluation runs (a bridge callback asking), while
    /// potential-constant frame objects are tracked, or while a caller's
    /// option for the next evaluation is still set.
    pub fn snap_ready(&self) -> Result<(), String> {
        let busy = |what: &str| Err(format!("Oven.{what}"));
        if self.ctx_depth != 0 {
            return busy(&format!("ctx_depth: {} evaluations are running", self.ctx_depth));
        }
        if !self.stack.is_empty() || !self.cleanups.is_empty() {
            return busy("stack: call frames are live");
        }
        if !self.run.at_rest() {
            return busy("run: a memoizable call is pending");
        }
        if !self.exc.inflight.is_empty() || !self.exc.caught.is_empty() || self.exc.base != (0, 0) {
            return busy("exc: an exception is in flight or being handled");
        }
        if let Some(i) = self.funcs.iter().position(|f| f.state == FState::Compiling) {
            return busy(&format!("funcs: function {i} is being compiled"));
        }
        let next = [
            ("cev_warn_next", self.cev_warn_next),
            ("ub_check_next", self.ub_check_next),
            ("contracts_off_next", self.contracts_off_next),
            ("potential_next", self.potential_next.is_some()),
            ("plain_next", self.plain_next),
            ("void_next", self.void_next),
            ("addr_next", self.addr_next),
            ("unchecked_next", self.unchecked_next),
            ("fold_float_ub", self.fold_float_ub),
            ("addr_int", self.addr_int),
            ("fold_leaks", self.fold_leaks),
            ("fold_c_static", self.fold_c_static),
            ("srcloc_root", self.srcloc_root.is_some()),
            ("bound_obj", self.bound_obj.is_some()),
            ("let_pending", !self.let_pending.is_empty()),
        ];
        if let Some((name, _)) = next.iter().find(|(_, set)| *set) {
            return busy(&format!("{name}: set for an evaluation that has not run"));
        }
        if !self.hooks.at_rest() {
            return busy("hooks: a call mark is open");
        }
        self.frame.snap_ready().or_else(busy)?;
        self.mem.snap_ready().map_err(|why| format!("Oven.mem > {why}"))
    }

    /// Writes the Oven's state (see the module docs) after
    /// [`Oven::snap_ready`]; writes nothing when that fails. A
    /// `Writer::canonical()` writer gives the bytes two equal states share.
    ///
    /// # Errors
    /// [`Oven::snap_ready`]'s.
    pub fn snap_put(&self, w: &mut Writer) -> Result<(), String> {
        self.snap_ready()?;
        self.put_state(w, &mut |_, _| {});
        Ok(())
    }

    /// The encoded size of each state field ([`Oven::snap_put`]'s
    /// order), in bytes: what a snapshot of this Oven spends where.
    ///
    /// # Errors
    /// [`Oven::snap_ready`]'s.
    pub fn snap_sizes(&self) -> Result<Vec<(&'static str, usize)>, String> {
        self.snap_ready()?;
        let mut w = Writer::new();
        let mut sizes = Vec::new();
        let mut last = 0;
        self.put_state(&mut w, &mut |field, end| {
            sizes.push((field, end - last));
            last = end;
        });
        Ok(sizes)
    }

    /// Writes the state fields, calling `mark` with each field's name and
    /// the writer's position after it.
    fn put_state(&self, w: &mut Writer, mark: &mut dyn FnMut(&'static str, usize)) {
        let Oven {
            budget: _,
            backtrace_limit: _,
            clang_compat: _,
            cxx_year: _,
            cev_warn_next: _,
            ub_check_next: _,
            contracts_off_next: _,
            contract_violated: _,
            potential_next: _,
            plain_next: _,
            void_next: _,
            addr_next: _,
            unchecked_next: _,
            types,
            code,
            padded,
            sites,
            step_locs,
            origins,
            traps,
            funcs,
            func_map,
            base_funcs,
            ctor_steps,
            volatile_decls,
            volatile_exprs,
            thunks,
            thunks_at,
            srcloc_root: _,
            last_thunk,
            globals,
            bound_obj: _,
            global_map,
            provisional,
            provisional_objs,
            fold_float_ub: _,
            addr_int: _,
            fold_leaks: _,
            fold_c_static: _,
            addr_int_disps,
            synth,
            label_diffs,
            label_diff_cells,
            lit_exprs,
            thawed,
            lets,
            unknowns,
            let_pending: _,
            msg_chars: _,
            subst_binds: _,
            frame,
            mem,
            umemo: _,
            regs: _,
            stack: _,
            cleanups: _,
            exc,
            memo,
            stats: _,
            run: _,
            scratch: _,
            mp,
            virt,
            ctx_depth: _,
            inner: _,
            meta,
            refl_seen,
            dms,
            hooks: _,
        } = self;
        macro_rules! put {
            ($($f:ident),*) => {$(
                $f.put(w);
                mark(stringify!($f), w.position());
            )*};
        }
        put!(
            types,
            code,
            padded,
            sites,
            step_locs,
            origins,
            traps,
            funcs,
            func_map,
            base_funcs,
            ctor_steps,
            volatile_decls,
            volatile_exprs,
            thunks,
            thunks_at,
            last_thunk,
            globals,
            global_map,
            provisional,
            provisional_objs,
            addr_int_disps,
            synth,
            label_diffs,
            label_diff_cells,
            lit_exprs,
            thawed,
            lets,
            unknowns,
            frame
        );
        mem.snap_put(w);
        mark("mem", w.position());
        put!(exc, memo, mp, virt, meta, refl_seen, dms);
    }

    /// Whether nothing was compiled, interned, allocated or evaluated
    /// yet: what [`Oven::snap_restore`] accepts.
    fn is_fresh(&self) -> bool {
        self.code.is_empty()
            && self.funcs.is_empty()
            && self.globals.is_empty()
            && self.types.infos.is_empty()
            && self.mem.statics.is_empty()
            && self.memo.hits == 0
            && self.memo.misses == 0
            && self.stats.jobs().jobs == 0
            && self.ctx_depth == 0
    }

    /// Applies a snapshot [`Oven::snap_put`] wrote to this Oven, which the
    /// consumer built and configured ([`Oven::new`] with its budget, the
    /// setters for its options) but has not used. Every field is decoded
    /// and validated before any is assigned, so on an error the Oven is
    /// unchanged. The AST, type and constant ids the state holds are the
    /// producer's: the AST must be restored from the same snapshot.
    ///
    /// # Errors
    /// Bad or truncated input; an Oven already used; a snapshot whose
    /// memo capacity is not this Oven's (`Budget::memo_entries`).
    pub fn snap_restore(&mut self, r: &mut Reader<'_>) -> Result<(), SnapError> {
        if !self.is_fresh() {
            return Err(r.invalid::<Oven>("snapshot applied to an Oven already used"));
        }
        let types: Types = at("Oven.types", r.get())?;
        let ntypes = types.infos.len();
        let parts = Parts {
            types,
            code: at("Oven.code", r.get())?,
            padded: at("Oven.padded", r.get())?,
            sites: at("Oven.sites", r.get())?,
            step_locs: at("Oven.step_locs", r.get())?,
            origins: at("Oven.origins", r.get())?,
            traps: at("Oven.traps", r.get())?,
            funcs: at("Oven.funcs", r.get())?,
            func_map: at("Oven.func_map", r.get())?,
            base_funcs: at("Oven.base_funcs", r.get())?,
            ctor_steps: at("Oven.ctor_steps", r.get())?,
            volatile_decls: at("Oven.volatile_decls", r.get())?,
            volatile_exprs: at("Oven.volatile_exprs", r.get())?,
            thunks: at("Oven.thunks", r.get())?,
            thunks_at: at("Oven.thunks_at", r.get())?,
            last_thunk: at("Oven.last_thunk", r.get())?,
            globals: at("Oven.globals", r.get())?,
            global_map: at("Oven.global_map", r.get())?,
            provisional: at("Oven.provisional", r.get())?,
            provisional_objs: at("Oven.provisional_objs", r.get())?,
            addr_int_disps: at("Oven.addr_int_disps", r.get())?,
            synth: at("Oven.synth", r.get())?,
            label_diffs: at("Oven.label_diffs", r.get())?,
            label_diff_cells: at("Oven.label_diff_cells", r.get())?,
            lit_exprs: at("Oven.lit_exprs", r.get())?,
            thawed: at("Oven.thawed", r.get())?,
            lets: at("Oven.lets", r.get())?,
            unknowns: at("Oven.unknowns", r.get())?,
            frame: at("Oven.frame", r.get())?,
            mem: at("Oven.mem", Memory::snap_get(r, self.mem.limit, ntypes))?,
            exc: at("Oven.exc", r.get())?,
            memo: at("Oven.memo", r.get())?,
            mp: at("Oven.mp", r.get())?,
            virt: at("Oven.virt", r.get())?,
            meta: at("Oven.meta", r.get())?,
            refl_seen: at("Oven.refl_seen", r.get())?,
            dms: at("Oven.dms", r.get())?,
        };
        match parts
            .check(self.budget.memo_entries)
            .map_err(str::to_owned)
            .and_then(|()| parts.verify_code())
        {
            Err(why) => return Err(r.invalid::<Oven>(why)),
            // The code's AST ids, for the AST's range check after the
            // whole restore (spec/snap.md "Id ranges").
            Ok(ids) => {
                use kiln_ast::ids::space;
                r.note_id(space::DECL, ids.decl);
                r.note_id(space::EXPR, ids.expr);
                r.note_id(space::TYPE, ids.ty);
            }
        }
        let Parts {
            types,
            code,
            padded,
            sites,
            step_locs,
            origins,
            traps,
            funcs,
            func_map,
            base_funcs,
            ctor_steps,
            volatile_decls,
            volatile_exprs,
            thunks,
            thunks_at,
            last_thunk,
            globals,
            global_map,
            provisional,
            provisional_objs,
            addr_int_disps,
            synth,
            label_diffs,
            label_diff_cells,
            lit_exprs,
            thawed,
            lets,
            unknowns,
            frame,
            mem,
            exc,
            memo,
            mp,
            virt,
            meta,
            refl_seen,
            dms,
        } = parts;
        self.types = types;
        self.code = code;
        self.padded = padded;
        self.sites = sites;
        self.step_locs = step_locs;
        self.origins = origins;
        self.traps = traps;
        self.funcs = funcs;
        self.func_map = func_map;
        self.base_funcs = base_funcs;
        self.ctor_steps = ctor_steps;
        self.volatile_decls = volatile_decls;
        self.volatile_exprs = volatile_exprs;
        self.thunks = thunks;
        self.thunks_at = thunks_at;
        self.last_thunk = last_thunk;
        self.globals = globals;
        self.global_map = global_map;
        self.provisional = provisional;
        self.provisional_objs = provisional_objs;
        self.addr_int_disps = addr_int_disps;
        self.synth = synth;
        self.label_diffs = label_diffs;
        self.label_diff_cells = label_diff_cells;
        self.lit_exprs = lit_exprs;
        self.thawed = thawed;
        self.lets = lets;
        self.unknowns = unknowns;
        self.frame = frame;
        self.mem = mem;
        self.exc = exc;
        self.memo = memo;
        self.mp = mp;
        self.virt = virt;
        self.meta = meta;
        self.refl_seen = refl_seen;
        self.dms = dms;
        Ok(())
    }

    /// Registers the encoding of [`Oven::snap_put`]: its hash is
    /// `{ let mut s = Schema::new(); Oven::snap_schema(&mut s); s.hash() }`.
    /// The constant schema hash of the root ([`Self::snap_schema`]).
    pub const SNAP_SCHEMA_HASH: u64 = SchemaHash::new("configured root", "Oven")
        .field::<Types>("types")
        .field::<Vec<u32>>("code")
        .field::<usize>("padded")
        .field::<Vec<Site>>("sites")
        .field::<Vec<Loc>>("step_locs")
        .field::<Vec<mem::Origin>>("origins")
        .field::<Vec<TrapInfo>>("traps")
        .field::<Vec<Func>>("funcs")
        .field::<FxHashMap<DeclId, u32>>("func_map")
        .field::<FxHashMap<DeclId, u32>>("base_funcs")
        .field::<FxHashMap<u32, Box<[CtorStep]>>>("ctor_steps")
        .field::<FxHashSet<DeclId>>("volatile_decls")
        .field::<FxHashSet<ExprId>>("volatile_exprs")
        .field::<FxHashMap<(ExprId, Option<DeclId>, bool), u32>>("thunks")
        .field::<FxHashMap<(ExprId, ExprId, bool), u32>>("thunks_at")
        .field::<Option<LastThunk>>("last_thunk")
        .field::<Vec<Global>>("globals")
        .field::<FxHashMap<DeclId, u32>>("global_map")
        .field::<Vec<u32>>("provisional")
        .field::<FxHashMap<u32, (OTy, u64)>>("provisional_objs")
        .field::<FxHashMap<(mem::Sp, u64), i64>>("addr_int_disps")
        .field::<std::cell::RefCell<Vec<(u64, i64)>>>("synth")
        .field::<Vec<(ExprId, ExprId)>>("label_diffs")
        .field::<FxHashMap<(mem::Sp, u64), (u32, i64)>>("label_diff_cells")
        .field::<FxHashMap<ExprId, u32>>("lit_exprs")
        .field::<FxHashMap<ConstValueId, u64>>("thawed")
        .field::<FxHashMap<DeclId, u64>>("lets")
        .field::<FxHashMap<DeclId, u64>>("unknowns")
        .field::<FrameState>("frame")
        .root_field("mem", Memory::SNAP_SCHEMA_HASH)
        .field::<Excs>("exc")
        .field::<Memo>("memo")
        .field::<MemPtrs>("mp")
        .field::<Virt>("virt")
        .field::<MetaState>("meta")
        .field::<ReflSeen>("refl_seen")
        .field::<DmsTable>("dms")
        .finish();
    pub fn snap_schema(s: &mut Schema) {
        let d = Desc::new("configured root")
            .hash(Self::SNAP_SCHEMA_HASH)
            .field::<Types>("types")
            .field::<Vec<u32>>("code")
            .field::<usize>("padded")
            .field::<Vec<Site>>("sites")
            .field::<Vec<Loc>>("step_locs")
            .field::<Vec<mem::Origin>>("origins")
            .field::<Vec<TrapInfo>>("traps")
            .field::<Vec<Func>>("funcs")
            .field::<FxHashMap<DeclId, u32>>("func_map")
            .field::<FxHashMap<DeclId, u32>>("base_funcs")
            .field::<FxHashMap<u32, Box<[CtorStep]>>>("ctor_steps")
            .field::<FxHashSet<DeclId>>("volatile_decls")
            .field::<FxHashSet<ExprId>>("volatile_exprs")
            .field::<FxHashMap<(ExprId, Option<DeclId>, bool), u32>>("thunks")
            .field::<FxHashMap<(ExprId, ExprId, bool), u32>>("thunks_at")
            .field::<Option<LastThunk>>("last_thunk")
            .field::<Vec<Global>>("globals")
            .field::<FxHashMap<DeclId, u32>>("global_map")
            .field::<Vec<u32>>("provisional")
            .field::<FxHashMap<u32, (OTy, u64)>>("provisional_objs")
            .field::<FxHashMap<(mem::Sp, u64), i64>>("addr_int_disps")
            .field::<std::cell::RefCell<Vec<(u64, i64)>>>("synth")
            .field::<Vec<(ExprId, ExprId)>>("label_diffs")
            .field::<FxHashMap<(mem::Sp, u64), (u32, i64)>>("label_diff_cells")
            .field::<FxHashMap<ExprId, u32>>("lit_exprs")
            .field::<FxHashMap<ConstValueId, u64>>("thawed")
            .field::<FxHashMap<DeclId, u64>>("lets")
            .field::<FxHashMap<DeclId, u64>>("unknowns")
            .field::<FrameState>("frame")
            .field::<Memory>("mem")
            .field::<Excs>("exc")
            .field::<Memo>("memo")
            .field::<MemPtrs>("mp")
            .field::<Virt>("virt")
            .field::<MetaState>("meta")
            .field::<ReflSeen>("refl_seen")
            .field::<DmsTable>("dms");
        if s.register::<Oven>(d) {
            Types::schema(s);
            Vec::<u32>::schema(s);
            usize::schema(s);
            Vec::<Site>::schema(s);
            Vec::<Loc>::schema(s);
            Vec::<mem::Origin>::schema(s);
            Vec::<TrapInfo>::schema(s);
            Vec::<Func>::schema(s);
            FxHashMap::<DeclId, u32>::schema(s);
            FxHashMap::<u32, Box<[CtorStep]>>::schema(s);
            FxHashSet::<DeclId>::schema(s);
            FxHashSet::<ExprId>::schema(s);
            FxHashMap::<(ExprId, Option<DeclId>, bool), u32>::schema(s);
            FxHashMap::<(ExprId, ExprId, bool), u32>::schema(s);
            Option::<LastThunk>::schema(s);
            Vec::<Global>::schema(s);
            FxHashMap::<u32, (OTy, u64)>::schema(s);
            FxHashMap::<(mem::Sp, u64), i64>::schema(s);
            std::cell::RefCell::<Vec<(u64, i64)>>::schema(s);
            Vec::<(ExprId, ExprId)>::schema(s);
            FxHashMap::<(mem::Sp, u64), (u32, i64)>::schema(s);
            FxHashMap::<ExprId, u32>::schema(s);
            FxHashMap::<ConstValueId, u64>::schema(s);
            FxHashMap::<DeclId, u64>::schema(s);
            FrameState::schema(s);
            Memory::snap_schema(s);
            Excs::schema(s);
            Memo::schema(s);
            MemPtrs::schema(s);
            Virt::schema(s);
            MetaState::schema(s);
            ReflSeen::schema(s);
            DmsTable::schema(s);
        }
    }
}

impl Parts {
    /// The cross-table invariants the evaluator relies on without a
    /// check (see the module docs).
    fn check(&self, memo_cap: usize) -> Result<(), &'static str> {
        self.types.check_restored()?;
        let ntypes = self.types.infos.len();
        let ty_ok = |t: OTy| (t.0 as usize) < ntypes;
        let (ncode, nfuncs, nglobals) = (self.code.len(), self.funcs.len(), self.globals.len());
        if self.padded > ncode {
            return Err("code padding past the end of the code");
        }
        let (_, nothrow) = self.exc.snap_refs();
        if nothrow.into_iter().any(|f| f as usize >= nfuncs) {
            return Err("non-throwing cache names an unknown function");
        }
        for f in &self.funcs {
            let outside = f.entry as usize >= ncode;
            if (outside && (f.entry != UNCOMPILED || f.state == FState::Ready))
                || f.state == FState::Compiling
            {
                return Err("function entry outside the code");
            }
            let tys = f.params.iter().map(|p| p.ty).chain(f.this_ty);
            let ret = match f.ret {
                crate::compile::RetKind::Scalar(t) | crate::compile::RetKind::Sret(t) => Some(t),
                crate::compile::RetKind::Void => None,
            };
            if !tys.chain(ret).all(ty_ok) {
                return Err("function signature of an unknown type");
            }
        }
        if self.sites.iter().any(|s| s.pc as usize > ncode) {
            return Err("site outside the code");
        }
        let func_ix = self
            .func_map
            .values()
            .chain(self.base_funcs.values())
            .chain(self.ctor_steps.keys())
            .chain(self.thunks.values())
            .chain(self.thunks_at.values());
        if func_ix.copied().chain(self.frame.stub_funcs()).any(|i| i as usize >= nfuncs) {
            return Err("function table names an unknown function");
        }
        if self.last_thunk.is_some_and(|t| !t.fits(ncode, nfuncs)) {
            return Err("last thunk outside the tables");
        }
        for g in &self.globals {
            if let GKind::Str(_, t, _) = g.kind
                && !ty_ok(t)
            {
                return Err("string literal object of an unknown type");
            }
        }
        let global_ix = self
            .global_map
            .values()
            .chain(&self.provisional)
            .chain(self.provisional_objs.keys())
            .chain(self.lit_exprs.values());
        if global_ix.copied().any(|i| i as usize >= nglobals)
            || !self.provisional_objs.values().all(|&(t, _)| ty_ok(t))
        {
            return Err("global table names an unknown global");
        }
        if self.label_diff_cells.values().any(|&(i, _)| i as usize >= self.label_diffs.len()) {
            return Err("label difference cell names an unknown difference");
        }
        self.memo.check_restored(memo_cap, nfuncs)?;
        self.mp.check_restored(nfuncs)?;
        self.virt.check_restored(nfuncs)?;
        let recipe_ok = |r: &Recipe| r.types().all(ty_ok);
        for s in &self.meta.sites {
            let recipes = s.result.iter().chain(s.ranges.iter().flatten());
            let exc = s.exc.iter().flat_map(|e| [&e.what, &e.at]);
            if !recipes.chain(exc).all(recipe_ok) {
                return Err("intrinsic call site of an unknown type");
            }
        }
        if self.meta.info_ty.is_some_and(|t| !ty_ok(t)) {
            return Err("reflection type out of range");
        }
        Ok(())
    }

    /// The bytecode verifier over the checked tables (`kbc/verify.rs`):
    /// the AST ids the code holds, for the AST's range check.
    fn verify_code(&self) -> Result<crate::kbc::verify::AstIds, String> {
        let (unwind, _) = self.exc.snap_refs();
        let heap: Vec<bool> =
            self.origins.iter().map(|o| matches!(o, mem::Origin::Heap(..))).collect();
        let tables = crate::kbc::verify::Tables {
            funcs: &self.funcs,
            nglobals: self.globals.len(),
            ntypes: self.types.infos.len(),
            ntraps: self.traps.len(),
            nsteps: self.step_locs.len(),
            heap_origins: &heap,
        };
        crate::kbc::verify::verify(&self.code, &tables, self.sites.iter().map(|s| s.pc), unwind)
            .map_err(|f| f.to_string())
    }
}
