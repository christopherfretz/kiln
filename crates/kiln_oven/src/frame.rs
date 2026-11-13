//! Potential-constant frames (spec/oven.md "Evaluation entry points",
//! WP-1150): the local variables of a `constexpr` function body that
//! sema's never-constant walk tracks, as clang's
//! `Expr::isPotentialConstantExpr` keeps their values in its call frame
//! while the parameters stay unknown.
//!
//! Each tracked variable is a static object ([`crate::mem::Origin::Var`],
//! writable) that outlives the evaluations of the walk: one evaluation
//! initializes it ([`Oven::frame_local`]), later ones read and modify it
//! ([`Oven::evaluate_in_frame`]) with each tracked variable's name
//! designating its object, and the walk frees it when its scope ends
//! ([`Oven::frame_free`]). A tracked object is a scalar (a pointer
//! included, WP-1163), or an array or class of them with no virtual
//! pointer, union, `mutable` or volatile subobject and no destructor. A
//! pointer one evaluation leaves in it must outlive that evaluation: an
//! evaluation that stores one into its own frame or heap space (which
//! dangles once it ends) fails quietly ([`Oven::frame_escapes`]).
//!
//! In a frame job, as in clang's potential mode, reading a parameter,
//! `this`, an untracked local or an object whose lifetime ended in an
//! earlier job fails quietly, and a potential result is not checked as
//! a constant result. A loop over tracked objects runs as one job
//! ([`Oven::evaluate_loop_in_frame`]) within a step budget; a failed
//! loop job leaves the frame as it found it.

use kiln_ast::{ConstValueId, DeclId, ExprId, StmtId};
use kiln_source::fx::FxHashMap;

use crate::compile::RetKind;
use crate::failure::{EvalFailure, FailKind};
use crate::mem::{GRAN, Origin, Ptr, flags};
use crate::ty::{OTy, TyKind};
use crate::value::Freeze;
use crate::{Job, Oven, SemaBridge};

/// The most bytes the tracked objects of a walk take; a variable past
/// it is not tracked.
const MAX_FRAME_BYTES: u64 = 64 * 1024;

/// The object of a local variable a potential-constant frame tracks
/// ([`Oven::frame_local`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameObject(u64);

/// How [`Oven::evaluate_in_frame`] evaluates its expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameEval {
    /// For its value, a scalar (a condition).
    Value,
    /// For its side effects, the value discarded (an expression
    /// statement, a `for` increment, a returned value).
    Discard,
}

/// What a frame stub does with its expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, kiln_snap::Snap)]
pub(crate) enum FrameHow {
    /// Its value.
    Value,
    /// Its side effects.
    Discard,
    /// The initialization of local variable `d`'s object of type `ty`.
    Init(DeclId, OTy),
    /// The execution of loop statement `s` (its expression only anchors
    /// the job; [`Oven::evaluate_loop_in_frame`]).
    Loop(StmtId),
}

/// The state of the objects of a frame ([`Oven::frame_save`]): each
/// object's base, bytes, and initialized and pointer granule marks.
type FrameSave = Vec<(usize, Vec<u8>, Vec<u8>, Vec<u8>)>;

/// The stubs compiled for one expression and evaluation: each with the
/// variables of the frame it was compiled for, in order.
type FrameStubs = Vec<(Vec<DeclId>, u32)>;

/// The state of the potential-constant frame evaluations.
#[derive(Debug, Default, kiln_snap::Snap)]
pub(crate) struct FrameState {
    /// The running job's frame: each tracked variable and its object.
    #[snap(skip)]
    binds: Vec<(DeclId, u64)>,
    /// The running job's evaluation.
    #[snap(skip)]
    how: Option<FrameHow>,
    /// The object a [`FrameHow::Init`] job initializes.
    #[snap(skip)]
    target: u64,
    /// The bytes the tracked objects take.
    #[snap(skip)]
    bytes: u64,
    /// The stubs compiled so far, by expression and evaluation: each for
    /// the variables of a frame, in order.
    stubs: FxHashMap<(ExprId, FrameHow), FrameStubs>,
}

impl Oven {
    /// Starts tracking local variable `d` of a `constexpr` function body
    /// in a potential-constant frame (WP-1150): its object, initialized
    /// from `init` (`None`: left uninitialized) with the variables of
    /// `frame` designating their objects, as clang's
    /// `Expr::isPotentialConstantExpr` initializes a local variable.
    /// `Ok(None)`: `d` is not tracked (a reference, an object that may
    /// hold a pointer or has a destructor, or past the frame's size
    /// limit). The failure is the initialization's, with its notes
    /// (calls fail quietly, as in [`Oven::evaluate_potential`]); the
    /// object is then freed.
    pub fn frame_local(
        &mut self,
        bridge: &mut dyn SemaBridge,
        d: DeclId,
        init: Option<ExprId>,
        frame: &[(DeclId, FrameObject)],
    ) -> Result<Option<FrameObject>, EvalFailure> {
        if self.ctx_depth != 0 {
            return Ok(None);
        }
        self.types.refresh(bridge);
        let ast = bridge.ast();
        let n = ast.decls.get(d);
        let Some(q) = n.ty() else { return Ok(None) };
        let quals = ast.types.effective_quals(q);
        if n.has(kiln_ast::decl::DeclFlags::INVALID)
            || ast.types.is_reference(q)
            || quals.is_volatile()
            || ast.needs_destruction(q)
            || !crate::compile::is_literal_type(ast, q)
        {
            return Ok(None);
        }
        let ty = self.types.intern(bridge, q);
        let size = self.types.size(ty);
        if size == 0 || !self.frame_trackable(ty, 0) || self.frame.bytes + size > MAX_FRAME_BYTES {
            return Ok(None);
        }
        let p = self.mem.alloc_static(ty, size, 0, Origin::Var(d));
        if let Some(e) = init
            && let Err(f) = self.frame_job(bridge, e, frame, FrameHow::Init(d, ty), p).0
        {
            self.mem.free_static(p, &self.types);
            return Err(f);
        }
        if self.frame_escapes(p) {
            // Its value is a pointer into the evaluation's frame or heap
            // space, which [`crate::mem::Memory::reset`] ends: clang
            // knows it, the walk does not.
            self.mem.free_static(p, &self.types);
            return Err(EvalFailure::silent(FailKind::NotConstant));
        }
        if quals.is_const()
            && let Ok(a) = self.mem.alloc_of(Ptr::decode(p))
        {
            let fl = a.flags | flags::CONST;
            self.mem.set_flags(Ptr::decode(p), fl);
        }
        self.frame.bytes += size;
        Ok(Some(FrameObject(p)))
    }

    /// Evaluates `e`, a full-expression of a `constexpr` function body,
    /// as `how` with each variable of `frame` designating its object
    /// (WP-1150; [`Oven::frame_local`]), manifestly constant-evaluated
    /// and with calls failing quietly, as in
    /// [`Oven::evaluate_potential`]: its value ([`FrameEval::Discard`]:
    /// `true`), or the failure with its notes. A modification of a
    /// tracked object stays.
    pub fn evaluate_in_frame(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: ExprId,
        frame: &[(DeclId, FrameObject)],
        how: FrameEval,
    ) -> Result<ConstValueId, EvalFailure> {
        if self.ctx_depth != 0 {
            return Err(EvalFailure::silent(FailKind::NotConstant));
        }
        let how = match how {
            FrameEval::Value => FrameHow::Value,
            FrameEval::Discard => FrameHow::Discard,
        };
        let r = self.frame_job(bridge, e, frame, how, 0).0?;
        if frame.iter().any(|&(_, o)| self.frame_escapes(o.0)) {
            // A tracked object now holds a pointer that dangles once the
            // evaluation ends ([`Oven::frame_escapes`]): its value is
            // lost, as after a side effect.
            return Err(EvalFailure::silent(FailKind::NotConstant));
        }
        Ok(r)
    }

    /// Executes loop statement `s` of a `constexpr` function body whole,
    /// in one job (WP-1163), with each variable of `frame` designating
    /// its object as in [`Oven::evaluate_in_frame`]; `at` is an
    /// expression of the loop (the job's location). At most `steps`
    /// statements are executed, `s` itself the first: the steps taken,
    /// or the failure (the step limit's at the statement clang runs out
    /// at). On a failure, or when a tracked object is left holding a
    /// pointer that dangles ([`Oven::frame_escapes`], failing quietly),
    /// the objects of `frame` are as they were before.
    pub fn evaluate_loop_in_frame(
        &mut self,
        bridge: &mut dyn SemaBridge,
        s: StmtId,
        at: ExprId,
        frame: &[(DeclId, FrameObject)],
        steps: u64,
    ) -> Result<u64, EvalFailure> {
        if self.ctx_depth != 0 || steps == 0 {
            return Err(EvalFailure::silent(FailKind::NotConstant));
        }
        let saved = self.frame_save(frame);
        let limit = self.budget.steps;
        self.budget.steps = steps.min(limit);
        let (r, used) = self.frame_job(bridge, at, frame, FrameHow::Loop(s), 0);
        self.budget.steps = limit;
        let r = match r {
            Ok(_) if frame.iter().any(|&(_, o)| self.frame_escapes(o.0)) => {
                Err(EvalFailure::silent(FailKind::NotConstant))
            }
            Ok(_) => return Ok(used),
            Err(f) => Err(f),
        };
        self.frame_restore(&saved);
        r
    }

    /// The state of the objects of `frame` ([`Oven::frame_restore`]).
    fn frame_save(&self, frame: &[(DeclId, FrameObject)]) -> FrameSave {
        let s = &self.mem.stat;
        let mut out = Vec::with_capacity(frame.len());
        for &(_, o) in frame {
            let Ok(a) = self.mem.alloc_of(Ptr::decode(o.0)) else { continue };
            let (base, size) = (u64::from(a.base) as usize, a.size as usize);
            let (g, n) = (base / GRAN as usize, size.div_ceil(GRAN as usize));
            let part = |v: &[u8], from: usize, len: usize| {
                v.get(from..from + len).unwrap_or_default().to_vec()
            };
            out.push((base, part(&s.bytes, base, size), part(&s.init, g, n), part(&s.ptr, g, n)));
        }
        out
    }

    /// Puts the objects saved by [`Oven::frame_save`] back as they were.
    fn frame_restore(&mut self, saved: &FrameSave) {
        let s = &mut self.mem.stat;
        for (base, bytes, init, ptr) in saved {
            let g = base / GRAN as usize;
            let put = |v: &mut [u8], from: usize, src: &[u8]| {
                if let Some(d) = v.get_mut(from..from + src.len()) {
                    d.copy_from_slice(src);
                }
            };
            put(&mut s.bytes, *base, bytes);
            put(&mut s.init, g, init);
            put(&mut s.ptr, g, ptr);
        }
    }

    /// Stops tracking the object `o` ([`Oven::frame_local`]): it is freed.
    pub fn frame_free(&mut self, o: FrameObject) {
        if let Ok(a) = self.mem.alloc_of(Ptr::decode(o.0)) {
            self.frame.bytes = self.frame.bytes.saturating_sub(u64::from(a.size));
        }
        self.mem.free_static(o.0, &self.types);
    }

    /// The state of the objects of `frame`: their bytes and which are
    /// initialized (two frames in the same state are equal).
    pub fn frame_snapshot(&self, frame: &[(DeclId, FrameObject)]) -> Vec<u8> {
        let mut out = Vec::new();
        for &(_, o) in frame {
            let Ok(a) = self.mem.alloc_of(Ptr::decode(o.0)) else { continue };
            let (base, size) = (u64::from(a.base) as usize, a.size as usize);
            let s = &self.mem.stat;
            out.extend_from_slice(s.bytes.get(base..base + size).unwrap_or_default());
            let g = base / GRAN as usize;
            let n = size.div_ceil(GRAN as usize);
            out.extend_from_slice(s.init.get(g..g + n).unwrap_or_default());
        }
        out
    }

    /// Ends a walk's potential-constant frame evaluations: the stubs
    /// compiled for it are forgotten.
    pub fn frame_end(&mut self) {
        self.frame.stubs.clear();
    }

    /// Whether an object of type `ty` can be tracked: scalars (pointers
    /// included, WP-1163; not pointers to members), and arrays and
    /// classes of them, with no virtual pointer, union, `mutable` or
    /// volatile subobject. A pointer it holds is checked after every
    /// evaluation ([`Oven::frame_escapes`]).
    fn frame_trackable(&self, ty: OTy, depth: u32) -> bool {
        let info = self.types.info(ty);
        if depth > 32 || info.has_vptr || info.has_union || info.has_mutable || info.has_volatile {
            return false;
        }
        match info.kind {
            TyKind::Bool
            | TyKind::Int { .. }
            | TyKind::Float(_)
            | TyKind::WideInt { .. }
            | TyKind::Ptr(_)
            | TyKind::NullPtr => true,
            TyKind::Array { elem, .. } => self.frame_trackable(elem, depth + 1),
            TyKind::Record { union, .. } => {
                !union
                    && self.types.fields(ty).iter().all(|f| self.frame_trackable(f.ty, depth + 1))
            }
            _ => false,
        }
    }

    /// Whether the tracked object at `o` holds a pointer that does not
    /// outlive the evaluation that stored it (WP-1163): one into the
    /// frame or heap space, which [`crate::mem::Memory::reset`] ends.
    /// Null, a function pointer, a pointer that points at nothing and
    /// one to a static object (a variable, a string literal, a tracked
    /// object; a stale one fails its generation check) keep.
    fn frame_escapes(&self, o: u64) -> bool {
        let Ok(a) = self.mem.alloc_of(Ptr::decode(o)) else { return false };
        if !self.types.info(a.ty).has_ptr {
            return false;
        }
        let s = &self.mem.stat;
        let (base, size) = (u64::from(a.base), u64::from(a.size));
        (base / GRAN..(base + size).div_ceil(GRAN)).any(|g| {
            if s.ptr.get(g as usize).is_none_or(|&m| m == 0) {
                return false;
            }
            let v = s.read(g * GRAN, 8) as u64;
            !(v == 0 || Ptr::is_nowhere(v) || Ptr::decode(v).is_static())
        })
    }

    /// Whether `fault` of the running frame job is the read or
    /// modification of a parameter or local variable the frame does not
    /// track (WP-1163): clang knows no value for it (a parameter) or
    /// fails quietly after the first one (C++14 `findCompleteObject`
    /// once `HasSideEffects` is set), so the job fails quietly and the
    /// walk takes the expression apart ([`Oven::frame_job`]).
    pub(crate) fn frame_unknown_read(&self, b: &dyn SemaBridge, fault: &crate::vm::Fault) -> bool {
        use crate::mem::MemFault;
        use crate::mem::Origin;
        let crate::vm::Fault::Mem { f, .. } = fault else { return false };
        self.frame.how.is_some()
            && match f {
                MemFault::External(d, _) | MemFault::Unknown(d) => {
                    crate::compile::is_local_var(b.ast(), *d)
                }
                // An object whose lifetime ended between jobs (a scoped
                // local, a temporary): clang's potential mode does not
                // report the access (WP-1163).
                MemFault::Dead(Origin::Var(_) | Origin::Temp(_) | Origin::Let(_))
                | MemFault::Stale => true,
                _ => false,
            }
    }

    /// Whether a frame job is running: `this` fails quietly in it
    /// ([`Oven::frame_unknown_read`]).
    pub(crate) fn frame_running(&self) -> bool {
        self.frame.how.is_some()
    }

    /// Runs a frame job: `e` evaluated as `how` with `frame` bound (and,
    /// for [`FrameHow::Init`], the object at `target` initialized).
    fn frame_job(
        &mut self,
        bridge: &mut dyn SemaBridge,
        e: ExprId,
        frame: &[(DeclId, FrameObject)],
        how: FrameHow,
        target: u64,
    ) -> (Result<ConstValueId, EvalFailure>, u64) {
        self.frame.binds = frame.iter().map(|&(d, o)| (d, o.0)).collect();
        self.frame.how = Some(how);
        self.frame.target = target;
        self.potential_next = Some(0);
        let r = self.eval_job(bridge, Job::Frame(e), None, false, false, true).0;
        // The outermost context's count, nested contexts' steps included
        // (they share its budget).
        let steps = self.run.counters.steps;
        self.potential_next = None;
        self.frame.how = None;
        self.frame.binds.clear();
        (r, steps)
    }

    /// The stub of the running frame job, for `e`.
    fn frame_stub(&mut self, b: &mut dyn SemaBridge, e: ExprId, how: FrameHow) -> u32 {
        let decls: Vec<DeclId> = self.frame.binds.iter().map(|&(d, _)| d).collect();
        if let Some(list) = self.frame.stubs.get(&(e, how))
            && let Some(&(_, f)) = list.iter().find(|(ds, _)| *ds == decls)
        {
            return f;
        }
        let f = self.compile_frame(b, e, &decls, how);
        self.frame.stubs.entry((e, how)).or_default().push((decls, f));
        f
    }

    /// Runs the job of [`Oven::frame_local`] or
    /// [`Oven::evaluate_in_frame`] with its frame at `fb`.
    pub(crate) fn eval_frame(
        &mut self,
        b: &mut dyn SemaBridge,
        e: ExprId,
        fb: u32,
    ) -> Result<ConstValueId, EvalFailure> {
        self.run.object = None;
        let Some(how) = self.frame.how else {
            return Err(EvalFailure::silent(FailKind::NotConstant));
        };
        let f = self.frame_stub(b, e, how);
        self.let_pending.clear();
        // Register 0 is the result, the objects' addresses follow (the
        // target's last).
        let fb = fb as usize;
        let n = self.frame.binds.len();
        self.grow_regs(fb + self.funcs[f as usize].nregs as usize + n + 2);
        for i in 0..n {
            self.regs[fb + 1 + i] = u128::from(self.frame.binds[i].1);
        }
        self.regs[fb + 1 + n] = u128::from(self.frame.target);
        self.exec(b, f, fb as u32)?;
        let v = self.regs[fb];
        let RetKind::Scalar(ty) = self.funcs[f as usize].ret else {
            return Err(EvalFailure::silent(FailKind::NotConstant));
        };
        let loc = crate::value::expr_loc(b.ast(), e);
        let r = self.freeze_scalar(b, ty, v, loc).map_err(|f| match f {
            Freeze::Fault(k) => EvalFailure::silent(k),
            Freeze::Notes(notes) => EvalFailure { kind: FailKind::NotConstant, notes },
        });
        match self.leak_notes(b) {
            None => r,
            Some(_) => Err(EvalFailure::silent(FailKind::NotConstant)),
        }
    }
}
