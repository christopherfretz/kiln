//! Structural verification of a finished function (`spec/llvm.md` §4.2).
//!
//! [`crate::IrBuilder`] checks operand types as instructions are appended;
//! this pass checks what can only be judged once a function is complete:
//! every placed block ends in exactly one terminator, phis lead their block
//! and list exactly one incoming value per predecessor edge, the entry block
//! has no predecessors, branch targets are placed, and landing pads are
//! first in their block in a function with a personality.

use crate::error::{IrError, IrErrorKind, IrResult};
use crate::function::Function;
use crate::inst::InstKind;
use crate::value::{BlockId, Value};

/// Verify the structure of a function definition. Declarations always
/// pass.
///
/// # Errors
///
/// Returns the first violation found, in block layout order.
#[allow(clippy::too_many_lines)]
pub fn verify_function(f: &Function) -> IrResult<()> {
    if f.layout.is_empty() {
        return Ok(());
    }
    let err = |kind, msg: String| Err(IrError::new(kind, format!("@{}: {msg}", f.name)));
    // Predecessor edge counts per block (an edge per successor slot).
    let mut preds: Vec<Vec<BlockId>> = vec![Vec::new(); f.blocks.len()];
    for &bb in &f.layout {
        let insts = &f.blocks[bb.index()].insts;
        let Some(&last) = insts.last() else {
            return err(IrErrorKind::MissingTerminator, format!("block {} is empty", bb.index()));
        };
        if !f.insts[last.index()].is_terminator() {
            return err(
                IrErrorKind::MissingTerminator,
                format!("block {} does not end in a terminator", bb.index()),
            );
        }
        for &s in &f.successors(last) {
            if !f.blocks[s.index()].placed {
                return err(
                    IrErrorKind::DetachedBlock,
                    format!("block {} branches to unplaced block {}", bb.index(), s.index()),
                );
            }
            preds[s.index()].push(bb);
        }
    }
    let entry = f.layout[0];
    if !preds[entry.index()].is_empty() {
        return err(IrErrorKind::BadFunction, "entry block has predecessors".into());
    }
    for &bb in &f.layout {
        let insts = &f.blocks[bb.index()].insts;
        let mut leading = true;
        let mut first_non_phi = true;
        for (pos, &i) in insts.iter().enumerate() {
            let inst = &f.insts[i.index()];
            if pos + 1 != insts.len() && inst.is_terminator() {
                return err(
                    IrErrorKind::TerminatorNotLast,
                    format!("terminator in the middle of block {}", bb.index()),
                );
            }
            match inst.kind {
                InstKind::Phi { incoming, .. } => {
                    if !leading {
                        return err(
                            IrErrorKind::PhiNotFirst,
                            format!("phi after a non-phi in block {}", bb.index()),
                        );
                    }
                    let mut want = preds[bb.index()].clone();
                    let mut got: Vec<BlockId> =
                        f.ops(incoming).chunks(2).map(|p| p[1].as_block()).collect();
                    want.sort_unstable();
                    got.sort_unstable();
                    // LLVM allows one entry per edge; duplicate edges from a
                    // block (e.g. a switch) must list that block per edge
                    // with the same value, or once per unique predecessor.
                    let mut uniq = want.clone();
                    uniq.dedup();
                    if got != want && got != uniq {
                        return err(
                            IrErrorKind::PhiArity,
                            format!(
                                "phi in block {} has {} incoming blocks, block has {} predecessors",
                                bb.index(),
                                got.len(),
                                uniq.len()
                            ),
                        );
                    }
                }
                InstKind::LandingPad(_) => {
                    leading = false;
                    if !first_non_phi {
                        return err(
                            IrErrorKind::BadFunction,
                            format!("landingpad is not first in block {}", bb.index()),
                        );
                    }
                    if f.personality.is_none() {
                        return err(
                            IrErrorKind::BadFunction,
                            "landingpad in a function without a personality".into(),
                        );
                    }
                    first_non_phi = false;
                }
                _ => {
                    leading = false;
                    first_non_phi = false;
                }
            }
        }
    }
    // Every local operand refers to an instruction in a placed block.
    for &v in &f.ops {
        if let Some(b) = block_of(f, v)
            && !f.blocks[b.index()].placed
        {
            return err(
                IrErrorKind::DetachedBlock,
                format!("operand refers to block {} which is not placed", b.index()),
            );
        }
    }
    Ok(())
}

fn block_of(f: &Function, v: Value) -> Option<BlockId> {
    match v.get() {
        crate::value::ValueRef::Inst(i) => f.inst_block.get(i.index()).copied(),
        crate::value::ValueRef::Block(b) => Some(b),
        _ => None,
    }
}
