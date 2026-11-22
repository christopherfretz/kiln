//! Union paths of constant pointers (spec/ast.md §9.1 "Union paths",
//! WP-2200).
//!
//! A constant pointer is `(base, offset)`: the designator path from the
//! base's object to the subobject it designates is recovered from the
//! types and the offset, which cannot tell two members of one union apart
//! when both hold an object of the pointee type there (`union { int a;
//! int b; } u`: `&u.a` and `&u.b`). So a pointer stores the union members
//! its path enters that are not their union's **default member**, the
//! one a reader takes when the path names none
//! ([`union_default_member`]); every reader (printing, mangling,
//! substitution) and the Oven's freeze apply the same rule, through
//! [`union_path_member`] and [`canonical_union_path`].

use super::print::{Member, PrintEnv, Shape};
use crate::ids::{DeclId, QualType};

/// How deep a walk descends at most (a bound on a malformed type; a real
/// object nests far less).
const MAX_DEPTH: u32 = 512;

/// Whether member `m` of a record covers byte `off` of the record (a
/// non-bit-field at a byte offset; an empty member covers its offset).
fn covers(m: &Member, off: u64) -> bool {
    let at = m.offset_bits / 8;
    m.bit_width.is_none()
        && m.offset_bits.is_multiple_of(8)
        && at <= off
        && (off < at + m.size || (off == at && m.size == 0))
}

/// Whether an object of type `ty` has a subobject of type `pointee` (the
/// object itself included) at byte `off`.
fn holds<E: PrintEnv + ?Sized>(
    env: &E,
    ty: QualType,
    off: u64,
    pointee: QualType,
    depth: u32,
) -> bool {
    if depth > MAX_DEPTH {
        return false;
    }
    if off == 0 && env.same_unqualified(ty, pointee) {
        return true;
    }
    match env.shape(ty) {
        Shape::Array { elem, stride, len } if stride != 0 => {
            let i = off / stride;
            i < len && holds(env, elem, off - i * stride, pointee, depth + 1)
        }
        Shape::Record { members, .. } => members.iter().any(|m| {
            covers(m, off) && holds(env, m.ty, off - m.offset_bits / 8, pointee, depth + 1)
        }),
        _ => false,
    }
}

/// The default member of a union (`members`, its [`Shape::Record`]
/// members) for a designator through byte `off` of the union toward a
/// subobject of type `pointee`: the first non-bit-field member in
/// declaration order whose bytes hold `off` and that has a subobject of
/// type `pointee` there; without one (or without a pointee), the first
/// member whose bytes hold `off`.
pub fn union_default_member<'m, E: PrintEnv + ?Sized>(
    env: &E,
    members: &'m [Member],
    off: u64,
    pointee: Option<QualType>,
) -> Option<&'m Member> {
    let mut hits = members.iter().filter(|m| m.field.is_some() && covers(m, off));
    let first = hits.clone().next();
    match pointee {
        Some(p) => hits.find(|m| holds(env, m.ty, off - m.offset_bits / 8, p, 0)).or(first),
        None => first,
    }
}

/// The member of a union (`members`) a designator through byte `off`
/// enters: the member union path `sel` names, if it names one of this
/// union's that holds `off`, else [`union_default_member`].
pub fn union_path_member<'m, E: PrintEnv + ?Sized>(
    env: &E,
    members: &'m [Member],
    off: u64,
    pointee: Option<QualType>,
    sel: &[DeclId],
) -> Option<&'m Member> {
    if !sel.is_empty()
        && let Some(m) =
            members.iter().find(|m| m.field.is_some_and(|f| sel.contains(&f)) && covers(m, off))
    {
        return Some(m);
    }
    union_default_member(env, members, off, pointee)
}

/// The canonical union path ([`super::Scalar::Pointer`] `unions`) of a
/// pointer to the subobject of type `pointee` at byte `offset` of an
/// object of type `root`, whose designator names the union members `sel`
/// (in any order; at most one per union): walking from `root`, at each
/// union the member `sel` names (when it holds the subobject) else the
/// default member is entered, and it is kept when it is not the default.
/// Other records are entered by their first member holding the
/// subobject, arrays by the element holding `offset`. Outermost first.
pub fn canonical_union_path<E: PrintEnv + ?Sized>(
    env: &E,
    root: QualType,
    offset: u64,
    pointee: QualType,
    sel: &[DeclId],
) -> Vec<DeclId> {
    let mut out = Vec::new();
    if !sel.is_empty() {
        union_walk(env, root, offset, pointee, sel, |m, default| {
            if !default {
                out.extend(m);
            }
        });
    }
    out
}

/// Every union member the designator of the pointer
/// [`canonical_union_path`] describes enters, outermost first: the members
/// `sel` (a union path) names and, at the other unions on the way, the
/// default members. The Oven tags a thawed pointer with them, so that a
/// pointer derived from it (`&p->x` of `p = &u.t`) still designates the
/// member `p` does.
pub fn union_members_entered<E: PrintEnv + ?Sized>(
    env: &E,
    root: QualType,
    offset: u64,
    pointee: QualType,
    sel: &[DeclId],
) -> Vec<DeclId> {
    let mut out = Vec::new();
    union_walk(env, root, offset, pointee, sel, |m, _| out.extend(m));
    out
}

/// The walk of [`canonical_union_path`]: `f(member, is_default)` at each
/// union entered.
fn union_walk<E: PrintEnv + ?Sized>(
    env: &E,
    root: QualType,
    offset: u64,
    pointee: QualType,
    sel: &[DeclId],
    mut f: impl FnMut(Option<DeclId>, bool),
) {
    let (mut cur, mut off) = (root, offset);
    for _ in 0..MAX_DEPTH {
        if off == 0 && env.same_unqualified(cur, pointee) {
            break;
        }
        match env.shape(cur) {
            Shape::Array { elem, stride, len } if stride != 0 => {
                let i = off / stride;
                if i >= len {
                    break;
                }
                off -= i * stride;
                cur = elem;
            }
            Shape::Record { members, union: true } => {
                let Some(def) = union_default_member(env, &members, off, Some(pointee)) else {
                    break;
                };
                let named = members.iter().find(|m| {
                    m.field.is_some_and(|f| sel.contains(&f))
                        && covers(m, off)
                        && holds(env, m.ty, off - m.offset_bits / 8, pointee, 0)
                });
                let m = match named {
                    Some(m) if m.field != def.field => m,
                    _ => def,
                };
                f(m.field, m.field == def.field);
                off -= m.offset_bits / 8;
                cur = m.ty;
            }
            Shape::Record { members, union: false } => {
                let Some(m) = members.iter().find(|m| {
                    covers(m, off) && holds(env, m.ty, off - m.offset_bits / 8, pointee, 0)
                }) else {
                    break;
                };
                off -= m.offset_bits / 8;
                cur = m.ty;
            }
            _ => break,
        }
    }
}
