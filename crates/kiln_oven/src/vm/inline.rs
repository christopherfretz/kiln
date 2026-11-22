//! Inlining of small leaf callees at their call sites (spec/oven.md
//! "Inlining", WP-1924).
//!
//! The first time a direct `Call` enters a callee whose compiled body
//! may be inlined (`Func::inline`, `kbc::inline::inlinable`), the VM
//! appends a copy of that body to the code, its registers renumbered
//! into the caller's frame (the callee's window starts at the call's arg
//! base, so callee register `r` is caller register `base + r`), its
//! jumps and sites moved with it and each `Ret` turned into an `InlRet`
//! to the call's successor; the call instruction becomes `Inl` naming
//! the copy ([`InlSite`]). A later execution of the site runs the copy
//! in the caller's frame: no frame is pushed and none popped, the step,
//! call and depth counters move as the call moved them, and a fault in
//! the copy rebuilds the frame the call would have had
//! ([`Oven::inline_frame`]) before anything reads the stack. When the
//! call must be a real one (the depth limit, a potential-constant check,
//! the per-call hooks), `Inl` calls the callee as `Call` did.

use crate::kbc::verify::{K, kinds};
use crate::kbc::{self, Op};
use crate::vm::{BOTTOM, Frame};
use crate::{Oven, compile::Site};

/// An inlined call site: `code[call_pc]` is `Inl` naming this entry,
/// and `code[entry..end]` the copy of `callee`'s body, whose registers
/// are the caller's from the call's arg `base` on, `need` of them past
/// the caller's frame base (`base` + the callee's own + 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub(crate) struct InlSite {
    pub callee: u32,
    pub call_pc: u32,
    pub base: u32,
    pub entry: u32,
    pub end: u32,
    pub need: u32,
}

impl Oven {
    /// Inlines `callee` at the direct `Call` at `call_pc` (arg base
    /// `base`), which just entered it (see the module docs). Nothing
    /// happens when inlining is off ([`Oven::set_inlining`]), while a
    /// unit is being compiled (it owns the end of the code) or when the
    /// site is no `Call` any more.
    #[cold]
    #[inline(never)]
    pub(super) fn inline_call(&mut self, call_pc: usize, callee: u32, base: usize) {
        if !self.inlining
            || self.padded != self.code.len()
            || self.code.get(call_pc) != Some(&(Op::Call as u32))
        {
            return;
        }
        let f = &self.funcs[callee as usize];
        let (from, to) = (f.entry as usize, f.code_end as usize);
        let (Ok(idx), Ok(base32), Ok(need)) = (
            u32::try_from(self.inl.len()),
            u32::try_from(base),
            u32::try_from(base + f.nregs as usize + 1),
        ) else {
            return;
        };
        if !f.inline
            || to <= from
            || to > self.code.len()
            || !self.inlinable_body(from, to, f.nregs)
        {
            return;
        }
        kbc::unpad(&mut self.code, &mut self.padded);
        let entry = self.code.len();
        let ret_to = (call_pc + 3) as u32;
        // Where each instruction of the body lands (`InlRet` is longer
        // than `Ret`): the jump targets and sites move by this map.
        let mut at = Vec::with_capacity(to - from + 1);
        let mut pc = from;
        let mut len = 0;
        while pc < to {
            let op = Op::from_u8(self.code[pc] as u8);
            let n = if op == Op::Ret { Op::InlRet.operands() } else { op.operands() };
            at.push((pc, entry + len));
            len += 1 + n;
            pc += 1 + op.operands();
        }
        let moved = |old: usize| {
            let i = at.partition_point(|&(p, _)| p < old);
            at.get(i).filter(|&&(p, _)| p == old).map_or(entry + len, |&(_, n)| n)
        };
        let mut out = Vec::with_capacity(len);
        for &(pc, _) in &at {
            let w = self.code[pc];
            let op = Op::from_u8(w as u8);
            if op == Op::Ret {
                let src = base32 + self.code[pc + 1];
                out.extend_from_slice(&[Op::InlRet as u32, base32, src, ret_to]);
                continue;
            }
            out.push(w);
            let ks = kinds(op, w >> 8);
            debug_assert_eq!(ks.len(), op.operands());
            for (i, &k) in ks.iter().enumerate() {
                let v = self.code[pc + 1 + i];
                out.push(match k {
                    K::R => v + base32,
                    K::Ro if v != u32::MAX => v + base32,
                    K::J => moved(v as usize) as u32,
                    _ => v,
                });
            }
        }
        debug_assert_eq!(out.len(), len);
        // The sites: the one covering the body's first instruction, then
        // each inside it, at their new pcs (`Oven::site` takes the last
        // at or before a pc).
        let first = self.site(from);
        let lo = self.sites.partition_point(|s| (s.pc as usize) < from);
        let hi = self.sites.partition_point(|s| (s.pc as usize) < to);
        let mut sites = Vec::with_capacity(hi - lo + 1);
        sites.push(Site { pc: entry as u32, ..first });
        for s in &self.sites[lo..hi] {
            sites.push(Site { pc: moved(s.pc as usize) as u32, ..*s });
        }
        self.code.extend_from_slice(&out);
        let end = self.code.len();
        kbc::pad(&mut self.code, &mut self.padded);
        self.sites.extend(sites);
        self.inl.push(InlSite {
            callee,
            call_pc: call_pc as u32,
            base: base32,
            entry: entry as u32,
            end: end as u32,
            need,
        });
        self.code[call_pc] = Op::Inl as u32;
        self.code[call_pc + 1] = idx;
    }

    /// Whether `code[from..to]` still is a body `inline_call` may copy:
    /// whitelisted instructions only (`kbc::inline::inlinable`), ending
    /// at `to`, with registers within the callee's `nregs` and jumps
    /// within the body. Compilation
    /// guarantees it; a snapshot's `Func` is checked here, once per site,
    /// rather than trusted.
    fn inlinable_body(&self, from: usize, to: usize, nregs: u32) -> bool {
        if kbc::inline::inlinable(&self.code, from, to).is_err() {
            return false;
        }
        let mut pc = from;
        while pc < to {
            let w = self.code[pc];
            let op = Op::from_u8(w as u8);
            let regs_ok = kinds(op, w >> 8).iter().enumerate().all(|(i, k)| {
                let v = self.code.get(pc + 1 + i).copied().unwrap_or(u32::MAX);
                match k {
                    K::R => v <= nregs,
                    K::Ro => v <= nregs || v == u32::MAX,
                    K::J => (from..to).contains(&(v as usize)),
                    _ => true,
                }
            });
            if !regs_ok {
                return false;
            }
            pc += 1 + op.operands();
        }
        pc == to
    }

    /// The inlined site whose copy holds `pc`, if any.
    pub(crate) fn inline_site_at(&self, pc: usize) -> Option<InlSite> {
        if self.inl.first().is_none_or(|s| pc < s.entry as usize) {
            return None;
        }
        let i = self.inl.partition_point(|s| s.entry as usize <= pc);
        let s = self.inl[i.checked_sub(1)?];
        (pc < s.end as usize).then_some(s)
    }

    /// After a run stopped at `pc` (a fault): when `pc` is in an inlined
    /// copy, pushes the frame its call would have had over the caller's,
    /// so the call stack, the "in call to" notes and every check that
    /// walks the stack see the call (spec/oven.md "Inlining"). The
    /// runner truncates the stack to its entry afterwards, as it does
    /// for the frames a fault leaves.
    #[cold]
    #[inline(never)]
    pub(super) fn inline_frame(&mut self, pc: usize) {
        let Some(s) = self.inline_site_at(pc) else { return };
        let Some(top) = self.stack.last() else { return };
        let fb = top.fb + s.base;
        self.stack.push(Frame {
            func: s.callee,
            ret_pc: s.call_pc + 3,
            fb,
            mark: BOTTOM,
            call_pc: s.call_pc,
            memo: false,
            // A light frame keeps its caller's serial (spec/oven.md
            // "Light frames").
            serial: self.mem.cur_frame,
        });
    }
}
