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
        let trial = self.run.fold || self.mem.in_trial();
        let ctx =
            MetaCtx { scope: None, loc: self.site(pc).loc, plainly: false, trial, elem: OTy(t1) };
        match op.purity() {
            Purity::Stable => {}
            Purity::PointDependent => self.run.impure += 1,
            Purity::Mutating => {
                self.run.impure += 1;
                if trial {
                    return Err(self.meta_trap(Vec::new()));
                }
            }
        }
        let mut out = MetaOut::Unset;
        if let Err(e) = b.meta(op, &args[..n], &mut out, &ctx, self) {
            return Err(match e.kind {
                MetaErrorKind::Fault(OvenFault(f)) => f,
                MetaErrorKind::Failed | MetaErrorKind::NotImplemented { .. } => {
                    self.meta_trap(e.notes)
                }
            });
        }
        match out {
            MetaOut::Scalar(v) => Ok(v),
            MetaOut::Info(r) => Ok(u128::from(r.bits())),
            MetaOut::Frozen(cv) => {
                let cur = self.stack[self.stack.len() - 1].func;
                let top = fb + self.funcs[cur as usize].nregs as usize + 1;
                let top = u32::try_from(top).map_err(|_| Fault::Invalid)?;
                Ok(self.thaw_ptr(b, cv, OTy(t2), top))
            }
            MetaOut::Unset => {
                let msg = format!("kiln: internal: '{}' produced no result", op.display());
                Err(self.meta_trap(vec![Note::new(ctx.loc, msg)]))
            }
        }
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
