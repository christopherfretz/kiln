//! `bi::TRAITS`: a `std::char_traits<C>` operation evaluated natively
//! (WP-1864, WP-1918), and `bi::MINMAX`, a `std::min`/`std::max` of two
//! scalars (WP-1917; `compile/fast.rs`; spec/oven.md "Library fast
//! paths").
//!
//! The operation reads the elements the library's loop reads, in its
//! order, through the same pointer arithmetic and loads (`Oven::ptr_add`,
//! `Oven::load`), so each read it completes is one the loop completes. A
//! read the loop would fault on, a budget the call could cross (the
//! steps, the depth of the call and of the helper it calls) or a
//! potential-constant-expression check declines: nothing has changed
//! (both are reads), and the call runs and diagnoses as before.
//!
//! A run of elements within one array is checked once
//! ([`Oven::tr_run`]): the access to all of them passes the checks of a
//! load (allocation, bounds, lifetime, union members, initialization,
//! no pointer bytes) exactly when the access to each does, since they are
//! the same scalar position of one array. A run that does not pass is
//! read element by element, as the loop reads it.

use super::udesig;
use super::{Fault, value_len};
use crate::Oven;
use crate::arith::{canon, float_cmp};
use crate::kbc::{ak, bi};
use crate::mem::{Sp, flags};
use crate::ty::{OTy, TyKind};

/// A completed operation: its value and the elements it examined.
type Done = Option<(u128, u64)>;

impl Oven {
    /// `bi::TRAITS` `sub` on the arguments at `regs[nb..]` (element type
    /// `et`, result type `rt`): whether it evaluated the operation (the
    /// result is then in `regs[nb]`). It charges one step per element
    /// examined, at least one.
    pub(super) fn traits_fast(&mut self, nb: usize, sub: u32, et: OTy, rt: OTy) -> bool {
        // The call and the helper it calls (`lt`, `eq`, `length`).
        if self.run.potential.is_some() || self.active_calls() + 2 > self.budget.depth {
            return false;
        }
        let Some(left) = self.budget.steps.checked_sub(self.run.counters.steps) else {
            return false;
        };
        let TyKind::Int { width, signed } = self.types.kind(et) else { return false };
        let cmp = Cmp { width: u32::from(width), signed: signed && sub & bi::TR_UNSIGNED == 0 };
        let arg = |i: usize| self.regs[nb + i];
        if sub & 0xF == bi::TR_ASSIGN {
            return left > 0 && self.tr_assign(arg(0), arg(1), et);
        }
        let done = match sub & 0xF {
            bi::TR_COMPARE => {
                self.tr_compare(arg(0) as u64, arg(1) as u64, arg(2) as u64, et, cmp, left)
            }
            bi::TR_LENGTH => self.tr_length(arg(0) as u64, et, cmp, left),
            bi::TR_FIND => self.tr_find(arg(0) as u64, arg(1) as u64, arg(2) as u64, et, cmp, left),
            _ => None,
        };
        let Some((v, n)) = done else { return false };
        let steps = n.max(1);
        if steps > left {
            return false;
        }
        let v = match self.types.kind(rt) {
            TyKind::Int { width, signed } => canon(v, u32::from(width), signed),
            _ => v,
        };
        self.run.counters.steps += steps;
        self.regs[nb] = v;
        true
    }

    /// `assign(c1, c2)` (libstdc++: `std::construct_at(&c1, c2)` in a
    /// constant evaluation, libc++: `c1 = c2`), as the assignment: a read
    /// of `c2` and a write of `c1` that both pass, with one step. A
    /// target in or through a union, inside a `mutable` member or on a
    /// const path declines, as does any fault: there construction and
    /// assignment may differ, and the library code diagnoses.
    fn tr_assign(&mut self, p: u128, q: u128, et: OTy) -> bool {
        if udesig::desig_of(p) != 0 || udesig::desig_of(q) != 0 {
            return false;
        }
        let (p, q) = (p as u64, q as u64);
        let Ok(v) = self.load(q, et, ak::READ) else { return false };
        let len = value_len(self.types.info(et).size, self.types.info(et).kind);
        let Ok((_, _, a)) = self.mem.check(p, len, true) else { return false };
        if a.flags & (flags::HAS_UNION | flags::MUTABLE_OUTSIDE | flags::CONST_PATH) != 0
            || self.store(p, et, v, ak::ASSIGN, false).is_err()
        {
            return false;
        }
        self.run.counters.steps += 1;
        true
    }

    /// `bi::MINMAX` `sub` on the two `const T&` arguments at `regs[nb..]`
    /// (`T` the scalar type `t`; WP-1917): whether it evaluated the call
    /// (the address of the argument it returns is then in `regs[nb]`).
    /// `min(a, b)` is `b < a ? b : a` and `max(a, b)` is `a < b ? b : a`
    /// in both libraries (with `comp(x, y)` for `x < y` given a
    /// comparator); both reads must pass, and it charges one step.
    pub(super) fn minmax_fast(&mut self, nb: usize, sub: u32, t: OTy) -> bool {
        // The call, libc++'s comparator overload, and its comparator call.
        if self.run.potential.is_some()
            || self.active_calls() + 3 > self.budget.depth
            || self.run.counters.steps >= self.budget.steps
        {
            return false;
        }
        let (ra, rb) = (self.regs[nb], self.regs[nb + 1]);
        let arg = |r: u128, value: bool| {
            if value {
                Some(r)
            } else if udesig::desig_of(r) != 0 {
                None
            } else {
                self.load(r as u64, t, ak::READ).ok()
            }
        };
        let (Some(a), Some(b)) =
            (arg(ra, sub & bi::MM_VAL_A != 0), arg(rb, sub & bi::MM_VAL_B != 0))
        else {
            return false;
        };
        // `x < y` (or `y < x` for `std::greater`) picks `b`.
        let (x, y) = if sub & bi::MM_MAX != 0 { (a, b) } else { (b, a) };
        let (x, y) = if sub & bi::MM_GREATER != 0 { (y, x) } else { (x, y) };
        let less = match self.types.kind(t) {
            TyKind::Int { width, signed: true } => {
                let w = u32::from(width);
                (canon(x, w, true) as i128) < (canon(y, w, true) as i128)
            }
            TyKind::Int { width, signed: false } => {
                let w = u32::from(width);
                canon(x, w, false) < canon(y, w, false)
            }
            TyKind::Bool => (x & 1) < (y & 1),
            TyKind::Float(k) => float_cmp(k, x, y) == Some(std::cmp::Ordering::Less),
            _ => return false,
        };
        self.run.counters.steps += 1;
        self.regs[nb] = match (sub & bi::MM_VALUE != 0, less) {
            (true, true) => b,
            (true, false) => a,
            (false, true) => rb,
            (false, false) => ra,
        };
        true
    }

    /// The `n` elements at `p` when they lie in one array and a load of
    /// each would succeed (checked once for all of them): their space,
    /// the first one's address and the element size.
    fn tr_run(&self, p: u64, n: u64, et: OTy) -> Option<(Sp, u64, u64)> {
        if n == 0 {
            return None;
        }
        let (_, len, cur, _) = self.extent_of(p, et).ok()?;
        let info = self.types.info(et);
        let es = value_len(info.size, info.kind);
        if n > len.checked_sub(cur)? || es != info.size || es == 0 {
            return None;
        }
        let (sp, abs, a) = self.mem.check(p, n.checked_mul(es)?, false).ok()?;
        if a.flags & (flags::HAS_UNION | flags::MUTABLE_OUTSIDE) != 0
            && self.slow_access(a, p, 0, et, ak::READ).is_err()
        {
            return None;
        }
        let space = self.mem.space(sp);
        (space.is_init(abs, n * es) && !space.any_ptr(abs, n * es)).then_some((sp, abs, es))
    }

    /// Element `i` of a run [`Oven::tr_run`] checked.
    fn tr_at(&self, (sp, abs, es): (Sp, u64, u64), i: u64) -> u128 {
        self.mem.space(sp).read(abs + i * es, es)
    }

    /// The element `i` past `p` (`p[i]`), read as the loop reads it.
    fn tr_elem(&self, p: u64, i: u64, et: OTy) -> Result<(u64, u128), Fault> {
        let at = self.ptr_add(p, i128::from(i), et)?;
        Ok((at, self.load(at, et, ak::READ)?))
    }

    /// `compare(p, q, n)` (libstdc++: `lt(p[i], q[i])`, then `lt(q[i],
    /// p[i])`, for each `i` up to the first difference).
    #[allow(clippy::too_many_arguments, reason = "the operation's operands")]
    fn tr_compare(&self, p: u64, q: u64, n: u64, et: OTy, c: Cmp, left: u64) -> Done {
        if n <= left
            && let (Some(x), Some(y)) = (self.tr_run(p, n, et), self.tr_run(q, n, et))
        {
            for i in 0..n {
                match c.key(self.tr_at(x, i)).cmp(&c.key(self.tr_at(y, i))) {
                    std::cmp::Ordering::Less => return Some(((-1i128) as u128, i + 1)),
                    std::cmp::Ordering::Greater => return Some((1, i + 1)),
                    std::cmp::Ordering::Equal => {}
                }
            }
            return Some((0, n));
        }
        for i in 0..n {
            if i >= left {
                return None;
            }
            let (_, x) = self.tr_elem(p, i, et).ok()?;
            let (_, y) = self.tr_elem(q, i, et).ok()?;
            match c.key(x).cmp(&c.key(y)) {
                std::cmp::Ordering::Less => return Some(((-1i128) as u128, i + 1)),
                std::cmp::Ordering::Greater => return Some((1, i + 1)),
                std::cmp::Ordering::Equal => {}
            }
        }
        Some((0, n))
    }

    /// `length(p)`: the elements before the first null one.
    fn tr_length(&self, p: u64, et: OTy, c: Cmp, left: u64) -> Done {
        let mut i = 0u64;
        loop {
            if i >= left {
                return None;
            }
            let (_, x) = self.tr_elem(p, i, et).ok()?;
            if c.key(x) == 0 {
                return Some((u128::from(i), i + 1));
            }
            i += 1;
        }
    }

    /// `find(p, n, *a)`: the address of the first of `n` elements at `p`
    /// equal to `*a`, else null (`*a` is read only when `n > 0`).
    #[allow(clippy::too_many_arguments, reason = "the operation's operands")]
    fn tr_find(&self, p: u64, n: u64, a: u64, et: OTy, c: Cmp, left: u64) -> Done {
        if n == 0 {
            return Some((0, 0));
        }
        let want = c.key(self.load(a, et, ak::READ).ok()?);
        if n <= left
            && let Some(x) = self.tr_run(p, n, et)
        {
            return match (0..n).find(|&i| c.key(self.tr_at(x, i)) == want) {
                Some(i) => Some((u128::from(self.ptr_add(p, i128::from(i), et).ok()?), i + 1)),
                None => Some((0, n)),
            };
        }
        for i in 0..n {
            if i >= left {
                return None;
            }
            let (at, x) = self.tr_elem(p, i, et).ok()?;
            if c.key(x) == want {
                return Some((u128::from(at), i + 1));
            }
        }
        Some((0, n))
    }
}

/// How elements compare: as their type's values, or as unsigned.
#[derive(Clone, Copy)]
struct Cmp {
    width: u32,
    signed: bool,
}

impl Cmp {
    /// The element's value, ordered as the operation orders it.
    fn key(self, v: u128) -> i128 {
        let m = if self.width >= 128 { u128::MAX } else { (1u128 << self.width) - 1 };
        let u = v & m;
        if self.signed && self.width < 128 && u >> (self.width - 1) & 1 != 0 {
            (u | !m) as i128
        } else {
            u as i128
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cmp;

    #[test]
    fn keys_order_as_the_traits_lt_does() {
        // `char_traits<char>::lt` compares `unsigned char`s: '\xff' > 'a'.
        let u = Cmp { width: 8, signed: false };
        assert!(u.key(0xFF) > u.key(u128::from(b'a')));
        assert!(u.key(u128::MAX) > u.key(1), "a sign-extended char is still 0xff");
        // A signed `wchar_t` orders as its values: -1 < 1.
        let s = Cmp { width: 32, signed: true };
        assert!(s.key((-1i128) as u128) < s.key(1));
        assert!(s.key(0xFFFF_FFFF) < s.key(0));
        assert_eq!(s.key(0), 0);
        let w = Cmp { width: 32, signed: false };
        assert!(w.key(0xFFFF_FFFF) > w.key(0));
    }
}
