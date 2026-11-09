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
//! ([`Oven::frame_free`]). Only an object no evaluation can leave a
//! pointer in is tracked (no pointer, reference, virtual pointer, union,
//! `mutable` or volatile subobject, no destructor): a pointer into the
//! frame or heap space of one evaluation dangles once it ends.

use kiln_ast::{ConstValueId, DeclId, ExprId};
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum FrameHow {
    /// Its value.
    Value,
    /// Its side effects.
    Discard,
    /// The initialization of local variable `d`'s object of type `ty`.
    Init(DeclId, OTy),
}

/// The state of the potential-constant frame evaluations.
#[derive(Debug, Default)]
pub(crate) struct FrameState {
    /// The running job's frame: each tracked variable and its object.
    binds: Vec<(DeclId, u64)>,
    /// The running job's evaluation.
    how: Option<FrameHow>,
    /// The object a [`FrameHow::Init`] job initializes.
    target: u64,
    /// The bytes the tracked objects take.
    bytes: u64,
    /// The stubs compiled so far, by expression and evaluation: each for
    /// the variables of a frame, in order.
    stubs: FxHashMap<(ExprId, FrameHow), Vec<(Vec<DeclId>, u32)>>,
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
        if size == 0 || !self.frame_trackable(ty, 0) || self.frame.bytes + size > MAX_FRAME_BYTES
        {
            return Ok(None);
        }
        let p = self.mem.alloc_static(ty, size, 0, Origin::Var(d));
        if let Some(e) = init
            && let Err(f) = self.frame_job(bridge, e, frame, FrameHow::Init(d, ty), p)
        {
            self.mem.free_static(p, &self.types);
            return Err(f);
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
        self.frame_job(bridge, e, frame, how, 0)
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

    /// Whether an object of type `ty` can be tracked: no evaluation can
    /// leave a pointer in it.
    fn frame_trackable(&self, ty: OTy, depth: u32) -> bool {
        let info = self.types.info(ty);
        if depth > 32
            || info.has_ptr
            || info.has_vptr
            || info.has_union
            || info.has_mutable
            || info.has_volatile
        {
            return false;
        }
        match info.kind {
            TyKind::Bool | TyKind::Int { .. } | TyKind::Float(_) | TyKind::WideInt { .. } => true,
            TyKind::Array { elem, .. } => self.frame_trackable(elem, depth + 1),
            TyKind::Record { union, .. } => !union,
            _ => false,
        }
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
    ) -> Result<ConstValueId, EvalFailure> {
        self.frame.binds = frame.iter().map(|&(d, o)| (d, o.0)).collect();
        self.frame.how = Some(how);
        self.frame.target = target;
        self.potential_next = Some(0);
        let r = self.eval_job(bridge, Job::Frame(e), None, false, false, true).0;
        self.potential_next = None;
        self.frame.how = None;
        self.frame.binds.clear();
        r
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

