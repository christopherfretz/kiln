//! `#pragma clang attribute` (spec/sema.md §4.5 "`#pragma clang
//! attribute`"; clang `ActOnPragmaAttributeAttribute`,
//! `ActOnPragmaAttributeEmptyPush`, `ActOnPragmaAttributePop`,
//! `AddPragmaAttributes`, `DiagnoseUnterminatedPragmaAttribute`).
//!
//! A `push` opens a group (optionally in a namespace, `NS.push`); each
//! attribute of a `push (attr, apply_to = rules)` or `(attr, apply_to =
//! rules)` joins the innermost group as an entry with its subject-match
//! rules, checked against the attribute's own rule set (clang's
//! `getMatchRules`, [`attr_rules`]). Every later declaration a rule
//! selects receives a copy of the entry's attribute node before its
//! attributes are checked ([`Sema::add_pragma_attrs`]), until the group is
//! popped; an entry no declaration used is reported at the pop.

use kiln_ast::QualType;
use kiln_ast::attr::{AttrNode, normalize_attr_name};
use kiln_ast::decl::{DeclKind, TagKind, TlsKind};
use kiln_ast::ids::{AttrId, DeclId};
use kiln_diag::DiagId;
use kiln_source::symbol::Symbol;
use kiln_source::{Loc, Span};

use crate::Sema;

/// A subject-match rule (clang `attr::SubjectMatchRule`, in its order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, kiln_snap::Snap)]
pub enum MatchRule {
    /// `block` (never matches: kiln has no blocks).
    Block,
    /// `enum`.
    Enum,
    /// `enum_constant`.
    EnumConstant,
    /// `field`.
    Field,
    /// `function`.
    Function,
    /// `function(is_member)`.
    FunctionIsMember,
    /// `namespace`.
    Namespace,
    /// `objc_category` (never matches).
    ObjcCategory,
    /// `objc_implementation` (never matches).
    ObjcImplementation,
    /// `objc_interface` (never matches).
    ObjcInterface,
    /// `objc_method` (never matches).
    ObjcMethod,
    /// `objc_method(is_instance)` (never matches).
    ObjcMethodIsInstance,
    /// `objc_property` (never matches).
    ObjcProperty,
    /// `objc_protocol` (never matches).
    ObjcProtocol,
    /// `record`.
    Record,
    /// `record(unless(is_union))`.
    RecordNotIsUnion,
    /// `hasType` (abstract: only through a sub-rule).
    HasType,
    /// `hasType(functionType)`.
    HasTypeFunctionType,
    /// `type_alias`.
    TypeAlias,
    /// `variable`.
    Variable,
    /// `variable(is_thread_local)`.
    VariableIsThreadLocal,
    /// `variable(is_global)`.
    VariableIsGlobal,
    /// `variable(is_local)`.
    VariableIsLocal,
    /// `variable(is_parameter)`.
    VariableIsParameter,
    /// `variable(unless(is_parameter))`.
    VariableNotIsParameter,
}

impl MatchRule {
    /// The primary rule spelled `name` (clang `isAttributeSubjectMatchRule`).
    pub fn primary(name: &str) -> Option<MatchRule> {
        use MatchRule as R;
        Some(match name {
            "block" => R::Block,
            "enum" => R::Enum,
            "enum_constant" => R::EnumConstant,
            "field" => R::Field,
            "function" => R::Function,
            "namespace" => R::Namespace,
            "objc_category" => R::ObjcCategory,
            "objc_implementation" => R::ObjcImplementation,
            "objc_interface" => R::ObjcInterface,
            "objc_method" => R::ObjcMethod,
            "objc_property" => R::ObjcProperty,
            "objc_protocol" => R::ObjcProtocol,
            "record" => R::Record,
            "hasType" => R::HasType,
            "type_alias" => R::TypeAlias,
            "variable" => R::Variable,
            _ => return None,
        })
    }

    /// Sub-rule `name` (inside `unless(...)` if `unless`) of this primary
    /// rule (clang `isAttributeSubjectMatchSubRuleFor_*`).
    pub fn sub_rule(self, name: &str, unless: bool) -> Option<MatchRule> {
        use MatchRule as R;
        Some(match (self, name, unless) {
            (R::Function, "is_member", false) => R::FunctionIsMember,
            (R::ObjcMethod, "is_instance", false) => R::ObjcMethodIsInstance,
            (R::Record, "is_union", true) => R::RecordNotIsUnion,
            (R::HasType, "functionType", false) => R::HasTypeFunctionType,
            (R::Variable, "is_thread_local", false) => R::VariableIsThreadLocal,
            (R::Variable, "is_global", false) => R::VariableIsGlobal,
            (R::Variable, "is_local", false) => R::VariableIsLocal,
            (R::Variable, "is_parameter", false) => R::VariableIsParameter,
            (R::Variable, "is_parameter", true) => R::VariableNotIsParameter,
            _ => return None,
        })
    }

    /// Whether the rule is abstract (`hasType`: written with a sub-rule).
    pub fn is_abstract(self) -> bool {
        self == MatchRule::HasType
    }

    /// The sub-rules a primary rule supports, as clang lists them
    /// (`validAttributeSubjectMatchSubRules`).
    pub fn sub_rules(self) -> Option<&'static str> {
        use MatchRule as R;
        Some(match self {
            R::Function => "'is_member'",
            R::ObjcMethod => "'is_instance'",
            R::Record => "'unless(is_union)'",
            R::HasType => "'functionType'",
            R::Variable => {
                "'is_thread_local', 'is_global', 'is_local', 'is_parameter', 'unless(is_parameter)'"
            }
            _ => return None,
        })
    }

    /// The primary rule of a sub-rule (clang `getParentAttrMatcherRule`).
    pub fn parent(self) -> Option<MatchRule> {
        use MatchRule as R;
        Some(match self {
            R::FunctionIsMember => R::Function,
            R::ObjcMethodIsInstance => R::ObjcMethod,
            R::RecordNotIsUnion => R::Record,
            R::HasTypeFunctionType => R::HasType,
            R::VariableIsThreadLocal
            | R::VariableIsGlobal
            | R::VariableIsLocal
            | R::VariableIsParameter
            | R::VariableNotIsParameter => R::Variable,
            _ => return None,
        })
    }

    /// Whether the rule is an `unless(...)` sub-rule.
    pub fn is_negated(self) -> bool {
        matches!(self, MatchRule::RecordNotIsUnion | MatchRule::VariableNotIsParameter)
    }

    /// The rule's spelling (clang `getSubjectMatchRuleSpelling`).
    pub fn spelling(self) -> &'static str {
        use MatchRule as R;
        match self {
            R::Block => "block",
            R::Enum => "enum",
            R::EnumConstant => "enum_constant",
            R::Field => "field",
            R::Function => "function",
            R::FunctionIsMember => "function(is_member)",
            R::Namespace => "namespace",
            R::ObjcCategory => "objc_category",
            R::ObjcImplementation => "objc_implementation",
            R::ObjcInterface => "objc_interface",
            R::ObjcMethod => "objc_method",
            R::ObjcMethodIsInstance => "objc_method(is_instance)",
            R::ObjcProperty => "objc_property",
            R::ObjcProtocol => "objc_protocol",
            R::Record => "record",
            R::RecordNotIsUnion => "record(unless(is_union))",
            R::HasType => "hasType",
            R::HasTypeFunctionType => "hasType(functionType)",
            R::TypeAlias => "type_alias",
            R::Variable => "variable",
            R::VariableIsThreadLocal => "variable(is_thread_local)",
            R::VariableIsGlobal => "variable(is_global)",
            R::VariableIsLocal => "variable(is_local)",
            R::VariableIsParameter => "variable(is_parameter)",
            R::VariableNotIsParameter => "variable(unless(is_parameter))",
        }
    }
}

/// The subject-match rules of attribute `name` (normalized: `__x__` is
/// `x`) when `#pragma clang attribute` supports it, else `None` (clang
/// `isSupportedByPragmaAttribute` and `getMatchRules`, as clang 19
/// reports them for C++). An empty set accepts any rule (`annotate`).
pub fn attr_rules(name: &str) -> Option<&'static [MatchRule]> {
    use MatchRule as R;
    const ANY: &[MatchRule] = &[];
    const FN: &[MatchRule] = &[R::Function];
    const FN_GLOBAL: &[MatchRule] = &[R::Function, R::VariableIsGlobal];
    const VAR_FN_REC: &[MatchRule] = &[R::Variable, R::Function, R::Record];
    const ABI_TAG: &[MatchRule] = &[R::Record, R::Variable, R::Function, R::Namespace];
    const AVAILABILITY: &[MatchRule] = &[
        R::Enum,
        R::EnumConstant,
        R::Field,
        R::Function,
        R::Namespace,
        R::Record,
        R::TypeAlias,
        R::Variable,
    ];
    const WARN_UNUSED_RESULT: &[MatchRule] =
        &[R::Enum, R::Record, R::HasTypeFunctionType, R::TypeAlias];
    const NODEBUG: &[MatchRule] =
        &[R::TypeAlias, R::HasTypeFunctionType, R::VariableNotIsParameter];
    const REC: &[MatchRule] = &[R::Record];
    const VAR: &[MatchRule] = &[R::Variable];
    const FN_VAR: &[MatchRule] = &[R::Function, R::Variable];
    const GLOBAL: &[MatchRule] = &[R::VariableIsGlobal];
    const LOCAL: &[MatchRule] = &[R::VariableIsLocal];
    const PARAM: &[MatchRule] = &[R::VariableIsParameter];
    const FN_TYPE: &[MatchRule] = &[R::HasTypeFunctionType];
    const TLS: &[MatchRule] = &[R::VariableIsThreadLocal];
    Some(match name {
        "annotate" | "swift_attr" => ANY,
        "noinline"
        | "always_inline"
        | "target"
        | "target_clones"
        | "cold"
        | "hot"
        | "flatten"
        | "naked"
        | "minsize"
        | "optnone"
        | "malloc"
        | "constructor"
        | "destructor"
        | "leaf"
        | "returns_nonnull"
        | "returns_twice"
        | "error"
        | "warning"
        | "overloadable"
        | "patchable_function_entry"
        | "zero_call_used_regs"
        | "no_split_stack"
        | "no_stack_protector"
        | "not_tail_called"
        | "disable_tail_calls"
        | "no_instrument_function"
        | "no_profile_instrument_function"
        | "noduplicate"
        | "convergent"
        | "enable_if"
        | "callback"
        | "no_builtin"
        | "gnu_inline"
        | "assume_aligned"
        | "ifunc" => FN,
        "section" | "no_sanitize" | "no_sanitize_address" | "alias" => FN_GLOBAL,
        "availability" => AVAILABILITY,
        "warn_unused_result" => WARN_UNUSED_RESULT,
        "weak" | "internal_linkage" | "exclude_from_explicit_instantiation" => VAR_FN_REC,
        "abi_tag" => ABI_TAG,
        "weakref" => FN_VAR,
        "nodebug" => NODEBUG,
        "standalone_debug" | "ms_struct" | "trivial_abi" => REC,
        "no_destroy" | "always_destroy" | "init_priority" => VAR,
        "require_constant_initialization" | "loader_uninitialized" => GLOBAL,
        "uninitialized" | "cleanup" => LOCAL,
        "noescape" | "maybe_undef" => PARAM,
        "nothrow" | "nocf_check" => FN_TYPE,
        "tls_model" => TLS,
        _ => return None,
    })
}

/// [`attr_rules`] of an attribute as written (`__x__` normalized).
pub fn attr_rules_of(written: &str) -> Option<&'static [MatchRule]> {
    attr_rules(normalize_attr_name(written))
}

/// An attribute of a `#pragma clang attribute` group (clang
/// `PragmaAttributeEntry`).
#[derive(Clone, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub(crate) struct PragmaAttrEntry {
    /// The attribute's name as written (diagnostics).
    pub(crate) name: String,
    /// The attribute-name token (the unused warning).
    pub(crate) name_loc: Loc,
    /// The attribute node copied to each declaration selected (`None`:
    /// an attribute kiln accepts without effect).
    pub(crate) attr: Option<AttrId>,
    /// The rules that select a declaration.
    pub(crate) rules: Vec<MatchRule>,
    /// Whether a declaration received the attribute.
    pub(crate) used: bool,
}

/// A `#pragma clang attribute push` group (clang `PragmaAttributeGroup`).
#[derive(Clone, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub struct PragmaAttrGroup {
    /// The push's `attribute` token.
    pub(crate) loc: Loc,
    /// `NS` of `NS.push`.
    pub(crate) namespace: Option<Symbol>,
    /// The attributes pushed into the group.
    pub(crate) entries: Vec<PragmaAttrEntry>,
}

/// `'a', 'b', and 'c'` (clang `attrMatcherRuleListToString`).
fn rule_list(rules: &[MatchRule]) -> String {
    let mut s = String::new();
    for (i, r) in rules.iter().enumerate() {
        if i > 0 {
            s.push_str(if i == rules.len() - 1 { ", and " } else { ", " });
        }
        s.push('\'');
        s.push_str(r.spelling());
        s.push('\'');
    }
    s
}

impl Sema<'_, '_> {
    /// `#pragma clang attribute [NS.]push` (clang
    /// `ActOnPragmaAttributeEmptyPush`).
    pub fn act_on_pragma_attribute_push(&mut self, loc: Loc, namespace: Option<Symbol>) {
        self.pragma_attrs.push(PragmaAttrGroup { loc, namespace, entries: Vec::new() });
    }

    /// `#pragma clang attribute [NS.]pop` (clang `ActOnPragmaAttributePop`):
    /// closes the innermost group of namespace `namespace`, reporting its
    /// unused entries.
    pub fn act_on_pragma_attribute_pop(&mut self, loc: Loc, namespace: Option<Symbol>) {
        let Some(i) = self.pragma_attrs.iter().rposition(|g| g.namespace == namespace) else {
            // An empty stack is reported without the namespace, as clang.
            let ns = namespace
                .filter(|_| !self.pragma_attrs.is_empty())
                .map(|n| format!("{}.", self.ast.symbols().as_str(n)))
                .unwrap_or_default();
            self.emit(self.diag(DiagId::sema_pragma_attribute_stack_mismatch, loc).arg(ns));
            return;
        };
        let g = self.pragma_attrs.remove(i);
        for e in g.entries.iter().filter(|e| !e.used) {
            let d = self
                .diag(DiagId::sema_pragma_attribute_unused, e.name_loc)
                .arg(&e.name)
                .note(DiagId::note_pragma_attribute_region_ends_here, Span::empty(loc));
            self.emit(d);
        }
    }

    /// One attribute of a `#pragma clang attribute` with its parsed
    /// `apply_to` rules (`rules`, each at its primary rule's token, in
    /// rule order without duplicates): clang `ActOnPragmaAttributeAttribute`.
    /// `pragma` is the `attribute` token.
    pub fn act_on_pragma_attribute_attribute(
        &mut self,
        pragma: Loc,
        name: &str,
        name_loc: Loc,
        attr: Option<AttrId>,
        rules: &[(MatchRule, Loc)],
    ) {
        let strict = attr_rules_of(name).unwrap_or(&[]);
        let mut rest: Vec<(MatchRule, Loc)> = rules.to_vec();
        let mut chosen = Vec::new();
        if strict.is_empty() {
            // Any rule; a sub-rule beside its own primary rule, or two
            // sub-rules of a primary one of which is negated, contradict.
            let mut negated: Vec<(MatchRule, (MatchRule, Loc))> = Vec::new();
            for &(r, at) in &rest {
                let Some(p) = r.parent() else { continue };
                if rest.iter().any(|&(q, _)| q == p) {
                    let d = self
                        .diag(DiagId::sema_pragma_attribute_matcher_subrule_contradicts_rule, at)
                        .arg(r.spelling())
                        .arg(p.spelling());
                    self.emit(d);
                    continue;
                }
                if r.is_negated() && !negated.iter().any(|(q, _)| *q == p) {
                    negated.push((p, (r, at)));
                }
            }
            let mut ignore_negated = false;
            for &(r, _) in &rest {
                let Some(p) = r.parent() else { continue };
                if let Some(i) = negated.iter().position(|(q, _)| *q == p)
                    && negated[i].1.0 != r
                {
                    let (n, at) = negated[i].1;
                    let d = self
                        .diag(
                            DiagId::sema_pragma_attribute_matcher_negated_subrule_contradicts_subrule,
                            at,
                        )
                        .arg(n.spelling())
                        .arg(r.spelling());
                    self.emit(d);
                    ignore_negated = true;
                    negated.remove(i);
                }
            }
            chosen.extend(
                rest.iter().map(|&(r, _)| r).filter(|r| !(ignore_negated && r.is_negated())),
            );
            rest.clear();
        } else {
            for &s in strict {
                if let Some(i) = rest.iter().position(|&(r, _)| r == s) {
                    rest.remove(i);
                    chosen.push(s);
                }
            }
            rest.retain(|&(r, _)| {
                if r.parent().is_some_and(|p| strict.contains(&p)) {
                    chosen.push(r);
                    false
                } else {
                    true
                }
            });
        }
        if !rest.is_empty() {
            rest.sort_by_key(|&(r, _)| r);
            let list: Vec<MatchRule> = rest.iter().map(|&(r, _)| r).collect();
            let d = self
                .diag(DiagId::sema_pragma_attribute_invalid_matchers, pragma)
                .arg(name)
                .arg(rule_list(&list));
            self.emit(d);
        }
        let Some(g) = self.pragma_attrs.last_mut() else {
            self.emit(self.diag(DiagId::sema_pragma_attr_attr_no_push, pragma));
            return;
        };
        g.entries.push(PragmaAttrEntry {
            name: name.to_owned(),
            name_loc,
            attr,
            rules: chosen,
            used: false,
        });
    }

    /// At the end of the translation unit: a group still open is an error
    /// at its push (clang `DiagnoseUnterminatedPragmaAttribute`).
    pub(crate) fn diagnose_unterminated_pragma_attr(&mut self) {
        if let Some(g) = self.pragma_attrs.last() {
            self.emit(self.diag(DiagId::sema_pragma_attribute_no_pop_eof, g.loc));
        }
    }

    /// Adds to `attrs`, the attributes of declaration `d` being checked, a
    /// copy of each pushed attribute a rule of its entry selects `d` for
    /// (clang `AddPragmaAttributes`).
    pub(crate) fn add_pragma_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        if self.pragma_attrs.is_empty() || !self.inst.stack.is_empty() {
            return;
        }
        let kind = self.kind_of(d);
        let var = self.ast.decls.var(d);
        let subject = Subject {
            kind,
            ty: self.ast.decls.get(d).ty(),
            local: matches!(kind, DeclKind::Var | DeclKind::ParmVar) && self.has_local_storage(d),
            tls: var.is_some_and(|v| v.spec.tls() != TlsKind::None),
            union: self.ast.decls.record(d).is_some_and(|x| x.tag == TagKind::Union),
        };
        self.add_pragma_attrs_to(&subject, attrs);
    }

    /// [`Sema::add_pragma_attrs`] for a function parameter of type `ty`
    /// not declared yet (its attributes go with its `ParamInfo`). A
    /// `void` parameter receives none.
    pub(crate) fn add_pragma_param_attrs(&mut self, ty: QualType, attrs: &mut Vec<AttrId>) {
        if self.pragma_attrs.is_empty() || !self.inst.stack.is_empty() || self.ast.types.is_void(ty)
        {
            return;
        }
        let subject = Subject {
            kind: DeclKind::ParmVar,
            ty: Some(ty),
            local: true,
            tls: false,
            union: false,
        };
        self.add_pragma_attrs_to(&subject, attrs);
    }

    fn add_pragma_attrs_to(&mut self, subject: &Subject, attrs: &mut Vec<AttrId>) {
        let mut add: Vec<AttrNode> = Vec::new();
        let mut groups = std::mem::take(&mut self.pragma_attrs);
        for g in &mut groups {
            for e in &mut g.entries {
                if e.rules.iter().any(|&r| self.pragma_rule_applies(r, subject)) {
                    e.used = true;
                    if let Some(a) = e.attr {
                        add.push(*self.ast.attrs.get(a));
                    }
                }
            }
        }
        self.pragma_attrs = groups;
        for n in add {
            attrs.push(self.ast.attrs.push(n));
        }
    }

    /// Whether rule `r` selects declaration `s` (clang
    /// `checkAttributeMatchRuleAppliesTo`).
    fn pragma_rule_applies(&self, r: MatchRule, s: &Subject) -> bool {
        use MatchRule as R;
        let k = s.kind;
        let var = matches!(k, DeclKind::Var | DeclKind::ParmVar);
        match r {
            R::Function => k.is_function(),
            R::FunctionIsMember => matches!(
                k,
                DeclKind::CXXMethod
                    | DeclKind::CXXConstructor
                    | DeclKind::CXXDestructor
                    | DeclKind::CXXConversion
            ),
            R::Enum => k == DeclKind::Enum,
            R::EnumConstant => k == DeclKind::EnumConstant,
            R::Field => k == DeclKind::Field,
            R::Namespace => k == DeclKind::Namespace,
            R::Record => matches!(
                k,
                DeclKind::CXXRecord
                    | DeclKind::ClassTemplateSpecialization
                    | DeclKind::ClassTemplatePartialSpecialization
            ),
            R::RecordNotIsUnion => self.pragma_rule_applies(R::Record, s) && !s.union,
            R::TypeAlias => matches!(k, DeclKind::Typedef | DeclKind::TypeAlias),
            R::Variable => var,
            R::VariableIsThreadLocal => k == DeclKind::Var && s.tls,
            R::VariableIsGlobal => k == DeclKind::Var && !s.local,
            R::VariableIsLocal => k == DeclKind::Var && s.local,
            R::VariableIsParameter => k == DeclKind::ParmVar,
            R::VariableNotIsParameter => k == DeclKind::Var,
            R::HasTypeFunctionType => {
                // clang `Decl::getFunctionType`: a value declaration's or
                // typedef-name's type, through a pointer or reference.
                let fn_like = k.is_function()
                    || var
                    || matches!(
                        k,
                        DeclKind::Field
                            | DeclKind::EnumConstant
                            | DeclKind::Typedef
                            | DeclKind::TypeAlias
                    );
                fn_like
                    && s.ty.is_some_and(|t| {
                        let ty = &self.ast.types;
                        ty.is_function(t)
                            || ((ty.is_pointer(t) || ty.is_reference(t))
                                && ty.pointee(t).is_some_and(|p| ty.is_function(p)))
                    })
            }
            _ => false,
        }
    }
}

/// What a subject-match rule tests of a declaration.
struct Subject {
    kind: DeclKind,
    ty: Option<QualType>,
    /// A variable with local storage (clang `hasLocalStorage`).
    local: bool,
    /// A thread-local variable.
    tls: bool,
    /// A union.
    union: bool,
}
