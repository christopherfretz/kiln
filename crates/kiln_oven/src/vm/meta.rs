//! The reflection intrinsics (`bi::META`; spec/reflection.md §3). A
//! `std::meta::info` value is the [`kiln_ast::ReflectionId`]'s bits (null
//! is 0); the work is sema's, reached through the single
//! [`SemaBridge::meta`] entry point, which holds no borrow of the Oven's
//! state across the call (the running Oven is lent back to sema for
//! anything that evaluates).
//!
//! Each op's purity class ([`MetaOp::purity`]) decides what the call does
//! to memoization: a `Stable` op leaves the evaluation pure, the others
//! mark it impure (no call that reached one is memoized), and a
//! `Mutating` op fails silently in a trial.

use super::Fault;
use crate::compile::TrapInfo;
use crate::kbc::ak;
use crate::meta::materialize::Recipe;
use crate::meta::value::recipe_elem;
use crate::meta::{MAX_META_ARGS, MetaCtx, MetaError, MetaErrorKind, MetaOut, OvenFault, Purity};
use crate::ty::OTy;
use crate::{MetaOp, Note, Oven, SemaBridge};

/// The longest string [`Oven::meta_c_string`] reads (a bound on a
/// runaway string).
const MAX_NAME: u64 = 4096;

impl Oven {
    /// Runs reflection intrinsic `sub` (a [`MetaOp`] code) of the
    /// `Op::Builtin` at `pc`: its `n` arguments are in the consecutive
    /// registers from `base` of the frame at `fb`; `t1` is the element
    /// type of the last pointer argument, `t2` the result type.
    pub(super) fn exec_meta(
        &mut self,
        b: &mut dyn SemaBridge,
        pc: usize,
        fb: usize,
        sub: u32,
        (base, n): (usize, usize),
        (t1, t2): (u32, u32),
    ) -> Result<u128, Fault> {
        let Some(op) = MetaOp::from_code(sub) else { return Err(Fault::Invalid) };
        if n > MAX_META_ARGS {
            return Err(Fault::Invalid);
        }
        let mut args = [0u128; MAX_META_ARGS];
        args[..n].copy_from_slice(&self.regs[fb + base..fb + base + n]);
        // The call's site: recipes and exception path (spec/reflection.md
        // §3.5, §8.2).
        let (dst, sidx) = (self.code[pc + 1] as usize, self.code[pc + 4] as usize);
        let site = sidx.checked_sub(1).and_then(|i| self.meta.sites.get(i)).copied();
        if let Some(s) = site {
            for (i, a) in args[..n].iter_mut().enumerate() {
                if let Some(rc) = s.ranges[i] {
                    *a = self.range_arg(rc, *a as u64)?;
                }
            }
        }
        // A trial for the bridge is a fold or a speculative checkpoint
        // (spec/reflection.md §4.3). The Transactional checkpoint
        // `substitute` opens around an instantiation is not one: a
        // variable template's initializer evaluated there is a real
        // manifestly constant-evaluated evaluation, so its traits and
        // queries may instantiate and evaluate (WP-538). A `Mutating` op
        // still fails under any checkpoint: a rollback cannot undo it.
        let trial = self.run.fold || self.mem.in_speculative_trial();
        let no_mutation = trial || self.mem.in_trial();
        // A value op, or `type_order` (its `strong_ordering` copied whole).
        let value_op = matches!(op, MetaOp::Value(_) | MetaOp::Trait(crate::TraitOp::TypeOrder));
        // A value op's range argument: its element type (WP-479); a
        // trait's `reflection_range` argument: `info` (WP-484).
        let elem = site
            .filter(|_| value_op || matches!(op, MetaOp::Trait(_)))
            .and_then(|s| s.ranges.iter().flatten().find_map(|&rc| recipe_elem(&self.types, rc)))
            .unwrap_or(OTy(t1));
        // The function whose body holds the call (thunks have none).
        let scope = self.stack.last().and_then(|f| self.funcs[f.func as usize].decl);
        // The first register above the calling frame's: where anything
        // the bridge has the Oven evaluate or thaw runs.
        let cur = self.stack[self.stack.len() - 1].func;
        let top = fb + self.funcs[cur as usize].nregs as usize + 1;
        let top = u32::try_from(top).map_err(|_| Fault::Invalid)?;
        let ctx = MetaCtx {
            scope,
            loc: self.site(pc).loc,
            plainly: self.plain(),
            trial,
            elem,
            callee: site.map(|s| s.callee),
            top,
        };
        match op.purity() {
            Purity::Stable => {}
            Purity::PointDependent => self.run.impure += 1,
            Purity::Mutating => {
                self.run.impure += 1;
                if no_mutation {
                    // Under a Transactional checkpoint alone the
                    // evaluation is reported: say why it stopped.
                    let notes = if trial {
                        Vec::new()
                    } else {
                        let msg = format!(
                            "'{}' cannot run while a substitution is being tried \
                             (std::meta::substitute)",
                            op.display()
                        );
                        vec![Note::new(ctx.loc, msg)]
                    };
                    return Err(self.meta_trap(notes));
                }
            }
        }
        let mut out = MetaOut::Unset;
        let mark = self.refl_mark(b);
        let t = self.hook_meta_begin();
        let r = b.meta(op, &args[..n], &mut out, &ctx, self);
        if let Some(t) = t {
            Oven::hook_meta_end(t, op);
        }
        self.charge_refl(b, mark, matches!(op, MetaOp::Value(_) | MetaOp::Query(_)))?;
        if let Err(e) = r {
            return Err(match e.kind {
                MetaErrorKind::Fault(OvenFault(f)) => f,
                MetaErrorKind::Failed => {
                    if let Some(s) = site
                        && let Some(x) = s.exc
                        && !trial
                        && self.meta_handler()
                        && let Some(first) = e.notes.first()
                        && let Some(t) = b.meta_throw(op, s.callee, &first.msg, ctx.loc)
                    {
                        // Thrown by the block after the call (`bi::META_EXC`).
                        self.meta.pending = Some(t);
                        self.regs[fb + x.flag as usize] = 1;
                        return Ok(0);
                    }
                    let mut notes = e.notes;
                    if notes.is_empty() && !trial {
                        // A silent failure is for a trial, whose notes are
                        // dropped; anywhere else the bridge owes a reason
                        // (WP-538: a trait silently refused under
                        // `substitute`'s checkpoint gave a bare "not a
                        // constant expression").
                        let msg =
                            format!("kiln: internal: '{}' failed without a reason", op.display());
                        notes.push(Note::new(ctx.loc, msg));
                    }
                    self.meta_trap(notes)
                }
                MetaErrorKind::NotImplemented { .. } => self.meta_trap(e.notes),
            });
        }
        if op.purity() == Purity::Mutating {
            // `define_aggregate` completed a class: its provisional slot
            // is rebuilt now, so the rest of this evaluation sees it
            // complete (spec/reflection.md §6.2; O(1) when nothing did).
            self.types.refresh(b);
            self.mp_refresh(b);
        }
        let unfit = |s: &mut Self| {
            let msg = format!("kiln: internal: '{}' produced no result of its type", op.display());
            Err(s.meta_trap(vec![Note::new(ctx.loc, msg)]))
        };
        match out {
            MetaOut::Scalar(v) => Ok(v),
            MetaOut::Info(r) => Ok(u128::from(r.bits())),
            MetaOut::Frozen(_) | MetaOut::Deref(_) if value_op => {
                let whole = site
                    .and_then(|s| s.result)
                    .filter(|rc| matches!(rc, Recipe::Whole))
                    .map(|_| self.regs[fb + dst] as u64);
                match self.meta_value_out(b, out, OTy(t2), whole, top)? {
                    Some(v) => Ok(v),
                    None => unfit(self),
                }
            }
            MetaOut::Frozen(cv) => Ok(self.thaw_ptr(b, cv, OTy(t2), top)),
            MetaOut::Deref(_) => unfit(self),
            MetaOut::Str(..) | MetaOut::SrcLoc(_) | MetaOut::Pair(..) | MetaOut::Infos(_) => {
                let Some((rc, s)) = site.and_then(|s| Some((s.result?, s))) else {
                    return unfit(self);
                };
                let addr = self.regs[fb + dst];
                if self.materialize_out(b, out, rc, addr as u64, (s.e, top))? {
                    Ok(addr)
                } else {
                    unfit(self)
                }
            }
            MetaOut::Unset => {
                let msg = format!("kiln: internal: '{}' produced no result", op.display());
                Err(self.meta_trap(vec![Note::new(ctx.loc, msg)]))
            }
        }
    }

    /// `bi::META_EXC` at `pc`: fills the message and location arguments
    /// of the pending `std::meta::exception`'s constructor; `^^fn` is the
    /// result.
    pub(super) fn exec_meta_exc(
        &mut self,
        b: &mut dyn SemaBridge,
        pc: usize,
        fb: usize,
    ) -> Result<u128, Fault> {
        let arg = |i: usize| self.code[pc + 1 + i] as usize;
        let site = arg(3).checked_sub(1).and_then(|i| self.meta.sites.get(i)).copied();
        let (Some(s), Some(t)) = (site, self.meta.pending.take()) else {
            return Err(Fault::Invalid);
        };
        let Some(x) = s.exc else { return Err(Fault::Invalid) };
        let (what, at) = (self.regs[fb + arg(1)] as u64, self.regs[fb + arg(2)] as u64);
        let cur = self.stack[self.stack.len() - 1].func;
        let top = fb + self.funcs[cur as usize].nregs as usize + 1;
        let top = u32::try_from(top).map_err(|_| Fault::Invalid)?;
        let ok = self.materialize_out(b, MetaOut::Str(t.what, t.len), x.what, what, (s.e, top))?
            && self.materialize_out(b, MetaOut::SrcLoc(t.at), x.at, at, (s.e, top))?;
        if !ok {
            return Err(Fault::Invalid);
        }
        Ok(u128::from(t.from.bits()))
    }

    /// Whether a try-block of the running evaluation encloses the current
    /// point (an exception thrown here may be caught).
    fn meta_handler(&self) -> bool {
        let entry = self.run.entry;
        self.cleanups
            .iter()
            .rposition(|c| c.func == super::exc::TRY_MARK)
            .is_some_and(|i| self.cleanups[i].depth as usize > entry)
    }

    /// The narrow string at `p` (a pointer argument of the intrinsic
    /// running with `ctx`, whose elements are of the call's last pointer
    /// argument's element type), as written, up to 4096 characters.
    ///
    /// # Errors
    /// The evaluator's fault reading it (out of bounds, uninitialized).
    pub fn meta_c_string(&self, p: u128, ctx: &MetaCtx) -> Result<String, MetaError> {
        let fault = |f| MetaError { notes: Vec::new(), kind: MetaErrorKind::Fault(OvenFault(f)) };
        let (p, ct) = (p as u64, ctx.elem);
        let n = self.bi_strlen(p, ct).map_err(fault)?.min(MAX_NAME);
        let mut bytes = Vec::with_capacity(n as usize);
        let mut q = p;
        for _ in 0..n {
            bytes.push(self.load(q, ct, ak::READ).map_err(fault)? as u8);
            q = self.ptr_add(q, 1, ct).map_err(fault)?;
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Element `i` of the `std::meta::info` array at `p` (the pointer
    /// argument of a `(const info*, size_t)` range intrinsic, so `ctx.elem`
    /// is `info`; spec/reflection-traits.md §1.3). Reads Oven memory in
    /// place: no copy of the range is made.
    ///
    /// # Errors
    /// [`MetaError::invalid`] if the pointer's element type is not `info`
    /// or the element holds no reflection; the evaluator's fault reading
    /// it (out of bounds, uninitialized).
    pub fn meta_info_at(
        &self,
        p: u128,
        i: u64,
        ctx: &MetaCtx,
    ) -> Result<kiln_ast::ReflectionId, MetaError> {
        let fault = |f| MetaError { notes: Vec::new(), kind: MetaErrorKind::Fault(OvenFault(f)) };
        let ct = ctx.elem;
        if self.types.kind(ct) != crate::ty::TyKind::Reflection {
            return Err(MetaError::invalid());
        }
        let q = self.ptr_add(p as u64, i128::from(i), ct).map_err(fault)?;
        let v = self.load(q, ct, ak::READ).map_err(fault)?;
        kiln_ast::ReflectionId::from_bits(v).ok_or_else(MetaError::invalid)
    }

    /// The fault reporting a bridge failure: its first note is the
    /// reason, the rest supporting notes; none is a silent failure.
    fn meta_trap(&mut self, notes: Vec<Note>) -> Fault {
        let mut it = notes.into_iter();
        let info = match it.next() {
            Some(first) => TrapInfo { loc: first.loc, msg: first.msg, extra: it.collect() },
            None => {
                TrapInfo { loc: kiln_source::Loc::INVALID, msg: String::new(), extra: Vec::new() }
            }
        };
        let i = self.traps.len() as u32;
        self.traps.push(info);
        Fault::Trap(i)
    }
}
