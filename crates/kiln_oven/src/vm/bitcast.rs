//! `__builtin_bit_cast` ([bit.cast]; clang 19's `APValueToBufferConverter`
//! and `BufferToAPValueConverter`): the value representation of the source
//! is read into a byte buffer, scalar by scalar (padding, `nullptr_t` and
//! uninitialized scalars stay indeterminate), and the destination is built
//! from it scalar by scalar through the `kiln_abi` layout the Oven's types
//! carry. An indeterminate byte may only initialize `unsigned char`,
//! unsigned plain `char` or `std::byte`; a `bool` byte must be 0 or 1.
//!
//! Types the compiler rejects never reach here: unions, pointers, member
//! pointers, volatile and reference members (spec/oven.md "Builtins").
//! Base class subobjects are walked as members; a bit-field is copied bit
//! by bit (WP-1526), so definedness is tracked per bit: the bits of a
//! byte outside every bit-field are padding and stay indeterminate.
//!
//! When both types are dense (every byte belongs to an integer or
//! floating-point scalar with no padding) and the source is fully
//! initialized, the bytes are copied at once.

use kiln_ast::Ast;
use kiln_ast::types::BuiltinType;

use super::Fault;
use crate::Oven;
use crate::arith::canon;
use crate::kbc::ak;
use crate::mem::{MemFault, Sp, flags};
use crate::ty::{FieldInfo, OTy, TyKind};
use crate::vm::{BiFault, value_len};

/// The source bytes and which of their bits are determinate (a mask per
/// byte).
struct Buf {
    bytes: Vec<u8>,
    def: Vec<u8>,
}

impl Buf {
    fn new(n: u64) -> Buf {
        Buf { bytes: vec![0; n as usize], def: vec![0; n as usize] }
    }

    fn all_def(&self, off: u64, len: u64) -> bool {
        self.def[off as usize..(off + len) as usize].iter().all(|&d| d == 0xff)
    }

    /// Bits `bit..bit + width` from byte `off` (little-endian bit order),
    /// or `None` when one of them is indeterminate.
    fn bits(&self, off: u64, bit: u32, width: u32) -> Option<u128> {
        let mut v = 0u128;
        for i in (0..width).rev() {
            let (b, m) = ((off + u64::from((bit + i) / 8)) as usize, 1u8 << ((bit + i) % 8));
            if self.def[b] & m == 0 {
                return None;
            }
            v = v << 1 | u128::from(self.bytes[b] & m != 0);
        }
        Some(v)
    }

    fn get(&self, off: u64, len: u64) -> &[u8] {
        &self.bytes[off as usize..(off + len) as usize]
    }
}

fn le(bytes: &[u8]) -> u128 {
    bytes.iter().rev().fold(0u128, |v, &b| v << 8 | u128::from(b))
}

impl Oven {
    /// Whether every byte of `t` belongs to a scalar whose stored bytes are
    /// its value representation (`bool` too when `bools`): no padding, no
    /// bit-fields, no `long double` with unused bytes, no pointers.
    fn dense(&self, t: OTy, bools: bool) -> bool {
        let info = self.types.info(t);
        match info.kind {
            TyKind::Int { .. } => true,
            TyKind::Bool => bools,
            TyKind::Float(_) => value_len(info.size, info.kind) == info.size,
            TyKind::Array { elem, .. } => self.dense(elem, bools),
            // Every bit is a lane (`ty.rs`): dense when the lanes fill
            // the bytes.
            TyKind::BoolVec { n, .. } => n == info.size * 8,
            TyKind::Record { union: false, .. } => {
                let mut end = 0;
                for f in self.types.fields(t) {
                    if f.width.is_some() || f.offset != end || !self.dense(f.ty, bools) {
                        return false;
                    }
                    end += self.types.size(f.ty);
                }
                end == info.size
            }
            _ => false,
        }
    }

    /// Bit-casts the object at `src` (of type `ts`) to `td`: into the
    /// object at `dst` being initialized when `to_mem`, else as the
    /// returned scalar value.
    pub(super) fn bit_cast(
        &mut self,
        ast: &Ast,
        src: u64,
        dst: u64,
        ts: OTy,
        td: OTy,
        to_mem: bool,
        read_only: bool,
    ) -> Result<u128, Fault> {
        let n = self.types.size(ts);
        let (ss, sa, has_union) = {
            let (ss, sa, a) = self.rmw_check(src, n, ts, ak::READ)?;
            (ss, sa, a.flags & flags::HAS_UNION != 0)
        };
        if has_union {
            self.union_path(src, ts, ak::READ)?;
        }
        if read_only {
            return Ok(0);
        }
        let nd = self.types.size(td);
        let bool_vec = |t| matches!(self.types.kind(t), TyKind::BoolVec { .. });
        if nd < n && bool_vec(ts) && bool_vec(td) {
            return self.gnu_bool_vec_cast(ast, ss, sa, ts, dst, td);
        }
        if self.dense(ts, true) && self.dense(td, false) && self.mem.space(ss).is_init(sa, n) {
            if !to_mem {
                let v = self.mem.space(ss).read(sa, n);
                return Ok(match self.types.kind(td) {
                    TyKind::Int { width, signed } => canon(v, u32::from(width), signed),
                    _ => v,
                });
            }
            let (ds, da) = self.dest(dst, n, td)?;
            self.mem.copy(ds, da, ss, sa, n);
            self.mem.space_mut(ds).set_ptr(da, n, false);
            return Ok(0);
        }
        let mut buf = Buf::new(n);
        self.read_obj(ast, ss, sa, ts, 0, &mut buf);
        if !to_mem {
            // An indeterminate `unsigned char` result is read as a value.
            return self.cast_scalar(ast, &buf, td, 0)?.ok_or(Fault::Mem {
                f: MemFault::Uninit,
                ak: ak::READ,
                ty: td,
                p: src,
            });
        }
        let (ds, da) = self.dest(dst, n, td)?;
        let sp = self.mem.space_mut(ds);
        sp.set_init(da, n, false);
        sp.set_ptr(da, n, false);
        self.write_obj(ast, ds, da, &buf, td, 0)
    }

    /// clang 19's `CK_BitCast` of a widening `__builtin_shufflevector`
    /// of `bool` ext vectors (the only cast whose destination is smaller
    /// than its source): the result is a GNU vector of `bool`, which
    /// clang 19's `BitCastBuffer` writes a byte per lane, and the packed
    /// ext vector `td` is read from the first of those bytes, so lane `k`
    /// of `td` is lane `k / 8` of the source when `k % 8 == 0`, else
    /// false (spec/oven.md "Bool vectors"). kiln's `ts` is packed.
    fn gnu_bool_vec_cast(
        &mut self,
        ast: &Ast,
        ss: Sp,
        sa: u64,
        ts: OTy,
        dst: u64,
        td: OTy,
    ) -> Result<u128, Fault> {
        let nd = self.types.size(td);
        let n = self.types.size(ts);
        let mut src = Buf::new(n);
        self.read_obj(ast, ss, sa, ts, 0, &mut src);
        let mut buf = Buf::new(nd);
        for j in 0..nd as usize {
            buf.bytes[j] = (src.bytes[j / 8] >> (j % 8)) & 1;
            buf.def[j] = if src.def[j / 8] == 0xff { 0xff } else { 0 };
        }
        let (ds, da) = self.dest(dst, nd, td)?;
        let sp = self.mem.space_mut(ds);
        sp.set_init(da, nd, false);
        sp.set_ptr(da, nd, false);
        self.write_obj(ast, ds, da, &buf, td, 0)
    }

    /// The destination object's space and absolute address.
    fn dest(&mut self, dst: u64, n: u64, td: OTy) -> Result<(Sp, u64), Fault> {
        let fault = |f| Fault::Mem { f, ak: ak::INIT, ty: td, p: dst };
        let (ds, da, _) = self.mem.check(dst, n, true).map_err(fault)?;
        self.mem.trial_write(dst, n).map_err(fault)?;
        Ok((ds, da))
    }

    /// Copies the value representation of the object of type `t` at
    /// `abs + off` into `buf` at `off`.
    fn read_obj(&self, ast: &Ast, sp: Sp, abs: u64, t: OTy, off: u64, buf: &mut Buf) {
        let info = *self.types.info(t);
        match info.kind {
            TyKind::Int { .. } | TyKind::Bool | TyKind::Float(_) => {
                let len = value_len(info.size, info.kind);
                let space = self.mem.space(sp);
                if space.is_init(abs + off, len) {
                    let (a, b) = ((abs + off) as usize, (off + len) as usize);
                    buf.bytes[off as usize..b].copy_from_slice(&space.bytes[a..a + len as usize]);
                    buf.def[off as usize..b].fill(0xff);
                }
            }
            TyKind::Array { elem, n } => {
                let es = self.types.size(elem);
                let space = self.mem.space(sp);
                if self.dense(elem, true) && space.is_init(abs + off, es * n) {
                    let (a, len) = ((abs + off) as usize, (es * n) as usize);
                    let o = off as usize;
                    buf.bytes[o..o + len].copy_from_slice(&space.bytes[a..a + len]);
                    buf.def[o..o + len].fill(0xff);
                    return;
                }
                for i in 0..n {
                    self.read_obj(ast, sp, abs, elem, off + i * es, buf);
                }
            }
            TyKind::BoolVec { n, .. } => {
                let len = n.div_ceil(8);
                let space = self.mem.space(sp);
                if space.is_init(abs + off, len) {
                    let (a, o) = ((abs + off) as usize, off as usize);
                    let len = len as usize;
                    buf.bytes[o..o + len].copy_from_slice(&space.bytes[a..a + len]);
                    buf.def[o..o + len].fill(0xff);
                }
            }
            TyKind::Record { .. } => {
                for f in self.types.fields(t) {
                    match f.width {
                        // An unnamed bit-field is padding ([class.bit]/2).
                        Some(_) if ast.decls.get(f.decl).name().is_empty() => {}
                        Some(w) => self.read_bits(sp, abs + off + f.offset, *f, w, off, buf),
                        None => self.read_obj(ast, sp, abs, f.ty, off + f.offset, buf),
                    }
                }
            }
            // `nullptr_t` has an indeterminate value representation.
            _ => {}
        }
    }

    /// The value of the scalar of type `t` at `off` of `buf`; `None` when
    /// it is indeterminate and `t` may hold that.
    fn cast_scalar(&self, ast: &Ast, buf: &Buf, t: OTy, off: u64) -> Result<Option<u128>, Fault> {
        let info = *self.types.info(t);
        if info.kind == TyKind::NullPtr {
            return Ok(Some(0));
        }
        let len = value_len(info.size, info.kind);
        if !buf.all_def(off, len) {
            if self.holds_indeterminate(ast, t) {
                return Ok(None);
            }
            return Err(Fault::Bi(BiFault::CastIndet { t }));
        }
        let v = le(buf.get(off, len));
        Ok(Some(match info.kind {
            TyKind::Bool if v > 1 => {
                return Err(Fault::Bi(BiFault::CastUnrep { v: v as u8 }));
            }
            TyKind::Int { width, signed } => canon(v, u32::from(width), signed),
            _ => v,
        }))
    }

    /// clang: `unsigned char`, unsigned plain `char` or `std::byte` may be
    /// initialized by an indeterminate value ([basic.indet]/2).
    fn holds_indeterminate(&self, ast: &Ast, t: OTy) -> bool {
        let q = self.types.info(t).q;
        if let Some(e) = ast.types.as_enum(q) {
            return crate::compile::is_std_named(ast, e, "byte");
        }
        match ast.types.builtin(q) {
            Some(BuiltinType::UChar) => true,
            Some(BuiltinType::Char) => !ast.types.target().char_signed,
            _ => false,
        }
    }

    /// Builds the object of type `t` at `abs + off` from `buf` at `off`.
    fn write_obj(
        &mut self,
        ast: &Ast,
        sp: Sp,
        abs: u64,
        buf: &Buf,
        t: OTy,
        off: u64,
    ) -> Result<u128, Fault> {
        let info = *self.types.info(t);
        match info.kind {
            TyKind::Int { .. } | TyKind::Bool | TyKind::Float(_) | TyKind::NullPtr => {
                if let Some(v) = self.cast_scalar(ast, buf, t, off)? {
                    let len = value_len(info.size, info.kind);
                    let space = self.mem.space_mut(sp);
                    space.write(abs + off, len, v);
                    space.set_init(abs + off, len, true);
                    space.set_ptr(abs + off, len, info.kind == TyKind::NullPtr);
                }
            }
            TyKind::Array { elem, n } => {
                let es = self.types.size(elem);
                if self.dense(elem, false) && buf.all_def(off, es * n) {
                    let (a, len) = ((abs + off) as usize, es * n);
                    let space = self.mem.space_mut(sp);
                    space.bytes[a..a + len as usize].copy_from_slice(buf.get(off, len));
                    space.set_init(abs + off, len, true);
                    return Ok(0);
                }
                for i in 0..n {
                    self.write_obj(ast, sp, abs, buf, elem, off + i * es)?;
                }
            }
            TyKind::BoolVec { n, .. } => {
                // Any byte is a valid set of lanes; an indeterminate one
                // is not a value of `bool`.
                let len = n.div_ceil(8);
                if !buf.all_def(off, len) {
                    return Err(Fault::Bi(BiFault::CastIndet { t }));
                }
                let a = (abs + off) as usize;
                let space = self.mem.space_mut(sp);
                space.bytes[a..a + len as usize].copy_from_slice(buf.get(off, len));
                space.set_init(abs + off, len, true);
            }
            TyKind::Record { .. } => {
                let (first, count) = match info.kind {
                    TyKind::Record { first, count, .. } => (first, count),
                    _ => (0, 0),
                };
                for i in first..first + count {
                    let f = self.types.fields[i as usize];
                    match f.width {
                        Some(_) if ast.decls.get(f.decl).name().is_empty() => {}
                        Some(w) => self.write_bits(ast, sp, abs, buf, &f, w, off)?,
                        None => _ = self.write_obj(ast, sp, abs, buf, f.ty, off + f.offset)?,
                    }
                }
            }
            _ => {}
        }
        Ok(0)
    }

    /// The value bits of a bit-field of width `w` (the rest of a wider
    /// bit-field is padding, [class.bit]/1).
    fn bit_width(&self, f: &FieldInfo, w: u32) -> u32 {
        let ty = self.types.info(f.ty);
        w.min((value_len(ty.size, ty.kind) * 8) as u32)
    }

    /// Copies the bit-field `f` of width `w` whose first byte is at `at`
    /// (and at `base + f.offset` of `buf`) into `buf`, marking its bits
    /// determinate when its bytes are initialized.
    fn read_bits(&self, sp: Sp, at: u64, f: FieldInfo, w: u32, base: u64, buf: &mut Buf) {
        let n = u64::from((f.bit + w).div_ceil(8));
        let space = self.mem.space(sp);
        if w == 0 || !space.is_init(at, n) {
            return;
        }
        let o = base + f.offset;
        for i in f.bit..f.bit + self.bit_width(&f, w) {
            let (b, m) = (u64::from(i / 8), 1u8 << (i % 8));
            let byte = &mut buf.bytes[(o + b) as usize];
            *byte = (*byte & !m) | (space.bytes[(at + b) as usize] & m);
            buf.def[(o + b) as usize] |= m;
        }
    }

    /// Initializes the bit-field `f` of width `w` of the record at
    /// `abs + base` from `buf`. An indeterminate bit leaves an `unsigned
    /// char` or `std::byte` bit-field indeterminate ([bit.cast]/2: it is
    /// the smallest object containing the bit) and is undefined otherwise.
    #[allow(clippy::too_many_arguments)]
    fn write_bits(
        &mut self,
        ast: &Ast,
        sp: Sp,
        abs: u64,
        buf: &Buf,
        f: &FieldInfo,
        w: u32,
        base: u64,
    ) -> Result<(), Fault> {
        let vw = self.bit_width(f, w);
        if vw == 0 {
            return Ok(());
        }
        let o = base + f.offset;
        let Some(v) = buf.bits(o, f.bit, vw) else {
            if self.holds_indeterminate(ast, f.ty) {
                return Ok(());
            }
            return Err(Fault::Bi(BiFault::CastIndet { t: f.ty }));
        };
        if self.types.kind(f.ty) == TyKind::Bool && v > 1 {
            return Err(Fault::Bi(BiFault::CastUnrep { v: v as u8 }));
        }
        let n = u64::from((f.bit + w).div_ceil(8));
        let at = abs + o;
        let space = self.mem.space_mut(sp);
        // Bytes not yet initialized hold zero bits (as `Vm::store_bits`).
        let mut old = 0u128;
        for i in (0..n).rev() {
            let byte = if space.is_init(at + i, 1) { space.bytes[(at + i) as usize] } else { 0 };
            old = old << 8 | u128::from(byte);
        }
        let mask = if vw >= 128 { u128::MAX } else { (1u128 << vw) - 1 } << f.bit;
        space.write(at, n, (old & !mask) | ((v << f.bit) & mask));
        space.set_init(at, n, true);
        Ok(())
    }
}
