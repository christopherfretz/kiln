//! The content key of a reflection intrinsic call, for an evaluation
//! session's diagnostic replay (spec/eval-session.md §7, WP-2074).
//!
//! A run records the diagnostics of the semantic work a call did under
//! the call's key and replays them when a later run makes the same call.
//! An operand that is an address (a range's data pointer, a string, an
//! object of class type passed by address) designates an object of the
//! run, whose address depends on what the run (or an earlier one)
//! allocated. So the key holds no address: each operand is replaced by
//! what the intrinsic reads through it, read through the same path the
//! intrinsic reads it ([`operand`] names the path of every operand):
//! a range's `info` elements as reflections, a value range's elements
//! and a `reflect_constant` operand frozen as values (content-interned
//! [`kiln_ast::ConstValueId`]s), a string's code units, an address the
//! op takes by identity frozen as a pointer (a declaration or a constant
//! plus an offset). Equal content gives one key in every run; different
//! content gives different keys (the encoding is exact, compared whole).
//!
//! The key is computed after the call ran, and only when it is needed
//! (the call reported something, or a record of the op exists): the
//! operands are unchanged by the call, and a value the intrinsic froze
//! is found interned, not interned again. A read that fails ends the
//! operand with [`FAIL`]: the intrinsic, reading the same objects in the
//! same order, stopped there too, so what it did depends only on what
//! came before.

use kiln_ast::{DeclId, QualType};
use kiln_oven::{
    AccessOp, DefineOp, ExtOp, MemberOp, MetaCtx, MetaOp, Oven, QueryOp, TemplOp, TraitOp, ValueOp,
};

use crate::Sema;

/// A read that failed (out of bounds, uninitialized, not a constant).
pub(crate) const FAIL: u128 = u128::MAX;
/// An empty result where a value was expected (an empty value range).
const EMPTY: u128 = u128::MAX - 1;

/// How a reflection intrinsic reads one of its operands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Operand {
    /// The operand's value is its content: an `info`'s bits (interned by
    /// reflectee), an integer, an enumerator, an `access_context`'s two
    /// reflections.
    Bits,
    /// A `reflection_range` (data | size << 64): its `info` elements.
    Infos,
    /// The `const info *` of a `(const info *, size_t)` trait form; the
    /// next operand is the count.
    InfoPtr,
    /// A value range of any element type (`reflect_constant_string`,
    /// `reflect_constant_array`): its elements frozen as the op freezes
    /// them, a string's whole-literal terminator dropped.
    Elems { string: bool },
    /// A range of characters read as bytes (`std::meta::exception`'s
    /// encoding conversions).
    Chars,
    /// `reflect_constant`'s operand: its value frozen at the parameter's
    /// type.
    Value,
    /// `reflect_object`'s and `reflect_function`'s operand: the address
    /// frozen as a pointer to the parameter's type (the entity, not the
    /// encoding).
    Address,
    /// `is_string_literal`'s pointer: whether it points into a string
    /// literal object.
    StringLiteral,
    /// `data_member_spec`'s `data_member_options` object: the options as
    /// read (name, alignment, width, `no_unique_address`, annotations).
    Options,
    /// A NUL-terminated name (`__kiln_define_member`).
    CString,
}

/// How `op` reads its operand `i`: `None` for an operand no table row
/// names (a test checks that every non-`info` operand of every op is
/// named).
pub(crate) fn operand(op: MetaOp, i: usize) -> Option<Operand> {
    use Operand as K;
    if op.sig().as_bytes().get(i) == Some(&b'i') || scalar_operand(op, i) {
        return Some(K::Bits);
    }
    Some(match (op, i) {
        (MetaOp::Query(QueryOp::KilnReflectionRange), 0)
        | (
            MetaOp::Template(TemplOp::CanSubstitute | TemplOp::Substitute)
            | MetaOp::Define(DefineOp::DefineAggregate),
            1,
        ) => K::Infos,
        (MetaOp::Trait(t), _) if super::traits::range_n(t).is_some_and(|(_, k)| k == i) => K::Infos,
        (MetaOp::Trait(t), _) if super::traits::is_range_n(t) => {
            let ptr = op.sig().find('x')?;
            match i.checked_sub(ptr)? {
                0 => K::InfoPtr,
                1 => K::Bits,
                _ => return None,
            }
        }
        (MetaOp::Value(v), 0) => match v {
            ValueOp::ReflectConstant => K::Value,
            ValueOp::ReflectObject | ValueOp::ReflectFunction => K::Address,
            ValueOp::ReflectConstantString | ValueOp::ReflectConstantStringStd => {
                K::Elems { string: true }
            }
            ValueOp::ReflectConstantArray | ValueOp::ReflectConstantArrayStd => {
                K::Elems { string: false }
            }
            ValueOp::IsStringLiteral => K::StringLiteral,
            ValueOp::ExcCvtFromUtf8 | ValueOp::ExcCvtToUtf8 => K::Chars,
            ValueOp::Extract | ValueOp::ReflectConstantInfo => return None,
        },
        (MetaOp::Define(DefineOp::DataMemberSpec), 1) => K::Options,
        (MetaOp::Ext(ExtOp::DefineMember), 2) => K::CString,
        _ => return None,
    })
}

/// Whether operand `i` of `op` is a scalar that is not an `info`: an
/// access context (two reflections), an `operators` enumerator, a member
/// offset's integers, an index or a rank.
fn scalar_operand(op: MetaOp, i: usize) -> bool {
    match op {
        MetaOp::Member(
            MemberOp::MembersOf
            | MemberOp::BasesOf
            | MemberOp::StaticDataMembersOf
            | MemberOp::NonstaticDataMembersOf
            | MemberOp::SubobjectsOf,
        )
        | MetaOp::Access(
            AccessOp::IsAccessible
            | AccessOp::HasInaccessibleNonstaticDataMembers
            | AccessOp::HasInaccessibleBases
            | AccessOp::HasInaccessibleSubobjects,
        )
        | MetaOp::Trait(TraitOp::Extent) => i == 1,
        MetaOp::Query(QueryOp::SymbolOf | QueryOp::U8SymbolOf)
        | MetaOp::Trait(TraitOp::TupleElement | TraitOp::VariantAlternative) => i == 0,
        MetaOp::Query(QueryOp::KilnMemberOffset) => i <= 1,
        _ => false,
    }
}

/// Appends `bytes`: their length, then 16 per word.
fn push_bytes(k: &mut Vec<u128>, bytes: &[u8]) {
    k.push(bytes.len() as u128);
    for c in bytes.chunks(16) {
        let mut w = [0u8; 16];
        w[..c.len()].copy_from_slice(c);
        k.push(u128::from_le_bytes(w));
    }
}

/// An `Option<i64>` as one word.
fn opt_word(v: Option<i64>) -> u128 {
    v.map_or(0, |v| 1 << 64 | u128::from(v.cast_unsigned()))
}

impl Sema<'_, '_> {
    /// The replay key of intrinsic call `op(args)` that ran with `ctx`
    /// (see the module documentation): the context that decides what the
    /// op may do (a trial, an evaluation point), a value op's
    /// specialization types, then each operand as [`operand`] reads it.
    /// What the computation interns is not charged to the evaluation
    /// ([`Oven::meta_uncharged`]): a call without a session would not
    /// have interned it.
    pub(crate) fn meta_replay_key(
        &mut self,
        op: MetaOp,
        args: &[u128],
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Box<[u128]> {
        let (content, entries) = (self.ast.consts.content_bytes(), self.ast.reflections.len());
        let mut k = Vec::with_capacity(args.len() + 2);
        // The context the op reads: whether it may complete or
        // instantiate (a trial; inside semantic work), and, only for the
        // ops that read them, the evaluation point's scope
        // (`access_context::current`), the nested evaluation point
        // (`variable_of`) and whether the evaluation is plainly constant-
        // evaluated (the injecting ops). Keying the others by the scope
        // would make two calls of one query differ by where they are.
        let decl = |d: Option<DeclId>| d.map_or(0, |d| 1 + u128::from(d.index()));
        let point = match op {
            MetaOp::Access(_) => decl(ctx.scope),
            MetaOp::Query(QueryOp::VariableOf) => decl(ctx.nested_point),
            MetaOp::Define(_) | MetaOp::Ext(_) => u128::from(ctx.plainly),
            _ => 0,
        };
        k.push(u128::from(ctx.trial) | u128::from(self.inst.eval_loc.is_some()) << 1 | point << 2);
        let mut p0 = None;
        if let MetaOp::Value(_) = op {
            // A function template specialization: its `T`.
            k.push(match self.value_sig(ctx.callee) {
                Some((ret, p)) => {
                    p0 = p.map(|p| self.referenced(p)).map(|t| (op, t));
                    u128::from(ret.to_raw()) | u128::from(p.map_or(0, QualType::to_raw)) << 32
                }
                None => FAIL,
            });
        }
        for (i, &a) in args.iter().enumerate() {
            let kind = operand(op, i);
            debug_assert!(kind.is_some(), "operand {i} of '{}' has no key row", op.display());
            let next = args.get(i + 1).copied();
            self.key_operand(kind.unwrap_or(Operand::Bits), (a, next), p0, ctx, oven, &mut k);
        }
        let grew = self.ast.consts.content_bytes().saturating_sub(content);
        let added = self.ast.reflections.len().saturating_sub(entries) as u64;
        oven.meta_uncharged(grew, added);
        k.into_boxed_slice()
    }

    /// Appends operand `a` (the operand after it: `next`) read as `kind`;
    /// `p0` is a value op with its parameter type, unreferenced. An
    /// operand the op rejects by its type alone (before reading it) is
    /// [`EMPTY`]: the type is in the key already.
    fn key_operand(
        &mut self,
        kind: Operand,
        (a, next): (u128, Option<u128>),
        p0: Option<(MetaOp, QualType)>,
        ctx: &MetaCtx,
        oven: &mut Oven,
        k: &mut Vec<u128>,
    ) {
        let loc = ctx.loc;
        match kind {
            Operand::Bits => k.push(a),
            Operand::Infos => {
                let n = oven.meta_range_len(a);
                k.push(u128::from(n));
                for i in 0..n {
                    let Ok(r) = oven.meta_range_at(a, i) else {
                        k.push(FAIL);
                        break;
                    };
                    k.push(u128::from(r.bits()));
                }
            }
            Operand::InfoPtr => {
                // The count is the next operand (in the key on its own);
                // a count over the bound fails before any element is read.
                let n = next
                    .and_then(|n| u64::try_from(n).ok())
                    .filter(|&n| n <= super::traits::MAX_RANGE)
                    .unwrap_or(0);
                for i in 0..n {
                    let Ok(r) = oven.meta_info_at(a, i, ctx) else {
                        k.push(FAIL);
                        break;
                    };
                    k.push(u128::from(r.bits()));
                }
            }
            Operand::Elems { string } => {
                let e = self.unqual(oven.meta_elem(ctx));
                k.push(u128::from(e.to_raw()));
                if self.elems_type_error(e, string).is_some() {
                    k.push(EMPTY);
                    return;
                }
                let Ok(m) = oven.meta_elems_len(a, string, ctx) else {
                    k.push(FAIL);
                    return;
                };
                k.push(u128::from(m));
                if m == 0 && !string {
                    k.push(EMPTY);
                    return;
                }
                let arr = self.ast.types.constant_array(e, m + u64::from(string), None);
                let cv = self.bridge(|b| oven.meta_freeze_elems(b, (a, m), arr, string, ctx));
                k.push(cv.map_or(FAIL, |cv| u128::from(cv.index())));
            }
            Operand::Chars => match oven.meta_elems_bytes(a, (a >> 64) as u64, ctx) {
                Ok(Some(bytes)) => push_bytes(k, &bytes),
                Ok(None) => k.push(EMPTY),
                Err(_) => k.push(FAIL),
            },
            Operand::Value | Operand::Address => {
                let Some((op, t)) = p0 else {
                    k.push(FAIL);
                    return;
                };
                let types = &self.ast.types;
                let rejected = match op {
                    MetaOp::Value(ValueOp::ReflectObject) => types.is_function(t),
                    MetaOp::Value(ValueOp::ReflectFunction) => !types.is_function(t),
                    _ => types.is_array(t) || types.is_function(t) || !self.is_structural(t),
                };
                if rejected {
                    k.push(EMPTY);
                    return;
                }
                let t = if kind == Operand::Address { self.ast.types.pointer_to(t) } else { t };
                let cv = self.bridge(|b| oven.meta_freeze(b, a, t, loc));
                k.push(cv.map_or(FAIL, |cv| u128::from(cv.index())));
            }
            Operand::StringLiteral => k.push(u128::from(oven.meta_is_string_literal(a))),
            Operand::Options => {
                let raw = oven.dms_plan().map(|plan| oven.meta_dms_options(a, &plan));
                let Some(Ok(raw)) = raw else {
                    k.push(FAIL);
                    return;
                };
                let name = raw.name_u8.map_or(0, |u8| 1 + u128::from(u8));
                k.push(name | u128::from(raw.nua) << 2);
                k.push(opt_word(raw.alignment));
                k.push(opt_word(raw.bit_width));
                if raw.name_u8.is_some() {
                    push_bytes(k, oven.dms_name_bytes());
                }
                let anns = oven.dms_annotation_args();
                k.push(anns.len() as u128);
                k.extend(anns.iter().map(|r| u128::from(r.bits())));
            }
            Operand::CString => match oven.meta_c_bytes(a, ctx) {
                Ok(bytes) => push_bytes(k, &bytes),
                Err(_) => k.push(FAIL),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every operand of every op is read by a named path: an op added
    /// later gets a row here, or its address operands would be keyed by
    /// their encoding.
    #[test]
    fn every_operand_has_a_key_row() {
        for op in MetaOp::all() {
            for i in 0..op.sig().len() {
                assert!(
                    operand(op, i).is_some(),
                    "operand {i} of '{}' has no key row",
                    op.display()
                );
            }
        }
    }

    /// The rows agree with the call-site shapes: a range operand is read
    /// as elements, an access context as its bits.
    #[test]
    fn range_rows_match_the_trait_forms() {
        assert_eq!(operand(MetaOp::Template(TemplOp::Substitute), 1), Some(Operand::Infos));
        assert_eq!(operand(MetaOp::Trait(TraitOp::IsConstructibleType), 1), Some(Operand::Infos));
        assert_eq!(
            operand(MetaOp::Trait(TraitOp::IsConstructibleTypeN), 1),
            Some(Operand::InfoPtr)
        );
        assert_eq!(operand(MetaOp::Trait(TraitOp::IsConstructibleTypeN), 2), Some(Operand::Bits));
        assert_eq!(operand(MetaOp::Trait(TraitOp::CommonTypeN), 0), Some(Operand::InfoPtr));
        assert_eq!(operand(MetaOp::Trait(TraitOp::IsInvocableRTypeN), 2), Some(Operand::InfoPtr));
        assert_eq!(operand(MetaOp::Member(MemberOp::MembersOf), 1), Some(Operand::Bits));
        assert_eq!(operand(MetaOp::Value(ValueOp::Extract), 0), Some(Operand::Bits));
    }

    #[test]
    fn byte_strings_are_length_prefixed() {
        let mut a = Vec::new();
        push_bytes(&mut a, b"ab");
        let mut b = Vec::new();
        push_bytes(&mut b, b"ab\0");
        assert_ne!(a, b);
        assert_eq!(a.len(), 2);
        let mut c = Vec::new();
        push_bytes(&mut c, &[7; 17]);
        assert_eq!(c.len(), 3);
    }
}
