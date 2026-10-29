//! `__builtin_bit_cast` ([bit.cast]; clang 19's `APValueToBufferConverter`
//! and `BufferToAPValueConverter`): the value representation of the source
//! is read into a byte buffer, scalar by scalar (padding, `nullptr_t` and
//! uninitialized scalars stay indeterminate), and the destination is built
//! from it scalar by scalar through the `kiln_abi` layout the Oven's types
//! carry. An indeterminate byte may only initialize `unsigned char`,
//! unsigned plain `char` or `std::byte`; a `bool` byte must be 0 or 1.
//!
//! Types the compiler rejects never reach here: unions, pointers, member
//! pointers, volatile and reference members, source bit-fields and
//! classes with bases (spec/oven.md "Builtins").
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
use crate::ty::{OTy, TyKind};
use crate::vm::{BiFault, value_len};

/// The source bytes and which of them are determinate.
struct Buf {
    bytes: Vec<u8>,
    def: Vec<bool>,
}

impl Buf {
    fn all_def(&self, off: u64, len: u64) -> bool {
        self.def[off as usize..(off + len) as usize].iter().all(|&d| d)
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
        let mut buf = Buf { bytes: vec![0; n as usize], def: vec![false; n as usize] };
        self.read_obj(ss, sa, ts, 0, &mut buf);
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

    /// The destination object's space and absolute address.
    fn dest(&self, dst: u64, n: u64, td: OTy) -> Result<(Sp, u64), Fault> {
        let fault = |f| Fault::Mem { f, ak: ak::INIT, ty: td, p: dst };
        let (ds, da, _) = self.mem.check(dst, n, true).map_err(fault)?;
        Ok((ds, da))
    }

    /// Copies the value representation of the object of type `t` at
    /// `abs + off` into `buf` at `off`.
    fn read_obj(&self, sp: Sp, abs: u64, t: OTy, off: u64, buf: &mut Buf) {
        let info = *self.types.info(t);
        match info.kind {
            TyKind::Int { .. } | TyKind::Bool | TyKind::Float(_) => {
                let len = value_len(info.size, info.kind);
                let space = self.mem.space(sp);
                if space.is_init(abs + off, len) {
                    let (a, b) = ((abs + off) as usize, (off + len) as usize);
                    buf.bytes[off as usize..b].copy_from_slice(&space.bytes[a..a + len as usize]);
                    buf.def[off as usize..b].fill(true);
                }
            }
            TyKind::Array { elem, n } => {
                let es = self.types.size(elem);
                let space = self.mem.space(sp);
                if self.dense(elem, true) && space.is_init(abs + off, es * n) {
                    let (a, len) = ((abs + off) as usize, (es * n) as usize);
                    let o = off as usize;
                    buf.bytes[o..o + len].copy_from_slice(&space.bytes[a..a + len]);
                    buf.def[o..o + len].fill(true);
                    return;
                }
                for i in 0..n {
                    self.read_obj(sp, abs, elem, off + i * es, buf);
                }
            }
            TyKind::Record { .. } => {
                for f in self.types.fields(t) {
                    self.read_obj(sp, abs, f.ty, off + f.offset, buf);
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
            TyKind::Record { .. } => {
                let (first, count) = match info.kind {
                    TyKind::Record { first, count, .. } => (first, count),
                    _ => (0, 0),
                };
                for i in first..first + count {
                    let f = self.types.fields[i as usize];
                    if f.width.is_some() {
                        return Err(Fault::Bi(BiFault::CastBitfield));
                    }
                    self.write_obj(ast, sp, abs, buf, f.ty, off + f.offset)?;
                }
            }
            _ => {}
        }
        Ok(0)
    }
}
