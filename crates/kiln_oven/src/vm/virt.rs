//! Dynamic types (WP-077; spec/oven.md "Dynamic types"): the vptr slot of
//! each polymorphic subobject holds a *dynamic-type word* — the class `C`
//! whose constructor or destructor last ran on the object (its Oven type,
//! low 32 bits) and the subobject's offset in `C` (high 32 bits). Written
//! by `Op::SetVptr` ([class.cdtor]/4: the dynamic type of an object under
//! construction or destruction is the class of the running constructor or
//! destructor), read by virtual calls, `dynamic_cast` and `typeid`. The
//! words are not part of a frozen value: a thawed object gets them back
//! from its type (`Oven::thaw_vptrs`).

use kiln_ast::class::SubobjectId;
use kiln_ast::decl::{Access, TagKind};
use kiln_ast::{Ast, DeclId, QualType};
use kiln_source::fx::FxHashMap;

use super::Fault;
use crate::Oven;
use crate::SemaBridge;
use crate::kbc::ak;
use crate::mem::{MemFault, Origin, Ptr};
use crate::ty::{OTy, TyKind};

/// One subobject of a class (indexed by [`SubobjectId`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sub {
    /// Byte offset in the complete object.
    pub off: u64,
    pub cls: OTy,
    /// The class (first declaration).
    pub decl: DeclId,
    /// A virtual base or one of its subobjects (offset relative to a
    /// complete object of the class only).
    pub virt: bool,
    /// The class is polymorphic (has a vptr slot at `off`).
    pub poly: bool,
    /// The subobject this one is a direct base of (itself for 0).
    pub parent: u16,
    /// That base-specifier grants public access ([class.access.base]/1).
    pub public: bool,
}

/// The subobject graph of a class with offsets.
#[derive(Debug)]
pub(crate) struct PolyInfo {
    /// The class (first declaration; `None` for a non-class).
    pub decl: Option<DeclId>,
    pub subs: Vec<Sub>,
    /// Distinct offsets of the vptr slots `SetVptr` writes.
    pub vptrs: Vec<u64>,
}

/// A resolved virtual call.
#[derive(Clone, Copy, Debug)]
struct Target {
    func: u32,
    /// The overrider's subobject offset minus the called subobject's.
    delta: i64,
    ov: DeclId,
    pure: bool,
}

/// The Oven's per-class caches (insert-only: the class graph of a
/// complete class never changes).
#[derive(Debug, Default)]
pub(crate) struct Virt {
    map: FxHashMap<OTy, u32>,
    infos: Vec<PolyInfo>,
    dispatch: FxHashMap<(OTy, u64, DeclId), Target>,
    /// The `std::type_info` object of each type `typeid` named (by its
    /// canonical type): a static allocation, so `&typeid(T)` is one
    /// address for the translation unit.
    type_infos: FxHashMap<u32, u64>,
}

impl Virt {
    pub fn footprint(&self) -> usize {
        self.map.capacity() * 12
            + self
                .infos
                .iter()
                .map(|i| i.subs.capacity() * 40 + i.vptrs.capacity() * 8)
                .sum::<usize>()
            + self.dispatch.capacity() * 48
            + self.type_infos.capacity() * 12
    }
}

/// Encodes a dynamic-type word.
pub(crate) fn word(cls: OTy, off: u64) -> u128 {
    u128::from(cls.0) | u128::from(off) << 32
}

/// The record type of class declaration `d`.
pub(crate) fn class_qtype(ast: &Ast, d: DeclId) -> Option<QualType> {
    ast.decls.get(ast.definition(d).unwrap_or(d)).ty()
}

impl Oven {
    /// The index of class `cls`'s [`PolyInfo`] (computed on first use).
    pub(crate) fn poly_info(&mut self, b: &mut dyn SemaBridge, cls: OTy) -> u32 {
        if let Some(&i) = self.virt.map.get(&cls) {
            return i;
        }
        let TyKind::Record { decl, .. } = self.types.kind(cls) else {
            let i = self.virt.infos.len() as u32;
            self.virt.infos.push(PolyInfo { decl: None, subs: Vec::new(), vptrs: Vec::new() });
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
                let l = b.ensure_layout(def);
                (l.vbases.iter().find(|v| v.0 == s.class).map_or(0, |v| v.1), true)
            } else {
                let parent = subs[s.parent.index()];
                let pdef = b.ast().definition(parent.decl).unwrap_or(parent.decl);
                let l = b.ensure_layout(pdef);
                let o = l.bases.iter().find(|v| v.0 == s.class).map_or(0, |v| v.1);
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
            subs.push(Sub {
                off,
                cls: cty,
                decl: s.class,
                virt,
                poly,
                parent: s.parent.index() as u16,
                public,
            });
        }
        let mut vptrs: Vec<u64> =
            subs.iter().filter(|s| s.poly && !s.virt).map(|s| s.off).collect();
        vptrs.sort_unstable();
        vptrs.dedup();
        let i = self.virt.infos.len() as u32;
        self.virt.infos.push(PolyInfo { decl: Some(first), subs, vptrs });
        self.virt.map.insert(cls, i);
        i
    }

    /// `Op::SetVptr`: the object at `raw` (of class `cls`) has dynamic
    /// type `cls`.
    pub(crate) fn set_vptrs(
        &mut self,
        b: &mut dyn SemaBridge,
        raw: u64,
        cls: OTy,
    ) -> Result<(), Fault> {
        let i = self.poly_info(b, cls);
        let n = self.virt.infos[i as usize].vptrs.len();
        for k in 0..n {
            let off = self.virt.infos[i as usize].vptrs[k];
            let p = Ptr::with_off(raw, u64::from(Ptr::decode(raw).off) + off, false);
            self.write_vptr(p, word(cls, off)).map_err(|f| Fault::Mem {
                f,
                ak: ak::INIT,
                ty: cls,
                p: raw,
            })?;
        }
        Ok(())
    }

    fn write_vptr(&mut self, p: u64, w: u128) -> Result<(), MemFault> {
        let (sp, abs, _) = self.mem.check(p, 8, true)?;
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
                let i = self.poly_info(b, t);
                let n = self.virt.infos[i as usize].vptrs.len();
                for k in 0..n {
                    let off = self.virt.infos[i as usize].vptrs[k];
                    let s = self.mem.space_mut(sp);
                    s.write(abs + off, 8, word(t, off));
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
    pub(crate) fn read_vptr(&self, raw: u64, cls: OTy, k: u32) -> Result<(OTy, u64), Fault> {
        let (sp, abs, _) = self.mem.check(raw, 8, false).map_err(|f| match f {
            MemFault::External(..) | MemFault::Temporary(..) => {
                Fault::UnknownDynamic { ak: k, p: raw, ty: cls }
            }
            f => Fault::Mem { f, ak: k, ty: cls, p: raw },
        })?;
        let s = self.mem.space(sp);
        if !s.is_init(abs, 8) {
            return Err(Fault::Invalid);
        }
        let w = s.read(abs, 8) as u64;
        Ok((OTy(w as u32), w >> 32))
    }

    /// The subobject of the dynamic type `c` at offset `ott` whose class
    /// is `cls`: the subobject `raw` designates, viewed as part of `c`.
    pub(crate) fn locate(
        &mut self,
        b: &mut dyn SemaBridge,
        c: OTy,
        ott: u64,
        cls: OTy,
    ) -> Option<(u32, u16)> {
        let i = self.poly_info(b, c);
        let k = self.virt.infos[i as usize]
            .subs
            .iter()
            .position(|s| !s.virt && s.off == ott && s.cls == cls)?;
        Some((i, k as u16))
    }

    /// `Op::VCall`: the function and adjusted `this` of a virtual call of
    /// `method` (a member of class `cls`) on the object at `raw`.
    pub(crate) fn vcall(
        &mut self,
        b: &mut dyn SemaBridge,
        method: DeclId,
        cls: OTy,
        raw: u64,
    ) -> Result<(u32, u64), Fault> {
        let (c, ott) = self.read_vptr(raw, cls, ak::MEMBER_CALL)?;
        let t = match self.virt.dispatch.get(&(c, ott, method)) {
            Some(&t) => t,
            None => {
                let t = self.resolve(b, method, cls, c, ott)?;
                self.virt.dispatch.insert((c, ott, method), t);
                t
            }
        };
        if t.pure {
            return Err(Fault::PureVirtual(t.ov));
        }
        let off = i64::from(Ptr::decode(raw).off) + t.delta;
        Ok((t.func, Ptr::with_off(raw, off as u64, false)))
    }

    fn resolve(
        &mut self,
        b: &mut dyn SemaBridge,
        method: DeclId,
        cls: OTy,
        c: OTy,
        ott: u64,
    ) -> Result<Target, Fault> {
        let (i, at) = self.locate(b, c, ott, cls).ok_or(Fault::Invalid)?;
        let cdecl = self.virt.infos[i as usize].decl.ok_or(Fault::Invalid)?;
        let (ov, s2) = b.final_overrider(cdecl, SubobjectId(at), method).ok_or(Fault::Invalid)?;
        let to = self.virt.infos[i as usize].subs.get(s2.index()).copied().ok_or(Fault::Invalid)?;
        if to.virt {
            return Err(Fault::Invalid);
        }
        let ast = b.ast();
        let pure = ast.decls.function(ast.first(ov)).is_some_and(|f| f.spec.is_pure());
        let func = self.func_index(ast, ov);
        Ok(Target { func, delta: to.off as i64 - ott as i64, ov, pure })
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
        let (c, ott) = self.read_vptr(raw, stat, ak::DYNAMIC_CAST)?;
        let base = u64::from(Ptr::decode(raw).off) - ott;
        let at = |off: u64| Ptr::with_off(raw, base + off, false);
        let Some(t) = target else { return Ok(at(0)) };
        let (i, k) = self.locate(b, c, ott, stat).ok_or(Fault::Invalid)?;
        let subs = &self.virt.infos[i as usize].subs;
        let fail = |kind: u8| {
            if glvalue { Err(Fault::DynCast { kind, stat, dynamic: c, target: t }) } else { Ok(0) }
        };
        // Phase 1: from the operand's subobject up towards the most
        // derived object, across public edges only.
        let mut cur = usize::from(k);
        loop {
            let s = subs[cur];
            if s.cls == t {
                return Ok(at(s.off));
            }
            if cur == 0 {
                break;
            }
            if !s.public {
                let has = subs.iter().any(|s| s.cls == t);
                return fail(if has { 0 } else { 1 });
            }
            cur = usize::from(s.parent);
        }
        // Phase 2: an unambiguous public base of the dynamic type.
        let mut found = subs.iter().enumerate().filter(|(_, s)| s.cls == t);
        let Some((j, s)) = found.next() else { return fail(1) };
        if found.next().is_some() {
            return fail(2);
        }
        let mut j = j;
        while j != 0 {
            if !subs[j].public {
                return fail(3);
            }
            j = usize::from(subs[j].parent);
        }
        if s.virt {
            return Err(Fault::Invalid);
        }
        Ok(at(s.off))
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

    /// `Op::TypeId` of a polymorphic glvalue at `raw` (static class
    /// `cls`): its dynamic type ([expr.typeid]/3).
    pub(crate) fn type_info_dyn(&mut self, raw: u64, cls: OTy, ti: OTy) -> Result<u64, Fault> {
        let (c, _) = self.read_vptr(raw, cls, ak::TYPEID)?;
        let q = self.types.info(c).q;
        Ok(self.type_info(q, ti))
    }
}
