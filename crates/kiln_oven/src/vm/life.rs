//! Object lifetime outside `std::allocator` storage (WP-222; spec/oven.md
//! "Object lifetime"). The first explicit destruction of an object in a
//! frame or `new` allocation makes the allocation [`flags::LIFE`]-tracked
//! (`Memory::set_lifetime`); its bytes' lifetime bits are then consulted
//! by every access (`Memory::check`) and by the implicit destructions
//! here, which diagnose destroying an object whose lifetime has already
//! ended ([basic.life]/9). Untracked allocations pay one flag test.
//!
//! A class containing a union is never diagnosed as destroyed: ending the
//! lifetime of its active member (`u.t.~T()`, `std::optional::reset`)
//! leaves the union with no active member, not dead, and the bytes do not
//! tell the two apart.

use super::{Cleanup, Fault, NO_CALL, UNWIND_ONLY};
use crate::Oven;
use crate::kbc::{Op, ak};
use kiln_ast::Ast;
use kiln_ast::decl::DeclKind;

use crate::mem::{MemFault, Origin, Ptr, flags};
use crate::ty::{OTy, TyKind};

/// Which of a frame's objects `Oven::scope_life` checks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Objs {
    All,
    Locals,
    Params,
}

impl Oven {
    /// Whether the `ty` object at `raw` is one whose lifetime an explicit
    /// destruction ended (no byte of it within a lifetime).
    fn life_ended(&self, raw: u64, ty: OTy) -> bool {
        if !self.mem.unconstructed.is_empty() && self.mem.empty_unconstructed(raw, ty) {
            return true;
        }
        let i = self.types.info(ty);
        !i.has_union
            && !matches!(i.kind, TyKind::Record { union: true, .. })
            && self.mem.ended(raw, self.types.size(ty))
            && (self.mem.unconstructed.is_empty()
                || !self.mem.holds_unconstructed(raw, self.types.size(ty), ty))
    }

    /// The implicit destruction of the `ty` object at `raw`: arrays
    /// element by element from the last ([class.dtor]/13 order).
    fn destroy_check(&self, raw: u64, ty: OTy) -> Result<(), Fault> {
        if let TyKind::Array { elem, n } = self.types.kind(ty) {
            let es = self.types.size(elem);
            let off = u64::from(Ptr::decode(raw).off);
            for i in (0..n).rev() {
                self.destroy_check(Ptr::with_off(raw, off + i * es, false), elem)?;
            }
            return Ok(());
        }
        if self.life_ended(raw, ty) {
            return Err(Fault::DestroyEnded { p: raw, ty });
        }
        Ok(())
    }

    /// Before a registered destruction runs (`run_cleanup`, the running
    /// function's frame at `fb`).
    pub(super) fn cleanup_life(&self, c: &Cleanup, fb: usize) -> Result<(), Fault> {
        // Registered destructions are of frame objects.
        if !self.mem.life_frame || c.this == 0 || !self.tracked(c.this) {
            return Ok(());
        }
        match self.destroy_check(c.this, c.ty) {
            // An empty base its constructor never ran, destroyed by its
            // derived class's destructor: clang prints the base's lvalue
            // as the derived object (WP-401); the empty base's address
            // alone would designate the member sharing it.
            Err(Fault::DestroyEnded { .. }) if self.mem.empty_unconstructed(c.this, c.ty) => {
                let derived = self.stack.last().and_then(|fr| {
                    let f = &self.funcs[fr.func as usize];
                    Some((self.regs[fb + f.this_reg? as usize] as u64, f.this_ty?))
                });
                let (p, ty) = derived.unwrap_or((c.this, c.ty));
                Err(Fault::DestroyEnded { p, ty })
            }
            r => r,
        }
    }

    /// The instruction a fault of `cleanup_life` for `c` (run at `pc`) is
    /// reported at: a subobject destroyed by its destructor's epilogue
    /// (`PushCleanup` aux 2) at the outermost such destructor's call (the
    /// `delete`), as clang's `HandleDestructionImpl` passes its call range
    /// down (WP-401); any other destruction at `pc`.
    pub(super) fn cleanup_fault_pc(&self, c: &Cleanup, pc: usize) -> usize {
        let sub = |at: usize| {
            self.code
                .get(at)
                .is_some_and(|&w| Op::from_u8(w as u8) == Op::PushCleanup && (w >> 8) & 2 != 0)
        };
        if !sub((c.pc & !UNWIND_ONLY) as usize) {
            return pc;
        }
        let frames = self.stack.get(self.run.entry + 1..).unwrap_or_default();
        let mut at = pc;
        for fr in frames.iter().rev() {
            if fr.call_pc == NO_CALL {
                break;
            }
            at = fr.call_pc as usize;
            if !sub(at) {
                break;
            }
        }
        at
    }

    /// Releasing the frame objects above `mark` (leaving a scope: `All`;
    /// returning: the callee's `Locals`, then, in the caller, its
    /// `Params`): each tracked one, innermost first, must still be within
    /// its lifetime. Objects with a destructor were checked by
    /// `cleanup_life`; this catches the trivially destructible ones.
    pub(super) fn scope_life(&self, ast: &Ast, mark: u64, objs: Objs) -> Result<(), Fault> {
        let n = (mark >> 32) as usize;
        if n >= self.mem.life_hi {
            return Ok(());
        }
        for &s in self.mem.live.get(n..).unwrap_or_default().iter().rev() {
            let a = &self.mem.slots[s as usize];
            if a.flags & flags::LIFE == 0 {
                continue;
            }
            let param = matches!(a.origin, Origin::Var(d)
                if ast.decls.get(d).kind() == DeclKind::ParmVar);
            if objs == Objs::Locals && param || objs == Objs::Params && !param {
                continue;
            }
            let raw = Ptr { slot: s, generation: a.generation, past: false, off: 0 }.encode();
            self.destroy_check(raw, a.ty)?;
        }
        Ok(())
    }

    /// `delete` of the (non-`std::allocator`) heap object holding `raw`.
    pub(super) fn delete_life(&self, raw: u64) -> Result<(), Fault> {
        if raw == 0 {
            return Ok(());
        }
        let p = Ptr::decode(raw);
        let Ok(a) = self.mem.alloc_of(p) else { return Ok(()) };
        if a.flags & flags::LIFE == 0 {
            return Ok(());
        }
        self.destroy_check(Ptr { off: 0, past: false, ..p }.encode(), a.ty)
    }

    /// A member call on the `ty` object at `raw` (clang's
    /// `checkDynamicType` with `AK_MemberCall`).
    pub(super) fn member_call_life(&self, raw: u64, ty: OTy) -> Result<(), Fault> {
        if self.tracked(raw) && self.life_ended(raw, ty) {
            return Err(Fault::Mem { f: MemFault::Lifetime, ak: ak::MEMBER_CALL, ty, p: raw });
        }
        Ok(())
    }

    /// Assigning a member of the `uty` union at `raw` makes it active
    /// ([class.union.general]/6), beginning its lifetime even after an
    /// explicit destruction ended the previous member's.
    pub(super) fn revive_union(&mut self, raw: u64, uty: OTy) {
        if raw == 0 {
            return;
        }
        let size = self.types.size(uty);
        // A union destroyed itself (or with its enclosing object) has no
        // entry (`end_life`): assigning its member begins no lifetime
        // ([class.union.general]/6 needs a union within its lifetime), and
        // `set_active`'s access check reports it. A union whose active
        // member alone was destroyed keeps its entry and is revived.
        if matches!(self.mem.check(raw, size, false), Err(MemFault::Lifetime))
            && !self.mem.unions.contains_key(&Ptr::union_key(
                raw,
                u64::from(Ptr::decode(raw).off),
                uty,
            ))
        {
            return;
        }
        // Faults surface in `set_active`, which checks the access.
        let _ = self.mem.revive(raw, size);
    }

    fn tracked(&self, raw: u64) -> bool {
        self.mem.alloc_of(Ptr::decode(raw)).is_ok_and(|a| a.flags & flags::LIFE != 0)
    }
}
