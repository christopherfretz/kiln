//! Run-time type identification: `typeid` ([expr.typeid]; spec/sema.md
//! §5.6). Kiln checks and builds the expression; the Oven and codegen do
//! not evaluate it yet.

use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::expr::{CastKind, TypeidOperand};
use kiln_ast::types::{Nns, Quals, TagKeyword, TypeKind};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv;
use crate::lookup::LookupKind;

impl Sema<'_, '_> {
    /// The class `std::type_info` ([type.info]), clang's
    /// `CXXTypeInfoDecl`: a class named `type_info` in the global
    /// namespace `std`, if both are declared.
    fn std_type_info(&self) -> Option<(DeclId, DeclId)> {
        let tu = self.ast.tu;
        let std = self.intern("std");
        let ns = self
            .lookup_in(tu, std, LookupKind::Nested)
            .single()
            .filter(|&d| self.ast.decls.get(d).kind() == DeclKind::Namespace)?;
        let name = self.intern("type_info");
        let d = self.lookup_qualified(ns, name, LookupKind::Tag).single()?;
        self.ast.types.as_record(self.tag_type(d)).map(|_| (ns, d))
    }

    /// `typeid ( type-id )` (`operand` is `Err(type)`) or
    /// `typeid ( expression )` (`Ok(e)`, parsed as an unevaluated operand)
    /// ([expr.typeid]): an lvalue of type `const std::type_info`.
    ///
    /// - `std::type_info` must have been declared ([expr.typeid]/7; clang's
    ///   `err_need_header_before_typeid`), and RTTI enabled;
    /// - references and top-level cv-qualifiers are ignored
    ///   ([expr.typeid]/4,5); a cv-qualified expression operand gets a
    ///   `NoOp` cast to its unqualified type, as clang;
    /// - a class type must be complete ([expr.typeid]/4; clang's
    ///   `err_incomplete_typeid` at the `typeid` keyword).
    ///
    /// An expression operand is evaluated only if it is a glvalue of
    /// polymorphic class type ([expr.typeid]/3). Its names were still
    /// checked as an unevaluated operand (clang re-transforms it; a
    /// known deviation, spec/sema-class.md §8).
    pub fn act_on_typeid(
        &mut self,
        operand: Result<ExprId, QualType>,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let kids: Vec<ExprId> = operand.ok().into_iter().collect();
        let Some((std, info)) = self.std_type_info() else {
            self.error(DiagId::sema_need_header_before_typeid, loc);
            return self.recover_err(&kids, loc);
        };
        if !self.rtti {
            self.error(DiagId::sema_typeid_no_rtti, loc);
            return self.recover_err(&kids, loc);
        }
        if operand.is_ok_and(|e| self.is_err_expr(e)) {
            return self.recover_err(&kids, loc);
        }
        let written = match operand {
            Ok(e) => self.ety(e),
            Err(t) => self.non_reference(t),
        };
        if written == QualType::ERROR {
            return self.recover_err(&kids, loc);
        }
        let unqual = conv::strip_cv(self, written);
        if (operand.is_err() && self.is_incomplete_dependent_record(unqual))
            || self.ast.types.as_record(unqual).is_some() && !self.require_complete(unqual, loc)
        {
            let msg = self.diag(DiagId::sema_typeid_incomplete, loc).arg(self.ty_arg(unqual));
            self.emit(self.incomplete_note(msg, unqual));
            return self.recover_err(&kids, loc);
        }
        // [expr.typeid]/3: a glvalue of polymorphic class type is
        // evaluated (WP-078a).
        let evaluated = operand.is_ok_and(|e| self.is_polymorphic_glvalue(e));
        if let Ok(e) = operand
            && !evaluated
        {
            self.check_unevaluated_side_effects(e);
        }
        if let Ok(e) = operand
            && evaluated
        {
            self.check_typeid_side_effects(e);
        }
        let operand = match operand {
            Ok(e) if !self.ast.types.effective_quals(written).is_empty() => {
                let vk = self.ast.exprs.get(e).value_category();
                TypeidOperand::Expr(conv::raw_cast(self, CastKind::NoOp, e, unqual, vk, None))
            }
            Ok(e) => TypeidOperand::Expr(e),
            Err(t) => TypeidOperand::Type(t),
        };
        // clang's type is the bare `RecordType`, printed with its scope:
        // kiln spells that `std::type_info` (spec/types.md §15 R4).
        let rec = self.ast.types.canonical(self.tag_type(info));
        let nns = self.ast.types.nns(Nns::Namespace { prefix: None, ns: self.ast.first(std) });
        let named = self.ast.types.elaborated(TagKeyword::None, Some(nns), rec);
        let ty = self.ast.types.qualify(named, Quals::CONST);
        self.builder().typeid_expr(operand, evaluated, ty, loc, rparen)
    }

    /// Whether the type-id operand `t` (cv-unqualified, not a reference)
    /// names a dependent class that has no definition yet and is neither
    /// a class template's pattern nor one of its specializations: a
    /// local class or a member class of the current instantiation that
    /// is only declared. clang diagnoses such an operand where the
    /// template is defined, before any instantiation, because its
    /// `RequireCompleteType` looks at the record itself, not at whether
    /// the type is dependent (`BuildCXXTypeId`; spec/sema.md §5.6). An
    /// injected-class-name or a template-id is not a plain record type
    /// to clang and waits for instantiation. GCC 16.1 diagnoses at
    /// instantiation only; kiln follows clang (WP-884).
    fn is_incomplete_dependent_record(&self, t: QualType) -> bool {
        let types = &self.ast.types;
        if !types.is_dependent(t) || types.canonical_node(t).kind() != TypeKind::Record {
            return false;
        }
        let Some(r) = types.as_record(t) else { return false };
        self.ast.definition(r).is_none()
            && self.ast.described_template(r).is_none()
            && self.ast.spec_info(r).is_none()
    }

    /// Whether `e` is a glvalue of (possibly cv-qualified) polymorphic
    /// class type: the operand of `typeid` that is evaluated
    /// ([expr.typeid]/3).
    pub(crate) fn is_polymorphic_glvalue(&self, e: ExprId) -> bool {
        let x = self.ast.exprs.get(e);
        x.value_category().is_glvalue()
            && self
                .ast
                .types
                .as_record(x.ty())
                .and_then(|r| self.ast.definition(r))
                .and_then(|d| self.ast.decls.record(d))
                .is_some_and(|r| r.defn.contains(DefinitionBits::POLYMORPHIC))
    }
}
