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

use crate::ty::{OTy, TyKind, Types};
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
    /// in the class that directly contains it (for a union's active
    /// member).
    pub field: Option<(u64, OTy, u32)>,
    /// The union that directly contains the data member (`cls`, or for a
    /// member of an anonymous union the union's type, [class.union.anon]).
    pub union: Option<OTy>,
    /// A member function: its function index, and whether it is virtual
    /// (a call through the member pointer dispatches, `Op::MpFn`).
    pub fn_index: u32,
    pub virt: bool,
    /// Interned while its class (or a class of its path) was incomplete
    /// and not laid out yet ([`Oven::mp_refresh`]): `adj`, `field` and
    /// `union` are not meaningful.
    pub pending: bool,
}

/// The layout-dependent part of an [`MpEntry`].
struct MpLayout {
    adj: u64,
    cls: OTy,
    field: Option<(u64, OTy, u32)>,
    union: Option<OTy>,
}

/// The interned member pointer values of an Oven.
#[derive(Debug, Default)]
pub(crate) struct MemPtrs {
    entries: Vec<MpEntry>,
    map: FxHashMap<(DeclId, bool, Box<[DeclId]>), u32>,
    /// The `pending` entries.
    pending: Vec<u32>,
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
        let vb = self.types.vbase_start(cls);
        self.types.fields(cls)[..vb]
            .iter()
            .find(|f| f.base && self.types.same_class(f.ty, base))
            .map_or(0, |f| f.offset)
    }

    /// Interns the member pointer to `member` (a field or member
    /// function) with conversion state `derived` and `path`; returns its
    /// register value (never 0), or `None` if the member cannot be
    /// designated (a bit-field, a member reached through an anonymous
    /// union holding an anonymous struct). A member
    /// of an anonymous union or struct is its `IndirectField`: the offset
    /// sums its access path ([class.union.anon]/1).
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
        let chain: Vec<DeclId> = match kind {
            _ if func => Vec::new(),
            DeclKind::Field => vec![member],
            DeclKind::IndirectField => ast.lists.iter(ast.indirect_field_chain(member)).collect(),
            _ => return None,
        };
        // A class still being defined (`&C::m` in a static data member
        // initializer of `C`, or of a class nested in it): the value is
        // only its member and path ([expr.unary.op]/4 needs no layout),
        // so it is interned now and laid out once the classes complete
        // (`mp_refresh`); until then it designates no subobject.
        let ast = b.ast();
        let pending =
            Types::incomplete(ast, owner) || path.iter().any(|&d| Types::incomplete(ast, d));
        let MpLayout { adj, cls, field, union } = if pending {
            MpLayout { adj: 0, cls: self.record_ty(b, owner)?, field: None, union: None }
        } else {
            self.mp_layout(b, owner, &chain, derived, &path)?
        };
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
            pending,
        });
        if pending {
            self.mp.pending.push(i);
        }
        self.mp.map.insert((member, derived, path), i);
        Some(u128::from(i) + 1)
    }

    /// The layout part of a member pointer to the data member reached
    /// through `chain` (empty for a member function) of complete class
    /// `owner`, with conversion state `derived` and `path`; `None` if the
    /// member cannot be designated.
    fn mp_layout(
        &mut self,
        b: &mut dyn SemaBridge,
        owner: DeclId,
        chain: &[DeclId],
        derived: bool,
        path: &[DeclId],
    ) -> Option<MpLayout> {
        let cls = self.record_ty(b, owner)?;
        let mut union = None;
        let field = if chain.is_empty() {
            None
        } else {
            let (mut ty, mut off, mut last) = (cls, 0, None);
            for (k, &fd) in chain.iter().enumerate() {
                let is_union = matches!(self.types.kind(ty), TyKind::Record { union: true, .. });
                if is_union && k + 1 < chain.len() {
                    // Activating the anonymous struct inside the union is
                    // not modelled.
                    return None;
                }
                let fs = self.types.fields(ty);
                let i = fs.iter().position(|f| !f.base && f.decl == fd)?;
                let f = fs[i];
                if f.width.is_some() {
                    return None;
                }
                if is_union {
                    union = Some(ty);
                }
                off += f.offset;
                ty = f.ty;
                last = Some(i as u32);
            }
            Some((off, ty, last?))
        };
        let mut tys = Vec::with_capacity(path.len());
        for &d in path {
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
        Some(MpLayout { adj, cls, field, union })
    }

    /// Lays out the pending member pointer values whose classes completed
    /// since they were interned; called after [`Types::refresh`]. O(1)
    /// when none is pending.
    pub(crate) fn mp_refresh(&mut self, b: &mut dyn SemaBridge) {
        if self.mp.pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.mp.pending);
        for i in pending {
            let e = &self.mp.entries[i as usize];
            let (member, derived, path, func) = (e.member, e.derived, e.path.clone(), e.func);
            let ast = b.ast();
            let Some(owner) = ast.decls.get(member).ctx().map(|c| def(&*b, c)) else { continue };
            if Types::incomplete(ast, owner) || path.iter().any(|&d| Types::incomplete(ast, d)) {
                self.mp.pending.push(i);
                continue;
            }
            let chain: Vec<DeclId> = match ast.decls.get(member).kind() {
                _ if func => Vec::new(),
                DeclKind::IndirectField => {
                    ast.lists.iter(ast.indirect_field_chain(member)).collect()
                }
                _ => vec![member],
            };
            // A member that cannot be designated stays pending: it
            // designates no subobject (`Fault::Invalid` at a use).
            if let Some(l) = self.mp_layout(b, owner, &chain, derived, &path) {
                let e = &mut self.mp.entries[i as usize];
                (e.adj, e.cls, e.field, e.union, e.pending) =
                    (l.adj, l.cls, l.field, l.union, false);
            }
        }
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
