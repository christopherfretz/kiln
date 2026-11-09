//! Keys for evaluations inside template instantiations (spec/modules.md
//! §11.11 "Instantiation keys", WP-1046).
//!
//! An evaluation inside an instantiation is keyed by the instantiated
//! entity of the innermost instantiation frame, written as a byte string
//! that names no `DeclId`: the template and its canonical arguments for a
//! specialization, the enclosing instantiated entity and the pattern
//! member for a member of a specialization (or any other instantiated
//! declaration), and kind, name and replay-stable position for a
//! declaration that was not instantiated. Types are written structurally
//! (canonical, with their cv bits); values as part 1's carried values.
//! The producing compile and the importer's replay build the same bytes
//! for the same specialization, so the bytes are compared as they are.
//! A key that cannot be formed (a dependent or unsupported type, an
//! implicit declaration, a position the replay does not reproduce)
//! leaves the evaluation uncarried.
//!
//! The same value encoding carries pointer, reference and reflection
//! results ([`value_bytes`]); the importer resolves the entities in it
//! back to its own declarations (`value.rs`). Objects without a
//! declaration of their own are written by what the replay rebuilds them
//! from: a string literal and a label address by the position of the
//! expression whose evaluation made them, a lifetime-extended temporary
//! by its extending declaration and mangling index (WP-1160).

use kiln_ast::consts::{ConstValue, MemberKind, PtrBase, Scalar};
use kiln_ast::decl::DeclFlags;
use kiln_ast::ids::{AnnId, ArgListId, ExprId};
use kiln_ast::names::DeclNameKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{ArgView, ExcSpec, TemplateName, TypeView};
use kiln_ast::{Ast, ConstValueId, DeclId, QualType};
use kiln_oven::Oven;
use kiln_source::{FileId, Loc, SourceManager};

use super::{
    Rd, Res, Skip, TAG_AGGREGATE, TAG_AGGREGATE_EX, TAG_MEMBER_NULL, TAG_MEMBER_POINTER,
    TAG_POINTER, TAG_REFLECTION, TAG_TYPED, bad, carried_value, loc_key, put_bytes, put_u32,
    put_value, put_var, read_plain,
};

/// The deepest nesting a key may have (entities, argument lists and
/// types together); deeper keys are not formed.
const MAX_DEPTH: u32 = 48;

/// The longest key formed, in bytes.
const MAX_KEY: usize = 1 << 14;

/// Entity tags.
pub(super) const ENT_SPEC: u8 = b'S';
pub(super) const ENT_MEMBER: u8 = b'M';
pub(super) const ENT_PATTERN: u8 = b'P';
pub(super) const ENT_DECL: u8 = b'D';
/// Top level only: a substitution frame ([`subst_key`]).
const ENT_SUBST: u8 = b'F';
/// The translation unit (the global namespace).
pub(super) const ENT_TU: u8 = b'U';

/// Argument tags.
const ARG_TYPE: u8 = b'T';
const ARG_VALUE: u8 = b'V';
const ARG_TEMPLATE: u8 = b'N';
const ARG_PACK: u8 = b'K';

/// Type tags (after the cv byte).
pub(super) const TY_BUILTIN: u8 = 0;
pub(super) const TY_BITINT: u8 = 1;
pub(super) const TY_POINTER: u8 = 2;
pub(super) const TY_LREF: u8 = 3;
pub(super) const TY_RREF: u8 = 4;
pub(super) const TY_MEMBER_POINTER: u8 = 5;
pub(super) const TY_ARRAY: u8 = 6;
pub(super) const TY_INCOMPLETE_ARRAY: u8 = 7;
pub(super) const TY_FUNCTION: u8 = 8;
pub(super) const TY_RECORD: u8 = 9;
pub(super) const TY_ENUM: u8 = 10;
pub(super) const TY_COMPLEX: u8 = 11;
pub(super) const TY_ATOMIC: u8 = 12;
pub(super) const TY_VECTOR: u8 = 13;

/// Pointer base kinds (after [`TAG_POINTER`] and its flags).
pub(super) const BASE_DECL: u8 = 0;
pub(super) const BASE_FUNCTION: u8 = 1;
pub(super) const BASE_TYPE_INFO: u8 = 2;
/// A template parameter object, by its type and value.
pub(super) const BASE_PARAM_OBJECT: u8 = 3;
/// A subobject of another constant, by its type and value.
pub(super) const BASE_CONST: u8 = 4;
/// A string literal object, by the position of the `StringLiteral` whose
/// evaluation made it (version 4).
pub(super) const BASE_STRING: u8 = 5;
/// A lifetime-extended temporary, by its extending declaration and
/// mangling index, its type and its value if it has one (version 4).
pub(super) const BASE_TEMPORARY: u8 = 6;
/// A label address, by the label and the position of the `&&label`
/// (version 4).
pub(super) const BASE_LABEL: u8 = 7;

/// Type reflection forms (after the reflectee tag 0).
pub(super) const REFL_TYPE: u8 = 0;
pub(super) const REFL_ALIAS: u8 = 1;
/// An alias template specialization: the alias template, its canonical
/// arguments and the canonical aliased type (version 4).
pub(super) const REFL_ALIAS_SPEC: u8 = 2;

/// Data member specification flags (after the reflectee tag 9).
pub(super) const DMS_NAME: u8 = 1;
pub(super) const DMS_ALIGN: u8 = 2;
pub(super) const DMS_WIDTH: u8 = 4;
pub(super) const DMS_NUA: u8 = 8;

/// The writer of one key. `file` maps a file to its index in the
/// section's path table (`None`: a file the replay does not reproduce).
struct KeyCx<'a, F> {
    ast: &'a Ast,
    /// The Oven, for the data member specifications a value names.
    oven: Option<&'a Oven>,
    sm: &'a SourceManager,
    file: F,
    out: Vec<u8>,
    /// The longest output formed.
    limit: usize,
    /// Every type written inside a value must already be canonical (the
    /// importer rebuilds the value from the canonical form, so a sugared
    /// type would make it a different value).
    exact: bool,
    /// Why the value could not be written, when a part of it that has a
    /// reason of its own failed first (the `-v` note).
    why: Option<Skip>,
}

impl<'a, F> KeyCx<'a, F> {
    fn key(ast: &'a Ast, sm: &'a SourceManager, file: F, out: Vec<u8>) -> Self {
        KeyCx { ast, oven: None, sm, file, out, limit: MAX_KEY, exact: false, why: None }
    }

    /// `r`, noting `why` when it failed and nothing more specific was.
    fn or_why(&mut self, r: Option<()>, why: Skip) -> Option<()> {
        if r.is_none() {
            self.why.get_or_insert(why);
        }
        r
    }
}

/// The key of instantiated entity `d`, or `None` when it cannot be
/// formed or `d` is not an instantiated entity (its key would not name
/// the arguments the evaluation was substituted with).
pub(super) fn entity_key(
    ast: &Ast,
    sm: &SourceManager,
    file: impl FnMut(FileId) -> Option<u32>,
    d: DeclId,
) -> Option<Box<[u8]>> {
    let mut cx = KeyCx::key(ast, sm, file, Vec::new());
    cx.ent(d, 0)?;
    (cx.out[0] != ENT_DECL && cx.out[0] != ENT_TU).then(|| cx.out.into_boxed_slice())
}

/// The entity bytes of any declaration `d` (as a value names it: the
/// importer's check that a declaration it resolved is the one named).
pub(super) fn decl_key(
    ast: &Ast,
    sm: &SourceManager,
    file: impl FnMut(FileId) -> Option<u32>,
    d: DeclId,
) -> Option<Box<[u8]>> {
    let mut cx = KeyCx::key(ast, sm, file, Vec::new());
    cx.ent(d, 0)?;
    Some(cx.out.into_boxed_slice())
}

/// The full encoding of value `v` (tag first), when it can be written
/// and every type in it is canonical; else why not, when a part with a
/// reason of its own failed.
pub(super) fn value_bytes(
    ast: &Ast,
    oven: Option<&Oven>,
    sm: &SourceManager,
    file: impl FnMut(FileId) -> Option<u32>,
    v: ConstValueId,
    typed: bool,
) -> Result<Box<[u8]>, Option<Skip>> {
    let mut cx =
        KeyCx { ast, oven, sm, file, out: Vec::new(), limit: usize::MAX, exact: true, why: None };
    let ok = (|| {
        if typed {
            cx.out.push(TAG_TYPED);
            cx.vty(ast.consts.ty(v), 0)?;
        }
        cx.value(v, 0)
    })();
    match ok {
        Some(()) => Ok(cx.out.into_boxed_slice()),
        None => Err(cx.why),
    }
}

/// The length of the entity at the start of `b` (bytes a checked section
/// holds).
pub(super) fn ent_len(b: &[u8]) -> Option<usize> {
    let mut ck = Ck { r: Rd(b), npaths: usize::MAX };
    ck.ent(0).ok()?;
    Some(b.len() - ck.r.0.len())
}

/// Checks the entry value at the start of `r` (any tag, or a
/// [`TAG_TYPED`] prefix) and consumes it.
pub(super) fn check_value(r: &mut Rd<'_>, npaths: usize) -> Res<()> {
    let mut ck = Ck { r: Rd(r.0), npaths };
    if ck.r.0.first() == Some(&TAG_TYPED) {
        ck.r.u8()?;
        ck.ty(0)?;
    }
    ck.value(0)?;
    r.0 = ck.r.0;
    Ok(())
}

/// The key of a substitution frame of kind `code` into `template` with
/// the innermost arguments `args`, at `entity` (the template itself, or
/// the parameter whose type or default argument is substituted). Only a
/// template whose parameters are its outermost level (depth 0) is keyed:
/// then `args` are all the arguments the substitution uses, and an
/// enclosing specialization's are in the template's own key.
pub(super) fn subst_key(
    ast: &Ast,
    sm: &SourceManager,
    file: impl FnMut(FileId) -> Option<u32>,
    code: u8,
    template: DeclId,
    entity: DeclId,
    args: ArgListId,
) -> Option<Box<[u8]>> {
    let info = ast.template_info(template)?;
    if ast.templates.param_list(info.params).depth != 0 {
        return None;
    }
    let mut cx = KeyCx::key(ast, sm, file, vec![ENT_SUBST, code]);
    cx.ent(template, 1)?;
    cx.ent(entity, 1)?;
    cx.args(ast.types.canonical_args(args), 1)?;
    Some(cx.out.into_boxed_slice())
}

impl<F: FnMut(FileId) -> Option<u32>> KeyCx<'_, F> {
    fn room(&self, depth: u32) -> Option<()> {
        (depth <= MAX_DEPTH && self.out.len() <= self.limit).then_some(())
    }

    fn ent(&mut self, d: DeclId, depth: u32) -> Option<()> {
        self.room(depth)?;
        let ast = self.ast;
        let d = ast.first(d);
        if d == ast.tu {
            self.out.push(ENT_TU);
            return Some(());
        }
        if let Some((_, s)) = ast.spec_info(d) {
            let (template, args) = (s.template, s.args);
            self.out.push(ENT_SPEC);
            self.ent(template, depth + 1)?;
            return self.args(ast.types.canonical_args(args), depth + 1);
        }
        let inst = ast
            .member_spec_info(d)
            .map(|(_, m)| (ENT_MEMBER, m.instantiated_from))
            .or_else(|| ast.templates.instantiated_from_map(d).map(|p| (ENT_PATTERN, p)));
        if let Some((tag, pattern)) = inst {
            let ctx = ast.decls.get(d).ctx()?;
            self.out.push(tag);
            self.ent(ctx, depth + 1)?;
            return self.ent(pattern, depth + 1);
        }
        // A declaration that was not instantiated, inside nothing that
        // was: its position identifies it. An instantiated declaration
        // without a link to its pattern would share its pattern's
        // position with every other instantiation, so it has no key.
        let n = ast.decls.get(d);
        if n.has(DeclFlags::IMPLICIT) {
            return None;
        }
        let mut c = n.ctx();
        for _ in 0..256 {
            let Some(x) = c else { break };
            let x = ast.first(x);
            if ast.spec_info(x).is_some()
                || ast.member_spec_info(x).is_some()
                || ast.templates.instantiated_from_map(x).is_some()
            {
                return None;
            }
            c = ast.decls.get(x).ctx();
        }
        if c.is_some() {
            return None;
        }
        self.out.push(ENT_DECL);
        self.out.push(n.kind() as u8);
        let name = n.name();
        self.out.push(name.kind() as u8);
        match name.kind() {
            DeclNameKind::Identifier => {
                let s = name.as_ident().map_or("", |s| ast.symbols().as_str(s));
                put_bytes(&mut self.out, s.as_bytes());
            }
            DeclNameKind::Operator => self.out.push(name.payload() as u8),
            _ => {}
        }
        self.at(n.loc())
    }

    /// A replay-stable position.
    fn at(&mut self, loc: Loc) -> Option<()> {
        let at = loc_key(self.sm, loc, &mut self.file)?;
        for w in at.0 {
            put_u32(&mut self.out, w);
        }
        Some(())
    }

    /// The position of expression `e` (a string literal's or label
    /// address's origin; the importer finds its own by it, `value.rs`).
    fn expr_at(&mut self, e: ExprId) -> Option<()> {
        let loc = self.ast.exprs.get(e).loc();
        self.at(loc)
    }

    fn args(&mut self, l: ArgListId, depth: u32) -> Option<()> {
        self.room(depth)?;
        let ast = self.ast;
        let args = ast.types.args(l);
        put_u32(&mut self.out, u32::try_from(args.len()).ok()?);
        for a in args {
            match a.view() {
                ArgView::Type(q) => {
                    self.out.push(ARG_TYPE);
                    self.ty(q, depth + 1)?;
                }
                ArgView::Value { ty, value } => {
                    // Inside a value the importer rebuilds the argument
                    // with the value typed as the argument is.
                    if self.exact && ast.consts.ty(value) != ast.types.canonical(ty) {
                        return None;
                    }
                    self.out.push(ARG_VALUE);
                    self.ty(ty, depth + 1)?;
                    self.value(value, depth + 1)?;
                }
                ArgView::Template(n) => {
                    let n = ast.types.canonical_template_name(n);
                    let TemplateName::Template(d) = ast.types.template_name_view(n) else {
                        return None;
                    };
                    self.out.push(ARG_TEMPLATE);
                    self.ent(d, depth + 1)?;
                }
                ArgView::Pack(p) => {
                    self.out.push(ARG_PACK);
                    self.args(p, depth + 1)?;
                }
                _ => return None,
            }
        }
        Some(())
    }

    fn ty(&mut self, q: QualType, depth: u32) -> Option<()> {
        self.room(depth)?;
        let ast = self.ast;
        let types = &ast.types;
        let c = types.canonical(q);
        self.out.push(c.quals_bits());
        let d = depth + 1;
        match types.view(c.ty()) {
            TypeView::Builtin(b) => self.out.extend_from_slice(&[TY_BUILTIN, b as u8]),
            TypeView::BitInt { signed, width } => {
                self.out.extend_from_slice(&[TY_BITINT, u8::from(signed)]);
                put_u32(&mut self.out, width);
            }
            TypeView::Pointer(p) => self.wrap(TY_POINTER, p, d)?,
            TypeView::LValueRef(p) => self.wrap(TY_LREF, p, d)?,
            TypeView::RValueRef(p) => self.wrap(TY_RREF, p, d)?,
            TypeView::MemberPointer { pointee, class } => {
                self.wrap(TY_MEMBER_POINTER, pointee, d)?;
                self.ty(QualType::unqual(class), d)?;
            }
            TypeView::ConstantArray(a) => {
                self.out.push(TY_ARRAY);
                self.out.extend_from_slice(&a.size.to_le_bytes());
                self.ty(a.elem, d)?;
            }
            TypeView::IncompleteArray(e) => self.wrap(TY_INCOMPLETE_ARRAY, e, d)?,
            TypeView::Function(p) => {
                let exc = match types.exc_spec(&p) {
                    ExcSpec::None => 0,
                    ExcSpec::Nothrow => 1,
                    _ => return None,
                };
                self.wrap(TY_FUNCTION, p.ret(), d)?;
                let params = types.params(&p);
                put_u32(&mut self.out, params.len() as u32);
                for &t in params {
                    self.ty(t, d)?;
                }
                self.out.extend_from_slice(&p.info().bits().to_le_bytes());
                self.out.push(exc);
            }
            TypeView::Record(r) => {
                self.out.push(TY_RECORD);
                self.ent(r, d)?;
            }
            TypeView::Enum(e) => {
                self.out.push(TY_ENUM);
                self.ent(e, d)?;
            }
            TypeView::Complex(e) => self.wrap(TY_COMPLEX, e, d)?,
            TypeView::Atomic(e) => self.wrap(TY_ATOMIC, e, d)?,
            TypeView::Vector { elem, count, kind } => {
                self.wrap(TY_VECTOR, elem, d)?;
                put_u32(&mut self.out, count);
                self.out.push(kind as u8);
            }
            _ => return None,
        }
        Some(())
    }

    fn wrap(&mut self, tag: u8, inner: QualType, depth: u32) -> Option<()> {
        self.out.push(tag);
        self.ty(inner, depth)
    }

    /// A type inside a value: canonical already when [`KeyCx::exact`].
    fn vty(&mut self, q: QualType, depth: u32) -> Option<()> {
        if self.exact && self.ast.types.canonical(q) != q {
            return None;
        }
        self.ty(q, depth)
    }

    /// Value `v`: part 1's encoding when it has one (the bytes of every
    /// value part 1 carries are unchanged), else by the entities it names
    /// (spec/modules.md §11.11 "Encoding"). Its own type is not written.
    fn value(&mut self, v: ConstValueId, depth: u32) -> Option<()> {
        self.room(depth)?;
        let ast = self.ast;
        let store = &ast.consts;
        if let Ok(c) = carried_value(store, v) {
            put_value(&mut self.out, &c);
            return Some(());
        }
        let d = depth + 1;
        match store.get(v) {
            ConstValue::Scalar(Scalar::Null { kind, .. }) => {
                self.out.extend_from_slice(&[TAG_MEMBER_NULL, kind as u8]);
            }
            ConstValue::Scalar(Scalar::Pointer { base, offset, designated, past, .. }) => {
                self.out.extend_from_slice(&[
                    TAG_POINTER,
                    u8::from(past) | u8::from(designated.is_some()) << 1,
                ]);
                self.ptr_base(base, d)?;
                put_var(&mut self.out, offset);
                if let Some(q) = designated {
                    self.vty(q, d)?;
                }
            }
            ConstValue::Scalar(Scalar::MemberPtr { member, kind, derived, path, .. }) => {
                self.out.extend_from_slice(&[
                    TAG_MEMBER_POINTER,
                    u8::from(kind == MemberKind::Function) | u8::from(derived) << 1,
                ]);
                self.ent(member, d)?;
                put_var(&mut self.out, path.len() as u64);
                for &p in path {
                    self.ent(p, d)?;
                }
            }
            ConstValue::Scalar(Scalar::Reflection { id, .. }) => {
                self.out.push(TAG_REFLECTION);
                self.reflectee(ast.reflections.get(id), d)?;
            }
            ConstValue::Scalar(_) => return None,
            ConstValue::Aggregate(img) => {
                self.out.push(TAG_AGGREGATE_EX);
                put_bytes(&mut self.out, img.bytes);
                put_var(&mut self.out, img.padding.len() as u64);
                for r in img.padding {
                    put_var(&mut self.out, u64::from(r.start));
                    put_var(&mut self.out, u64::from(r.len));
                }
                put_var(&mut self.out, img.relocs.len() as u64);
                for r in img.relocs {
                    put_var(&mut self.out, u64::from(r.at));
                    self.vty(store.ty(r.value), d)?;
                    self.value(r.value, d)?;
                }
                put_var(&mut self.out, img.unions.len() as u64);
                for u in img.unions {
                    put_var(&mut self.out, u64::from(u.at));
                    self.ent(u.field, d)?;
                }
            }
        }
        self.room(depth)
    }

    /// Pointer base `base` of a pointer value: its base tag, then what
    /// identifies the object it points into.
    fn ptr_base(&mut self, base: PtrBase, d: u32) -> Option<()> {
        let ast = self.ast;
        let store = &ast.consts;
        match base {
            PtrBase::Decl(x) => {
                if let Some(pv) = ast.templates.param_object_value(x) {
                    self.out.push(BASE_PARAM_OBJECT);
                    self.vty(store.ty(pv), d)?;
                    self.value(pv, d)?;
                } else {
                    self.out.push(BASE_DECL);
                    self.ent(x, d)?;
                }
            }
            PtrBase::Function(x) => {
                self.out.push(BASE_FUNCTION);
                self.ent(x, d)?;
            }
            PtrBase::TypeInfo(q) => {
                self.out.push(BASE_TYPE_INFO);
                self.vty(q, d)?;
            }
            PtrBase::Const(c) => {
                self.out.push(BASE_CONST);
                self.vty(store.ty(c), d)?;
                self.value(c, d)?;
            }
            PtrBase::StringLit { origin, .. } => {
                self.out.push(BASE_STRING);
                let r = self.expr_at(origin);
                self.or_why(r, "a pointer to a string literal outside the replayed source")?;
            }
            PtrBase::Label { label, origin } => {
                self.out.push(BASE_LABEL);
                let r = self.ent(label, d).and_then(|()| self.expr_at(origin));
                self.or_why(r, "a label address whose label has no key")?;
            }
            PtrBase::Temporary(t) => {
                let r = self.temporary(t, d);
                self.or_why(r, "a pointer to a temporary whose declaration has no key")?;
            }
        }
        Some(())
    }

    /// Lifetime-extended temporary `t`: its extending declaration and
    /// mangling index (what the replay creates it with, so the importer
    /// finds its own), its type, and the value the evaluation froze for
    /// it, which the importer then gives its own (the evaluation that
    /// would have does not run).
    fn temporary(&mut self, t: DeclId, d: u32) -> Option<()> {
        let ast = self.ast;
        let l = *ast.decls.lifetime_extended(t)?;
        self.out.push(BASE_TEMPORARY);
        self.ent(l.extending, d)?;
        put_var(&mut self.out, u64::from(l.mangling_index));
        self.ty(ast.decls.get(t).ty()?, d)?;
        let Some(v) = l.value else {
            self.out.push(0);
            return Some(());
        };
        self.out.push(1);
        self.vty(ast.consts.ty(v), d)?;
        self.value(v, d)
    }

    /// Reflectee `r`: its tag, then what identifies it.
    fn reflectee(&mut self, r: Reflectee, d: u32) -> Option<()> {
        let ast = self.ast;
        let store = &ast.consts;
        self.out.push(r.tag());
        match r {
            Reflectee::Null => {}
            Reflectee::Type(q) => match ast.types.view(q.ty()) {
                // A kept top-level alias ([`Ast::reflection_type`]).
                TypeView::Typedef { decl, .. } if q.quals_bits() == 0 => {
                    self.out.push(REFL_ALIAS);
                    self.ent(decl, d)?;
                }
                // A kept alias template specialization, canonical inside.
                TypeView::TemplateSpecialization {
                    name,
                    args,
                    underlying: Some(u),
                    alias: true,
                } if q.quals_bits() == 0 => {
                    let r = self.alias_spec(name, args, u, d);
                    self.or_why(r, "a reflection of an alias template specialization with no key")?;
                }
                TypeView::TemplateSpecialization { .. } => return None,
                _ => {
                    self.out.push(REFL_TYPE);
                    self.vty(q, d)?;
                }
            },
            Reflectee::Decl(x) | Reflectee::Namespace(x) | Reflectee::NamespaceAlias(x) => {
                self.ent(x, d)?;
            }
            Reflectee::Template(n) => {
                let TemplateName::Template(x) = ast.types.template_name_view(n) else {
                    return None;
                };
                self.ent(x, d)?;
            }
            Reflectee::Value(c) | Reflectee::Object(c) => {
                self.vty(store.ty(c), d)?;
                self.value(c, d)?;
            }
            Reflectee::BaseSpec(x, i) | Reflectee::FnParam(x, i) => {
                self.ent(x, d)?;
                put_var(&mut self.out, u64::from(i));
            }
            Reflectee::DataMemberSpec(id) => {
                let r = self.dms(id, d);
                self.or_why(r, "a data member specification with a value that has no key")?;
            }
            Reflectee::Annotation(a) => self.annotation(a, d)?,
        }
        Some(())
    }

    fn alias_spec(
        &mut self,
        name: kiln_ast::ids::TemplateNameId,
        args: ArgListId,
        u: QualType,
        d: u32,
    ) -> Option<()> {
        let types = &self.ast.types;
        let TemplateName::Template(x) = types.template_name_view(name) else { return None };
        if types.canonical_template_name(name) != name || types.canonical_args(args) != args {
            return None;
        }
        self.out.push(REFL_ALIAS_SPEC);
        self.ent(x, d)?;
        self.args(args, d)?;
        self.vty(u, d)
    }

    /// Data member specification `id` ([meta.reflection.define.aggregate]):
    /// interned by its contents, so the contents are its identity.
    fn dms(&mut self, id: kiln_ast::ids::DmsId, d: u32) -> Option<()> {
        let s = *self.oven?.dms(id)?;
        self.vty(s.ty, d)?;
        let flags = if s.name.is_some() { DMS_NAME } else { 0 }
            | if s.alignment.is_some() { DMS_ALIGN } else { 0 }
            | if s.bit_width.is_some() { DMS_WIDTH } else { 0 }
            | if s.nua { DMS_NUA } else { 0 };
        self.out.push(flags);
        if let Some(n) = s.name {
            put_bytes(&mut self.out, self.ast.symbols().as_str(n).as_bytes());
        }
        if let Some(a) = s.alignment {
            put_u32(&mut self.out, a);
        }
        if let Some(w) = s.bit_width {
            put_u32(&mut self.out, w);
        }
        let annots = self.oven?.dms_annots(s.annots);
        put_var(&mut self.out, annots.len() as u64);
        for &a in annots {
            self.reflectee(self.ast.reflections.get(a), d)?;
        }
        Some(())
    }

    /// Annotation `a`: the entity (or base-specifier) it is an annotation
    /// of, its position, and its ordinal among that entity's annotations
    /// at the position (a pack expansion makes several at one).
    fn annotation(&mut self, a: AnnId, d: u32) -> Option<()> {
        let ast = self.ast;
        let n = *ast.annots.get(a);
        if n.loc == Loc::INVALID {
            self.why.get_or_insert("an annotation reflection made by define_aggregate");
            return None;
        }
        let own: Vec<AnnId> = ast.lists.iter::<AnnId>(ast.annots.of_decl(n.decl)).collect();
        let (list, base) = if own.contains(&a) {
            (ast.annots.of_entity(ast, n.decl), None)
        } else {
            let first = ast.first(n.decl);
            let count = ast.decls.bases(n.decl, &ast.lists).count() as u32;
            let i = (0..count)
                .find(|&i| ast.lists.iter::<AnnId>(ast.annots.of_base(first, i)).any(|x| x == a))?;
            (ast.lists.iter::<AnnId>(ast.annots.of_base(first, i)).collect(), Some(i))
        };
        match base {
            None => self.out.push(0),
            Some(i) => {
                self.out.push(1);
                put_var(&mut self.out, u64::from(i));
            }
        }
        let r = self.ent(n.decl, d);
        self.or_why(r, "an annotation reflection whose entity has no key")?;
        let at = loc_key(self.sm, n.loc, &mut self.file);
        let Some(at) = at else {
            self.why.get_or_insert("an annotation reflection outside the replayed source");
            return None;
        };
        let mut k = 0u64;
        for &x in &list {
            if x == a {
                break;
            }
            if loc_key(self.sm, ast.annots.get(x).loc, &mut self.file) == Some(at) {
                k += 1;
            }
        }
        for w in at.0 {
            put_u32(&mut self.out, w);
        }
        put_var(&mut self.out, k);
        Some(())
    }
}

// --- checking a decoded key --------------------------------------------

/// Checks that `b` is a well-formed key whose path indices are below
/// `npaths`. Keys are only compared with the importer's own, never
/// interpreted, but the reader still rejects malformed ones (the section
/// is damaged).
pub(super) fn check_key(b: &[u8], npaths: usize) -> Res<()> {
    let mut ck = Ck { r: Rd(b), npaths };
    match b.first() {
        Some(&ENT_SPEC | &ENT_MEMBER | &ENT_PATTERN) => ck.ent(0)?,
        Some(&ENT_SUBST) => {
            ck.r.take(2)?;
            ck.ent(1)?;
            ck.ent(1)?;
            ck.args(1)?;
        }
        _ => return bad("bad instantiation key"),
    }
    if !ck.r.0.is_empty() {
        return bad("bad instantiation key");
    }
    Ok(())
}

struct Ck<'a> {
    r: Rd<'a>,
    npaths: usize,
}

impl Ck<'_> {
    fn depth(depth: u32) -> Res<()> {
        if depth > MAX_DEPTH + 1 { bad("instantiation key nested too deeply") } else { Ok(()) }
    }

    fn ent(&mut self, depth: u32) -> Res<()> {
        Self::depth(depth)?;
        match self.r.u8()? {
            ENT_SPEC => {
                self.ent(depth + 1)?;
                self.args(depth + 1)
            }
            ENT_MEMBER | ENT_PATTERN => {
                self.ent(depth + 1)?;
                self.ent(depth + 1)
            }
            ENT_TU => Ok(()),
            ENT_DECL => {
                let _kind = self.r.u8()?;
                match self.r.u8()? {
                    0 => {
                        self.r.bytes()?;
                    }
                    1 => {
                        self.r.u8()?;
                    }
                    2..=6 => {}
                    _ => return bad("bad instantiation key"),
                }
                self.at()
            }
            _ => bad("bad instantiation key"),
        }
    }

    /// A position: two (path index, offset) pairs.
    fn at(&mut self) -> Res<()> {
        for i in 0..4 {
            let w = self.r.u32()?;
            if i % 2 == 0 && w as usize >= self.npaths {
                return bad("path index out of range");
            }
        }
        Ok(())
    }

    /// A varint that fits a `u32`.
    fn small(&mut self, what: &'static str) -> Res<()> {
        if self.r.var()? > u64::from(u32::MAX) { bad(what) } else { Ok(()) }
    }

    fn args(&mut self, depth: u32) -> Res<()> {
        Self::depth(depth)?;
        let n = self.r.count(2)?;
        for _ in 0..n {
            match self.r.u8()? {
                ARG_TYPE => self.ty(depth + 1)?,
                ARG_VALUE => {
                    self.ty(depth + 1)?;
                    self.value(depth + 1)?;
                }
                ARG_TEMPLATE => self.ent(depth + 1)?,
                ARG_PACK => self.args(depth + 1)?,
                _ => return bad("bad instantiation key"),
            }
        }
        Ok(())
    }

    fn ty(&mut self, depth: u32) -> Res<()> {
        Self::depth(depth)?;
        if self.r.u8()? > 7 {
            return bad("bad instantiation key");
        }
        let d = depth + 1;
        match self.r.u8()? {
            TY_BUILTIN => {
                self.r.u8()?;
            }
            TY_BITINT => {
                self.r.flag()?;
                self.r.u32()?;
            }
            TY_POINTER | TY_LREF | TY_RREF | TY_INCOMPLETE_ARRAY | TY_COMPLEX | TY_ATOMIC => {
                self.ty(d)?;
            }
            TY_MEMBER_POINTER => {
                self.ty(d)?;
                self.ty(d)?;
            }
            TY_ARRAY => {
                self.r.u64()?;
                self.ty(d)?;
            }
            TY_FUNCTION => {
                self.ty(d)?;
                let n = self.r.count(2)?;
                for _ in 0..n {
                    self.ty(d)?;
                }
                self.r.take(2)?;
                if self.r.u8()? > 1 {
                    return bad("bad instantiation key");
                }
            }
            TY_RECORD | TY_ENUM => self.ent(d)?,
            TY_VECTOR => {
                self.ty(d)?;
                self.r.u32()?;
                self.r.u8()?;
            }
            _ => return bad("bad instantiation key"),
        }
        Ok(())
    }

    /// A value of any tag (the grammar [`KeyCx::value`] writes).
    fn value(&mut self, depth: u32) -> Res<()> {
        Self::depth(depth)?;
        let d = depth + 1;
        match self.r.0.first() {
            Some(&t) if t <= TAG_AGGREGATE => return read_plain(&mut self.r).map(drop),
            _ => {}
        }
        match self.r.u8()? {
            TAG_MEMBER_NULL => {
                if !matches!(self.r.u8()?, 1 | 2) {
                    return bad("bad null kind");
                }
            }
            TAG_POINTER => {
                let flags = self.r.u8()?;
                if flags > 3 {
                    return bad("bad pointer flags");
                }
                match self.r.u8()? {
                    BASE_DECL | BASE_FUNCTION => self.ent(d)?,
                    BASE_TYPE_INFO => self.ty(d)?,
                    BASE_PARAM_OBJECT | BASE_CONST => {
                        self.ty(d)?;
                        self.value(d)?;
                    }
                    BASE_STRING => self.at()?,
                    BASE_LABEL => {
                        self.ent(d)?;
                        self.at()?;
                    }
                    BASE_TEMPORARY => {
                        self.ent(d)?;
                        self.small("bad temporary")?;
                        self.ty(d)?;
                        if self.r.flag()? {
                            self.ty(d)?;
                            self.value(d)?;
                        }
                    }
                    _ => return bad("bad pointer base"),
                }
                self.r.var()?;
                if flags & 2 != 0 {
                    self.ty(d)?;
                }
            }
            TAG_MEMBER_POINTER => {
                if self.r.u8()? > 3 {
                    return bad("bad member pointer flags");
                }
                self.ent(d)?;
                for _ in 0..self.r.var_count(1)? {
                    self.ent(d)?;
                }
            }
            TAG_REFLECTION => self.reflectee(d)?,
            TAG_AGGREGATE_EX => {
                self.r.bytes()?;
                for _ in 0..self.r.var_count(2)? {
                    self.r.var()?;
                    self.r.var()?;
                }
                for _ in 0..self.r.var_count(3)? {
                    self.r.var()?;
                    self.ty(d)?;
                    self.value(d)?;
                }
                for _ in 0..self.r.var_count(2)? {
                    self.r.var()?;
                    self.ent(d)?;
                }
            }
            _ => return bad("unknown value tag"),
        }
        Ok(())
    }

    /// A reflectee (the grammar [`KeyCx::reflectee`] writes).
    fn reflectee(&mut self, d: u32) -> Res<()> {
        Self::depth(d)?;
        match self.r.u8()? {
            0 => match self.r.u8()? {
                REFL_TYPE => self.ty(d)?,
                REFL_ALIAS => self.ent(d)?,
                REFL_ALIAS_SPEC => {
                    self.ent(d)?;
                    self.args(d)?;
                    self.ty(d)?;
                }
                _ => return bad("bad reflection"),
            },
            1 | 3 | 4 | 5 => self.ent(d)?,
            2 => {}
            6 | 7 => {
                self.ty(d)?;
                self.value(d)?;
            }
            8 | 11 => {
                self.ent(d)?;
                self.small("bad reflection")?;
            }
            9 => {
                self.ty(d)?;
                let flags = self.r.u8()?;
                if flags > DMS_NAME | DMS_ALIGN | DMS_WIDTH | DMS_NUA {
                    return bad("bad reflection");
                }
                if flags & DMS_NAME != 0 {
                    self.r.bytes()?;
                }
                if flags & DMS_ALIGN != 0 {
                    self.r.u32()?;
                }
                if flags & DMS_WIDTH != 0 {
                    self.r.u32()?;
                }
                for _ in 0..self.r.var_count(1)? {
                    self.reflectee(d + 1)?;
                }
            }
            10 => {
                match self.r.u8()? {
                    0 => {}
                    1 => self.small("bad reflection")?,
                    _ => return bad("bad reflection"),
                }
                self.ent(d)?;
                self.at()?;
                self.small("bad reflection")?;
            }
            _ => return bad("bad reflection"),
        }
        Ok(())
    }
}
