//! The reflection intrinsics (`bi::META`; WP-267 spike, `-freflection`;
//! spec/oven.md "Reflection runtime"). A `std::meta::info` value is the
//! [`ReflectionId`]'s bits (null is 0); the work is sema's, reached
//! through the `reflect_*`/`define_member` bridge methods, which hold no
//! borrow of the Oven's state across the call (the running Oven is lent
//! back to sema for anything that evaluates).
//!
//! Every intrinsic marks the evaluation impure: what it reads (a class's
//! members, a name) can change as the TU grows, and `define_member`
//! changes it, so no call that reached one is memoized.

use kiln_ast::ReflectionId;

use super::Fault;
use crate::compile::TrapInfo;
use crate::kbc::ak;
use crate::ty::OTy;
use crate::{MetaOp, Note, Oven, SemaBridge};

/// The longest name `define_member` reads (a bound on a runaway string).
const MAX_NAME: u64 = 4096;

impl Oven {
    /// Runs reflection intrinsic `sub` (a [`MetaOp`] code) of the
    /// `Op::Builtin` at `pc` on the argument values `a`..`c`; `t1` is the
    /// name argument's character type, `t2` the result type.
    #[allow(clippy::too_many_arguments, reason = "the instruction's operands")]
    pub(super) fn exec_meta(
        &mut self,
        b: &mut dyn SemaBridge,
        pc: usize,
        fb: usize,
        sub: u32,
        (a, bb, c): (u128, u128, u128),
        (t1, t2): (u32, u32),
    ) -> Result<u128, Fault> {
        let Some(op) = MetaOp::from_code(sub) else { return Err(Fault::Invalid) };
        self.run.impure += 1;
        let loc = self.site(pc).loc;
        let refl = |v: u128| ReflectionId::from_bits(v).ok_or(Fault::Invalid);
        let r = match op {
            MetaOp::TypeOf => b.reflect_type_of(refl(a)?, loc).map(|t| u128::from(t.bits())),
            MetaOp::NameOf => match b.reflect_name(refl(a)?, loc) {
                Ok(cv) => {
                    let cur = self.stack[self.stack.len() - 1].func;
                    let top = fb + self.funcs[cur as usize].nregs as usize + 1;
                    let top = u32::try_from(top).map_err(|_| Fault::Invalid)?;
                    return Ok(self.thaw_ptr(b, cv, OTy(t2), top));
                }
                Err(n) => Err(n),
            },
            MetaOp::MembersCount => b.reflect_members(refl(a)?, loc, self).map(|m| m.len() as u128),
            MetaOp::MemberAt => {
                let i = bb;
                b.reflect_members(refl(a)?, loc, self).and_then(|m| {
                    usize::try_from(i)
                        .ok()
                        .and_then(|i| m.get(i))
                        .map(|r| u128::from(r.bits()))
                        .ok_or_else(|| {
                            vec![Note::new(
                                loc,
                                format!(
                                    "member index {i} is out of range for a class with {} \
                                     members",
                                    m.len()
                                ),
                            )]
                        })
                })
            }
            MetaOp::DefineMember => {
                let (cls, ty) = (refl(a)?, refl(bb)?);
                let name = self.read_name(c as u64, OTy(t1))?;
                b.define_member(cls, ty, &name, loc, self).map(|m| u128::from(m.bits()))
            }
        };
        r.map_err(|notes| self.meta_trap(notes))
    }

    /// The narrow string at `p` (element type `ct`), as written.
    fn read_name(&self, p: u64, ct: OTy) -> Result<String, Fault> {
        let n = self.bi_strlen(p, ct)?.min(MAX_NAME);
        let mut bytes = Vec::with_capacity(n as usize);
        let mut q = p;
        for _ in 0..n {
            bytes.push(self.load(q, ct, ak::READ)? as u8);
            q = self.ptr_add(q, 1, ct)?;
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
