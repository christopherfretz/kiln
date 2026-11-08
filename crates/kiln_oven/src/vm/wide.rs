//! `Op::Wide`: `_BitInt(N)` with `N > 128` (`TyKind::WideInt`), whose
//! values live in memory (spec/oven.md "`_BitInt`"). Operands are read
//! with the checks of `Load` and results written with those of `Store`;
//! the limb arithmetic is `crate::bigint`, checked against the width as
//! `arith::int_bin` checks a register value ([expr.pre]/4 for signed
//! types; unsigned ones wrap, [basic.fundamental]/2).

use super::{Fault, compare, write_fault};
use crate::Oven;
use crate::arith::{IntErr, canon};
use crate::bigint as bi;
use crate::compile::float_of;
use crate::kbc::{ak, wide};
use crate::mem::{MemFault, flags};
use crate::softfloat;
use crate::ty::OTy;

impl Oven {
    /// Limbs of a wide object of type `ty`.
    fn wide_limbs(&self, ty: OTy) -> usize {
        (self.types.size(ty) / 8) as usize
    }

    /// The canonical limbs of the wide value of type `ty` at `raw`.
    pub(crate) fn wide_load(&self, raw: u64, ty: OTy, k: u32) -> Result<Vec<u64>, Fault> {
        let len = self.types.size(ty);
        let fault = |f| Fault::Mem { f, ak: k, ty, p: raw };
        let (st, abs, a) = self.rmw_check(raw, len, ty, k)?;
        if a.flags & (flags::HAS_UNION | flags::MUTABLE_OUTSIDE) != 0 {
            self.slow_access(a, raw, ty, k)?;
        }
        let sp = self.mem.space(st);
        if !sp.is_init(abs, len) {
            return Err(fault(MemFault::Uninit));
        }
        if sp.any_ptr(abs, len) {
            return Err(fault(MemFault::Incompatible));
        }
        let mut w: Vec<u64> = (0..len / 8).map(|i| sp.read(abs + i * 8, 8) as u64).collect();
        if let Some((width, signed)) = self.types.wide(ty) {
            bi::canon(&mut w, width, signed);
        }
        Ok(w)
    }

    /// Stores the canonical limbs `w` as the wide value of type `ty` at
    /// `raw` (`init`: an initialization).
    fn wide_store(
        &mut self,
        raw: u64,
        ty: OTy,
        w: &[u64],
        k: u32,
        init: bool,
    ) -> Result<(), Fault> {
        let len = self.types.size(ty);
        let k = if init { ak::INIT } else { k };
        let fault = |f| Fault::Mem { f, ak: k, ty, p: raw };
        let (st, abs, a) = self.mem.check(raw, len, true).map_err(fault)?;
        if !init && let Some(f) = write_fault(st, a, ty) {
            return Err(fault(f));
        }
        let has_union = a.flags & flags::HAS_UNION != 0;
        let mutable = a.flags & (flags::MUTABLE_OUTSIDE | flags::CONST) == flags::MUTABLE_OUTSIDE;
        if mutable && !init {
            self.mutable_path(raw, ty, k)?;
        }
        self.mem.trial_write(raw, len).map_err(fault)?;
        if has_union {
            self.union_path(raw, ty, k)?;
        }
        let sp = self.mem.space_mut(st);
        for (i, &x) in w.iter().enumerate().take((len / 8) as usize) {
            sp.write(abs + i as u64 * 8, 8, u128::from(x));
        }
        sp.set_init(abs, len, true);
        sp.set_ptr_mark(abs, len, 0);
        Ok(())
    }

    /// The canonical limbs of the bit-field of wide declared type `ty`
    /// at `raw`, bit `bit` of that byte, `width` bits (the checks of
    /// `load_bits`).
    fn wide_load_bits(
        &self,
        raw: u64,
        ty: OTy,
        bit: u32,
        width: u32,
        k: u32,
    ) -> Result<Vec<u64>, Fault> {
        let nb = u64::from((bit + width).div_ceil(8));
        let fault = |f| Fault::Mem { f, ak: k, ty, p: raw };
        let (st, abs, a) = self.rmw_check(raw, nb, ty, k)?;
        if a.flags & (flags::HAS_UNION | flags::MUTABLE_OUTSIDE) != 0 {
            self.slow_access(a, raw, ty, k)?;
        }
        let sp = self.mem.space(st);
        if !sp.is_init(abs, nb) {
            return Err(fault(MemFault::Uninit));
        }
        let Some((tw, signed)) = self.types.wide(ty) else { return Err(Fault::Invalid) };
        let n = self.wide_limbs(ty);
        let mut w = vec![0u64; n.max(nb.div_ceil(8) as usize)];
        for i in 0..nb {
            w[(i / 8) as usize] |= (sp.read(abs + i, 1) as u64) << (8 * (i % 8));
        }
        let mut v = bi::shr(&w, bit, false);
        bi::canon(&mut v, width.clamp(1, tw), signed);
        v.truncate(n);
        Ok(v)
    }

    /// Stores the low `width` bits of `v` as the bit-field of wide declared
    /// type `ty` at `raw`, bit `bit` (the checks of `store_bits`).
    fn wide_store_bits(
        &mut self,
        raw: u64,
        ty: OTy,
        v: &[u64],
        (bit, width): (u32, u32),
        k: u32,
        init: bool,
    ) -> Result<(), Fault> {
        let nb = u64::from((bit + width).div_ceil(8));
        let k = if init { ak::INIT } else { k };
        let fault = |f| Fault::Mem { f, ak: k, ty, p: raw };
        let (st, abs, a) = self.mem.check(raw, nb, true).map_err(fault)?;
        if !init && let Some(f) = write_fault(st, a, ty) {
            return Err(fault(f));
        }
        let has_union = a.flags & flags::HAS_UNION != 0;
        let mutable = a.flags & (flags::MUTABLE_OUTSIDE | flags::CONST) == flags::MUTABLE_OUTSIDE;
        if mutable && !init {
            self.mutable_path(raw, ty, k)?;
        }
        self.mem.trial_write(raw, nb).map_err(fault)?;
        if has_union {
            self.union_path(raw, ty, k)?;
        }
        if width == 0 {
            return Ok(());
        }
        let sp = self.mem.space_mut(st);
        // Bytes not yet initialized hold zero bits.
        let mut bytes: Vec<u8> = (0..nb)
            .map(|i| if sp.is_init(abs + i, 1) { sp.read(abs + i, 1) as u8 } else { 0 })
            .collect();
        for j in 0..width {
            let on = v.get((j / 64) as usize).is_some_and(|&x| x >> (j % 64) & 1 != 0);
            let at = bit + j;
            let (byte, m) = ((at / 8) as usize, 1u8 << (at % 8));
            bytes[byte] = if on { bytes[byte] | m } else { bytes[byte] & !m };
        }
        for (i, &b) in bytes.iter().enumerate() {
            sp.write(abs + i as u64, 1, u128::from(b));
        }
        sp.set_init(abs, nb, true);
        sp.set_ptr(abs, nb, false);
        Ok(())
    }

    /// `Op::Wide` `[d, a, b, ty, x]` with aux `aux` (`regs` the values of
    /// registers `d`, `a`, `b`): the register value for `d` when the
    /// sub-op yields one (`CMP`, `TO`, `TO_BOOL`).
    pub(crate) fn exec_wide(
        &mut self,
        regs: [u128; 3],
        ty: OTy,
        x: u32,
        aux: u32,
    ) -> Result<Option<u128>, Fault> {
        let [d, a, b] = regs;
        let sub = aux & 0x3F;
        let k = aux >> 8;
        let init = aux & wide::INIT != 0;
        let ka = if aux & wide::RMW != 0 { k } else { ak::READ };
        let Some((width, signed)) = self.types.wide(ty) else { return Err(Fault::Invalid) };
        let n = self.wide_limbs(ty);
        let r = match sub {
            wide::FROM => {
                let (w, s) = (x & 0xFF, x >> 8 & 1 != 0);
                let mut r = bi::from_u128(canon(a, w, s), s, n);
                bi::canon(&mut r, width, signed);
                r
            }
            wide::TO | wide::TO_BOOL => {
                let v = self.wide_load(a as u64, ty, ak::READ)?;
                if sub == wide::TO_BOOL {
                    return Ok(Some(u128::from(!bi::is_zero(&v))));
                }
                let (w, s) = (x & 0xFF, x >> 8 & 1 != 0);
                // A shift count: clamped to `i128` (any count out of it
                // is out of range for every shift).
                if x >> 9 & 1 != 0 && !bi::fits(&v, 128, signed) {
                    let neg = signed && bi::is_neg(&v);
                    return Ok(Some(if neg { i128::MIN as u128 } else { i128::MAX as u128 }));
                }
                return Ok(Some(canon(bi::low_u128(&v), w, s)));
            }
            wide::TO_FLOAT => {
                let v = self.wide_load(a as u64, ty, ka)?;
                let neg = signed && bi::is_neg(&v);
                let mag = if neg { bi::neg(&v) } else { v };
                return Ok(Some(softfloat::from_limbs(float_of(x), neg, &mag)));
            }
            wide::FROM_FLOAT => {
                let kind = float_of(x);
                let range = Fault::FloatRange { kind, bits: a };
                let Some((neg, mag)) = softfloat::trunc_limbs(kind, a, n + 1) else {
                    return Err(range);
                };
                // Exact in `n + 1` limbs only below `2^(64 n)`, which
                // bounds every width of `n` limbs.
                if mag[n] != 0 {
                    return Err(range);
                }
                let zero = bi::is_zero(&mag);
                let v = if neg { bi::neg(&mag) } else { mag };
                if (neg && !zero && !signed) || !bi::fits(&v, width, signed) {
                    return Err(range);
                }
                let mut r = v;
                r.truncate(n);
                r
            }
            wide::LOAD_BITS => self.wide_load_bits(a as u64, ty, x & 7, x >> 3, ka)?,
            wide::STORE_BITS => {
                let v = self.wide_load(a as u64, ty, ak::READ)?;
                self.wide_store_bits(d as u64, ty, &v, (x & 7, x >> 3), k, init)?;
                return Ok(None);
            }
            wide::CAST => {
                let from = OTy(x);
                let (_, fs) = self.types.wide(from).unwrap_or((width, signed));
                let v = self.wide_load(a as u64, from, ak::READ)?;
                let mut r = bi::extend(&v, n.max(v.len()), fs);
                r.truncate(n);
                bi::canon(&mut r, width, signed);
                r
            }
            wide::CMP => {
                let va = self.wide_load(a as u64, ty, ak::READ)?;
                let vb = self.wide_load(b as u64, ty, ak::READ)?;
                return Ok(Some(u128::from(compare(x, bi::cmp(&va, &vb, signed)))));
            }
            wide::MOV => self.wide_load(a as u64, ty, ak::READ)?,
            _ => {
                let va = self.wide_load(a as u64, ty, ka)?;
                let unary = matches!(sub, wide::NEG | wide::NOT | wide::INC | wide::DEC);
                let shift = matches!(sub, wide::SHL | wide::SHR);
                let vb = if unary || shift {
                    Vec::new()
                } else {
                    self.wide_load(b as u64, ty, ak::READ)?
                };
                match wide_bin(sub, width, signed, &va, &vb, b, x) {
                    Ok(r) => r,
                    Err((err, text)) => {
                        return Err(Fault::Wide { err, text: text.into_boxed_str(), width });
                    }
                }
            }
        };
        self.wide_store(d as u64, ty, &r, k, init)?;
        Ok(None)
    }
}

/// `a op b` (or `op a`) on canonical limbs of a `width`-bit integer;
/// `count`/`cx` are a shift's count register and its width | signed << 8.
/// An error carries the decimal text clang's note prints (the exact
/// result, or the shift count).
fn wide_bin(
    sub: u32,
    width: u32,
    signed: bool,
    a: &[u64],
    b: &[u64],
    count: u128,
    cx: u32,
) -> Result<Vec<u64>, (IntErr, String)> {
    let n = a.len();
    // Signed `+`, `-`: exact in one more limb.
    let exact = |r: Vec<u64>| -> Result<Vec<u64>, (IntErr, String)> {
        if signed && !bi::fits(&r, width, true) {
            return Err((IntErr::Overflow, bi::to_decimal(&r, true)));
        }
        let mut r = r;
        r.truncate(n);
        bi::canon(&mut r, width, signed);
        Ok(r)
    };
    let one = bi::from_u128(1, false, n + 1);
    let ext = |v: &[u64], m: usize| bi::extend(v, m, signed);
    match sub {
        wide::ADD => exact(bi::add(&ext(a, n + 1), &ext(b, n + 1))),
        wide::SUB => exact(bi::sub(&ext(a, n + 1), &ext(b, n + 1))),
        wide::INC => exact(bi::add(&ext(a, n + 1), &one)),
        wide::DEC => exact(bi::sub(&ext(a, n + 1), &one)),
        wide::NEG => exact(bi::neg(&ext(a, n + 1))),
        wide::MUL => exact(bi::mul(&ext(a, 2 * n + 1), &ext(b, 2 * n + 1))),
        wide::DIV | wide::REM => {
            if bi::is_zero(b) {
                return Err((IntErr::DivZero, String::new()));
            }
            let (na, nb) = (signed && bi::is_neg(a), signed && bi::is_neg(b));
            let ma = if na { bi::neg(&ext(a, n + 1)) } else { ext(a, n + 1) };
            let mb = if nb { bi::neg(&ext(b, n + 1)) } else { ext(b, n + 1) };
            let (q, r) = bi::divrem_u(&ma, &mb);
            // MIN / -1 (and MIN % -1, as clang): the quotient overflows.
            let q = if na == nb { q } else { bi::neg(&q) };
            if signed && !bi::fits(&q, width, true) {
                return Err((IntErr::Overflow, bi::to_decimal(&q, true)));
            }
            exact(if sub == wide::DIV {
                q
            } else if na {
                bi::neg(&r)
            } else {
                r
            })
        }
        wide::SHL | wide::SHR => {
            let (cw, cs) = (cx & 0xFF, cx >> 8 & 1 != 0);
            let c = canon(count, cw, cs);
            if cs && (c as i128) < 0 {
                return Err((IntErr::ShiftNeg, (c as i128).to_string()));
            }
            if c >= u128::from(width) {
                let t = if cs { (c as i128).to_string() } else { c.to_string() };
                return Err((IntErr::ShiftBig, t));
            }
            let mut r =
                if sub == wide::SHL { bi::shl(a, c as u32) } else { bi::shr(a, c as u32, signed) };
            bi::canon(&mut r, width, signed);
            Ok(r)
        }
        wide::AND | wide::OR | wide::XOR | wide::NOT => {
            let mut r: Vec<u64> = match sub {
                wide::AND => a.iter().zip(b).map(|(x, y)| x & y).collect(),
                wide::OR => a.iter().zip(b).map(|(x, y)| x | y).collect(),
                wide::XOR => a.iter().zip(b).map(|(x, y)| x ^ y).collect(),
                _ => a.iter().map(|x| !x).collect(),
            };
            bi::canon(&mut r, width, signed);
            Ok(r)
        }
        _ => Err((IntErr::DivZero, String::new())),
    }
}
