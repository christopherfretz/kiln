//! Reading carried values of tags 5 to 9 back (spec/modules.md §11.11
//! "Encoding", WP-1047): pointers, references, pointers to members,
//! reflections and aggregates holding them, written by the entities they
//! name.
//!
//! An entity is resolved to the importer's own declaration: a declared
//! entity (`D`) by its kind and replay-stable position, through an index
//! of the first, non-implicit declarations at each position (two at one
//! position resolve to neither); a specialization (`S`) among its
//! template's specializations by their keys; a member of a specialization
//! (`M`, `P`) through the specialization's member map. Every resolved
//! declaration is checked by writing its key again and comparing the
//! bytes, so a value names exactly the entity the writer's did or is not
//! used (the evaluation runs). Types are rebuilt in canonical form.
//!
//! The section was checked when it was installed ([`super::inst`]'s value
//! grammar), but decoding is total all the same: anything unexpected is
//! `None`, never a panic.

use std::hash::{Hash, Hasher};

use kiln_ast::consts::{
    ByteImage, ByteRange, IntBits, MemberKind, NullKind, PtrBase, Reloc, Scalar, UnionMember,
};
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{BuiltinType, ExcSpec, FuncInfo, Quals, TypeView, VectorKind};
use kiln_ast::{ConstValueId, DeclId, QualType};
use kiln_source::fx::{FxHashMap, FxHasher};

use super::inst::{
    self, BASE_CONST, BASE_DECL, BASE_FUNCTION, BASE_PARAM_OBJECT, BASE_TYPE_INFO, ENT_DECL,
    ENT_MEMBER, ENT_PATTERN, ENT_SPEC, ENT_TU, REFL_ALIAS, REFL_TYPE, TY_ARRAY, TY_ATOMIC,
    TY_BITINT, TY_BUILTIN, TY_COMPLEX, TY_ENUM, TY_FUNCTION, TY_INCOMPLETE_ARRAY, TY_LREF,
    TY_MEMBER_POINTER, TY_POINTER, TY_RECORD, TY_RREF, TY_VECTOR,
};
use super::{
    LocKey, Rd, TAG_AGGREGATE, TAG_AGGREGATE_EX, TAG_MEMBER_NULL, TAG_MEMBER_POINTER, TAG_POINTER,
    TAG_REFLECTION, TAG_TYPED, file_index, loc_key, read_plain,
};
use crate::Sema;

/// Nesting deeper than this is not decoded (the writer's keys stop well
/// before it).
const MAX_DEPTH: u32 = 128;

/// One table's resolved entities.
#[derive(Default)]
pub(super) struct Index {
    /// Declarations below this id are in `by_pos`.
    cursor: u32,
    /// [`pos_hash`] of a first, non-implicit declaration's position and
    /// kind -> the declaration, `None` when more than one has it.
    by_pos: FxHashMap<u64, Option<DeclId>>,
    /// Entity bytes -> declaration, resolved and checked.
    ents: FxHashMap<Box<[u8]>, DeclId>,
    /// Template -> how many of its specializations' keys are in `ents`.
    specs: FxHashMap<DeclId, usize>,
}

fn pos_hash(at: LocKey, kind: u8) -> u64 {
    let mut h = FxHasher::default();
    at.hash(&mut h);
    kind.hash(&mut h);
    h.finish()
}

impl Sema<'_, '_> {
    /// Interns the value `b` encodes (a whole [`super::Carried::Wire`]) as
    /// a value of type `ty`, with table `t`'s entities, or `None` when an
    /// entity does not resolve or the value does not fit.
    pub(super) fn intern_wire(&mut self, t: usize, ty: QualType, b: &[u8]) -> Option<ConstValueId> {
        let mut r = Rd(b);
        let mut ty = ty;
        if r.0.first() == Some(&TAG_TYPED) {
            // A reference's value: of the named reference type, which must
            // refer to the expression's.
            r.u8().ok()?;
            let rt = self.wire_ty(t, &mut r, 0)?;
            if self.carried_referee(rt) != Some(ty) {
                return None;
            }
            ty = rt;
        }
        let v = self.wire_value(t, ty, &mut r, 0)?;
        r.0.is_empty().then_some(v)
    }

    fn wire_value(
        &mut self,
        t: usize,
        ty: QualType,
        r: &mut Rd<'_>,
        depth: u32,
    ) -> Option<ConstValueId> {
        if depth > MAX_DEPTH {
            return None;
        }
        let d = depth + 1;
        if r.0.first().is_some_and(|&x| x <= TAG_AGGREGATE) {
            let c = read_plain(r).ok()?;
            return self.intern_carried(t, ty, &c);
        }
        let view = self.ast.types.canonical_view(ty);
        let member_pointer = matches!(view, TypeView::MemberPointer { .. });
        let s = match r.u8().ok()? {
            TAG_MEMBER_NULL => {
                let kind = match r.u8().ok()? {
                    1 => NullKind::MemberData,
                    2 => NullKind::MemberFunction,
                    _ => return None,
                };
                if !member_pointer {
                    return None;
                }
                Scalar::Null { ty, kind }
            }
            TAG_POINTER => {
                if !matches!(
                    view,
                    TypeView::Pointer(_) | TypeView::LValueRef(_) | TypeView::RValueRef(_)
                ) {
                    return None;
                }
                let flags = r.u8().ok()?;
                let base = match r.u8().ok()? {
                    BASE_DECL => PtrBase::Decl(self.wire_ent(t, r, d)?),
                    BASE_FUNCTION => PtrBase::Function(self.wire_ent(t, r, d)?),
                    BASE_TYPE_INFO => PtrBase::TypeInfo(self.wire_ty(t, r, d)?),
                    BASE_PARAM_OBJECT => {
                        let pt = self.wire_ty(t, r, d)?;
                        let pv = self.wire_value(t, pt, r, d)?;
                        PtrBase::Decl(self.ast.templates.param_object(pv)?)
                    }
                    BASE_CONST => {
                        let ct = self.wire_ty(t, r, d)?;
                        PtrBase::Const(self.wire_value(t, ct, r, d)?)
                    }
                    _ => return None,
                };
                let offset = r.var().ok()?;
                let designated = if flags & 2 != 0 { Some(self.wire_ty(t, r, d)?) } else { None };
                Scalar::Pointer { ty, base, offset, designated, past: flags & 1 != 0 }
            }
            TAG_MEMBER_POINTER => {
                if !member_pointer {
                    return None;
                }
                let flags = r.u8().ok()?;
                let member = self.wire_ent(t, r, d)?;
                let n = r.var_count(1).ok()?;
                let mut path = Vec::with_capacity(n);
                for _ in 0..n {
                    path.push(self.wire_ent(t, r, d)?);
                }
                let kind = if flags & 1 != 0 { MemberKind::Function } else { MemberKind::Data };
                let s =
                    Scalar::MemberPtr { ty, member, kind, derived: flags & 2 != 0, path: &path };
                return self.ast.consts.push_scalar(s).ok();
            }
            TAG_REFLECTION => {
                if !matches!(view, TypeView::Builtin(BuiltinType::MetaInfo)) {
                    return None;
                }
                let refl = self.wire_reflectee(t, r, d)?;
                let id = self.ast.reflections.intern(refl).ok()?;
                Scalar::Reflection { ty, id }
            }
            TAG_AGGREGATE_EX => return self.wire_aggregate(t, ty, r, d),
            _ => return None,
        };
        self.ast.consts.push_scalar(s).ok()
    }

    fn wire_aggregate(
        &mut self,
        t: usize,
        ty: QualType,
        r: &mut Rd<'_>,
        d: u32,
    ) -> Option<ConstValueId> {
        if matches!(self.ast.types.canonical_view(ty), TypeView::Builtin(_))
            || self.ast.types.is_pointer(ty)
        {
            return None;
        }
        let (size, _) = crate::target::size_align(&self.ast, ty, self.abi())?;
        let bytes = r.bytes().ok()?;
        if size != bytes.len() as u64 {
            return None;
        }
        let small = |r: &mut Rd<'_>| r.var().ok().and_then(|v| u32::try_from(v).ok());
        let n = r.var_count(2).ok()?;
        let mut padding = Vec::with_capacity(n);
        for _ in 0..n {
            padding.push(ByteRange { start: small(r)?, len: small(r)? });
        }
        let n = r.var_count(3).ok()?;
        let mut relocs = Vec::with_capacity(n);
        for _ in 0..n {
            let at = small(r)?;
            let vt = self.wire_ty(t, r, d)?;
            relocs.push(Reloc { at, value: self.wire_value(t, vt, r, d)? });
        }
        let n = r.var_count(2).ok()?;
        let mut unions = Vec::with_capacity(n);
        for _ in 0..n {
            let at = small(r)?;
            unions.push(UnionMember { at, field: self.wire_ent(t, r, d)? });
        }
        let img = ByteImage { ty, bytes, relocs: &relocs, padding: &padding, unions: &unions };
        self.ast.consts.push_aggregate(img).ok()
    }

    fn wire_reflectee(&mut self, t: usize, r: &mut Rd<'_>, d: u32) -> Option<Reflectee> {
        Some(match r.u8().ok()? {
            0 => match r.u8().ok()? {
                REFL_TYPE => Reflectee::Type(self.wire_ty(t, r, d)?),
                REFL_ALIAS => {
                    let a = self.wire_ent(t, r, d)?;
                    let n = self.ast.decls.get(a);
                    if !matches!(n.kind(), DeclKind::Typedef | DeclKind::TypeAlias) {
                        return None;
                    }
                    let u = self.ast.types.canonical(n.ty()?);
                    Reflectee::Type(self.ast.types.typedef_type(a, u).into())
                }
                _ => return None,
            },
            1 => Reflectee::Decl(self.wire_first(t, r, d)?),
            2 => Reflectee::Null,
            3 => Reflectee::Namespace(self.wire_first(t, r, d)?),
            4 => Reflectee::NamespaceAlias(self.wire_first(t, r, d)?),
            5 => {
                let x = self.wire_first(t, r, d)?;
                let n = self.template_name_for(x);
                Reflectee::Template(self.ast.types.canonical_template_name(n))
            }
            tag @ (6 | 7) => {
                let vt = self.wire_ty(t, r, d)?;
                let v = self.wire_value(t, vt, r, d)?;
                if tag == 6 { Reflectee::Value(v) } else { Reflectee::Object(v) }
            }
            tag @ (8 | 11) => {
                let x = self.wire_first(t, r, d)?;
                let i = u32::try_from(r.var().ok()?).ok()?;
                if tag == 8 { Reflectee::BaseSpec(x, i) } else { Reflectee::FnParam(x, i) }
            }
            _ => return None,
        })
    }

    /// A structural type, rebuilt canonical.
    fn wire_ty(&mut self, t: usize, r: &mut Rd<'_>, depth: u32) -> Option<QualType> {
        if depth > MAX_DEPTH {
            return None;
        }
        let d = depth + 1;
        let cv = r.u8().ok()?;
        if cv > 7 {
            return None;
        }
        let q = match r.u8().ok()? {
            TY_BUILTIN => BuiltinType::from_index(u32::from(r.u8().ok()?))?.qual_type(),
            TY_BITINT => {
                let signed = r.flag().ok()?;
                let width = r.u32().ok()?;
                if width == 0 || width > IntBits::MAX_WIDTH {
                    return None;
                }
                self.ast.types.bit_int(signed, width)
            }
            TY_POINTER => {
                let p = self.wire_ty(t, r, d)?;
                self.ast.types.pointer_to(p)
            }
            TY_LREF => {
                let p = self.wire_ty(t, r, d)?;
                self.ast.types.lvalue_ref_to(p)
            }
            TY_RREF => {
                let p = self.wire_ty(t, r, d)?;
                self.ast.types.rvalue_ref_to(p)
            }
            TY_MEMBER_POINTER => {
                let p = self.wire_ty(t, r, d)?;
                let c = self.wire_ty(t, r, d)?;
                if !matches!(self.ast.types.canonical_view(c), TypeView::Record(_)) {
                    return None;
                }
                self.ast.types.member_pointer(p, c.ty())
            }
            TY_ARRAY => {
                let n = r.u64().ok()?;
                let e = self.wire_ty(t, r, d)?;
                self.ast.types.constant_array(e, n, None)
            }
            TY_INCOMPLETE_ARRAY => {
                let e = self.wire_ty(t, r, d)?;
                self.ast.types.incomplete_array(e)
            }
            TY_FUNCTION => {
                let ret = self.wire_ty(t, r, d)?;
                let n = r.count(2).ok()?;
                let mut params = Vec::with_capacity(n);
                for _ in 0..n {
                    params.push(self.wire_ty(t, r, d)?);
                }
                let b = r.take(2).ok()?;
                let info = FuncInfo::from_bits(u16::from_le_bytes([b[0], b[1]]));
                let exc = match r.u8().ok()? {
                    0 => ExcSpec::None,
                    1 => ExcSpec::Nothrow,
                    _ => return None,
                };
                self.ast.types.function(ret, &params, info, exc)
            }
            tag @ (TY_RECORD | TY_ENUM) => {
                let e = self.wire_ent(t, r, d)?;
                let q = self.ast.types.canonical(self.ast.decls.get(e).ty()?);
                let ok = match self.ast.types.canonical_view(q) {
                    TypeView::Record(_) => tag == TY_RECORD,
                    TypeView::Enum(_) => tag == TY_ENUM,
                    _ => false,
                };
                if !ok {
                    return None;
                }
                q
            }
            TY_COMPLEX => {
                let e = self.wire_ty(t, r, d)?;
                self.ast.types.complex(e)
            }
            TY_ATOMIC => {
                let e = self.wire_ty(t, r, d)?;
                self.ast.types.atomic(e)
            }
            TY_VECTOR => {
                let e = self.wire_ty(t, r, d)?;
                let count = r.u32().ok()?;
                let kind = VectorKind::from_u8(r.u8().ok()?);
                self.ast.types.vector(e, count, kind)
            }
            _ => return None,
        };
        let q = self.ast.types.qualify(q, Quals::from_bits(cv));
        Some(self.ast.types.canonical(q))
    }

    /// The entity at the start of `r`, resolved.
    fn wire_ent(&mut self, t: usize, r: &mut Rd<'_>, depth: u32) -> Option<DeclId> {
        let n = inst::ent_len(r.0)?;
        let b = r.take(n).ok()?;
        self.resolve_ent(t, b, depth)
    }

    /// The first declaration of the entity at the start of `r`.
    fn wire_first(&mut self, t: usize, r: &mut Rd<'_>, depth: u32) -> Option<DeclId> {
        let d = self.wire_ent(t, r, depth)?;
        Some(self.ast.first(d))
    }

    /// The importer's declaration for entity bytes `b` of table `t`.
    fn resolve_ent(&mut self, t: usize, b: &[u8], depth: u32) -> Option<DeclId> {
        if depth > MAX_DEPTH {
            return None;
        }
        if let Some(&d) = self.bmi.tables[t].ix.ents.get(b) {
            return Some(d);
        }
        let inner = |b: &[u8]| inst::ent_len(b.get(1..)?).map(|n| 1 + n);
        let d = match *b.first()? {
            ENT_TU => return Some(self.ast.tu),
            ENT_DECL => self.find_decl(t, &b[1..])?,
            ENT_SPEC => {
                let n = inner(b)?;
                let template = self.resolve_ent(t, &b[1..n], depth + 1)?;
                self.index_specs(t, self.ast.first(template));
                return self.bmi.tables[t].ix.ents.get(b).copied();
            }
            ENT_MEMBER | ENT_PATTERN => {
                let n = inner(b)?;
                let ctx = self.resolve_ent(t, &b[1..n], depth + 1)?;
                let pattern = self.resolve_ent(t, &b[n..], depth + 1)?;
                let templates = &self.ast.templates;
                templates.find_member(ctx, pattern)?
            }
            _ => return None,
        };
        let key = self.key_in(t, d)?;
        if *key != *b {
            return None;
        }
        self.bmi.tables[t].ix.ents.insert(key, d);
        Some(d)
    }

    /// The entity bytes of `d` under table `t`'s paths.
    fn key_in(&mut self, t: usize, d: DeclId) -> Option<Box<[u8]>> {
        let sm = self.diags.sm();
        let bmi = &mut self.bmi;
        inst::decl_key(&self.ast, sm, |f| file_index(bmi, Some(t), sm, f), d)
    }

    /// The declaration a `D` entity names (`rest` follows the tag).
    fn find_decl(&mut self, t: usize, rest: &[u8]) -> Option<DeclId> {
        let mut r = Rd(rest);
        let kind = r.u8().ok()?;
        match r.u8().ok()? {
            0 => {
                r.bytes().ok()?;
            }
            1 => {
                r.u8().ok()?;
            }
            _ => {}
        }
        let mut at = [0u32; 4];
        for w in &mut at {
            *w = r.u32().ok()?;
        }
        self.index_decls(t);
        self.bmi.tables[t].ix.by_pos.get(&pos_hash(LocKey(at), kind)).copied().flatten()
    }

    /// Adds the declarations made since the last call to table `t`'s
    /// position index.
    fn index_decls(&mut self, t: usize) {
        let sm = self.diags.sm();
        let end = self.ast.decls.len() as u32 + 1;
        let mut cur = self.bmi.tables[t].ix.cursor.max(1);
        while cur < end {
            let d = DeclId::new(cur);
            cur += 1;
            let n = self.ast.decls.get(d);
            if n.has(DeclFlags::IMPLICIT) || self.ast.first(d) != d {
                continue;
            }
            let bmi = &mut self.bmi;
            let Some(at) = loc_key(sm, n.loc(), |f| file_index(bmi, Some(t), sm, f)) else {
                continue;
            };
            let slot =
                bmi.tables[t].ix.by_pos.entry(pos_hash(at, n.kind() as u8)).or_insert(Some(d));
            if *slot != Some(d) {
                *slot = None;
            }
        }
        self.bmi.tables[t].ix.cursor = cur;
    }

    /// Adds the keys of `template`'s specializations made since the last
    /// call to table `t`'s resolved entities.
    fn index_specs(&mut self, t: usize, template: DeclId) {
        let from = self.bmi.tables[t].ix.specs.get(&template).copied().unwrap_or(0);
        let n = self.ast.templates.specs_of(template).len();
        for i in from..n {
            let s = self.ast.templates.specs_of(template)[i];
            if let Some(k) = self.key_in(t, s) {
                self.bmi.tables[t].ix.ents.entry(k).or_insert(s);
            }
        }
        self.bmi.tables[t].ix.specs.insert(template, n);
    }
}
