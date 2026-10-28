//! Declaration facts the layout algorithm reads (spec/abi.md §4):
//! attributes (`aligned`, `alignas`, `packed`, `no_unique_address`),
//! bit-field widths, and the class properties "POD for the purpose of
//! layout" and "empty".

use kiln_ast::Ast;
use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::decl::{Access, DeclFlags, DeclKind, Defaulted, DefinitionBits, FieldData, TagKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::names::{DeclNameKind, OverloadedOperator};
use kiln_ast::types::TypeView;
use kiln_ast::{AttrId, ConstValueId, DeclId, ExprId, QualType, RawId};

use crate::target::Target;

/// The attributes of `d` (none unless `HAS_ATTRS` is set).
pub(crate) fn attrs(ast: &Ast, d: DeclId) -> impl Iterator<Item = &AttrNode> + '_ {
    let list = if ast.decls.get(d).has(DeclFlags::HAS_ATTRS) {
        ast.lists.get(ast.attrs.of_decl(d))
    } else {
        &[]
    };
    list.iter().filter_map(|&raw| AttrId::from_raw(raw)).map(|a| ast.attrs.get(a))
}

/// `d` or any redeclaration of it carries an attribute of `kind`
/// (clang merges inheritable attributes onto later declarations).
pub(crate) fn has_attr(ast: &Ast, d: DeclId, kind: AttrKind) -> bool {
    redecls(ast, d).any(|r| attrs(ast, r).any(|a| a.kind == kind))
}

fn redecls(ast: &Ast, d: DeclId) -> impl Iterator<Item = DeclId> + '_ {
    ast.redecls_of(d)
}

/// Evaluates an integral constant expression already checked by sema:
/// a `ConstantExpr` with its cached value, or a literal under parentheses
/// and implicit casts.
pub(crate) fn eval_int(ast: &Ast, mut e: ExprId) -> Option<u128> {
    for _ in 0..64 {
        let kind = ast.exprs.get(e).kind();
        match kind {
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

fn const_int(ast: &Ast, v: ConstValueId) -> Option<u128> {
    match ast.consts.scalar(v)? {
        kiln_ast::consts::Scalar::Int { bits, .. } => bits.to_u128(),
        kiln_ast::consts::Scalar::Bool { value, .. } => Some(u128::from(value)),
        _ => None,
    }
}

/// The largest alignment, in bits, requested for `d` by `alignas` /
/// `__attribute__((aligned))` on any of its declarations (clang
/// `Decl::getMaxAlignment`); 0 if none. Encoding (spec/abi.md §4.1):
/// an `ALIGNED` attribute's `a` is the argument `ExprId` (0 = no
/// argument: the target's biggest alignment) and `b` a raw `QualType` for
/// `alignas(type-id)`; `DeclArena::cold.alignment` holds further
/// `alignas` expressions. A value of 0 is ignored ([dcl.align]/4).
pub(crate) fn max_align_attr(
    ast: &Ast,
    d: DeclId,
    target: &Target,
    type_align_bits: &mut dyn FnMut(QualType) -> u64,
) -> u64 {
    let mut best = 0u64;
    for r in redecls(ast, d) {
        for a in attrs(ast, r) {
            if a.kind != AttrKind::ALIGNED {
                continue;
            }
            let bits = if let Some(q) = QualType::from_raw(a.b) {
                type_align_bits(q)
            } else if let Some(e) = ExprId::from_raw(a.a) {
                bytes_to_bits(eval_int(ast, e).unwrap_or(0))
            } else {
                target.biggest_align * 8
            };
            best = best.max(bits);
        }
        if let Some(&e) = ast.decls.cold.alignment.get(r) {
            best = best.max(bytes_to_bits(eval_int(ast, e).unwrap_or(0)));
        }
    }
    best
}

fn bytes_to_bits(v: u128) -> u64 {
    u64::try_from(v).unwrap_or(u64::MAX).saturating_mul(8)
}

/// One non-static data member as the layout algorithm sees it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FieldDesc {
    /// The `Field` declaration.
    pub decl: DeclId,
    /// Its declared type.
    pub ty: QualType,
    /// The bit-field width, if it is a bit-field.
    pub bit_width: Option<u64>,
    /// It has a name (unnamed bit-fields and anonymous members do not).
    pub named: bool,
    /// `[[no_unique_address]]`.
    pub nua: bool,
    /// `__attribute__((packed))` on the field.
    pub packed: bool,
}

/// The `Field` members of the class definition `def`, in declaration
/// order ([class.mem]).
pub(crate) fn fields(ast: &Ast, def: DeclId) -> Vec<FieldDesc> {
    ast.members(def)
        .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
        .map(|m| {
            let node = ast.decls.get(m);
            let data = ast.decls.field(m);
            let bit_width = data
                .and_then(|f| f.bit_width)
                .map(|e| u64::try_from(eval_int(ast, e).unwrap_or(0)).unwrap_or(u64::MAX));
            FieldDesc {
                decl: m,
                ty: node.ty().unwrap_or(QualType::INT),
                bit_width,
                named: !node.name().is_empty(),
                nua: data.is_some_and(FieldData::no_unique_address)
                    || attrs(ast, m).any(|a| a.kind == AttrKind::NO_UNIQUE_ADDRESS),
                packed: attrs(ast, m).any(|a| a.kind == AttrKind::PACKED),
            }
        })
        .collect()
}

/// Class-level facts of the definition `def`.
#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "independent class properties")]
pub(crate) struct ClassDesc {
    /// A `union`.
    pub is_union: bool,
    /// `__attribute__((packed))` on the class.
    pub packed: bool,
    /// Has at least one base class (laid out in M3; spec/abi.md §8).
    pub has_bases: bool,
    /// Polymorphic ([class.virtual]/1) or has a virtual base.
    pub dynamic: bool,
}

pub(crate) fn class_desc(ast: &Ast, def: DeclId) -> ClassDesc {
    let data = ast.decls.record(def);
    let defn = data.map_or(DefinitionBits::empty(), |r| r.defn);
    let has_virtual_fn = ast
        .members(def)
        .any(|m| ast.decls.function(m).is_some_and(|f| f.spec.is_virtual() || f.spec.is_pure()));
    ClassDesc {
        is_union: data.is_some_and(|r| r.tag == TagKind::Union),
        packed: has_attr(ast, def, AttrKind::PACKED),
        has_bases: ast.decls.bases(def, &ast.lists).next().is_some(),
        dynamic: has_virtual_fn
            || defn.contains(DefinitionBits::POLYMORPHIC)
            || defn.contains(DefinitionBits::HAS_VIRTUAL_BASES),
    }
}

/// The record a field's type designates through arrays (clang
/// `getBaseElementType` + `getAs<RecordType>`), as its first declaration.
pub(crate) fn base_element_record(ast: &Ast, q: QualType) -> Option<DeclId> {
    match strip_arrays(ast, q) {
        TypeView::Record(d) => Some(ast.first(d)),
        _ => None,
    }
}

/// The canonical view of `q` with every array level removed.
pub(crate) fn strip_arrays(ast: &Ast, q: QualType) -> TypeView {
    let mut v = ast.types.canonical_view(q);
    for _ in 0..1024 {
        match v {
            TypeView::ConstantArray(a) => v = ast.types.canonical_view(a.elem),
            TypeView::IncompleteArray(e) | TypeView::VariableArray { elem: e, .. } => {
                v = ast.types.canonical_view(e);
            }
            TypeView::DependentSizedArray { elem, .. } => v = ast.types.canonical_view(elem),
            _ => return v,
        }
    }
    v
}

/// The record a field's type designates directly (clang
/// `getAsCXXRecordDecl`: through sugar and cv, not arrays or `_Atomic`).
pub(crate) fn as_record(ast: &Ast, q: QualType) -> Option<DeclId> {
    match ast.types.canonical_view(q) {
        TypeView::Record(d) => Some(ast.first(d)),
        _ => None,
    }
}

/// Is the class "POD for the purpose of layout" (Itanium ABI §1.1 "POD
/// for the purpose of layout" = C++03 POD; clang `CXXRecordDecl::isPOD`)?
/// `field_pod` answers the question for a member's class.
pub(crate) fn is_pod(
    ast: &Ast,
    def: DeclId,
    class: ClassDesc,
    fields: &[FieldDesc],
    field_pod: &dyn Fn(DeclId) -> bool,
) -> bool {
    if class.has_bases || class.dynamic {
        return false;
    }
    for f in fields {
        if f.bit_width.is_some() && !f.named {
            // Unnamed bit-fields are not members ([class.bit]/2).
            continue;
        }
        let node = ast.decls.get(f.decl);
        if matches!(node.misc().access(), Access::Private | Access::Protected) {
            return false;
        }
        if ast.decls.field(f.decl).is_some_and(|d| d.init.is_some() || (d.flags >> 2) & 3 != 0) {
            return false;
        }
        match strip_arrays(ast, f.ty) {
            TypeView::LValueRef(_) | TypeView::RValueRef(_) | TypeView::Atomic(_) => return false,
            TypeView::Record(r) if !field_pod(ast.first(r)) => return false,
            _ => {}
        }
    }
    let record_ty = ast.decls.get(def).ty();
    for m in ast.members(def) {
        let node = ast.decls.get(m);
        let user = !node.has(DeclFlags::IMPLICIT);
        match node.kind() {
            DeclKind::CXXConstructor | DeclKind::CXXDestructor if user => {
                if ast
                    .decls
                    .function(m)
                    .is_some_and(|f| !f.spec.is_deleted() && f.spec.defaulted() == Defaulted::No)
                {
                    return false;
                }
            }
            DeclKind::FunctionTemplate if node.name().kind() == DeclNameKind::Constructor => {
                return false;
            }
            DeclKind::CXXMethod
                if user && node.name().as_operator() == Some(OverloadedOperator::Equal) =>
            {
                let provided = ast
                    .decls
                    .function(m)
                    .is_some_and(|f| !f.spec.is_deleted() && f.spec.defaulted() == Defaulted::No);
                if provided && is_copy_assign(ast, node.ty(), record_ty) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

/// A copy assignment operator takes `X`, `X&`, `const X&`, `volatile X&`
/// or `const volatile X&` ([class.copy.assign]/1).
fn is_copy_assign(ast: &Ast, fn_ty: Option<QualType>, record: Option<QualType>) -> bool {
    let (Some(fn_ty), Some(record)) = (fn_ty, record) else { return false };
    let Some(proto) = ast.types.proto(ast.types.canonical(fn_ty)) else { return false };
    let params = ast.types.params(proto);
    if params.len() != 1 || proto.variadic() {
        return false;
    }
    let mut p = ast.types.canonical(params[0]);
    if let TypeView::LValueRef(inner) = ast.types.view(p.ty()) {
        p = ast.types.canonical(inner);
    }
    ast.types.same_unqualified(p, ast.types.canonical(record))
}

/// Is the class empty ([class.prop]/? `std::is_empty`; clang
/// `CXXRecordDecl::isEmpty`): no non-static data members other than
/// zero-width bit-fields and `[[no_unique_address]]` members of empty
/// class type, no virtual functions or virtual bases, and only empty
/// bases. `record_empty` answers the question for other classes.
pub(crate) fn is_empty(
    ast: &Ast,
    def: DeclId,
    class: ClassDesc,
    fields: &[FieldDesc],
    record_empty: &dyn Fn(DeclId) -> bool,
) -> bool {
    if class.dynamic {
        return false;
    }
    for b in ast.decls.bases(def, &ast.lists) {
        if b.is_virtual {
            return false;
        }
        match as_record(ast, b.ty) {
            Some(r) if record_empty(r) => {}
            _ => return false,
        }
    }
    fields.iter().all(|f| match f.bit_width {
        Some(w) => !f.named && w == 0,
        None => f.nua && as_record(ast, f.ty).is_some_and(record_empty),
    })
}
