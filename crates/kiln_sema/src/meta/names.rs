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
#[derive(Debug, Default, kiln_snap::Snap)]
pub(crate) struct NameCache {
    /// (reflection, [`Text`] kind) to the frozen string and its length.
    strs: FxHashMap<(ReflectionId, u8), (ConstValueId, u64)>,
    /// `source_location_of`: the `const __impl *` per reflection.
    locs: FxHashMap<ReflectionId, ConstValueId>,
    /// The `std::meta::exception` constructor once found, and whether its
    /// message parameter is a `u8string_view` (`recipe.rs`).
    /// Not in a snapshot: rebuilt: found again by lookup on first use (a pure cache).
    #[snap(skip)]
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
        // An instantiation has the names of what it was instantiated from,
        // as GCC 16.1 keeps them: a function template's from its last
        // declaration (GCC's redeclaration of a template keeps the newer
        // parameters and forgets the older names), a member of a class
        // template specialization's from every declaration of the
        // primary's member, which must agree.
        let mut f = f;
        if let Some((_, info)) = self.ast.spec_info(f)
            && self.kind_of(info.template) == DeclKind::FunctionTemplate
            && let Some(p) = self.ast.template_info(info.template).and_then(|t| t.pattern)
        {
            let last = self.ast.definition(p).unwrap_or_else(|| self.ast.latest(p));
            let data = self.ast.decls.function(last)?;
            let p = self.ast.lists.iter::<DeclId>(data.params).nth(i as usize)?;
            return self.ast.decls.get(p).name().as_ident();
        }
        if let Some((_, m)) = self.ast.member_spec_info(self.ast.first(f))
            && self.ast.decls.get(m.instantiated_from).kind().is_function()
        {
            f = m.instantiated_from;
        }
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

    /// `display_string_of(r)` (implementation-defined): GCC 16.1's string
    /// (gcc/cp/reflect.cc `eval_display_string_of`; spec/reflection.md
    /// §3.9). A type as the diagnostic `%T` prints it (`long unsigned
    /// int`, `const char*`, `ns::Al {aka ns::S}`), a declaration as `%D`
    /// (`void ns::S::f() const`, `ns::e1`, `template<class T> struct
    /// ns::TT`), a parameter as `<parameter x of void f(int)>`, a base as
    /// `D: B`, a value or object as its source spelling
    /// ([`Self::value_spelling`], [`Self::object_spelling`]); anything
    /// else as the reflection as a note prints it.
    fn display_string(&self, r: ReflectionId) -> String {
        let s = match self.ast.reflections.get(r) {
            Reflectee::Null => "<null reflection>".to_owned(),
            // A class or array value prints as it does inside a
            // specialization's name (GCC's form; `fs<4>{"abc"}`).
            Reflectee::Value(v) if self.ast.consts.scalar(v).is_none() => {
                self.value_spelling_gcc(v).unwrap_or_default()
            }
            Reflectee::Value(v) => self.value_spelling(v).unwrap_or_default(),
            Reflectee::Object(p) => self.object_spelling(p).unwrap_or_default(),
            Reflectee::Type(t) => self.gcc_type_string(t),
            Reflectee::Decl(d) | Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
                self.gcc_decl_string(d)
            }
            Reflectee::Template(n) => {
                self.ast.types.template_decl(n).map(|d| self.gcc_decl_string(d)).unwrap_or_default()
            }
            Reflectee::FnParam(f, i) => {
                let fs = self.gcc_decl_string(f);
                match self.param_identifier(f, i) {
                    Some(n) => {
                        format!("<parameter {} of {fs}>", self.ast.symbols().as_str(n))
                    }
                    None => format!("<unnamed parameter {} of {fs}>", i + 1),
                }
            }
            Reflectee::BaseSpec(c, i) => match self.base_spec_ty(c, i) {
                Some(b) => {
                    let d = self.tag_type(c);
                    let b = self.ast.types.canonical(b);
                    format!("{}: {}", self.gcc_type_string(d), self.gcc_type_string(b))
                }
                None => String::new(),
            },
            _ => String::new(),
        };
        if s.is_empty() { self.reflection_str(r) } else { s }
    }

    /// The type of direct base `i` of class `c`.
    fn base_spec_ty(&self, c: DeclId, i: u32) -> Option<kiln_ast::QualType> {
        let def = self.ast.definition(c)?;
        self.ast.decls.bases(def, &self.ast.lists).nth(i as usize).map(|b| b.ty)
    }

    /// The source spelling of constant `v` (spec/reflection.md §3.9): the
    /// diagnostic constant printer in template-argument style without
    /// literal suffixes (`86400`, `'a'`, `Col::red`, `Pt{3}`, `&g`,
    /// `nullptr`), an enumeration value naming no enumerator as a cast
    /// (`(Col)5`), floating point in GCC's form with the type's suffix
    /// (`1.5e+0f`, [`kiln_ast::consts::format_float_gcc`]), a reflection
    /// as `^^` and its operand as GCC prints it (`^^ns::S`, `^^5`;
    /// [`Sema::gcc_reflection_operand`]). `None` when some part has no
    /// spelling (a temporary, an opaque subobject, an unnamed class or
    /// closure type, a reflection with no `^^` operand such as a base
    /// specifier).
    pub(super) fn value_spelling(&self, v: ConstValueId) -> Option<String> {
        self.value_spelling_in(v, false)
    }

    /// [`Self::value_spelling`] as GCC 16.1's diagnostics print the
    /// constant inside a type (`ns::fs<11>{"operator[]"}`, `int [2]{1, 2}`,
    /// `ns::E()`; [`kiln_ast::consts::PrintOptions::gcc`]), the types in
    /// it as `display_string_of` prints them.
    pub(super) fn value_spelling_gcc(&self, v: ConstValueId) -> Option<String> {
        self.value_spelling_in(v, true)
    }

    fn value_spelling_in(&self, v: ConstValueId, gcc: bool) -> Option<String> {
        use kiln_ast::consts::{PrintOptions, PrintStyle};
        let opts = PrintOptions {
            include_type: false,
            enum_cast: true,
            max_depth: u32::from(u16::MAX),
            max_elems: u32::MAX,
            gcc,
            gcc_float: true,
            ..PrintOptions::with_style(PrintStyle::TemplateArg)
        };
        let env = SpellEnv {
            sema: self,
            names: kiln_ast::dump::AstNames::new(&self.ast),
            lossy: Cell::new(false),
            gcc,
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
            return self.value_spelling_gcc(v);
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
        // A named parameter is where its name is, an unnamed one where
        // its decl-specifiers begin (GCC's `DECL_SOURCE_LOCATION`).
        let loc = if let Reflectee::FnParam(f, i) = self.ast.reflections.get(r) {
            let p = self.gcc_param_decl(f, i)?;
            let n = self.ast.decls.get(p);
            match n.name().as_ident() {
                Some(_) => n.loc(),
                None => self.ast.decls.cold.begin_loc.get(p).copied().unwrap_or(n.loc()),
            }
        } else {
            let d = self.named_decl(r)?;
            if self.kind_of(d) == DeclKind::TranslationUnit {
                return None;
            }
            self.ast.decls.get(self.gcc_location_decl(r, d)).loc()
        };
        let cv = self.meta_source_location(loc, "")?;
        self.meta_names.locs.insert(r, cv);
        Some(cv)
    }
}

impl Sema<'_, '_> {
    /// The declaration whose location `source_location_of(r)` reports for
    /// `d`, the first declaration `r` names, as GCC 16.1 keeps one location
    /// per entity (`DECL_SOURCE_LOCATION`, updated as it is redeclared):
    /// a class or enumeration where it is defined, else where first
    /// declared; an implicit or partial specialization of a class
    /// template where its primary template's pattern is (defined, else
    /// declared); a variable or function its definition, else its latest
    /// declaration; anything else (a template, namespace, alias, member)
    /// its first declaration.
    fn gcc_location_decl(&self, r: ReflectionId, d: DeclId) -> DeclId {
        use kiln_ast::templates::SpecKind;
        let k = self.kind_of(d);
        let is_type = matches!(self.ast.reflections.get(r), Reflectee::Type(_));
        if is_type && matches!(k, DeclKind::CXXRecord | DeclKind::Enum) {
            if let Some((_, info)) = self.ast.spec_info(d)
                && info.kind != SpecKind::ExplicitSpecialization
                && let Some(p) = self.ast.template_info(info.template).and_then(|t| t.pattern)
            {
                return self.ast.definition(p).unwrap_or(p);
            }
            return self.ast.definition(d).unwrap_or(d);
        }
        if k.is_function()
            && let Some((_, info)) = self.ast.spec_info(d)
            && info.kind != SpecKind::ExplicitSpecialization
        {
            // A function template specialization is where its template's
            // definition is (instantiated or not).
            if let Some(p) = self.ast.template_info(info.template).and_then(|t| t.pattern) {
                return self.ast.definition(p).unwrap_or(p);
            }
            return info.from;
        }
        if k.is_function() || matches!(k, DeclKind::Var | DeclKind::VarTemplateSpecialization) {
            return self.ast.definition(d).unwrap_or_else(|| self.ast.latest(d));
        }
        d
    }

    /// The declaration of parameter `i` of function `f` that GCC 16.1
    /// keeps: the definition's, else the first declaration's.
    fn gcc_param_decl(&self, f: DeclId, i: u32) -> Option<DeclId> {
        let decl = self.ast.definition(f).unwrap_or_else(|| self.ast.first(f));
        let data = self.ast.decls.function(decl)?;
        self.ast.lists.iter::<DeclId>(data.params).nth(i as usize)
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
    /// Types print as GCC's `display_string_of` (`%T`).
    gcc: bool,
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

    /// A declaration as clang names it, its scope's specializations with
    /// their arguments (`S<int>::m`); for GCC a variable template
    /// specialization with its own as well (`ns::vt<int>`, WP-1258).
    fn decl_name(&self, decl: DeclId, out: &mut String) {
        if self.gcc
            && self
                .sema
                .ast
                .spec_info(decl)
                .is_some_and(|(_, i)| self.sema.kind_of(i.template) == DeclKind::VarTemplate)
            && let Some(s) = self.sema.gcc_reflection_operand(Reflectee::Decl(decl))
        {
            out.push_str(&s);
            return;
        }
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
        if self.gcc {
            out.push_str(&self.sema.gcc_type_string(self.sema.ast.types.canonical(ty)));
        } else {
            out.push_str(&self.sema.ty_canonical_str(ty));
        }
    }

    fn arg_type_name(&self, ty: kiln_ast::QualType, out: &mut String) {
        self.type_name(ty, out);
    }

    fn enumerator(
        &self,
        ty: kiln_ast::QualType,
        bits: &kiln_ast::consts::IntBits<'_>,
        out: &mut String,
    ) -> bool {
        self.names.enumerator(ty, bits, out)
    }

    fn float_suffix(&self, ty: kiln_ast::QualType) -> &'static str {
        self.names.float_suffix(ty)
    }

    /// GCC 16.1's `^^` and operand: a type, namespace, declaration or
    /// template fully qualified ([`Sema::gcc_reflection_operand`]), a
    /// value or object reflection as its spelling (`^^5`, `^^gv`,
    /// `^^ns::In{3}`).
    fn reflection(&self, id: ReflectionId, out: &mut String) {
        let r = self.sema.ast.reflections.get(id);
        let s = match r {
            Reflectee::Null => return self.names.reflection(id, out),
            Reflectee::Value(v) if self.gcc || self.sema.ast.consts.scalar(v).is_none() => {
                self.sema.value_spelling_gcc(v)
            }
            Reflectee::Value(v) => self.sema.value_spelling(v),
            Reflectee::Object(p) => self.sema.object_spelling(p),
            _ => self.sema.gcc_reflection_operand(r),
        };
        if let Some(s) = s.filter(|s| !s.is_empty()) {
            out.push_str("^^");
            out.push_str(&s);
        } else {
            self.lossy.set(true);
            self.names.reflection(id, out);
        }
    }
}
