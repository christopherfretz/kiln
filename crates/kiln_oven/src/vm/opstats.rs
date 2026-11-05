//! The dynamic opcode histogram (feature `op-stats`, a measurement aid;
//! never built by default, so the release interpreter carries no counter).
//! Every executed instruction counts its opcode and the (previous, this)
//! pair; the totals print to stderr when the compiling thread exits:
//! `op-stats: insns=N`, then the top 20 opcodes and the top 10 adjacent
//! pairs (dynamically consecutive instructions, a call's or a jump's
//! target included).

use std::cell::RefCell;
use std::fmt::Write;

use crate::kbc::Op;

const N: usize = 256;

struct Stats {
    ops: Vec<u64>,
    pairs: Vec<u64>,
    prev: u8,
}

impl Drop for Stats {
    fn drop(&mut self) {
        let total: u64 = self.ops.iter().sum();
        if total == 0 {
            return;
        }
        let name = |b: usize| format!("{:?}", Op::from_u8(b as u8));
        let mut ops: Vec<(u64, usize)> =
            self.ops.iter().enumerate().filter(|e| *e.1 != 0).map(|(i, &c)| (c, i)).collect();
        ops.sort_unstable_by(|a, b| b.cmp(a));
        let mut pairs: Vec<(u64, usize)> =
            self.pairs.iter().enumerate().filter(|e| *e.1 != 0).map(|(i, &c)| (c, i)).collect();
        pairs.sort_unstable_by(|a, b| b.cmp(a));
        let pct = |c: u64| c as f64 * 100.0 / total as f64;
        let mut s = format!("op-stats: insns={total}\n");
        for &(c, i) in ops.iter().take(20) {
            let _ = writeln!(s, "op-stats: op {} {c} {:.2}%", name(i), pct(c));
        }
        for &(c, i) in pairs.iter().take(10) {
            let (a, b) = (name(i / N), name(i % N));
            let _ = writeln!(s, "op-stats: pair {a}+{b} {c} {:.2}%", pct(c));
        }
        eprint!("{s}");
    }
}

thread_local! {
    static STATS: RefCell<Stats> =
        RefCell::new(Stats { ops: vec![0; N], pairs: vec![0; N * N], prev: 0 });
}

/// Counts one executed instruction.
#[inline]
pub(crate) fn count(op: Op) {
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        let b = op as u8;
        s.ops[usize::from(b)] += 1;
        let i = usize::from(s.prev) * N + usize::from(b);
        s.pairs[i] += 1;
        s.prev = b;
    });
}
