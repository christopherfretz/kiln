//! The pragmas that name a function or variable by its identifier:
//! `#pragma weak name`, `#pragma weak alias = target` and `#pragma
//! redefine_extname old new` (spec/sema.md §4.5 "Pragmas naming
//! symbols"; clang `ActOnPragmaWeakID`, `ActOnPragmaWeakAlias`,
//! `DeclApplyPragmaWeak`, `DeclClonePragmaWeak`, `ProcessPragmaWeak`,
//! `ActOnPragmaRedefineExtname`).
//!
//! Each pragma looks its identifier up at namespace scope of the
//! translation unit. A function or variable found takes the effect now;
//! otherwise the pragma waits for the first `extern "C"` (C: any)
//! function or variable declared with that name ([`WeakPending`],
//! [`ExtnamePending`]). The weak alias form declares a new function or
//! variable `alias` with the target's type at namespace scope, with an
//! implicit `weak` attribute and an `alias` attribute naming the target
//! by its identifier (the assembly-level name: a C++ function's mangled
//! name is not found, so codegen reports the alias as undefined, as clang
//! does). `redefine_extname` gives an `extern "C"` declaration the asm
//! label `new` (the `__asm__("new")` machinery, [dcl.asm] GNU).

use kiln_ast::DeclId;
use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::builder::DeclHead;
use kiln_ast::decl::{DeclFlags, DeclKind, FnSpec};
use kiln_ast::ids::{AttrId, RawId as _};
use kiln_ast::names::DeclName;
use kiln_ast::strings::StringEncoding;
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::lookup::LookupKind;

/// A `#pragma weak` whose identifier was not declared when it was read
/// (clang `WeakUndeclaredIdentifiers`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub(crate) struct WeakPending {
    /// The identifier waited for: `name` of `#pragma weak name`, `target`
    /// of `#pragma weak alias = target`.
    pub(crate) name: Symbol,
    /// The pragma's first identifier (clang's `WeakInfo` location).
    pub(crate) loc: Loc,
    /// `alias` of the alias form.
    pub(crate) alias: Option<Symbol>,
}

/// A `#pragma redefine_extname old new` whose `old` was not declared
/// when it was read (clang `ExtnameUndeclaredIdentifiers`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub(crate) struct ExtnamePending {
    /// `old`.
    pub(crate) old: Symbol,
    /// `new`.
    pub(crate) new: Symbol,
    /// The `new` token.
    pub(crate) loc: Loc,
}

impl Sema<'_, '_> {
    /// The function or variable `name` declares at namespace scope of
    /// the translation unit, or `Err(found)` with whether lookup found
    /// anything else (clang `LookupSingleName(TUScope, ...)`).
    fn tu_fn_or_var(&self, name: Symbol) -> Result<DeclId, bool> {
        let found = self.lookup_in(self.ast.tu, name, LookupKind::Ordinary);
        match found.decls.first().copied() {
            Some(d) if self.kind_of(d) == DeclKind::Var || self.kind_of(d).is_function() => Ok(d),
            Some(_) => Err(true),
            None => Err(false),
        }
    }

    /// `#pragma weak alias = target` (clang `ActOnPragmaWeakAlias`):
    /// `alias` and its token, `target` and its token. A function or
    /// variable `target` without an `alias` attribute gets its weak alias
    /// now; anything else waits for an `extern "C"` declaration of
    /// `target` ([`Sema::apply_pragma_weak`]).
    pub fn act_on_pragma_weak_alias(&mut self, alias: (Symbol, Loc), target: (Symbol, Loc)) {
        match self.tu_fn_or_var(target.0) {
            Ok(d) => {
                if self.find_attr(d, AttrKind::ALIAS).is_none() {
                    self.declare_pragma_weak_alias(d, alias.0, alias.1);
                }
            }
            Err(_) => {
                self.weak_pending.push(WeakPending {
                    name: target.0,
                    loc: alias.1,
                    alias: Some(alias.0),
                });
            }
        }
    }

    /// clang `DeclClonePragmaWeak`: the weak alias `alias` of function or
    /// variable `target`, declared at `loc` at namespace scope of the
    /// translation unit with `target`'s type (and so its language
    /// linkage is C++'s in C++; clang's "FIXME: Mangling?").
    fn declare_pragma_weak_alias(&mut self, target: DeclId, alias: Symbol, loc: Loc) {
        let Some(ty) = self.ast.decls.get(target).ty() else { return };
        let Some(tname) = self.ast.decls.get(target).name().as_ident() else { return };
        let tu = self.ast.tu;
        let h = DeclHead::new(tu, DeclName::ident(alias), loc);
        let d = if self.kind_of(target) == DeclKind::Var {
            let spec = self.ast.decls.var(target).map(|v| v.spec).unwrap_or_default();
            self.builder().var(h, ty, spec, None)
        } else {
            let f = self.builder().function(h, DeclKind::Function, ty, FnSpec::new(), None);
            let params = self.ast.types.proto(ty).map(|p| self.ast.types.params(p).to_vec());
            let mut pds = Vec::new();
            for (i, pty) in params.unwrap_or_default().into_iter().enumerate() {
                let idx = u16::try_from(i).unwrap_or(u16::MAX);
                let ph = DeclHead::new(f, DeclName::EMPTY, loc).flags(DeclFlags::IMPLICIT);
                pds.push(self.builder().param_head(ph, pty, idx, 0));
            }
            if !pds.is_empty() {
                let _ = self.builder().set_params(f, &pds);
            }
            f
        };
        let bytes = self.ast.symbols().as_str(tname).as_bytes().to_vec();
        let lit = self.ast.strings.intern(StringEncoding::Ordinary, &bytes);
        let implicit = AttrNode::IMPLICIT;
        let node =
            |kind, a| AttrNode { kind, syntax: AttrSyntax::Pragma, flags: implicit, loc, a, b: 0 };
        let attrs = [
            self.ast.attrs.push(node(AttrKind::ALIAS, lit.raw())),
            self.ast.attrs.push(node(AttrKind::WEAK, 0)),
        ];
        let _ = self.builder().attach_attrs(d, &attrs);
    }

    /// clang `ProcessPragmaWeak`: an `extern "C"` function or variable
    /// `decl` whose name earlier `#pragma weak` pragmas named while it
    /// was undeclared takes an implicit `weak` attribute (`#pragma weak
    /// name`) and gets the weak aliases (`#pragma weak alias = name`)
    /// (spec/sema.md §4.5).
    pub(crate) fn apply_pragma_weak(&mut self, decl: DeclId, attrs: &mut Vec<AttrId>) {
        if self.weak_pending.is_empty() {
            return;
        }
        let n = self.ast.decls.get(decl);
        if !(n.kind() == DeclKind::Var || n.kind().is_function()) {
            return;
        }
        let Some(name) = n.name().as_ident() else { return };
        if !self.weak_pending.iter().any(|w| w.name == name)
            || !kiln_abi::mangle::is_extern_c(&self.ast, decl)
        {
            return;
        }
        let mut taken = Vec::new();
        self.weak_pending.retain(|w| {
            let t = w.name == name;
            if t {
                taken.push(*w);
            }
            !t
        });
        let mut weak = false;
        for w in taken {
            match w.alias {
                Some(alias) => self.declare_pragma_weak_alias(decl, alias, w.loc),
                None if !weak => {
                    weak = true;
                    attrs.push(self.ast.attrs.push(crate::visibility::weak_pragma_attr(w.loc)));
                }
                None => {}
            }
        }
    }

    /// The end of the translation unit (clang `ActOnEndOfTranslationUnit`):
    /// each `#pragma weak` still waiting warns that its identifier was
    /// never declared, or, when the identifier names something other than
    /// a function or variable, that `weak` does not apply to it. The
    /// pragmas are grouped by identifier, in the order each identifier
    /// was first named.
    /// The pending list is kept: a precompiled header's image check
    /// (`pch_uncarried_state`) reads it after the end of the unit.
    pub(crate) fn diagnose_pending_weak(&mut self) {
        let pending = self.weak_pending.clone();
        let mut names: Vec<Symbol> = Vec::new();
        for w in &pending {
            if !names.contains(&w.name) {
                names.push(w.name);
            }
        }
        for name in names {
            let other = matches!(self.tu_fn_or_var(name), Err(true));
            let text = self.ast.symbols().as_str(name).to_owned();
            let mut aliases: Vec<Option<Symbol>> = Vec::new();
            for w in pending.iter().filter(|w| w.name == name) {
                // clang's set of `WeakInfo`s keeps one per alias name.
                if aliases.contains(&w.alias) {
                    continue;
                }
                aliases.push(w.alias);
                let d = if other {
                    self.diag(DiagId::sema_attribute_wrong_decl_type, w.loc)
                        .arg("'weak'")
                        .arg("variables and functions")
                } else {
                    self.diag(DiagId::sema_weak_identifier_undeclared, w.loc).arg(&text)
                };
                self.emit(d);
            }
        }
    }

    /// `#pragma redefine_extname old new` (clang
    /// `ActOnPragmaRedefineExtname`): `old` and `new` with their tokens.
    /// A function or variable `old` with C language linkage now gets the
    /// asm label `new`; one with C++ linkage is warned about and keeps
    /// its name. Otherwise the pragma waits for the first `extern "C"`
    /// declaration of `old` ([`Sema::apply_pragma_extname`]); a second
    /// pragma for the same undeclared `old` is ignored (clang's map
    /// insert).
    pub fn act_on_pragma_redefine_extname(&mut self, old: (Symbol, Loc), new: (Symbol, Loc)) {
        match self.tu_fn_or_var(old.0) {
            Ok(d) => {
                if kiln_abi::mangle::is_extern_c(&self.ast, d) {
                    self.give_pragma_asm_label(d, new.0, new.1);
                } else {
                    self.warn_extname_not_applied(d);
                }
            }
            Err(_) => {
                if !self.extname_pending.iter().any(|e| e.old == old.0) {
                    self.extname_pending.push(ExtnamePending {
                        old: old.0,
                        new: new.0,
                        loc: new.1,
                    });
                }
            }
        }
    }

    /// clang `warn_redefine_extname_not_applied` at `d`.
    fn warn_extname_not_applied(&mut self, d: DeclId) {
        let n = self.ast.decls.get(d);
        let what = if n.kind() == DeclKind::Var { "1" } else { "0" };
        let loc = n.loc();
        let name = n.name().as_ident().map(|s| self.ast.symbols().as_str(s).to_owned());
        let msg = self.diag(DiagId::sema_redefine_extname_not_applied, loc).arg(what);
        self.emit(msg.arg(name.unwrap_or_default()));
    }

    /// The asm label `new` the pragma at `loc` gives the existing
    /// declaration `d`: none when `d` already has one (clang adds a second
    /// label, and the first wins), or is already used or defined (clang's
    /// code generator has named the symbol by then, inline definitions
    /// included; GCC renames every use).
    fn give_pragma_asm_label(&mut self, d: DeclId, new: Symbol, loc: Loc) {
        let first = self.ast.first(d);
        let named = |r: DeclId| {
            self.ast.decls.get(r).flags().contains(DeclFlags::USED)
                || self.ast.decls.function(r).is_some_and(|f| f.body.is_some())
                || (self.ast.decls.var(r).is_some() && self.is_var_definition(r))
        };
        if self.ast.decls.cold.asm_label.contains(first) || self.ast.redecls_of(d).any(named) {
            return;
        }
        let a = self.pragma_asm_label(first, new, loc);
        self.builder().append_attrs(d, &[a]);
    }

    /// An implicit asm label `new` for the function or variable whose
    /// first declaration is `first`, recorded for mangling.
    fn pragma_asm_label(&mut self, first: DeclId, new: Symbol, loc: Loc) -> AttrId {
        let bytes = self.ast.symbols().as_str(new).as_bytes().to_vec();
        let lit = self.ast.strings.intern(StringEncoding::Ordinary, &bytes);
        let _ = self.ast.decls.cold.asm_label.insert(first, lit);
        self.ast.attrs.push(AttrNode {
            kind: AttrKind::ASM_LABEL,
            syntax: AttrSyntax::Pragma,
            flags: AttrNode::IMPLICIT,
            loc,
            a: lit.raw(),
            b: 0,
        })
    }

    /// clang `ActOn{Function,Variable}Declarator`: the first declaration
    /// `decl` of a function or variable, written without an asm label,
    /// whose name a waiting `#pragma redefine_extname` names takes the
    /// pragma's label if it has C language linkage; otherwise it is
    /// warned about and the pragma keeps waiting.
    pub(crate) fn apply_pragma_extname(&mut self, decl: DeclId, attrs: &mut Vec<AttrId>) {
        if self.extname_pending.is_empty() {
            return;
        }
        let Some(name) = self.ast.decls.get(decl).name().as_ident() else { return };
        let Some(i) = self.extname_pending.iter().position(|e| e.old == name) else { return };
        if !kiln_abi::mangle::is_extern_c(&self.ast, decl) {
            self.warn_extname_not_applied(decl);
            return;
        }
        let e = self.extname_pending.remove(i);
        let first = self.ast.first(decl);
        attrs.insert(0, self.pragma_asm_label(first, e.new, e.loc));
    }
}
