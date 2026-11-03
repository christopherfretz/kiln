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

use kiln_ast::decl::DeclKind;
use kiln_ast::names::DeclNameKind;
use kiln_ast::reflect::Reflectee;
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
        match self.ast.reflections.get(r) {
            Reflectee::Type(t) => {
                if !t.quals().is_empty() {
                    return None;
                }
                let d = self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t))?;
                self.decl_identifier(d)
            }
            // `Namespace`/`NamespaceAlias` are their own kinds (WP-472).
            Reflectee::Decl(d) | Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
                self.decl_identifier(d)
            }
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
    /// name as a diagnostic prints it (`operator+`, `~X`), a type as
    /// written in a diagnostic, else the reflection as a note prints it.
    fn display_string(&self, r: ReflectionId) -> String {
        let s = match self.ast.reflections.get(r) {
            Reflectee::Type(t) => self.ty_str(t),
            Reflectee::Decl(d) => self.decl_name(d),
            _ => String::new(),
        };
        if s.is_empty() { self.reflection_str(r) } else { s }
    }

    /// `source_location_of(r)` ([meta.reflection.names]/7): the location
    /// of the (first) declaration `r` reflects, with an empty function
    /// name; `None` (a `source_location{}`) for the global namespace,
    /// types other than classes and enumerations, and reflections of no
    /// declaration.
    fn meta_location_of(&mut self, r: ReflectionId) -> Option<ConstValueId> {
        if let Some(&cv) = self.meta_names.locs.get(&r) {
            return Some(cv);
        }
        let d = match self.ast.reflections.get(r) {
            Reflectee::Type(t) => {
                self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t))?
            }
            Reflectee::Decl(d) => d,
            _ => return None,
        };
        if self.kind_of(d) == DeclKind::TranslationUnit {
            return None;
        }
        let loc = self.ast.decls.get(d).loc();
        let cv = self.meta_source_location(loc, "")?;
        self.meta_names.locs.insert(r, cv);
        Some(cv)
    }
}
