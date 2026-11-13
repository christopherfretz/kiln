//! Dynamic types (WP-077; spec/oven.md "Dynamic types"): the vptr slot of
//! each polymorphic subobject holds a *dynamic-type word* — the class `C`
//! whose constructor or destructor last ran on the object (its Oven type,
//! low 30 bits, with the [`REL`] and [`CMP`] flags above it) and the
//! subobject's offset in `C` (high 32 bits). Written
//! by `Op::SetVptr` ([class.cdtor]/4: the dynamic type of an object under
//! construction or destruction is the class of the running constructor or
//! destructor), read by virtual calls, `dynamic_cast` and `typeid`. The
//! words are not part of a frozen value: a thawed object gets them back
//! from its type (`Oven::thaw_vptrs`).

use kiln_ast::class::SubobjectId;
use kiln_ast::decl::{Access, DeclKind, TagKind};
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_source::fx::{FxHashMap, FxHashSet};

use super::Fault;
use crate::Oven;
use crate::SemaBridge;
use crate::kbc::ak;
use crate::mem::{MemFault, Origin, Ptr};
use crate::ty::{OTy, TyKind};

/// One subobject of a class (indexed by [`SubobjectId`]).
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
pub(crate) struct Sub {
    /// Byte offset in the complete object.
    pub off: u64,
    pub cls: OTy,
    /// The class (first declaration).
    pub decl: DeclId,
    /// A virtual base or one of its subobjects (offset relative to a
    /// complete object of the class only).
    pub virt: bool,
    /// A virtual base class subobject itself.
    pub vbase: bool,
    /// The class is polymorphic (has a vptr slot at `off`).
    pub poly: bool,
    /// The subobject this one is a direct base of (itself for 0).
    pub parent: u16,
    /// That base-specifier grants public access ([class.access.base]/1).
    pub public: bool,
    /// With `virt`: the virtual base it is (in), its nearest virtual base
    /// ancestor or itself.
    pub top: u16,
}

/// The subobject graph of a class with offsets.
#[derive(Debug, kiln_snap::Snap)]
pub(crate) struct PolyInfo {
    /// The class (first declaration; `None` for a non-class).
    pub decl: Option<DeclId>,
    pub subs: Vec<Sub>,
    /// Distinct offsets of the vptr slots of the non-virtual polymorphic
    /// subobjects (`SetVptr` writes them at the object's address plus
    /// the offset).
    pub vptrs: Vec<u64>,
    /// The polymorphic subobjects in or below a virtual base, one per
    /// distinct vptr slot (indices into `subs`): their slots are at the
    /// complete object's offsets only in a complete object.
    pub vvptrs: Vec<u16>,
    /// The offset of each virtual base (by class) in a complete object.
    pub vbase_at: FxHashMap<DeclId, u64>,
    /// The (offset, class) of each subobject, for [`Oven::base_owner`].
    pub at: FxHashSet<(u64, OTy)>,
}

/// A resolved virtual call.
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
struct Target {
    func: u32,
    /// The overrider's subobject offset minus the called subobject's.
    delta: i64,
    ov: DeclId,
    pure: bool,
    /// A covariant return ([class.virtual]/8), [`cov_word`] encoded.
    cov: u128,
    /// The overrider's subobject (its index in the dynamic type's graph)
    /// is (in) a virtual base: `delta` holds only in a complete object,
    /// so the address is found through the most derived object.
    to: Option<u16>,
}

/// The key of [`Virt::rel_dispatch`].
type RelKey = (OTy, i64, DeclId, OTy, u32, u64);

/// A resolved call through a [`REL`] word.
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
struct RelTarget {
    t: Target,
    /// The overrider's subobject (index in the running class's graph).
    k: u16,
}

/// A covariant return adjustment ([class.virtual]/8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CovAdj {
    /// Add this byte offset to the returned pointer.
    Off(i64),
    /// Convert the returned pointer to class `.0` to its (virtual)
    /// subobject `.1` (as `Op::VBase`).
    VBase(OTy, u16),
}

/// The register word of a [`CovAdj`]: an offset sign-extended (high half
/// 0 or all ones), a virtual-base conversion with high half 1.
pub(crate) fn cov_word(c: CovAdj) -> u128 {
    match c {
        CovAdj::Off(o) => o as u128,
        CovAdj::VBase(t, k) => (1u128 << 64) | (u128::from(t.0) << 16) | u128::from(k),
    }
}

/// Decodes [`cov_word`].
pub(crate) fn cov_adj(w: u128) -> CovAdj {
    if (w >> 64) as u64 == 1 {
        CovAdj::VBase(OTy((w >> 16) as u32), w as u16)
    } else {
        CovAdj::Off(w as i64)
    }
}

/// The Oven's per-class caches (insert-only: the class graph of a
/// complete class never changes).
#[derive(Debug, Default, kiln_snap::Snap)]
pub(crate) struct Virt {
    map: FxHashMap<OTy, u32>,
    infos: Vec<PolyInfo>,
    dispatch: FxHashMap<(OTy, u64, DeclId), Target>,
    /// The calls through [`REL`] words: keyed by the running class, the
    /// slot's distance from its start, the method, the called class, the
    /// most derived object's [`PolyInfo`] and the running class's offset
    /// in it, which together fix the subobject called (WP-414).
    rel_dispatch: FxHashMap<RelKey, RelTarget>,
    /// The `std::type_info` object of each type `typeid` named (by its
    /// canonical type): a static allocation, so `&typeid(T)` is one
    /// address for the translation unit.
    type_infos: FxHashMap<u32, u64>,
    /// The object of each label address (`&&l`) evaluated, by its
    /// `AddrLabelExpr`: an unbacked static allocation, so one expression
    /// is one address and two are two (clang 22).
    labels: FxHashMap<ExprId, u64>,
}

impl Virt {
    /// The subobject graph of the class with [`PolyInfo`] index `i`.
    pub(crate) fn subs(&self, i: usize) -> &[Sub] {
        &self.infos[i].subs
    }

    pub fn footprint(&self) -> usize {
        self.map.capacity() * 12
            + self
                .infos
                .iter()
                .map(|i| {
                    i.subs.capacity() * 48
                        + i.vptrs.capacity() * 8
                        + i.vvptrs.capacity() * 2
                        + i.vbase_at.capacity() * 16
                        + i.at.capacity() * 16
                })
                .sum::<usize>()
            + self.dispatch.capacity() * 48
            + self.rel_dispatch.capacity() * 96
            + self.type_infos.capacity() * 12
            + self.labels.capacity() * 12
    }
}

/// Encodes a dynamic-type word.
pub(crate) fn word(cls: OTy, off: u64) -> u128 {
    u128::from(cls.0) | u128::from(off) << 32
}

/// The flag of a dynamic-type word written by a complete-object
/// constructor or destructor of its class (or by thawing a complete
/// object): the object of that class is the most derived object, so its
/// virtual bases are at their complete-object offsets and a virtual call
/// needs no walk of the object model to find one (WP-414). Oven types
/// stay below 2^30 (each takes far more than a byte of the evaluation's
/// memory budget).
const CMP: u32 = 1 << 30;

/// Encodes a complete object's dynamic-type word ([`CMP`]).
fn cmp_word(cls: OTy, off: u64) -> u128 {
    word(cls, off) | u128::from(CMP)
}

/// The flag of a dynamic-type word whose offset is relative to this
/// object (P3533R2): the slot of a subobject in a virtual base of a
/// class `C` whose base-object constructor or destructor is running, so
/// `C` is a base class subobject and the virtual base is the most
/// derived object's, at no fixed offset in `C`. The high word is then
/// the signed distance from `C`'s start to the slot.
const REL: u32 = 1 << 31;

/// Encodes a relative dynamic-type word ([`REL`]).
fn rel_word(cls: OTy, delta: i64) -> u128 {
    u128::from(cls.0 | REL) | u128::from(delta as i32 as u32) << 32
}

/// A decoded dynamic-type word.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Dyn {
    /// The class whose constructor or destructor last ran (or the most
    /// derived class).
    pub cls: OTy,
    /// The slot's offset in it; with `rel`, the signed distance from the
    /// start of that class's object to the slot.
    pub off: i64,
    pub rel: bool,
    /// [`CMP`]: the object of class `cls` is a complete object.
    pub complete: bool,
}

/// The record type of class declaration `d`.
pub(crate) fn class_qtype(ast: &Ast, d: DeclId) -> Option<QualType> {
    ast.decls.get(ast.definition(d).unwrap_or(d)).ty()
}

impl Oven {
    /// The index of class `cls`'s [`PolyInfo`] (computed on first use).
    pub(crate) fn poly_info(&mut self, b: &mut dyn SemaBridge, cls: OTy) -> u32 {
        let cls = self.types.class_of(cls);
        if let Some(&i) = self.virt.map.get(&cls) {
            return i;
        }
        let TyKind::Record { decl, .. } = self.types.kind(cls) else {
            let i = self.virt.infos.len() as u32;
            self.virt.infos.push(PolyInfo {
                decl: None,
                subs: Vec::new(),
                vptrs: Vec::new(),
                vvptrs: Vec::new(),
                vbase_at: FxHashMap::default(),
                at: FxHashSet::default(),
            });
            self.virt.map.insert(cls, i);
            return i;
        };
        let first = b.ast().first(decl);
        let graph: Vec<kiln_ast::class::Subobject> = b.subobjects(first).to_vec();
        let mut subs: Vec<Sub> = Vec::with_capacity(graph.len());
        for (i, s) in graph.iter().enumerate() {
            let ast = b.ast();
            let q = class_qtype(ast, s.class).unwrap_or(QualType::INT);
            let poly = ast
                .definition(s.class)
                .and_then(|d| ast.decls.record(d))
                .is_some_and(|r| r.defn.contains(kiln_ast::decl::DefinitionBits::POLYMORPHIC));
            let cty = self.types.intern(b, q);
            let (off, virt) = if i == 0 {
                (0, false)
            } else if s.is_virtual {
                let def = b.ast().definition(first).unwrap_or(first);
                let o = b.ensure_layout(def).and_then(|l| l.vbases.iter().find(|v| v.0 == s.class));
                (o.map_or(0, |v| v.1), true)
            } else {
                let parent = subs[s.parent.index()];
                let pdef = b.ast().definition(parent.decl).unwrap_or(parent.decl);
                let l = b.ensure_layout(pdef);
                let o = l.and_then(|l| l.bases.iter().find(|v| v.0 == s.class)).map_or(0, |v| v.1);
                (parent.off + o, parent.virt)
            };
            let public = i == 0 || {
                let ast = b.ast();
                let pd = graph[s.parent.index()].class;
                let pdef = ast.definition(pd).unwrap_or(pd);
                let written = ast
                    .decls
                    .bases(pdef, &ast.lists)
                    .nth(usize::from(s.base_index))
                    .map_or(Access::Public, |bs| bs.access);
                match written {
                    Access::Public => true,
                    Access::None => ast.decls.record(pdef).is_none_or(|r| r.tag != TagKind::Class),
                    _ => false,
                }
            };
            let top = if s.is_virtual {
                i as u16
            } else if virt {
                subs[s.parent.index()].top
            } else {
                0
            };
            subs.push(Sub {
                off,
                cls: cty,
                decl: s.class,
                virt,
                vbase: s.is_virtual,
                poly,
                parent: s.parent.index() as u16,
                public,
                top,
            });
        }
        let mut vptrs: Vec<u64> =
            subs.iter().filter(|s| s.poly && !s.virt).map(|s| s.off).collect();
        vptrs.sort_unstable();
        vptrs.dedup();
        let mut vvptrs: Vec<u16> = Vec::new();
        for (k, s) in subs.iter().enumerate() {
            if s.poly && s.virt && !vvptrs.iter().any(|&j| subs[usize::from(j)].off == s.off) {
                vvptrs.push(k as u16);
            }
        }
        let vbase_at = subs.iter().filter(|s| s.vbase).map(|s| (s.decl, s.off)).collect();
        let i = self.virt.infos.len() as u32;
        let at = subs.iter().map(|s| (s.off, s.cls)).collect();
        self.virt.infos.push(PolyInfo { decl: Some(first), subs, vptrs, vvptrs, vbase_at, at });
        self.virt.map.insert(cls, i);
        i
    }

    /// `Op::SetVptr`: the object at `raw` (of class `cls`) has dynamic
    /// type `cls`. With `base_obj` it is a base class subobject (a
    /// base-object constructor or destructor is running): the slots of
    /// its virtual bases are the most derived object's, and get
    /// [`REL`] words.
    pub(crate) fn set_vptrs(
        &mut self,
        b: &mut dyn SemaBridge,
        raw: u64,
        cls: OTy,
        base_obj: bool,
    ) -> Result<(), Fault> {
        let i = self.poly_info(b, cls);
        let fault = |f| Fault::Mem { f, ak: ak::INIT, ty: cls, p: raw };
        let start = u64::from(Ptr::decode(raw).off);
        let n = self.virt.infos[i as usize].vptrs.len();
        for k in 0..n {
            let off = self.virt.infos[i as usize].vptrs[k];
            let p = Ptr::with_off(raw, start + off, false);
            let w = if base_obj { word(cls, off) } else { cmp_word(cls, off) };
            self.write_vptr(p, w).map_err(fault)?;
        }
        let n = self.virt.infos[i as usize].vvptrs.len();
        if n == 0 {
            return Ok(());
        }
        let owner =
            if base_obj { Some(self.owner(b, raw, cls).ok_or(Fault::Invalid)?) } else { None };
        for k in 0..n {
            let j = self.virt.infos[i as usize].vvptrs[k];
            let off = self.virt.infos[i as usize].subs[usize::from(j)].off;
            let Some((m, at)) = owner else {
                let p = Ptr::with_off(raw, start + off, false);
                self.write_vptr(p, cmp_word(cls, off)).map_err(fault)?;
                continue;
            };
            let p = self.sub_in(raw, i as usize, j, m, at).ok_or(Fault::Invalid)?;
            let delta = i64::from(Ptr::decode(p).off) - start as i64;
            self.write_vptr(p, rel_word(cls, delta)).map_err(fault)?;
        }
        Ok(())
    }

    fn write_vptr(&mut self, p: u64, w: u128) -> Result<(), MemFault> {
        let (sp, abs, _) = self.mem.check(p, 8, true)?;
        self.mem.trial_write(p, 8)?;
        let s = self.mem.space_mut(sp);
        s.write(abs, 8, w);
        s.set_init(abs, 8, true);
        s.set_ptr(abs, 8, false);
        Ok(())
    }

    /// Writes the dynamic-type words of every complete object of class type
    /// in the object of type `t` at absolute address `abs` of space `sp`
    /// (a thawed value: each such object has its own type as its dynamic
    /// type).
    pub(crate) fn thaw_vptrs(
        &mut self,
        b: &mut dyn SemaBridge,
        sp: crate::mem::Sp,
        abs: u64,
        t: OTy,
    ) {
        if !self.types.info(t).has_vptr {
            return;
        }
        match self.types.kind(t) {
            TyKind::Array { elem, n } => {
                let es = self.types.size(elem);
                for i in 0..n {
                    self.thaw_vptrs(b, sp, abs + i * es, elem);
                }
            }
            // A union's members are not all alive (no polymorphic member
            // of a union is tracked).
            TyKind::Record { union: true, .. } => {}
            TyKind::Record { .. } => {
                let i = self.poly_info(b, t) as usize;
                let info = &self.virt.infos[i];
                let offs: Vec<u64> = info
                    .vptrs
                    .iter()
                    .copied()
                    .chain(info.vvptrs.iter().map(|&j| info.subs[usize::from(j)].off))
                    .collect();
                for off in offs {
                    let s = self.mem.space_mut(sp);
                    s.write(abs + off, 8, cmp_word(t, off));
                    s.set_init(abs + off, 8, true);
                    s.set_ptr(abs + off, 8, false);
                }
                // Members (of this class and of its bases) are complete
                // objects of their own.
                self.thaw_member_vptrs(b, sp, abs, t);
            }
            _ => {}
        }
    }

    fn thaw_member_vptrs(&mut self, b: &mut dyn SemaBridge, sp: crate::mem::Sp, abs: u64, t: OTy) {
        let fields = self.types.fields(t).to_vec();
        for f in fields {
            if f.width.is_some() || !self.types.info(f.ty).has_vptr {
                continue;
            }
            if f.base {
                self.thaw_member_vptrs(b, sp, abs + f.offset, f.ty);
            } else {
                self.thaw_vptrs(b, sp, abs + f.offset, f.ty);
            }
        }
    }

    /// Reads the dynamic-type word of the polymorphic subobject at `raw`
    /// (of class `cls`) for an access of kind `k`: the most-derived class
    /// and the subobject's offset in it. An object of unknown dynamic type
    /// (not usable in constant expressions) or one whose word is not set
    /// (its construction has not reached it: CWG1517) fails.
    pub(crate) fn read_vptr(&self, ast: &Ast, raw: u64, cls: OTy, k: u32) -> Result<Dyn, Fault> {
        let (sp, abs, _) = self.mem.check(raw, 8, false).map_err(|f| match f {
            MemFault::External(..) | MemFault::Unknown(..) | MemFault::Temporary(..) => {
                Fault::UnknownDynamic { ak: k, p: raw, ty: cls }
            }
            f => Fault::Mem { f, ak: k, ty: cls, p: raw },
        })?;
        let s = self.mem.space(sp);
        if !s.is_init(abs, 8) {
            return Err(self.no_dynamic(ast, raw, cls, k));
        }
        let w = s.read(abs, 8) as u64;
        let lo = w as u32;
        let rel = lo & REL != 0;
        let off = if rel { i64::from((w >> 32) as u32 as i32) } else { (w >> 32) as i64 };
        Ok(Dyn { cls: OTy(lo & !(REL | CMP)), off, rel, complete: lo & CMP != 0 })
    }

    /// The fault of a polymorphic operation (access kind `k`) on the
    /// object of class `cls` at `raw` that has no dynamic type yet (or
    /// any more). clang: an object whose constructor is running (in its
    /// bases phase) is CWG1517's "subexpression not valid"; one whose
    /// construction has not started is outside its lifetime.
    fn no_dynamic(&self, ast: &Ast, raw: u64, cls: OTy, k: u32) -> Fault {
        let constructing = self.stack[self.run.entry + 1..].iter().any(|fr| {
            let f = &self.funcs[fr.func as usize];
            f.this_ty == Some(cls)
                && f.this_reg.is_some_and(|r| self.regs[(fr.fb + r) as usize] as u64 == raw)
                && f.decl.is_some_and(|d| ast.decls.get(d).kind() == DeclKind::CXXConstructor)
        });
        if constructing { Fault::Invalid } else { Fault::NoDynamic(k) }
    }

    /// The subobject of the dynamic type `d.cls` whose class is `cls`
    /// and whose slot the word `d` (read at `raw`) is in: the subobject
    /// `raw` designates, viewed as part of `d.cls`. A [`REL`] word's is
    /// the virtual one found at `raw` through the most derived object.
    pub(crate) fn locate(
        &mut self,
        b: &mut dyn SemaBridge,
        d: Dyn,
        cls: OTy,
        raw: u64,
    ) -> Option<(u32, u16)> {
        let i = self.poly_info(b, d.cls);
        let subs = &self.virt.infos[i as usize].subs;
        if !d.rel {
            let off = u64::try_from(d.off).ok()?;
            let at = |virt: bool| {
                subs.iter().position(|s| s.virt == virt && s.off == off && s.cls == cls)
            };
            let k = at(false).or_else(|| at(true))?;
            return Some((i, k as u16));
        }
        let start = self.rel_start(raw, d);
        let (m, at) = self.owner(b, start, d.cls)?;
        self.locate_rel(i as usize, cls, raw, m, at).map(|k| (i, k))
    }

    /// The virtual subobject of class `cls` of the class with info `i`
    /// that is at `raw`, its most derived object having info `m` and
    /// being at `at` (one object-model walk for all the candidates).
    fn locate_rel(&self, i: usize, cls: OTy, raw: u64, m: usize, at: u64) -> Option<u16> {
        let subs = &self.virt.infos[i].subs;
        (0..subs.len()).find_map(|j| {
            let j16 = u16::try_from(j).ok()?;
            (subs[j].virt && subs[j].cls == cls && self.sub_in(raw, i, j16, m, at) == Some(raw))
                .then_some(j16)
        })
    }

    /// The start of the object of class `d.cls` whose [`REL`] word is at
    /// `raw`.
    fn rel_start(&self, raw: u64, d: Dyn) -> u64 {
        let off = i64::from(Ptr::decode(raw).off) - d.off;
        Ptr::with_off(raw, off as u64, false)
    }

    /// The address of subobject `k` of the object of class `cls` at
    /// `raw`: a virtual base (or a subobject of one) is the most derived
    /// object's ([class.mi]/4), located in the object model
    /// (`base_owner`) through the nearest virtual base above it. `None`
    /// when it cannot be: a pointer past the end, or no object of `cls`
    /// at `raw`.
    pub(crate) fn sub_addr(
        &mut self,
        b: &mut dyn SemaBridge,
        raw: u64,
        cls: OTy,
        k: u16,
    ) -> Option<u64> {
        let i = self.poly_info(b, cls) as usize;
        let s = *self.virt.infos[i].subs.get(usize::from(k))?;
        if !s.virt {
            return Some(Ptr::with_off(raw, u64::from(Ptr::decode(raw).off) + s.off, false));
        }
        let (m, at) = self.owner(b, raw, cls)?;
        self.sub_in(raw, i, k, m, at)
    }

    /// The most derived object of the `cls` object at `raw`: its
    /// [`PolyInfo`] index and offset in the allocation.
    fn owner(&mut self, b: &mut dyn SemaBridge, raw: u64, cls: OTy) -> Option<(usize, u64)> {
        let p = Ptr::decode(raw);
        if p.past {
            return None;
        }
        let root = self.alloc_any(p)?.ty;
        let (md, at) = self.base_owner(b, (root, raw), u64::from(p.off), 0, true, cls)?;
        Some((self.poly_info(b, md) as usize, at))
    }

    /// The address of the virtual subobject `k` of the class with info
    /// `i` whose most derived object has info `m` and is at `at`.
    fn sub_in(&self, raw: u64, i: usize, k: u16, m: usize, at: u64) -> Option<u64> {
        let subs = &self.virt.infos[i].subs;
        let s = subs[usize::from(k)];
        let w = subs[usize::from(s.top)];
        let wo = *self.virt.infos[m].vbase_at.get(&w.decl)?;
        Some(Ptr::with_off(raw, at + wo + (s.off - w.off), false))
    }

    /// `Op::VCall`: the function and adjusted `this` of a virtual call of
    /// `method` (a member of class `cls`) on the object at `raw`.
    pub(crate) fn vcall(
        &mut self,
        b: &mut dyn SemaBridge,
        method: DeclId,
        cls: OTy,
        raw: u64,
    ) -> Result<(u32, u64, u128), Fault> {
        let d = self.read_vptr(b.ast(), raw, cls, ak::MEMBER_CALL)?;
        if d.rel {
            return self.vcall_rel(b, method, cls, d, raw);
        }
        let key = (d.cls, d.off as u64, method);
        let t = match self.virt.dispatch.get(&key) {
            Some(&t) => t,
            None => {
                let t = self.resolve(b, method, cls, d, raw)?;
                self.virt.dispatch.insert(key, t);
                t
            }
        };
        if t.pure {
            return Err(Fault::PureVirtual(t.ov));
        }
        let this = match t.to {
            // In a complete object the virtual base is at its
            // complete-object offset (`delta` holds).
            Some(k) if !d.complete => {
                self.sub_addr(b, self.rel_start(raw, d), d.cls, k).ok_or(Fault::Invalid)?
            }
            _ => Ptr::with_off(raw, (i64::from(Ptr::decode(raw).off) + t.delta) as u64, false),
        };
        Ok((t.func, this, t.cov))
    }

    /// [`Self::vcall`] through a [`REL`] word: one walk of the object
    /// model finds the most derived object, then the call is cached by
    /// the running class's position in it.
    fn vcall_rel(
        &mut self,
        b: &mut dyn SemaBridge,
        method: DeclId,
        cls: OTy,
        d: Dyn,
        raw: u64,
    ) -> Result<(u32, u64, u128), Fault> {
        let start = self.rel_start(raw, d);
        let (m, at) = self.owner(b, start, d.cls).ok_or(Fault::Invalid)?;
        let pos = u64::from(Ptr::decode(start).off).wrapping_sub(at);
        let key = (d.cls, d.off, method, cls, m as u32, pos);
        let i = self.poly_info(b, d.cls) as usize;
        let rt = match self.virt.rel_dispatch.get(&key) {
            Some(&rt) => rt,
            None => {
                let at_k = self.locate_rel(i, cls, raw, m, at).ok_or(Fault::Invalid)?;
                let t = self.resolve_at(b, method, i as u32, at_k, d)?;
                let k = t.to.ok_or(Fault::Invalid)?;
                let rt = RelTarget { t, k };
                self.virt.rel_dispatch.insert(key, rt);
                rt
            }
        };
        if rt.t.pure {
            return Err(Fault::PureVirtual(rt.t.ov));
        }
        let s = self.virt.infos[i].subs[usize::from(rt.k)];
        let this = if s.virt {
            self.sub_in(start, i, rt.k, m, at).ok_or(Fault::Invalid)?
        } else {
            Ptr::with_off(start, u64::from(Ptr::decode(start).off) + s.off, false)
        };
        Ok((rt.t.func, this, rt.t.cov))
    }

    /// The final overrider of `method` for the subobject (of class `cls`)
    /// whose dynamic-type word `d` is at `raw`.
    fn resolve(
        &mut self,
        b: &mut dyn SemaBridge,
        method: DeclId,
        cls: OTy,
        d: Dyn,
        raw: u64,
    ) -> Result<Target, Fault> {
        let (i, at) = self.locate(b, d, cls, raw).ok_or(Fault::Invalid)?;
        self.resolve_at(b, method, i, at, d)
    }

    /// [`Self::resolve`] of the subobject `at` of the dynamic type with
    /// info `i`.
    fn resolve_at(
        &mut self,
        b: &mut dyn SemaBridge,
        method: DeclId,
        i: u32,
        at: u16,
        d: Dyn,
    ) -> Result<Target, Fault> {
        let cdecl = self.virt.infos[i as usize].decl.ok_or(Fault::Invalid)?;
        let (ov, s2) = b.final_overrider(cdecl, SubobjectId(at), method).ok_or(Fault::Invalid)?;
        let subs = &self.virt.infos[i as usize].subs;
        let to = subs.get(s2.index()).copied().ok_or(Fault::Invalid)?;
        let from = subs[usize::from(at)];
        let ast = b.ast();
        let pure = ast.decls.function(ast.first(ov)).is_some_and(|f| f.spec.is_pure());
        let func = self.func_index(ast, ov);
        let cov = self.covariance(b, method, ov).ok_or(Fault::Invalid)?;
        let delta = to.off as i64 - from.off as i64;
        let to = (to.virt || d.rel).then_some(s2.index() as u16);
        Ok(Target { func, delta, ov, pure, cov, to })
    }

    /// The return adjustment of a call of `method` that runs `ov`
    /// ([`cov_word`]): the offset of the class `method` returns a pointer
    /// or reference to in the class `ov` returns one to (0 if they are
    /// the same), or, when that class is (in) a virtual base, its
    /// subobject in `ov`'s class, found through the returned object's
    /// most derived object at `Op::CovAdj` (clang's
    /// `HandleCovariantReturnAdjustment` cast path).
    fn covariance(&mut self, b: &mut dyn SemaBridge, method: DeclId, ov: DeclId) -> Option<u128> {
        let ret_class = |b: &mut dyn SemaBridge, f: DeclId| {
            let ast = b.ast();
            let def = ast.definition(f).unwrap_or(f);
            let fty = ast.decls.get(def).ty()?;
            let kiln_ast::types::TypeView::Function(p) = ast.types.canonical_view(fty) else {
                return None;
            };
            let r = ast.types.pointee(p.ret())?;
            ast.types.as_record(r).map(|_| r)
        };
        let (Some(rm), Some(ro)) = (ret_class(b, method), ret_class(b, ov)) else {
            return Some(0);
        };
        let (tm, to) = (self.types.intern(b, rm), self.types.intern(b, ro));
        if tm == to {
            return Some(0);
        }
        let i = self.poly_info(b, to);
        let subs = &self.virt.infos[i as usize].subs;
        match subs.iter().position(|s| s.cls == tm) {
            Some(k) if subs[k].virt => Some(cov_word(CovAdj::VBase(to, u16::try_from(k).ok()?))),
            k => Some(cov_word(CovAdj::Off(k.map_or(0, |k| subs[k].off as i64)))),
        }
    }

    /// `Op::DynCast`: `dynamic_cast` of the object at `raw` (static class
    /// `stat`) to class `target` (`None`: `void*`, the most derived
    /// object), clang's `HandleDynamicCast` ([expr.dynamic.cast]/7-9). A
    /// failed pointer cast is null (0); a failed reference cast is a
    /// [`Fault::DynCast`] (it would throw `std::bad_cast`).
    pub(crate) fn dyn_cast(
        &mut self,
        b: &mut dyn SemaBridge,
        raw: u64,
        stat: OTy,
        target: Option<OTy>,
        glvalue: bool,
    ) -> Result<u64, Fault> {
        if raw == 0 && !glvalue {
            return Ok(0);
        }
        let d = self.read_vptr(b.ast(), raw, stat, ak::DYNAMIC_CAST)?;
        let (c, start) = (d.cls, self.rel_start(raw, d));
        let Some(t) = target else { return Ok(start) };
        let (i, k) = self.locate(b, d, stat, raw).ok_or(Fault::Invalid)?;
        let j = self.cast_target(i, k, c, t, glvalue);
        let j = match j {
            Ok(j) => j,
            Err(None) => return Ok(0),
            Err(Some(f)) => return Err(f),
        };
        if self.virt.infos[i as usize].subs[j].virt {
            return self.sub_addr(b, start, c, j as u16).ok_or(Fault::Invalid);
        }
        let off = self.virt.infos[i as usize].subs[j].off;
        Ok(Ptr::with_off(start, u64::from(Ptr::decode(start).off) + off, false))
    }

    /// The subobject (index) of the dynamic type `c` (info `i`) that the
    /// `dynamic_cast` of its subobject `k` to class `t` designates;
    /// `Err(None)` for a null result, `Err(Some(_))` for a fault.
    fn cast_target(
        &self,
        i: u32,
        k: u16,
        c: OTy,
        t: OTy,
        glvalue: bool,
    ) -> Result<usize, Option<Fault>> {
        let subs = &self.virt.infos[i as usize].subs;
        let stat = subs[usize::from(k)].cls;
        let fail =
            |kind: u8| Err(glvalue.then_some(Fault::DynCast { kind, stat, dynamic: c, target: t }));
        // Phase 1: from the operand's subobject up towards the most
        // derived object, across public edges only.
        let mut cur = usize::from(k);
        loop {
            let s = subs[cur];
            if s.cls == t {
                return Ok(cur);
            }
            if cur == 0 {
                break;
            }
            if !s.public {
                let has = subs.iter().any(|s| s.cls == t);
                return fail(u8::from(!has));
            }
            cur = usize::from(s.parent);
        }
        // Phase 2: an unambiguous public base of the dynamic type.
        let mut found = subs.iter().enumerate().filter(|(_, s)| s.cls == t);
        let Some((j, _)) = found.next() else { return fail(1) };
        if found.next().is_some() {
            return fail(2);
        }
        let mut p = j;
        while p != 0 {
            if !subs[p].public {
                return fail(3);
            }
            p = usize::from(subs[p].parent);
        }
        Ok(j)
    }

    /// The offset of the unambiguous public base class `base` in a
    /// complete `cls` object, and whether it is (in) a virtual base
    /// ([except.handle]/3.2, [conv.ptr]/3).
    pub(crate) fn public_base(
        &mut self,
        b: &mut dyn SemaBridge,
        cls: OTy,
        base: OTy,
    ) -> Option<(u64, bool)> {
        let rec = |t| matches!(self.types.kind(t), TyKind::Record { union: false, .. });
        if !rec(cls) || !rec(base) {
            return None;
        }
        let i = self.poly_info(b, cls) as usize;
        let subs = &self.virt.infos[i].subs;
        let mut found = subs.iter().enumerate().filter(|(_, s)| s.cls == base);
        let (mut j, s) = found.next()?;
        if found.next().is_some() {
            return None;
        }
        while j != 0 {
            if !subs[j].public {
                return None;
            }
            j = usize::from(subs[j].parent);
        }
        Some((s.off, s.virt))
    }

    /// [conv.ptr]/3 of the non-null pointer `raw` to class `cls` into a
    /// pointer to its unambiguous base `base` that is (in) a virtual base
    /// of `cls`: the virtual base's offset is the most derived object's
    /// ([class.mi]/4), so it is found through it ([`Self::sub_addr`]).
    /// `None` when it cannot be: a pointer past the end, or no `cls`
    /// object at `raw`.
    pub(crate) fn vbase_ptr(
        &mut self,
        b: &mut dyn SemaBridge,
        raw: u64,
        cls: OTy,
        base: OTy,
    ) -> Option<u64> {
        let i = self.poly_info(b, cls) as usize;
        let j = self.virt.infos[i].subs.iter().position(|s| s.cls == base)?;
        self.sub_addr(b, raw, cls, j as u16)
    }

    /// The most derived object (its type and offset in the allocation)
    /// one of whose subobjects of class `cls` lies at byte `rel` of the
    /// object of type `t` at offset `abs`: `t` itself when `cand` (a
    /// complete object, member or array element), else, descending through
    /// array elements, members and non-virtual bases, the innermost such
    /// object containing the byte. In a union only the active member is
    /// searched (any member while the active one is not known); `raw`
    /// names the allocation the union keys are in.
    fn base_owner(
        &mut self,
        b: &mut dyn SemaBridge,
        (t, raw): (OTy, u64),
        rel: u64,
        abs: u64,
        cand: bool,
        cls: OTy,
    ) -> Option<(OTy, u64)> {
        match self.types.kind(t) {
            TyKind::Array { elem, n } => {
                let es = self.types.size(elem);
                if es == 0 || rel / es >= n {
                    return None;
                }
                self.base_owner(b, (elem, raw), rel % es, abs + rel / es * es, true, cls)
            }
            TyKind::Record { union, .. } => {
                if cand && !union {
                    let i = self.poly_info(b, t) as usize;
                    if self.virt.infos[i].at.contains(&(rel, cls)) {
                        return Some((t, abs));
                    }
                }
                // [class.union]/2: at most one member is active.
                let active = if union {
                    self.mem.unions.get(&Ptr::union_key(raw, abs, t)).copied()
                } else {
                    None
                };
                let fields: Vec<_> = self
                    .types
                    .fields(t)
                    .iter()
                    .enumerate()
                    .filter(|&(i, f)| {
                        active.is_none_or(|a| a as usize == i)
                            && f.width.is_none()
                            && rel >= f.offset
                            && rel < f.offset + self.types.size(f.ty).max(1)
                    })
                    .map(|(_, f)| *f)
                    .collect();
                fields.into_iter().find_map(|f| {
                    let at = abs + f.offset;
                    self.base_owner(b, (f.ty, raw), rel - f.offset, at, !f.base, cls)
                })
            }
            _ => None,
        }
    }

    /// The subobjects of a complete `cls` object: offset, class (type and
    /// first declaration) and whether it is polymorphic, the object itself
    /// first.
    pub(crate) fn sub_list(
        &mut self,
        b: &mut dyn SemaBridge,
        cls: OTy,
    ) -> Vec<(u64, OTy, DeclId, bool)> {
        let i = self.poly_info(b, cls) as usize;
        self.virt.infos[i].subs.iter().map(|s| (s.off, s.cls, s.decl, s.poly)).collect()
    }

    /// `Op::TypeId`: the `std::type_info` object (of type `ti`) for the
    /// canonical type `q` ([expr.typeid]/1).
    pub(crate) fn type_info(&mut self, q: QualType, ti: OTy) -> u64 {
        let key = q.to_raw();
        if let Some(&p) = self.virt.type_infos.get(&key) {
            return p;
        }
        let size = self.types.size(ti);
        let p = self.mem.alloc_static(ti, size, crate::mem::flags::CONST, Origin::TypeInfo(q));
        self.virt.type_infos.insert(key, p);
        p
    }

    /// `Op::LabelAddr`: the address `&&l` (`AddrLabelExpr` `e`, of type
    /// `void *`) evaluates to; made on first use.
    pub(crate) fn label_addr(&mut self, b: &mut dyn SemaBridge, e: ExprId) -> u64 {
        if let Some(&p) = self.virt.labels.get(&e) {
            return p;
        }
        let q = b.ast().exprs.get(e).ty();
        let t = self.types.intern(b, q);
        let size = self.types.size(t);
        let fl = crate::mem::flags::CONST | crate::mem::flags::EXTERNAL;
        let p = self.mem.alloc_unbacked(t, size, fl, Origin::Label(e));
        self.virt.labels.insert(e, p);
        p
    }

    /// `Op::TypeId` of a polymorphic glvalue at `raw` (static class
    /// `cls`): its dynamic type ([expr.typeid]/3).
    pub(crate) fn type_info_dyn(
        &mut self,
        ast: &Ast,
        raw: u64,
        cls: OTy,
        ti: OTy,
    ) -> Result<u64, Fault> {
        let d = self.read_vptr(ast, raw, cls, ak::TYPEID)?;
        let q = self.types.info(d.cls).q;
        Ok(self.type_info(q, ti))
    }
}
