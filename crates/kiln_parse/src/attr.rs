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
    ) || kiln_ast::attr::CC_ATTR_SPELLINGS.contains(&n)
        || class_b_takes_expr_args(n)
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
        // optional message and `annotate`'s annotation.
        "target" | "target_version" | "unavailable" | "annotate" => i == 0,
        "target_clones" | "no_sanitize" => true,
        _ => false,
    }
}

/// True for the attributes whose arguments may be identifiers (clang
/// `attributeHasVariadicIdentifierArg`: `callback`; and
/// `attributeHasIdentifierArg`: the string `EnumArgument`s of
/// `zero_call_used_regs` and `function_return`, which sema rejects as
/// "requires a string").
fn takes_ident_args(name: &str) -> bool {
    matches!(
        kiln_ast::attr::normalize_attr_name(name),
        "callback" | "zero_call_used_regs" | "function_return"
    )
}

/// True for the attributes whose argument is a type-id (clang
/// `attributeIsTypeArgAttr`).
fn takes_type_arg(name: &str) -> bool {
    kiln_ast::attr::normalize_attr_name(name) == "preferred_name"
}

/// True for the attributes whose first argument is an identifier when
/// one is written (clang `attributeHasIdentifierArg`).
fn takes_ident_arg(name: &str) -> bool {
    let n = kiln_ast::attr::normalize_attr_name(name);
    // `weakref` / `alias` / `ifunc` take a string literal; clang parses
    // an identifier there as one (and then rejects it).
    matches!(
        n,
        "enum_extensibility"
            | "format"
            | "mode"
            | "weakref"
            | "alias"
            | "ifunc"
            | "__clang_arm_builtin_alias"
            | "clang_builtin_alias"
            | "builtin_alias"
    )
}

/// True for the standard attributes that take no argument list
/// (clang `IsBuiltInOrStandardCXX11Attribute` with no arguments).
fn forbids_args(name: &str) -> bool {
    matches!(
        name,
        "carries_dependency"
            | "fallthrough"
            | "noreturn"
            | "no_unique_address"
            | "likely"
            | "unlikely"
            | "maybe_unused"
    )
}

impl Parser<'_, '_> {
    /// True at `[[`.
    pub(crate) fn at_cxx11_attr(&self) -> bool {
        self.at(Punct::LSquare) && self.peek_is(1, Punct::LSquare)
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
            } else if self.kw_of(t) == Some(Kw::Alignas) {
                i = self.skip_balanced(i + 1)?;
            } else {
                return Some(i);
            }
        }
    }

    /// True at GNU `__attribute__((...))` specifiers followed by `;`: an
    /// attributed null statement, `__attribute__((fallthrough));` (clang
    /// `ParseStatementOrDeclarationAfterAttributes`, `kw___attribute`; a
    /// `[[` after them starts an expression there, so it is not taken).
    /// Likewise before `return` or `{`, which cannot start a declaration
    /// (`__attribute__((musttail)) return f(x);`, WP-809).
    pub(crate) fn at_gnu_attr_null_stmt(&self) -> bool {
        let mut i = self.pos();
        while self.kw_of(self.tok_at(i)) == Some(Kw::Attribute) {
            let Some(j) = self.skip_balanced(i + 1) else { return false };
            i = j;
        }
        let t = self.tok_at(i);
        i != self.pos()
            && (matches!(Self::punct_of(t), Some(Punct::Semi | Punct::LBrace))
                || self.kw_of(t) == Some(Kw::Return))
    }

    /// A C++11 attribute-specifier-seq at namespace scope before a
    /// declaration that takes none ([dcl.pre]/5, [dcl.attr.grammar]/5):
    /// a namespace-definition or namespace-alias-definition, a
    /// static_assert-declaration, a template-declaration, explicit
    /// instantiation or specialization, a linkage-specification, an
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
            Some(Kw::Namespace | Kw::StaticAssert | Kw::Template | Kw::Asm | Kw::Export) => true,
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
        loop {
            if self.at_cxx11_attr() {
                self.parse_cxx11_attributes(out);
            } else if self.at_kw(Kw::Alignas) {
                self.parse_alignas(out);
            } else if self.at_kw(Kw::Attribute) {
                self.parse_gnu_attributes(out);
            } else if self.at_kw(Kw::Declspec) {
                let loc = self.bump().span.lo;
                self.unsupported("'__declspec'", loc);
                if let Some(l) = self.eat(Punct::LParen) {
                    self.skip_until(&[Punct::RParen], false, true);
                    let _ = l;
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
            _ => None,
        }
    }

    /// The name of a GNU attribute: an identifier or keyword.
    fn attr_name(&self) -> Option<Symbol> {
        match self.tok().kind {
            TokKind::Identifier(s) => Some(s),
            _ => None,
        }
    }

    /// `[[ attribute-list ]]` ([dcl.attr.grammar]).
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per attribute argument grammar, as clang's ParseCXX11AttributeArgs"
    )]
    fn parse_cxx11_attributes(&mut self, out: &mut Vec<AttrId>) {
        let before = self.prev_lo();
        let open = self.bump().span.lo;
        self.bump();
        let mut default_scope = None;
        if self.at_kw(Kw::Using) {
            self.bump();
            default_scope = self.cxx11_attr_ident();
            if default_scope.is_some() {
                self.bump();
            } else {
                self.error_here(DiagId::parse_expected_unqualified_id);
            }
            self.expect(Punct::Colon);
        }
        // An annotation-list ([dcl.attr.grammar]/1, P3394) takes neither
        // a using-prefix nor attributes: `[[=1, nodiscard]]` is ill-formed.
        let mut mix = AnnotMix { attr: default_scope.is_some(), ..AnnotMix::default() };
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
            let Some(mut name) = self.cxx11_attr_ident() else {
                self.error_here(DiagId::parse_expected_unqualified_id);
                self.skip_until(&[Punct::RSquare], false, false);
                break;
            };
            let mut loc = self.bump().span.lo;
            let mut scope = default_scope;
            let mut name_loc = loc;
            if self.at(Punct::ColonColon) {
                self.bump();
                let Some(n) = self.cxx11_attr_ident() else {
                    self.error_here(DiagId::parse_expected_unqualified_id);
                    self.skip_until(&[Punct::RSquare], false, false);
                    break;
                };
                scope = Some(name);
                name = n;
                name_loc = self.bump().span.lo;
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
            } else if scope.is_none()
                && self.at(Punct::LParen)
                && ((forbids_args(&text)
                    // P3475R2: no longer a standard attribute in C++26.
                    && !(text == "carries_dependency" && self.std >= kiln_lex::Std::Cxx26))
                    || (text == "indeterminate" && self.std >= kiln_lex::Std::Cxx26))
            {
                // clang `ParseCXX11AttributeArgs`: a standard attribute
                // without arguments takes no argument list, even `()`.
                let d =
                    self.s.diag(DiagId::parse_attribute_forbids_arguments, self.loc()).arg(text);
                self.emit(d);
                self.parse_attr_args(name);
                None
            } else {
                self.parse_attr_args(name)
            };
            if let Some(l) = self.eat(Punct::Ellipsis) {
                self.unsupported("attribute pack expansions", l);
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
        if self.eat(Punct::RSquare).is_some() {
            self.expect(Punct::RSquare);
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
                _ => self.parse_attr_args(name),
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
    fn parse_attr_args(&mut self, name: Symbol) -> Option<Vec<AttrArg>> {
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
        if takes_ident_arg(&text)
            && let Some(id) = self.ident()
        {
            let at = self.bump().span.lo;
            args.push(AttrArg::Ident(id, at));
            if self.eat(Punct::Comma).is_none() {
                self.expect_closing(Punct::RParen, Punct::LParen, open);
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
            if !self.at_string() && string_literal_arg(&text, args.len()) {
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
            // clang drops an attribute whose argument is invalid; kiln
            // does so for the string-literal attributes above.
            if string_literal_arg(&text, 0) && self.s.contains_errors(e) {
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
            if let Some(full) = self.parse_rhs_from(c, prec::COMMA) {
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
