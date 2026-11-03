//! Reflection (spike, `-freflection`; WP-267; spec/sema.md "Reflection
//! (spike, -freflection)"; P2996R13 [expr.reflect], [basic.splice],
//! [meta.reflection]).
//!
//! `^^operand` builds a `CXXReflectExpr` whose constant value is a
//! [`ReflectionId`] interned in `ast.reflections` (see
//! `kiln_ast::reflect` for the entity mapping). A type splice evaluates
//! its operand through the Oven and maps the id back. The `std::meta`
//! queries are consteval intrinsics the Oven dispatches by name through
//! [`kiln_oven::SemaBridge::meta_intrinsic`]; their sema halves live here.

use kiln_ast::consts::Scalar;
use kiln_ast::decl::{Access, DeclFlags, DeclKind};
use kiln_ast::names::DeclName;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::BuiltinType;
use kiln_ast::{ConstValueId, DeclHead, DeclId, ExprId, QualType, ReflectionId};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind, MetaOp, Note, Oven};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::lookup::LookupKind;
use crate::using::Qualifier;

impl Sema<'_, '_> {
    /// The type `std::meta::info` (spelled `decltype(^^int)`).
    pub fn meta_info_type(&self) -> QualType {
        BuiltinType::MetaInfo.qual_type()
    }

    /// True if `t` is (cv) `std::meta::info`.
    pub fn is_meta_info(&self, t: QualType) -> bool {
        self.ast.types.canonical(t).without_quals() == self.meta_info_type()
    }

    /// `^^type-id` ([expr.reflect]/2).
    pub fn act_on_reflect_type(&mut self, ty: QualType, loc: Loc, end: Loc) -> ExprId {
        let info = self.meta_info_type();
        if ty == QualType::ERROR {
            return self.recover(info, &[], loc);
        }
        self.build_reflect_expr(Reflectee::Type(ty), info, loc, end)
    }

    /// `^^name` / `^^nns name` ([expr.reflect]/3-5): a namespace, a
    /// variable, a non-overloaded function or a data member. Overload
    /// sets, templates and anything else are not reflected by the spike.
    pub fn act_on_reflect_name(
        &mut self,
        qual: Option<Qualifier>,
        name: Symbol,
        name_loc: Loc,
        loc: Loc,
        end: Loc,
    ) -> ExprId {
        let info = self.meta_info_type();
        let lookup = |s: &Self, k| match qual {
            Some(q) => s.lookup_qualified(q.ctx, name, k),
            None => s.lookup_name(name, k),
        };
        let mut found = lookup(self, LookupKind::Ordinary);
        if found.decls.is_empty() {
            found = lookup(self, LookupKind::Nested);
        }
        let text = crate::decl::sym_str(self, name);
        match found.decls.as_slice() {
            [] => {
                let d = self.diag(DiagId::sema_undeclared_var_use, name_loc).arg(text);
                self.emit(d);
                self.recover(info, &[], loc)
            }
            &[d] => match self.kind_of(d) {
                // Spike: an alias reflects the namespace it denotes (P2996
                // reflects the alias itself; `dealias` unwraps it).
                DeclKind::NamespaceAlias => {
                    let d = self.entity_decl(self.ast.resolve_alias(d));
                    self.build_reflect_expr(Reflectee::Decl(d), info, loc, end)
                }
                DeclKind::Namespace
                | DeclKind::Var
                | DeclKind::Field
                | DeclKind::Function
                | DeclKind::CXXMethod => {
                    // One reflection per entity (`Sema::same_entity`).
                    let d = self.entity_decl(d);
                    self.build_reflect_expr(Reflectee::Decl(d), info, loc, end)
                }
                _ => {
                    let d =
                        self.diag(DiagId::sema_reflect_operand, name_loc).arg(format!("'{text}'"));
                    self.emit(d);
                    self.recover(info, &[], loc)
                }
            },
            _ => {
                let d = self
                    .diag(DiagId::sema_reflect_operand, name_loc)
                    .arg(format!("overload set '{text}'"));
                self.emit(d);
                self.recover(info, &[], loc)
            }
        }
    }

    /// The type a splice-type-specifier `[: e :]` designates
    /// ([dcl.type.splice]/2): `e` must be a converted constant expression
    /// of type `std::meta::info` reflecting a type.
    pub fn act_on_splice_type(&mut self, e: ExprId, open: Loc, close: Loc) -> Option<QualType> {
        let _ = close;
        if self.expr_has_error(e) {
            return None;
        }
        let at = self.begin_loc(e);
        if self.expr_is_value_dependent(e) || self.is_dependent_type(self.ast.exprs.get(e).ty()) {
            self.unsupported("dependent splice", open);
            return None;
        }
        let ety = self.ast.exprs.get(e).ty();
        if !self.is_meta_info(ety) {
            let d = self.diag(DiagId::sema_splice_not_info, at).arg(self.ty_arg(ety));
            self.emit(d);
            return None;
        }
        let e = crate::conv::decay(self, e);
        let v = match self.eval_const(e, EvalContext::new(EvalKind::Other, at)) {
            Ok(v) => v,
            Err(f) => {
                let d = self.diag(DiagId::sema_splice_not_constant, at);
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                return None;
            }
        };
        let r = match self.ast.consts.scalar(v) {
            Some(Scalar::Reflection { id, .. }) => self.ast.reflections.get(id),
            _ => Reflectee::Null,
        };
        if let Reflectee::Type(t) = r {
            Some(t)
        } else {
            self.error(DiagId::sema_splice_not_type, at);
            None
        }
    }
}

/// The reason note of a failed reflection intrinsic.
fn fail<T>(loc: Loc, msg: impl Into<String>) -> Result<T, Vec<Note>> {
    Err(vec![Note::new(loc, msg)])
}

/// Whether `s` is an identifier ([lex.name]/1, basic source characters).
fn is_identifier(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next().is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && cs.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

impl Sema<'_, '_> {
    /// The reflection intrinsic `f` is (spec/sema.md "Reflection (spike,
    /// -freflection)"): `std::meta::{type_of, name_of,
    /// __kiln_members_count, __kiln_member_at}` and the global
    /// `__kiln_define_member`, recognized by name and context only under
    /// `-freflection`.
    pub(crate) fn meta_intrinsic_of(&self, f: DeclId) -> Option<MetaOp> {
        if !self.reflection {
            return None;
        }
        let name = self.decl_name(f);
        let ctx = self.ast.decls.get(f).ctx()?;
        if name == "__kiln_define_member" {
            return (self.kind_of(ctx) == DeclKind::TranslationUnit)
                .then_some(MetaOp::DefineMember);
        }
        let ns =
            |d: DeclId, n: &str| self.kind_of(d) == DeclKind::Namespace && self.decl_name(d) == n;
        let parent = self.ast.decls.get(ctx).ctx()?;
        if !ns(ctx, "meta")
            || !ns(parent, "std")
            || self
                .ast
                .decls
                .get(parent)
                .ctx()
                .is_none_or(|t| self.kind_of(t) != DeclKind::TranslationUnit)
        {
            return None;
        }
        Some(match name.as_str() {
            "type_of" => MetaOp::TypeOf,
            "name_of" => MetaOp::NameOf,
            "__kiln_members_count" => MetaOp::MembersCount,
            "__kiln_member_at" => MetaOp::MemberAt,
            _ => return None,
        })
    }

    /// `r` printed for a note.
    fn reflection_str(&self, r: ReflectionId) -> String {
        use kiln_ast::consts::PrintEnv as _;
        let mut s = String::new();
        self.ast.reflection(r, &mut s);
        s
    }

    /// `type_of(r)` ([meta.reflection.queries]): the type of a variable,
    /// data member or function; the spike reflects it canonically.
    pub(crate) fn reflect_type_of(
        &mut self,
        r: ReflectionId,
        loc: Loc,
    ) -> Result<ReflectionId, Vec<Note>> {
        let ty = match self.ast.reflections.get(r) {
            Reflectee::Decl(d)
                if matches!(
                    self.kind_of(d),
                    DeclKind::Var | DeclKind::Field | DeclKind::Function | DeclKind::CXXMethod
                ) =>
            {
                self.ast.decls.get(d).ty()
            }
            _ => None,
        };
        match ty {
            Some(t) if t != QualType::ERROR => {
                let t = self.ast.types.canonical(t);
                self.intern_reflection(Reflectee::Type(t), loc)
            }
            _ => fail(loc, format!("'{}' has no type", self.reflection_str(r))),
        }
    }

    /// `name_of(r)`: the identifier of a declaration, or a type's name.
    /// The spike returns a `const char *` to a string literal object
    /// (one per content, shared with `std::source_location`'s).
    pub(crate) fn reflect_name(
        &mut self,
        r: ReflectionId,
        loc: Loc,
    ) -> Result<ConstValueId, Vec<Note>> {
        let name = match self.ast.reflections.get(r) {
            Reflectee::Null => String::new(),
            Reflectee::Decl(d) => self.decl_name(d),
            Reflectee::Type(t) => match self.ast.types.as_record(t) {
                Some(d) => self.decl_name(d),
                None => self.ty_str(t),
            },
        };
        if name.is_empty() {
            return fail(loc, format!("'{}' has no name", self.reflection_str(r)));
        }
        match self.srcloc_string(&name) {
            Some(cv) => Ok(cv),
            None => fail(loc, "kiln: name too large for a constant"),
        }
    }

    /// The class type reflected by `r`, completed (instantiated at `loc`
    /// if it is an implicitly instantiable specialization; `oven` lent).
    fn reflected_complete_class(
        &mut self,
        r: ReflectionId,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<DeclId, Vec<Note>> {
        let Reflectee::Type(t) = self.ast.reflections.get(r) else {
            return fail(
                loc,
                format!("'{}' does not reflect a class type", self.reflection_str(r)),
            );
        };
        let Some(d) = self.ast.types.as_record(t) else {
            return fail(
                loc,
                format!("'{}' does not reflect a class type", self.reflection_str(r)),
            );
        };
        if self.undefined_spec(t).is_some() {
            if self.inst.eval_loc.is_none() {
                return Err(Vec::new());
            }
            self.lend_oven(oven, |s| s.require_complete(t, loc));
        }
        match self.ast.definition(d) {
            Some(def) if self.is_complete(t) => Ok(def),
            _ => fail(loc, format!("'{}' reflects an incomplete class", self.reflection_str(r))),
        }
    }

    /// The members of the class `r` reflects, in declaration order
    /// ([meta.reflection.member.queries], `members_of`): every named
    /// member declared in the class (an implicitly declared special member
    /// is excluded: kiln declares those lazily, so whether one exists
    /// depends on what was used before).
    pub(crate) fn reflect_members(
        &mut self,
        r: ReflectionId,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<Vec<ReflectionId>, Vec<Note>> {
        let def = self.reflected_complete_class(r, loc, oven)?;
        let members: Vec<DeclId> = self
            .ast
            .members(def)
            .filter(|&m| {
                let n = self.ast.decls.get(m);
                !n.has(DeclFlags::IMPLICIT)
                    && !self.decl_name(m).is_empty()
                    && !matches!(
                        n.kind(),
                        DeclKind::AccessSpec | DeclKind::StaticAssert | DeclKind::Friend
                    )
            })
            .map(|m| self.entity_decl(m))
            .collect();
        members.into_iter().map(|m| self.intern_reflection(Reflectee::Decl(m), loc)).collect()
    }

    /// `__kiln_define_member(cls, ty, name)` (the spike's experiment;
    /// spec/sema.md "Reflection (spike, -freflection)"): appends the
    /// public non-static data member `name` of type `ty` to `cls` while
    /// the Oven evaluates. Permitted only in a manifestly
    /// constant-evaluated context while `cls` is the innermost class
    /// being defined (its member-specification is being processed: a
    /// `static_assert` or another constant expression among its member
    /// declarations, including an instantiation of it); the member is
    /// placed where the evaluation happens in the member-specification.
    pub(crate) fn define_member(
        &mut self,
        cls: ReflectionId,
        ty: ReflectionId,
        name: &str,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<ReflectionId, Vec<Note>> {
        // A speculative fold must not mutate: it fails silently and the
        // manifestly constant-evaluated evaluation (if any) defines.
        if self.inst.eval_loc.is_none() {
            return Err(Vec::new());
        }
        let Some(r) = (match self.ast.reflections.get(cls) {
            Reflectee::Type(t) => self.ast.types.as_record(t),
            _ => None,
        }) else {
            return fail(
                loc,
                format!("'{}' does not reflect a class type", self.reflection_str(cls)),
            );
        };
        // `^^S` names the first declaration of `S`; the class being
        // defined is its definition: compare entities, not decls.
        let building = self.classes.last().map(|c| c.record);
        let Some(r) = building.filter(|&b| self.same_entity(b, r)) else {
            let cname = self.decl_name(r);
            let mut notes = vec![Note::new(
                loc,
                format!(
                    "'__kiln_define_member' can only add a member to the class whose \
                     member-specification is being evaluated ('{cname}' is not)"
                ),
            )];
            if let Some(b) = building {
                let bloc = self.ast.decls.get(b).loc();
                notes.push(Note::new(
                    bloc,
                    format!("class being defined is '{}'", self.decl_name(b)),
                ));
            }
            return Err(notes);
        };
        let Reflectee::Type(t) = self.ast.reflections.get(ty) else {
            return fail(loc, format!("'{}' does not reflect a type", self.reflection_str(ty)));
        };
        if self.undefined_spec(t).is_some() {
            self.lend_oven(oven, |s| s.require_complete(t, loc));
        }
        if self.ast.types.is_function(t)
            || self.ast.types.is_reference(t)
            || !self.is_complete(t)
            || self.ast.types.as_record(t).is_some_and(|d| self.same_entity(d, r))
        {
            return fail(
                loc,
                format!(
                    "'{}' is not a complete object type for a data member",
                    self.reflection_str(ty)
                ),
            );
        }
        if !is_identifier(name) {
            return fail(loc, format!("'{name}' is not an identifier"));
        }
        let sym = self.ast.symbols().intern(name);
        let class_loc = self.ast.decls.get(r).loc();
        let in_eval = Note::new(
            class_loc,
            format!(
                "in evaluation of '__kiln_define_member' for class '{}' being defined here",
                self.decl_name(r)
            ),
        );
        if let Some(&prev) = self.lookup_in(r, sym, LookupKind::Ordinary).decls.first() {
            let ploc = self.ast.decls.get(prev).loc();
            return Err(vec![
                Note::new(loc, format!("duplicate member '{name}'")),
                Note::new(ploc, "previous declaration is here"),
                in_eval,
            ]);
        }
        let mut h = DeclHead::new(r, DeclName::ident(sym), loc);
        h.flags = DeclFlags::INJECTED;
        let f = self.builder().field(h, t, None, 0);
        self.ast.decls.set_access(f, Access::Public);
        self.intern_reflection(Reflectee::Decl(f), loc)
    }

    /// Runs `f` with the running Oven (`oven`, from a bridge callback)
    /// lent back to sema, so the work can evaluate (nested, R-OVEN-1).
    pub(crate) fn lend_oven<R>(&mut self, oven: &mut Oven, f: impl FnOnce(&mut Self) -> R) -> R {
        debug_assert!(self.oven.is_none(), "lend_oven outside an evaluation");
        let lent = std::mem::replace(oven, Oven::new(oven.budget()));
        self.oven = Some(lent);
        let r = f(self);
        if let Some(o) = self.oven.take() {
            *oven = o;
        }
        r
    }
}
