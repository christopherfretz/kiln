//! The names family ([meta.reflection.names]; spec/reflection.md §3.9):
//! `has_identifier`, `identifier_of`, `u8identifier_of`,
//! `display_string_of`, `u8display_string_of` and `source_location_of`,
//! plus the kiln test helpers of the materialization recipes
//! (`__kiln_reflection_range`, `__kiln_member_offset`,
//! `__kiln_access_context`; §3.5).
//!
//! Every string is a frozen string literal object, made once per
//! reflection and kind and cached here: a call in steady state is one
//! hash lookup and allocates nothing.

// `Reflectee` grows variants (WP-472); its matches here keep a wildcard.
#![allow(clippy::match_wildcard_for_single_variants)]

use std::cell::Cell;

use kiln_ast::decl::DeclKind;
use kiln_ast::names::DeclNameKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::TypeView;
use kiln_ast::{ConstValueId, DeclId, ReflectionId};
use kiln_oven::{ExcCtor, MetaCtx, MetaError, MetaOut, Oven, QueryOp, info_arg};
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;
use kiln_source::symbol::Symbol;

use super::fail;
use crate::Sema;

/// The names family's caches (a `Sema` field).
#[derive(Debug, Default)]
pub(crate) struct NameCache {
    /// (reflection, [`Text`] kind) to the frozen string and its length.
    strs: FxHashMap<(ReflectionId, u8), (ConstValueId, u64)>,
    /// `source_location_of`: the `const __impl *` per reflection.
    locs: FxHashMap<ReflectionId, ConstValueId>,
    /// The `std::meta::exception` constructor once found, and whether its
    /// message parameter is a `u8string_view` (`recipe.rs`).
    pub(super) exc: Option<(ExcCtor, bool)>,
}

/// Whether `q` is implemented here (the dispatcher's arm).
pub(super) fn is_names_op(q: QueryOp) -> bool {
    matches!(
        q,
        QueryOp::HasIdentifier
            | QueryOp::IdentifierOf
            | QueryOp::U8IdentifierOf
            | QueryOp::DisplayStringOf
            | QueryOp::U8DisplayStringOf
            | QueryOp::SourceLocationOf
            | QueryOp::KilnReflectionRange
            | QueryOp::KilnMemberOffset
            | QueryOp::KilnAccessContext
    )
}

/// Which string of a reflection.
#[derive(Clone, Copy)]
enum Text {
    Identifier = 0,
    Display = 2,
}

/// What `identifier_of` returns: an identifier, or the ud-suffix of a
/// literal operator.
type Ident = Symbol;

impl Sema<'_, '_> {
    /// Runs names-family op `q` (spec/reflection.md §3.9).
    pub(super) fn meta_names(
        &mut self,
        q: QueryOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        *out = match q {
            QueryOp::HasIdentifier => {
                MetaOut::Scalar(u128::from(self.identifier(info_arg(args, 0)?).is_some()))
            }
            QueryOp::IdentifierOf => {
                self.meta_text(info_arg(args, 0)?, Text::Identifier, false, loc)?
            }
            QueryOp::U8IdentifierOf => {
                self.meta_text(info_arg(args, 0)?, Text::Identifier, true, loc)?
            }
            QueryOp::DisplayStringOf => {
                self.meta_text(info_arg(args, 0)?, Text::Display, false, loc)?
            }
            QueryOp::U8DisplayStringOf => {
                self.meta_text(info_arg(args, 0)?, Text::Display, true, loc)?
            }
            QueryOp::SourceLocationOf => MetaOut::SrcLoc(self.meta_location_of(info_arg(args, 0)?)),
            QueryOp::KilnReflectionRange => {
                let r = args.first().copied().ok_or_else(MetaError::invalid)?;
                let n = oven.meta_range_len(r);
                let count = u32::try_from(n).map_err(|_| MetaError::invalid())?;
                let mut v = std::mem::take(oven.meta_infos());
                v.clear();
                let read = (0..n).try_for_each(|i| {
                    v.push(oven.meta_range_at(r, i)?);
                    Ok::<(), MetaError>(())
                });
                *oven.meta_infos() = v;
                read?;
                MetaOut::Infos(count)
            }
            QueryOp::KilnMemberOffset => {
                let (&a, &b) = args.first().zip(args.get(1)).ok_or_else(MetaError::invalid)?;
                MetaOut::Pair(a, b)
            }
            QueryOp::KilnAccessContext => {
                let (a, b) = (info_arg(args, 0)?, info_arg(args, 1)?);
                MetaOut::Pair(u128::from(a.bits()), u128::from(b.bits()))
            }
            _ => return Err(MetaError::invalid()),
        };
        Ok(())
    }

    /// The string `t` of `r`, as `char8_t` when `u8`: frozen once.
    fn meta_text(
        &mut self,
        r: ReflectionId,
        t: Text,
        u8: bool,
        loc: Loc,
    ) -> Result<MetaOut, MetaError> {
        let key = (r, t as u8 + u8::from(u8));
        if let Some(&(cv, len)) = self.meta_names.strs.get(&key) {
            return Ok(MetaOut::Str(cv, len));
        }
        let s = match t {
            Text::Identifier => match self.identifier(r) {
                Some(id) => self.ast.symbols().as_str(id).to_owned(),
                None => {
                    return fail(loc, format!("'{}' has no identifier", self.reflection_str(r)));
                }
            },
            Text::Display => self.display_string(r),
        };
        let Some(cv) = self.frozen_string(&s, u8) else {
            return fail(loc, "kiln: name too large for a constant");
        };
        let v = (cv, s.len() as u64);
        self.meta_names.strs.insert(key, v);
        Ok(MetaOut::Str(v.0, v.1))
    }

    /// `identifier_of(r)` when `has_identifier(r)` ([meta.reflection.names]
    /// /1-2): a cv-unqualified class that is not a specialization, or a
    /// cv-unqualified enumeration; a declaration with an identifier that
    /// is not a specialization; a literal operator (its ud-suffix). Not
    /// constructors, destructors, operator and conversion functions (and
    /// their templates), unnamed entities and other types.
    fn identifier(&self, r: ReflectionId) -> Option<Ident> {
        if let Reflectee::FnParam(f, i) = self.ast.reflections.get(r) {
            return self.param_identifier(f, i);
        }
        self.decl_identifier(self.named_decl(r)?)
    }

    /// The name of parameter `i` of function `f` ([meta.reflection.names]/1,
    /// P3096): the one name every declaration of `f` that names the
    /// parameter gives it; `None` when none names it or two disagree.
    fn param_identifier(&self, f: DeclId, i: u32) -> Option<Ident> {
        let mut found = None;
        for d in self.ast.redecls_of(f) {
            let Some(data) = self.ast.decls.function(d) else { continue };
            let Some(p) = self.ast.lists.iter::<DeclId>(data.params).nth(i as usize) else {
                continue;
            };
            let Some(n) = self.ast.decls.get(p).name().as_ident() else { continue };
            match found {
                None => found = Some(n),
                Some(m) if m == n => {}
                Some(_) => return None,
            }
        }
        found
    }

    /// The declaration whose name the names family reports for `r`: a
    /// variable, function, member or enumerator; a namespace or namespace
    /// alias; a template; a type alias (kept by `^^`, spec/reflection-repr.md
    /// §1); the class or enumeration a cv-unqualified type denotes.
    /// `None` for other types, alias template specializations and
    /// reflections of no declaration (values, objects, base specifiers).
    fn named_decl(&self, r: ReflectionId) -> Option<DeclId> {
        match self.ast.reflections.get(r) {
            Reflectee::Type(t) => {
                if !t.quals().is_empty() {
                    return None;
                }
                match self.ast.types.view(t.ty()) {
                    TypeView::Typedef { decl, .. } => return Some(decl),
                    TypeView::TemplateSpecialization { alias: true, .. } => return None,
                    _ => {}
                }
                self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t))
            }
            Reflectee::Decl(d) | Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => Some(d),
            Reflectee::Template(n) => self.ast.types.template_decl(n),
            _ => None,
        }
    }

    /// The identifier declaration `d` introduces, if it has one.
    fn decl_identifier(&self, d: DeclId) -> Option<Ident> {
        let n = self.ast.decls.get(d);
        if self.ast.spec_info(d).is_some()
            || matches!(
                n.kind(),
                DeclKind::TranslationUnit
                    | DeclKind::ClassTemplateSpecialization
                    | DeclKind::ClassTemplatePartialSpecialization
                    | DeclKind::VarTemplateSpecialization
                    | DeclKind::VarTemplatePartialSpecialization
            )
        {
            return None;
        }
        let name = n.name();
        match name.kind() {
            DeclNameKind::Identifier => match name.as_ident() {
                Some(s) => Some(s),
                // An unnamed class or enumeration named by a typedef for
                // linkage purposes has that name ([dcl.typedef]/9).
                None if matches!(n.kind(), DeclKind::CXXRecord | DeclKind::Enum) => self
                    .ast
                    .decls
                    .typedef_for_anon(d)
                    .and_then(|td| self.ast.decls.get(td).name().as_ident()),
                None => None,
            },
            DeclNameKind::LiteralOperator => name.as_literal_operator(),
            _ => None,
        }
    }

    /// `display_string_of(r)` (implementation-defined): a declaration's
    /// qualified name as a diagnostic prints it (`N::S::operator+`,
    /// `N::S::~S`; GCC 16.1 qualifies too), a type canonical and fully
    /// scoped (`N::T1<int>`), a value or object as its source spelling
    /// ([`Self::value_spelling`], [`Self::object_spelling`]), else the
    /// reflection as a note prints it.
    fn display_string(&self, r: ReflectionId) -> String {
        let s = match self.ast.reflections.get(r) {
            Reflectee::Value(v) => self.value_spelling(v).unwrap_or_default(),
            Reflectee::Object(p) => self.object_spelling(p).unwrap_or_default(),
            Reflectee::Type(t) => self.ty_canonical_str(t),
            Reflectee::Decl(d) | Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
                self.qualified_decl_name(d)
            }
            Reflectee::Template(n) => self
                .ast
                .types
                .template_decl(n)
                .map(|d| self.qualified_decl_name(d))
                .unwrap_or_default(),
            _ => String::new(),
        };
        if s.is_empty() { self.reflection_str(r) } else { s }
    }

    /// The source spelling of constant `v` (spec/reflection.md §3.9): the
    /// diagnostic constant printer in template-argument style without
    /// literal suffixes (`86400`, `'a'`, `Col::red`, `Pt{3}`, `&g`,
    /// `nullptr`), an enumeration value naming no enumerator as a cast
    /// (`(Col)5`), floating point as the shortest round-tripping decimal
    /// (`1.5e0`), a reflection of a value or object as the
    /// `reflect_constant` / `reflect_object` call making it. `None` when
    /// some part has no spelling (a temporary, an opaque subobject, an
    /// infinity or NaN, an unnamed class or closure type, a reflection
    /// with no `^^` operand such as a base specifier).
    fn value_spelling(&self, v: ConstValueId) -> Option<String> {
        use kiln_ast::consts::{PrintOptions, PrintStyle};
        let opts = PrintOptions {
            include_type: false,
            enum_cast: true,
            max_depth: u32::from(u16::MAX),
            max_elems: u32::MAX,
            ..PrintOptions::with_style(PrintStyle::TemplateArg)
        };
        let env = SpellEnv {
            sema: self,
            names: kiln_ast::dump::AstNames::new(&self.ast),
            lossy: Cell::new(false),
        };
        let mut s = String::new();
        let exact = self.ast.consts.print_exact(v, &self.ast.strings, &env, &opts, &mut s);
        (exact && !env.lossy.get() && !s.is_empty()).then_some(s)
    }

    /// The source spelling of the object pointer `p` designates
    /// (`reflect_object`, a class or reference template argument): a
    /// template parameter object as its value (`Pt{3}`, as GCC 16.1), a
    /// named object or subobject as its designator (`ns::v`, `arr[1]`,
    /// `pt.x`). `None` for anything else (a temporary, a subobject of a
    /// template parameter object).
    fn object_spelling(&self, p: ConstValueId) -> Option<String> {
        use kiln_ast::consts::{PtrBase, Scalar};
        let Some(Scalar::Pointer { base, offset, .. }) = self.ast.consts.scalar(p) else {
            return None;
        };
        let PtrBase::Decl(d) = base else { return None };
        if self.kind_of(d) == DeclKind::TemplateParamObject {
            let v = self.ast.templates.param_object_value(d).filter(|_| offset == 0)?;
            return self.value_spelling(v);
        }
        // The pointer `&x.m` (or `arr` for a whole array) names the object
        // without its `&`.
        let s = self.value_spelling(p)?;
        Some(s.strip_prefix('&').map(str::to_owned).unwrap_or(s))
    }

    /// `source_location_of(r)` ([meta.reflection.names]/7): the location
    /// of the (first) declaration `r` reflects, with an empty function
    /// name; `None` (a `source_location{}`) for the global namespace,
    /// types other than classes, enumerations and aliases, and reflections
    /// of no declaration.
    fn meta_location_of(&mut self, r: ReflectionId) -> Option<ConstValueId> {
        if let Some(&cv) = self.meta_names.locs.get(&r) {
            return Some(cv);
        }
        let d = self.named_decl(r)?;
        if self.kind_of(d) == DeclKind::TranslationUnit {
            return None;
        }
        let loc = self.ast.decls.get(d).loc();
        let cv = self.meta_source_location(loc, "")?;
        self.meta_names.locs.insert(r, cv);
        Some(cv)
    }
}

/// The [`kiln_ast::consts::PrintEnv`] of [`Sema::value_spelling`]: the
/// AST's, with types (a class value's `T{...}`, an enumeration cast's
/// `(T)`) canonical and fully scoped as `display_string_of` of the type
/// prints them, so the spelling names the type from any scope. `lossy` is set when a
/// type or reflection has no spelling.
struct SpellEnv<'s, 'a, 'b> {
    sema: &'s Sema<'a, 'b>,
    names: kiln_ast::dump::AstNames<'s>,
    lossy: Cell<bool>,
}

impl SpellEnv<'_, '_, '_> {
    /// Marks `ty` unspellable when it is an unnamed class (a closure type,
    /// `struct {} v;`) without a typedef name for linkage.
    fn check_type(&self, ty: kiln_ast::QualType) {
        let ast = &self.sema.ast;
        if let Some(d) = ast.types.as_record(ast.types.canonical(ty))
            && self.names.is_unnamed(d)
            && ast.decls.typedef_for_anon(d).is_none()
        {
            self.lossy.set(true);
        }
    }
}

impl kiln_ast::consts::PrintEnv for SpellEnv<'_, '_, '_> {
    fn shape(&self, ty: kiln_ast::QualType) -> kiln_ast::consts::Shape {
        self.names.shape(ty)
    }

    fn decl_name(&self, decl: DeclId, out: &mut String) {
        self.names.decl_name(decl, out);
    }

    fn decl_type(&self, decl: DeclId) -> Option<kiln_ast::QualType> {
        self.names.decl_type(decl)
    }

    fn same_unqualified(&self, a: kiln_ast::QualType, b: kiln_ast::QualType) -> bool {
        self.names.same_unqualified(a, b)
    }

    fn type_name(&self, ty: kiln_ast::QualType, out: &mut String) {
        self.check_type(ty);
        out.push_str(&self.sema.ty_canonical_str(ty));
    }

    fn arg_type_name(&self, ty: kiln_ast::QualType, out: &mut String) {
        self.check_type(ty);
        out.push_str(&self.sema.ty_canonical_str(ty));
    }

    fn enumerator(
        &self,
        ty: kiln_ast::QualType,
        bits: &kiln_ast::consts::IntBits<'_>,
        out: &mut String,
    ) -> bool {
        self.names.enumerator(ty, bits, out)
    }

    fn reflection(&self, id: ReflectionId, out: &mut String) {
        let (call, s) = match self.sema.ast.reflections.get(id) {
            Reflectee::Value(v) => ("std::meta::reflect_constant(", self.sema.value_spelling(v)),
            Reflectee::Object(p) => ("std::meta::reflect_object(", self.sema.object_spelling(p)),
            Reflectee::BaseSpec(..)
            | Reflectee::DataMemberSpec(_)
            | Reflectee::Annotation(_)
            | Reflectee::FnParam(..) => ("", None),
            _ => return self.names.reflection(id, out),
        };
        match s {
            Some(s) => {
                out.push_str(call);
                out.push_str(&s);
                out.push(')');
            }
            None => {
                self.lossy.set(true);
                self.names.reflection(id, out);
            }
        }
    }
}
