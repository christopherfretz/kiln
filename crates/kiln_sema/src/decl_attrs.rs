//! Which declarations and positions an attribute may appertain to
//! ([dcl.attr.grammar]/5, [dcl.attr]; spec/sema.md §4.5): the subject
//! table shared by every declarator, alias-declaration, class-head and
//! enum-head (`check_decl_attrs`), and the attributes written where only
//! type attributes may appear (`act_on_type_position_attrs`). Wording
//! follows clang 19 (`diagAppertainsToDecl`, `handleWarnUnusedResult`,
//! `err_attribute_not_type_attr`).

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::DeclKind;
use kiln_ast::{AttrId, DeclId, QualType};
use kiln_diag::DiagId;

use crate::Sema;

/// The attribute's name as clang prints it (no scope, no `__` affixes);
/// `None` for the vector and other type attributes, which may appear in
/// a type position.
fn attr_name(node: &AttrNode) -> Option<&'static str> {
    Some(match node.kind {
        AttrKind::NODISCARD if node.is_gnu_spelling() => "warn_unused_result",
        AttrKind::NODISCARD => "nodiscard",
        AttrKind::ALIGNED if node.syntax == AttrSyntax::Keyword => "alignas",
        AttrKind::ALIGNED => "aligned",
        AttrKind::ASSUME => "assume",
        AttrKind::CARRIES_DEPENDENCY => "carries_dependency",
        AttrKind::DEPRECATED => "deprecated",
        AttrKind::FALLTHROUGH => "fallthrough",
        AttrKind::LIKELY => "likely",
        AttrKind::UNLIKELY => "unlikely",
        AttrKind::MAYBE_UNUSED => "maybe_unused",
        AttrKind::NORETURN => "noreturn",
        AttrKind::NO_UNIQUE_ADDRESS => "no_unique_address",
        AttrKind::ALWAYS_INLINE => "always_inline",
        AttrKind::NOINLINE => "noinline",
        AttrKind::PACKED => "packed",
        AttrKind::USED => "used",
        AttrKind::UNUSED => "unused",
        AttrKind::WEAK => "weak",
        AttrKind::CONST => "const",
        AttrKind::PURE => "pure",
        AttrKind::COLD => "cold",
        AttrKind::HOT => "hot",
        AttrKind::ABI_TAG => "abi_tag",
        AttrKind::MALLOC => "malloc",
        AttrKind::ALLOC_SIZE => "alloc_size",
        AttrKind::ALLOC_ALIGN => "alloc_align",
        AttrKind::USING_IF_EXISTS => "using_if_exists",
        AttrKind::FLAG_ENUM => "flag_enum",
        AttrKind::ENUM_EXTENSIBILITY => "enum_extensibility",
        AttrKind::EXCLUDE_FROM_EXPLICIT_INSTANTIATION => "exclude_from_explicit_instantiation",
        _ => return None,
    })
}

const fn is_record_kind(kind: DeclKind) -> bool {
    matches!(
        kind,
        DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
    )
}

const fn is_var_kind(kind: DeclKind) -> bool {
    matches!(
        kind,
        DeclKind::Var
            | DeclKind::VarTemplateSpecialization
            | DeclKind::VarTemplatePartialSpecialization
            | DeclKind::ParmVar
    )
}

impl Sema<'_, '_> {
    /// Attributes written where only type attributes may appear: at the
    /// end of a decl-specifier-seq (`int [[deprecated]] x;`,
    /// [dcl.spec.general]/1), after a ptr-operator or array declarator
    /// ([dcl.ptr]/1, [dcl.array]/1), after a function declarator
    /// (`void f() [[noreturn]];`, [dcl.fct]/1, `after_function`) or at
    /// the end of an alias-declaration's type-id. A declaration
    /// attribute there is `'X' attribute cannot be applied to types`
    /// (clang `err_attribute_not_type_attr`), or, written `[[gnu::X]]`
    /// after a function declarator, `attribute 'X' ignored, because it
    /// cannot be applied to a type`; it is dropped. GNU-syntax and
    /// vector attributes are kept.
    pub fn act_on_type_position_attrs(&mut self, attrs: &mut Vec<AttrId>, after_function: bool) {
        attrs.retain(|&a| {
            let node = *self.ast.attrs.get(a);
            if matches!(node.syntax, AttrSyntax::Gnu | AttrSyntax::Declspec) {
                return true;
            }
            let Some(name) = attr_name(&node) else {
                return true;
            };
            let id = if after_function && node.flags & AttrNode::GNU_SCOPED != 0 {
                DiagId::sema_attribute_ignored_on_type
            } else {
                DiagId::sema_attribute_not_type_attr
            };
            let msg = self.diag(id, node.loc).arg(format!("'{name}'"));
            self.emit(msg);
            false
        });
    }

    /// Checks attributes against the declaration `d` they appertain to
    /// ([dcl.attr]), dropping (and reporting) those that cannot apply to
    /// it: `alignas` ([`Sema::check_alignas_subject`]), the statement
    /// attributes, and the standard and GNU attributes whose subject is
    /// wrong.
    pub(crate) fn check_decl_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        self.check_alignas_subject(d, attrs);
        let kind = self.ast.decls.get(d).kind();
        attrs.retain(|&a| {
            let node = *self.ast.attrs.get(a);
            self.check_decl_attr(d, kind, node)
        });
    }

    /// One attribute of [`Sema::check_decl_attrs`]; false drops it.
    fn check_decl_attr(&mut self, d: DeclId, kind: DeclKind, node: AttrNode) -> bool {
        let wrong = |s: &mut Self, id: DiagId, name: &str, what: &str| {
            let msg = s.diag(id, node.loc).arg(format!("'{name}'")).arg(what.to_owned());
            s.emit(msg);
            false
        };
        match node.kind {
            // [dcl.attr.assume]/1, [dcl.attr.fallthrough]/1,
            // [dcl.attr.likelihood]/1: statement attributes.
            AttrKind::ASSUME | AttrKind::FALLTHROUGH | AttrKind::LIKELY | AttrKind::UNLIKELY => {
                let name = attr_name(&node).unwrap_or_default();
                let msg = self.diag(DiagId::sema_stmt_attribute_on_decl, node.loc).arg(name);
                self.emit(msg);
                false
            }
            AttrKind::NODISCARD => self.check_nodiscard_subject(d, kind, node),
            AttrKind::NORETURN if node.is_gnu_spelling() => self.check_gnu_noreturn(d, kind, node),
            // [dcl.attr.noreturn]/1.
            AttrKind::NORETURN => {
                kind.is_function()
                    || wrong(self, DiagId::sema_attribute_wrong_decl_type_error, "noreturn", "functions")
            }
            // [dcl.attr.nouniqueaddr]/1.
            AttrKind::NO_UNIQUE_ADDRESS => {
                (kind == DeclKind::Field
                    && self.ast.decls.field(d).is_some_and(|f| f.bit_width.is_none()))
                    || wrong(
                        self,
                        DiagId::sema_attribute_wrong_decl_type_error,
                        "no_unique_address",
                        "non-bit-field non-static data members",
                    )
            }
            // [dcl.attr.depend]/1.
            AttrKind::CARRIES_DEPENDENCY => {
                kind.is_function()
                    || kind == DeclKind::ParmVar
                    || wrong(
                        self,
                        DiagId::sema_attribute_wrong_decl_type_error,
                        "carries_dependency",
                        "parameters, Objective-C methods, and functions",
                    )
            }
            // clang `handlePackedAttr`: a tag or a data member.
            AttrKind::PACKED => {
                if kind == DeclKind::Field || kind == DeclKind::Enum || is_record_kind(kind) {
                    true
                } else {
                    let msg = self.diag(DiagId::parse_attribute_ignored, node.loc).arg("packed");
                    self.emit(msg);
                    false
                }
            }
            AttrKind::MALLOC if !kind.is_function() => {
                wrong(self, DiagId::sema_attribute_wrong_decl_type, "malloc", "functions")
            }
            AttrKind::ALLOC_SIZE | AttrKind::ALLOC_ALIGN if !kind.is_function() => {
                let name = attr_name(&node).unwrap_or_default();
                wrong(self, DiagId::sema_attribute_wrong_decl_type, name, "non-K&R-style functions")
            }
            AttrKind::MALLOC | AttrKind::ALLOC_SIZE | AttrKind::ALLOC_ALIGN => {
                self.check_alloc_attr(d, node);
                true
            }
            AttrKind::FLAG_ENUM | AttrKind::ENUM_EXTENSIBILITY if kind != DeclKind::Enum => {
                let name = attr_name(&node).unwrap_or_default();
                wrong(self, DiagId::sema_attribute_wrong_decl_type, name, "enums")
            }
            _ => true,
        }
    }

    /// The function type `d` has for clang's `Decl::getFunctionType`: its
    /// own (or, for a typedef, its underlying) type when that is a
    /// function type or a pointer or reference to one.
    fn decl_function_type(&self, d: DeclId, kind: DeclKind) -> Option<QualType> {
        let ty = self.decl_value_type(d, kind)?;
        let t = &self.ast.types;
        if t.is_function(ty) {
            return Some(ty);
        }
        if t.is_pointer(ty) || t.is_reference(ty) {
            return t.pointee(ty).filter(|&p| t.is_function(p));
        }
        None
    }

    /// The declared type of a value declaration, or a typedef's
    /// underlying type.
    fn decl_value_type(&self, d: DeclId, kind: DeclKind) -> Option<QualType> {
        if matches!(kind, DeclKind::Typedef | DeclKind::TypeAlias) {
            return self.ast.decls.typedef(d).map(|t| t.underlying);
        }
        if kind.is_function() || is_var_kind(kind) || kind == DeclKind::Field {
            return self.ast.decls.get(d).ty();
        }
        None
    }

    /// `[[nodiscard]]` / GNU `warn_unused_result` ([dcl.attr.nodiscard]/1,
    /// clang `WarnUnusedResult` subjects and `handleWarnUnusedResult`): a
    /// class, enumeration, function, function pointer or typedef; not a
    /// function returning `void` other than a constructor; the standard
    /// spelling not a variable; on a typedef only the GNU syntax.
    fn check_nodiscard_subject(&mut self, d: DeclId, kind: DeclKind, node: AttrNode) -> bool {
        let gnu = node.is_gnu_spelling();
        let name = if gnu { "'warn_unused_result'" } else { "'nodiscard'" };
        let typedef = matches!(kind, DeclKind::Typedef | DeclKind::TypeAlias);
        let fty = self.decl_function_type(d, kind);
        let warn = |s: &mut Self, id: DiagId, what: Option<&str>| {
            let mut msg = s.diag(id, node.loc).arg(name);
            if let Some(w) = what {
                msg = msg.arg(w.to_owned());
            }
            s.emit(msg);
            false
        };
        if !(typedef || kind == DeclKind::Enum || is_record_kind(kind) || fty.is_some()) {
            return warn(
                self,
                DiagId::sema_attribute_wrong_decl_type,
                Some(
                    "Objective-C methods, enums, structs, unions, classes, functions, function \
                     pointers, and typedefs",
                ),
            );
        }
        if let Some(f) = fty
            && kind != DeclKind::CXXConstructor
            && self.ast.types.proto(f).is_some_and(|p| self.ast.types.is_void(p.ret()))
        {
            return warn(self, DiagId::sema_attribute_void_function, None);
        }
        if !gnu && is_var_kind(kind) {
            return warn(
                self,
                DiagId::sema_attribute_wrong_decl_type,
                Some("functions, classes, or enumerations"),
            );
        }
        if typedef && node.syntax != AttrSyntax::Gnu {
            let spelling =
                if gnu { "'[[gnu::warn_unused_result]]'" } else { "'[[nodiscard]]'" };
            let msg = self.diag(DiagId::sema_unused_result_typedef, node.loc).arg(spelling);
            self.emit(msg);
            return false;
        }
        true
    }

    /// GNU `noreturn` (`__attribute__((noreturn))`, `[[gnu::noreturn]]`)
    /// on a declaration: a function type attribute (clang
    /// `warn_type_attribute_wrong_type`): accepted on a function, or a
    /// variable, data member or typedef whose type is a function type
    /// or a pointer, reference or member pointer to one; on a tag or
    /// other declaration `'noreturn' attribute only applies to functions
    /// and methods`.
    fn check_gnu_noreturn(&mut self, d: DeclId, kind: DeclKind, node: AttrNode) -> bool {
        if kind.is_function() {
            return true;
        }
        let Some(ty) = self.decl_value_type(d, kind) else {
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                .arg("'noreturn'")
                .arg("functions and methods");
            self.emit(msg);
            return false;
        };
        let t = &self.ast.types;
        let fnish = t.is_function(ty)
            || t.is_dependent(ty)
            || t.pointee(ty).is_some_and(|p| t.is_function(p) || t.is_dependent(p));
        if fnish {
            return true;
        }
        let ts = format!("'{}'", self.ty_str(ty));
        let msg = self
            .diag(DiagId::sema_type_attribute_wrong_type, node.loc)
            .arg("'noreturn'")
            .arg(ts);
        self.emit(msg);
        false
    }
}
