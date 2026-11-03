//! The sema half of materialization (spec/reflection.md §3.5, §8.2):
//! which recipe shape an intrinsic's class-typed result or parameter
//! has, and the `std::meta::exception` a failing intrinsic throws. The
//! Oven matches the shape against the class's layout
//! (`kiln_oven/src/meta/materialize.rs`).

use kiln_ast::decl::DeclKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::BuiltinType;
use kiln_ast::{DeclId, QualType};
use kiln_oven::{AccessOp, AnnotOp, ExcCtor, MemberOp, MetaOp, MetaThrow, QueryOp, Shape, TemplOp};
use kiln_source::Loc;

use crate::Sema;
use crate::lookup::LookupKind;

impl Sema<'_, '_> {
    /// The [`Shape`] of op `op`'s class-typed result (`param: None`) or
    /// argument `param`, of class type `q` (`SemaBridge::meta_shape`).
    ///
    /// # Errors
    /// Why `q` cannot hold the op's result (a string view whose
    /// characters are not the op's).
    pub(crate) fn meta_shape(
        &mut self,
        op: MetaOp,
        param: Option<usize>,
        q: QualType,
    ) -> Result<Option<Shape>, String> {
        use MetaOp as M;
        let Some(i) = param else {
            return match op {
                M::Query(QueryOp::IdentifierOf | QueryOp::DisplayStringOf | QueryOp::SymbolOf) => {
                    self.string_shape(q, QualType::CHAR)
                }
                M::Query(
                    QueryOp::U8IdentifierOf | QueryOp::U8DisplayStringOf | QueryOp::U8SymbolOf,
                ) => self.string_shape(q, QualType::CHAR8),
                M::Query(QueryOp::SourceLocationOf) => Ok(Some(Shape::SrcLoc)),
                M::Query(QueryOp::KilnReflectionRange)
                | M::Member(
                    MemberOp::MembersOf
                    | MemberOp::BasesOf
                    | MemberOp::StaticDataMembersOf
                    | MemberOp::NonstaticDataMembersOf
                    | MemberOp::SubobjectsOf
                    | MemberOp::EnumeratorsOf
                    | MemberOp::ParametersOf,
                )
                | M::Annot(AnnotOp::AnnotationsOf | AnnotOp::AnnotationsOfWithType)
                | M::Template(TemplOp::TemplateArgumentsOf) => Ok(Some(Shape::Vector)),
                M::Query(QueryOp::KilnMemberOffset | QueryOp::KilnAccessContext)
                | M::Member(MemberOp::OffsetOf)
                | M::Access(AccessOp::Current) => Ok(Some(Shape::Pair)),
                _ => Ok(None),
            };
        };
        Ok(match (op, i) {
            (M::Query(QueryOp::KilnReflectionRange), 0)
            | (M::Template(TemplOp::CanSubstitute | TemplOp::Substitute), 1) => Some(Shape::Range),
            // An access context (scope, designating class).
            (
                M::Member(
                    MemberOp::MembersOf
                    | MemberOp::BasesOf
                    | MemberOp::StaticDataMembersOf
                    | MemberOp::NonstaticDataMembersOf
                    | MemberOp::SubobjectsOf,
                )
                | M::Access(
                    AccessOp::IsAccessible
                    | AccessOp::HasInaccessibleNonstaticDataMembers
                    | AccessOp::HasInaccessibleBases
                    | AccessOp::HasInaccessibleSubobjects,
                ),
                1,
            ) => Some(Shape::Pair),
            _ => None,
        })
    }

    /// A string view of `elem` characters: `q` must hold a pointer to
    /// (const) `elem` (the Oven checks the rest of the layout).
    fn string_shape(&self, q: QualType, elem: QualType) -> Result<Option<Shape>, String> {
        if self.holds_char_ptr(q, elem) {
            Ok(Some(Shape::Str))
        } else {
            Err(format!("does not hold a 'const {} *'", self.ty_str(elem)))
        }
    }

    /// Whether class `q` has a direct non-static data member of type
    /// pointer to (cv) `elem`.
    fn holds_char_ptr(&self, q: QualType, elem: QualType) -> bool {
        let Some(d) = self.ast.types.as_record(q).and_then(|d| self.ast.definition(d)) else {
            return false;
        };
        self.ast.members(d).any(|m| {
            let n = self.ast.decls.get(m);
            n.kind() == DeclKind::Field
                && n.ty()
                    .and_then(|t| self.ast.types.pointee(self.ast.types.canonical(t)))
                    .is_some_and(|p| self.ast.types.canonical(p).without_quals() == elem)
        })
    }

    /// The constructor `(string_view what, info from, source_location
    /// where)` of a complete `std::meta::exception` (P3560R2), preferring
    /// a `u8string_view` message; found once (`SemaBridge::meta_exception`).
    pub(crate) fn meta_exception_ctor(&mut self) -> Option<ExcCtor> {
        if let Some((x, _)) = self.meta_names.exc {
            return Some(x);
        }
        let std = self.intern("std");
        let std = self
            .lookup_in(self.ast.tu, std, LookupKind::Nested)
            .single()
            .filter(|&d| self.kind_of(d) == DeclKind::Namespace)?;
        let meta = self.intern("meta");
        let meta = self
            .lookup_qualified(std, meta, LookupKind::Nested)
            .single()
            .filter(|&d| self.kind_of(d) == DeclKind::Namespace)?;
        let exc = self.intern("exception");
        let cls = self.lookup_qualified(meta, exc, LookupKind::Tag).single()?;
        let def = self.ast.definition(cls)?;
        self.ast.decls.record(def)?;
        let class = self.ast.types.canonical(self.tag_type(def));
        if !self.is_complete(class) {
            return None;
        }
        let info = BuiltinType::MetaInfo.qual_type();
        let ctors: Vec<DeclId> = self
            .ast
            .members(def)
            .filter(|&m| self.kind_of(m) == DeclKind::CXXConstructor)
            .collect();
        let mut best: Option<(ExcCtor, bool)> = None;
        for c in ctors {
            if self.fn_spec(c).is_deleted() {
                continue;
            }
            let Some(proto) = self.ast.decls.get(c).ty().and_then(|t| self.ast.types.proto(t))
            else {
                continue;
            };
            let ps: Vec<QualType> =
                self.ast.types.params(proto).iter().map(|&p| self.ast.types.canonical(p)).collect();
            let [what, from, at] = ps[..] else { continue };
            if from.without_quals() != info
                || !self.by_value_trivial(what)
                || !self.by_value_trivial(at)
            {
                continue;
            }
            let (what, at) = (what.without_quals(), at.without_quals());
            let u8 = self.holds_char_ptr(what, QualType::CHAR8);
            if !u8 && !self.holds_char_ptr(what, QualType::CHAR) {
                continue;
            }
            if best.is_none_or(|(_, b)| u8 && !b) {
                best = Some((ExcCtor { class, ctor: c, what, at }, u8));
            }
        }
        self.meta_names.exc = best;
        best.map(|(x, _)| x)
    }

    /// Whether parameter type `t` is a trivially destructible class taken
    /// by value (the throw block builds it in a temporary it does not
    /// destroy).
    fn by_value_trivial(&self, t: QualType) -> bool {
        self.ast
            .types
            .as_record(t)
            .and_then(|d| self.ast.definition(d))
            .is_some_and(|d| self.ast.destructor(d).is_none_or(|x| self.fn_spec(x).is_trivial()))
    }

    /// The exception the failing call of `callee` at `loc` throws, with
    /// message `what` (`SemaBridge::meta_throw`): `from()` is `^^callee`,
    /// `where()` the call's location.
    pub(crate) fn meta_throw(
        &mut self,
        _op: MetaOp,
        callee: DeclId,
        what: &str,
        loc: Loc,
    ) -> Option<MetaThrow> {
        let (_, u8) = self.meta_names.exc?;
        let what_cv = self.frozen_string(what, u8)?;
        let from = self.ast.reflections.intern(Reflectee::Decl(self.ast.first(callee)));
        let at = self.meta_source_location(loc, "");
        Some(MetaThrow { what: what_cv, len: what.len() as u64, from, at })
    }
}
