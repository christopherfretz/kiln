//! Which declarations and positions an attribute may appertain to
//! ([dcl.attr.grammar]/5, [dcl.attr]; spec/sema.md §4.5): the subject
//! table shared by every declarator, alias-declaration, class-head and
//! enum-head (`check_decl_attrs`), and the attributes written where only
//! type attributes may appear (`act_on_type_position_attrs`). Wording
//! follows clang 19 (`diagAppertainsToDecl`, `handleWarnUnusedResult`,
//! `err_attribute_not_type_attr`).

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{DeclKind, EnumData};
use kiln_ast::types::TagKeyword;
use kiln_ast::{AttrId, DeclId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::declspec::DeclSpec;

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
        AttrKind::INDETERMINATE => "indeterminate",
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
        AttrKind::CORO_ONLY_DESTROY_WHEN_COMPLETE => "coro_only_destroy_when_complete",
        AttrKind::OPTNONE => "optnone",
        AttrKind::MINSIZE => "minsize",
        AttrKind::NAKED => "naked",
        AttrKind::MAY_ALIAS => "may_alias",
        AttrKind::NONNULL => "nonnull",
        AttrKind::RETURNS_NONNULL => "returns_nonnull",
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
    /// (`void f() [[noreturn]];`, [dcl.fct]/1) or at the end of an
    /// alias-declaration's type-id. A declaration attribute there is
    /// `'X' attribute cannot be applied to types` (clang
    /// `err_attribute_not_type_attr`), or, written `[[gnu::X]]` in a
    /// declarator chunk (after a ptr-operator, an array or a function
    /// declarator: `in_chunk`), `attribute 'X' ignored, because it
    /// cannot be applied to a type`; it is dropped. GNU-syntax and
    /// vector attributes are kept.
    /// The attribute's name as a diagnostic prints it, `canonical` the
    /// kind's name: as written (no scope) at clang 19; at clang 22, for
    /// a `[[...]]` attribute, `scope::name` with the scope and the `__`
    /// affixes normalized (`[[_Clang::__fallthrough__]]` is
    /// `'clang::fallthrough'`), a GNU-syntax one still as written.
    pub(crate) fn attr_shown(&self, node: &AttrNode, canonical: &str) -> String {
        match self.attr_spellings.get(&node.loc) {
            Some(&(scope, name)) => self.attr_shown_parts(node.syntax, scope, name),
            None => canonical.to_owned(),
        }
    }

    /// [`Sema::attr_shown`] of the attribute-token `scope::name`.
    pub(crate) fn attr_shown_parts(
        &self,
        syntax: AttrSyntax,
        scope: Option<Symbol>,
        name: Symbol,
    ) -> String {
        let raw = self.ast.symbols().as_str(name).to_string();
        if self.clang_compat < 22 || syntax != AttrSyntax::Cxx11 {
            return raw;
        }
        let n = raw.trim_start_matches("__").trim_end_matches("__");
        match scope {
            Some(s) => {
                let s = self.ast.symbols().as_str(s).to_string();
                format!("{}::{n}", crate::decl::norm_attr_scope(&s))
            }
            None => n.to_owned(),
        }
    }

    pub fn act_on_type_position_attrs(&mut self, attrs: &mut Vec<AttrId>, in_chunk: bool) {
        attrs.retain(|&a| {
            let node = *self.ast.attrs.get(a);
            if matches!(node.syntax, AttrSyntax::Gnu | AttrSyntax::Declspec) {
                return true;
            }
            let Some(name) = attr_name(&node) else {
                return true;
            };
            let id = if in_chunk && node.flags & AttrNode::GNU_SCOPED != 0 {
                DiagId::sema_attribute_ignored_on_type
            } else {
                DiagId::sema_attribute_not_type_attr
            };
            let msg = self.diag(id, node.loc).arg(format!("'{}'", self.attr_shown(&node, name)));
            self.emit(msg);
            false
        });
    }

    /// The attributes written before a using-directive or
    /// using-declaration `d` ([namespace.udir]/1, [namespace.udecl]/1),
    /// or of an attribute-declaration (`[[nodiscard]];`, [dcl.pre]/10,
    /// with `d` the enclosing namespace): checked against the subject
    /// table of [`Sema::check_decl_attrs`]; kiln keeps none of them.
    pub fn act_on_decl_attrs(&mut self, d: DeclId, attrs: &[AttrId]) {
        let mut v = attrs.to_vec();
        self.check_decl_attrs(d, &mut v);
    }

    /// The declaration attributes of an attribute-specifier-seq that
    /// appertains to a statement other than a declaration
    /// ([dcl.attr.grammar]/5, [stmt.pre]/1): each is `'X' attribute
    /// cannot be applied to a statement` (clang
    /// `err_decl_attribute_invalid_on_stmt`); before an
    /// asm-declaration (`asm`) clang instead warns `'X' attribute
    /// ignored`. The statement attributes (`assume`, `likely`,
    /// `unlikely`, `fallthrough`) are left to
    /// [`Sema::act_on_attributed_stmt`].
    pub fn act_on_stmt_decl_attrs(&mut self, attrs: &[AttrId], asm: bool) {
        for &a in attrs {
            let node = *self.ast.attrs.get(a);
            if matches!(
                node.kind,
                AttrKind::ASSUME | AttrKind::LIKELY | AttrKind::UNLIKELY | AttrKind::FALLTHROUGH
            ) {
                continue;
            }
            let Some(name) = attr_name(&node) else { continue };
            let id = if asm {
                DiagId::parse_attribute_ignored
            } else {
                DiagId::sema_decl_attribute_on_stmt
            };
            let msg = self.diag(id, node.loc).arg(self.attr_shown(&node, name));
            self.emit(msg);
        }
    }

    /// The attributes before a block-scope using-directive or
    /// using-declaration `s`: at clang 22 each is also "cannot be applied
    /// to a statement" (after the subject check of
    /// [`Sema::act_on_decl_attrs`]); clang 19 reports only the subject.
    pub fn act_on_decl_stmt_attrs(&mut self, s: StmtId, attrs: &[AttrId]) {
        if self.clang_compat < 22 || attrs.is_empty() {
            return;
        }
        let list = self.ast.stmts.decl_stmt_decls(s);
        let using = self.ast.lists.iter::<DeclId>(list).any(|d| {
            matches!(self.ast.decls.get(d).kind(), DeclKind::UsingDirective | DeclKind::Using)
        });
        if using {
            self.act_on_stmt_decl_attrs(attrs, false);
        }
    }

    /// A decl-specifier-seq that only declares the class or enumeration
    /// `tag` (`__attribute__((packed)) struct S { };`): its attributes
    /// would appertain to the (absent) declarators, so clang warns
    /// `attribute 'X' is ignored, place it after "struct" to apply
    /// attribute to type declaration` (`warn_declspec_attribute_ignored`,
    /// [dcl.spec.general]/1, [dcl.pre]/5) for each of `attrs`: the
    /// trailing ones (`struct S { } [[x]];`, reported by the parser) and
    /// then the leading ones (`ds.attrs`, by
    /// [`Sema::act_on_decl_spec_only`]).
    pub fn warn_free_standing_tag_attrs(&mut self, ds: &DeclSpec, tag: DeclId, attrs: &[AttrId]) {
        if attrs.is_empty() {
            return;
        }
        let key = match ds.keyword {
            TagKeyword::Struct => "struct",
            TagKeyword::Class => "class",
            TagKeyword::Union => "union",
            TagKeyword::Enum => match self.ast.decls.enumeration(tag).map(|e| e.flags) {
                Some(f) if f & EnumData::SCOPED_STRUCT != 0 => "enum struct",
                Some(f) if f & EnumData::SCOPED != 0 => "enum class",
                _ => "enum",
            },
            _ => return,
        };
        for &a in attrs {
            let node = *self.ast.attrs.get(a);
            let Some(name) = attr_name(&node) else { continue };
            let msg = self
                .diag(DiagId::sema_declspec_attribute_ignored, node.loc)
                .arg(format!("'{}'", self.attr_shown(&node, name)))
                .arg(key);
            self.emit(msg);
        }
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
            if node.kind == AttrKind::NONNULL {
                return self.check_nonnull_attr(d, kind, a, node);
            }
            if node.kind == AttrKind::INDETERMINATE {
                return self.check_indeterminate_subject(d, kind, node);
            }
            self.check_decl_attr(d, kind, node)
        });
    }

    /// One attribute of [`Sema::check_decl_attrs`]; false drops it.
    fn check_decl_attr(&mut self, d: DeclId, kind: DeclKind, node: AttrNode) -> bool {
        let wrong = |s: &mut Self, id: DiagId, name: &str, what: &str| {
            let shown = s.attr_shown(&node, name);
            let msg = s.diag(id, node.loc).arg(format!("'{shown}'")).arg(what.to_owned());
            s.emit(msg);
            false
        };
        match node.kind {
            // [dcl.attr.assume]/1, [dcl.attr.fallthrough]/1,
            // [dcl.attr.likelihood]/1: statement attributes.
            AttrKind::ASSUME | AttrKind::FALLTHROUGH | AttrKind::LIKELY | AttrKind::UNLIKELY => {
                let name = self.attr_shown(&node, attr_name(&node).unwrap_or_default());
                let msg = self.diag(DiagId::sema_stmt_attribute_on_decl, node.loc).arg(name);
                self.emit(msg);
                false
            }
            AttrKind::NODISCARD => self.check_nodiscard_subject(d, kind, node),
            AttrKind::PREFERRED_NAME => self.check_preferred_name(d, node),
            // clang `handleDeprecatedAttr`: not (yet) on a
            // using-declaration.
            AttrKind::DEPRECATED if kind == DeclKind::Using => {
                let msg = self
                    .diag(DiagId::sema_attribute_no_effect_on_using, node.loc)
                    .arg("'deprecated'");
                self.emit(msg);
                false
            }
            AttrKind::NORETURN if node.is_gnu_spelling() => self.check_gnu_noreturn(d, kind, node),
            // [dcl.attr.noreturn]/1.
            AttrKind::NORETURN => {
                kind.is_function()
                    || wrong(
                        self,
                        DiagId::sema_attribute_wrong_decl_type_error,
                        "noreturn",
                        "functions",
                    )
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
            AttrKind::RETURNS_NONNULL => self.check_returns_nonnull_attr(d, kind, node),
            AttrKind::FLAG_ENUM | AttrKind::ENUM_EXTENSIBILITY if kind != DeclKind::Enum => {
                let name = attr_name(&node).unwrap_or_default();
                wrong(self, DiagId::sema_attribute_wrong_decl_type, name, "enums")
            }
            // clang `NakedAttr` / `OptimizeNoneAttr` subjects (ignored
            // with a warning); `MinSizeAttr`'s is an error.
            AttrKind::NAKED if !kind.is_function() => {
                wrong(self, DiagId::sema_attribute_wrong_decl_type, "naked", "functions")
            }
            AttrKind::OPTNONE if !kind.is_function() => wrong(
                self,
                DiagId::sema_attribute_wrong_decl_type,
                "optnone",
                "functions and Objective-C methods",
            ),
            AttrKind::MINSIZE if !kind.is_function() => wrong(
                self,
                DiagId::sema_attribute_wrong_decl_type_error,
                "minsize",
                "functions and Objective-C methods",
            ),
            AttrKind::CORO_ONLY_DESTROY_WHEN_COMPLETE if !is_record_kind(kind) => wrong(
                self,
                DiagId::sema_attribute_wrong_decl_type,
                "coro_only_destroy_when_complete",
                "classes",
            ),
            _ => true,
        }
    }

    /// `optnone` against `always_inline` and `minsize` on function `d`
    /// (clang `mergeOptimizeNoneAttr`, `mergeAlwaysInlineAttr`,
    /// `mergeMinSizeAttr`): `optnone` wins, on this declaration or a
    /// previous one; the losing attribute is "ignored" with a warning at
    /// it and a note at the `optnone`, and is dropped from `attrs` when
    /// it is this declaration's own. One already ignored on a previous
    /// declaration is not diagnosed again; codegen gives `optnone`
    /// precedence over any redeclaration's (spec/codegen.md).
    pub(crate) fn check_optnone_conflicts(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        let relevant = |k: AttrKind| {
            matches!(k, AttrKind::OPTNONE | AttrKind::ALWAYS_INLINE | AttrKind::MINSIZE)
        };
        if !attrs.iter().any(|&a| relevant(self.ast.attrs.get(a).kind))
            || !self.ast.decls.get(d).kind().is_function()
        {
            return;
        }
        let own = |s: &Self, k: AttrKind| {
            attrs.iter().map(|&a| *s.ast.attrs.get(a)).find(|n| n.kind == k)
        };
        let prior = |s: &Self, k: AttrKind| {
            s.ast.redecls_of(d).filter(|&r| r != d).find_map(|r| {
                s.ast
                    .lists
                    .iter::<AttrId>(s.ast.attrs.of_decl(r))
                    .map(|a| *s.ast.attrs.get(a))
                    .find(|n| n.kind == k)
            })
        };
        let own_optnone = own(self, AttrKind::OPTNONE);
        let prior_optnone = prior(self, AttrKind::OPTNONE);
        let Some(optnone) = own_optnone.or(prior_optnone) else { return };
        let mut losers = Vec::new();
        for (k, name) in
            [(AttrKind::ALWAYS_INLINE, "always_inline"), (AttrKind::MINSIZE, "minsize")]
        {
            if let Some(l) = own(self, k) {
                losers.push((l, name));
            } else if own_optnone.is_some()
                && prior_optnone.is_none()
                && let Some(l) = prior(self, k)
            {
                losers.push((l, name));
            }
        }
        losers.sort_by_key(|(l, _)| l.loc);
        for (l, name) in losers {
            let shown = self.attr_shown(&l, name);
            let note = self.diag(DiagId::note_conflicting_attribute, optnone.loc);
            let msg = self.diag(DiagId::parse_attribute_ignored, l.loc).arg(shown).with_note(note);
            self.emit(msg);
        }
        attrs.retain(|&a| {
            let k = self.ast.attrs.get(a).kind;
            k != AttrKind::ALWAYS_INLINE && k != AttrKind::MINSIZE
        });
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
        let name = format!(
            "'{}'",
            self.attr_shown(&node, if gnu { "warn_unused_result" } else { "nodiscard" })
        );
        let typedef = matches!(kind, DeclKind::Typedef | DeclKind::TypeAlias);
        let fty = self.decl_function_type(d, kind);
        let warn = |s: &mut Self, id: DiagId, what: Option<&str>| {
            let mut msg = s.diag(id, node.loc).arg(name.clone());
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
            let spelling = if gnu { "'[[gnu::warn_unused_result]]'" } else { "'[[nodiscard]]'" };
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
                .arg(format!("'{}'", self.attr_shown(&node, "noreturn")))
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
        let msg =
            // clang 22 names the type attribute unscoped here.
            self.diag(DiagId::sema_type_attribute_wrong_type, node.loc).arg("'noreturn'").arg(ts);
        self.emit(msg);
        false
    }

    /// [dcl.attr.indet]/1: `[[indeterminate]]` appertains to a block
    /// variable with automatic storage duration or a parameter (clang
    /// trunk's `LocalVar, ParmVar` subjects); false drops it.
    fn check_indeterminate_subject(&mut self, d: DeclId, kind: DeclKind, node: AttrNode) -> bool {
        if matches!(kind, DeclKind::Var | DeclKind::ParmVar) && self.has_local_storage(d) {
            return true;
        }
        let msg = self
            .diag(DiagId::sema_attribute_wrong_decl_type_error, node.loc)
            .arg(format!("'{}'", self.attr_shown(&node, "indeterminate")))
            .arg("local variables or function parameters");
        self.emit(msg);
        false
    }

    /// [dcl.attr.indet]/1: `[[indeterminate]]` on a parameter of a
    /// redeclaration of function `prev` must also be on that parameter
    /// in the function's first declaration (clang trunk
    /// `err_indeterminate_attr_not_on_first_decl`).
    pub(crate) fn check_param_indeterminate(
        &mut self,
        prev: DeclId,
        params: &[crate::declspec::ParamInfo],
    ) {
        let first = self.ast.first(prev);
        let Some(fparams) = self.ast.decls.function(first).map(|f| f.params) else { return };
        for (i, p) in params.iter().enumerate() {
            let Some(node) = p
                .attrs
                .iter()
                .map(|&a| *self.ast.attrs.get(a))
                .find(|n| n.kind == AttrKind::INDETERMINATE)
            else {
                continue;
            };
            let Some(op) = self.ast.lists.at::<DeclId>(fparams, i) else { continue };
            let on_first = self
                .ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(op))
                .any(|a| self.ast.attrs.get(a).kind == AttrKind::INDETERMINATE);
            if !on_first {
                let name = p
                    .name
                    .map_or_else(String::new, |(n, _)| self.ast.symbols().as_str(n).to_owned());
                let prev =
                    self.diag(DiagId::note_previous_declaration, self.ast.decls.get(first).loc());
                let msg = self
                    .diag(DiagId::sema_indeterminate_param_not_first_decl, node.loc)
                    .arg(format!("'{name}'"))
                    .with_note(prev);
                self.emit(msg);
            }
        }
    }
}
