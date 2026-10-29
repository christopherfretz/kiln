//! Library builtins evaluated natively (`Op::Builtin`; spec/oven.md
//! "Builtins"): the bit operations, floating-point classification and the
//! string/memory functions, as clang 19's `VisitBuiltinCallExpr` evaluates
//! them ([library.c]: the C library's semantics, with clang's restrictions
//! to typed, element-wise access).
//!
//! The string/memory functions take a byte fast path when the elements are
//! one byte (or scalars, for `memcpy`) and the allocations hold no union;
//! any irregularity (an uninitialized byte, the end of the array) falls
//! back to the element-wise loop, which reports the exact fault.

use kiln_ast::Ast;
use kiln_ast::decl::DefinitionBits;
use kiln_ast::types::BuiltinType;

use super::{Fault, write_fault};
use crate::Oven;
use crate::compile::float_of;
use crate::kbc::{ak, bi};
use crate::mem::{MemFault, Ptr, flags};
use crate::softfloat::{self, Class};
use crate::ty::{OTy, TyKind};

/// A builtin-specific failure (rendered by `notes`; the types are the
/// designated objects', re-read from the call's arguments when static).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BiFault {
    /// `note_constexpr_memcpy_null`: the source (`src`) or destination
    /// of `memcpy`/`memmove` (`mv`) is null.
    CopyNull { mv: bool, src: bool },
    /// `note_constexpr_memcpy_type_pun`.
    CopyPun { mv: bool, src: OTy, dst: OTy },
    /// `note_constexpr_memcpy_nontrivial`.
    CopyNontrivial { mv: bool, t: OTy },
    /// `note_constexpr_memcpy_overlap` (only `memcpy`).
    CopyOverlap,
    /// `note_constexpr_memcpy_unsupported` (size not a multiple).
    CopySize { mv: bool, t: OTy, n: u64, size: u64 },
    /// `note_constexpr_memcpy_unsupported` (`src`: the source, else the
    /// destination, has fewer than `n` elements left).
    CopyRange { mv: bool, src: bool, t: OTy, n: u64 },
    /// `note_constexpr_memcmp_unsupported`.
    CmpType { t1: OTy, t2: OTy },
    /// `note_constexpr_memchr_unsupported`.
    ChrType { t: OTy },
}

fn mask(w: u32) -> u128 {
    if w >= 128 { u128::MAX } else { (1u128 << w) - 1 }
}

/// clang's `isOneByteCharacterType`: `char`, `signed char`, `unsigned
/// char`, `char8_t`.
fn is_narrow_char(ast: &Ast, oven: &Oven, t: OTy) -> bool {
    let q = oven.types.info(t).q;
    matches!(
        ast.types.builtin(q),
        Some(BuiltinType::Char | BuiltinType::SChar | BuiltinType::UChar | BuiltinType::Char8)
    )
}

impl Oven {
    /// Evaluates builtin `aux` (`bi` kind | sub << 8) on registers `a`..`c`
    /// and types `t1`/`t2`.
    #[allow(clippy::too_many_arguments, reason = "the instruction's operands")]
    pub(super) fn builtin(
        &mut self,
        ast: &Ast,
        aux: u32,
        a: u128,
        b: u128,
        c: u128,
        t1: u32,
        t2: u32,
    ) -> Result<u128, Fault> {
        let sub = aux >> 8;
        match aux & 0xFF {
            k @ (bi::CLZ | bi::CTZ) => {
                let w = sub & 0xFF;
                let v = a & mask(w);
                if v == 0 {
                    // clang: the result of the GCC builtins is undefined
                    // for zero (`Error(E)`), unless `clzg`/`ctzg` were
                    // given a fallback.
                    return if sub >> 8 & 1 != 0 { Ok(b) } else { Err(Fault::Invalid) };
                }
                let n =
                    if k == bi::CLZ { v.leading_zeros() - (128 - w) } else { v.trailing_zeros() };
                Ok(u128::from(n))
            }
            bi::POPCOUNT => Ok(u128::from((a & mask(sub & 0xFF)).count_ones())),
            bi::BSWAP => {
                let w = sub & 0xFF;
                Ok((a & mask(w)).swap_bytes() >> (128 - w))
            }
            bi::FP_CLASS => {
                let c = softfloat::classify(float_of(sub & 15), a);
                let r = match sub >> 4 {
                    bi::FINITE => !matches!(c, Class::Inf | Class::NaN),
                    bi::INF => c == Class::Inf,
                    bi::NAN => c == Class::NaN,
                    _ => c == Class::Normal,
                };
                Ok(u128::from(r))
            }
            bi::STRLEN => self.bi_strlen(a as u64, OTy(t1)).map(u128::from),
            bi::MEMCMP => self.bi_memcmp(ast, a as u64, b as u64, c as u64, t1, t2, sub != 0),
            bi::MEMCHR => self.bi_memchr(ast, a as u64, b, c as u64, t1, sub != 0).map(u128::from),
            bi::MEMCPY => {
                self.bi_memcpy(ast, a as u64, b as u64, c as u64, t1, t2, sub != 0).map(|()| a)
            }
            _ => Err(Fault::Invalid),
        }
    }

    /// The type of the object `p` designates: `t`, or (`bi::DYN`, a
    /// `void` pointer) the innermost object at its offset.
    fn designated(&self, p: u64, t: u32) -> Result<OTy, Fault> {
        if t != bi::DYN {
            return Ok(OTy(t));
        }
        let ptr = Ptr::decode(p);
        let Some(a) = self.alloc_any(ptr) else {
            let f = match self.mem.check(p, 1, false) {
                Err(f) => f,
                Ok(_) => MemFault::Stale,
            };
            return Err(Fault::Mem { f, ak: ak::READ, ty: OTy(0), p });
        };
        let off = u64::from(ptr.off);
        if (ptr.past || off >= u64::from(a.size))
            && let TyKind::Array { elem, .. } = self.types.kind(a.ty)
        {
            return Ok(elem);
        }
        Ok(self.deepest_at(a.ty, off))
    }

    /// The bytes `[p, p + n)` of a one-byte-element run, if they are all
    /// initialized plain data of an allocation without unions; `n` is
    /// clamped to the elements left in `p`'s array.
    fn byte_run(&self, p: u64, elem: OTy, n: u64) -> Option<&[u8]> {
        let (_, len, cur, _) = self.extent_of(p, elem).ok()?;
        let n = n.min(len - cur);
        let (sp, abs, a) = self.mem.check(p, n, false).ok()?;
        let space = self.mem.space(sp);
        (n > 0
            && a.flags & flags::HAS_UNION == 0
            && space.is_init(abs, n)
            && !space.any_ptr(abs, n))
        .then(|| &space.bytes[abs as usize..(abs + n) as usize])
    }

    /// `strlen`/`wcslen`: reads elements of type `ct` up to the first
    /// zero.
    fn bi_strlen(&self, mut p: u64, ct: OTy) -> Result<u64, Fault> {
        let mut n = 0u64;
        if self.types.size(ct) == 1
            && let Some(run) = self.byte_run(p, ct, u64::MAX)
        {
            if let Some(i) = run.iter().position(|&x| x == 0) {
                return Ok(i as u64);
            }
            n = run.len() as u64;
            p = self.ptr_add(p, i128::from(n), ct)?;
        }
        loop {
            if self.load(p, ct, ak::READ)? == 0 {
                return Ok(n);
            }
            n += 1;
            p = self.ptr_add(p, 1, ct)?;
        }
    }

    /// `memcmp` (`wide` false: only arrays of narrow character types,
    /// compared as unsigned bytes) and `wmemcmp` (as `wchar_t`).
    #[allow(clippy::too_many_arguments, reason = "the instruction's operands")]
    fn bi_memcmp(
        &self,
        ast: &Ast,
        mut p: u64,
        mut q: u64,
        mut n: u64,
        t1: u32,
        t2: u32,
        wide: bool,
    ) -> Result<u128, Fault> {
        let ret = |o: std::cmp::Ordering| o as i8 as i128 as u128;
        if n == 0 {
            return Ok(0);
        }
        for x in [p, q] {
            if x == 0 {
                let ty = OTy(if t1 == bi::DYN { 0 } else { t1 });
                return Err(Fault::Mem { f: MemFault::Null, ak: ak::READ, ty, p: 0 });
            }
        }
        let (t1, t2) = (self.designated(p, t1)?, self.designated(q, t2)?);
        if !wide {
            if !(is_narrow_char(ast, self, t1) && is_narrow_char(ast, self, t2)) {
                return Err(Fault::Bi(BiFault::CmpType { t1, t2 }));
            }
            if let (Some(x), Some(y)) = (self.byte_run(p, t1, n), self.byte_run(q, t2, n)) {
                let m = x.len().min(y.len());
                if let Some(i) = (0..m).find(|&i| x[i] != y[i]) {
                    return Ok(ret(x[i].cmp(&y[i])));
                }
                if m as u64 == n {
                    return Ok(0);
                }
                n -= m as u64;
                p = self.ptr_add(p, m as i128, t1)?;
                q = self.ptr_add(q, m as i128, t2)?;
            }
        }
        let signed = wide && matches!(self.types.kind(t1), TyKind::Int { signed: true, .. });
        let bits = if wide { u32::MAX } else { 8 };
        for _ in 0..n {
            let x = self.load(p, t1, ak::READ)?;
            let y = self.load(q, t2, ak::READ)?;
            if x != y {
                let o = if signed {
                    (x as i128).cmp(&(y as i128))
                } else {
                    (x & mask(bits)).cmp(&(y & mask(bits)))
                };
                if o.is_ne() {
                    return Ok(ret(o));
                }
            }
            p = self.ptr_add(p, 1, t1)?;
            q = self.ptr_add(q, 1, t2)?;
        }
        Ok(0)
    }

    /// `memchr`/`char_memchr` (`wide` false: the value converted to
    /// `unsigned char`; `memchr` only over narrow character types) and
    /// `wmemchr`: the first of `n` elements at `p` equal to `v`, else null.
    fn bi_memchr(
        &self,
        ast: &Ast,
        mut p: u64,
        v: u128,
        mut n: u64,
        t: u32,
        wide: bool,
    ) -> Result<u64, Fault> {
        if n == 0 {
            return Ok(0);
        }
        if p == 0 {
            let ty = OTy(if t == bi::DYN { 0 } else { t });
            return Err(Fault::Mem { f: MemFault::Null, ak: ak::READ, ty, p: 0 });
        }
        let t = self.designated(p, t)?;
        let w = match self.types.kind(t) {
            TyKind::Int { width, .. } => u32::from(width),
            _ => 8,
        };
        if !wide && !is_narrow_char(ast, self, t) {
            return Err(Fault::Bi(BiFault::ChrType { t }));
        }
        let want = if wide { v & mask(w) } else { v & 0xFF };
        if !wide && let Some(run) = self.byte_run(p, t, n) {
            if let Some(i) = run.iter().position(|&x| u128::from(x) == want) {
                return self.ptr_add(p, i as i128, t);
            }
            n -= run.len() as u64;
            if n == 0 {
                return Ok(0);
            }
            p = self.ptr_add(p, run.len() as i128, t)?;
        }
        for _ in 0..n {
            if self.load(p, t, ak::READ)? & mask(w) == want {
                return Ok(p);
            }
            p = self.ptr_add(p, 1, t)?;
        }
        Ok(0)
    }

    /// Whether objects of type `t` are trivially copyable
    /// ([basic.types.general]/9).
    fn trivially_copyable(&self, ast: &Ast, t: OTy) -> bool {
        match self.types.kind(t) {
            TyKind::Array { elem, .. } => self.trivially_copyable(ast, elem),
            TyKind::Record { decl, .. } => ast
                .decls
                .record(decl)
                .is_some_and(|r| r.defn.contains(DefinitionBits::TRIVIALLY_COPYABLE)),
            TyKind::Func | TyKind::Opaque | TyKind::Void => false,
            _ => true,
        }
    }

    /// `memcpy`/`memmove` (`mv`) of `n` bytes from `src` to `dst`: whole
    /// elements of one trivially copyable type, within both arrays,
    /// assigned one at a time (backward for an overlapping `memmove`).
    #[allow(clippy::too_many_arguments, reason = "the instruction's operands")]
    fn bi_memcpy(
        &mut self,
        ast: &Ast,
        dst: u64,
        src: u64,
        n: u64,
        t1: u32,
        t2: u32,
        mv: bool,
    ) -> Result<(), Fault> {
        if n == 0 {
            return Ok(());
        }
        if src == 0 || dst == 0 {
            return Err(Fault::Bi(BiFault::CopyNull { mv, src: src == 0 }));
        }
        let t = self.designated(dst, t1)?;
        let st = self.designated(src, t2)?;
        if t != st {
            return Err(Fault::Bi(BiFault::CopyPun { mv, src: st, dst: t }));
        }
        if !self.trivially_copyable(ast, t) {
            return Err(Fault::Bi(BiFault::CopyNontrivial { mv, t }));
        }
        let size = self.types.size(t);
        if size == 0 {
            return Err(Fault::Invalid);
        }
        if n % size != 0 {
            return Err(Fault::Bi(BiFault::CopySize { mv, t, n, size }));
        }
        let ne = n / size;
        for (p, is_src) in [(src, true), (dst, false)] {
            let (_, len, cur, _) = self.extent_of(p, t)?;
            if ne > len - cur {
                return Err(Fault::Bi(BiFault::CopyRange { mv, src: is_src, t, n: ne }));
            }
        }
        let mut back = false;
        if Ptr::alloc_key(src) == Ptr::alloc_key(dst) {
            let so = u64::from(Ptr::decode(src).off);
            let dof = u64::from(Ptr::decode(dst).off);
            if dof >= so && dof - so < n {
                if !mv {
                    return Err(Fault::Bi(BiFault::CopyOverlap));
                }
                back = true;
            } else if !mv && so >= dof && so - dof < n {
                return Err(Fault::Bi(BiFault::CopyOverlap));
            }
        }
        let info = *self.types.info(t);
        let plain = !matches!(
            info.kind,
            TyKind::Ptr(_) | TyKind::NullPtr | TyKind::Record { .. } | TyKind::Array { .. }
        );
        if plain && self.bulk_copy(dst, src, n, t) {
            return Ok(());
        }
        for i in 0..ne {
            let i = if back { ne - 1 - i } else { i } as i128;
            let (d, s) = (self.ptr_add(dst, i, t)?, self.ptr_add(src, i, t)?);
            if plain || matches!(info.kind, TyKind::Ptr(_) | TyKind::NullPtr) {
                let v = self.load(s, t, ak::READ)?;
                self.store(d, t, v, ak::ASSIGN, false)?;
            } else {
                self.copy_obj(d, s, t, false)?;
            }
        }
        Ok(())
    }

    /// Copies `n` bytes of scalar elements of type `t` at once when the
    /// element-wise copy could not fail: the source fully initialized
    /// plain data, the destination writable, neither in an allocation
    /// with unions. Returns whether it copied.
    fn bulk_copy(&mut self, dst: u64, src: u64, n: u64, t: OTy) -> bool {
        let Ok((ss, sa, s_alloc)) = self.mem.check(src, n, false) else { return false };
        if s_alloc.flags & flags::HAS_UNION != 0 {
            return false;
        }
        let space = self.mem.space(ss);
        if !space.is_init(sa, n) || space.any_ptr(sa, n) {
            return false;
        }
        let Ok((ds, da, d_alloc)) = self.mem.check(dst, n, true) else { return false };
        if d_alloc.flags & flags::HAS_UNION != 0 || write_fault(ds, d_alloc, t).is_some() {
            return false;
        }
        self.mem.copy(ds, da, ss, sa, n);
        true
    }
}
