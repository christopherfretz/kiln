//! Reflection literals (WP-501, spec/reflection.md §8.3): a
//! `std::meta::info` value mangles as its reflectee, structurally, in
//! GCC 16's `<reflection>` grammar under one of two `<info-type>`
//! spellings:
//!
//! ```text
//! <expr-primary> ::= L <info-type> <reflection> E
//! <info-type>    ::= u6__info      # kiln (default; a substitution candidate)
//!                ::= Dm            # gcc  (builtin; not a candidate)
//! ```
//!
//! The same reflection spells the same string in every translation unit,
//! so a specialization on one has its reflectee's linkage.

use std::fmt::Write as _;

use kiln_ast::Ast;
use kiln_ast::consts::{ConstValue, PrintEnv, PtrBase, Scalar, Shape};
use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::ids::{ConstValueId, DeclId, DmsId, ExprId, QualType, ReflectionId};
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{BuiltinType, TemplateName, TypeView};

use super::{
    Mangler, Structor, StructorSpelling, enclosing_function, has_internal_linkage, in_anon_ns,
    is_anon_ns, is_function_kind, is_internal_symbol, mangle_param_object, tag_externally_visible,
    type_externally_visible, value_externally_visible,
};

/// How a `std::meta::info` value mangles (`-fkiln-reflection-mangling=`,
/// spec/reflection.md §8.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ReflectionMangling {
    /// `L u6__info <reflection> E`: GCC's `<reflection>` under kiln's
    /// vendor type (opt-in, experimental).
    Kiln,
    /// `L Dm <reflection> E`, link-compatible with g++ 16 (the default,
    /// WP-909).
    #[default]
    Gcc,
    /// Prototype (hidden): `L u6__info h <32 hex digits> E`, a 128-bit
    /// hash of the `kiln` form's `<reflection>`.
    KilnHashed,
}

impl ReflectionMangling {
    /// The form named `s` (`kiln`, `gcc`, or the hidden `kiln-hashed`).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "kiln" => Some(Self::Kiln),
            "gcc" => Some(Self::Gcc),
            "kiln-hashed" => Some(Self::KilnHashed),
            _ => None,
        }
    }
}

/// Whether reflection `id` mangles as an externally visible entity: the
/// linkage of its reflectee ([basic.link]/15: a reflection of an entity
/// without linkage is TU-local). An annotation has no cross-TU spelling
/// and is internal.
pub(super) fn reflection_externally_visible(ast: &Ast, id: ReflectionId) -> bool {
    match ast.reflections.get(id) {
        Reflectee::Null => true,
        Reflectee::Type(q) => type_externally_visible(ast, q) && alias_visible(ast, q),
        Reflectee::Decl(d) => decl_visible(ast, d),
        Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
            !is_anon_ns(ast, d) && !in_anon_ns(ast, d) && enclosing_function(ast, d).is_none()
        }
        Reflectee::Template(n) => match ast.types.template_name_view(n) {
            TemplateName::Template(t) => !has_internal_linkage(ast, t),
            _ => true,
        },
        Reflectee::Value(v) | Reflectee::Object(v) => value_externally_visible(ast, v),
        Reflectee::BaseSpec(d, _) => tag_externally_visible(ast, ast.first(d)),
        Reflectee::FnParam(f, _) => !is_internal_symbol(ast, f),
        // Its type's and annotations' linkage, once described.
        Reflectee::DataMemberSpec(s) => ast.reflections.dms(s).is_some_and(|d| {
            type_externally_visible(ast, d.ty)
                && d.annotations.iter().all(|&a| reflection_externally_visible(ast, a))
        }),
        // GCC's annotation index is TU-local.
        Reflectee::Annotation(_) => false,
    }
}

/// A kept alias (`TypeView::Typedef`) declared in a function or an
/// unnamed namespace is TU-local.
fn alias_visible(ast: &Ast, q: QualType) -> bool {
    match ast.types.view(q.ty()) {
        TypeView::Typedef { decl, .. } => {
            enclosing_function(ast, decl).is_none() && !in_anon_ns(ast, decl)
        }
        _ => true,
    }
}

fn decl_visible(ast: &Ast, d: DeclId) -> bool {
    match ast.decls.get(d).kind() {
        // A member of a class or enumeration has the class's linkage.
        DeclKind::Field | DeclKind::IndirectField | DeclKind::EnumConstant => {
            match ast.decls.get(d).ctx() {
                Some(c)
                    if matches!(ast.decls.get(c).kind(), DeclKind::Enum) || is_record(ast, c) =>
                {
                    tag_externally_visible(ast, ast.first(c))
                }
                _ => false,
            }
        }
        DeclKind::ParmVar => false,
        _ => !is_internal_symbol(ast, d),
    }
}

fn is_record(ast: &Ast, d: DeclId) -> bool {
    super::is_record_kind(ast.decls.get(d).kind())
}

/// The FNV-1a 128-bit hash of `s` (the `kiln-hashed` prototype).
fn fnv1a_128(s: &str) -> u128 {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    s.bytes().fold(OFFSET, |h, b| (h ^ u128::from(b)).wrapping_mul(PRIME))
}

impl Mangler<'_, '_> {
    fn gcc(&self) -> bool {
        self.anon.refl_mode == ReflectionMangling::Gcc
    }

    /// `L <info-type> <reflection> E` for reflection `id` (`None`: the
    /// null reflection) of type `ty`.
    pub(super) fn reflection_literal(&mut self, ty: QualType, id: Option<ReflectionId>) {
        self.out.push('L');
        self.ty(ty);
        let r = id.map_or(Reflectee::Null, |id| self.ast.reflections.get(id));
        if self.anon.refl_mode == ReflectionMangling::KilnHashed {
            // The structural form in a fresh substitution context, so the
            // hash is the reflectee's alone.
            let body = {
                let mut m = Mangler::new(self.ast, &mut *self.anon, "");
                m.reflection(r);
                m.out
            };
            let _ = write!(self.out, "h{:032x}", fnv1a_128(&body));
        } else {
            self.reflection(r);
        }
        self.out.push('E');
        self.anon.refl_literals += 1;
    }

    /// `<compact-number>`: `_` for 0, `<n-1>_` otherwise.
    fn compact(&mut self, n: u32) {
        if n > 0 {
            let _ = write!(self.out, "{}", n - 1);
        }
        self.out.push('_');
    }

    /// `<prefix>` of context `dc` as GCC's `write_prefix`: nothing for the
    /// translation unit, `St` for `std`; a context in a function is
    /// written `Z <encoding> E <prefix>` (kiln).
    fn refl_prefix(&mut self, dc: Option<DeclId>) {
        let Some(dc) = dc else { return };
        let ast = self.ast;
        let f = if is_function_kind(ast.decls.get(dc).kind()) {
            Some(dc)
        } else {
            enclosing_function(ast, dc)
        };
        if let Some(f) = f {
            self.out.push('Z');
            self.encoding(f, Structor::Complete);
            self.out.push('E');
            if f != dc {
                self.prefix(dc, Some(f));
            }
        } else {
            self.prefix(dc, None);
        }
    }

    /// The context of member `d`, looking through anonymous structs and
    /// unions (GCC's `dm`).
    fn member_ctx(&self, d: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        let mut c = ast.decls.get(d).ctx();
        while let Some(x) = c
            && ast.decls.record(x).is_some_and(|r| r.defn.contains(DefinitionBits::ANONYMOUS))
        {
            c = ast.decls.get(x).ctx();
        }
        c
    }

    /// `<reflection>` of `r` (GCC `write_reflection`).
    pub(super) fn reflection(&mut self, r: Reflectee) {
        let ast = self.ast;
        match r {
            Reflectee::Null => self.out.push_str("nu"),
            Reflectee::Type(q) => self.reflected_type(q),
            Reflectee::Decl(d) => self.reflected_decl(d),
            Reflectee::Namespace(d) if ast.decls.get(d).kind() == DeclKind::TranslationUnit => {
                self.out.push_str("gs");
            }
            Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
                self.out.push_str(if matches!(r, Reflectee::Namespace(_)) { "ns" } else { "na" });
                let p = self.parent(d);
                self.refl_prefix(p);
                self.unqualified(d, Structor::Complete);
            }
            Reflectee::Template(n) => {
                let n = ast.types.canonical_template_name(n);
                if let TemplateName::Template(t) = ast.types.template_name_view(n) {
                    self.out.push_str(match ast.decls.get(t).kind() {
                        DeclKind::FunctionTemplate => "ft",
                        DeclKind::VarTemplate => "vt",
                        DeclKind::TypeAliasTemplate => "at",
                        DeclKind::Concept => "co",
                        _ => "ct",
                    });
                    let p = self.parent(t);
                    self.refl_prefix(p);
                    self.unqualified(t, Structor::Complete);
                } else {
                    self.out.push_str("tt");
                    self.template_name(n);
                }
            }
            Reflectee::Value(v) => {
                self.out.push_str("vl");
                let ty = ast.consts.ty(v);
                match ast.consts.get(v) {
                    ConstValue::Scalar(Scalar::Pointer { .. }) => self.refl_pointer(v, true),
                    // `nullptr` is GCC's `LDnE` (clang's argument form is
                    // `LDn0E`).
                    _ if ast.types.builtin(ast.types.canonical(ty))
                        == Some(BuiltinType::NullPtr) =>
                    {
                        self.out.push_str("LDnE");
                    }
                    _ => self.value_in_arg(ty, v, false),
                }
            }
            Reflectee::Object(v) => {
                self.out.push_str("ob");
                self.refl_pointer(v, false);
            }
            Reflectee::BaseSpec(d, i) => {
                self.out.push_str("ba");
                self.compact(i);
                self.tag(d);
            }
            Reflectee::FnParam(f, i) => {
                self.out.push_str("pa");
                self.compact(i);
                self.encoding(f, Structor::Complete);
            }
            Reflectee::Annotation(a) => {
                self.out.push_str("an");
                self.compact(a.index() - 1);
            }
            Reflectee::DataMemberSpec(s) => self.data_member_spec(s),
        }
    }

    /// `ds <type> _ [<source-name>] _ [<alignment>] _ [<width>] _ [n]
    /// <template-arg>*` (one per annotation); `ds _ <id> _` (TU-local)
    /// when codegen recorded no description.
    fn data_member_spec(&mut self, s: DmsId) {
        let ast = self.ast;
        let Some(d) = ast.reflections.dms(s) else {
            let _ = write!(self.out, "ds_{}_", s.index());
            return;
        };
        self.out.push_str("ds");
        self.ty(d.ty);
        self.out.push('_');
        if let Some(n) = d.name {
            self.source_name(ast.symbols().as_str(n));
        }
        self.out.push('_');
        if let Some(a) = d.alignment {
            let _ = write!(self.out, "{a}");
        }
        self.out.push('_');
        if let Some(w) = d.bit_width {
            let _ = write!(self.out, "{w}");
        }
        self.out.push('_');
        if d.nua {
            self.out.push('n');
        }
        for &a in &d.annotations {
            // GCC writes each annotation's constant as a template
            // argument.
            match ast.reflections.get(a) {
                Reflectee::Value(v) => self.value_arg(ast.consts.ty(v), v),
                r => self.reflection(r),
            }
        }
    }

    /// `ta <prefix> <unqualified> [<template-args>] _ <type>` for a kept
    /// alias, else `ty <type>`.
    fn reflected_type(&mut self, q: QualType) {
        let ast = self.ast;
        if q.quals_bits() == 0 {
            match ast.types.view(q.ty()) {
                TypeView::Typedef { decl, underlying } => {
                    self.out.push_str("ta");
                    let p = self.parent(decl);
                    self.refl_prefix(p);
                    self.unqualified(decl, Structor::Complete);
                    // GCC writes the arguments of an aliased class
                    // template specialization (`maybe_template_info` of
                    // the alias); they are also in the type, so kiln
                    // does not.
                    if self.gcc()
                        && let TypeView::Record(r) =
                            ast.types.canonical_view(ast.types.canonical(underlying))
                        && let Some((_, args)) = self.spec_of(r)
                    {
                        self.template_args(args);
                    }
                    self.out.push('_');
                    self.ty(underlying);
                    return;
                }
                TypeView::TemplateSpecialization {
                    name,
                    args,
                    underlying: Some(u),
                    alias: true,
                } => {
                    if let TemplateName::Template(t) =
                        ast.types.template_name_view(ast.types.canonical_template_name(name))
                    {
                        self.out.push_str("ta");
                        let p = self.parent(t);
                        self.refl_prefix(p);
                        self.unqualified(t, Structor::Complete);
                        // GCC writes the arguments of an aliased class
                        // template specialization, not the alias's, so
                        // its `^^A<int>` and `^^A<long>` collide when both
                        // alias one type; kiln writes the alias's.
                        if !self.gcc() {
                            self.template_args(args);
                        } else if let TypeView::Record(r) =
                            ast.types.canonical_view(ast.types.canonical(u))
                            && let Some((_, a)) = self.spec_of(r)
                        {
                            self.template_args(a);
                        }
                        self.out.push('_');
                        self.ty(u);
                        return;
                    }
                }
                _ => {}
            }
        }
        self.out.push_str("ty");
        self.ty(q);
    }

    fn reflected_decl(&mut self, d: DeclId) {
        let ast = self.ast;
        match ast.decls.get(d).kind() {
            DeclKind::Binding => {
                self.out.push_str("sb");
                self.name(d, Structor::Complete);
            }
            k if is_function_kind(k) => {
                self.out.push_str("fn");
                let saved = self.structors;
                self.structors = if matches!(k, DeclKind::CXXConstructor | DeclKind::CXXDestructor)
                {
                    StructorSpelling::Unified
                } else if saved == StructorSpelling::Unified {
                    StructorSpelling::Itanium
                } else {
                    saved
                };
                self.encoding(d, Structor::Complete);
                self.structors = saved;
            }
            DeclKind::EnumConstant => {
                self.out.push_str("en");
                let p = ast.decls.get(d).ctx();
                self.refl_prefix(p);
                self.unqualified(d, Structor::Complete);
            }
            DeclKind::Field | DeclKind::IndirectField => {
                let unnamed_bitfield = ast.decls.get(d).name().is_empty()
                    && ast.decls.field(d).is_some_and(|f| f.bit_width.is_some());
                if unnamed_bitfield {
                    // `un <prefix> <index among the unnamed bit-fields>`.
                    self.out.push_str("un");
                    let c = ast.decls.get(d).ctx();
                    let idx = c.map_or(0, |c| {
                        ast.members(c)
                            .take_while(|&m| m != d)
                            .filter(|&m| {
                                ast.decls.get(m).kind() == DeclKind::Field
                                    && ast.decls.get(m).name().is_empty()
                                    && ast.decls.field(m).is_some_and(|f| f.bit_width.is_some())
                            })
                            .count()
                    });
                    self.refl_prefix(c);
                    self.compact(u32::try_from(idx).unwrap_or(u32::MAX));
                } else {
                    self.out.push_str("dm");
                    let c = self.member_ctx(d);
                    self.refl_prefix(c);
                    self.unqualified(d, Structor::Complete);
                }
            }
            _ => {
                self.out.push_str("vr");
                self.name(d, Structor::Complete);
            }
        }
    }

    /// A pointer value (`addr`: `vl`, its address) or the object it
    /// points to (`ob`): an element of a declared array is GCC's
    /// `ix <array> <index>`; a template parameter object is the
    /// external name `L _Z <length> _ZTA... E` (GCC spells its mangled
    /// name as a source name); else the template-argument form without
    /// its `X`.
    fn refl_pointer(&mut self, v: ConstValueId, addr: bool) {
        let ast = self.ast;
        let ty = ast.consts.ty(v);
        let ConstValue::Scalar(Scalar::Pointer { base, offset, designated, .. }) =
            ast.consts.get(v)
        else {
            self.value_in_arg(ty, v, false);
            return;
        };
        let is_ref =
            matches!(ast.types.canonical_view(ty), TypeView::LValueRef(_) | TypeView::RValueRef(_));
        let addr = addr && !is_ref;
        if let PtrBase::Decl(d) = base {
            if ast.decls.get(d).kind() == DeclKind::TemplateParamObject && offset == 0 {
                if addr {
                    self.out.push_str("ad");
                }
                let s = mangle_param_object(ast, &mut *self.anon, d);
                let _ = write!(self.out, "L_Z{}{s}E", s.len());
                return;
            }
            if let Some(i) =
                self.array_index(d, offset, designated.or_else(|| ast.types.pointee(ty)))
            {
                if addr {
                    self.out.push_str("ad");
                }
                self.out.push_str("ixL");
                self.entity(d);
                self.out.push_str("EL");
                self.ty(QualType::unqual(ast.types.target().ptrdiff_t));
                let _ = write!(self.out, "{i}E");
                return;
            }
        }
        let at = self.out.len();
        self.pointer_value(ty, base, offset, designated, false);
        if !addr && !is_ref && self.out[at..].starts_with("ad") {
            self.out.replace_range(at..at + 2, "");
        }
    }

    /// The index of the element of array variable `d` at byte `offset`,
    /// when the pointee is that element.
    fn array_index(&self, d: DeclId, offset: u64, pointee: Option<QualType>) -> Option<u64> {
        let ast = self.ast;
        let Shape::Array { elem, stride, len } = ast.shape(ast.decls.get(d).ty()?) else {
            return None;
        };
        let same = ast.types.canonical(pointee?).without_quals()
            == ast.types.canonical(elem).without_quals();
        (same && stride != 0 && offset.is_multiple_of(stride) && offset / stride < len)
            .then_some(offset / stride)
    }

    /// A `CXXReflectExpr` in a dependent template argument or signature
    /// (`K<^^T>`): `L <info-type> ty <type> E`, `L <info-type> de
    /// <expression> E` for a dependent name, else the reflectee. `false`
    /// when the operand has no encoding.
    pub(super) fn reflect_expr(&mut self, e: ExprId) -> bool {
        let ast = self.ast;
        self.out.push('L');
        self.ty(ast.exprs.get(e).ty());
        if let Some(name) = ast.exprs.reflect_dependent_name(e) {
            self.out.push_str("de");
            if !self.expression(name) {
                return false;
            }
        } else {
            match ast.reflect_operand(e) {
                Reflectee::Type(q) if ast.types.is_dependent(q) => {
                    self.out.push_str("ty");
                    self.ty(q);
                }
                r => self.reflection(r),
            }
        }
        self.out.push('E');
        self.anon.refl_literals += 1;
        true
    }
}
