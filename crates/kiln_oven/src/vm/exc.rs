//! Exceptions in constant evaluation (P3068, C++26 [except], [expr.const];
//! spec/oven.md "Exceptions").
//!
//! A throw-expression creates the exception object in the heap
//! (`Op::New`, then its initialization) and executes `Op::Throw`. The
//! handlers of a try-block are registered by `Op::TryEnter` as a marker in
//! the cleanup stack ([`TRY_MARK`]), so leaving the try-block by any path
//! (falling off its end, `break`, `return`, an exception) pops it with the
//! destructions registered inside it. A throw finds the innermost marker
//! of the running evaluation, checks the frames it leaves (a `noexcept`
//! function is left: the evaluation fails, [except.terminate]/1.4), and
//! unwinds to it: the registered destructions above it run in order
//! ([except.ctor]/2), each returning to the single `Op::Unwind` of the
//! Oven, and the frames above its frame are popped as by a return. The
//! handler code then matches the exception against each handler in turn
//! (`Op::Catch`, [except.handle]/3); the matching one is entered with
//! `Op::CatchBegin`, which makes the exception the handled one and
//! registers its end ([`EXC_END`]) in the handler's scope; when no handler
//! matches, `Op::Throw` resumes unwinding. An exception object is
//! destroyed and freed when the last handler using it exits
//! ([except.throw]/4). An exception no handler of the evaluation catches
//! fails it ([`Fault::Uncaught`]).

use kiln_source::fx::FxHashMap;

use super::{BOTTOM, Cleanup, Fault, UNWIND_ONLY};
use crate::compile::{FState, RetKind};
use crate::kbc::{NO_DTOR, Op, ak};
use crate::mem::{DynKind, Ptr};
use crate::ty::{OTy, TyKind};
use crate::{Oven, SemaBridge};

/// `Cleanup::func` sentinels (never function indexes): the handler marker
/// of a try-block (`this` = the handler's pc).
pub(crate) const TRY_MARK: u32 = u32::MAX - 1;
/// The end of a handler: the handled exception is no longer handled.
pub(crate) const EXC_END: u32 = u32::MAX - 2;
/// The deallocation of an exception object (`this`, of type `ty`) whose
/// destructor runs above it.
pub(crate) const EXC_FREE: u32 = u32::MAX - 3;
/// The lowest sentinel.
pub(crate) const SENTINEL: u32 = EXC_FREE;
/// Not an exception sentinel (below [`SENTINEL`]): the destruction of a
/// trivially destructible base subobject (`this`, of type `ty`) by its
/// derived class's destructor, which runs no function but requires the
/// base within its lifetime (clang's `HandleDestructionImpl`; `run_cleanup`).
pub(crate) const LIFE_CHECK: u32 = u32::MAX - 4;

/// An exception object.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Exc {
    pub obj: u64,
    pub ty: OTy,
    /// Its destructor's function (`NO_DTOR`).
    pub dtor: u32,
    /// The throw that created it (rethrows keep it).
    pub pc: u32,
    /// The index of the handler marker it unwinds to.
    pub h: u32,
    /// The stack depth of the frame being unwound.
    pub at: u32,
}

/// The exception state of the Oven.
#[derive(Debug, Default)]
pub(crate) struct Excs {
    /// Thrown, not yet caught: the innermost last.
    pub inflight: Vec<Exc>,
    /// Handled by an active handler: the innermost last.
    pub caught: Vec<Exc>,
    /// The lengths of the two stacks when the innermost evaluation began:
    /// the exceptions below belong to enclosing evaluations, which a
    /// nested one does not see ([expr.const]: each is a separate
    /// evaluation).
    pub base: (usize, usize),
    /// The pc of the Oven's `Op::Unwind`, once emitted.
    unwind_pc: Option<u32>,
    /// Whether each function (by index) is non-throwing.
    nothrow: FxHashMap<u32, bool>,
}

impl Excs {
    pub fn footprint(&self) -> usize {
        (self.inflight.capacity() + self.caught.capacity()) * size_of::<Exc>()
            + self.nothrow.capacity() * 8
    }

    /// The lengths of the two stacks (restored by a failed evaluation).
    pub fn lens(&self) -> (usize, usize) {
        (self.inflight.len(), self.caught.len())
    }

    pub fn truncate(&mut self, (i, c): (usize, usize)) {
        self.inflight.truncate(i);
        self.caught.truncate(c);
    }

    /// Forgets function index `f` (a reclaimed thunk's, reused by the
    /// next compiled function; `reclaim.rs`).
    pub fn forget_func(&mut self, f: u32) {
        self.nothrow.remove(&f);
    }

    pub fn clear(&mut self) {
        self.inflight.clear();
        self.caught.clear();
        self.base = (0, 0);
    }

    /// The exceptions of the innermost evaluation thrown and not yet
    /// caught ([except.throw]/5: `std::uncaught_exceptions()`).
    pub fn uncaught(&self) -> usize {
        self.inflight.len().saturating_sub(self.base.0)
    }

    /// The innermost in-flight exception of the innermost evaluation.
    fn active(&self) -> Option<&Exc> {
        self.inflight.get(self.base.0..).and_then(<[Exc]>::last)
    }

    /// The innermost exception the innermost evaluation handles.
    fn handled(&self) -> Option<Exc> {
        self.caught.get(self.base.1..).and_then(<[Exc]>::last).copied()
    }
}

impl Oven {
    /// `Op::Throw` (a new exception, a rethrow or a resumption) and
    /// `Op::Unwind` at `pc` in the frame at `fb`: the pc and frame base
    /// execution continues at.
    #[inline(never)]
    pub(super) fn throw_op(
        &mut self,
        b: &mut dyn SemaBridge,
        op: Op,
        pc: usize,
        fb: usize,
        aux: u32,
    ) -> Result<(usize, usize), (Fault, usize)> {
        if op == Op::Unwind {
            return self.unwind(b, pc, fb);
        }
        let arg = |i: usize| self.code[pc + 1 + i];
        let mut exc =
            match aux {
                0 => {
                    let obj = self.regs[fb + arg(0) as usize] as u64;
                    // The exception object is initialized: its release on an
                    // exception from that is no longer needed.
                    if self.cleanups.last().is_some_and(|c| {
                        c.func == EXC_FREE && c.this == obj && c.pc & UNWIND_ONLY != 0
                    }) {
                        self.cleanups.pop();
                    }
                    Exc { obj, ty: OTy(arg(1)), dtor: arg(2), pc: pc as u32, h: 0, at: 0 }
                }
                // [expr.throw]/4: the currently handled exception.
                1 => match self.exc.handled() {
                    Some(e) => e,
                    None => return Err((Fault::RethrowNone, pc)),
                },
                _ => match self.exc.inflight.pop() {
                    Some(e) => e,
                    None => return Err((Fault::Invalid, pc)),
                },
            };
        // The innermost try-block of this evaluation.
        let entry = self.run.entry;
        let h = self
            .cleanups
            .iter()
            .rposition(|c| c.func == TRY_MARK)
            .filter(|&i| self.cleanups[i].depth as usize > entry);
        // The frames the exception leaves: those above the handler's.
        let stop = h.map_or(entry, |i| self.cleanups[i].depth as usize);
        if aux != 2
            && let Some(active) = self.exc.active()
            && (h.is_none() || stop <= active.at as usize)
        {
            // An exception leaving a destructor run by unwinding
            // ([except.terminate]/1.4).
            return Err((Fault::ThrowWhileActive { prev: active.pc }, pc));
        }
        for i in (stop..self.stack.len()).rev() {
            let f = self.stack[i].func;
            if self.func_nothrow(b, f) {
                // [except.spec]/5: `std::terminate` ([except.terminate]).
                let at = self.stack.get(i + 1).map_or(pc, |fr| fr.call_pc as usize);
                self.stack.truncate(i + 1);
                return Err((Fault::NoexceptExit, at));
            }
        }
        let Some(h) = h else {
            let what = self.exc_what(b, &exc);
            return Err((Fault::Uncaught { obj: exc.obj, ty: exc.ty, what }, exc.pc as usize));
        };
        exc.h = h as u32;
        exc.at = self.stack.len() as u32;
        self.exc.inflight.push(exc);
        self.unwind(b, pc, fb)
    }

    /// Unwinds the innermost in-flight exception towards its handler
    /// from the frame at `fb` (`pc`: where it stands, for faults).
    fn unwind(
        &mut self,
        b: &mut dyn SemaBridge,
        pc: usize,
        mut fb: usize,
    ) -> Result<(usize, usize), (Fault, usize)> {
        let Some(&exc) = self.exc.inflight.last() else { return Err((Fault::Invalid, pc)) };
        let h = exc.h as usize;
        let Some(depth) = self.cleanups.get(h).map(|c| c.depth as usize) else {
            return Err((Fault::Invalid, pc));
        };
        loop {
            if self.cleanups.len() > h + 1
                && self.cleanups.last().is_some_and(|c| c.depth as usize == self.stack.len())
            {
                let n = self.stack.len() as u32;
                if let Some(e) = self.exc.inflight.last_mut() {
                    e.at = n;
                }
                let ret = self.unwind_pc();
                match self.run_cleanup(b, pc, ret, fb, true)? {
                    Some(next) => return Ok(next),
                    None => continue,
                }
            }
            if self.stack.len() > depth {
                // The frame is left as by a return, without a result: its
                // objects are destroyed ([except.ctor]/1), the trivially
                // destructible ones checked as by a return.
                if let Some(fr) = self.stack.last()
                    && fr.mark != BOTTOM
                    && self.mem.life_above(fr.mark)
                    && let Err(f) = self.scope_life(b.ast(), fr.mark, super::life::Objs::All)
                {
                    return Err((f, exc.pc as usize));
                }
                let Some(fr) = self.stack.pop() else { break };
                if fr.mark != BOTTOM {
                    self.mem.release(fr.mark, &self.types);
                }
                if fr.memo
                    && let Some(p) = self.run.pending.pop()
                {
                    self.mem.peak = self.mem.peak.max(p.peak_saved);
                    self.mem.arena_peak = self.mem.arena_peak.max(p.arena_saved);
                    self.run.maxd = self.run.maxd.max(p.maxd_saved);
                }
                self.mem.cur_frame = self.stack.last().map_or(0, |f| f.serial);
                fb = self.stack[self.stack.len() - 1].fb as usize;
                continue;
            }
            break;
        }
        match self.cleanups.pop() {
            Some(m) if m.func == TRY_MARK => Ok((m.this as usize, fb)),
            _ => Err((Fault::Invalid, pc)),
        }
    }

    /// The pc of the Oven's `Op::Unwind` (appended on first use, over the
    /// last unit's padding, `kbc::unpad`).
    fn unwind_pc(&mut self) -> usize {
        if let Some(p) = self.exc.unwind_pc {
            return p as usize;
        }
        crate::kbc::unpad(&mut self.code, &mut self.padded);
        let p = self.code.len() as u32;
        self.code.push(Op::Unwind as u32);
        crate::kbc::pad(&mut self.code, &mut self.padded);
        self.exc.unwind_pc = Some(p);
        p as usize
    }

    /// `Op::CatchBegin` at `pc`: the in-flight exception becomes the
    /// handled one ([except.handle]/8) until the handler's scope exits.
    pub(super) fn catch_begin(&mut self, pc: usize) -> Result<(), Fault> {
        let Some(e) = self.exc.inflight.pop() else { return Err(Fault::Invalid) };
        self.exc.caught.push(e);
        let depth = self.stack.len() as u32;
        self.cleanups.push(Cleanup { this: 0, ty: e.ty, func: EXC_END, pc: pc as u32, depth });
        Ok(())
    }

    /// A sentinel cleanup `c` popped by `run_cleanup`: the destruction to
    /// run in its place, if any.
    pub(super) fn exc_cleanup(&mut self, c: Cleanup) -> Result<Option<Cleanup>, Fault> {
        match c.func {
            EXC_END => {
                let Some(e) = self.exc.caught.pop() else { return Ok(None) };
                if self.exc.caught.iter().chain(&self.exc.inflight).any(|x| x.obj == e.obj) {
                    // Rethrown, or handled by an enclosing handler too.
                    return Ok(None);
                }
                let depth = self.stack.len() as u32;
                if e.dtor == NO_DTOR {
                    self.free_exc(e.obj, e.ty)?;
                    return Ok(None);
                }
                self.cleanups.push(Cleanup {
                    this: e.obj,
                    ty: e.ty,
                    func: EXC_FREE,
                    pc: c.pc,
                    depth,
                });
                Ok(Some(Cleanup { this: e.obj, ty: e.ty, func: e.dtor, pc: c.pc, depth }))
            }
            EXC_FREE => {
                self.free_exc(c.this, c.ty)?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn free_exc(&mut self, obj: u64, ty: OTy) -> Result<(), Fault> {
        self.mem.heap_free(obj, DynKind::New, Some(ty), &self.types).map_err(|f| Fault::Mem {
            f,
            ak: ak::READ,
            ty,
            p: obj,
        })
    }

    /// Whether the function at index `f` is non-throwing ([except.spec]).
    fn func_nothrow(&mut self, b: &mut dyn SemaBridge, f: u32) -> bool {
        if let Some(&n) = self.exc.nothrow.get(&f) {
            return n;
        }
        let n = self.funcs[f as usize].decl.is_some_and(|d| b.is_nothrow(d, self));
        self.exc.nothrow.insert(f, n);
        n
    }

    /// `Op::Catch` with `aux`, handler type `t`: whether the in-flight
    /// exception matches ([except.handle]/3), and the handler's operand.
    pub(super) fn catch_match(
        &mut self,
        b: &mut dyn SemaBridge,
        aux: u32,
        t: OTy,
    ) -> Result<Option<u128>, Fault> {
        let Some(&e) = self.exc.inflight.last() else { return Err(Fault::Invalid) };
        let at = |off: u64| Ptr::with_off(e.obj, u64::from(Ptr::decode(e.obj).off) + off, false);
        match aux {
            // `...`
            0 => Ok(Some(0)),
            // An object or reference handler: the same type (3.1) or an
            // unambiguous public base class (3.2).
            1 => {
                if e.ty == t {
                    return Ok(Some(u128::from(e.obj)));
                }
                Ok(self.public_base(b, e.ty, t).map(|(off, _)| u128::from(at(off))))
            }
            // A pointer (or pointer-to-member) handler (3.3, 3.4).
            _ => self.catch_ptr(b, e, t),
        }
    }

    fn catch_ptr(&mut self, b: &mut dyn SemaBridge, e: Exc, t: OTy) -> Result<Option<u128>, Fault> {
        match (self.types.kind(e.ty), self.types.kind(t)) {
            (TyKind::NullPtr, TyKind::Ptr(_) | TyKind::MemPtr { .. }) => Ok(Some(0)),
            _ if e.ty == t => self.load(e.obj, e.ty, ak::READ).map(Some),
            (TyKind::Ptr(ept), TyKind::Ptr(tpt)) => {
                let ast = b.ast();
                let (eq, tq) = (self.types.info(e.ty).q, self.types.info(t).q);
                let (Some(ep), Some(tp)) = (ast.types.pointee(eq), ast.types.pointee(tq)) else {
                    return Ok(None);
                };
                let (ep, tp) = (ast.types.canonical(ep), ast.types.canonical(tp));
                // The qualification conversion ([conv.qual]): the
                // handler's pointee at least as qualified.
                let (eb, tb) = (ep.quals_bits() & 3, tp.quals_bits() & 3);
                if eb & !tb != 0 {
                    return Ok(None);
                }
                let v = self.load(e.obj, e.ty, ak::READ)?;
                if ep.without_quals() == tp.without_quals()
                    || ast.types.is_void(tp) && !ast.types.is_function(ep)
                {
                    return Ok(Some(v));
                }
                // [conv.ptr]/3: to a pointer to an unambiguous public base.
                let Some((off, virt)) = self.public_base(b, ept, tpt) else { return Ok(None) };
                if v == 0 {
                    return Ok(Some(0));
                }
                if virt {
                    // Through the most derived object's virtual base
                    // ([class.mi]/4): the pointer may designate a base
                    // class subobject.
                    return self
                        .vbase_ptr(b, v as u64, ept, tpt)
                        .map(|p| Some(u128::from(p)))
                        .ok_or(Fault::Invalid);
                }
                let raw = v as u64;
                Ok(Some(u128::from(Ptr::with_off(
                    raw,
                    u64::from(Ptr::decode(raw).off) + off,
                    false,
                ))))
            }
            _ => Ok(None),
        }
    }

    /// The `what()` of an uncaught exception object of a class derived
    /// from `std::exception` (for its note): the string its `what` (a
    /// virtual call of `std::exception::what`) returns.
    fn exc_what(&mut self, b: &mut dyn SemaBridge, e: &Exc) -> Option<Box<str>> {
        if self.run.silent || !matches!(self.types.kind(e.ty), TyKind::Record { .. }) {
            return None;
        }
        let subs = self.sub_list(b, e.ty);
        let ast = b.ast();
        let &(off, cls, base, poly) =
            subs.iter().find(|s| crate::compile::is_std_named(ast, s.2, "exception"))?;
        let def = ast.definition(base)?;
        let what = ast.members(def).find(|&m| {
            crate::compile::is_named(ast, m, "what")
                && ast.decls.function(m).is_some_and(|f| ast.lists.get(f.params).is_empty())
        })?;
        let this = Ptr::with_off(e.obj, u64::from(Ptr::decode(e.obj).off) + off, false);
        let (func, this) = if poly {
            let (f, t, _) = self.vcall(b, what, cls, this).ok()?;
            (f, t)
        } else {
            (self.func_index(b.ast(), what), this)
        };
        if matches!(self.funcs[func as usize].state, FState::Stub | FState::Undefined) {
            self.compile_func(b, func);
        }
        let f = &self.funcs[func as usize];
        let (FState::Ready, RetKind::Scalar(rt), Some(tr)) = (f.state, f.ret, f.this_reg) else {
            return None;
        };
        let TyKind::Ptr(ch) = self.types.kind(rt) else { return None };
        let fr = self.stack.last()?;
        let top = fr.fb as usize + self.funcs[fr.func as usize].nregs as usize + 1;
        self.grow_regs(top + tr as usize + 1);
        self.regs[top + tr as usize] = u128::from(this);
        let silent = std::mem::replace(&mut self.run.silent, true);
        let r = self.exec(b, func, top as u32);
        self.run.silent = silent;
        r.ok()?;
        let mut s = self.regs[top] as u64;
        let mut out = Vec::new();
        while out.len() < 1024 {
            let c = self.load(s, ch, ak::READ).ok()?;
            if c == 0 {
                return Some(String::from_utf8_lossy(&out).into());
            }
            out.push(c as u8);
            s = self.ptr_add(s, 1, ch).ok()?;
        }
        None
    }
}
