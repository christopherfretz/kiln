//! An unqualified template-id whose name lookup finds nothing or only
//! functions ([temp.names]/3) and that is not called: the template-id
//! names nothing (spec/sema.md "Undeclared template-ids", WP-372,
//! WP-391). clang 19 reports it through delayed typo correction; clang 22
//! typo-corrects a name that lookup found functions for to the same name
//! qualified by its namespace (`val<int> + 1` with `int val(int)`: "use of
//! undeclared identifier 'val'; did you mean '::val'?").

use kiln_ast::{DeclId, ExprId};
use kiln_ast::decl::DeclKind;
use kiln_ast::names::DeclName;
use kiln_ast::types::TemplateArg;
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::decl::sym_str;
use crate::lookup::LookupKind;

/// The keywords clang's typo correction offers in an expression
/// (`AddKeywordsToConsumer` with every `Want*` flag, C++ mode, regardless
/// of the scope conditions some of them have).
const EXPR_KEYWORDS: &[&str] = &[
    "char", "const", "double", "enum", "float", "int", "long", "short", "signed", "struct",
    "union", "unsigned", "void", "volatile", "_Complex", "_Imaginary", "extern", "inline",
    "static", "typedef", "restrict", "bool", "class", "typename", "wchar_t", "char16_t",
    "char32_t", "constexpr", "decltype", "thread_local", "typeof", "const_cast",
    "dynamic_cast", "reinterpret_cast", "static_cast", "sizeof", "false", "true", "delete",
    "new", "operator", "throw", "typeid", "this", "alignof", "nullptr", "static_assert", "do",
    "else", "for", "goto", "if", "return", "switch", "while", "catch", "try", "break",
    "continue", "case", "default", "namespace", "template", "explicit", "friend", "mutable",
    "private", "protected", "public", "virtual", "using",
];

impl Sema<'_, '_> {
    /// clang 19's undeclared template-id `name<...>` at `loc` (not
    /// called, not a nested-name-specifier): "use of undeclared
    /// identifier" through delayed typo correction, on an erroneous
    /// expression the enclosing expression goes on parsing around
    /// (clang 19's `TypoExpr`; [`Sema::correct_delayed_typos_in`] finds it,
    /// so a call missing its `)` skips to it quietly).
    pub fn act_on_undeclared_template_id_typo(&mut self, name: Symbol, loc: Loc) -> ExprId {
        let text = sym_str(self, name);
        let msg = self.diag(DiagId::sema_undeclared_var_use, loc).arg(text);
        let e = self.recover_err(&[], loc);
        self.emit_delayed_typo(msg, e);
        e
    }

    /// clang 22's typo correction of the template-id `name<args>` at
    /// `loc` whose ordinary lookup found only namespace-scope functions
    /// (assumed a template, [temp.names]/3) and that is not called: the
    /// correction is the same name qualified from the enclosing namespace
    /// (`::val`, `::N::fn`) when it is the least distant candidate. Emits
    /// "use of undeclared identifier 'val'; did you mean '::val'?" with
    /// "'::val' declared here" and returns the overload set of the first
    /// function declared (no template arguments; an erroneous expression
    /// when an argument is dependent). `None`, diagnosing nothing, when
    /// the functions are not one namespace's, or a keyword or another
    /// visible name is as close (clang suggests that one; kiln does not
    /// yet) -- the caller reports the undeclared name.
    pub fn correct_function_template_id(
        &mut self,
        name: Symbol,
        args: &[TemplateArg],
        loc: Loc,
    ) -> Option<ExprId> {
        let found = self.lookup_name(name, LookupKind::Ordinary);
        if found.decls.is_empty() || found.decls.len() != found.found.len() {
            return None;
        }
        // One namespace's functions, found directly or as one
        // using-declarator's single target (clang finds nothing to
        // correct to beside a using-declarator, or across namespaces).
        let shadows = found.found.iter().filter(|&&f| self.kind_of(f) == DeclKind::UsingShadow);
        let shadows = shadows.count();
        if shadows > 0 && found.found.len() > 1 {
            return None;
        }
        let mut ns = None;
        for &f in &found.found {
            if !matches!(self.kind_of(self.ast.strip_shadow(f)), DeclKind::Function) {
                return None;
            }
            let n = self.ast.first(self.namespace_of(f)?);
            if ns.is_some_and(|x| x != n) {
                return None;
            }
            ns = Some(n);
        }
        let ns = ns?;
        // The first function declared, as the correction's declaration.
        let i = (0..found.decls.len()).min_by_key(|&i| self.ast.first(found.decls[i]))?;
        let (decl, shown) = (found.decls[i], found.found[i]);
        let cur = self.enclosing_namespace().map(|n| self.named_ns_chain(n)).unwrap_or_default();
        let (qual, q) = self.correction_qualifier(ns, &cur);
        let distance = (110 * q + 50) / 100;
        let len = self.ast.symbols().as_str(name).len();
        if len < 3 && distance > len {
            return None;
        }
        if self.closer_typo_candidate(name, distance) {
            return None;
        }
        let text = sym_str(self, name);
        let corrected = format!("'{qual}{text}'");
        let note = self.diag(DiagId::note_entity_declared_here, self.decl_loc(shown)).arg(&corrected);
        let msg = self
            .diag(DiagId::sema_undeclared_var_use_suggest, loc)
            .arg(text)
            .arg(format!("{qual}{}", sym_str(self, name)))
            .with_note(note);
        self.emit(msg);
        if args.iter().any(|&a| Self::dependent_flags(self.ast.types.arg_flags(a))) {
            return Some(self.recover_err(&[], loc));
        }
        // clang's recovery: an overload set of the one declaration, which
        // a built-in operator resolves to it
        // ([`Sema::resolve_single_overload_candidate`]).
        Some(self.builder().unresolved_lookup(DeclName::ident(name), &[decl], false, None, None, loc))
    }

    /// The namespace (or translation unit) declaration `d` belongs to,
    /// through linkage specifications and export declarations; `None` at
    /// block or class scope.
    fn namespace_of(&self, d: DeclId) -> Option<DeclId> {
        let mut c = self.ast.decls.get(d).ctx()?;
        for _ in 0..10_000 {
            match self.kind_of(c) {
                DeclKind::Namespace | DeclKind::TranslationUnit => return Some(c),
                DeclKind::LinkageSpec | DeclKind::Export => c = self.ast.decls.get(c).ctx()?,
                _ => return None,
            }
        }
        None
    }

    /// Whether clang's typo correction has a candidate for `name` other
    /// than `name` itself at a normalized distance of at most `distance`:
    /// an expression keyword, or an identifier ordinary lookup finds,
    /// within `(len+2)/3` byte edits with `len/edits >= 3`
    /// (`CorrectTypo`'s limits; clang walks its identifier table, kiln its
    /// interner).
    fn closer_typo_candidate(&self, name: Symbol, distance: usize) -> bool {
        let typo = sym_str(self, name);
        let len = typo.len();
        let max = len.div_ceil(3);
        let near = |s: &str| {
            if s.len().abs_diff(len) > max || s == typo {
                return false;
            }
            let ed = crate::local_friend::edit_distance(&typo, s);
            ed <= max && ed <= distance && ed > 0 && len / ed >= 3
        };
        if EXPR_KEYWORDS.iter().any(|k| near(k)) {
            return true;
        }
        let syms = self.ast.symbols();
        let n = u32::try_from(syms.len()).unwrap_or(u32::MAX);
        (0..n).map(Symbol::from_index).any(|s| {
            s != name
                && near(syms.as_str(s))
                && !self.lookup_name(s, LookupKind::Ordinary).decls.is_empty()
        })
    }
}


