//! GNU `alias("target")` and `ifunc("resolver")` definitions (clang
//! `EmitAliasDefinition`, `emitIFuncDefinition`, `checkAliases`;
//! spec/codegen.md "Aliases and ifuncs").
//!
//! A namespace-scope function or variable whose declaration carries one of
//! the attributes is emitted, at that declaration, as an IR alias (or
//! ifunc) of the symbol the attribute names. The symbol is a mangled name:
//! an existing global of that name, a deferred definition that mangles to
//! it (emitted now), or else a declaration created under that name for a
//! later definition to complete. A declaration of the aliasing entity
//! already in the module (an earlier use) is replaced by the alias, which
//! takes its name and uses. At the end of the module, once no error has
//! been reported, every alias is checked: its chain must end at a
//! definition (no cycle), an ifunc's resolver must be a function
//! returning a pointer, and an alias of a weak alias is pointed at that
//! alias's aliasee with a warning.
use kiln_abi::mangle;
use kiln_ast::attr::AttrKind;
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::{AttrId, StringLitId};
use kiln_ast::{DeclId, ids::RawId as _};
use kiln_diag::{Diag, DiagId, FixIt, Severity};
use kiln_llvm::{AliasId, Attr, AttrKind as LAttr, FuncId, Linkage, TypeId, Value, ValueRef};
use kiln_source::fx::{FxHashMap, FxHashSet};
use kiln_source::{Loc, Span};

use crate::module::{Cg, R, err};

/// The alias bookkeeping of a module.
#[derive(Default)]
pub(crate) struct AliasState {
    /// First declarations whose alias or ifunc was handled.
    done: FxHashSet<DeclId>,
    /// The aliases and ifuncs emitted, in order, with their first
    /// declarations (clang `Aliases`).
    emitted: Vec<(DeclId, AliasId)>,
    /// Symbols declared by name as an aliasee or resolver: a later
    /// definition of that symbol completes the declaration rather than
    /// being deferred or renamed.
    pub(crate) targets: FxHashSet<String>,
    /// Functions declared by name as an aliasee, with the alias's first
    /// declaration: their parameter and return attributes stay that
    /// declaration's when the aliasee is defined (clang
    /// `GetOrCreateLLVMFunction` sets them once, from the alias's
    /// `GlobalDecl`; an `extern "C"` alias gives no `noundef` return).
    pub(crate) declared_for: FxHashMap<FuncId, DeclId>,
}

impl Cg<'_> {
    /// The `alias` / `ifunc` attribute that makes `first` a definition:
    /// `(is_ifunc, target, attribute location)`.
    fn alias_attr(&self, first: DeclId) -> Option<(bool, StringLitId, Loc)> {
        let ast = self.ast;
        ast.redecls_of(first).find_map(|r| {
            ast.lists
                .iter::<AttrId>(ast.attrs.of_decl(r))
                .map(|a| *ast.attrs.get(a))
                .find(|a| matches!(a.kind, AttrKind::ALIAS | AttrKind::IFUNC))
                .and_then(|a| {
                    let l = StringLitId::from_raw(a.a)?;
                    Some((a.kind == AttrKind::IFUNC, l, a.loc))
                })
        })
    }

    /// One past the `)` of `first`'s `alias` / `ifunc` attribute (recorded
    /// by Sema in the node's `b`).
    fn alias_attr_end(&self, first: DeclId) -> Option<Loc> {
        let ast = self.ast;
        ast.redecls_of(first).find_map(|r| {
            ast.lists
                .iter::<AttrId>(ast.attrs.of_decl(r))
                .map(|a| *ast.attrs.get(a))
                .find(|a| matches!(a.kind, AttrKind::ALIAS | AttrKind::IFUNC) && a.b != 0)
                .map(|a| Loc::from_raw(a.b))
        })
    }

    /// clang `EmitGlobal` for a declaration `d` with `alias` / `ifunc`
    /// (not a `weakref`): true when `d` is one, emitted here.
    pub(crate) fn emit_alias_definition(&mut self, d: DeclId) -> R<bool> {
        let ast = self.ast;
        if ast.alias_definition(d).is_none() {
            return Ok(false);
        }
        let kind = ast.decls.get(d).kind();
        if !matches!(kind, DeclKind::Function | DeclKind::CXXMethod | DeclKind::Var) {
            return Ok(false);
        }
        let first = ast.first(d);
        if !self.alias.done.insert(first) {
            return Ok(true);
        }
        let Some((ifunc, lit, at)) = self.alias_attr(first) else { return Ok(true) };
        let target = String::from_utf8_lossy(ast.strings.bytes(lit)).into_owned();
        let name = mangle::mangle_decl(ast, &mut self.anon, first);
        if name == target {
            self.alias_error(DiagId::cg_cyclic_alias, at, ifunc);
            return Ok(true);
        }
        let entry = self.m.lookup(&name).map(|v| self.m.resolve(v));
        let defined = entry.is_some_and(|v| match v.get() {
            ValueRef::Func(f) => !self.m.func(f).is_declaration(),
            ValueRef::Global(g) => self.m.global(g).initializer().is_some(),
            _ => true,
        });
        if defined {
            // clang: a definition in the module wins over an alias; an
            // ifunc is a duplicate definition.
            if ifunc && !self.dup_reported {
                let loc = ast.decls.get(d).loc();
                let mut diag =
                    Diag::new(DiagId::cg_duplicate_mangled_name, Span::empty(loc)).arg(&name);
                if let Some(&rep) = self.reps.get(&name) {
                    let prev = ast.definition(rep).unwrap_or(rep);
                    diag = diag.note(
                        DiagId::note_previous_definition,
                        Span::empty(ast.decls.get(prev).loc()),
                    );
                }
                self.diags.push(diag);
                self.dup_reported = true;
            }
            return Ok(true);
        }
        let a = if kind == DeclKind::Var {
            let g = self.get_gvar(first)?;
            if self.m.resolve(Value::global(g)) != Value::global(g) {
                return Ok(true);
            }
            let vt = self.m.global(g).value_type();
            let aliasee = self.aliasee(&target, first, false, vt)?;
            let Ok(a) = self.m.replace_global_with_alias(g, vt, aliasee) else {
                self.alias_error(DiagId::cg_cyclic_alias, at, false);
                return Ok(true);
            };
            let tls = self.m.global(g).thread_local;
            self.m.alias_mut(a).thread_local = tls;
            a
        } else {
            let f = self.get_fn(first)?;
            if self.m.resolve(Value::func(f)) != Value::func(f) {
                return Ok(true);
            }
            let fty = self.m.func(f).ty();
            let aliasee = self.aliasee(&target, first, ifunc, fty)?;
            let r = if ifunc {
                self.m.replace_function_with_ifunc(f, fty, aliasee)
            } else {
                self.m.replace_function_with_alias(f, fty, aliasee)
            };
            let Ok(a) = r else {
                self.alias_error(DiagId::cg_cyclic_alias, at, ifunc);
                return Ok(true);
            };
            a
        };
        // clang `EmitAliasDefinition`: the entity's definition linkage,
        // weak for `weak`; `emitIFuncDefinition`: external.
        let linkage = if ifunc {
            Linkage::External
        } else if mangle::has_internal_linkage(ast, first) {
            Linkage::Internal
        } else if ast.is_weak_decl(first) {
            Linkage::Weak
        } else {
            Linkage::External
        };
        let vis = self.decl_visibility(first, true);
        let al = self.m.alias_mut(a);
        al.linkage = linkage;
        al.visibility = vis;
        // A definition (clang `SetCommonAttributes` -> `setDSOLocal`);
        // the relocation model settles it ([`Cg::finish_dso_local`]).
        al.dso_local = linkage != Linkage::Internal;
        self.alias.emitted.push((first, a));
        Ok(true)
    }

    /// The aliasee (resolver) named `target` for the alias (ifunc) of
    /// `first` with value type `vt` (clang `GetOrCreateLLVMFunction` /
    /// `GetOrCreateLLVMGlobal` by name).
    fn aliasee(&mut self, target: &str, first: DeclId, ifunc: bool, vt: TypeId) -> R<Value> {
        if let Some(v) = self.m.lookup(target) {
            return Ok(v);
        }
        if let Some(def) = self.deferred_named(target) {
            return Ok(if self.ast.decls.get(def).kind() == DeclKind::Var {
                Value::global(self.get_gvar(def)?)
            } else {
                Value::func(self.get_fn(def)?)
            });
        }
        self.alias.targets.insert(target.to_owned());
        let loc = Some(self.ast.decls.get(first).loc());
        if ifunc {
            // clang 22 `emitIFuncDefinition`: an incomplete `void ()`
            // placeholder without attributes, replaced (moved to the end
            // of the module) by the resolver's definition
            // ([`Cg::fn_for_definition`]).
            let rty = self.m.types.function(TypeId::VOID, &[], false);
            return match self.m.add_function(target, rty) {
                Ok(f) => Ok(Value::func(f)),
                Err(e) => err(loc, format!("cannot declare {target}: {e:?}")),
            };
        }
        if self.ast.decls.get(first).kind() == DeclKind::Var {
            return Ok(Value::global(self.m.add_global(target, vt)));
        }
        let sig = self.decl_sig(first)?;
        let f = self.declare_fn(target, first, &sig)?;
        self.alias.declared_for.insert(f, first);
        Ok(Value::func(f))
    }

    /// A deferred definition whose symbol is `target` (clang
    /// `GetOrCreateLLVMFunction` moving it from `DeferredDecls`).
    fn deferred_named(&mut self, target: &str) -> Option<DeclId> {
        let ast = self.ast;
        let mut cands: Vec<DeclId> = self
            .deferred
            .keys()
            .copied()
            .filter(|&f| {
                mangle::asm_label(ast, f).is_some()
                    || ast
                        .decls
                        .get(f)
                        .name()
                        .as_ident()
                        .is_some_and(|n| target.contains(ast.symbols().as_str(n)))
            })
            .collect();
        cands.sort_unstable();
        cands.into_iter().find(|&f| mangle::mangle_decl(ast, &mut self.anon, f) == target)
    }

    /// A deferred definition `first` was just put off: one whose symbol an
    /// alias named is emitted now.
    pub(crate) fn alias_target_needed(&mut self, first: DeclId) -> R<()> {
        if self.alias.targets.is_empty() {
            return Ok(());
        }
        let name = mangle::mangle_decl(self.ast, &mut self.anon, first);
        if !self.alias.targets.contains(&name) {
            return Ok(());
        }
        if self.ast.decls.get(first).kind() == DeclKind::Var {
            self.get_gvar(first)?;
        } else {
            self.get_fn(first)?;
        }
        Ok(())
    }

    fn alias_error(&mut self, id: DiagId, at: Loc, ifunc: bool) {
        self.diags.push(Diag::new(id, Span::empty(at)).arg(u32::from(ifunc)));
    }

    /// clang `checkAliases`, after everything is emitted and only when no
    /// error was reported: the alias chains, the ifunc resolvers, the
    /// sections, and aliases of weak aliases. On an error every alias is
    /// dropped by the caller (the compilation fails).
    pub(crate) fn check_aliases(&mut self) {
        if self.alias.emitted.is_empty()
            || self
                .diags
                .iter()
                .any(|d| d.id().is_some_and(|i| i.default_severity() == Severity::Error))
        {
            return;
        }
        for (first, a) in self.alias.emitted.clone() {
            let Some((ifunc, _, at)) = self.alias_attr(first) else { continue };
            let Some(gv) = self.aliased_global(a) else {
                self.alias_error(DiagId::cg_cyclic_alias, at, ifunc);
                continue;
            };
            let declaration = match gv.get() {
                ValueRef::Func(f) => self.m.func(f).is_declaration(),
                ValueRef::Global(g) => self.m.global(g).initializer().is_none(),
                _ => true,
            };
            if declaration {
                self.alias_undefined(a, first, at, ifunc);
                continue;
            }
            if ifunc {
                let ValueRef::Func(f) = gv.get() else {
                    self.alias_error(DiagId::cg_alias_to_undefined, at, true);
                    continue;
                };
                let ret = self.m.func(f).ret_type();
                if !self.m.types.is_ptr(ret) {
                    self.diags.push(Diag::new(DiagId::cg_ifunc_resolver_return, Span::empty(at)));
                    continue;
                }
            }
            let aliasee = self.m.resolve(self.m.alias(a).aliasee());
            self.check_alias_section(first, aliasee, ifunc);
            if let ValueRef::Alias(wa) = aliasee.get()
                && matches!(
                    self.m.alias(wa).linkage,
                    Linkage::Weak | Linkage::LinkOnce | Linkage::ExternWeak | Linkage::Common
                )
            {
                let shown = self.global_name(gv);
                let weak = self.m.alias(wa).name().to_owned();
                self.diags.push(
                    Diag::new(DiagId::cg_alias_to_weak_alias, Span::empty(at))
                        .arg(shown)
                        .arg(weak)
                        .arg(u32::from(ifunc)),
                );
                let to = self.m.alias(wa).aliasee();
                let _ = self.m.set_aliasee(a, to);
            }
            if ifunc && let ValueRef::Func(f) = self.m.resolve(self.m.alias(a).aliasee()).get() {
                let set = self.m.func(f).attrs.func;
                let mut v = self.m.attrs_of(set).to_vec();
                if !v.contains(&Attr::Flag(LAttr::DisableSanitizerInstrumentation)) {
                    v.push(Attr::Flag(LAttr::DisableSanitizerInstrumentation));
                    self.m.func_mut(f).attrs.func = self.m.attr_set(&v);
                }
            }
        }
    }

    /// clang `getAliasedGlobal`: the global at the end of alias `a`'s
    /// chain (an ifunc's resolver), or `None` on a cycle.
    fn aliased_global(&self, a: AliasId) -> Option<Value> {
        let mut seen = FxHashSet::default();
        let mut cur = a;
        loop {
            if !seen.insert(cur) {
                return None;
            }
            let v = self.m.resolve(self.m.alias(cur).aliasee());
            match v.get() {
                ValueRef::Alias(n) => cur = n,
                _ => return Some(v),
            }
        }
    }

    fn global_name(&self, v: Value) -> String {
        match v.get() {
            ValueRef::Func(f) => self.m.func(f).name().to_owned(),
            ValueRef::Global(g) => self.m.global(g).name().to_owned(),
            ValueRef::Alias(a) => self.m.alias(a).name().to_owned(),
            _ => String::new(),
        }
    }

    /// clang `checkAliasedGlobal` for an undefined aliasee: the error, the
    /// mangled-name note, and a note naming each emitted declaration whose
    /// identifier is the target, with its symbol.
    fn alias_undefined(&mut self, a: AliasId, first: DeclId, at: Loc, ifunc: bool) {
        let ast = self.ast;
        let range_end = self.alias_attr_end(first).unwrap_or(at);
        let target = self.global_name(self.m.resolve(self.m.alias(a).aliasee()));
        let mut diag = Diag::new(DiagId::cg_alias_to_undefined, Span::empty(at))
            .arg(u32::from(ifunc))
            .with_note(
                Diag::new(DiagId::note_alias_requires_mangled_name, Span::empty(at))
                    .arg(u32::from(ifunc)),
            );
        let mut decls: Vec<DeclId> =
            self.fns.keys().chain(self.gvars.keys()).chain(self.deferred.keys()).copied().collect();
        decls.sort_unstable();
        decls.dedup();
        let mut shown = FxHashSet::default();
        for d in decls {
            let same = ast
                .decls
                .get(d)
                .name()
                .as_ident()
                .is_some_and(|n| ast.symbols().as_str(n) == target);
            if !same {
                continue;
            }
            let name = mangle::mangle_decl(ast, &mut self.anon, d);
            if shown.insert(name.clone()) {
                let kw = if ifunc { "ifunc" } else { "alias" };
                let fix = FixIt::replace(Span::new(at, range_end), format!("{kw}(\"{name}\")"));
                diag = diag.with_note(
                    Diag::new(DiagId::note_alias_mangled_name_alternative, Span::empty(at))
                        .arg(name)
                        .fixit(fix),
                );
            }
        }
        self.diags.push(diag);
    }

    /// clang `checkAliases`: an alias in a section other than its
    /// aliasee's is `-Wignored-attributes`.
    fn check_alias_section(&mut self, first: DeclId, aliasee: Value, ifunc: bool) {
        let ast = self.ast;
        let Some((lit, at)) = ast.redecls_of(first).find_map(|r| {
            ast.lists
                .iter::<AttrId>(ast.attrs.of_decl(r))
                .map(|a| *ast.attrs.get(a))
                .find(|a| a.kind == AttrKind::SECTION)
                .and_then(|a| Some((StringLitId::from_raw(a.a)?, a.loc)))
        }) else {
            return;
        };
        let sec = String::from_utf8_lossy(ast.strings.bytes(lit)).into_owned();
        let theirs = match aliasee.get() {
            ValueRef::Func(f) => self.m.func(f).section.as_deref(),
            ValueRef::Global(g) => self.m.global(g).section.as_deref(),
            _ => None,
        };
        if theirs != Some(sec.as_str()) {
            self.diags.push(
                Diag::new(DiagId::cg_alias_with_section, Span::empty(at))
                    .arg(sec)
                    .arg(u32::from(ifunc)),
            );
        }
    }
}
