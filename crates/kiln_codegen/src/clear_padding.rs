//! `__builtin_clear_padding(p)` (GCC; spec/codegen.md "Clearing
//! padding", WP-774): zeroes the padding bits of `*p`, the bits
//! `kiln_abi::layout::padding::padding_mask` marks, and leaves every value
//! bit alone.
//!
//! A run of whole padding bytes is one `llvm.memset` to 0 (a lone byte a
//! `store i8 0`); a byte holding both value and padding bits (a bit-field's
//! unit) is a load, an `and` with the value bits and a store. An array
//! whose elements have padding and which spans more than
//! [`UNROLL_BYTES`] bytes is cleared by a loop over its (flattened)
//! elements; a volatile pointee makes every access volatile.

use kiln_ast::ExprId;
use kiln_ast::types::TypeView;
use kiln_llvm::{BinOp, Flags, IntPredicate, TypeId as LTy};

use crate::fir::{Fir, Op, V};
use crate::func::{Addr, min_align};
use crate::module::{Cg, R, err};

/// The largest object cleared by straight-line code; a larger array of
/// elements with padding is cleared by a loop.
const UNROLL_BYTES: u64 = 64;

impl Cg<'_> {
    /// `__builtin_clear_padding(p)`: `p` is evaluated once.
    pub(crate) fn emit_clear_padding(&mut self, p: ExprId) -> R<()> {
        let ast = self.ast;
        let pq = ast.exprs.get(p).ty();
        let (v, align) = self.ptr_align(p)?;
        let Some(t) = ast.types.pointee(pq) else {
            return err(self.loc_of(p), "__builtin_clear_padding of a non-pointer");
        };
        let volatile = ast.types.effective_quals(t).is_volatile();
        // The innermost element type of nested constant arrays and their
        // total count.
        let (mut elem, mut n) = (t, 1u64);
        while let TypeView::ConstantArray(a) = ast.types.canonical_view(elem) {
            n = n.saturating_mul(a.size);
            elem = a.elem;
        }
        let esz = self.size_align(elem).0;
        if n == 0 || esz == 0 {
            return Ok(());
        }
        let Some(mask) = kiln_abi::padding_mask(self.tys.layouter(), elem) else {
            return err(self.loc_of(p), "__builtin_clear_padding of an object over 16 MiB");
        };
        if mask.iter().all(|&b| b == 0) {
            return Ok(());
        }
        if n == 1 || n.saturating_mul(esz) <= UNROLL_BYTES {
            for i in 0..n {
                self.clear_masked(v, align, i * esz, &mask, volatile);
            }
            return Ok(());
        }
        // for (off = 0; off != n * esz; off += esz) clear(p + off)
        let zero = self.i64c(0);
        let entry = self.f.fir.cur().expect("insertion point");
        let body = self.f.fir.create_block();
        self.emit_block(body, false);
        let phi = self.f.fir.push(Op::Phi { ty: LTy::I64, inc: vec![(zero, entry)] }, LTy::I64);
        let el = self.gep(LTy::I8, v, vec![phi], true);
        self.clear_masked(el, min_align(align, esz), 0, &mask, volatile);
        let step = self.i64c(i128::from(esz));
        let next = self.bin(BinOp::Add, Flags::NUW, phi, step);
        let cur = self.f.fir.cur().expect("insertion point");
        self.f.fir.add_incoming(Fir::last_inst(phi), next, cur);
        let total = self.i64c(i128::from(n.saturating_mul(esz)));
        let done = self.icmp(IntPredicate::Eq, next, total);
        let end = self.f.fir.create_block();
        self.cond_br(done, end, body);
        self.emit_block(end, false);
        Ok(())
    }

    /// Clears the padding bits `mask` marks in the bytes at `base + at`
    /// (`base` aligned to `align`).
    fn clear_masked(&mut self, base: V, align: u64, at: u64, mask: &[u8], volatile: bool) {
        let mut i = 0usize;
        while i < mask.len() {
            let m = mask[i];
            let off = at + i as u64;
            if m == 0 {
                i += 1;
                continue;
            }
            let ptr = if off == 0 {
                base
            } else {
                let o = self.i64c(i128::from(off));
                self.gep(LTy::I8, base, vec![o], true)
            };
            let a = min_align(align, off);
            if m == 0xff {
                let len = mask[i..].iter().take_while(|&&b| b == 0xff).count();
                if len == 1 {
                    let z = self.m.const_int(LTy::I8, 0);
                    self.store(V::C(z), ptr, a, volatile);
                } else {
                    self.memset(Addr { v: ptr, ty: LTy::I8, align: a }, 0, len as u64, volatile);
                }
                i += len;
                continue;
            }
            let old = self.load(LTy::I8, ptr, a, volatile);
            let keep = V::C(self.m.const_int(LTy::I8, i128::from(!m)));
            let new = self.bin(BinOp::And, Flags::NONE, old, keep);
            self.store(new, ptr, a, volatile);
            i += 1;
        }
    }
}
