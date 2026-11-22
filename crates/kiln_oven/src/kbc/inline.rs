//! Which compiled bodies the VM may inline at a call site (spec/oven.md
//! "Inlining", WP-1924): a short leaf body whose every instruction is
//! one the VM executes the same way in its caller's frame as in its own.

use super::Op;

/// The longest body inlined, in instructions (spec/oven.md "Inlining":
/// the measured bodies of one-return library functions are 3 to 14
/// instructions; a longer one runs long enough that its call is a small
/// share of it).
pub(crate) const MAX_INSNS: usize = 24;

/// Why a body is not inlined (also the Step 0 counters of WP-1924).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refuse {
    /// An instruction the inlined copy would not run the same way: a
    /// call (the body is no leaf), anything that reads or changes the
    /// call stack, registers a cleanup, allocates, throws, or reads the
    /// call's own context.
    Op(Op),
    /// More than [`MAX_INSNS`] instructions.
    Size(usize),
}

/// Whether `op` (with `aux`) runs the same in an inlined copy, in the
/// caller's frame with its registers renumbered, as in the callee's own
/// frame: it touches only registers, memory, the step counter and the
/// evaluation's context, and a fault in it renders the call stack the
/// fault handler rebuilds (`Oven::inline_frame`).
fn inlinable_op(op: Op, aux: u32) -> bool {
    match op {
        Op::Nop
        | Op::Step
        | Op::Const
        | Op::Const2
        | Op::Mov
        | Op::Jmp
        | Op::JmpIf
        | Op::JmpIfNot
        | Op::IAdd
        | Op::ISub
        | Op::IMul
        | Op::IDiv
        | Op::IRem
        | Op::IShl
        | Op::IShr
        | Op::IAnd
        | Op::IOr
        | Op::IXor
        | Op::INeg
        | Op::INot
        | Op::LNot
        | Op::ICmp
        | Op::ICast
        | Op::ToBool
        | Op::FAdd
        | Op::FSub
        | Op::FMul
        | Op::FDiv
        | Op::FNeg
        | Op::FCmp
        | Op::FCast
        | Op::IToF
        | Op::FToI
        | Op::FToBool
        | Op::PtrAdd
        | Op::PtrSub
        | Op::PtrCmp
        | Op::PtrToBool
        | Op::Field
        | Op::Load
        | Op::Store
        | Op::InitStore
        | Op::LoadBits
        | Op::StoreBits
        | Op::ActiveCheck
        | Op::Trap
        | Op::Ret => true,
        // `if consteval` reads the run's context, which a call does not
        // change; the other forms read or change the plain bit or name
        // the enclosing call (`std::is_constant_evaluated`'s warning).
        Op::IsConstEval => aux == 16,
        _ => false,
    }
}

/// The number of instructions of the body `code[from..to]` when it can be
/// inlined (spec/oven.md "Inlining").
///
/// # Errors
/// The first reason it cannot.
pub(crate) fn inlinable(code: &[u32], from: usize, to: usize) -> Result<usize, Refuse> {
    let mut pc = from;
    let mut n = 0;
    while pc < to.min(code.len()) {
        let w = code[pc];
        let op = Op::from_u8(w as u8);
        if !inlinable_op(op, w >> 8) {
            return Err(Refuse::Op(op));
        }
        n += 1;
        pc += 1 + op.operands();
    }
    if n > MAX_INSNS {
        return Err(Refuse::Size(n));
    }
    Ok(n)
}
