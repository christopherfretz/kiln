//! Pointers to members ([dcl.mptr], [conv.mem], [expr.mptr.oper]).
//!
//! A member pointer value is clang's `APValue::MemberPointer`: the member,
//! whether the base/derived conversions applied so far walk from the
//! member's class towards a base (`derived`, clang's
//! `isMemberPointerToDerivedMember`), and the classes traversed. The Oven
//! interns each distinct value; a register (and the object representation
//! in memory) holds its index plus one, so the null member pointer is 0
//! (zero-initialization yields it), equality is index equality
//! ([expr.eq]/4: same member, same path), and the stored form maps 1:1 to
//! [`Scalar::MemberPtr`](kiln_ast::consts::Scalar::MemberPtr).
//!
//! An entry precomputes how `obj.*mp` reaches the member from an object
//! of the member pointer's class: a non-derived value adds the offsets of
//! the base subobjects along its path (most derived first); a derived
//! value checks that the object is the matching base subobject of an
//! object of the member's class and steps back to it.

use kiln_ast::consts::{ConstError, MemberKind, NullKind, Scalar};
use kiln_ast::decl::DeclKind;
use kiln_ast::{ConstValueId, DeclId, QualType};
use kiln_source::fx::FxHashMap;

use crate::ty::{OTy, TyKind};
use crate::{Oven, SemaBridge};

/// One interned member pointer value.
#[derive(Clone, Debug)]
pub(crate) struct MpEntry {
    /// The `FieldDecl` or `CXXMethodDecl` (first declaration).
    pub member: DeclId,
    /// A member function.
    pub func: bool,
    pub derived: bool,
    /// Record definitions, outermost first (clang's `Path`).
    pub path: Box<[DeclId]>,
    /// Byte offset from the object to the member's class subobject
    /// (non-derived), or from the member's class object to the object
    /// (derived).
    pub adj: u64,
    /// The member's class.
    pub cls: OTy,
    /// A data member: its offset and type in `cls`; `idx` its field index
    /// (for a union's active member).
    pub field: Option<(u64, OTy, u32)>,
    /// The member's class is a union.
    pub union: bool,
    /// A member function: its function index, and whether it is virtual
    /// (a call through the member pointer dispatches, `Op::MpFn`).
    pub fn_index: u32,
    pub virt: bool,
}

/// The interned member pointer values of an Oven.
#[derive(Debug, Default)]
pub(crate) struct MemPtrs {
    entries: Vec<MpEntry>,
    map: FxHashMap<(DeclId, bool, Box<[DeclId]>), u32>,
}

impl MemPtrs {
    /// The entry of register value `v` (`None` for the null value or an
    /// unknown one).
    pub fn get(&self, v: u128) -> Option<&MpEntry> {
        usize::try_from(v).ok().and_then(|i| i.checked_sub(1)).and_then(|i| self.entries.get(i))
    }

    pub fn footprint(&self) -> usize {
        self.entries.len() * (size_of::<MpEntry>() + 48)
            + self.entries.iter().map(|e| e.path.len() * 8).sum::<usize>()
    }
}

/// The definition of record `d`.
fn def(b: &dyn SemaBridge, d: DeclId) -> DeclId {
    let ast = b.ast();
    ast.definition(d).unwrap_or_else(|| ast.first(d))
}

impl Oven {
    /// The type of record `d`.
    fn record_ty(&mut self, b: &mut dyn SemaBridge, d: DeclId) -> Option<OTy> {
        let q: QualType = b.ast().decls.get(d).ty()?;
        Some(self.types.intern(b, q))
    }

    /// The offset of the direct base `base` in `cls` (0 for an empty one).
    fn direct_base(&self, cls: OTy, base: OTy) -> u64 {
        self.types.fields(cls).iter().find(|f| f.base && f.ty == base).map_or(0, |f| f.offset)
    }

    /// Interns the member pointer to `member` (a field or member
    /// function) with conversion state `derived` and `path`; returns its
    /// register value (never 0), or `None` if the member cannot be
    /// designated (an anonymous-member field, an incomplete class).
    pub(crate) fn mp_intern(
        &mut self,
        b: &mut dyn SemaBridge,
        member: DeclId,
        derived: bool,
        path: &[DeclId],
    ) -> Option<u128> {
        let member = b.ast().first(member);
        let path: Box<[DeclId]> = path.iter().map(|&d| def(&*b, d)).collect();
        let key = (member, derived, path);
        if let Some(&i) = self.mp.map.get(&key) {
            return Some(u128::from(i) + 1);
        }
        let (member, derived, path) = key;
        let ast = b.ast();
        let kind = ast.decls.get(member).kind();
        let func = kind.is_function();
        let owner = def(&*b, ast.decls.get(member).ctx()?);
        let fn_index = if func { self.func_index(b.ast(), member) } else { 0 };
        let virt = func
            && b.ast()
                .decls
                .function(member)
                .is_some_and(|f| f.spec.is_virtual() && !f.spec.is_final());
        let cls = self.record_ty(b, owner)?;
        let union = matches!(self.types.kind(cls), TyKind::Record { union: true, .. });
        let field = if func {
            None
        } else {
            if kind != DeclKind::Field {
                return None;
            }
            let fs = self.types.fields(cls);
            let i = fs.iter().position(|f| !f.base && f.decl == member)?;
            let f = fs[i];
            if f.width.is_some() {
                return None;
            }
            Some((f.offset, f.ty, i as u32))
        };
        let mut tys = Vec::with_capacity(path.len());
        for &d in &path {
            tys.push(self.record_ty(b, d)?);
        }
        let mut adj = 0;
        if derived {
            // The member's class, then each base towards the object.
            let mut cur = cls;
            for &t in &tys {
                adj += self.direct_base(cur, t);
                cur = t;
            }
        } else if let Some(&last) = tys.last() {
            // The object (the last class), then each base towards the
            // member's class.
            let mut cur = last;
            for &t in tys.iter().rev().skip(1) {
                adj += self.direct_base(cur, t);
                cur = t;
            }
            adj += self.direct_base(cur, cls);
        }
        let i = self.mp.entries.len() as u32;
        self.mp.entries.push(MpEntry {
            member,
            func,
            derived,
            path: path.clone(),
            adj,
            cls,
            field,
            union,
            fn_index,
            virt,
        });
        self.mp.map.insert((member, derived, path), i);
        Some(u128::from(i) + 1)
    }

    /// clang's `MemberPtr::castToDerived` (`to_base` false) or
    /// `castToBase` towards class `class`: the converted value, `None`
    /// if the conversion leaves the member's hierarchy (clang's
    /// `castBack` failure, [expr.static.cast]/12).
    pub(crate) fn mp_cast(
        &mut self,
        b: &mut dyn SemaBridge,
        v: u128,
        class: DeclId,
        to_base: bool,
    ) -> Option<u128> {
        let Some(e) = self.mp.get(v) else { return Some(v) };
        let class = def(&*b, class);
        let (member, mut derived, mut path) = (e.member, e.derived, e.path.to_vec());
        let owner = def(&*b, b.ast().decls.get(member).ctx()?);
        if to_base && path.is_empty() {
            derived = true;
        }
        if derived == to_base {
            path.push(class);
        } else {
            // clang's `castBack`.
            let expected = if path.len() >= 2 { path[path.len() - 2] } else { owner };
            if expected != class {
                return None;
            }
            path.pop();
            if !to_base && path.is_empty() {
                derived = false;
            }
        }
        self.mp_intern(b, member, derived, &path)
    }

    /// The stored form of member pointer register value `v` of type `ty`.
    pub(crate) fn mp_freeze(
        &self,
        b: &mut dyn SemaBridge,
        ty: OTy,
        v: u128,
    ) -> Result<ConstValueId, ConstError> {
        let q = self.types.info(ty).q;
        let func = matches!(self.types.kind(ty), TyKind::MemPtr { func: true });
        match self.mp.get(v) {
            None => {
                let kind = if func { NullKind::MemberFunction } else { NullKind::MemberData };
                b.consts().push_scalar(Scalar::Null { ty: q, kind })
            }
            Some(e) => {
                let kind = if e.func { MemberKind::Function } else { MemberKind::Data };
                let s = Scalar::MemberPtr {
                    ty: q,
                    member: e.member,
                    kind,
                    derived: e.derived,
                    path: &e.path,
                };
                b.consts().push_scalar(s)
            }
        }
    }

    /// The register value of stored member pointer `cv` (0 for null).
    pub(crate) fn mp_thaw(&mut self, b: &mut dyn SemaBridge, cv: ConstValueId) -> u128 {
        let (member, derived, path) = match b.consts().scalar(cv) {
            Some(Scalar::MemberPtr { member, derived, path, .. }) => {
                (member, derived, path.to_vec())
            }
            _ => return 0,
        };
        self.mp_intern(b, member, derived, &path).unwrap_or(0)
    }
}
