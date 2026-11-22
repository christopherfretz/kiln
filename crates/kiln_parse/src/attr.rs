//! Attributes ([dcl.attr], GNU `__attribute__`; spec/parse.md §6).
//!
//! The parser collects the attribute's scope, name and arguments and hands
//! them to [`kiln_sema::Sema::act_on_attribute`], which decides what is
//! known. Arguments are parsed as expressions only for the attributes that
//! take them ([`takes_expr_args`]); the arguments of other attributes are
//! skipped as balanced tokens.

use kiln_ast::AttrId;
use kiln_ast::attr::AttrSyntax;
use kiln_diag::DiagId;
use kiln_lex::{Punct, TokKind};
use kiln_sema::Declarator;
use kiln_sema::decl::AttrArg;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::expr::prec;
use crate::keywords::Kw;
use crate::parser::Parser;
use crate::tentative::TypeIdContext;

/// What one `[[...]]` has held so far: an annotation-list mixes with
/// neither attributes nor a using-prefix (reported once).
#[derive(Default)]
struct AnnotMix {
    /// An attribute or a using-prefix (which makes it an attribute-list).
    attr: bool,
    annot: bool,
    reported: bool,
}

/// True for the attributes whose arguments are expressions.
fn takes_expr_args(name: &str) -> bool {
    let n = kiln_ast::attr::normalize_attr_name(name);
    matches!(
        n,
        "deprecated"
            | "unavailable"
            | "annotate"
            | "require_constant_initialization"
            | "nodiscard"
            | "aligned"
            | "warn_unused_result"
            | "ext_vector_type"
            | "vector_size"
            | "address_space"
            | "opencl_global"
            | "opencl_local"
            | "opencl_constant"
            | "opencl_private"
            | "opencl_generic"
            | "opencl_global_device"
            | "opencl_global_host"
            | "neon_vector_type"
            | "neon_polyvector_type"
            | "alloc_size"
            | "alloc_align"
            | "abi_tag"
            | "flag_enum"
            | "enum_extensibility"
            | "preferred_name"
            | "coro_only_destroy_when_complete"
            | "optnone"
            | "musttail"
            | "selectany"
            | "ms_struct"
            | "gcc_struct"
            | "minsize"
            | "disable_tail_calls"
            | "trivial_abi"
            | "naked"
            | "may_alias"
            | "init_priority"
            | "visibility"
            | "type_visibility"
            | "nonnull"
            | "format"
            | "enable_if"
            | "diagnose_if"
            | "target"
            | "min_vector_width"
            | "pass_object_size"
            | "pass_dynamic_object_size"
            | "weakref"
            | "alias"
            | "ifunc"
            | "section"
            | "analyzer_noreturn"
            | "constructor"
            | "destructor"
            | "cleanup"
            | "internal_linkage"
            | "common"
            | "tls_model"
            | "model"
            | "returns_twice"
            | "mode"
            | "arm_sve_vector_bits"
            | "__clang_arm_builtin_alias"
            | "clang_builtin_alias"
            | "builtin_alias"
            | "target_version"
            | "target_clones"
            | "cpu_dispatch"
            | "cpu_specific"
            | "regparm"
            | "no_caller_saved_registers"
            | "no_builtin"
            | "interrupt"
            // clang `ParameterABIAttr` (WP-1745).
            | "swift_context"
            | "swift_async_context"
            | "swift_error_result"
            | "swift_indirect_result"
    ) || kiln_ast::attr::CC_ATTR_SPELLINGS.contains(&n)
        || class_b_takes_expr_args(n)
}

/// The attributes of [`takes_expr_args`] whose argument count is checked
/// at parse time from the argument table (spec/parse.md §6.2 "Argument
/// table", WP-1879): kiln's handlers of these accept a wrong count
/// (`abi_tag`) or report it in other words (`aligned(1, 2)` is "takes no
/// more than 1 argument", a `[[clang::x]]` one at clang 22 is named
/// `clang::x`). The other handlers count as clang does, after their
/// subject checks, so they keep clang's order.
fn counted_at_parse(name: &str) -> bool {
    matches!(
        kiln_ast::attr::normalize_attr_name(name),
        "abi_tag"
            | "alias"
            | "aligned"
            | "alloc_align"
            | "alloc_size"
            | "enum_extensibility"
            | "format"
            | "ifunc"
            | "min_vector_width"
            | "mode"
            | "neon_polyvector_type"
            | "neon_vector_type"
            | "no_caller_saved_registers"
            | "pass_dynamic_object_size"
            | "pass_object_size"
            | "regparm"
            | "section"
            | "target"
            | "target_version"
            | "vector_size"
    )
}

/// WP-813: the class B attributes, whose arguments are expressions.
fn class_b_takes_expr_args(n: &str) -> bool {
    matches!(
        n,
        "no_stack_protector"
            | "patchable_function_entry"
            | "zero_call_used_regs"
            | "speculative_load_hardening"
            | "no_speculative_load_hardening"
            | "no_split_stack"
            | "no_instrument_function"
            | "no_profile_instrument_function"
            | "xray_always_instrument"
            | "xray_never_instrument"
            | "xray_log_args"
            | "disable_sanitizer_instrumentation"
            | "nocf_check"
            | "function_return"
            | "cfi_canonical_jump_table"
            | "force_align_arg_pointer"
            | "flatten"
            | "leaf"
            | "nomerge"
            | "not_tail_called"
            | "noduplicate"
            | "convergent"
            | "nouwtable"
            | "lifetimebound"
            | "noinline"
            | "always_inline"
            | "assume_aligned"
            | "align_value"
            | "callback"
            | "noescape"
            | "maybe_undef"
            | "code_align"
            | "loader_uninitialized"
            | "lto_visibility_public"
            | "standalone_debug"
            | "no_sanitize"
            | "no_sanitize_address"
            | "no_address_safety_analysis"
            | "no_sanitize_memory"
            | "no_sanitize_thread"
            | "coro_await_elidable"
            | "coro_await_elidable_argument"
            // `[[omp::assume("s")]]` (the others parse their own).
            | "assume"
    )
}

/// True for the attributes whose arguments are all identifiers when
/// written as one (clang `VariadicIdentifierArgument`: `cpu_dispatch`,
/// `cpu_specific`).
fn takes_ident_list(name: &str) -> bool {
    matches!(kiln_ast::attr::normalize_attr_name(name), "cpu_dispatch" | "cpu_specific")
}

/// True when argument `i` of attribute `name` must be written as a string
/// literal (clang `attributeStringLiteralListArg`, for the attributes kiln
/// checks this way): another token there is "expected string literal as
/// argument of 'name' attribute" at that token and the attribute is
/// dropped, without parsing the argument as an expression.
fn string_literal_arg(name: &str, i: usize) -> bool {
    match kiln_ast::attr::normalize_attr_name(name) {
        // WP-954: clang's `StringArgument` first also for `unavailable`'s
        // optional message and `annotate`'s annotation; WP-1745: and for
        // `model` (clang `CodeModelAttr`), known to the parser on every
        // target and release. WP-1853: clang's table is keyed by the name
        // alone, so it holds for an `__attribute__((nodiscard(...)))`
        // clang does not know.
        // WP-1881: and the string `EnumArgument`s that take a leading
        // identifier ([`takes_ident_arg`]), and the names of `alias`,
        // `ifunc` and `weakref` (the table of spec/parse.md §6.2).
        "target"
        | "target_version"
        | "unavailable"
        | "annotate"
        | "model"
        | "nodiscard"
        | "warn_unused_result"
        | "section"
        | "tls_model"
        | "visibility"
        | "type_visibility"
        | "pcs"
        | "zero_call_used_regs"
        | "function_return"
        | "alias"
        | "ifunc"
        | "weakref" => i == 0,
        // The messages of `enable_if` and `diagnose_if` (and the
        // latter's kind).
        "enable_if" => i == 1,
        "diagnose_if" => i == 1 || i == 2,
        "deprecated" => i <= 1,
        "target_clones" | "no_sanitize" | "abi_tag" | "no_builtin" => true,
        _ => false,
    }
}

/// True for the attributes whose arguments may be identifiers (clang
/// `attributeHasVariadicIdentifierArg`: `callback`).
fn takes_ident_args(name: &str) -> bool {
    kiln_ast::attr::normalize_attr_name(name) == "callback"
}

/// True for the attributes whose argument is a type-id (clang
/// `attributeIsTypeArgAttr`).
fn takes_type_arg(name: &str) -> bool {
    kiln_ast::attr::normalize_attr_name(name) == "preferred_name"
}

/// True for the attributes whose first argument is an identifier when
/// one is written (clang `attributeHasIdentifierArg`; WP-1881: the string
/// `EnumArgument`s of `visibility`, `type_visibility`, `pcs`,
/// `zero_call_used_regs` and `function_return`, whose sema then rejects
/// an identifier: "'visibility' attribute requires a string").
fn takes_ident_arg(name: &str) -> bool {
    let n = kiln_ast::attr::normalize_attr_name(name);
    matches!(
        n,
        "enum_extensibility"
            | "format"
            | "mode"
            | "visibility"
            | "type_visibility"
            | "pcs"
            | "zero_call_used_regs"
            | "function_return"
            | "__clang_arm_builtin_alias"
            | "clang_builtin_alias"
            | "builtin_alias"
    )
}

/// How clang checks the argument clause of a standard attribute once it
/// has parsed it (clang `ParseCXX11AttributeArgs` for an
/// `IsBuiltInOrStandardCXX11Attribute`, [dcl.attr.grammar]/4; WP-1852).
#[derive(Clone, Copy, PartialEq, Eq)]
enum StdArgClause {
    /// It takes none, not even `()`: "attribute 'x' cannot have an
    /// argument list".
    Forbidden,
    /// It takes an optional one, which must not be empty:
    /// `[[deprecated()]]` is "parentheses must be omitted if 'deprecated'
    /// attribute's argument list is empty".
    NonEmpty,
}

/// The [`StdArgClause`] of the `[[...]]` attribute `scope::name` (`c`: a
/// C unit), if it has one. As clang: `__x__` is `x` (clang's attribute
/// kind), except for `nodiscard` and `maybe_unused`, whose kinds are
/// shared with GNU attributes (`warn_unused_result`, `unused`) and so are
/// matched by their exact names; scoped, only `[[clang::fallthrough]]`
/// (C++ only). In C, the C++-only attributes are unknown and not checked.
fn std_arg_clause(
    name: &str,
    scope: Option<&str>,
    c: bool,
    std: kiln_lex::Std,
) -> Option<StdArgClause> {
    let n = kiln_ast::attr::normalize_attr_name(name);
    if let Some(sc) = scope {
        return (matches!(sc, "clang" | "_Clang") && !c && n == "fallthrough")
            .then_some(StdArgClause::Forbidden);
    }
    match name {
        "nodiscard" => return Some(StdArgClause::NonEmpty),
        "maybe_unused" => return Some(StdArgClause::Forbidden),
        _ => {}
    }
    match n {
        "deprecated" => Some(StdArgClause::NonEmpty),
        "fallthrough" | "noreturn" => Some(StdArgClause::Forbidden),
        "no_unique_address" | "likely" | "unlikely" if !c => Some(StdArgClause::Forbidden),
        // P3475R2: no longer a standard attribute in C++26.
        "carries_dependency" if !c && std < kiln_lex::Std::Cxx26 => Some(StdArgClause::Forbidden),
        "indeterminate" if !c && std >= kiln_lex::Std::Cxx26 => Some(StdArgClause::Forbidden),
        _ => None,
    }
}

impl Parser<'_, '_> {
    /// True at `[[`, or at an SME keyword attribute (`__arm_streaming`,
    /// clang `KEYWORD_ATTRIBUTE`s, parsed wherever a C++11
    /// attribute-specifier is).
    pub(crate) fn at_cxx11_attr(&self) -> bool {
        (self.at(Punct::LSquare) && self.peek_is(1, Punct::LSquare)) || self.at_sme_attr()
    }

    /// True at an SME keyword attribute.
    fn at_sme_attr(&self) -> bool {
        self.kw().is_some_and(|k| k.sme_index().is_some())
    }

    /// An SME keyword attribute (ACLE; clang `ParseCXX11AttributeSpecifier`
    /// of a `KEYWORD_ATTRIBUTE`): the keyword, and for the state keywords
    /// `__arm_new`, `__arm_in`, `__arm_out`, `__arm_inout`,
    /// `__arm_preserves` and `__arm_agnostic` a parenthesized list of
    /// string literals.
    fn parse_sme_attr(&mut self, out: &mut Vec<AttrId>) {
        let Some(idx) = self.kw().and_then(Kw::sme_index) else { return };
        let loc = self.bump().span.lo;
        let mut states = Vec::new();
        let takes_args = idx >= 2 && idx != 7;
        if takes_args {
            let Some(open) = self.expect(Punct::LParen) else { return };
            // `__arm_in()`: `missing state for '__arm_in'` (sema).
            while !self.at(Punct::RParen) {
                if !self.at_string() {
                    let name = kiln_ast::attr::SME_KEYWORDS[idx];
                    let d = self.s.diag(DiagId::parse_attribute_requires_string, self.loc());
                    self.emit(d.arg(name));
                    self.skip_until(&[Punct::RParen], false, true);
                    return;
                }
                let Some(e) = self.parse_string_literal(true) else {
                    self.skip_until(&[Punct::RParen], false, true);
                    return;
                };
                states.push(e);
                if self.eat(Punct::Comma).is_none() {
                    break;
                }
            }
            self.expect_closing(Punct::RParen, Punct::LParen, open);
        }
        if let Some(a) = self.s.act_on_sme_attribute(idx, &states, loc) {
            out.push(a);
        }
    }

    /// True at a `[[...]]` / `alignas(...)` attribute-specifier-seq
    /// followed by `using`.
    pub(crate) fn at_attributed_using(&self) -> bool {
        if !self.at_cxx11_attr() && !self.at_kw(Kw::Alignas) {
            return false;
        }
        self.after_cxx11_attrs().is_some_and(|j| self.kw_of(self.tok_at(j)) == Some(Kw::Using))
    }

    /// `[[...]] using ...`: the attribute-specifier-seq, then the
    /// using-directive, using-declaration or alias-declaration
    /// ([`Parser::parse_using_leading`]).
    pub(crate) fn parse_attributed_using(&mut self) -> Vec<kiln_ast::DeclId> {
        let at = self.loc();
        let mut attrs = Vec::new();
        self.parse_attributes(&mut attrs);
        self.parse_using_leading(&attrs, Some(at))
    }

    /// A `using` declaration after the attribute-specifier-seq `attrs`
    /// that starts at `at` (clang `ParseUsingDirectiveOrDeclaration`):
    /// they appertain to a using-directive ([namespace.udir]/1); before
    /// an alias-declaration they are an error, `an attribute list cannot
    /// appear here`, and dropped ([dcl.pre]/5);
    /// before a using-declaration or using-enum-declaration they are a
    /// clang extension, `ISO C++ does not allow an attribute list to
    /// appear here` at each C++11 attribute, and appertain to each
    /// using-declarator.
    pub(crate) fn parse_using_leading(
        &mut self,
        attrs: &[AttrId],
        at: Option<Loc>,
    ) -> Vec<kiln_ast::DeclId> {
        if attrs.is_empty() || at.is_none() {
            return self.parse_using();
        }
        let next = self.peek(1);
        if self.kw_of(next) == Some(Kw::Namespace) {
            return self.parse_using_attrs(attrs);
        }
        let alias = self.ident_of(next).is_some()
            && (self.peek_is(2, Punct::Equal)
                || self.peek_is(2, Punct::LSquare)
                || matches!(self.kw_of(self.peek(2)), Some(Kw::Attribute | Kw::Alignas)));
        if alias {
            self.prohibit_attrs_at(at);
            return self.parse_using();
        }
        for &a in attrs {
            let node = *self.s.ast.attrs.get(a);
            if matches!(node.syntax, AttrSyntax::Cxx11 | AttrSyntax::Keyword) {
                self.s.error(DiagId::parse_cxx11_attr_placement, node.loc);
            }
        }
        if self.kw_of(next) == Some(Kw::Enum) {
            return self.parse_using();
        }
        self.parse_using_attrs(attrs)
    }

    /// `an attribute list cannot appear here` at `at`, the start of an
    /// attribute-specifier-seq already parsed and dropped (clang
    /// `ProhibitAttributes`).
    /// An attribute-specifier-seq after the class-name of a
    /// class-specifier or a class-head-only declaration
    /// (`struct S [[x]] final { };`, `struct S alignas(4);`): its place
    /// is after the class-key ([class.pre]/1). Like clang, reports "an
    /// attribute list cannot appear here" and moves the attributes into
    /// `attrs`, the class's own. One after `final` (`struct S final
    /// [[x]] { }`) is reported and dropped by
    /// [`Self::parse_class_virt_specifiers`]. Not at a reference
    /// (`struct S [[x]] *p;`): those appertain to the type
    /// ([dcl.spec.general]/1).
    pub(crate) fn parse_class_head_attrs(&mut self, storage: bool, attrs: &mut Vec<AttrId>) {
        if !self.at_kw(Kw::Alignas) && !self.at_cxx11_attr() {
            return;
        }
        let Some(i) = self.after_cxx11_attrs() else { return };
        let mut j = i;
        while self.is_final_tok(self.tok_at(j)) {
            j += 1;
        }
        let Some(j) = self.after_cxx11_attrs_at(j) else { return };
        let ok = match Self::punct_of(self.tok_at(j)) {
            Some(Punct::LBrace | Punct::Colon) => true,
            Some(Punct::Semi) => j == i && !storage,
            _ => false,
        };
        if !ok {
            return;
        }
        let at = self.loc();
        self.prohibit_attrs_at(Some(at));
        self.parse_attributes(attrs);
    }

    pub(crate) fn prohibit_attrs_at(&mut self, at: Option<Loc>) {
        if let Some(at) = at {
            self.s.error(DiagId::parse_attributes_not_allowed, at);
        }
    }

    /// The token index after the `[[...]]` / `alignas(...)` specifiers at
    /// the current token (the current index if there are none).
    fn after_cxx11_attrs(&self) -> Option<usize> {
        self.after_cxx11_attrs_at(self.pos())
    }

    /// [`Self::after_cxx11_attrs`] from token index `i`.
    pub(crate) fn after_cxx11_attrs_at(&self, mut i: usize) -> Option<usize> {
        loop {
            let t = self.tok_at(i);
            if Self::punct_of(t) == Some(Punct::LSquare)
                && Self::punct_of(self.tok_at(i + 1)) == Some(Punct::LSquare)
            {
                i = self.skip_balanced(i)?;
            } else if let Some(k) = self.kw_of(t).and_then(Kw::sme_index) {
                i += 1;
                if k >= 2 && k != 7 && Self::punct_of(self.tok_at(i)) == Some(Punct::LParen) {
                    i = self.skip_balanced(i)?;
                }
            } else if self.kw_of(t) == Some(Kw::Alignas) {
                i = self.skip_balanced(i + 1)?;
            } else {
                return Some(i);
            }
        }
    }

    /// True at GNU `__attribute__((...))` specifiers followed by `;`: an
    /// attributed null statement, `__attribute__((fallthrough));` (clang
    /// `ParseStatementOrDeclarationAfterAttributes`, `kw___attribute`).
    /// Likewise before `return` or `{`, which cannot start a declaration
    /// (`__attribute__((musttail)) return f(x);`, WP-809).
    ///
    /// The specifier sequence may mix the GNU and the C++11 forms in any
    /// order (`__attribute__(()) [[]] __attribute__((assume(x))) [[]];`,
    /// clang's `ParseStatementOrDeclarationAfterAttributes` retries after
    /// each run), and a label may follow it
    /// (`__attribute__((unused)) L: ...`, [stmt.label]/1). Only a leading
    /// `__attribute__` is looked at here: a leading `[[` is already a
    /// statement's attribute-specifier-seq.
    pub(crate) fn at_gnu_attr_null_stmt(&self) -> bool {
        if !self.at_kw(Kw::Attribute) {
            return false;
        }
        let mut i = self.pos();
        loop {
            if self.kw_of(self.tok_at(i)) == Some(Kw::Attribute) {
                let Some(j) = self.skip_balanced(i + 1) else { return false };
                i = j;
                continue;
            }
            let Some(j) = self.after_cxx11_attrs_at(i) else { return false };
            if j == i {
                break;
            }
            i = j;
        }
        let t = self.tok_at(i);
        matches!(Self::punct_of(t), Some(Punct::Semi | Punct::LBrace))
            || self.kw_of(t) == Some(Kw::Return)
            || (self.ident_of(t).is_some()
                && Self::punct_of(self.tok_at(i + 1)) == Some(Punct::Colon))
    }

    /// A C++11 attribute-specifier-seq at namespace scope before a
    /// declaration that takes none ([dcl.pre]/5, [dcl.attr.grammar]/5):
    /// a namespace-definition or namespace-alias-definition, a
    /// static_assert-declaration, a template-declaration, explicit
    /// instantiation or specialization, a linkage-specification, a C
    /// asm-declaration or an export-declaration (clang
    /// `ProhibitAttributes`: "an attribute list cannot appear here").
    /// Reports and drops it.
    pub(crate) fn discard_attrs_before_unattributed_decl(&mut self) {
        if !self.at_kw(Kw::Alignas) && !self.at_cxx11_attr() {
            return;
        }
        let Some(j) = self.after_cxx11_attrs() else { return };
        let next = self.kw_of(self.tok_at(j));
        let bad = match next {
            Some(Kw::Namespace | Kw::StaticAssert | Kw::Template | Kw::Export) => true,
            // CWG2262: an asm-declaration takes them (`parse_asm_decl_attrs`).
            Some(Kw::Asm) => self.s.is_c(),
            Some(Kw::Inline) => self.kw_of(self.tok_at(j + 1)) == Some(Kw::Namespace),
            Some(Kw::Extern) => {
                (!self.s.is_c()
                    && matches!(
                        self.tok_at(j + 1).kind,
                        TokKind::StringLit(_) | TokKind::RawStringLit(_)
                    ))
                    || self.kw_of(self.tok_at(j + 1)) == Some(Kw::Template)
            }
            // `[[x]] __attribute__((y)) extern "C" ...`: clang's
            // `ProhibitAttributes` after the GNU decl-specifiers.
            Some(Kw::Attribute) => self.at_gnu_attrs_before_linkage_spec(j - self.pos()),
            _ => false,
        };
        if bad {
            let at = self.loc();
            let mut attrs = Vec::new();
            self.parse_attributes(&mut attrs);
            self.s.error(DiagId::parse_attributes_not_allowed, at);
        }
    }

    /// A C++11 attribute-specifier-seq (`[[...]]` and `alignas(...)`
    /// specifiers, no GNU attributes) into `out` (clang
    /// `MaybeParseCXX11Attributes`).
    pub(crate) fn parse_cxx11_attr_seq(&mut self, out: &mut Vec<AttrId>) {
        let held = self.s.hold_unknown_attrs();
        self.parse_cxx11_attr_seq_held(out);
        self.s.release_unknown_attrs(held);
    }

    fn parse_cxx11_attr_seq_held(&mut self, out: &mut Vec<AttrId>) {
        while self.at_cxx11_attr() || self.at_kw(Kw::Alignas) {
            if self.at_kw(Kw::Alignas) {
                self.parse_alignas(out);
            } else {
                self.parse_cxx11_attributes(out);
            }
        }
    }

    /// clang `DiagnoseAndSkipCXX11Attributes`: a `[[...]]` / `alignas(...)`
    /// specifier-seq where none may appear is reported once, at its
    /// start ("an attribute list cannot appear here"), and skipped.
    pub(crate) fn diagnose_and_skip_cxx11_attrs(&mut self) {
        if !self.at_cxx11_attr() && !self.at_kw(Kw::Alignas) {
            return;
        }
        let at = self.loc();
        let mut attrs = Vec::new();
        self.parse_cxx11_attr_seq(&mut attrs);
        self.s.error(DiagId::parse_attributes_not_allowed, at);
    }

    /// The attributes after a member-declarator's bit-width or asm label
    /// (clang `ParseCXXMemberDeclaratorBeforeInitializer`): GNU
    /// attributes appertain to the member; a C++11 specifier-seq before
    /// or after them is reported and dropped.
    pub(crate) fn parse_member_tail_attrs(&mut self, d: &mut Declarator) {
        self.diagnose_and_skip_cxx11_attrs();
        self.parse_declarator_gnu_attr_seq(d);
        self.diagnose_and_skip_cxx11_attrs();
    }

    /// [`Parser::parse_gnu_attr_seq`] after declarator `d`, into its
    /// attributes: the arguments of an `enable_if` or `diagnose_if` after a function
    /// declarator see its parameters (clang `ParseGNUAttributeArgs`
    /// re-enters the prototype scope, `ActOnReentrantPrototypeParam`).
    pub(crate) fn parse_declarator_gnu_attr_seq(&mut self, d: &mut Declarator) {
        if !self.at_kw(Kw::Attribute) {
            return;
        }
        let mut attrs = std::mem::take(&mut d.attrs);
        while self.at_kw(Kw::Attribute) {
            self.parse_gnu_attributes_in(&mut attrs, Some(d));
        }
        d.attrs = attrs;
    }

    /// A run of GNU `__attribute__((...))` specifiers into `out` (clang
    /// `MaybeParseGNUAttributes`), as after a declarator.
    pub(crate) fn parse_gnu_attr_seq(&mut self, out: &mut Vec<AttrId>) {
        while self.at_kw(Kw::Attribute) {
            self.parse_gnu_attributes(out);
        }
    }

    /// True at any attribute-specifier.
    pub(crate) fn at_attr(&self) -> bool {
        self.at_cxx11_attr() || matches!(self.kw(), Some(Kw::Alignas | Kw::Attribute))
    }

    /// Parses a sequence of attribute-specifiers (`[[...]]`, `alignas(...)`,
    /// `__attribute__((...))`) into `out`.
    pub(crate) fn parse_attributes(&mut self, out: &mut Vec<AttrId>) {
        let held = self.s.hold_unknown_attrs();
        self.parse_attributes_held(out);
        self.s.release_unknown_attrs(held);
    }

    fn parse_attributes_held(&mut self, out: &mut Vec<AttrId>) {
        loop {
            if self.at_cxx11_attr() {
                self.parse_cxx11_attributes(out);
            } else if self.at_kw(Kw::Alignas) {
                self.parse_alignas(out);
            } else if self.at_kw(Kw::Attribute) {
                self.parse_gnu_attributes(out);
            } else if self.at_kw(Kw::Declspec) {
                // Microsoft `__declspec(...)` is an extension clang enables
                // only with `-fdeclspec` / `-fms-extensions`, which kiln
                // does not offer (no Microsoft target): clang's
                // `err_ms_attributes_not_enabled` at the keyword, and the
                // parenthesized list is skipped.
                self.error_here(DiagId::parse_ms_attributes_not_enabled);
                self.bump();
                if self.eat(Punct::LParen).is_some() {
                    self.skip_until(&[Punct::RParen], false, true);
                }
            } else {
                return;
            }
        }
    }

    /// `[[...]]` where no attribute-specifier may appear, such as after
    /// a parenthesized declarator (`int (x) [[a]];`; clang's
    /// `CheckProhibitedCXX11Attribute` in `ParseBracketDeclarator`):
    /// each specifier is reported at its `[[` and skipped unparsed.
    pub(crate) fn discard_prohibited_cxx11_attrs(&mut self) {
        while self.at_cxx11_attr() {
            let at = self.bump().span.lo;
            let mut depth = 1usize;
            while depth > 0 {
                let t = self.tok();
                if t.kind == TokKind::Eof {
                    break;
                }
                match Self::punct_of(t) {
                    Some(Punct::LSquare) => depth += 1,
                    Some(Punct::RSquare) => depth -= 1,
                    _ => {}
                }
                self.bump();
            }
            self.s.error(DiagId::parse_attributes_not_allowed, at);
        }
    }

    /// Parses and discards attributes (where kiln does not yet attach them).
    pub(crate) fn skip_attributes(&mut self) {
        let mut v = Vec::new();
        self.parse_attributes(&mut v);
    }

    /// At an attribute-specifier-seq (`[[...]]`, `alignas(...)` and GNU
    /// `__attribute__((...))` in any order) followed by `asm`: the
    /// attributed asm-declaration of CWG2262 ([dcl.asm]/1). C++ only.
    pub(crate) fn at_attrs_before_asm(&self) -> bool {
        if self.s.is_c() || !self.at_attr() {
            return false;
        }
        let mut i = self.pos();
        loop {
            let Some(j) = self.after_cxx11_attrs_at(i) else { return false };
            i = j;
            if self.kw_of(self.tok_at(i)) != Some(Kw::Attribute) {
                return self.kw_of(self.tok_at(i)) == Some(Kw::Asm);
            }
            if Self::punct_of(self.tok_at(i + 1)) != Some(Punct::LParen) {
                return false;
            }
            let Some(j) = self.skip_balanced(i + 1) else { return false };
            i = j;
        }
    }

    /// The attribute-specifier-seq of an asm-declaration (CWG2262,
    /// [dcl.asm]/1; spec/parse.md "Attributes on an asm-declaration"),
    /// before its `asm`. No attribute appertains to it: each attribute of
    /// a `[[...]]` or an `alignas` is `'x' attribute ignored`
    /// [-Wignored-attributes] at its start (clang 22's block-scope
    /// `ParseStatementOrDeclarationAfterAttributes`: unknown ones too, and
    /// not as unknown; arguments are skipped unparsed). clang 22 rejects
    /// any attribute before a namespace-scope asm-declaration ("an
    /// attribute list cannot appear here"); the text allows it, so kiln
    /// treats it as at block scope (GCC 16.1 accepts both and warns
    /// "attributes ignored on 'asm' declaration"). GNU `__attribute__`
    /// specifiers are as clang treats them: skipped silently at namespace
    /// scope, "an attribute list cannot appear here" at the first one in
    /// a block (`block`). An annotation is "an annotation cannot
    /// appertain to an asm-declaration" ([dcl.attr.annotation]/1).
    pub(crate) fn parse_asm_decl_attrs(&mut self, block: bool) {
        let mut gnu_at = None;
        // Only annotations land here: [dcl.attr.annotation]/1 rejects them.
        let mut annots = Vec::new();
        loop {
            if self.at_cxx11_attr() {
                self.parse_cxx11_attributes_in(&mut annots, true);
            } else if self.at_kw(Kw::Alignas) {
                let at = self.bump().span.lo;
                if self.eat(Punct::LParen).is_some() {
                    self.skip_until(&[Punct::RParen], false, true);
                }
                let d = self.s.diag(DiagId::parse_attribute_ignored, at).arg("alignas");
                self.emit(d);
            } else if self.at_kw(Kw::Attribute) {
                let at = self.bump().span.lo;
                gnu_at.get_or_insert(at);
                if self.eat(Punct::LParen).is_some() {
                    self.skip_until(&[Punct::RParen], false, true);
                }
            } else {
                break;
            }
        }
        self.s.reject_annotations_in(&annots, Some("an asm-declaration"));
        if block && let Some(at) = gnu_at {
            self.s.error(DiagId::parse_attributes_not_allowed, at);
        }
    }

    /// One attribute of an asm-declaration's `[[...]]`, after its
    /// attribute-token: `'x' attribute ignored` at `loc` (clang 22 names a
    /// scoped attribute `'ns::x'`, clang 19 `'x'`); the argument clause
    /// and `...` are skipped.
    fn ignore_asm_attribute(&mut self, scope: Option<Symbol>, name: Symbol, loc: Loc) {
        if self.eat(Punct::LParen).is_some() {
            self.skip_until(&[Punct::RParen], false, true);
        }
        self.eat(Punct::Ellipsis);
        let syms = self.s.ast.symbols();
        let shown = match scope {
            Some(sc) if self.s.clang_compat >= 22 => {
                format!("{}::{}", syms.as_str(sc), syms.as_str(name))
            }
            _ => syms.as_str(name).to_owned(),
        };
        let d = self.s.diag(DiagId::parse_attribute_ignored, loc).arg(shown);
        self.emit(d);
    }

    /// The name or scope of a `[[...]]` attribute: an identifier or
    /// keyword (clang `TryParseCXX11AttributeIdentifier`). A numeric
    /// constant produced by expanding the predefined `__clang__` macro (the
    /// expansion is spelled `__clang__`) is read as `_Clang`, with "'__clang__'
    /// is a predefined macro name, not an attribute scope specifier"
    /// [-Wignored-attributes] and a fix-it to `_Clang`; it is still the
    /// clang scope (spec/parse.md, attribute scopes; WP-891). The caller
    /// consumes the token.
    fn cxx11_attr_ident(&mut self) -> Option<Symbol> {
        let t = self.tok();
        match t.kind {
            TokKind::Identifier(s) => Some(s),
            TokKind::PpNumber if self.sm.is_macro_loc(t.span.lo) => {
                let at = self.sm.expansion_span(t.span.lo);
                if self.sm.span_text(at) != Some("__clang__") {
                    return None;
                }
                let d = self
                    .s
                    .diag(DiagId::parse_wrong_clang_attr_namespace, t.span.lo)
                    .fixit(kiln_diag::FixIt::replace(at, "_Clang"));
                self.emit(d);
                Some(self.s.intern("_Clang"))
            }
            // An alternative token in an attribute-token is an identifier
            // (`[[and, bitor, compl]]`, [dcl.attr.grammar]/5; clang
            // `TryParseCXX11AttributeIdentifier`, WP-1466).
            TokKind::Punct(_) => {
                let sp = self.spelling_of(&t);
                sp.bytes().all(|b| b.is_ascii_lowercase() || b == b'_').then(|| self.s.intern(&sp))
            }
            _ => None,
        }
    }

    /// The name of a GNU attribute: an identifier or keyword.
    pub(crate) fn attr_name(&self) -> Option<Symbol> {
        match self.tok().kind {
            TokKind::Identifier(s) => Some(s),
            _ => None,
        }
    }

    /// `[[ attribute-list ]]` ([dcl.attr.grammar]).
    fn parse_cxx11_attributes(&mut self, out: &mut Vec<AttrId>) {
        let held = self.s.hold_unknown_attrs();
        self.parse_cxx11_attributes_in(out, false);
        self.s.release_unknown_attrs(held);
    }

    /// [`Self::parse_cxx11_attributes`]; `on_asm`: the specifier
    /// appertains to an asm-declaration, so each attribute is only
    /// named (`'x' attribute ignored`, its arguments skipped unparsed).
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per attribute argument grammar, as clang's ParseCXX11AttributeArgs"
    )]
    fn parse_cxx11_attributes_in(&mut self, out: &mut Vec<AttrId>, on_asm: bool) {
        if self.at_sme_attr() {
            self.parse_sme_attr(out);
            return;
        }
        let before = self.prev_lo();
        let open = self.bump().span.lo;
        self.bump();
        let mut default_scope = None;
        let mut default_scope_loc = Loc::INVALID;
        // An attribute-using-prefix is `using attribute-namespace :`
        // ([dcl.attr.grammar]/1). A `using` followed by what ends or
        // continues an attribute (`,`, `]`, `::`, `(`, `...`) cannot start
        // one, so it is an attribute-token: a keyword there is an
        // identifier ([dcl.attr.grammar]/4), `[[using]]` an unknown
        // attribute. GCC 16.1 agrees; clang 22 reads every `using` as a
        // prefix ("expected identifier"): the text decides.
        if self.at_kw(Kw::Using)
            && !matches!(
                Self::punct_of(self.peek(1)),
                Some(
                    Punct::Comma
                        | Punct::RSquare
                        | Punct::ColonColon
                        | Punct::LParen
                        | Punct::Ellipsis
                )
            )
        {
            // clang `ext_using_attribute_ns` (C++17, P0028).
            if self.std < kiln_lex::Std::Cxx17 {
                let w = self.s.diag(DiagId::parse_ext_using_attribute_ns, self.loc());
                self.emit(w);
            }
            self.bump();
            default_scope = self.cxx11_attr_ident();
            if default_scope.is_some() {
                default_scope_loc = self.bump().span.lo;
            } else {
                self.error_here(DiagId::parse_expected_unqualified_id);
            }
            self.expect(Punct::Colon);
        }
        // An annotation-list ([dcl.attr.grammar]/1, P3394) takes neither
        // a using-prefix nor attributes: `[[=1, nodiscard]]` is ill-formed.
        let mut mix = AnnotMix { attr: default_scope.is_some(), ..AnnotMix::default() };
        let mut no_name = false;
        loop {
            if self.at(Punct::RSquare) || self.at_eof() {
                break;
            }
            if self.eat(Punct::Comma).is_some() {
                continue;
            }
            if self.at(Punct::Equal) {
                self.parse_annotation(out, &mut mix);
                if self.at(Punct::RSquare) || self.at_eof() {
                    break;
                }
                if self.expect(Punct::Comma).is_none() {
                    self.skip_until(&[Punct::RSquare], true, false);
                    break;
                }
                continue;
            }
            mix.attr = true;
            if mix.annot && !mix.reported {
                mix.reported = true;
                self.error_here(DiagId::parse_annotation_mixed);
            }
            // clang: no attribute-token ends the list, at "expected ']'".
            let Some(mut name) = self.cxx11_attr_ident() else {
                no_name = true;
                break;
            };
            let mut loc = self.bump().span.lo;
            let mut scope = None;
            let mut name_loc = loc;
            if self.at(Punct::ColonColon) {
                self.bump();
                let Some(n) = self.cxx11_attr_ident() else {
                    let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("identifier");
                    self.emit(d);
                    self.skip_until(&[Punct::RSquare], true, false);
                    break;
                };
                scope = Some(name);
                name = n;
                name_loc = self.bump().span.lo;
            }
            // [dcl.attr.grammar]/3: under an attribute-using-prefix an
            // attribute-token is not an attribute-scoped-token (clang
            // `err_using_attribute_ns_conflict`, at the scope; the written
            // scope is kept). Otherwise the prefix is the scope and the
            // attribute starts there, as in clang.
            if default_scope.is_some() {
                if scope.is_some() {
                    self.s.error(DiagId::parse_using_attribute_ns_conflict, loc);
                } else {
                    scope = default_scope;
                    loc = default_scope_loc;
                }
            }
            if on_asm {
                // clang 22 reports an attribute of a using-prefix here at
                // its name (clang 19 at the prefix's namespace).
                let at = if default_scope.is_some() && self.s.clang_compat >= 22 {
                    name_loc
                } else {
                    loc
                };
                self.ignore_asm_attribute(scope, name, at);
                if self.at(Punct::RSquare) || self.at_eof() {
                    break;
                }
                if self.expect(Punct::Comma).is_none() {
                    self.skip_until(&[Punct::RSquare], true, false);
                    break;
                }
                continue;
            }
            let text = self.s.ast.symbols().as_str(name).to_owned();
            // clang 19 `Clang<"assume">` is the same statement attribute.
            let clang_assume =
                scope.is_some_and(|s| matches!(self.s.ast.symbols().as_str(s), "clang" | "_Clang"));
            let gsl = scope.is_some_and(|s| self.s.ast.symbols().as_str(s) == "gsl")
                && matches!(text.as_str(), "Owner" | "Pointer");
            let args = if (scope.is_none() || clang_assume) && text == "assume" {
                self.parse_assume_arg(name)
            } else if gsl && self.at(Punct::LParen) {
                // clang `ParseAttributeWithTypeArg`: the attribute starts
                // at its name when it has an argument list.
                loc = name_loc;
                self.parse_gsl_type_arg()
            } else if let Some(clause) = self.at(Punct::LParen).then(|| {
                let sc = scope.map(|s| self.s.ast.symbols().as_str(s));
                std_arg_clause(&text, sc, self.s.is_c(), self.std)
            }) && let Some(clause) = clause
            {
                // clang `ParseCXX11AttributeArgs` (WP-1852): the clause is
                // parsed first (an attribute with no arguments takes
                // expressions); only when that succeeds is its presence, or
                // an empty one, an error, and the attribute dropped.
                let lparen = self.loc();
                let args = match clause {
                    StdArgClause::Forbidden => self.parse_common_attr_args(name, false),
                    StdArgClause::NonEmpty => self.parse_attr_args(name),
                };
                match args {
                    Some(args) if clause == StdArgClause::NonEmpty && !args.is_empty() => {
                        Some(args)
                    }
                    Some(_) => {
                        let id = if clause == StdArgClause::Forbidden {
                            DiagId::parse_attribute_forbids_arguments
                        } else {
                            DiagId::parse_attribute_requires_arguments
                        };
                        let d = self.s.diag(id, lparen).arg(text.as_str());
                        self.emit(d);
                        None
                    }
                    None => None,
                }
            } else if self.at(Punct::LParen)
                && self.s.attribute_is_unknown(AttrSyntax::Cxx11, scope, name)
            {
                // clang `ParseCXX11AttributeArgs` (WP-1836): the argument
                // clause of an attribute clang does not know is balanced
                // tokens, skipped unparsed; only `unknown attribute 'x'
                // ignored` follows ([dcl.attr.grammar]/6).
                self.bump();
                self.skip_until(&[Punct::RParen], false, true);
                Some(Vec::new())
            } else if let Some(rule) = self.table_rule(AttrSyntax::Cxx11, scope, name) {
                self.parse_table_attr_args(AttrSyntax::Cxx11, scope, name, rule, loc)
            } else {
                self.parse_counted_attr_args(AttrSyntax::Cxx11, scope, name, loc)
            };
            // [dcl.attr.grammar]/4: an attribute pack expansion needs an
            // attribute that allows one, and none does (clang reports it
            // at the token after the `...` and keeps the attribute).
            if self.eat(Punct::Ellipsis).is_some() {
                let d = self.s.diag(DiagId::parse_attribute_forbids_ellipsis, self.loc()).arg(text);
                self.emit(d);
            }
            if let Some(args) = args
                && let Some(a) = self.s.act_on_attribute(AttrSyntax::Cxx11, scope, name, &args, loc)
            {
                out.push(a);
            }
            // clang `ParseCXX11AttributeSpecifierInternal`: anything but
            // `]` after an attribute is "expected ','" (after the previous
            // token), then skip to the `]` without consuming it.
            if self.at(Punct::RSquare) || self.at_eof() {
                break;
            }
            if self.expect(Punct::Comma).is_none() {
                self.skip_until(&[Punct::RSquare], true, false);
                break;
            }
        }
        if no_name {
            // clang `ExpectAndConsume(r_square)` twice, each failure
            // followed by `SkipUntil(r_square)` (balanced, consuming).
            for _ in 0..2 {
                if self.expect(Punct::RSquare).is_none() {
                    self.skip_until(&[Punct::RSquare], false, true);
                }
            }
        } else if self.eat(Punct::RSquare).is_some() {
            // clang: a second `]` that is missing is "expected ']'", then
            // `SkipUntil(r_square)` (balanced, consuming).
            if self.expect(Punct::RSquare).is_none() {
                self.skip_until(&[Punct::RSquare], false, true);
            }
        } else {
            self.expect_closing(Punct::RSquare, Punct::LSquare, open);
            self.skip_until(&[Punct::RSquare], false, true);
            self.eat(Punct::RSquare);
        }
        let start = if before == self.cxx11_attr_run.1 { self.cxx11_attr_run.0 } else { before };
        self.cxx11_attr_run = (start, self.prev_lo());
    }

    /// `= constant-expression ...opt`, one annotation of an
    /// annotation-list ([dcl.attr.grammar]/1, [dcl.attr.annotation]; P3394,
    /// spec/annotations.md §2), at the `=`. The operand is a
    /// conditional-expression (a constant-expression), so `[[=a, =b]]` is
    /// two annotations; an `...` makes it a pack expansion
    /// ([dcl.attr.grammar]/6).
    fn parse_annotation(&mut self, out: &mut Vec<AttrId>, mix: &mut AnnotMix) {
        let eq = self.bump().span.lo;
        if !mix.annot && self.std < kiln_lex::Std::Cxx26 {
            let w = self.s.diag(DiagId::parse_ext_annotations, eq);
            self.emit(w);
        }
        mix.annot = true;
        let bad = mix.attr;
        if bad && !mix.reported {
            mix.reported = true;
            let d = self.s.diag(DiagId::parse_annotation_mixed, eq);
            self.emit(d);
        }
        let e = self.parse_constant_expr();
        let ellipsis = self.eat(Punct::Ellipsis);
        if let Some(e) = e
            && !bad
            && let Some(a) = self.s.act_on_annotation(e, ellipsis, eq)
        {
            out.push(a);
        }
    }

    /// `__attribute__(( gnu-attribute-list ))`.
    pub(crate) fn parse_gnu_attributes(&mut self, out: &mut Vec<AttrId>) {
        self.parse_gnu_attributes_in(out, None);
    }

    /// [`Parser::parse_gnu_attributes`] after declarator `decl`, if any.
    fn parse_gnu_attributes_in(&mut self, out: &mut Vec<AttrId>, decl: Option<&Declarator>) {
        self.bump();
        if self.expect(Punct::LParen).is_none() {
            return;
        }
        if self.expect(Punct::LParen).is_none() {
            self.skip_until(&[Punct::RParen], false, true);
            return;
        }
        loop {
            if self.eat(Punct::Comma).is_some() {
                continue;
            }
            // clang `ParseGNUAttributes`: anything but a name ends the
            // list, and the closing `))` below report what follows.
            let Some(name) = self.attr_name() else { break };
            let loc = self.bump().span.lo;
            let args = match decl {
                Some(d) if d.is_function_declarator() && self.sees_params(name) => {
                    let scope = self.s.act_on_start_trailing_requires(d);
                    let args = self.parse_attr_args(name);
                    self.s.act_on_end_trailing_requires(scope);
                    args
                }
                // clang 19 `Clang<"assume">`: `[[assume]]`'s argument.
                _ if self.s.ast.symbols().as_str(name) == "assume" => self.parse_assume_arg(name),
                // WP-1853: whether clang's parser knows the name decides,
                // not kiln's own argument tables.
                _ if self.at(Punct::LParen) && self.s.gnu_attribute_is_unknown_to_parser(name) => {
                    self.parse_common_attr_args(name, true)
                }
                // One clang knows for another target (`long_call`): its
                // arguments are expressions, then `unknown attribute`.
                _ if self.at(Punct::LParen)
                    && !takes_expr_args(self.s.ast.symbols().as_str(name))
                    && self.s.attribute_is_unknown(AttrSyntax::Gnu, None, name) =>
                {
                    self.parse_common_attr_args(name, false)
                }
                _ => match self.table_rule(AttrSyntax::Gnu, None, name) {
                    Some(rule) => {
                        self.parse_table_attr_args(AttrSyntax::Gnu, None, name, rule, loc)
                    }
                    None => self.parse_counted_attr_args(AttrSyntax::Gnu, None, name, loc),
                },
            };
            if let Some(args) = args
                && let Some(a) = self.s.act_on_attribute(AttrSyntax::Gnu, None, name, &args, loc)
            {
                out.push(a);
            }
            if !self.at(Punct::Comma) {
                break;
            }
        }
        // clang `ExpectAndConsume(tok::r_paren)` then `SkipUntil(r_paren,
        // StopAtSemi)`, once per `)`: "expected ')'" after the previous
        // token, without a "to match this '('" note (WP-878); both are
        // reported when both are missing, even at one location.
        for _ in 0..2 {
            if self.eat(Punct::RParen).is_none() {
                let d = self
                    .s
                    .diag(DiagId::parse_expected, self.expected_loc())
                    .arg(crate::parser::quote(Punct::RParen))
                    .allow_duplicate();
                self.emit(d);
                self.skip_until(&[Punct::RParen], true, true);
            }
        }
        self.gnu_attr_end = self.prev_lo();
    }

    /// Whether attribute name `name` is `enable_if` or `diagnose_if` (or
    /// `__enable_if__`, `__diagnose_if__`), whose arguments see the
    /// function's parameters.
    fn sees_params(&self, name: Symbol) -> bool {
        let n = self.s.ast.symbols().as_str(name);
        matches!(kiln_ast::attr::normalize_attr_name(n), "enable_if" | "diagnose_if")
    }

    /// `( type-id opt )` of `gsl::Owner` / `gsl::Pointer`, at the `(`
    /// (clang `ParseAttributeWithTypeArg`): a missing `)` is reported
    /// without skipping, so the attribute list resumes at the next token.
    fn parse_gsl_type_arg(&mut self) -> Option<Vec<AttrArg>> {
        self.bump();
        if self.eat(Punct::RParen).is_some() {
            return Some(Vec::new());
        }
        let Some(t) = self.parse_type_id() else {
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        };
        self.expect(Punct::RParen)?;
        Some(vec![AttrArg::Type(t)])
    }

    /// The arguments of attribute `name`, if any: `Some(args)`, or `None`
    /// after a syntax error.
    pub(crate) fn parse_attr_args(&mut self, name: Symbol) -> Option<Vec<AttrArg>> {
        let Some(open) = self.eat(Punct::LParen) else { return Some(Vec::new()) };
        let text = self.s.ast.symbols().as_str(name).to_owned();
        if !takes_expr_args(&text) {
            self.skip_until(&[Punct::RParen], false, true);
            return Some(Vec::new());
        }
        if takes_type_arg(&text) {
            let Some(t) = self.parse_type_id() else {
                self.skip_until(&[Punct::RParen], false, true);
                return None;
            };
            self.expect_closing(Punct::RParen, Punct::LParen, open);
            return Some(vec![AttrArg::Type(t)]);
        }
        let mut args = Vec::new();
        let mut ok = true;
        // clang 19's identifier-argument table is not per target: its
        // `interrupt` (ARM's, MIPS', RISC-V's) takes one everywhere.
        let ident_first = takes_ident_arg(&text)
            || (self.s.clang_compat < 22
                && kiln_ast::attr::normalize_attr_name(&text) == "interrupt");
        // clang 19 counts the string-literal positions from the first
        // argument after an identifier one, clang 22 from the first.
        let mut skipped = 0;
        if ident_first && let Some(id) = self.ident() {
            let at = self.bump().span.lo;
            args.push(AttrArg::Ident(id, at));
            if self.s.clang_compat < 22 {
                skipped = 1;
            }
            if self.eat(Punct::Comma).is_none() {
                // clang `ExpectAndConsume(tok::r_paren)`: "expected ')'"
                // after the identifier, nothing skipped, the attribute
                // dropped.
                if self.eat(Punct::RParen).is_none() {
                    let d = self
                        .s
                        .diag(DiagId::parse_expected, self.expected_loc())
                        .arg(crate::parser::quote(Punct::RParen))
                        .allow_duplicate();
                    self.emit(d);
                    return None;
                }
                return Some(args);
            }
        }
        let ident_list = takes_ident_list(&text);
        let idents = takes_ident_args(&text);
        while !self.at(Punct::RParen) && !self.at_eof() {
            if let Some(id) = self.ident()
                && (ident_list
                    || idents && (self.peek_is(1, Punct::Comma) || self.peek_is(1, Punct::RParen)))
            {
                let at = self.bump().span.lo;
                args.push(AttrArg::Ident(id, at));
                if self.eat(Punct::Comma).is_none() {
                    break;
                }
                continue;
            }
            if !self.at_string() && string_literal_arg(&text, args.len() - skipped) {
                let at = self.tok().span.lo;
                let d = self.s.diag(DiagId::parse_attribute_requires_string, at).arg(text);
                self.emit(d);
                self.skip_until(&[Punct::RParen], false, true);
                return None;
            }
            // clang parses only a `StringArgument` position as an
            // unevaluated string; `annotate`'s other arguments are
            // expressions (`u8"s"`, `L"w"`; WP-1113).
            let annotate_expr =
                !args.is_empty() && kiln_ast::attr::normalize_attr_name(&text) == "annotate";
            let e = if self.at_string() && !annotate_expr {
                self.parse_string_literal(true)
            } else {
                self.parse_assignment_expr()
            };
            let Some(mut e) = e else {
                ok = false;
                self.skip_until(&[Punct::RParen], false, false);
                break;
            };
            // clang `AcceptsExprPack` (WP-954): `annotate`'s arguments may
            // be pack expansions, expanded at instantiation.
            if kiln_ast::attr::normalize_attr_name(&text) == "annotate"
                && let Some(l) = self.eat(Punct::Ellipsis)
            {
                e = self.s.act_pack_expansion(e, l);
            }
            // clang drops an attribute whose argument is invalid, and so
            // reports nothing more about it (WP-1880); and one whose
            // argument names an unexpanded pack.
            if self.s.is_invalid_attr_arg(e) {
                ok = false;
            }
            if !self.s.check_attribute_arg_packs(e) {
                ok = false;
            }
            args.push(AttrArg::Expr(e));
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        ok.then_some(args)
    }

    /// The arguments of attribute `name` in clang's common form, at the
    /// `(` (clang `ParseAttributeArgsCommon` with no argument grammar of
    /// its own: an `__attribute__` clang does not know, WP-1836, or a
    /// standard attribute that takes no arguments, WP-1852). With
    /// `lone_ident` (an unknown attribute) an identifier alone before `,`
    /// or `)` is an identifier argument; the others are
    /// assignment-expressions or braced lists (a string literal an
    /// ordinary one, except where clang's per-name table wants one,
    /// [`string_literal_arg`]), each perhaps a pack expansion. `None` when
    /// one is invalid: clang then drops the attribute without its
    /// `unknown attribute` warning.
    fn parse_common_attr_args(&mut self, name: Symbol, lone_ident: bool) -> Option<Vec<AttrArg>> {
        self.parse_common_attr_args_as(name, lone_ident, None)
    }

    /// [`Parser::parse_common_attr_args`], or with `rule` the clause of an
    /// attribute of the argument table (spec/parse.md §6.2 "Argument
    /// table"): a leading identifier is an identifier argument when the
    /// rule says so, its string-literal positions and whether it takes a
    /// pack expansion are the rule's.
    fn parse_common_attr_args_as(
        &mut self,
        name: Symbol,
        lone_ident: bool,
        rule: Option<kiln_sema::attr_args::AttrArgRule>,
    ) -> Option<Vec<AttrArg>> {
        self.bump();
        let text = self.s.ast.symbols().as_str(name).to_owned();
        let string_at =
            |i: usize| rule.map_or_else(|| string_literal_arg(&text, i), |r| r.string_at(i));
        let mut args = Vec::new();
        let mut more = !self.at(Punct::RParen);
        if let Some(id) = self.ident()
            && (rule.is_some_and(|r| r.ident)
                || lone_ident && (self.peek_is(1, Punct::Comma) || self.peek_is(1, Punct::RParen)))
        {
            let at = self.bump().span.lo;
            args.push(AttrArg::Ident(id, at));
            more = self.eat(Punct::Comma).is_some();
        }
        // clang 22 counts the string-literal positions from the first
        // argument, clang 19 from the first after the identifier.
        let skipped = if self.s.clang_compat >= 22 { 0 } else { args.len() };
        let mut pack = false;
        while more {
            if !self.at_string() && string_at(args.len() - skipped) {
                let d = self
                    .s
                    .diag(DiagId::parse_attribute_requires_string, self.loc())
                    .arg(text.as_str());
                self.emit(d);
                self.skip_until(&[Punct::RParen], true, true);
                return None;
            }
            let e = if self.at_string() && string_at(args.len() - skipped) {
                self.parse_string_literal(true)
            } else if self.at(Punct::LBrace) {
                self.parse_init_list().ok().map(|l| self.s.act_on_braced_arg(l))
            } else {
                self.parse_assignment_expr()
            };
            let expansion = self.eat(Punct::Ellipsis).is_some();
            pack |= expansion && !rule.is_some_and(|r| r.pack);
            let Some(e) = e.filter(|&e| {
                !self.s.is_invalid_attr_arg(e) && (expansion || self.s.check_attribute_arg_packs(e))
            }) else {
                self.skip_until(&[Punct::RParen], true, true);
                return None;
            };
            args.push(AttrArg::Expr(e));
            more = self.eat(Punct::Comma).is_some();
        }
        // No such attribute takes a pack expansion (clang
        // `attributeAcceptsExprPack`), reported at the `)`.
        if pack {
            let d = self
                .s
                .diag(DiagId::parse_attribute_pack_not_supported, self.loc())
                .arg(text.as_str());
            self.emit(d);
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        }
        // clang `ExpectAndConsume(tok::r_paren)`: "expected ')'" after the
        // previous token, no note, nothing skipped; the attribute is
        // dropped.
        if self.eat(Punct::RParen).is_none() {
            let d = self
                .s
                .diag(DiagId::parse_expected, self.expected_loc())
                .arg(crate::parser::quote(Punct::RParen))
                .allow_duplicate();
            self.emit(d);
            return None;
        }
        Some(args)
    }

    /// The arguments of an attribute of the argument table (spec/parse.md
    /// §6.2 "Argument table", WP-1879) whose arguments kiln does not
    /// interpret: the clause, if any, is parsed as clang's
    /// `ParseAttributeArgsCommon` does (an unevaluated operand here: the
    /// arguments are not kept), then the count is checked. `None`, the
    /// attribute dropped, after an error in either; otherwise no
    /// arguments, as before.
    fn parse_table_attr_args(
        &mut self,
        syntax: AttrSyntax,
        scope: Option<Symbol>,
        name: Symbol,
        rule: kiln_sema::attr_args::AttrArgRule,
        loc: Loc,
    ) -> Option<Vec<AttrArg>> {
        let count = if self.at(Punct::LParen) {
            self.s.enter_unevaluated();
            let args = self.parse_common_attr_args_as(name, false, Some(rule));
            self.s.exit_unevaluated();
            args?.len()
        } else {
            0
        };
        self.s.check_attr_arg_count(syntax, scope, name, rule, count, loc).then(Vec::new)
    }

    /// [`Parser::parse_attr_args`], then, for an attribute whose handler
    /// does not count its arguments as clang does ([`counted_at_parse`]),
    /// the argument count of the table's rule (WP-1879).
    fn parse_counted_attr_args(
        &mut self,
        syntax: AttrSyntax,
        scope: Option<Symbol>,
        name: Symbol,
        loc: Loc,
    ) -> Option<Vec<AttrArg>> {
        let args = self.parse_attr_args(name)?;
        if counted_at_parse(self.s.ast.symbols().as_str(name))
            && let Some(rule) = self.s.attr_arg_rule(syntax, scope, name)
            && !self.s.check_attr_arg_count(syntax, scope, name, rule, args.len(), loc)
        {
            return None;
        }
        Some(args)
    }

    /// The [`kiln_sema::attr_args::AttrArgRule`] of `scope::name` when the
    /// parser would otherwise skip its arguments ([`takes_expr_args`]).
    fn table_rule(
        &self,
        syntax: AttrSyntax,
        scope: Option<Symbol>,
        name: Symbol,
    ) -> Option<kiln_sema::attr_args::AttrArgRule> {
        if takes_expr_args(self.s.ast.symbols().as_str(name)) {
            return None;
        }
        self.s.attr_arg_rule(syntax, scope, name)
    }

    /// The argument of `[[assume(conditional-expression)]]`
    /// ([dcl.attr.assume]/1; clang's `ParseCXXAssumeAttributeArg`). An
    /// expression that is not a conditional-expression (a comma or an
    /// assignment at the top) is diagnosed at its operator and the
    /// attribute is dropped (`None`). The expression is converted to
    /// `bool` by `Sema::act_on_attributed_stmt` once the statement is known.
    fn parse_assume_arg(&mut self, name: Symbol) -> Option<Vec<AttrArg>> {
        let Some(open) = self.eat(Punct::LParen) else { return Some(Vec::new()) };
        let Some(c) = self.parse_cast_expr().and_then(|l| self.parse_rhs_from(l, prec::COND))
        else {
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        };
        if !self.at(Punct::RParen) && !self.at(Punct::RSquare) {
            // An invalid expression is its own error alone (WP-1880).
            if let Some(full) = self.parse_rhs_from(c, prec::COMMA)
                && !self.s.contains_errors(full)
            {
                let loc = self.s.expr_loc(full);
                let text = self.s.ast.symbols().as_str(name).to_owned();
                let d = self.s.diag(DiagId::parse_assume_attr_expects_cond_expr, loc).arg(text);
                self.emit(d);
            }
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        }
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(vec![AttrArg::Expr(c)])
    }

    /// `alignas ( type-id )` / `alignas ( constant-expression )` ([dcl.align]).
    fn parse_alignas(&mut self, out: &mut Vec<AttrId>) {
        self.c_keyword_extension();
        let loc = self.bump().span.lo;
        let Some(open) = self.expect(Punct::LParen) else { return };
        let begin = self.tok().span.lo;
        // clang `ParseAlignArgument`: `isTypeIdInParens`, the same test as
        // `sizeof(...)`, so a class template's injected-class-name
        // (`alignas(V)` inside `V<T>`, [temp.local]/1) is a type-id.
        let arg = if self.is_cxx_type_id_at(self.pos(), TypeIdContext::InParens).0 {
            self.parse_type_id().map(AttrArg::Type)
        } else {
            self.parse_constant_expr().map(AttrArg::Expr)
        };
        // `alignas(Ts...)` ([dcl.align]/1, [temp.variadic]/5).
        let ellipsis = self.eat(Punct::Ellipsis);
        let arg = arg.map(|a| self.s.act_on_alignas_operand(a, ellipsis, begin));
        if let Some(None) = arg {
            self.expect_closing(Punct::RParen, Punct::LParen, open);
            return;
        }
        let Some(arg) = arg.flatten() else {
            self.skip_until(&[Punct::RParen], false, true);
            return;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        let name = self.s.intern("alignas");
        if let Some(a) = self.s.act_on_attribute(AttrSyntax::Keyword, None, name, &[arg], loc) {
            out.push(a);
        }
    }
}
