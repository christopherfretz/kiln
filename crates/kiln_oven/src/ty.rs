//! The Oven's type table: every type the evaluator touches, flattened to
//! what the VM needs (size, alignment, scalar kind, array extent, field
//! offsets). Built lazily from the AST through the bridge and kept for the
//! whole translation unit; the VM never looks at the AST's type graph.

use kiln_ast::consts::FloatKind;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::types::{BuiltinType, FloatFormat, TypeView};
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_source::fx::FxHashMap;

use crate::SemaBridge;

/// A type in the Oven's table.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub(crate) struct OTy(pub u32);

/// The shape of a type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TyKind {
    Void,
    Bool,
    Int {
        width: u8,
        signed: bool,
    },
    Float(FloatKind),
    /// Object or function pointer; the pointee.
    Ptr(OTy),
    NullPtr,
    Array {
        elem: OTy,
        n: u64,
    },
    Record {
        decl: DeclId,
        union: bool,
        first: u32,
        count: u32,
    },
    Func,
    /// Incomplete or not representable in M2 (member pointers,
    /// vectors, ...): it has no storage the VM can touch.
    Opaque,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TyInfo {
    /// Canonical, cv-unqualified.
    pub q: QualType,
    pub size: u64,
    pub align: u32,
    pub kind: TyKind,
    /// Contains a union subobject (loads and stores take the slow path).
    pub has_union: bool,
    /// Contains a pointer subobject.
    pub has_ptr: bool,
    /// Union subobjects at fixed offsets: `union_pos[upos.0..upos.1]`.
    pub upos: (u32, u32),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FieldInfo {
    pub decl: DeclId,
    pub ty: OTy,
    /// Byte offset (for a bit-field: of the byte holding its first bit).
    pub offset: u64,
    /// Bit offset within that byte (bit-fields only).
    pub bit: u32,
    /// Bit-field width (`None` for an ordinary member).
    pub width: Option<u32>,
}

/// The value class of a scalar type (how many registers, which ops).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Scal {
    Int { width: u8, signed: bool },
    Bool,
    Float(FloatKind),
    Ptr,
}

#[derive(Debug, Default)]
pub(crate) struct Types {
    pub infos: Vec<TyInfo>,
    pub fields: Vec<FieldInfo>,
    pub union_pos: Vec<(u64, OTy)>,
    map: FxHashMap<QualType, OTy>,
    arrays: FxHashMap<(OTy, u64), OTy>,
}

/// Evaluates an integral constant already checked by sema (bit-field
/// widths, array bounds): a `ConstantExpr` with its value, or a literal
/// under parentheses and casts.
pub(crate) fn eval_int(ast: &Ast, mut e: ExprId) -> Option<u128> {
    for _ in 0..64 {
        match ast.exprs.get(e).kind() {
            ExprKind::ConstantExpr => {
                if let Some(v) = ast.exprs.constant_value(e) {
                    return const_int(ast, v);
                }
                e = ast.exprs.operand(e);
            }
            ExprKind::IntegerLiteral => {
                return match ast.exprs.int_value(e) {
                    Some(v) => Some(u128::from(v)),
                    None => ast.exprs.literal_const(e).and_then(|v| const_int(ast, v)),
                };
            }
            ExprKind::ParenExpr => e = ast.exprs.operand(e),
            k if k.is_cast() => e = ast.exprs.operand(e),
            _ => return None,
        }
    }
    None
}

fn const_int(ast: &Ast, v: kiln_ast::ConstValueId) -> Option<u128> {
    match ast.consts.scalar(v)? {
        kiln_ast::consts::Scalar::Int { bits, .. } => bits.to_u128(),
        kiln_ast::consts::Scalar::Bool { value, .. } => Some(u128::from(value)),
        _ => None,
    }
}

/// The float kind of a builtin floating type on the target.
pub(crate) fn float_kind(b: BuiltinType, long_double: FloatFormat) -> Option<FloatKind> {
    Some(match b {
        BuiltinType::Float16 | BuiltinType::Half => FloatKind::Half,
        BuiltinType::BFloat16 => FloatKind::BFloat16,
        BuiltinType::Float => FloatKind::Single,
        BuiltinType::Double => FloatKind::Double,
        BuiltinType::Float128 => FloatKind::Quad,
        BuiltinType::LongDouble => match long_double {
            FloatFormat::IeeeSingle => FloatKind::Single,
            FloatFormat::X87Extended => FloatKind::X87Extended,
            FloatFormat::IeeeQuad => FloatKind::Quad,
            FloatFormat::IeeeHalf => FloatKind::Half,
            FloatFormat::BFloat => FloatKind::BFloat16,
            FloatFormat::IeeeDouble | FloatFormat::PpcDoubleDouble => FloatKind::Double,
        },
        _ => return None,
    })
}

impl Types {
    pub fn info(&self, t: OTy) -> &TyInfo {
        &self.infos[t.0 as usize]
    }

    pub fn kind(&self, t: OTy) -> TyKind {
        self.infos[t.0 as usize].kind
    }

    pub fn size(&self, t: OTy) -> u64 {
        self.infos[t.0 as usize].size
    }

    pub fn fields(&self, t: OTy) -> &[FieldInfo] {
        match self.kind(t) {
            TyKind::Record { first, count, .. } => {
                &self.fields[first as usize..(first + count) as usize]
            }
            _ => &[],
        }
    }

    pub fn unions(&self, t: OTy) -> &[(u64, OTy)] {
        let (a, b) = self.info(t).upos;
        &self.union_pos[a as usize..b as usize]
    }

    /// The scalar class of `t`, if it is a scalar.
    pub fn scal(&self, t: OTy) -> Option<Scal> {
        Some(match self.kind(t) {
            TyKind::Bool => Scal::Bool,
            TyKind::Int { width, signed } => Scal::Int { width, signed },
            TyKind::Float(k) => Scal::Float(k),
            TyKind::Ptr(_) | TyKind::NullPtr => Scal::Ptr,
            _ => return None,
        })
    }

    pub fn is_aggregate(&self, t: OTy) -> bool {
        matches!(self.kind(t), TyKind::Array { .. } | TyKind::Record { .. })
    }

    /// Interns `q` (any qualifiers and sugar are dropped).
    pub fn intern(&mut self, bridge: &mut dyn SemaBridge, q: QualType) -> OTy {
        let c = bridge.ast().types.canonical(q).without_quals();
        if let Some(&t) = self.map.get(&c) {
            return t;
        }
        let t = self.build(bridge, c);
        self.map.insert(c, t);
        t
    }

    /// A synthetic array of `n` elements of scalar type `elem` (string
    /// literal objects thawed from the store; its `q` is the element's).
    pub fn array_of(&mut self, elem: OTy, n: u64) -> OTy {
        if let Some(&t) = self.arrays.get(&(elem, n)) {
            return t;
        }
        let e = *self.info(elem);
        let t = self.push(TyInfo {
            q: e.q,
            size: e.size.saturating_mul(n),
            align: e.align,
            kind: TyKind::Array { elem, n },
            has_union: false,
            has_ptr: false,
            upos: (0, 0),
        });
        self.arrays.insert((elem, n), t);
        t
    }

    fn push(&mut self, info: TyInfo) -> OTy {
        let t = OTy(self.infos.len() as u32);
        self.infos.push(info);
        t
    }

    fn simple(&mut self, q: QualType, size: u64, align: u32, kind: TyKind) -> OTy {
        self.push(TyInfo { q, size, align, kind, has_union: false, has_ptr: false, upos: (0, 0) })
    }

    fn build(&mut self, bridge: &mut dyn SemaBridge, c: QualType) -> OTy {
        let ast = bridge.ast();
        let target = ast.types.target();
        let view = ast.types.canonical_view(c);
        let pbytes = u64::from(target.pointer_bytes.max(8));
        match view {
            TypeView::Builtin(b) => {
                let (size, align) = target.builtin_size_align(b);
                let (size, align) = (u64::from(size), u32::from(align));
                let kind = match b {
                    BuiltinType::Void => TyKind::Void,
                    BuiltinType::Bool => TyKind::Bool,
                    BuiltinType::NullPtr => TyKind::NullPtr,
                    BuiltinType::Char => TyKind::Int { width: 8, signed: target.char_signed },
                    BuiltinType::WChar => {
                        TyKind::Int { width: (size * 8) as u8, signed: target.wchar_signed }
                    }
                    BuiltinType::SChar
                    | BuiltinType::Short
                    | BuiltinType::Int
                    | BuiltinType::Long
                    | BuiltinType::LongLong
                    | BuiltinType::Int128 => TyKind::Int { width: (size * 8) as u8, signed: true },
                    BuiltinType::UChar
                    | BuiltinType::Char8
                    | BuiltinType::Char16
                    | BuiltinType::Char32
                    | BuiltinType::UShort
                    | BuiltinType::UInt
                    | BuiltinType::ULong
                    | BuiltinType::ULongLong
                    | BuiltinType::UInt128 => {
                        TyKind::Int { width: (size * 8) as u8, signed: false }
                    }
                    _ => match float_kind(b, target.long_double) {
                        Some(k) => TyKind::Float(k),
                        None => TyKind::Opaque,
                    },
                };
                self.simple(c, size, align.max(1), kind)
            }
            // A reference is stored as the address of its referent (only
            // a declaration has reference type: a variable, parameter,
            // field or return type; expressions never do).
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                let pointee = self.intern(bridge, p);
                self.simple(c, pbytes, pbytes as u32, TyKind::Ptr(pointee))
            }
            TypeView::Enum(d) => {
                let def = ast.definition(d).unwrap_or(d);
                let int =
                    ast.decls.enumeration(def).and_then(|e| e.int_type).unwrap_or(QualType::INT);
                let it = self.intern(bridge, int);
                let info = *self.info(it);
                self.simple(c, info.size, info.align, info.kind)
            }
            TypeView::ConstantArray(a) => {
                let elem = self.intern(bridge, a.elem);
                let e = *self.info(elem);
                let start = self.union_pos.len() as u32;
                if e.has_union {
                    for i in 0..a.size {
                        for k in e.upos.0..e.upos.1 {
                            let (o, u) = self.union_pos[k as usize];
                            self.union_pos.push((i * e.size + o, u));
                        }
                    }
                }
                let end = self.union_pos.len() as u32;
                self.push(TyInfo {
                    q: c,
                    size: e.size.saturating_mul(a.size),
                    align: e.align,
                    kind: TyKind::Array { elem, n: a.size },
                    has_union: e.has_union,
                    has_ptr: e.has_ptr || matches!(e.kind, TyKind::Ptr(_) | TyKind::NullPtr),
                    upos: (start, end),
                })
            }
            TypeView::Function(_) => self.simple(c, 1, 1, TyKind::Func),
            TypeView::Record(d) => self.record(bridge, c, d),
            _ => self.simple(c, 0, 1, TyKind::Opaque),
        }
    }

    fn record(&mut self, bridge: &mut dyn SemaBridge, c: QualType, d: DeclId) -> OTy {
        let ast = bridge.ast();
        let Some(def) = ast.definition(d) else {
            return self.simple(c, 0, 1, TyKind::Opaque);
        };
        let union = ast.decls.record(def).is_some_and(|r| r.tag == kiln_ast::decl::TagKind::Union);
        let fdecls: Vec<(DeclId, QualType, Option<u32>)> = ast
            .members(def)
            .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
            .map(|m| {
                let w = ast
                    .decls
                    .field(m)
                    .and_then(|f| f.bit_width)
                    .map(|e| eval_int(ast, e).map_or(0, |v| v.min(1 << 20) as u32));
                (m, ast.decls.get(m).ty().unwrap_or(QualType::INT), w)
            })
            .collect();
        let layout = bridge.ensure_layout(def);
        let (size, align) = (layout.size, layout.align);
        let offs: Vec<u64> = layout.fields.to_vec();
        // Reserve the slot first so self-referential pointers terminate.
        let me = self.simple(c, size, align.max(1), TyKind::Opaque);
        self.map.insert(c, me);
        let mut fields = Vec::with_capacity(fdecls.len());
        let (mut has_union, mut has_ptr) = (union, false);
        let mut upos = Vec::new();
        if union {
            upos.push((0, me));
        }
        for (i, &(m, fq, w)) in fdecls.iter().enumerate() {
            let fty = self.intern(bridge, fq);
            let bits = offs.get(i).copied().unwrap_or(0);
            let fi = *self.info(fty);
            has_union |= fi.has_union;
            has_ptr |= fi.has_ptr || matches!(fi.kind, TyKind::Ptr(_) | TyKind::NullPtr);
            if fi.has_union {
                for k in fi.upos.0..fi.upos.1 {
                    let (o, u) = self.union_pos[k as usize];
                    upos.push((bits / 8 + o, u));
                }
            }
            match w {
                Some(w) => fields.push(FieldInfo {
                    decl: m,
                    ty: fty,
                    offset: bits / 8,
                    bit: (bits % 8) as u32,
                    width: Some(w),
                }),
                None => fields.push(FieldInfo {
                    decl: m,
                    ty: fty,
                    offset: bits / 8,
                    bit: 0,
                    width: None,
                }),
            }
        }
        let first = self.fields.len() as u32;
        let count = fields.len() as u32;
        self.fields.extend(fields);
        upos.sort_by_key(|&(o, _)| o);
        let ustart = self.union_pos.len() as u32;
        self.union_pos.extend(upos);
        let uend = self.union_pos.len() as u32;
        let info = &mut self.infos[me.0 as usize];
        info.kind = TyKind::Record { decl: def, union, first, count };
        info.has_union = has_union;
        info.has_ptr = has_ptr;
        info.upos = (ustart, uend);
        me
    }

    /// The array extent containing the object of type `p` that a pointer
    /// at byte `off` of an object of type `root` designates: `(start, n)`
    /// in units of `p`. With `past`, the pointer is one past the end of
    /// that array. The flag is false for an object that is not an array
    /// element (treated as an array of one, [expr.add]/4).
    pub fn extent(&self, root: OTy, off: u64, past: bool, p: OTy) -> Option<(u64, u64, bool)> {
        let o = if past { off.checked_sub(1)? } else { off };
        self.extent_in(root, 0, o, p)
    }

    fn extent_in(&self, mut t: OTy, mut base: u64, o: u64, p: OTy) -> Option<(u64, u64, bool)> {
        loop {
            match self.kind(t) {
                TyKind::Array { elem, n } => {
                    if elem == p {
                        return Some((base, n, true));
                    }
                    let es = self.size(elem);
                    if es == 0 || n == 0 {
                        return None;
                    }
                    let i = ((o - base) / es).min(n - 1);
                    base += i * es;
                    t = elem;
                }
                _ if t == p => return Some((base, 1, false)),
                TyKind::Record { union, .. } => {
                    let rel = o - base;
                    let mut found = None;
                    for f in self.fields(t) {
                        if f.width.is_some() {
                            continue;
                        }
                        let sz = self.size(f.ty);
                        if rel >= f.offset && rel < f.offset + sz.max(1) {
                            if union {
                                if let Some(r) = self.extent_in(f.ty, base + f.offset, o, p) {
                                    return Some(r);
                                }
                                continue;
                            }
                            found = Some(*f);
                            break;
                        }
                    }
                    let f = found?;
                    base += f.offset;
                    t = f.ty;
                }
                _ => return None,
            }
        }
    }

    /// Whether an object of type `p` lives at byte `off` of an object of
    /// type `root` (C++26 [expr.static.cast]/14 for `void*` casts).
    pub fn has_subobject(&self, root: OTy, off: u64, p: OTy) -> bool {
        if p == root && off == 0 {
            return true;
        }
        match self.kind(root) {
            TyKind::Array { elem, n } => {
                let es = self.size(elem);
                es != 0 && off / es < n && self.has_subobject(elem, off % es, p)
            }
            TyKind::Record { .. } => self.fields(root).iter().any(|f| {
                f.width.is_none()
                    && off >= f.offset
                    && off < f.offset + self.size(f.ty).max(1)
                    && self.has_subobject(f.ty, off - f.offset, p)
            }),
            _ => false,
        }
    }
}
