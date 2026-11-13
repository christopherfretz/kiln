//! Target-predefined typedefs in the declaration image (WP-1369,
//! spec/modules.md §11.11, "Forms carried by WP-1369").
//!
//! On `AArch64` every translation unit declares one implicit typedef per
//! SVE ACLE sizeless type (`__SVFloat32_t`, `__SVBool_t`, ...,
//! `__clang_svint8x2_t`, ...; `declare_sve_typedefs`, clang's
//! `Sema::Initialize`), as it declares `__builtin_va_list`. A type naming
//! one (glibc's `typedef __SVFloat32_t __sv_f32_t;`) is written as a
//! builtin typedef ([`super::TyRec::Sugar`]) numbered after the six of
//! `image_sugar` and the `va_list` record: `SUGAR_SVE + j` for the `j`-th
//! sizeless builtin type in `BuiltinType::ALL` order. The importer names
//! its own typedef of that name. An image of another target names none
//! (they are not declared there), and a precompiled header is only used
//! for its own target.
//!
//! The class `__builtin_va_list` names (`std::__va_list` on `AArch64`,
//! `__va_list_tag` on x86-64, `declare_builtin_va_list`) is implicit too:
//! a type naming it (a `va_list` argument copied into `vsnprintf(..., v)`)
//! is builtin typedef number [`SUGAR_VA_RECORD`], the importer's own
//! class.

use kiln_ast::QualType;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::names::{DeclName, DeclNameView};
use kiln_ast::types::{BuiltinType, TypeView};
use kiln_ast::{DeclId, TypeId};

use super::{DECL_FLAGS, DeclRec, Image, NONE, Payload, Writer};
use crate::Sema;

/// The [`super::TyRec::Sugar`] number of the class `__builtin_va_list`
/// names.
pub(super) const SUGAR_VA_RECORD: u8 = 6;
/// The first [`super::TyRec::Sugar`] number of the SVE typedefs.
pub(super) const SUGAR_SVE: u8 = 7;

/// The sizeless builtin types, in `BuiltinType::ALL` order.
fn sizeless() -> impl Iterator<Item = BuiltinType> {
    BuiltinType::ALL.into_iter().filter(|b| b.is_sizeless())
}

/// The number of builtin typedef numbers (the six of `image_sugar`, the
/// `va_list` record and the SVE typedefs).
pub(super) fn sugar_count() -> usize {
    usize::from(SUGAR_SVE) + sizeless().count()
}

/// The class `__builtin_va_list` names in `s` (through its array type on
/// x86-64).
fn va_record(s: &Sema<'_, '_>) -> Option<DeclId> {
    let types = &s.ast.types;
    let mut q = types.canonical(s.va_list_type?);
    if let TypeView::ConstantArray(a) = types.view(q.ty()) {
        q = types.canonical(a.elem);
    }
    match types.view(q.ty()) {
        TypeView::Record(d) if q.quals_bits() == 0 => Some(s.ast.first(d)),
        _ => None,
    }
}

impl Writer<'_, '_, '_> {
    /// Whether class `d` (a first declaration) is the one
    /// `__builtin_va_list` names.
    pub(super) fn is_va_record(&self, d: DeclId) -> bool {
        self.s.ast.decls.get(d).has(DeclFlags::IMPLICIT) && va_record(self.s) == Some(d)
    }

    /// The builtin typedef number of `t`, the type of typedef `d`, when
    /// `d` is a predefined SVE typedef.
    pub(super) fn sve_sugar(&self, d: DeclId, t: TypeId) -> Option<u8> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        if !n.has(DeclFlags::IMPLICIT)
            || n.kind() != DeclKind::Typedef
            || n.ctx() != Some(ast.tu)
            || n.ty().is_none_or(|q| q.ty() != t || q.quals_bits() != 0)
        {
            return None;
        }
        let TypeView::Typedef { underlying, .. } = ast.types.view(t) else { return None };
        let j = sizeless().position(|b| b.qual_type() == underlying)?;
        let name = ast.names.view(n.name());
        let named = matches!(name, DeclNameView::Identifier(s)
            if ast.symbols().as_str(s) == sizeless().nth(j)?.name());
        if !named {
            return None;
        }
        u8::try_from(usize::from(SUGAR_SVE) + j).ok()
    }
}

impl Sema<'_, '_> {
    /// The importer's SVE typedef of builtin typedef number `k`
    /// ([`SUGAR_SVE`] on): its own implicit typedef of that name, or the
    /// builtin type itself when it declares none.
    pub(super) fn image_predef_sugar(&mut self, k: u8) -> QualType {
        if k == SUGAR_VA_RECORD {
            return va_record(self)
                .and_then(|d| self.ast.decls.get(d).ty())
                .unwrap_or(QualType::ERROR);
        }
        let Some(b) = sizeless().nth(usize::from(k.saturating_sub(SUGAR_SVE))) else {
            return QualType::ERROR;
        };
        let name = DeclName::ident(self.ast.symbols().intern(b.name()));
        let found = self.ast.lookup_local(self.ast.tu, name);
        (0..found.len())
            .filter_map(|i| found.get(i).map(|(d, _)| d))
            .find(|&d| {
                let n = self.ast.decls.get(d);
                n.has(DeclFlags::IMPLICIT) && n.kind() == DeclKind::Typedef
            })
            .and_then(|d| self.ast.decls.get(d).ty())
            .unwrap_or_else(|| b.qual_type())
    }
}

/// The `Implicit` record kind standing for the class `__builtin_va_list`
/// names (WP-1369): its implicit special members' records are in it.
pub(super) const VA_RECORD_KIND: u8 = 11;

/// Whether record `r` is the [`VA_RECORD_KIND`] stub.
pub(super) fn is_va_stub(img: &Image, r: u32) -> bool {
    img.decls
        .get(r as usize)
        .is_some_and(|d| d.payload == Payload::Implicit { kind: VA_RECORD_KIND })
}

/// Whether record `i`, a [`VA_RECORD_KIND`] stub, is the image's only one.
pub(super) fn va_stub_ok(img: &Image, i: usize) -> bool {
    let stub = |d: &DeclRec| d.payload == Payload::Implicit { kind: VA_RECORD_KIND };
    let d = &img.decls[i];
    d.prev == NONE
        && d.name == NONE
        && d.ctx == NONE
        && d.lexical == NONE
        && img.decls.iter().position(stub) == Some(i)
        && img.decls.iter().filter(|d| stub(d)).count() == 1
}

impl Writer<'_, '_, '_> {
    /// The record of `d` when it is an implicitly declared special member
    /// of the class `__builtin_va_list` names (WP-1369: its copy
    /// constructor, a `va_list` passed by value on `AArch64`): an
    /// `Implicit` record of its special kind in the [`VA_RECORD_KIND`]
    /// stub, which the importer resolves in its own class, declaring the
    /// member as sema did on use ([class.copy.ctor]/6).
    pub(super) fn va_member(&mut self, d: DeclId) -> Option<u32> {
        let s = self.s;
        let ast = &s.ast;
        let n = ast.decls.get(d);
        let c = ast.first(n.ctx()?);
        if !n.has(DeclFlags::IMPLICIT) || !self.is_va_record(c) {
            return None;
        }
        let k = s.special_kind_of(d)?;
        let stub = if let Some(&r) = self.recs.get(&c) {
            r
        } else {
            let r = u32::try_from(self.img.decls.len()).ok()?;
            self.img.decls.push(DeclRec {
                flags: 0,
                owner: 0,
                access: 0,
                name: NONE,
                ctx: NONE,
                lexical: NONE,
                loc: NONE,
                prev: NONE,
                payload: Payload::Implicit { kind: VA_RECORD_KIND },
            });
            self.recs.insert(c, r);
            r
        };
        let i = u32::try_from(self.img.decls.len()).ok()?;
        self.img.decls.push(DeclRec {
            flags: n.flags().bits() & DECL_FLAGS,
            owner: 0,
            access: 0,
            name: NONE,
            ctx: stub,
            lexical: NONE,
            loc: NONE,
            prev: NONE,
            payload: Payload::Implicit { kind: k as u8 },
        });
        self.recs.insert(d, i);
        Some(i)
    }
}

impl Sema<'_, '_> {
    /// The importer's class `__builtin_va_list` names (the translation
    /// unit when it declares none).
    pub(super) fn image_va_record(&self) -> DeclId {
        va_record(self).unwrap_or(self.ast.tu)
    }
}
