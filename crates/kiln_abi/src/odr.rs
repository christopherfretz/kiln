//! Facts about classes for the link-time one-definition-rule checks of the
//! mortar linker (spec/codegen.md "Link-time metadata (`.mason.meta`)",
//! kind 3; mortar `spec/odr.md` O-17, O-18; WP-1566).
//!
//! [`NotTuLocal`] is O-17's **syntactic** not-TU-local test: a
//! conservative under-approximation of [basic.link]/13-15 that needs no
//! TU-locality engine. It is made on the AST, never by reading a mangled
//! name, and it can only say "no" too often (a miss), never "yes" for a
//! type whose `<type>` encoding has a per-TU component (`_GLOBAL__N`, a
//! local-entity `Z...E`, an unnamed `Ut`/`Ul`), which is what lets a type
//! row be compared across units without a false finding.

use std::fmt::Write as _;

use kiln_ast::attr::AttrKind;
use kiln_ast::class::{VTableComponent, VTableLayout};
use kiln_ast::decl::{DeclKind, NamespaceData};
use kiln_ast::types::{ArgView, BuiltinType, RecordLayout, TypeView};
use kiln_ast::{Ast, DeclId, QualType};
use kiln_source::fx::FxHashMap;

use crate::layout::props;
use crate::mangle::{self, AnonIds, Structor};

/// O-17's syntactic test, memoized per class or enumeration and per
/// canonical type, so a TU pays one walk per distinct type.
///
/// A class or enumeration **passes** when all of these hold:
///
/// 1. it has a name of its own: no anonymous struct or union, unnamed
///    enumeration, closure type, or unnamed class with a typedef name for
///    linkage purposes (and no `[[clang::internal_linkage]]`);
/// 2. every enclosing scope is a named namespace (inline namespaces
///    included; linkage specifications and `export` are looked through)
///    or a class that itself passes: no unnamed namespace at any depth,
///    no enclosing function or lambda;
/// 3. for a class template specialization: every template argument is a
///    type built only from fundamental types and passing classes and
///    enumerations (through pointers, references, arrays, function types,
///    member pointers and cv-qualifiers), or a value of integral,
///    enumeration (a passing one) or `std::nullptr_t` type; a pack
///    passes when each of its elements does. A value that names or
///    points to an entity, a floating-point or class-type value, and a
///    template template argument fail. (The template itself has the
///    specialization's name and scopes, so 1-2 already cover it, and an
///    enclosing class template specialization's arguments are clause 3
///    of that class.)
#[derive(Debug)]
pub struct NotTuLocal<'a> {
    ast: &'a Ast,
    tags: FxHashMap<DeclId, bool>,
    types: FxHashMap<(QualType, bool), bool>,
}

impl<'a> NotTuLocal<'a> {
    /// An empty memo over `ast`.
    pub fn new(ast: &'a Ast) -> Self {
        NotTuLocal { ast, tags: FxHashMap::default(), types: FxHashMap::default() }
    }

    /// Whether the class or enumeration `d` (any declaration of it)
    /// passes the test. Anything else (a partial specialization, a
    /// template pattern's injected class, a non-tag) does not.
    pub fn tag(&mut self, d: DeclId) -> bool {
        let first = self.ast.first(d);
        if let Some(&r) = self.tags.get(&first) {
            return r;
        }
        // No well-formed type reaches itself through its own scopes or
        // arguments; a cycle would fail, which is the safe answer.
        self.tags.insert(first, false);
        let r = self.compute(first);
        self.tags.insert(first, r);
        r
    }

    /// Whether type `q` is built only from fundamental types and passing
    /// classes and enumerations, through pointers, references, arrays
    /// (of known or unknown bound), function types, member pointers and
    /// cv-qualifiers (clause 3's rule for a type argument).
    pub fn ty(&mut self, q: QualType) -> bool {
        self.walk(q, false)
    }

    /// [`Self::ty`], also through the built-in compound types that have
    /// no per-TU component in their encoding (`_Complex`, `_Atomic`,
    /// vector types): the rule for a data member's or a virtual
    /// function's types inside a passing class's layout.
    pub fn member_ty(&mut self, q: QualType) -> bool {
        self.walk(q, true)
    }

    fn walk(&mut self, q: QualType, compound: bool) -> bool {
        let ast = self.ast;
        let t = ast.types.canonical(q);
        match ast.types.canonical_view(t) {
            TypeView::Builtin(_) | TypeView::BitInt { .. } => true,
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                self.walk(p, compound)
            }
            TypeView::ConstantArray(_) | TypeView::IncompleteArray(_) => {
                ast.types.element(t).is_some_and(|e| self.walk(e, compound))
            }
            TypeView::Record(d) | TypeView::Enum(d) => self.tag(d),
            TypeView::Complex(e) | TypeView::Atomic(e) if compound => self.walk(e, compound),
            TypeView::Vector { elem, .. } if compound => self.walk(elem, compound),
            TypeView::MemberPointer { .. } | TypeView::Function(_) => {
                let key = (t, compound);
                if let Some(&r) = self.types.get(&key) {
                    return r;
                }
                self.types.insert(key, false);
                let r = match ast.types.canonical_view(t) {
                    TypeView::MemberPointer { pointee, class } => {
                        self.walk(pointee, compound) && self.walk(QualType::unqual(class), compound)
                    }
                    TypeView::Function(p) => {
                        self.walk(p.ret(), compound)
                            && ast.types.params(&p).iter().all(|&x| self.walk(x, compound))
                    }
                    _ => false,
                };
                self.types.insert(key, r);
                r
            }
            _ => false,
        }
    }

    /// Clause 2 alone: every enclosing scope of `d` is a named namespace
    /// (linkage specifications and `export` looked through) or a passing
    /// class. Also what an unnamed member class must satisfy to be
    /// described by its contents inside its class's layout.
    pub fn scopes(&mut self, d: DeclId) -> bool {
        let ast = self.ast;
        let mut c = ast.decls.get(d).ctx();
        while let Some(x) = c {
            match ast.decls.get(x).kind() {
                DeclKind::TranslationUnit => return true,
                DeclKind::Namespace => {
                    if ast
                        .decls
                        .namespace(x)
                        .is_none_or(|ns| ns.flags & NamespaceData::ANONYMOUS != 0)
                    {
                        return false;
                    }
                    c = ast.decls.get(x).ctx();
                }
                DeclKind::LinkageSpec | DeclKind::Export => c = ast.decls.get(x).ctx(),
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization => {
                    return self.tag(x);
                }
                _ => return false,
            }
        }
        true
    }

    fn compute(&mut self, d: DeclId) -> bool {
        let ast = self.ast;
        let n = ast.decls.get(d);
        if !matches!(
            n.kind(),
            DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization | DeclKind::Enum
        ) {
            return false;
        }
        // (1) A name of its own.
        if n.name().is_empty() || ast.closure_lambda(d).is_some() {
            return false;
        }
        if ast.has_internal_linkage_attr(d) {
            return false;
        }
        // (2) Every enclosing scope a named namespace or a passing class.
        if !self.scopes(d) {
            return false;
        }
        // (3) A specialization's arguments.
        match ast.spec_info(d) {
            Some((_, s)) => {
                let args = ast.types.canonical_args(s.args);
                self.args(args)
            }
            // A specialization kind without its record is not provable.
            None => n.kind() != DeclKind::ClassTemplateSpecialization,
        }
    }

    pub(crate) fn args(&mut self, list: kiln_ast::ids::ArgListId) -> bool {
        let ast = self.ast;
        ast.types.args(list).iter().all(|a| match a.view() {
            ArgView::Type(q) => self.ty(q),
            ArgView::Value { ty, .. } => {
                let t = ast.types.canonical(ty);
                match ast.types.canonical_view(t) {
                    TypeView::Enum(e) => self.tag(e),
                    TypeView::Builtin(BuiltinType::NullPtr) => true,
                    _ => ast.types.is_integral(t),
                }
            }
            ArgView::Pack(l) => self.args(l),
            ArgView::Null
            | ArgView::Template(_)
            | ArgView::TemplateExpansion { .. }
            | ArgView::Expr { .. }
            | ArgView::Profile(_) => false,
        })
    }
}

/// Whether `d` or an enclosing declaration is a template pattern or a
/// partial specialization (a templated entity, [temp.pre]/8): such a
/// class has no layout of its own and gets no type row.
pub fn is_templated(ast: &Ast, d: DeclId) -> bool {
    let mut c = Some(d);
    while let Some(x) = c {
        if ast.decls.get(x).kind() == DeclKind::ClassTemplatePartialSpecialization
            || matches!(ast.templ_link(x).get(), Some((kiln_ast::decl::TemplLinkKind::Pattern, _)))
        {
            return true;
        }
        c = ast.decls.get(x).ctx();
    }
    false
}

/// The canonical byte serialization of a class's layout that the
/// link-time metadata's `layout` field hashes (mortar `spec/odr.md` O-18;
/// spec/codegen.md "Link-time metadata (`.mason.meta`)", "The layout
/// hash", which is the field-by-field contract: a later kiln must
/// reproduce these bytes). Every integer is little-endian; a string is
/// its `u32` length and its bytes.
///
/// `layouts` holds the layouts the TU computed (the class's own and those
/// of its unnamed member classes); `vtable` is the class's vtable group
/// when it is dynamic. `None` when the class cannot be described without a
/// per-TU component or a fact the caller does not have: a base or a
/// virtual function's type that fails [`NotTuLocal`], a data member whose
/// type is neither passing nor an unnamed class or enumeration of this
/// class, a missing layout or vtable, a size past `u32` or an alignment
/// past `u16` (the row's fields).
#[derive(Debug)]
pub struct LayoutSerializer<'a, 'm> {
    /// The AST.
    pub ast: &'a Ast,
    /// The module's mangling state (nothing that passes is numbered).
    pub anon: &'m mut AnonIds,
    /// The memoized syntactic test.
    pub test: &'m mut NotTuLocal<'a>,
    /// The layouts the TU computed, by first declaration.
    pub layouts: &'m FxHashMap<DeclId, &'m RecordLayout>,
    /// `Some` to collect the kind 4 items of the type being serialized
    /// (`-fmason-metadata=detail`; [`Self::class`] and
    /// [`Self::enumeration`] start from an empty list): built by the same
    /// walk that writes the hashed bytes, so the two cannot disagree.
    /// `None` costs nothing.
    pub details: Option<Vec<DetailItem>>,
}

/// One kind 4 item (mortar `spec/odr.md` O-19 as fixed in its maintainers'
/// answer to WP-1566): `{u32 name; u32 type; u64 value; u32 width; u32
/// flags}` before the strings are laid out. Items appear in the order the
/// layout hash consumes the same facts, one per fact that has an item
/// kind, so the first differing item is the first difference.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct DetailItem {
    /// The member's or enumerator's name, a vtable slot's final overrider
    /// (its mangled name); empty when unused.
    pub name: String,
    /// The member's type as the hash spells it, a base's `<type>`, an
    /// enumeration's underlying type; empty when unused.
    pub ty: String,
    /// An offset in bits, a slot index or an enumerator's value.
    pub value: u64,
    /// A bit-field's width (0 otherwise).
    pub width: u32,
    /// Bits 0-3 the item kind (`ITEM_*`), bit 4 [`ITEM_BITFIELD`], bit 5
    /// [`ITEM_VIRTUAL`].
    pub flags: u32,
}

/// Item kind 0: a non-static data member (name, type, bit offset, width).
pub const ITEM_MEMBER: u32 = 0;
/// Item kind 1: a base (type = its `<type>`, bit offset, [`ITEM_VIRTUAL`]).
pub const ITEM_BASE: u32 = 1;
/// Item kind 2: the vptr (bit offset).
pub const ITEM_VPTR: u32 = 2;
/// Item kind 3: a vtable slot (name = the final overrider's mangled name,
/// value = the slot's index in the group as hashed).
pub const ITEM_SLOT: u32 = 3;
/// Item kind 4: an enumerator (name, value).
pub const ITEM_ENUMERATOR: u32 = 4;
/// Item kind 5: an enumeration's underlying type (type).
pub const ITEM_UNDERLYING: u32 = 5;
/// A data member item's bit-field flag.
pub const ITEM_BITFIELD: u32 = 1 << 4;
/// A base item's virtual flag.
pub const ITEM_VIRTUAL: u32 = 1 << 5;

/// The format version of [`LayoutSerializer`]'s bytes, part of the
/// scheme name written with the rows (`odr.scheme`).
pub const LAYOUT_SERIALIZATION_VERSION: u32 = 1;

fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&u32::try_from(s.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

impl LayoutSerializer<'_, '_> {
    /// Appends a kind 4 item when they are collected.
    fn item(&mut self, make: impl FnOnce() -> DetailItem) {
        if let Some(d) = &mut self.details {
            d.push(make());
        }
    }

    fn ident(&self, d: DeclId) -> String {
        let ast = self.ast;
        ast.decls
            .get(d)
            .name()
            .as_ident()
            .map(|s| ast.symbols().as_str(s).to_owned())
            .unwrap_or_default()
    }

    /// The bytes of the definition `def` of a complete enumeration (the
    /// layout hash's enumeration form, spec/codegen.md "The layout hash"):
    /// `E`; u8 bit 0 scoped, bit 1 fixed underlying type; the underlying
    /// type's `<type>` encoding; the enumerator count u32, then per
    /// enumerator in declaration order its name and its value u64 (the
    /// low 64 bits of its two's-complement value). `None` without an
    /// underlying type or with an enumerator without a value.
    pub fn enumeration(&mut self, def: DeclId) -> Option<Vec<u8>> {
        use kiln_ast::decl::EnumData;
        let ast = self.ast;
        if let Some(d) = &mut self.details {
            d.clear();
        }
        let e = ast.decls.enumeration(def)?;
        let int = e.int_type?;
        let mut out = Vec::with_capacity(64);
        out.push(b'E');
        let mut bits = 0u8;
        if e.flags & EnumData::SCOPED != 0 {
            bits |= 1;
        }
        if e.flags & EnumData::FIXED != 0 {
            bits |= 2;
        }
        out.push(bits);
        let enc = mangle::mangle_type(ast, self.anon, int);
        put_str(&mut out, &enc);
        self.item(|| DetailItem { ty: enc, flags: ITEM_UNDERLYING, ..DetailItem::default() });
        let members: Vec<DeclId> = ast.decls.members(def).collect();
        out.extend_from_slice(&u32::try_from(members.len()).ok()?.to_le_bytes());
        for m in members {
            let v = ast.decls.enumerator(m)?.value?;
            let value = match ast.consts.scalar(v)? {
                kiln_ast::consts::Scalar::Int { bits, signed: true, .. } => bits.to_i128()? as u64,
                kiln_ast::consts::Scalar::Int { bits, .. } => bits.to_u128()? as u64,
                kiln_ast::consts::Scalar::Bool { value, .. } => u64::from(value),
                _ => return None,
            };
            let name = self.ident(m);
            put_str(&mut out, &name);
            out.extend_from_slice(&value.to_le_bytes());
            self.item(|| DetailItem {
                name,
                value,
                flags: ITEM_ENUMERATOR,
                ..DetailItem::default()
            });
        }
        Some(out)
    }

    /// The `<type>` encoding of class or enumeration `d` (what `_ZTS`
    /// names; the row's `name`, and FNV-1a 64 of it its `key`).
    pub fn type_name(&mut self, d: DeclId) -> String {
        mangle::mangle_class_special(self.ast, self.anon, "", d)
    }

    /// The bytes of class definition `def`'s layout, `None` when it gets
    /// no row (above).
    pub fn class(&mut self, def: DeclId, vtable: Option<&VTableLayout>) -> Option<Vec<u8>> {
        if let Some(d) = &mut self.details {
            d.clear();
        }
        let mut out = Vec::with_capacity(128);
        self.record(def, vtable, &mut out)?;
        Some(out)
    }

    fn record(
        &mut self,
        def: DeclId,
        vtable: Option<&VTableLayout>,
        out: &mut Vec<u8>,
    ) -> Option<()> {
        let ast = self.ast;
        let first = ast.first(def);
        let l = *self.layouts.get(&first)?;
        let desc = props::class_desc(ast, def);
        // Kind, size, alignment.
        out.push(if desc.is_union { b'U' } else { b'C' });
        out.extend_from_slice(&l.size.to_le_bytes());
        out.extend_from_slice(&l.align.to_le_bytes());
        // The class attributes of O-10's list that are a class's property.
        let mut attrs = 0u8;
        if desc.packed {
            attrs |= 1;
        }
        if desc.ms_struct {
            attrs |= 2;
        }
        if props::has_attr(ast, def, AttrKind::GCC_STRUCT) {
            attrs |= 4;
        }
        if props::has_attr(ast, def, AttrKind::TRIVIAL_ABI) {
            attrs |= 8;
        }
        out.push(attrs);
        // Direct bases in declaration order: key, offset, virtuality.
        let bases = props::bases(ast, def);
        out.extend_from_slice(&u32::try_from(bases.len()).ok()?.to_le_bytes());
        for b in &bases {
            let c = ast.first(b.class?);
            if !self.test.tag(c) {
                return None;
            }
            let table = if b.is_virtual { &l.vbases } else { &l.bases };
            let offset = table.iter().find(|&&(x, _)| ast.first(x) == c)?.1;
            let name = self.type_name(c);
            out.extend_from_slice(&fnv1a64(name.as_bytes()).to_le_bytes());
            out.extend_from_slice(&offset.to_le_bytes());
            out.push(u8::from(b.is_virtual));
            let flags = ITEM_BASE | if b.is_virtual { ITEM_VIRTUAL } else { 0 };
            self.item(|| DetailItem {
                ty: name,
                value: offset.wrapping_mul(8),
                flags,
                ..DetailItem::default()
            });
        }
        // Every virtual base, in the layout's order, with its offset in
        // this class.
        out.extend_from_slice(&u32::try_from(l.vbases.len()).ok()?.to_le_bytes());
        for &(v, offset) in &l.vbases {
            let v = ast.first(v);
            if !self.test.tag(v) {
                return None;
            }
            let name = self.type_name(v);
            out.extend_from_slice(&fnv1a64(name.as_bytes()).to_le_bytes());
            out.extend_from_slice(&offset.to_le_bytes());
            self.item(|| DetailItem {
                ty: name,
                value: offset.wrapping_mul(8),
                flags: ITEM_BASE | ITEM_VIRTUAL,
                ..DetailItem::default()
            });
        }
        // Non-static data members in declaration order.
        let fields = props::fields(ast, def);
        if fields.len() != l.fields.len() {
            return None;
        }
        out.extend_from_slice(&u32::try_from(fields.len()).ok()?.to_le_bytes());
        for (f, &bit) in fields.iter().zip(l.fields.iter()) {
            out.extend_from_slice(&bit.to_le_bytes());
            let width = f.bit_width.map_or(u32::MAX, |w| u32::try_from(w).unwrap_or(u32::MAX - 1));
            out.extend_from_slice(&width.to_le_bytes());
            let item = self.details.is_some().then(|| DetailItem {
                name: self.ident(f.decl),
                value: bit,
                width: if f.bit_width.is_some() { width } else { 0 },
                flags: ITEM_MEMBER | if f.bit_width.is_some() { ITEM_BITFIELD } else { 0 },
                ty: String::new(),
            });
            self.member_type(def, f.ty, out, item)?;
        }
        // The vtable pointer.
        match l.vptr {
            Some(o) => {
                out.push(1);
                out.extend_from_slice(&o.to_le_bytes());
                self.item(|| DetailItem {
                    value: o.wrapping_mul(8),
                    flags: ITEM_VPTR,
                    ..DetailItem::default()
                });
            }
            None => out.push(0),
        }
        // The vtable group's shape, without its RTTI slots.
        let dynamic = ast.decls.record(def).is_some_and(|r| r.defn.is_dynamic());
        match (dynamic, vtable) {
            (false, _) => out.extend_from_slice(&0u32.to_le_bytes()),
            (true, None) => return None,
            (true, Some(vt)) => self.vtable(vt, out)?,
        }
        Some(())
    }

    /// A data member's type: `T` and its `<type>` encoding when it
    /// passes; `A` with the bound for an array of an unnamed class or
    /// enumeration; `S` and the nested layout for an unnamed class of
    /// this class (an anonymous union, `struct { int x; } m;`), `N` and
    /// the underlying type's encoding for an unnamed enumeration.
    ///
    /// `item` is the member's kind 4 item (when they are collected): its
    /// type string is the encoding (`T`), `A<bound>_` and the element's
    /// string (`A`), the underlying type's encoding (`N`), or empty (`S`:
    /// the nested class's own items follow it).
    fn member_type(
        &mut self,
        owner: DeclId,
        q: QualType,
        out: &mut Vec<u8>,
        mut item: Option<DetailItem>,
    ) -> Option<()> {
        let ast = self.ast;
        if self.test.member_ty(q) {
            out.push(b'T');
            let enc = mangle::mangle_type(ast, self.anon, q);
            put_str(out, &enc);
            if let (Some(mut it), Some(d)) = (item, &mut self.details) {
                it.ty.push_str(&enc);
                d.push(it);
            }
            return Some(());
        }
        let t = ast.types.canonical(q);
        match ast.types.canonical_view(t) {
            TypeView::ConstantArray(a) => {
                out.push(b'A');
                out.extend_from_slice(&a.size.to_le_bytes());
                if let Some(it) = &mut item {
                    let _ = write!(it.ty, "A{}_", a.size);
                }
                self.member_type(owner, ast.types.element(t)?, out, item)
            }
            TypeView::Record(d) | TypeView::Enum(d) => {
                let n = ast.decls.get(d);
                if !n.name().is_empty()
                    || ast.closure_lambda(d).is_some()
                    || ast.first(n.ctx()?) != ast.first(owner)
                    || !self.test.scopes(d)
                {
                    return None;
                }
                let def = ast.definition(d)?;
                if n.kind() == DeclKind::Enum {
                    let int = ast.decls.enumeration(def)?.int_type?;
                    out.push(b'N');
                    let enc = mangle::mangle_type(ast, self.anon, int);
                    put_str(out, &enc);
                    if let (Some(mut it), Some(d)) = (item, &mut self.details) {
                        it.ty.push_str(&enc);
                        d.push(it);
                    }
                    return Some(());
                }
                if ast.decls.record(def).is_some_and(|r| r.defn.is_dynamic()) {
                    return None;
                }
                out.push(b'S');
                if let (Some(it), Some(d)) = (item, &mut self.details) {
                    d.push(it);
                }
                self.record(def, None, out)
            }
            _ => None,
        }
    }

    /// Slot count and slots (O-18: per slot the final overrider's
    /// mangled name): `c`/`b`/`o` a vcall, vbase or offset-to-top offset
    /// (`i64`), `f` a virtual function, `d`/`D` the complete and deleting
    /// destructors, `u` an unused slot, each with the overrider's name;
    /// `p` a pure virtual, `x` a deleted one. RTTI slots are left out.
    fn vtable(&mut self, vt: &VTableLayout, out: &mut Vec<u8>) -> Option<()> {
        let ast = self.ast;
        let slots: Vec<&VTableComponent> =
            vt.components.iter().filter(|c| !matches!(c, VTableComponent::Rtti(_))).collect();
        out.extend_from_slice(&u32::try_from(slots.len()).ok()?.to_le_bytes());
        for (index, c) in slots.into_iter().enumerate() {
            let slot = |name: String| DetailItem {
                name,
                value: index as u64,
                flags: ITEM_SLOT,
                ..DetailItem::default()
            };
            let (tag, m, v) = match *c {
                VTableComponent::VCallOffset(o) => (b'c', None, o),
                VTableComponent::VBaseOffset(o) => (b'b', None, o),
                VTableComponent::OffsetToTop(o) => (b'o', None, o),
                VTableComponent::Rtti(_) => continue,
                VTableComponent::Func(m) => (b'f', Some((m, Structor::Complete)), 0),
                VTableComponent::CompleteDtor(m) => (b'd', Some((m, Structor::Complete)), 0),
                VTableComponent::DeletingDtor(m) => (b'D', Some((m, Structor::Deleting)), 0),
                VTableComponent::UnusedFunc(m) => (b'u', Some((m, Structor::Complete)), 0),
            };
            let Some((m, s)) = m else {
                out.push(tag);
                out.extend_from_slice(&v.to_le_bytes());
                continue;
            };
            let fd = ast.decls.function(m);
            if fd.is_some_and(|f| f.spec.is_pure()) {
                out.push(b'p');
                self.item(|| slot("__cxa_pure_virtual".to_owned()));
                continue;
            }
            if ast.redecls_of(m).any(|r| ast.decls.function(r).is_some_and(|f| f.spec.is_deleted()))
            {
                out.push(b'x');
                self.item(|| slot("__cxa_deleted_virtual".to_owned()));
                continue;
            }
            // The overrider's class is this class or a base (they pass);
            // its name has no per-TU part when its type has none.
            let class = ast.decls.get(m).ctx()?;
            if !self.test.tag(class)
                || !ast.decls.get(m).ty().is_some_and(|t| self.test.member_ty(t))
            {
                return None;
            }
            out.push(tag);
            let name = mangle::mangle_structor(ast, self.anon, m, s);
            put_str(out, &name);
            self.item(|| slot(name));
        }
        Some(())
    }
}

/// FNV-1a 64 (mortar `spec/meta.md` MT-13): the row's `key` over the
/// `<type>` encoding and its `layout` over [`LayoutSerializer`]'s bytes.
#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The FNV-1a 128-bit hash of `s` (the `kiln-hashed` prototype).
#[must_use]
pub fn fnv1a_128(s: &str) -> u128 {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    s.bytes().fold(OFFSET, |h, b| (h ^ u128::from(b)).wrapping_mul(PRIME))
}
