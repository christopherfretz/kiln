//! Which declarations and positions an attribute may appertain to
//! ([dcl.attr.grammar]/5, [dcl.attr]; spec/sema.md §4.5): the subject
//! table shared by every declarator, alias-declaration, class-head and
//! enum-head (`check_decl_attrs`), and the attributes written where only
//! type attributes may appear (`act_on_type_position_attrs`). Wording
//! follows clang 19 (`diagAppertainsToDecl`, `handleWarnUnusedResult`,
//! `err_attribute_not_type_attr`).

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{DeclKind, EnumData, TagKind};
use kiln_ast::types::{ExcSpec, NoexceptValue, TagKeyword, TypeView};
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
        AttrKind::UNINITIALIZED => "uninitialized",
        AttrKind::DEPRECATED => "deprecated",
        AttrKind::FALLTHROUGH => "fallthrough",
        AttrKind::LIKELY => "likely",
        AttrKind::UNLIKELY => "unlikely",
        AttrKind::MAYBE_UNUSED => "maybe_unused",
        AttrKind::NORETURN => "noreturn",
        AttrKind::NO_UNIQUE_ADDRESS => "no_unique_address",
        AttrKind::ALWAYS_INLINE => "always_inline",
        AttrKind::GNU_INLINE => "gnu_inline",
        AttrKind::NOINLINE => "noinline",
        AttrKind::PACKED => "packed",
        AttrKind::USED => "used",
        AttrKind::UNUSED => "unused",
        AttrKind::WEAK => "weak",
        AttrKind::WEAKREF => "weakref",
        AttrKind::ALIAS => "alias",
        AttrKind::IFUNC => "ifunc",
        AttrKind::SECTION => "section",
        AttrKind::ANALYZER_NORETURN => "analyzer_noreturn",
        AttrKind::DISABLE_TAIL_CALLS => "disable_tail_calls",
        AttrKind::CONST => "const",
        AttrKind::PURE => "pure",
        AttrKind::COLD => "cold",
        AttrKind::HOT => "hot",
        AttrKind::ABI_TAG => "abi_tag",
        AttrKind::ENABLE_IF => "enable_if",
        AttrKind::DIAGNOSE_IF => "diagnose_if",
        AttrKind::MALLOC => "malloc",
        AttrKind::ALLOC_SIZE => "alloc_size",
        AttrKind::ALLOC_ALIGN => "alloc_align",
        AttrKind::USING_IF_EXISTS => "using_if_exists",
        AttrKind::FLAG_ENUM => "flag_enum",
        AttrKind::ENUM_EXTENSIBILITY => "enum_extensibility",
        AttrKind::CORO_ONLY_DESTROY_WHEN_COMPLETE => "coro_only_destroy_when_complete",
        AttrKind::OPTNONE => "optnone",
        AttrKind::MINSIZE => "minsize",
        AttrKind::VISIBILITY => "visibility",
        AttrKind::TYPE_VISIBILITY => "type_visibility",
        AttrKind::TYPE_NULLABLE => "_Nullable",
        AttrKind::TRIVIAL_ABI => "trivial_abi",
        AttrKind::NAKED => "naked",
        AttrKind::MAY_ALIAS => "may_alias",
        AttrKind::INIT_PRIORITY => "init_priority",
        AttrKind::NONNULL => "nonnull",
        AttrKind::RETURNS_NONNULL => "returns_nonnull",
        AttrKind::EXCLUDE_FROM_EXPLICIT_INSTANTIATION => "exclude_from_explicit_instantiation",
        AttrKind::FORMAT => "format",
        AttrKind::NOTHROW => "nothrow",
        AttrKind::NODEBUG => "nodebug",
        AttrKind::MIN_VECTOR_WIDTH => "min_vector_width",
        _ => return None,
    })
}

/// clang's thread-safety-analysis attributes (`Attr.td`, the
/// `-Wthread-safety` family; spec/sema.md §4.5): known and ignored, with
/// their arguments skipped as balanced tokens by the parser (they may
/// name members declared later). Each entry is the name, whether clang
/// spells it `Clang<"x">` (`__attribute__((x))` and `[[clang::x]]`)
/// rather than only `GNU<"x">`, and the first clang release that knows
/// it (`reentrant_capability` is new in clang 22).
const THREAD_SAFETY_ATTRS: &[(&str, bool, u32)] = &[
    ("acquire_capability", true, 0),
    ("acquire_shared_capability", true, 0),
    ("acquired_after", false, 0),
    ("acquired_before", false, 0),
    ("assert_capability", true, 0),
    ("assert_exclusive_lock", false, 0),
    ("assert_shared_capability", true, 0),
    ("assert_shared_lock", false, 0),
    ("capability", true, 0),
    ("exclusive_lock_function", false, 0),
    ("exclusive_locks_required", true, 0),
    ("exclusive_trylock_function", false, 0),
    ("guarded_by", false, 0),
    ("guarded_var", true, 0),
    ("lock_returned", false, 0),
    ("lockable", false, 0),
    ("locks_excluded", false, 0),
    ("no_thread_safety_analysis", true, 0),
    ("pt_guarded_by", false, 0),
    ("pt_guarded_var", true, 0),
    ("reentrant_capability", true, 22),
    ("release_capability", true, 0),
    ("release_generic_capability", true, 0),
    ("release_shared_capability", true, 0),
    ("requires_capability", true, 0),
    ("requires_shared_capability", true, 0),
    ("scoped_lockable", true, 0),
    ("shared_capability", true, 0),
    ("shared_lock_function", false, 0),
    ("shared_locks_required", true, 0),
    ("shared_trylock_function", false, 0),
    ("try_acquire_capability", true, 0),
    ("try_acquire_shared_capability", true, 0),
    ("unlock_function", true, 0),
];

/// Whether `n` (its `__` affixes removed), written with `syntax` (and
/// the `clang::` scope when `clang_scope`), is a thread-safety attribute
/// clang `compat` knows: such an attribute is accepted silently and
/// attaches nothing.
pub(crate) fn is_thread_safety_attr(
    n: &str,
    syntax: AttrSyntax,
    clang_scope: bool,
    compat: u32,
) -> bool {
    THREAD_SAFETY_ATTRS.binary_search_by(|&(name, ..)| name.cmp(n)).is_ok_and(|i| {
        let (_, clang, since) = THREAD_SAFETY_ATTRS[i];
        compat >= since && (syntax == AttrSyntax::Gnu || (clang && clang_scope))
    })
}

/// clang attributes kiln accepts silently because they only steer
/// sanitizers or `-Wdangling` analysis, which kiln does not implement
/// (spec/sema.md §4.5, WP-667). Each entry is the name, whether clang
/// spells it `[[clang::x]]`, and the first clang release that spells it
/// `[[gnu::x]]` (`u32::MAX`: none). Every entry has the
/// `__attribute__((x))` spelling. `no_sanitize` gained `gnu::` in clang
/// 22 (clang 19 warns that it is unknown).
const IGNORED_CLANG_ATTRS: &[(&str, bool, u32)] = &[
    ("lifetimebound", true, u32::MAX),
    ("no_address_safety_analysis", false, 0),
    ("no_sanitize", true, 22),
    ("no_sanitize_address", false, 0),
    ("no_sanitize_memory", true, u32::MAX),
    ("no_sanitize_thread", false, 0),
];

/// Whether `n` (its `__` affixes removed), written with `syntax` and
/// scope `scope` (normalized; `None` for none), is one of
/// [`IGNORED_CLANG_ATTRS`] in a spelling clang `compat` knows: such an
/// attribute is accepted without a diagnostic and attaches nothing.
pub(crate) fn is_ignored_clang_attr(
    n: &str,
    syntax: AttrSyntax,
    scope: Option<&str>,
    compat: u32,
) -> bool {
    IGNORED_CLANG_ATTRS.binary_search_by(|&(name, ..)| name.cmp(n)).is_ok_and(|i| {
        let (_, clang, gnu_since) = IGNORED_CLANG_ATTRS[i];
        match scope {
            None => syntax == AttrSyntax::Gnu,
            Some("clang") => clang,
            Some("gnu") => compat >= gnu_since,
            Some(_) => false,
        }
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
        // An annotation appertains only to a declaration
        // ([dcl.attr.annotation]/1), never to a type.
        self.reject_annotations(attrs, None);
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
        // None of these declares an entity an annotation can appertain
        // to ([dcl.attr.annotation]/1).
        let what = match self.ast.decls.get(d).kind() {
            DeclKind::UsingDirective => "a using-directive",
            DeclKind::Using | DeclKind::UsingEnum => "a using-declaration",
            _ => "an attribute-declaration",
        };
        self.reject_annotations(&mut v, Some(what));
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
        // An asm-declaration declares nothing an annotation can
        // appertain to; any other statement is not a declaration.
        self.reject_annotations_in(attrs, asm.then_some("an asm-declaration"));
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
            // Its annotations were rejected by `act_on_decl_attrs`.
            let attrs: Vec<AttrId> = attrs
                .iter()
                .copied()
                .filter(|&a| self.ast.attrs.get(a).kind != AttrKind::ANNOTATION)
                .collect();
            self.act_on_stmt_decl_attrs(&attrs, false);
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
        // They would appertain to the absent declarators ([dcl.pre]/5).
        self.reject_annotations_in(attrs, Some("a declaration with no declarator"));
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
        self.take_annotations(d, attrs);
        self.check_alignas_subject(d, attrs);
        self.check_init_priority(d, attrs);
        self.check_visibility_attrs(d, attrs);
        self.check_weak_attrs(d, attrs);
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
        self.unify_fn_section(d, attrs);
    }

    /// One attribute of [`Sema::check_decl_attrs`]; false drops it.
    // One flat match arm per attribute kind's subject rule.
    #[allow(clippy::too_many_lines)]
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
            // clang `Section`: `SubjectList<[Function, GlobalVar,
            // ObjCMethod, ObjCProperty], ErrorDiag>`.
            AttrKind::SECTION => {
                let global_var =
                    matches!(kind, DeclKind::Var | DeclKind::VarTemplateSpecialization)
                        && (!self
                            .ast
                            .decls
                            .get(d)
                            .ctx()
                            .is_some_and(|c| self.ast.decls.get(c).kind().is_function())
                            || self.ast.decls.var(d).is_some_and(|v| {
                                matches!(
                                    v.spec.storage(),
                                    kiln_ast::decl::VarStorage::Static
                                        | kiln_ast::decl::VarStorage::Extern
                                )
                            }));
                kind.is_function()
                    || global_var
                    || wrong(
                        self,
                        DiagId::sema_attribute_wrong_decl_type_error,
                        "section",
                        "functions, global variables, Objective-C methods, and Objective-C properties",
                    )
            }
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
            AttrKind::NORETURN if !kind.is_function() => {
                wrong(self, DiagId::sema_attribute_wrong_decl_type_error, "noreturn", "functions")
            }
            // clang `EnableIf`: `SubjectList<[Function]>`.
            AttrKind::ENABLE_IF if !kind.is_function() => {
                wrong(self, DiagId::sema_attribute_wrong_decl_type, "enable_if", "functions")
            }
            // clang `DiagnoseIf`: `SubjectList<[Function, ObjCMethod,
            // ObjCProperty]>`.
            AttrKind::DIAGNOSE_IF if !kind.is_function() => wrong(
                self,
                DiagId::sema_attribute_wrong_decl_type,
                "diagnose_if",
                "functions, Objective-C methods, and Objective-C properties",
            ),
            AttrKind::ENABLE_IF | AttrKind::DIAGNOSE_IF => self.check_attr_cond_potential(d, node),
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
            AttrKind::FORMAT => self.check_format_decl_attr(d, kind, node),
            AttrKind::NOTHROW if node.flags & 1 == 0 => self.check_gnu_nothrow(d, kind, node),
            AttrKind::FLAG_ENUM | AttrKind::ENUM_EXTENSIBILITY if kind != DeclKind::Enum => {
                let name = attr_name(&node).unwrap_or_default();
                wrong(self, DiagId::sema_attribute_wrong_decl_type, name, "enums")
            }
            // clang `NakedAttr` / `OptimizeNoneAttr` subjects (ignored
            // with a warning); `MinSizeAttr`'s is an error.
            AttrKind::NAKED if !kind.is_function() => {
                wrong(self, DiagId::sema_attribute_wrong_decl_type, "naked", "functions")
            }
            AttrKind::MIN_VECTOR_WIDTH if !kind.is_function() => {
                wrong(self, DiagId::sema_attribute_wrong_decl_type, "min_vector_width", "functions")
            }
            AttrKind::OPTNONE if !kind.is_function() => wrong(
                self,
                DiagId::sema_attribute_wrong_decl_type,
                "optnone",
                "functions and Objective-C methods",
            ),
            AttrKind::DISABLE_TAIL_CALLS if !kind.is_function() => wrong(
                self,
                DiagId::sema_attribute_wrong_decl_type,
                "disable_tail_calls",
                "functions and Objective-C methods",
            ),
            AttrKind::MINSIZE if !kind.is_function() => wrong(
                self,
                DiagId::sema_attribute_wrong_decl_type_error,
                "minsize",
                "functions and Objective-C methods",
            ),
            AttrKind::UNINITIALIZED => self.check_uninitialized_subject(d, kind, node),
            // clang `handleAnalyzerNoReturnAttr`: a function, or a
            // declaration of function or block pointer type (ignored with
            // a warning otherwise).
            AttrKind::ANALYZER_NORETURN
                if !kind.is_function()
                    && !((is_var_kind(kind) || kind == DeclKind::Field)
                        && self.ast.decls.get(d).ty().is_some_and(|t| {
                            self.ast.types.is_pointer(t)
                                && self
                                    .ast
                                    .types
                                    .pointee(t)
                                    .is_some_and(|p| self.ast.types.is_function(p))
                        })) =>
            {
                wrong(
                    self,
                    DiagId::sema_attribute_wrong_decl_type,
                    "analyzer_noreturn",
                    "functions, methods and blocks",
                )
            }
            // clang `GNUInline`: `SubjectList<[Function]>` (ignored with a
            // warning).
            AttrKind::GNU_INLINE if !kind.is_function() => {
                wrong(self, DiagId::sema_attribute_wrong_decl_type, "gnu_inline", "functions")
            }
            AttrKind::GNU_INLINE => self.check_gnu_inline_attr(d, node),
            // clang `TrivialABI`: `SubjectList<[CXXRecord]>` (ignored
            // with a warning).
            AttrKind::TRIVIAL_ABI if !is_record_kind(kind) => {
                wrong(self, DiagId::sema_attribute_wrong_decl_type, "trivial_abi", "classes")
            }
            // clang `TypeNullable`: `SubjectList<[Struct]>`, a non-union
            // class.
            AttrKind::TYPE_NULLABLE
                if !is_record_kind(kind)
                    || self.ast.decls.record(d).is_some_and(|r| r.tag == TagKind::Union) =>
            {
                wrong(self, DiagId::sema_attribute_wrong_decl_type_error, "_Nullable", "classes")
            }
            AttrKind::CORO_ONLY_DESTROY_WHEN_COMPLETE if !is_record_kind(kind) => wrong(
                self,
                DiagId::sema_attribute_wrong_decl_type,
                "coro_only_destroy_when_complete",
                "classes",
            ),
            _ => true,
        }
    }

    /// Whether `d`'s semantic context is a namespace or the translation
    /// unit, through linkage specifications and export declarations
    /// (clang `getRedeclContext()->isFileContext()`).
    fn in_file_context(&self, d: DeclId) -> bool {
        let mut c = self.ast.decls.get(d).ctx();
        while let Some(ctx) = c {
            match self.ast.decls.get(ctx).kind() {
                DeclKind::TranslationUnit | DeclKind::Namespace => return true,
                DeclKind::LinkageSpec | DeclKind::Export => c = self.ast.decls.get(ctx).ctx(),
                _ => return false,
            }
        }
        true
    }

    /// GNU `weak`, `weakref`, `alias` and `ifunc` on `d` (clang
    /// `handleWeakRefAttr`, `handleAliasAttr`, `handleIFuncAttr`, the end
    /// of `ProcessDeclAttributeList` and `checkAttributesAfterMerging`):
    /// - `weakref` appertains to a variable or function (an error
    ///   otherwise) declared in a namespace (`must be in a global
    ///   context`); `weakref("target")` becomes an `alias("target")`
    ///   followed by a bare `weakref`, as in clang;
    /// - `alias` appertains to a function or a variable with static
    ///   storage duration and not to a definition: a function that
    ///   already has an alias (`weakref("x"), alias("y")`) or an
    ///   externally visible variable definition is `definition 'x' cannot
    ///   also be an alias` (a function body or a variable initializer is
    ///   reported when it is seen, [`Sema::check_alias_definition`]);
    /// - `ifunc` appertains to a function (`'ifunc' attribute only applies
    ///   to functions`, a `-Wignored-attributes` warning, otherwise); an
    ///   `alias` and an `ifunc` each make the declaration a definition, so
    ///   whichever of the two comes second is `definition 'f' cannot also
    ///   be an alias/ifunc`, and either on a redeclaration of a defined
    ///   entity is `redefinition of 'f'` (clang `hasDefiningAttr`);
    /// - a `weakref` without a target is `must also have an alias
    ///   attribute`, at the first attribute, and dropped;
    /// - a `weakref` on a declaration with external linkage is `weakref
    ///   declaration must have internal linkage` (dropped with its
    ///   alias), a `weak` one on a declaration with internal linkage
    ///   `weak declaration cannot have internal linkage` ([basic.link]).
    #[allow(clippy::too_many_lines, reason = "one step per weak/weakref/alias rule above")]
    pub(crate) fn check_weak_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        let has = |s: &Self, attrs: &[AttrId], k: AttrKind| {
            attrs.iter().any(|&a| s.ast.attrs.get(a).kind == k)
        };
        if !has(self, attrs, AttrKind::WEAKREF)
            && !has(self, attrs, AttrKind::ALIAS)
            && !has(self, attrs, AttrKind::IFUNC)
            && !has(self, attrs, AttrKind::WEAK)
        {
            return;
        }
        let kind = self.ast.decls.get(d).kind();
        let quoted = format!("'{}'", self.decl_name(d));
        let first_loc = attrs
            .iter()
            .map(|&a| *self.ast.attrs.get(a))
            .find(|n| n.kind != AttrKind::ASM_LABEL)
            .map(|n| n.loc);
        let mut out = Vec::with_capacity(attrs.len() + 1);
        let mut has_alias = false;
        let mut has_ifunc = false;
        for &a in attrs.iter() {
            let node = *self.ast.attrs.get(a);
            match node.kind {
                AttrKind::WEAKREF => {
                    // clang `WeakRef`: `SubjectList<[Var, Function], ErrorDiag>`.
                    if !kind.is_function() && kind != DeclKind::Var {
                        let shown = self.attr_shown(&node, "weakref");
                        let msg = self
                            .diag(DiagId::sema_attribute_wrong_decl_type_error, node.loc)
                            .arg(format!("'{shown}'"))
                            .arg("variables and functions");
                        self.emit(msg);
                        continue;
                    }
                    if !self.in_file_context(d) {
                        let msg = self
                            .diag(DiagId::sema_attribute_weakref_not_global_context, node.loc)
                            .arg(quoted.clone());
                        self.emit(msg);
                        continue;
                    }
                    if node.a != 0 {
                        let alias = AttrNode { kind: AttrKind::ALIAS, ..node };
                        out.push(self.ast.attrs.push(alias));
                        has_alias = true;
                        out.push(self.ast.attrs.push(AttrNode { a: 0, ..node }));
                    } else {
                        out.push(a);
                    }
                }
                AttrKind::ALIAS => {
                    let global_var = kind == DeclKind::Var
                        && (self.in_file_context(d)
                            || self.ast.decls.var(d).is_some_and(|v| {
                                matches!(
                                    v.spec.storage(),
                                    kiln_ast::decl::VarStorage::Static
                                        | kiln_ast::decl::VarStorage::Extern
                                )
                            }));
                    // clang `Alias`: `SubjectList<[Function, GlobalVar], ErrorDiag>`.
                    if !kind.is_function() && !global_var {
                        let shown = self.attr_shown(&node, "alias");
                        let msg = self
                            .diag(DiagId::sema_attribute_wrong_decl_type_error, node.loc)
                            .arg(format!("'{shown}'"))
                            .arg("functions and global variables");
                        self.emit(msg);
                        continue;
                    }
                    let definition = has_alias
                        || has_ifunc
                        || (!kind.is_function()
                            && self.is_var_definition(d)
                            && !kiln_abi::mangle::has_internal_linkage(&self.ast, d));
                    if definition {
                        let msg = self
                            .diag(DiagId::sema_alias_is_definition, node.loc)
                            .arg(quoted.clone())
                            .arg(0);
                        self.emit(msg);
                        continue;
                    }
                    has_alias = true;
                    out.push(a);
                }
                AttrKind::IFUNC => {
                    // clang `IFunc`: `SubjectList<[Function]>`, a warning.
                    if !kind.is_function() {
                        let shown = self.attr_shown(&node, "ifunc");
                        let msg = self
                            .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                            .arg(format!("'{shown}'"))
                            .arg("functions");
                        self.emit(msg);
                        continue;
                    }
                    if has_alias || has_ifunc {
                        let msg = self
                            .diag(DiagId::sema_alias_is_definition, node.loc)
                            .arg(quoted.clone())
                            .arg(1);
                        self.emit(msg);
                        continue;
                    }
                    has_ifunc = true;
                    out.push(a);
                }
                _ => out.push(a),
            }
        }
        if !has_alias && has(self, &out, AttrKind::WEAKREF) {
            let at = first_loc.unwrap_or_else(|| self.ast.decls.get(d).loc());
            let msg = self.diag(DiagId::sema_attribute_weakref_without_alias, at).arg(quoted);
            self.emit(msg);
            out.retain(|&a| self.ast.attrs.get(a).kind != AttrKind::WEAKREF);
        }
        let internal = kiln_abi::mangle::has_internal_linkage(&self.ast, d);
        if let Some(w) = out.iter().copied().find(|&a| self.ast.attrs.get(a).kind == AttrKind::WEAK)
            && internal
        {
            let msg = self.diag(DiagId::sema_attribute_weak_static, self.ast.attrs.get(w).loc);
            self.emit(msg);
            out.retain(|&a| a != w);
        }
        if let Some(w) =
            out.iter().copied().find(|&a| self.ast.attrs.get(a).kind == AttrKind::WEAKREF)
            && !internal
        {
            let msg =
                self.diag(DiagId::sema_attribute_weakref_not_static, self.ast.attrs.get(w).loc);
            self.emit(msg);
            out.retain(|&a| {
                !matches!(self.ast.attrs.get(a).kind, AttrKind::WEAKREF | AttrKind::ALIAS)
            });
        }
        if has(self, &out, AttrKind::ALIAS) || has(self, &out, AttrKind::IFUNC) {
            self.check_alias_redefines(d, kind);
        }
        *attrs = out;
    }

    /// An `alias` / `ifunc` declaration `d` whose entity an earlier
    /// declaration already defines (a function body, a variable
    /// definition, or another `alias` / `ifunc`): `redefinition of 'x'`
    /// at `d`, noting the definition (clang `checkNewAttributesAfterDef`
    /// via `hasDefiningAttr`).
    fn check_alias_redefines(&mut self, d: DeclId, kind: DeclKind) {
        let prev = self.ast.redecls_of(d).find(|&p| {
            p != d
                && (self.own_alias_attr(p).is_some()
                    || if kind.is_function() {
                        self.ast.decls.function(p).is_some_and(|f| f.body.is_some())
                    } else {
                        self.is_var_definition(p)
                    })
        });
        let Some(prev) = prev else { return };
        let msg = self
            .diag(DiagId::sema_redefinition, self.ast.decls.get(d).loc())
            .arg(self.decl_name(d));
        let msg = self.note_prev(msg, prev, true);
        self.emit(msg);
    }

    /// The `alias` / `ifunc` attribute written on `d` itself.
    pub(crate) fn own_alias_attr(&self, d: DeclId) -> Option<AttrNode> {
        self.ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(d))
            .map(|a| *self.ast.attrs.get(a))
            .find(|n| matches!(n.kind, AttrKind::ALIAS | AttrKind::IFUNC))
    }

    /// The definition of `d` (a function body or a variable initializer)
    /// where `d` has an `alias` or `ifunc` attribute: `definition 'x'
    /// cannot also be an alias/ifunc` at the attribute (clang
    /// `ActOnStartOfFunctionDef`, `checkAttributesAfterMerging`); where an
    /// earlier declaration has one, `redefinition of
    /// 'x'` noting it (clang `hasDefiningAttr`; a `weakref`'s `alias`
    /// counts). Returns whether a redefinition was reported.
    pub(crate) fn check_alias_definition(&mut self, d: DeclId) -> bool {
        if let Some(node) = self.own_alias_attr(d) {
            let msg = self
                .diag(DiagId::sema_alias_is_definition, node.loc)
                .arg(format!("'{}'", self.decl_name(d)))
                .arg(u32::from(node.kind == AttrKind::IFUNC));
            self.emit(msg);
            return false;
        }
        let Some(prev) =
            self.ast.redecls_of(d).find(|&p| p != d && self.own_alias_attr(p).is_some())
        else {
            return false;
        };
        let msg = self
            .diag(DiagId::sema_redefinition, self.ast.decls.get(d).loc())
            .arg(self.decl_name(d));
        let msg = self.note_prev(msg, prev, true);
        self.emit(msg);
        true
    }

    /// GNU `gnu_inline` on function `d` (clang `handleGNUInlineAttr`):
    /// ignored with a warning unless `d` is `inline`-specified (the
    /// keyword, not implicit inlineness); in C++ one without `extern`
    /// warns that it is externally available all the same (codegen gives
    /// both the `available_externally` treatment, spec/codegen.md "GNU
    /// inline functions").
    fn check_gnu_inline_attr(&mut self, d: DeclId, node: AttrNode) -> bool {
        let Some(spec) = self.ast.decls.function(d).map(|f| f.spec) else { return true };
        if !spec.is_inline() {
            let msg = self.diag(DiagId::sema_gnu_inline_requires_inline, node.loc);
            self.emit(msg);
            return false;
        }
        if spec.storage() != kiln_ast::decl::FnStorage::Extern {
            let msg = self.diag(DiagId::sema_gnu_inline_cpp_without_extern, node.loc);
            self.emit(msg);
        }
        true
    }

    /// `[[clang::uninitialized]]` (clang `UninitializedAttr`,
    /// `SubjectList<[LocalVar]>`): only on a variable with automatic
    /// storage; elsewhere ignored with a warning.
    fn check_uninitialized_subject(&mut self, d: DeclId, kind: DeclKind, node: AttrNode) -> bool {
        if kind == DeclKind::Var && self.has_local_storage(d) {
            return true;
        }
        let shown = self.attr_shown(&node, "uninitialized");
        let msg = self
            .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
            .arg(format!("'{shown}'"))
            .arg("local variables".to_owned());
        self.emit(msg);
        false
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

    /// `naked` against `disable_tail_calls` on function `d` (clang
    /// `MutualExclusions<[Naked, DisableTailCalls]>`, WP-682): the later
    /// of the two, on this declaration against an earlier attribute of it
    /// or of a previous declaration, is an error ("'later' and 'earlier'
    /// attributes are not compatible", a note at the earlier) and is
    /// dropped from `attrs`.
    pub(crate) fn check_naked_conflicts(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        const PAIR: [(AttrKind, &str); 2] =
            [(AttrKind::NAKED, "naked"), (AttrKind::DISABLE_TAIL_CALLS, "disable_tail_calls")];
        let own: Vec<AttrNode> = attrs
            .iter()
            .map(|&a| *self.ast.attrs.get(a))
            .filter(|n| PAIR.iter().any(|&(k, _)| k == n.kind))
            .collect();
        if own.is_empty() || !self.ast.decls.get(d).kind().is_function() {
            return;
        }
        let prior = |s: &Self, k: AttrKind| {
            s.ast.redecls_of(d).filter(|&r| r != d).find_map(|r| {
                s.ast
                    .lists
                    .iter::<AttrId>(s.ast.attrs.of_decl(r))
                    .map(|a| *s.ast.attrs.get(a))
                    .find(|n| n.kind == k)
            })
        };
        let name = |k: AttrKind| PAIR.iter().find(|&&(x, _)| x == k).map_or("", |&(_, n)| n);
        let mut dropped = Vec::new();
        for (i, n) in own.iter().enumerate() {
            let other = if n.kind == AttrKind::NAKED {
                AttrKind::DISABLE_TAIL_CALLS
            } else {
                AttrKind::NAKED
            };
            let earlier =
                prior(self, other).or_else(|| own[..i].iter().copied().find(|e| e.kind == other));
            let Some(e) = earlier else { continue };
            let note = self.diag(DiagId::note_conflicting_attribute, e.loc);
            let msg = self
                .diag(DiagId::sema_attributes_not_compatible, n.loc)
                .arg(self.attr_shown(n, name(n.kind)))
                .arg(self.attr_shown(&e, name(e.kind)))
                .with_note(note);
            self.emit(msg);
            dropped.push(n.loc);
        }
        attrs.retain(|&a| {
            let n = self.ast.attrs.get(a);
            !(dropped.contains(&n.loc) && PAIR.iter().any(|&(k, _)| k == n.kind))
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

    /// GNU `nothrow` written on a declarator (WP-186; clang
    /// `handleFunctionTypeAttr`, `AT_NoThrow`): a function type attribute,
    /// applied to the declared type's function type (directly or through
    /// one pointer, reference or member pointer) and removed from
    /// `attrs`. With no exception specification the function type
    /// becomes `__attribute__((nothrow))` ([`ExcSpec::NothrowAttr`],
    /// non-throwing); a potentially-throwing written one (`throw(X)`,
    /// `noexcept(false)`) warns `'nothrow' attribute conflicts with
    /// exception specification; attribute ignored`; a non-throwing or
    /// unresolved one is kept as is. On any other type the attribute
    /// stays for [`Sema::check_decl_attrs`] to reject.
    pub(crate) fn apply_nothrow_attrs(
        &mut self,
        ty: QualType,
        attrs: &mut Vec<AttrId>,
    ) -> QualType {
        let mut ty = ty;
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if node.kind != AttrKind::NOTHROW || node.flags & 1 != 0 {
                i += 1;
                continue;
            }
            match self.with_nothrow(ty, node.loc, true) {
                Some(t) => {
                    attrs.remove(i);
                    ty = t;
                }
                None => i += 1,
            }
        }
        ty
    }

    /// `ty` (a function type, or a pointer, reference or member pointer
    /// to one when `outer`) with GNU `nothrow` applied; None when it is
    /// no such type.
    fn with_nothrow(
        &mut self,
        ty: QualType,
        loc: kiln_source::Loc,
        outer: bool,
    ) -> Option<QualType> {
        if self.is_error_type(ty) {
            return None;
        }
        if self.ast.types.is_function(ty) {
            return Some(match self.written_exc_spec(ty) {
                ExcSpec::None => self.with_exc_spec(ty, ExcSpec::NothrowAttr),
                ExcSpec::Dynamic(_) | ExcSpec::NoexceptExpr { value: NoexceptValue::False, .. } => {
                    self.error(DiagId::sema_nothrow_conflicts_exc_spec, loc);
                    ty
                }
                _ => ty,
            });
        }
        if !outer {
            return None;
        }
        let built = match self.ast.types.canonical_view(ty) {
            TypeView::Pointer(p) => {
                let f = self.with_nothrow(p, loc, false)?;
                self.ast.types.pointer_to(f)
            }
            TypeView::LValueRef(p) => {
                let f = self.with_nothrow(p, loc, false)?;
                self.ast.types.lvalue_ref_to(f)
            }
            TypeView::RValueRef(p) => {
                let f = self.with_nothrow(p, loc, false)?;
                self.ast.types.rvalue_ref_to(f)
            }
            TypeView::MemberPointer { pointee, class } => {
                let f = self.with_nothrow(pointee, loc, false)?;
                self.ast.types.member_pointer(f, class)
            }
            _ => return None,
        };
        Some(self.ast.types.qualify(built, ty.quals()))
    }

    /// A type-id's declarator attributes (`using F = void
    /// (*__attribute__((noreturn)))(int);`): GNU `noreturn` among them
    /// makes the function type `noreturn` as on a declaration
    /// ([`Sema::apply_noreturn_attrs`]; spec/types.md §5.4, WP-708).
    pub fn act_on_type_id_decl_attrs(&mut self, ty: QualType, attrs: &[AttrId]) -> QualType {
        self.apply_noreturn_attrs(ty, attrs)
    }

    /// GNU `noreturn` (`__attribute__((noreturn))`, `[[gnu::noreturn]]`)
    /// written on a declaration (clang `handleFunctionTypeAttr`,
    /// `AT_NoReturn`): a function type attribute, so the declared type's
    /// function type (directly or through one pointer, reference or
    /// member pointer) becomes `noreturn` (spec/types.md §5.4). The
    /// attribute stays in `attrs`: [`Sema::check_decl_attrs`] validates
    /// it and a function keeps it ([`kiln_ast::Ast::is_noreturn_fn`]).
    /// `[[noreturn]]` and `_Noreturn` appertain to the function and do
    /// not change its type.
    pub(crate) fn apply_noreturn_attrs(&mut self, ty: QualType, attrs: &[AttrId]) -> QualType {
        let gnu = attrs.iter().any(|&a| {
            let n = self.ast.attrs.get(a);
            n.kind == AttrKind::NORETURN && n.is_gnu_spelling()
        });
        if !gnu || self.is_error_type(ty) {
            return ty;
        }
        self.with_fn_noreturn(ty, true, true).unwrap_or(ty)
    }

    /// `ty` (a function type, or a pointer, reference or member pointer
    /// to one when `outer`) with its function type's `noreturn` bit set
    /// to `v`; None when it is no such type or already has it.
    pub(crate) fn with_fn_noreturn(
        &mut self,
        ty: QualType,
        v: bool,
        outer: bool,
    ) -> Option<QualType> {
        // Sugar (a typedef, `decltype`, a substituted parameter) is
        // stepped through, not canonicalized (clang's
        // `FunctionTypeUnwrapper`): the function type as written keeps
        // every other bit, its exception specification included, so
        // `noreturn` is a pure addition (WP-748).
        let ty = self.ast.types.desugar_top(ty);
        if let TypeView::Function(p) = self.ast.types.view(ty.ty()) {
            if p.info().noreturn() == v {
                return None;
            }
            let params = self.ast.types.params(&p).to_vec();
            let exc = self.ast.types.exc_spec(&p);
            let f = self.ast.types.function(p.ret(), &params, p.info().with_noreturn(v), exc);
            return Some(self.ast.types.qualify(f, ty.quals()));
        }
        if self.ast.types.is_function(ty) {
            // Sugar `desugar_top` keeps (a dependent kind).
            let c = self.ast.types.canonical(ty);
            return self.with_fn_noreturn(c, v, false);
        }
        if !outer {
            return None;
        }
        let view = match self.ast.types.view(ty.ty()) {
            w @ (TypeView::Pointer(_)
            | TypeView::LValueRef(_)
            | TypeView::RValueRef(_)
            | TypeView::MemberPointer { .. }) => w,
            _ => self.ast.types.canonical_view(ty),
        };
        let built = match view {
            TypeView::Pointer(p) => {
                let f = self.with_fn_noreturn(p, v, false)?;
                self.ast.types.pointer_to(f)
            }
            TypeView::LValueRef(p) => {
                let f = self.with_fn_noreturn(p, v, false)?;
                self.ast.types.lvalue_ref_to(f)
            }
            TypeView::RValueRef(p) => {
                let f = self.with_fn_noreturn(p, v, false)?;
                self.ast.types.rvalue_ref_to(f)
            }
            TypeView::MemberPointer { pointee, class } => {
                let f = self.with_fn_noreturn(pointee, v, false)?;
                self.ast.types.member_pointer(f, class)
            }
            _ => return None,
        };
        Some(self.ast.types.qualify(built, ty.quals()))
    }

    /// A written GNU `nothrow` [`Sema::apply_nothrow_attrs`] did not
    /// apply: kept on a function (a declarator path that does not apply
    /// type attributes; [`kiln_ast::Ast`]'s `NOTHROW` check still makes
    /// it non-throwing), else clang's two warnings, `'nothrow' only
    /// applies to function types; type here is 'T'` (for a declaration
    /// with a type) and `'nothrow' attribute only applies to functions
    /// and function pointers`, and dropped.
    fn check_gnu_nothrow(&mut self, d: DeclId, kind: DeclKind, node: AttrNode) -> bool {
        if kind.is_function() {
            return true;
        }
        if let Some(ty) = self.decl_value_type(d, kind)
            && !self.is_error_type(ty)
        {
            let ts = format!("'{}'", self.ty_str(ty));
            let msg = self
                .diag(DiagId::sema_type_attribute_wrong_type, node.loc)
                .arg("'nothrow'")
                .arg(ts);
            self.emit(msg);
        }
        let msg = self
            .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
            .arg(format!("'{}'", self.attr_shown(&node, "nothrow")))
            .arg("functions and function pointers");
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is searched by name, so it must stay sorted.
    #[test]
    fn thread_safety_table_sorted() {
        assert!(THREAD_SAFETY_ATTRS.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(THREAD_SAFETY_ATTRS.len(), 34);
        assert!(is_thread_safety_attr("guarded_by", AttrSyntax::Gnu, false, 19));
        assert!(!is_thread_safety_attr("guarded_by", AttrSyntax::Cxx11, true, 22));
        assert!(is_thread_safety_attr("capability", AttrSyntax::Cxx11, true, 19));
        assert!(!is_thread_safety_attr("capability", AttrSyntax::Cxx11, false, 22));
        assert!(!is_thread_safety_attr("reentrant_capability", AttrSyntax::Gnu, false, 19));
        assert!(is_thread_safety_attr("reentrant_capability", AttrSyntax::Gnu, false, 22));
    }

    #[test]
    fn ignored_clang_attrs_sorted_and_spelled() {
        // [dcl.attr.grammar]/5: an unrecognized attribute-token is
        // implementation-defined; clang's spellings are the contract.
        assert!(IGNORED_CLANG_ATTRS.windows(2).all(|w| w[0].0 < w[1].0));
        let ig = is_ignored_clang_attr;
        assert!(ig("no_sanitize", AttrSyntax::Gnu, None, 19));
        assert!(ig("no_sanitize", AttrSyntax::Cxx11, Some("clang"), 19));
        assert!(!ig("no_sanitize", AttrSyntax::Cxx11, Some("gnu"), 19));
        assert!(ig("no_sanitize", AttrSyntax::Cxx11, Some("gnu"), 22));
        assert!(!ig("lifetimebound", AttrSyntax::Cxx11, None, 22));
        assert!(!ig("lifetimebound", AttrSyntax::Cxx11, Some("gnu"), 22));
        assert!(ig("no_sanitize_address", AttrSyntax::Cxx11, Some("gnu"), 19));
        assert!(!ig("no_sanitize_address", AttrSyntax::Cxx11, Some("clang"), 22));
    }
}
