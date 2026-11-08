//! The padding bits of an object representation (spec/abi.md §3
//! "Padding bits", WP-774): which bits of an object of a given type hold
//! no part of its value ([basic.types.general]/4), as GCC's
//! `__builtin_clear_padding` (`gimple-fold.cc` `clear_padding_type`)
//! sees them.
//!
//! A bit is a value bit when some scalar subobject's value representation
//! covers it: the bytes of a scalar (all of them, `bool` and
//! `std::nullptr_t` included, except the 6 trailing bytes of x87
//! `long double` and the bits above a `_BitInt`'s width), the declared
//! bits of a named bit-field, and the vtable pointer. Everything else is
//! padding: gaps between and after members, unnamed bit-fields, the byte
//! of an empty class, and in a union the bits no member's value covers
//! (a bit that is padding in every member).

use kiln_ast::types::{BuiltinType, FloatFormat, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_source::fx::FxHashMap;

use super::{Layouter, props};

/// The largest object (in bytes) [`padding_mask`] describes; a larger
/// one gets `None`.
pub const MAX_PADDING_MASK_BYTES: u64 = 1 << 24;

/// The padding mask of an object of type `q`: one byte per byte of its
/// object representation, a set bit marking a padding bit. `None` for an
/// object larger than [`MAX_PADDING_MASK_BYTES`]. Bit-fields are laid out
/// little-endian (the only byte order kiln targets).
pub fn padding_mask(lay: &mut Layouter<'_>, q: QualType) -> Option<Vec<u8>> {
    let size = lay.type_layout(q).size;
    if size > MAX_PADDING_MASK_BYTES {
        return None;
    }
    let mut v = vec![0u8; usize::try_from(size).ok()?];
    let mut p = Padding { lay, records: FxHashMap::default() };
    p.value(q, &mut v);
    for b in &mut v {
        *b = !*b;
    }
    Some(v)
}

struct Padding<'l, 'a> {
    lay: &'l mut Layouter<'a>,
    /// Value-bit masks of classes, complete (`true`) or as a base.
    records: FxHashMap<(DeclId, bool), Vec<u8>>,
}

/// Sets bits `[from, from + n)` of `out` (little-endian bit numbering).
fn set_bits(out: &mut [u8], from: u64, n: u64) {
    let end = from.saturating_add(n).min(out.len() as u64 * 8);
    let mut b = from;
    while b < end {
        let byte = (b / 8) as usize;
        let lo = b % 8;
        let hi = (end - (b - lo)).min(8);
        // Bits lo..hi of this byte.
        let m = ((1u16 << hi) - (1u16 << lo)) as u8;
        out[byte] |= m;
        b = b - lo + 8;
    }
}

fn or_into(out: &mut [u8], at: u64, src: &[u8]) {
    let Ok(at) = usize::try_from(at) else { return };
    for (o, s) in out.iter_mut().skip(at).zip(src) {
        *o |= *s;
    }
}

impl Padding<'_, '_> {
    /// ORs the value bits of a `q` object into `out` (its bytes).
    fn value(&mut self, q: QualType, out: &mut [u8]) {
        let ast = self.lay.ast();
        let size = out.len() as u64;
        match ast.types.canonical_view(q) {
            TypeView::Builtin(b) => {
                let fmt = ast.types.target().long_double;
                let ld = matches!(b, BuiltinType::LongDouble | BuiltinType::Float64x);
                if ld && fmt == FloatFormat::X87Extended {
                    set_bits(out, 0, 80);
                } else {
                    set_bits(out, 0, size * 8);
                }
            }
            TypeView::BitInt { width, .. } => set_bits(out, 0, u64::from(width)),
            TypeView::ConstantArray(a) => {
                let esz = self.lay.type_layout(a.elem).size;
                if esz == 0 || a.size == 0 {
                    return;
                }
                let mut e = vec![0u8; esz as usize];
                self.value(a.elem, &mut e);
                for i in 0..a.size {
                    or_into(out, i * esz, &e);
                }
            }
            TypeView::Complex(elem) => {
                let esz = self.lay.type_layout(elem).size;
                let mut e = vec![0u8; esz as usize];
                self.value(elem, &mut e);
                or_into(out, 0, &e);
                or_into(out, esz, &e);
            }
            TypeView::Vector { elem, count, .. } => {
                // A vector of `bool` packs its elements as bits; every
                // other vector is its elements, any tail being padding.
                let esz = self.lay.type_layout(elem).size;
                if matches!(ast.types.canonical_view(elem), TypeView::Builtin(BuiltinType::Bool))
                    || esz == 0
                {
                    set_bits(out, 0, size * 8);
                } else {
                    let mut e = vec![0u8; esz as usize];
                    self.value(elem, &mut e);
                    for i in 0..u64::from(count) {
                        or_into(out, i * esz, &e);
                    }
                }
            }
            // An `_Atomic(T)` wider than `T` pads it.
            TypeView::Atomic(t) if self.lay.type_layout(t).size <= size => {
                let n = self.lay.type_layout(t).size as usize;
                self.value(t, &mut out[..n]);
            }
            TypeView::Record(rd) => {
                let m = self.record(rd, true);
                or_into(out, 0, &m);
            }
            TypeView::IncompleteArray(_) | TypeView::Function(_) => {}
            // Enumerations (their integer underlying type), pointers,
            // references (stored as pointers), member pointers.
            _ => set_bits(out, 0, size * 8),
        }
    }

    /// The value-bit mask of class `rd`, as a complete object or as a
    /// base subobject (its non-virtual part).
    fn record(&mut self, rd: DeclId, complete: bool) -> Vec<u8> {
        let ast = self.lay.ast();
        let key = (ast.first(rd), complete);
        if let Some(m) = self.records.get(&key) {
            return m.clone();
        }
        let l = self.lay.record(rd).clone();
        let size = if complete { l.size } else { l.nv_size };
        let mut out = vec![0u8; usize::try_from(size).unwrap_or(0)];
        if let Some(def) = ast.definition(rd) {
            if let Some(v) = l.vptr {
                let pb = u64::from(ast.types.target().pointer_bytes);
                set_bits(&mut out, v * 8, pb * 8);
            }
            for &(b, off) in &*l.bases {
                if !self.lay.is_empty(b) {
                    let m = self.record(b, false);
                    or_into(&mut out, off, &m);
                }
            }
            if complete {
                for &(b, off) in &*l.vbases {
                    if !self.lay.is_empty(b) {
                        let m = self.record(b, false);
                        or_into(&mut out, off, &m);
                    }
                }
            }
            for (i, d) in props::fields(ast, def).into_iter().enumerate() {
                let bit = l.fields.get(i).copied().unwrap_or(0);
                if let Some(w) = d.bit_width {
                    if d.named {
                        let tw = self.lay.type_layout(d.ty).size * 8;
                        set_bits(&mut out, bit, w.min(tw));
                    }
                    continue;
                }
                let at = bit / 8;
                // A `[[no_unique_address]]` class member is laid out as a
                // base subobject: an empty one holds only padding, a
                // non-empty one may end in the enclosing class's tail.
                if d.nua
                    && let Some(r) = props::as_record(ast, d.ty)
                {
                    if !self.lay.is_empty(r) {
                        let m = self.record(r, false);
                        or_into(&mut out, at, &m);
                    }
                    continue;
                }
                let fsz = self.lay.type_layout(d.ty).size;
                if fsz == 0 {
                    continue;
                }
                let mut f = vec![0u8; usize::try_from(fsz).unwrap_or(0)];
                self.value(d.ty, &mut f);
                or_into(&mut out, at, &f);
            }
        }
        self.records.insert(key, out.clone());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::set_bits;

    #[test]
    fn set_bits_spans_bytes() {
        let mut v = [0u8; 4];
        set_bits(&mut v, 3, 7);
        assert_eq!(v, [0xf8, 0x03, 0, 0]);
        let mut v = [0u8; 4];
        set_bits(&mut v, 0, 32);
        assert_eq!(v, [0xff; 4]);
        let mut v = [0u8; 2];
        set_bits(&mut v, 10, 100);
        assert_eq!(v, [0, 0xfc]);
    }
}
