//! `[meta.reflection.define.aggregate]` (spec/reflection.md §6): the
//! op table, the data member descriptions `data_member_spec` hands out
//! (interned by content, §2.3), and the reader of a
//! `data_member_options` object in Oven memory. Sema validates the
//! options and defines the class (`kiln_sema/src/meta/define.rs`).

use kiln_ast::{DeclId, DmsId, QualType, ReflectionId};
use kiln_source::fx::FxHashMap;
use kiln_source::symbol::Symbol;

use super::{MetaError, MetaErrorKind, OvenFault};
use crate::Oven;
use crate::kbc::ak;
use crate::mem::Ptr;
use crate::meta::materialize::{Shape, recipe};
use crate::ty::{OTy, TyKind};
use crate::vm::Fault;

super::meta_family! { pub enum DefineOp {
    DataMemberSpec = ("data_member_spec", "ix", Stable, 480);
    IsDataMemberSpec = ("is_data_member_spec", "i", Stable, 480);
    DefineAggregate = ("define_aggregate", "ix", Mutating, 480);
} }

/// A data member description ([meta.reflection.define.aggregate]/3: the
/// tuple (T, N, A, W, NUA)), validated by sema. Two equal descriptions
/// are one reflection (spec/reflection.md §2.3): the name's encoding
/// (`u8string` or `string`) is not part of it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub struct Dms {
    /// `dealias(type)` (canonical, qualifiers kept).
    pub ty: QualType,
    /// The member's identifier; `None` for an unnamed bit-field.
    pub name: Option<Symbol>,
    /// `alignas(A)`, if given (a power of two).
    pub alignment: Option<u32>,
    /// The bit-field width, if given.
    pub bit_width: Option<u32>,
    /// `[[no_unique_address]]`.
    pub nua: bool,
    /// The annotations (P3394R4 `annotations`): an interned list of
    /// constant reflections ([`Oven::dms_annots`]); 0 for none.
    pub annots: u32,
}

/// Which members of `std::meta::data_member_options` hold what, found by
/// sema once per translation unit (by name: `name`, `alignment`,
/// `bit_width`, `no_unique_address`) and cached here.
#[derive(Clone, Copy, PartialEq, Eq, Debug, kiln_snap::Snap)]
pub struct OptionsPlan {
    /// The options class (canonical, unqualified).
    pub class: QualType,
    /// `optional<name-type> name` and its payload type `name-type`.
    pub name: (DeclId, QualType),
    /// `optional<int> alignment` and its payload type.
    pub alignment: (DeclId, QualType),
    /// `optional<int> bit_width` and its payload type.
    pub bit_width: (DeclId, QualType),
    /// `bool no_unique_address`.
    pub nua: DeclId,
    /// `vector<info> annotations`, when the options class has it
    /// (libstdc++ 16.1 does; a header without it reads none).
    pub annotations: Option<DeclId>,
}

/// A `data_member_options` value as read from Oven memory, before
/// validation. The name's bytes are [`Oven::dms_name_bytes`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RawOptions {
    /// An engaged `name`: whether it holds a `u8string`.
    pub name_u8: Option<bool>,
    /// An engaged `alignment`.
    pub alignment: Option<i64>,
    /// An engaged `bit_width`.
    pub bit_width: Option<i64>,
    /// `no_unique_address`.
    pub nua: bool,
    /// How many `annotations` were read ([`Oven::dms_annotation_args`]).
    pub annotations: u32,
}

/// A leaf walk: the object's type, the subobject type it stops at, and
/// whether it enters unions.
type LeafKey = (OTy, Option<OTy>, bool);

/// The interned data member descriptions of a translation unit, and
/// the reader's state.
#[derive(Debug, Default, kiln_snap::Snap)]
pub(crate) struct DmsTable {
    specs: Vec<Dms>,
    ids: FxHashMap<Dms, DmsId>,
    plan: Option<OptionsPlan>,
    /// The last name read (reused: no allocation per name).
    #[snap(skip)]
    name: Vec<u8>,
    /// `define_aggregate`'s descriptions (reused across calls).
    #[snap(skip)]
    buf: Vec<Dms>,
    /// The leaves of each `(type, stop, unions)` searched, as offsets
    /// from the object: the walk is done once per type, not per call.
    layouts: FxHashMap<LeafKey, Vec<(u64, OTy)>>,
    /// The leaves of the object being read (reused).
    #[snap(skip)]
    scratch: Vec<Leaf>,
    /// The `annotations` read last (reused).
    #[snap(skip)]
    anns: Vec<ReflectionId>,
    /// The interned annotation lists; list `k` is `ann_lists[k - 1]`.
    ann_lists: Vec<Box<[ReflectionId]>>,
    ann_ids: FxHashMap<Box<[ReflectionId]>, u32>,
}

impl DmsTable {
    pub(crate) fn footprint(&self) -> usize {
        self.specs.capacity() * size_of::<Dms>()
            + self.ids.capacity() * (size_of::<(Dms, DmsId)>() + 1)
            + self.name.capacity()
            + self.buf.capacity() * size_of::<Dms>()
            + self
                .layouts
                .values()
                .map(|v| v.capacity() * size_of::<(u64, OTy)>() + 48)
                .sum::<usize>()
            + self.scratch.capacity() * size_of::<Leaf>()
            + self.anns.capacity() * size_of::<ReflectionId>()
            + self.ann_lists.iter().map(|l| 2 * size_of_val(&**l) + 40).sum::<usize>()
            + self.ann_ids.capacity() * (size_of::<(Box<[ReflectionId]>, u32)>() + 1)
    }
}

/// How deep a `data_member_options` member is searched for its leaves
/// (`optional`'s base and union layers, `name-type`'s strings).
const MAX_DEPTH: u32 = 8;

/// The longest name read (an identifier longer than this is rejected).
const MAX_NAME: u64 = 4096;

/// One scalar (or payload) leaf of an object: its address and type.
#[derive(Clone, Copy, Debug)]
struct Leaf {
    addr: u64,
    ty: OTy,
}

fn at(raw: u64, off: u64) -> u64 {
    Ptr::with_off(raw, u64::from(Ptr::decode(raw).off) + off, false)
}

fn fault(f: Fault) -> MetaError {
    MetaError { notes: Vec::new(), kind: MetaErrorKind::Fault(OvenFault(f)) }
}

impl Oven {
    /// Interns description `d` (spec/reflection.md §2.3): equal
    /// descriptions get one id.
    pub fn dms_intern(&mut self, d: Dms) -> DmsId {
        let t = &mut self.dms;
        if let Some(&id) = t.ids.get(&d) {
            return id;
        }
        // Index 0 is `DmsId`'s reserved dummy.
        let id = DmsId::new(u32::try_from(t.specs.len() + 1).unwrap_or(u32::MAX - 1));
        t.specs.push(d);
        t.ids.insert(d, id);
        id
    }

    /// The description `id` names (`None`: not one of this Oven's).
    pub fn dms(&self, id: DmsId) -> Option<&Dms> {
        (id.index() as usize).checked_sub(1).and_then(|i| self.dms.specs.get(i))
    }

    /// The (empty) buffer `define_aggregate` collects its descriptions
    /// in; given back by [`Oven::put_dms_buf`].
    pub fn take_dms_buf(&mut self) -> Vec<Dms> {
        let mut v = std::mem::take(&mut self.dms.buf);
        v.clear();
        v
    }

    /// Gives back the buffer of [`Oven::take_dms_buf`].
    pub fn put_dms_buf(&mut self, mut v: Vec<Dms>) {
        v.clear();
        self.dms.buf = v;
    }

    /// How many descriptions are interned.
    pub fn dms_count(&self) -> usize {
        self.dms.specs.len()
    }

    /// The cached reading plan of `data_member_options`.
    pub fn dms_plan(&self) -> Option<OptionsPlan> {
        self.dms.plan
    }

    /// Caches the reading plan of `data_member_options`.
    pub fn set_dms_plan(&mut self, p: OptionsPlan) {
        self.dms.plan = Some(p);
    }

    /// The `annotations` [`Oven::meta_dms_options`] read last.
    pub fn dms_annotation_args(&self) -> &[ReflectionId] {
        &self.dms.anns
    }

    /// Interns annotation list `l` (equal lists get one id; the empty
    /// list is 0).
    pub fn dms_annots_intern(&mut self, l: &[ReflectionId]) -> u32 {
        if l.is_empty() {
            return 0;
        }
        let t = &mut self.dms;
        if let Some(&id) = t.ann_ids.get(l) {
            return id;
        }
        t.ann_lists.push(l.into());
        let id = u32::try_from(t.ann_lists.len()).unwrap_or(u32::MAX);
        t.ann_ids.insert(l.into(), id);
        id
    }

    /// The annotation list `id` ([`Dms::annots`]).
    pub fn dms_annots(&self, id: u32) -> &[ReflectionId] {
        (id as usize).checked_sub(1).and_then(|i| self.dms.ann_lists.get(i)).map_or(&[], |l| l)
    }

    /// The bytes of the name [`Oven::meta_dms_options`] read last.
    pub fn dms_name_bytes(&self) -> &[u8] {
        &self.dms.name
    }

    /// Reads the `data_member_options` object at `addr` by `plan`
    /// (spec/reflection.md §6.1): each `optional` is its first `bool`
    /// leaf (engaged) and its first leaf of the payload type (read only
    /// when engaged); a `name-type` is its first `bool` leaf (holds a
    /// `u8string`; absent: decided by the character type) and the first
    /// pointer to `char8_t` (or `char`) leaf followed by an integer leaf
    /// (the data and length of the string it holds). Union members are
    /// searched for the payload but never read unless engaged. The
    /// `annotations` vector, if the class has one, is read through its
    /// layout recipe into [`Oven::dms_annotation_args`].
    ///
    /// # Errors
    /// [`MetaError::invalid`] if the object does not have that shape; the
    /// evaluator's fault reading it (uninitialized, out of lifetime).
    pub fn meta_dms_options(
        &mut self,
        addr: u128,
        plan: &OptionsPlan,
    ) -> Result<RawOptions, MetaError> {
        let addr = addr as u64;
        let t = self.types.lookup(plan.class).ok_or_else(MetaError::invalid)?;
        let member = |s: &Self, d: DeclId| -> Result<Leaf, MetaError> {
            // `d` is one of the options class's own field declarations.
            let f = s.types.fields(t).iter().find(|f| f.decl == d && !f.base); // same_entity: raw ok
            let f = f.ok_or_else(MetaError::invalid)?;
            Ok(Leaf { addr: at(addr, f.offset), ty: f.ty })
        };
        let mut raw = RawOptions::default();
        let nua = member(self, plan.nua)?;
        raw.nua = self.load(nua.addr, nua.ty, ak::READ).map_err(fault)? != 0;
        for (which, (d, payload)) in [plan.alignment, plan.bit_width].into_iter().enumerate() {
            let opt = member(self, d)?;
            if let Some(v) = self.optional_payload(opt, payload)? {
                let v = self.load_int(v)?;
                if which == 0 {
                    raw.alignment = Some(v);
                } else {
                    raw.bit_width = Some(v);
                }
            }
        }
        self.dms.name.clear();
        let opt = member(self, plan.name.0)?;
        if let Some(v) = self.optional_payload(opt, plan.name.1)? {
            raw.name_u8 = Some(self.read_name(v)?);
        }
        self.dms.anns.clear();
        if let Some(d) = plan.annotations {
            let v = member(self, d)?;
            let rc = recipe(&self.types, v.ty, Shape::Vector).map_err(|_| MetaError::invalid())?;
            let range = self.range_arg(rc, v.addr).map_err(fault)?;
            let mut anns = std::mem::take(&mut self.dms.anns);
            let mut r = Ok(());
            for i in 0..self.meta_range_len(range) {
                match self.meta_range_at(range, i) {
                    Ok(x) => anns.push(x),
                    Err(e) => {
                        r = Err(e);
                        break;
                    }
                }
            }
            raw.annotations = u32::try_from(anns.len()).unwrap_or(u32::MAX);
            self.dms.anns = anns;
            r?;
        }
        Ok(raw)
    }

    /// The payload leaf of the engaged `optional` at `opt`; `None` when
    /// it is disengaged.
    fn optional_payload(
        &mut self,
        opt: Leaf,
        payload: QualType,
    ) -> Result<Option<Leaf>, MetaError> {
        let p = self.types.lookup(payload).ok_or_else(MetaError::invalid)?;
        let mut leaves = std::mem::take(&mut self.dms.scratch);
        self.leaves_of(opt, Some(p), true, &mut leaves);
        let engaged = leaves.iter().find(|l| self.types.kind(l.ty) == TyKind::Bool).copied();
        let value = leaves.iter().find(|l| l.ty == p).copied();
        self.dms.scratch = leaves;
        let (Some(e), Some(v)) = (engaged, value) else { return Err(MetaError::invalid()) };
        Ok((self.load(e.addr, e.ty, ak::READ).map_err(fault)? != 0).then_some(v))
    }

    /// Reads the string of the `name-type` object at `v` into the name
    /// buffer; whether it is a `u8string`.
    fn read_name(&mut self, v: Leaf) -> Result<bool, MetaError> {
        let mut leaves = std::mem::take(&mut self.dms.scratch);
        self.leaves_of(v, None, false, &mut leaves);
        let char8 = |s: &Self, e: OTy| s.types.info(e).q == QualType::CHAR8;
        let flag = leaves.iter().find(|l| self.types.kind(l.ty) == TyKind::Bool).copied();
        let flag = match flag {
            Some(f) => Some(self.load(f.addr, f.ty, ak::READ).map_err(fault)? != 0),
            None => None,
        };
        let string = leaves.windows(2).find_map(|w| {
            let TyKind::Ptr(e) = self.types.kind(w[0].ty) else { return None };
            let narrow = matches!(self.types.kind(e), TyKind::Int { width: 8, .. });
            let len = matches!(self.types.kind(w[1].ty), TyKind::Int { .. });
            (narrow && len && flag.is_none_or(|u8| u8 == char8(self, e))).then_some((w[0], w[1], e))
        });
        self.dms.scratch = leaves;
        let Some((p, n, e)) = string else { return Err(MetaError::invalid()) };
        let u8 = char8(self, e);
        let n = self.load(n.addr, n.ty, ak::READ).map_err(fault)?;
        let n = u64::try_from(n).unwrap_or(u64::MAX).min(MAX_NAME + 1);
        let mut q = self.load(p.addr, p.ty, ak::READ).map_err(fault)? as u64;
        let mut name = std::mem::take(&mut self.dms.name);
        name.clear();
        // Fast path: an initialized run of plain bytes, copied at once.
        if self.types.size(e) == 1
            && let Some(run) = self.byte_run(q, e, n)
            && run.len() as u64 == n
        {
            name.extend_from_slice(run);
            self.dms.name = name;
            return Ok(u8);
        }
        let mut r = Ok(u8);
        for i in 0..n {
            match self.load(q, e, ak::READ) {
                Ok(c) => name.push(c as u8),
                Err(f) => {
                    r = Err(fault(f));
                    break;
                }
            }
            if i + 1 < n {
                match self.ptr_add(q, 1, e) {
                    Ok(x) => q = x,
                    Err(f) => {
                        r = Err(fault(f));
                        break;
                    }
                }
            }
        }
        self.dms.name = name;
        r
    }

    /// [`Oven::leaves`] of `l` into `out`, through the per-type cache.
    fn leaves_of(&mut self, l: Leaf, stop: Option<OTy>, unions: bool, out: &mut Vec<Leaf>) {
        out.clear();
        let key = (l.ty, stop, unions);
        if let Some(rel) = self.dms.layouts.get(&key) {
            out.extend(rel.iter().map(|&(off, ty)| Leaf { addr: at(l.addr, off), ty }));
            return;
        }
        self.leaves(l, stop, unions, 0, out);
        let base = Ptr::decode(l.addr).off;
        let rel =
            out.iter().map(|x| (u64::from(Ptr::decode(x.addr).off.saturating_sub(base)), x.ty));
        let rel = rel.collect();
        self.dms.layouts.insert(key, rel);
    }

    /// The leaves of the object `l` in offset order: scalars, and (when
    /// `stop` is given) whole subobjects of type `stop`. Union members
    /// are entered only when `unions` (they are not read here).
    fn leaves(&self, l: Leaf, stop: Option<OTy>, unions: bool, depth: u32, out: &mut Vec<Leaf>) {
        if Some(l.ty) == stop {
            out.push(l);
            return;
        }
        match self.types.kind(l.ty) {
            TyKind::Record { union, .. } if depth < MAX_DEPTH && (unions || !union) => {
                for f in self.types.fields(l.ty) {
                    if f.width.is_some() {
                        continue;
                    }
                    let c = Leaf { addr: at(l.addr, f.offset), ty: f.ty };
                    self.leaves(c, stop, unions, depth + 1, out);
                }
            }
            TyKind::Bool | TyKind::Int { .. } | TyKind::Ptr(_) => out.push(l),
            _ => {}
        }
    }

    /// The integer leaf `l`, sign-extended.
    fn load_int(&self, l: Leaf) -> Result<i64, MetaError> {
        let v = self.load(l.addr, l.ty, ak::READ).map_err(fault)?;
        let TyKind::Int { width, signed } = self.types.kind(l.ty) else {
            return Err(MetaError::invalid());
        };
        let w = u32::from(width).clamp(1, 128);
        let v = if signed && w < 128 {
            let sh = 128 - w;
            ((v << sh) as i128) >> sh
        } else {
            v as i128
        };
        Ok(i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX }))
    }
}
