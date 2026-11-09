//! The entity queries ([meta.reflection.queries]): `object_of`,
//! `constant_of`, `parent_of`, `dealias`, `has_template_arguments`,
//! `operator_of`, and `symbol_of`/`u8symbol_of` over
//! `std::meta::operators`.

use kiln_ast::consts::{PtrBase, Scalar};
use kiln_ast::decl::{DeclKind, VarEvalState};
use kiln_ast::names::{DeclNameView, OverloadedOperator as Op};
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::TypeView;
use kiln_ast::{ConstValueId, DeclId, QualType, ReflectionId};
use kiln_oven::{MetaError, Oven};
use kiln_source::Loc;

use super::super::fail;
use crate::Sema;

/// `std::meta::operators` in GCC 16.1's order (its value is the index
/// plus one), with each operator's `symbol_of` spelling.
const OPERATORS: [(Op, &str); 44] = [
    (Op::New, "new"),
    (Op::Delete, "delete"),
    (Op::ArrayNew, "new[]"),
    (Op::ArrayDelete, "delete[]"),
    (Op::Coawait, "co_await"),
    (Op::Call, "()"),
    (Op::Subscript, "[]"),
    (Op::Arrow, "->"),
    (Op::ArrowStar, "->*"),
    (Op::Tilde, "~"),
    (Op::Exclaim, "!"),
    (Op::Plus, "+"),
    (Op::Minus, "-"),
    (Op::Star, "*"),
    (Op::Slash, "/"),
    (Op::Percent, "%"),
    (Op::Caret, "^"),
    (Op::Amp, "&"),
    (Op::Equal, "="),
    (Op::Pipe, "|"),
    (Op::PlusEqual, "+="),
    (Op::MinusEqual, "-="),
    (Op::StarEqual, "*="),
    (Op::SlashEqual, "/="),
    (Op::PercentEqual, "%="),
    (Op::CaretEqual, "^="),
    (Op::AmpEqual, "&="),
    (Op::PipeEqual, "|="),
    (Op::EqualEqual, "=="),
    (Op::ExclaimEqual, "!="),
    (Op::Less, "<"),
    (Op::Greater, ">"),
    (Op::LessEqual, "<="),
    (Op::GreaterEqual, ">="),
    (Op::Spaceship, "<=>"),
    (Op::AmpAmp, "&&"),
    (Op::PipePipe, "||"),
    (Op::LessLess, "<<"),
    (Op::GreaterGreater, ">>"),
    (Op::LessLessEqual, "<<="),
    (Op::GreaterGreaterEqual, ">>="),
    (Op::PlusPlus, "++"),
    (Op::MinusMinus, "--"),
    (Op::Comma, ","),
];

/// How many context links a parent walk follows at most (a bound on a
/// malformed chain).
const MAX_CTX: usize = 1024;

impl Sema<'_, '_> {
    /// The reflection `^^E` designates for the class, enumeration,
    /// function or namespace `c` (a parent, [meta.reflection.queries]).
    pub(super) fn entity_reflectee(&mut self, c: DeclId) -> Reflectee {
        let first = self.entity_decl(c);
        match self.kind_of(c) {
            DeclKind::TranslationUnit | DeclKind::Namespace => Reflectee::Namespace(first),
            DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
            | DeclKind::Enum => match self.ast.decls.get(c).ty() {
                Some(t) if t != QualType::ERROR => Reflectee::Type(self.ast.reflection_type(t)),
                _ => Reflectee::Decl(first),
            },
            _ => Reflectee::Decl(first),
        }
    }

    /// The declaration of the entity reflectee `x` represents, if it is
    /// one: a declaration, namespace, namespace alias, template, or the
    /// class, enumeration or alias a type reflection names.
    pub(super) fn reflectee_decl(&self, x: Reflectee) -> Option<DeclId> {
        match x {
            Reflectee::Decl(d) | Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => Some(d),
            Reflectee::Type(t) => self.type_decl(t),
            Reflectee::Template(n) => self.ast.types.template_decl(n),
            _ => None,
        }
    }

    /// The declaration of the class, enumeration, type alias or alias
    /// template specialization `t` is (a type reflection keeps the alias
    /// it was written with, spec/reflection-repr.md §2); `None` for any
    /// other type.
    pub(super) fn type_decl(&self, t: QualType) -> Option<DeclId> {
        if t.quals_bits() == 0 {
            match self.ast.types.view(t.ty()) {
                TypeView::Typedef { decl, .. } => return Some(decl),
                TypeView::TemplateSpecialization { name, alias: true, .. } => {
                    return self.ast.types.template_decl(name);
                }
                _ => {}
            }
        }
        self.ast.types.as_record(t).or_else(|| self.ast.types.as_enum(t))
    }

    /// Whether type reflection `t` is a type alias (a typedef name or an
    /// alias template specialization).
    pub(super) fn is_alias_type(&self, t: QualType) -> bool {
        t.quals_bits() == 0
            && matches!(
                self.ast.types.view(t.ty()),
                TypeView::Typedef { .. } | TypeView::TemplateSpecialization { alias: true, .. }
            )
    }

    /// `parent_of` ([meta.reflection.queries]; `has_parent` is
    /// `is_some`): the enumeration of an enumerator, the derived class of
    /// a direct base, the function of a parameter, else the innermost
    /// class, function or namespace whose scope encloses the target
    /// scope of the declaration (an anonymous union for its members).
    /// `None` for the global namespace, an entity with C language
    /// linkage (a function or variable, `has_c_language_linkage`), a type that is neither a class nor an enumeration (nor an
    /// alias), and what is not an entity.
    pub(super) fn parent_of(&mut self, x: Reflectee) -> Option<Reflectee> {
        let d = match x {
            Reflectee::BaseSpec(c, _) => return Some(self.entity_reflectee(c)),
            Reflectee::FnParam(f, _) => return Some(Reflectee::Decl(self.ast.first(f))),
            _ => self.reflectee_decl(x)?,
        };
        let d = match self.kind_of(d) {
            DeclKind::TranslationUnit => return None,
            // A member of an anonymous union, named from the enclosing
            // class: the union is its parent.
            DeclKind::IndirectField => {
                let chain = self.ast.indirect_field_chain(d);
                self.ast.lists.iter::<DeclId>(chain).last()?
            }
            _ => d,
        };
        // Only functions and variables have a language linkage
        // ([dcl.link]/1): a class, enumeration, alias or namespace
        // declared in an `extern "C"` block has its namespace as parent
        // (the linkage specification is transparent, [dcl.link]/4).
        if self.has_c_language_linkage(Reflectee::Decl(d)) {
            return None;
        }
        let mut c = self.ast.decls.get(d).ctx()?;
        for _ in 0..MAX_CTX {
            match self.kind_of(c) {
                DeclKind::TranslationUnit
                | DeclKind::Namespace
                | DeclKind::CXXRecord
                | DeclKind::ClassTemplateSpecialization
                | DeclKind::ClassTemplatePartialSpecialization
                | DeclKind::Enum => return Some(self.entity_reflectee(c)),
                k if k.is_function() => return Some(self.entity_reflectee(c)),
                _ => c = self.ast.decls.get(c).ctx()?,
            }
        }
        None
    }

    /// `dealias(r)`: the type a type alias denotes (its aliases removed
    /// throughout), the namespace a namespace alias names, else `r`.
    pub(super) fn dealias(&mut self, r: ReflectionId, loc: Loc) -> Result<ReflectionId, MetaError> {
        Ok(match self.ast.reflections.get(r) {
            Reflectee::Type(t) => {
                let c = self.ast.types.canonical(t);
                // `c` and `t` are types, not declarations.
                let same = c == t; // same_entity: raw ok
                if same { r } else { self.intern_reflection(Reflectee::Type(c), loc)? }
            }
            Reflectee::NamespaceAlias(d) => {
                let ns = self.ast.first(self.ast.resolve_alias(d));
                self.intern_reflection(Reflectee::Namespace(ns), loc)?
            }
            _ => r,
        })
    }

    /// `has_template_arguments(r)`: `r` is a specialization of a class,
    /// function, variable or alias template (an alias of a class template
    /// specialization is not: GCC 16.1 agrees).
    pub(super) fn has_template_arguments(&self, x: Reflectee) -> bool {
        match x {
            Reflectee::Type(t) if self.is_alias_type(t) => matches!(
                self.ast.types.view(t.ty()),
                TypeView::TemplateSpecialization { alias: true, .. }
            ),
            Reflectee::Type(t) => {
                self.ast.types.as_record(t).is_some_and(|d| self.ast.spec_info(d).is_some())
            }
            Reflectee::Decl(d) => {
                matches!(
                    self.kind_of(d),
                    DeclKind::Function
                        | DeclKind::CXXMethod
                        | DeclKind::CXXConstructor
                        | DeclKind::CXXConversion
                        | DeclKind::Var
                ) && self.ast.spec_info(d).is_some()
            }
            _ => false,
        }
    }

    /// The overloaded operator an operator function or operator function
    /// template is named by.
    pub(super) fn operator_name(&self, x: Reflectee) -> Option<Op> {
        let f = match x {
            Reflectee::Decl(d) if self.kind_of(d).is_function() => d,
            Reflectee::Template(n) => {
                let t = self.ast.types.template_decl(n)?;
                if self.kind_of(t) != DeclKind::FunctionTemplate {
                    return None;
                }
                self.ast.template_info(t)?.pattern?
            }
            _ => return None,
        };
        match self.ast.names.view(self.ast.decls.get(f).name()) {
            DeclNameView::Operator(op) => Some(op),
            _ => None,
        }
    }

    /// `operator_of(r)`: the `std::meta::operators` value (GCC 16.1's
    /// numbering) of the operator function or template `r`.
    pub(super) fn reflect_operator_of(&self, r: ReflectionId, loc: Loc) -> Result<u8, MetaError> {
        let op = self.operator_name(self.ast.reflections.get(r));
        match op.and_then(|op| OPERATORS.iter().position(|&(o, _)| o == op)) {
            Some(i) => Ok(i as u8 + 1),
            None => fail(
                loc,
                "reflection does not represent an operator function or operator function template",
            ),
        }
    }

    /// `symbol_of(op)` / `u8symbol_of(op)`: the operator's spelling, as
    /// a pointer to a string literal object, and its length.
    pub(super) fn reflect_symbol(
        &mut self,
        op: u128,
        utf8: bool,
        loc: Loc,
    ) -> Result<(ConstValueId, u64), MetaError> {
        let sym = usize::try_from(op)
            .ok()
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| OPERATORS.get(i))
            .map(|&(_, s)| s);
        let Some(sym) = sym else {
            return fail(loc, "operators argument is not a valid operator");
        };
        let cv = self.frozen_string(sym, utf8);
        let cv = cv.ok_or_else(|| MetaError::note(loc, "kiln: string too large for a constant"))?;
        Ok((cv, sym.len() as u64))
    }

    /// `object_of(r)`: `r` for an object; for a variable with static
    /// storage duration, the object it declares (an `Object` reflection,
    /// the interned pointer to it: spec/reflection-repr.md §1).
    pub(super) fn reflect_object_of(
        &mut self,
        r: ReflectionId,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        let x = self.ast.reflections.get(r);
        if let Reflectee::Object(_) = x {
            return Ok(r);
        }
        let not_object = |loc| {
            fail(
                loc,
                "reflection does not represent an object with static storage duration, or a \
                 reference to such an object",
            )
        };
        let Reflectee::Decl(d) = x else { return not_object(loc) };
        let Some(ty) = self.ast.decls.get(d).ty().filter(|&t| t != QualType::ERROR) else {
            return not_object(loc);
        };
        if self.ast.types.is_reference(ty) {
            return self.referenced_object(d, loc, oven);
        }
        if !self.has_static_storage(d) {
            return not_object(loc);
        }
        // Interned cv-unqualified, one per object (`const_object`).
        let ty = self.ast.types.canonical(ty).without_quals();
        let ptr = self.ast.types.pointer_to(ty);
        let base = PtrBase::Decl(self.ast.first(d));
        match self.ast.consts.push_scalar(Scalar::Pointer {
            ty: ptr,
            base,
            offset: 0,
            designated: None,
            past: false,
        }) {
            Ok(cv) => Ok(self.intern_reflection(Reflectee::Object(cv), loc)?),
            Err(_) => fail(loc, "kiln: constant store exhausted"),
        }
    }

    /// `object_of(^^R)` for a reference `R`: the object `R` refers to,
    /// when `R` is usable in constant expressions ([meta.reflection.queries]
    /// `object_of`; [expr.const]/3). Its value is the pointer to that
    /// object: a variable, a template parameter object, a string literal
    /// object or a `std::type_info` object, all static storage (a
    /// permitted result, [expr.const]/22).
    fn referenced_object(
        &mut self,
        d: DeclId,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        let v = self.var_init_value(d, oven).and_then(|cv| self.ast.consts.scalar(cv));
        let Some(Scalar::Pointer { ty, base, offset, .. }) = v else {
            return fail(
                loc,
                format!("reference '{}' is not usable in constant expressions", self.decl_name(d)),
            );
        };
        let static_object = match base {
            PtrBase::Decl(x) => self.has_static_storage(x),
            PtrBase::Const(_)
            | PtrBase::StringLit { .. }
            | PtrBase::TypeInfo(_)
            | PtrBase::Temporary(_) => true,
            _ => false,
        };
        if !static_object {
            return fail(
                loc,
                "reflection does not represent an object with static storage duration, or a \
                 reference to such an object",
            );
        }
        let Some(obj) = self.ast.types.pointee(ty) else {
            return fail(loc, "reference does not refer to an object");
        };
        let ty = self.ast.types.pointer_to(self.ast.types.canonical(obj).without_quals());
        let base = match base {
            PtrBase::Decl(x) => PtrBase::Decl(self.ast.first(x)),
            b => b,
        };
        match self.ast.consts.push_scalar(Scalar::Pointer {
            ty,
            base,
            offset,
            designated: None,
            past: false,
        }) {
            Ok(cv) => Ok(self.intern_reflection(Reflectee::Object(cv), loc)?),
            Err(_) => fail(loc, "kiln: constant store exhausted"),
        }
    }

    /// `constant_of(r)` ([meta.reflection.queries]): `reflect_constant`
    /// of the value `r` designates. A value is its own constant; an
    /// enumerator's is its value as the enumeration type; a variable's
    /// (or an object's) is the value of its constant initializer when the
    /// variable is usable in constant expressions and of integral,
    /// enumeration or `bool` type (other types wait for WP-479's
    /// `reflect_constant`).
    pub(in crate::meta) fn reflect_constant_of(
        &mut self,
        r: ReflectionId,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        let d = match self.ast.reflections.get(r) {
            Reflectee::Value(_) => return Ok(r),
            // An annotation's underlying constant (P3394R4).
            Reflectee::Annotation(a) => return self.annotation_constant(a, loc),
            Reflectee::Object(cv) => match self.ast.consts.scalar(cv) {
                Some(Scalar::Pointer { base: PtrBase::Decl(d), offset: 0, .. }) => d,
                _ => return fail(loc, "kiln: constant_of this object is not implemented yet"),
            },
            Reflectee::Decl(d)
                if matches!(
                    self.kind_of(d),
                    DeclKind::EnumConstant | DeclKind::Var | DeclKind::VarTemplateSpecialization
                ) =>
            {
                d
            }
            _ => {
                return fail(
                    loc,
                    "reflection does not represent an annotation or a valid argument to a splice-expression",
                );
            }
        };
        let Some(ty) = self.ast.decls.get(d).ty().filter(|&t| t != QualType::ERROR) else {
            return fail(loc, "reflection does not have a type");
        };
        let ty = self.ast.types.canonical(ty);
        let value = if self.kind_of(d) == DeclKind::EnumConstant {
            self.ast.decls.enumerator(d).and_then(|e| e.value)
        } else {
            self.var_constant_value(d, ty, loc, oven)?
        };
        match value.and_then(|v| self.const_int(v)) {
            Some(v) => {
                let cv = self.int_const(ty, v);
                Ok(self.intern_reflection(Reflectee::Value(cv), loc)?)
            }
            None => fail(
                loc,
                format!(
                    "the value of '{}' is not usable in a constant expression",
                    self.decl_name(d)
                ),
            ),
        }
    }

    /// The constant value of variable `d` (of canonical type `ty`) when
    /// it is usable in constant expressions ([expr.const]): `constexpr`,
    /// or `const` non-volatile of integral or enumeration type, with a
    /// constant initializer (evaluated now if it was not yet).
    fn var_constant_value(
        &mut self,
        d: DeclId,
        ty: QualType,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<Option<ConstValueId>, MetaError> {
        if self.ast.decls.var(d).is_none() {
            return Ok(None);
        }
        let constexpr = self
            .ast
            .redecls_of(d)
            .any(|x| self.ast.decls.var(x).is_some_and(|y| y.spec.is_constexpr()));
        let integral = crate::constfold::int_info(&self.ast, ty).is_some();
        let quals = self.ast.types.effective_quals(ty);
        let reference = self.ast.types.is_reference(ty);
        // [expr.const]/3: potentially-constant is constexpr, a reference,
        // or a const non-volatile integral or enumeration variable.
        let const_int = integral && quals.is_const() && !quals.is_volatile();
        if !(constexpr || reference || const_int) {
            return Ok(None);
        }
        if !integral {
            return fail(
                loc,
                format!(
                    "kiln: constant_of a variable of type '{}' is not implemented yet",
                    self.ty_str(ty)
                ),
            );
        }
        Ok(self.var_init_value(d, oven))
    }

    /// The value of the constant initializer of variable `d` (evaluated
    /// now if it was not yet); `None` when it has none.
    fn var_init_value(&mut self, d: DeclId, oven: &mut Oven) -> Option<ConstValueId> {
        let holder = self
            .ast
            .redecls_of(d)
            .find(|&x| self.ast.decls.var(x).is_some_and(|y| y.init.is_some()))?;
        let data = |s: &Self| s.ast.decls.var(holder).map(|y| (y.eval.state(), y.init));
        match data(self) {
            Some((VarEvalState::Value(cv), _)) => Some(cv),
            Some((VarEvalState::NotEvaluated, Some(init))) => {
                // A const variable that is not constexpr has no recorded
                // value: its initializer, if it is a constant initializer
                // ([basic.start.static]/2), gives it.
                // A reference's value is the object it binds, which only
                // the initializer evaluated as the variable's gives.
                let reference =
                    self.ast.decls.get(holder).ty().is_some_and(|t| self.ast.types.is_reference(t));
                self.lend_oven(oven, |s| {
                    if reference {
                        return s.fold_constant_init(init, holder);
                    }
                    if !s.is_constant_initialized(holder) {
                        return None;
                    }
                    match data(s) {
                        Some((VarEvalState::Value(cv), _)) => Some(cv),
                        _ => s.try_fold(init),
                    }
                })
            }
            _ => None,
        }
    }
}
