//! `__builtin_kiln_host_call` (`bi::HOST_CALL`; WP-1990,
//! spec/host-calls.md §2, §4): whether the call may run, the range
//! checks, the handler call and its results.

use kiln_diag::{Diag, DiagId};
use kiln_source::Span;

use super::Fault;
use crate::arith::canon;
use crate::host::{HostMode, HostOut};
use crate::kbc::ak;
use crate::mem::MemFault;
use crate::ty::{OTy, TyKind};
use crate::{Oven, SemaBridge};

/// The arguments' names, by their index in the call.
const NAME_IN: &str = "in";
const NAME_BIN: &str = "bin";
const NAME_OUT: &str = "out";
const NAME_BOUT: &str = "bout";

/// The width and signedness of integer element type `t`.
fn int_shape(o: &Oven, t: OTy) -> Result<(u32, bool), Fault> {
    match o.types.kind(t) {
        TyKind::Int { width, signed } => Ok((u32::from(width), signed)),
        _ => Err(Fault::Invalid),
    }
}

/// A failed range check of argument `arg`.
fn bad(arg: &str, why: &str) -> Fault {
    Fault::Host(format!("'{arg}' of __builtin_kiln_host_call {why}").into_boxed_str())
}

impl Oven {
    /// The call whose nine arguments are in the registers from `base`
    /// (`ti`, `tb`: the integer and byte element types).
    pub(super) fn exec_host_call(
        &mut self,
        b: &mut dyn SemaBridge,
        pc: usize,
        base: usize,
        (ti, tb): (OTy, OTy),
    ) -> Result<u128, Fault> {
        // Run or refused, a host call is impure: nothing pending around
        // it is memoized (§5).
        self.run.impure += 1;
        let speculative = match self.host.mode {
            HostMode::Silent => return Err(Fault::Quiet),
            HostMode::Speculative => true,
            HostMode::Allowed => {
                self.run.fold || self.mem.in_speculative_trial() || self.trial_forbids_effects()
            }
        };
        if speculative {
            // A hard error, at the call and with the call stack (the
            // evaluation itself fails without a note of its own).
            let mut notes = self.render(b, Fault::Host("".into()), pc).into_iter();
            let loc = notes.next().map_or(self.site(pc).loc, |n| n.loc);
            let mut d = Diag::new(DiagId::oven_host_call_speculative, Span::empty(loc));
            // Each call stack once: calls reached from two places are two
            // errors, the same evaluation tried twice is one.
            let mut key = 0u64;
            for n in notes {
                key = key.rotate_left(5) ^ u64::from(n.loc.to_raw());
                d = d.note(n.msg, Span::empty(n.loc));
            }
            d = d.context(key);
            b.diag(d);
            return Err(Fault::Quiet);
        }
        if self.host_cancelled() {
            return Err(Fault::Cancelled);
        }
        if self.host.handler.is_none() && b.host_calls().is_none() {
            return Err(Fault::Host("no host-call handler is registered".into()));
        }
        let mut a = [0u128; 9];
        for (i, v) in a.iter_mut().enumerate() {
            *v = *self.regs.get(base + i).ok_or(Fault::Invalid)?;
        }
        let op = a[0] as u32;
        let ins = (a[1] as u64, a[2] as u32, a[3] as u64, a[4] as u32);
        let outs = (a[5] as u64, a[6] as u32, a[7] as u64, a[8] as u32);
        // The buffers are the host's, reused across calls.
        let mut bufs = (
            std::mem::take(&mut self.host.ints_in),
            std::mem::take(&mut self.host.bytes_in),
            std::mem::take(&mut self.host.ints_out),
            std::mem::take(&mut self.host.bytes_out),
        );
        let r = self.host_call_with(b, pc, op, ins, outs, (ti, tb), &mut bufs);
        (self.host.ints_in, self.host.bytes_in, self.host.ints_out, self.host.bytes_out) = bufs;
        r
    }

    /// [`Oven::exec_host_call`] past the mode check, in `bufs` (the inputs
    /// read, the results written).
    #[allow(clippy::too_many_arguments)]
    fn host_call_with(
        &mut self,
        b: &mut dyn SemaBridge,
        pc: usize,
        op: u32,
        (pin, n_in, pbin, n_bin): (u64, u32, u64, u32),
        (pout, cap_out, pbout, cap_bout): (u64, u32, u64, u32),
        (ti, tb): (OTy, OTy),
        bufs: &mut (Vec<u64>, Vec<u8>, Vec<u64>, Vec<u8>),
    ) -> Result<u128, Fault> {
        let (ints, bytes, ints_out, bytes_out) = bufs;
        // 1. The call and its inputs' elements.
        self.host_charge(pc, 1 + u64::from(n_in) + u64::from(n_bin))?;
        // 2. Every range, before the handler runs: the inputs read in
        // order, so an output overlapping an input is defined.
        let (wi, si) = int_shape(self, ti)?;
        let (wb, sb) = int_shape(self, tb)?;
        ints.clear();
        bytes.clear();
        self.host_read(pin, n_in, ti, NAME_IN, |v| ints.push(v as u64))?;
        self.host_read(pbin, n_bin, tb, NAME_BIN, |v| bytes.push(v as u8))?;
        self.host_writable(pout, cap_out, ti, NAME_OUT)?;
        self.host_writable(pbout, cap_bout, tb, NAME_BOUT)?;
        // 3. The handler.
        let mut out = HostOut::new(ints_out, bytes_out, cap_out as usize, cap_bout as usize);
        self.host.calls += 1;
        // The registered handler, else a session run's (spec/eval-session.md §6).
        let r = match self.host.handler.as_mut() {
            Some(h) => h.call(op, ints, bytes, &mut out),
            None => match b.host_calls() {
                Some(h) => h.call(op, ints, bytes, &mut out),
                None => return Err(Fault::Invalid),
            },
        };
        let v = match r {
            Ok(v) => v,
            Err(e) => {
                return Err(Fault::Host(format!("host call failed: {}", e.message()).into()));
            }
        };
        // 4. The results: exactly the elements written.
        for (n, cap, what) in
            [(ints_out.len(), cap_out, "integers"), (bytes_out.len(), cap_bout, "bytes")]
        {
            if n > cap as usize {
                return Err(Fault::Host(
                    format!("host call handler wrote {n} {what} into a capacity of {cap}").into(),
                ));
            }
        }
        for (i, &x) in ints_out.iter().enumerate() {
            let q = self.ptr_add(pout, i as i128, ti)?;
            self.store(q, ti, canon(u128::from(x), wi, si), ak::ASSIGN, false)?;
        }
        for (i, &x) in bytes_out.iter().enumerate() {
            let q = self.ptr_add(pbout, i as i128, tb)?;
            self.store(q, tb, canon(u128::from(x), wb, sb), ak::ASSIGN, false)?;
        }
        self.host_charge(pc, (ints_out.len() + bytes_out.len()) as u64)?;
        Ok(u128::from(v))
    }

    /// Charges `n` steps to the call at `pc`.
    fn host_charge(&mut self, pc: usize, n: u64) -> Result<(), Fault> {
        let before = self.run.counters.steps;
        let need = before.saturating_add(n);
        if need > self.step_stop {
            match self.step_over(need) {
                Ok(()) => {}
                Err(true) => return Err(Fault::Cancelled),
                Err(false) => return Err(Fault::Steps(self.site(pc).loc)),
            }
        }
        self.run.counters.steps = before + n;
        Ok(())
    }

    /// The pointer `p` of argument `arg` designates `n` elements of type
    /// `t` (any pointer when `n` is 0).
    fn host_range(&self, p: u64, n: u32, t: OTy, arg: &str) -> Result<(), Fault> {
        if p == 0 {
            return Err(bad(arg, &format!("is null with a length of {n}")));
        }
        self.ptr_add(p, i128::from(n), t)
            .map(|_| ())
            .map_err(|_| bad(arg, &format!("does not point to {n} consecutive elements")))
    }

    /// Reads the `n` elements at `p` (argument `arg`, element type `t`),
    /// each initialized, into `push`.
    fn host_read(
        &self,
        p: u64,
        n: u32,
        t: OTy,
        arg: &str,
        mut push: impl FnMut(u128),
    ) -> Result<(), Fault> {
        if n == 0 {
            return Ok(());
        }
        self.host_range(p, n, t, arg)?;
        for i in 0..n {
            let q = self.ptr_add(p, i128::from(i), t)?;
            match self.load(q, t, ak::READ) {
                Ok(v) => push(v),
                Err(f) => {
                    let why = match f {
                        Fault::Mem { f: MemFault::Uninit, .. } => "is uninitialized",
                        Fault::Mem { f: MemFault::Lifetime | MemFault::Dead(_), .. } => {
                            "is outside its lifetime"
                        }
                        _ => "cannot be read in a constant expression",
                    };
                    return Err(bad(arg, &format!("element {i} {why}")));
                }
            }
        }
        Ok(())
    }

    /// The `n` elements at `p` (argument `arg`, element type `t`) are
    /// writable in this evaluation (spec/host-calls.md §2.2).
    fn host_writable(&self, p: u64, n: u32, t: OTy, arg: &str) -> Result<(), Fault> {
        if n == 0 {
            return Ok(());
        }
        self.host_range(p, n, t, arg)?;
        let len = self.types.size(t).max(1) * u64::from(n);
        match self.rmw_check(p, len, t, ak::ASSIGN) {
            Ok(_) => Ok(()),
            Err(f) => {
                let why = match f {
                    Fault::Mem { f: MemFault::Const(_), .. } => "designates a const object",
                    Fault::Mem { f: MemFault::Literal(_), .. } => "designates a string literal",
                    Fault::Mem { f: MemFault::ModifyGlobal | MemFault::External(..), .. } => {
                        "designates an object whose lifetime began outside the evaluation"
                    }
                    _ => "does not designate writable storage",
                };
                Err(bad(arg, why))
            }
        }
    }
}
