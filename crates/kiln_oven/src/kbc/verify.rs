//! The bytecode verifier of a decoded snapshot (spec/oven.md "Snapshots",
//! spec/snap.md "Threat model"): one linear pass over the restored code
//! that establishes every invariant the VM indexes by without a check, so
//! that a damaged snapshot is refused at decode instead of panicking (or
//! computing garbage) in a later evaluation.
//!
//! Invariants, per instruction (decoded from pc 0, each at the pc the
//! previous one ends at, the last ending exactly at the end of the code):
//! - the opcode is a KBC opcode and all its operand words are in the code;
//! - its owner is the compiled function with the greatest entry at or
//!   below its pc; every register operand is at most the owner's `nregs`
//!   (the VM grows the register file to `nregs + 1` past the frame base),
//!   or `u32::MAX` where the operand is optional;
//! - a jump target (`Jmp`, `JmpIf`, `JmpIfNot`, `Speculate`, `TryEnter`,
//!   `Catch`, `ContractSkip`, `ContractTrial`) is an instruction boundary
//!   with the same owner;
//! - a function operand indexes the function table (`Throw`'s destructor
//!   may also be `NO_DTOR`, `PushCleanup`'s the sentinels `EXC_FREE` and
//!   `LIFE_CHECK`: the others would name a handler pc), a global operand the global table, a type
//!   operand the type table (or `bi::DYN` / `u32::MAX` where the VM takes
//!   that as "none"), a trap operand the trap table, a `Step`'s locations
//!   `[idx, idx + aux)` the step-location pool, an origin operand the
//!   origin table (a `New`'s a heap origin);
//! - `Builtin`'s operands after `dst` are kind-specific (registers, types,
//!   constant ids, intrinsic call-site indices) and are not verified;
//! - an AST operand (`VCall`'s method and `MpCast`'s class, nonzero
//!   `DeclId`s; `LabelAddr`'s `ExprId`; `TypeId`'s `QualType`, with a
//!   nonzero type index) is returned for the AST's own range check.
//!
//! And of the tables that hold code positions: every compiled function's
//! entry and the `Unwind` pc are instruction boundaries (the latter an
//! `Unwind`), every call site's pc a boundary or the end.

use super::{Op, bi};
use crate::compile::{Func, UNCOMPILED};
use crate::vm::exc;

/// A broken invariant: what, and the instruction (its pc, and its opcode
/// when it has one).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Fail {
    pub why: &'static str,
    pub at: Option<(usize, Option<Op>)>,
}

impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.at {
            None => f.write_str(self.why),
            Some((pc, None)) => write!(f, "{} at pc {pc}", self.why),
            Some((pc, Some(op))) => write!(f, "{} ({op:?} at pc {pc})", self.why),
        }
    }
}

/// What an operand word is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum K {
    /// Not an index (an immediate, or unused).
    I,
    /// A register.
    R,
    /// A register, or `u32::MAX` for none.
    Ro,
    /// A jump target.
    J,
    /// A function.
    F,
    /// A function, or `NO_DTOR`.
    Fo,
    /// A function, or the cleanup sentinels the compiler registers
    /// (`exc::EXC_FREE`, `exc::LIFE_CHECK`).
    Fc,
    /// A global.
    G,
    /// A type.
    T,
    /// A type, or `bi::DYN` (`u32::MAX`) for none.
    To,
    /// A trap.
    Tr,
    /// The first step location of a `Step`.
    S,
    /// An origin.
    O,
    /// A heap origin (`New`).
    Oh,
    /// A nonzero AST `DeclId`.
    D,
    /// An AST `ExprId`, possibly 0.
    E,
    /// A `QualType`'s raw encoding.
    Q,
}

use K::{D, E, F, Fc, Fo, G, I, J, O, Oh, Q, R, Ro, S, T, To, Tr};

/// The operand kinds of `op` with auxiliary bits `aux`.
fn kinds(op: Op, aux: u32) -> &'static [K] {
    match op {
        Op::Nop | Op::Halt | Op::Unwind | Op::CatchBegin => &[],
        Op::Step => &[S],
        Op::Trap | Op::ContractFail => &[Tr],
        Op::Const => &[R, I, I],
        Op::Const2 | Op::EnumCheck => &[R, I, I, I, I],
        Op::Jmp | Op::TryEnter | Op::ContractSkip => &[J],
        Op::JmpIf | Op::JmpIfNot => &[R, J],
        Op::IAdd
        | Op::ISub
        | Op::IMul
        | Op::IDiv
        | Op::IRem
        | Op::IShl
        | Op::IShr
        | Op::IAnd
        | Op::IOr
        | Op::IXor
        | Op::ICmp
        | Op::FAdd
        | Op::FSub
        | Op::FMul
        | Op::FDiv
        | Op::FCmp
        | Op::MpAddr => &[R, R, R],
        Op::Mov
        | Op::INeg
        | Op::INot
        | Op::LNot
        | Op::ICast
        | Op::ToBool
        | Op::FNeg
        | Op::FCast
        | Op::IToF
        | Op::FToI
        | Op::FToBool
        | Op::PtrToBool
        | Op::CallInd
        | Op::MpActive
        | Op::MpCmp
        | Op::CovAdj => &[R, R],
        Op::PtrAdd | Op::PtrSub | Op::PtrCmp | Op::LoadLane | Op::StoreLane => &[R, R, R, T],
        Op::Field => &[R, R, I],
        Op::Downcast => &[R, R, I, T],
        Op::VBase => &[R, R, T, I],
        Op::Load | Op::Store | Op::InitStore | Op::Copy | Op::VoidCast => &[R, R, T],
        Op::LoadBits | Op::StoreBits => &[R, R, T, I],
        Op::ZeroInit | Op::RefChk | Op::Delete | Op::EndLife | Op::SetVptr => &[R, T],
        Op::Alloc => &[R, T, O],
        Op::ScopeEnter
        | Op::ScopeExit
        | Op::IsConstEval
        | Op::Ret
        | Op::MakeConst
        | Op::EndParam
        | Op::Fold => &[R],
        Op::SetActive | Op::ActiveCheck => &[R, I, T],
        Op::Call => &[F, R],
        Op::GlobalAddr | Op::BindObject => &[R, G],
        Op::New => &[R, T, R, Oh, I],
        Op::PushCleanup => &[R, Fc, T],
        Op::MpCast => &[R, D],
        Op::MpFn => &[R, R, R, Ro],
        // Per kind (`bi`): the operands after `dst` are registers, types,
        // constant ids or call-site indices; not verified here (spec/snap.md
        // "Threat model", known gaps).
        Op::Builtin => &[R, I, I, I, I, I],
        Op::VCall => &[D, R, T, Ro],
        Op::DynCast => &[R, R, T, To],
        Op::TypeId if aux & 1 != 0 => &[R, R, T, T],
        Op::TypeId => &[R, Q, T, I],
        Op::Speculate => &[R, R, J],
        Op::Cx => &[R, R, R, R, R, R],
        Op::Wide if aux & 0x3F == super::wide::CAST => &[R, R, R, T, T],
        Op::Wide => &[R, R, R, T, I],
        Op::Placement => &[R, T, To, R, I],
        Op::Throw if aux != 0 => &[I, I, I],
        Op::Throw => &[R, T, Fo],
        Op::Catch => &[R, T, J],
        Op::Cce => &[R, R, Tr],
        Op::AddrInt => &[R, R, R, I],
        Op::LabelAddr => &[R, E],
        Op::ContractTrial => &[R, J, Tr],
    }
}

/// The sizes of the tables the code indexes.
pub(crate) struct Tables<'a> {
    pub funcs: &'a [Func],
    pub nglobals: usize,
    pub ntypes: usize,
    pub ntraps: usize,
    pub nsteps: usize,
    /// For each origin, whether it is a heap origin.
    pub heap_origins: &'a [bool],
}

/// The largest AST ids the code holds (0: none), for the AST's range
/// check.
#[derive(Default, Debug)]
pub(crate) struct AstIds {
    pub decl: u32,
    pub expr: u32,
    pub ty: u32,
}

/// Verifies `code` against `t` (see the module docs): also that each of
/// `sites` is an instruction boundary or the end, and `unwind` (if any) an
/// `Unwind` instruction.
///
/// # Errors
/// The first invariant broken.
pub(crate) fn verify(
    code: &[u32],
    t: &Tables<'_>,
    sites: impl Iterator<Item = u32>,
    unwind: Option<u32>,
) -> Result<AstIds, Fail> {
    let n = code.len();
    // The compiled functions by entry (the owners of the code).
    let mut entries: Vec<(u32, u32)> = t
        .funcs
        .iter()
        .filter(|f| f.entry != UNCOMPILED && (f.entry as usize) < n)
        .map(|f| (f.entry, f.nregs))
        .collect();
    entries.sort_unstable();
    // One owner per entry: functions sharing an entry share the code; the
    // largest register count is the bound the VM grows to for any of them.
    entries.dedup_by(|b, a| {
        let same = a.0 == b.0;
        if same {
            a.1 = a.1.max(b.1);
        }
        same
    });
    // Pass 1: the instruction boundaries and each one's owner.
    let mut starts = vec![false; n + 1];
    let mut pc = 0;
    while pc < n {
        let w = code[pc];
        let op =
            Op::try_from_u8(w as u8).ok_or(Fail { why: "invalid opcode", at: Some((pc, None)) })?;
        starts[pc] = true;
        pc += 1 + op.operands();
    }
    if pc != n {
        return Err(Fail { why: "instruction runs past the end of the code", at: None });
    }
    starts[n] = true;
    if entries.iter().any(|&(e, _)| !starts[e as usize]) {
        return Err(Fail { why: "function entry inside an instruction", at: None });
    }
    let mut sites = sites;
    if sites.any(|pc| !starts.get(pc as usize).copied().unwrap_or(false)) {
        return Err(Fail { why: "call site inside an instruction", at: None });
    }
    if let Some(u) = unwind
        && !(starts.get(u as usize).copied().unwrap_or(false)
            && code.get(u as usize).is_some_and(|&w| w == Op::Unwind as u32))
    {
        return Err(Fail { why: "unwind entry not an Unwind instruction", at: None });
    }
    let owner_at = |pc: usize| entries.partition_point(|&(e, _)| e as usize <= pc);
    // Pass 2: the operands.
    let mut ids = AstIds::default();
    let mut pc = 0;
    let mut owner = 0;
    while pc < n {
        while owner < entries.len() && entries[owner].0 as usize <= pc {
            owner += 1;
        }
        // `owner` is one past the owning entry (0: no owner, no registers).
        let nregs = owner.checked_sub(1).map_or(0, |i| entries[i].1);
        let w = code[pc];
        let op = Op::from_u8(w as u8);
        let aux = w >> 8;
        let ops = &code[pc + 1..pc + 1 + op.operands()];
        for (&k, &v) in kinds(op, aux).iter().zip(ops) {
            let i = v as usize;
            let ok = match k {
                I => true,
                R => v <= nregs,
                Ro => v <= nregs || v == u32::MAX,
                J => i < n && starts[i] && owner_at(i) == owner,
                F => i < t.funcs.len(),
                Fo => i < t.funcs.len() || v == super::NO_DTOR,
                Fc => i < t.funcs.len() || v == exc::EXC_FREE || v == exc::LIFE_CHECK,
                G => i < t.nglobals,
                T => i < t.ntypes,
                To => i < t.ntypes || v == bi::DYN,
                Tr => i < t.ntraps,
                S => i.checked_add(aux as usize).is_some_and(|e| e <= t.nsteps),
                O => i < t.heap_origins.len(),
                Oh => t.heap_origins.get(i).copied().unwrap_or(false),
                D => {
                    ids.decl = ids.decl.max(v);
                    v != 0
                }
                E => {
                    ids.expr = ids.expr.max(v);
                    true
                }
                Q => {
                    ids.ty = ids.ty.max(v >> 3);
                    v >> 3 != 0
                }
            };
            if !ok {
                let why = match k {
                    R | Ro => "instruction register out of its function's frame",
                    J => "jump target not an instruction of the same function",
                    F | Fo | Fc => "instruction names an unknown function",
                    G => "instruction names an unknown global",
                    T | To => "instruction names an unknown type",
                    Tr => "instruction names an unknown trap",
                    S => "step locations out of range",
                    O | Oh => "instruction names an unknown origin",
                    D | Q => "instruction holds a zero AST id",
                    I | E => "invariant: never rejected",
                };
                return Err(Fail { why, at: Some((pc, Some(op))) });
            }
        }
        pc += 1 + op.operands();
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::{Tables, verify};
    use crate::compile::{FState, Func, RetKind};
    use crate::kbc::Op;

    fn func(entry: u32, nregs: u32) -> Func {
        Func {
            decl: None,
            entry,
            state: FState::Ready,
            nregs,
            ret: RetKind::Void,
            params: Vec::new(),
            memo: false,
            this_reg: None,
            this_ty: None,
            base_obj: false,
            structor: false,
        }
    }

    fn w(op: Op) -> u32 {
        op as u32
    }

    /// Two functions: 0 at pc 0 (2 registers), 1 at pc 6 (1 register).
    fn code() -> Vec<u32> {
        vec![w(Op::Mov), 2, 1, w(Op::Jmp), 5, w(Op::Halt), w(Op::Ret), 1, w(Op::Nop)]
    }

    fn check(code: &[u32], unwind: Option<u32>) -> Result<(), &'static str> {
        let funcs = [func(0, 2), func(6, 1)];
        let heap = [false, true];
        let t = Tables {
            funcs: &funcs,
            nglobals: 1,
            ntypes: 3,
            ntraps: 1,
            nsteps: 4,
            heap_origins: &heap,
        };
        verify(code, &t, [0, 6, 9].into_iter(), unwind).map(|_| ()).map_err(|f| f.why)
    }

    /// Each invariant of the module docs, broken once (spec/snap.md
    /// "Threat model").
    #[test]
    fn verifier_refuses_each_broken_invariant() {
        let ok = code();
        assert_eq!(check(&ok, None), Ok(()));
        let with = |at: usize, v: u32| {
            let mut c = ok.clone();
            c[at] = v;
            c
        };
        assert_eq!(check(&with(0, 0xff), None), Err("invalid opcode"));
        assert_eq!(
            check(&ok[..ok.len() - 2], None),
            Err("instruction runs past the end of the code")
        );
        assert_eq!(
            check(&with(1, 3), None),
            Err("instruction register out of its function's frame")
        );
        // Function 1 has one register: register 2 is past its frame.
        assert_eq!(
            check(&with(7, 2), None),
            Err("instruction register out of its function's frame")
        );
        // A jump into an instruction's operands, and into another function.
        assert_eq!(
            check(&with(4, 2), None),
            Err("jump target not an instruction of the same function")
        );
        assert_eq!(
            check(&with(4, 6), None),
            Err("jump target not an instruction of the same function")
        );
        let mut call = ok.clone();
        call.splice(5..6, [w(Op::Call), 2, 0]);
        let funcs = [func(0, 2), func(8, 1)];
        let t = Tables {
            funcs: &funcs,
            nglobals: 1,
            ntypes: 3,
            ntraps: 1,
            nsteps: 4,
            heap_origins: &[],
        };
        assert_eq!(
            verify(&call, &t, std::iter::empty(), None).map(|_| ()).map_err(|f| f.why),
            Err("instruction names an unknown function")
        );
        let tail = |extra: &[u32]| {
            let mut c = ok.clone();
            c.extend_from_slice(extra);
            check(&c, None)
        };
        assert_eq!(tail(&[w(Op::GlobalAddr), 0, 1]), Err("instruction names an unknown global"));
        assert_eq!(tail(&[w(Op::ZeroInit), 0, 3]), Err("instruction names an unknown type"));
        assert_eq!(tail(&[w(Op::Trap), 1]), Err("instruction names an unknown trap"));
        assert_eq!(tail(&[w(Op::Step) | 3 << 8, 2]), Err("step locations out of range"));
        assert_eq!(tail(&[w(Op::Step) | 2 << 8, 2]), Ok(()));
        assert_eq!(tail(&[w(Op::Alloc), 0, 0, 2]), Err("instruction names an unknown origin"));
        assert_eq!(tail(&[w(Op::New), 0, 0, 0, 0, 0]), Err("instruction names an unknown origin"));
        assert_eq!(tail(&[w(Op::New), 0, 0, 0, 1, 0]), Ok(()));
        assert_eq!(tail(&[w(Op::MpCast), 0, 0]), Err("instruction holds a zero AST id"));
        assert_eq!(tail(&[w(Op::TypeId), 0, 5, 0, 0]), Err("instruction holds a zero AST id"));
        assert_eq!(tail(&[w(Op::VCall), 7, 0, 0, u32::MAX]), Ok(()));
        let life = crate::vm::exc::LIFE_CHECK;
        assert_eq!(tail(&[w(Op::PushCleanup), 0, life, 0]), Ok(()));
        assert_eq!(
            tail(&[w(Op::PushCleanup), 0, crate::vm::exc::TRY_MARK, 0]),
            Err("instruction names an unknown function")
        );
        assert_eq!(tail(&[w(Op::Throw), 0, 0, crate::kbc::NO_DTOR]), Ok(()));
        assert_eq!(check(&ok, Some(5)), Err("unwind entry not an Unwind instruction"));
        assert_eq!(check(&with(5, w(Op::Unwind)), Some(5)), Ok(()));
        let funcs = [func(0, 2), func(6, 1)];
        let t = Tables {
            funcs: &funcs,
            nglobals: 1,
            ntypes: 3,
            ntraps: 1,
            nsteps: 4,
            heap_origins: &[],
        };
        assert_eq!(
            verify(&ok, &t, [2].into_iter(), None).map(|_| ()).map_err(|f| f.why),
            Err("call site inside an instruction")
        );
        let funcs = [func(0, 2), func(4, 1)];
        let t = Tables {
            funcs: &funcs,
            nglobals: 1,
            ntypes: 3,
            ntraps: 1,
            nsteps: 4,
            heap_origins: &[],
        };
        assert_eq!(
            verify(&ok, &t, std::iter::empty(), None).map(|_| ()).map_err(|f| f.why),
            Err("function entry inside an instruction")
        );
    }
}
