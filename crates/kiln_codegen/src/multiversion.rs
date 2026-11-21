//! x86-64 function multiversioning (WP-1745; clang `CodeGenModule`
//! `GetOrCreateMultiVersionResolver`, `EmitMultiVersionFunctionDefinition`,
//! `emitMultiVersionFunctions`, `emitCPUDispatchDefinition` and
//! `CodeGenFunction::EmitX86MultiVersionResolver`).
//!
//! Each version of a multiversioned function is a function of its own,
//! named with a suffix of its version ([`Cg::mv_version_name`]); every
//! call and address of the function goes to an `ifunc` whose resolver
//! picks a version at load time from `__cpu_model` / `__cpu_features2`
//! after `__cpu_indicator_init`. spec/codegen.md "Function
//! multiversioning".

use kiln_abi::{mangle, x86_fmv};
use kiln_ast::DeclId;
use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::multiversion::MvKind;
use kiln_llvm::{
    AliasId, Attr, AttrKind as LAttr, AttrList, BinOp, CallOpts, ComdatKind, FuncId, GepFlags,
    GlobalId, IntPredicate, IrBuilder, Linkage, TypeId as LTy, Value, ValueRef,
};
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::module::{Cg, Def, R, err};

/// The version [`Cg::target_attrs_of`] describes while one is declared
/// or emitted: its `target("...")`-syntax string (`None`: the module's
/// attributes) and its tune CPU.
pub(crate) struct MvCur {
    /// The function's first declaration.
    pub first: DeclId,
    /// The version's target string.
    pub target: Option<String>,
    /// The version's `tune-cpu` (a `cpu_specific` CPU).
    pub tune: Option<String>,
    /// A clone's `arch=` CPU gives its features only: the `target-cpu`
    /// and `tune-cpu` stay the module's.
    pub keep_cpu: bool,
}

/// Multiversioning state of a module.
#[derive(Default)]
pub(crate) struct MvState {
    /// Base symbol → the function its calls and addresses use: the
    /// declaration an `ifunc` replaced, or a `cpu_specific` function's
    /// `.ifunc` declaration until its `cpu_dispatch` is emitted.
    refs: FxHashMap<String, FuncId>,
    /// Functions whose resolver is emitted at the end of the module
    /// (clang `MultiVersionFuncs`), by first declaration.
    queue: Vec<DeclId>,
    /// Base symbols queued.
    queued: FxHashSet<String>,
    /// Base symbols whose `cpu_dispatch` resolver was emitted.
    dispatched: FxHashSet<String>,
    /// `cpu_dispatch` declarations emitted.
    dispatch_decls: FxHashSet<DeclId>,
    /// `cpu_specific` functions referenced (clang named their first
    /// version, `Manglings`), by first declaration.
    named: FxHashSet<DeclId>,
    /// The version being declared or emitted.
    pub(crate) cur: Option<MvCur>,
}

/// One option of a resolver (clang `FMVResolverOption`).
struct MvOption {
    /// The version.
    f: FuncId,
    /// Its `arch=` CPU.
    arch: Option<String>,
    /// Its features (for `cpu_dispatch`, of its CPU).
    features: Vec<String>,
    /// The `__builtin_cpu_supports` mask of its features.
    mask: [u32; 4],
}

/// The fields of a `target("...")` string clang's multiversioning reads
/// (`ParsedTargetAttr`): its `arch=` CPU and its added features.
fn parse_target(text: &str) -> (Option<String>, Vec<String>) {
    let mut arch = None;
    let mut features = Vec::new();
    for item in text.split(',').map(str::trim) {
        if let Some(a) = item.strip_prefix("arch=") {
            arch = Some(a.to_owned());
        } else if !item.is_empty()
            && !item.starts_with("no-")
            && !item.starts_with("tune=")
            && !item.starts_with("fpmath=")
        {
            features.push(item.to_owned());
        }
    }
    (arch, features)
}

/// clang `AppendTargetMangling`: `arch_<cpu>` then the features by
/// descending priority, joined by `_`.
fn target_mangling(text: &str) -> String {
    let (arch, mut features) = parse_target(text);
    features.sort_by_key(|f| std::cmp::Reverse(x86_fmv::fmv_priority(f)));
    arch.map(|a| format!("arch_{a}")).into_iter().chain(features).collect::<Vec<_>>().join("_")
}

/// clang `TargetClonesAttr::isFirstOfVersion`.
fn first_of_version(list: &[String], i: usize) -> bool {
    !list[..i].contains(&list[i])
}

/// clang `TargetClonesAttr::getMangledIndex`.
fn clone_mangled_index(list: &[String], i: usize) -> usize {
    let n = |s: &[String]| s.iter().filter(|x| *x != "default").count();
    if list[i] == "default" { n(list) } else { n(&list[..i]) }
}

/// The `arch=` CPU and features of clone string `s`.
fn clone_option(s: &str) -> (Option<String>, Vec<String>) {
    if let Some(a) = s.strip_prefix("arch=") {
        (Some(a.to_owned()), Vec::new())
    } else if s == "default" {
        (None, Vec::new())
    } else {
        (None, vec![s.to_owned()])
    }
}

/// One test of a resolver's condition: `__builtin_cpu_supports` words
/// (address, mask) or a `__builtin_cpu_is` field (address, value).
enum Step {
    Supports(Vec<(Value, u32)>),
    Is(Value, u32),
}

impl Cg<'_> {
    /// Whether `d`'s entity is an x86-64 multiversioned function.
    pub(crate) fn is_x86_multiversion(&self, d: DeclId) -> bool {
        self.is_x86() && self.ast.is_multiversion(d)
    }

    /// The multiversioning attribute of `d`'s entity, of its kind.
    fn mv_node(&self, d: DeclId) -> Option<(MvKind, AttrNode)> {
        let kind = self.ast.mv_kind(d);
        let k = match kind {
            MvKind::None => return None,
            MvKind::Target => AttrKind::TARGET,
            MvKind::Clones => AttrKind::TARGET_CLONES,
            MvKind::CpuSpecific => AttrKind::CPU_SPECIFIC,
            MvKind::CpuDispatch => AttrKind::CPU_DISPATCH,
        };
        Some((kind, self.ast.mv_attr(d, k)?))
    }

    /// The clone strings or CPU names of `d`'s `target_clones` /
    /// `cpu_*` attribute.
    fn mv_list(&self, d: DeclId) -> Vec<String> {
        match self.mv_node(d) {
            Some((MvKind::Target | MvKind::None, _)) | None => Vec::new(),
            Some((_, n)) => self.ast.mv_attr_list(&n),
        }
    }

    /// The symbol of `d`'s entity without its version suffix.
    fn mv_base(&mut self, d: DeclId) -> String {
        let first = self.ast.first(d);
        mangle::mangle_decl(self.ast, &mut self.anon, first)
    }

    /// The symbol of version `idx` of `d`'s entity (clang
    /// `getMangledNameImpl`): `target("default")` the plain name, a
    /// `target` version `.arch_<cpu>_<features>`, a clone
    /// `.<string>.<index>`, a `cpu_specific` CPU `.<mangling letter>`.
    pub(crate) fn mv_version_name(&mut self, d: DeclId, idx: usize) -> String {
        let base = self.mv_base(d);
        match self.mv_node(d) {
            Some((MvKind::Target, n)) => {
                let text = self.ast.target_attr_text(&n);
                if text == "default" { base } else { format!("{base}.{}", target_mangling(&text)) }
            }
            Some((MvKind::Clones, n)) => {
                let list = self.ast.mv_attr_list(&n);
                let Some(s) = list.get(idx) else { return base };
                let s2 = s.strip_prefix("arch=").map_or_else(|| s.clone(), |a| format!("arch_{a}"));
                format!("{base}.{s2}.{}", clone_mangled_index(&list, idx))
            }
            Some((MvKind::CpuSpecific | MvKind::CpuDispatch, n)) => {
                let list = self.ast.mv_attr_list(&n);
                let c =
                    list.get(idx).and_then(|c| x86_fmv::processor(c)).map_or('\0', |p| p.mangling);
                format!("{base}.{c}")
            }
            _ => base,
        }
    }

    /// The target string and tune CPU of version `idx` of `d`'s entity
    /// (clang `GetCPUAndFeaturesAttributes`): a clone's string, a
    /// `cpu_specific` CPU's features over the module's with the CPU as
    /// the tune CPU; `None` for a default version.
    fn mv_version_target(&self, d: DeclId, idx: usize) -> MvCur {
        let first = self.ast.first(d);
        let keep_cpu = self.ast.mv_kind(d) == MvKind::Clones;
        let (target, tune) = match self.mv_node(d) {
            Some((MvKind::Target, n)) => {
                let t = self.ast.target_attr_text(&n);
                ((t != "default").then_some(t), None)
            }
            Some((MvKind::Clones, n)) => {
                let s = self.ast.mv_attr_list(&n).get(idx).cloned();
                (s.filter(|s| s != "default"), None)
            }
            Some((MvKind::CpuSpecific, n)) => {
                let cpu = self.ast.mv_attr_list(&n).get(idx).cloned();
                let p = cpu.as_deref().and_then(x86_fmv::processor);
                (p.map(|p| p.features.to_owned()), cpu)
            }
            _ => (None, None),
        };
        MvCur { first, target, tune, keep_cpu }
    }

    /// The function a call or address of multiversioned `first` uses
    /// (clang `GetOrCreateMultiVersionResolver`): the `ifunc` (`.ifunc`;
    /// the plain name for `target_clones`) of resolver `.resolver`; for
    /// `cpu_specific` a declaration `.ifunc` its `cpu_dispatch` replaces.
    /// A `target` / `target_clones` resolver is emitted at the end.
    pub(crate) fn mv_ref(&mut self, first: DeclId) -> R<FuncId> {
        let kind = self.ast.mv_kind(first);
        let base = self.mv_base(first);
        if kind == MvKind::CpuSpecific {
            self.mv.named.insert(first);
        }
        if let Some(&f) = self.mv.refs.get(&base) {
            return Ok(f);
        }
        let sig = self.decl_sig(first)?;
        if matches!(kind, MvKind::Target | MvKind::Clones) && self.mv.queued.insert(base.clone()) {
            self.mv.queue.push(first);
        }
        let name = if kind == MvKind::Clones { base.clone() } else { format!("{base}.ifunc") };
        let f = if kind == MvKind::CpuSpecific {
            match self.m.add_function(&name, sig.fn_ty) {
                Ok(f) => f,
                Err(e) => return err(None, format!("cannot declare {name}: {e:?}")),
            }
        } else {
            let resolver = self.mv_resolver_fn(&base)?;
            self.mv_ifunc(first, &name, sig.fn_ty, resolver)?.0
        };
        self.mv.refs.insert(base, f);
        Ok(f)
    }

    /// The resolver `<base>.resolver` (`ptr ()`), declared if new.
    fn mv_resolver_fn(&mut self, base: &str) -> R<FuncId> {
        let name = format!("{base}.resolver");
        if let Some(f) = self.m.get_function(&name) {
            return Ok(f);
        }
        let ty = self.m.types.function(LTy::PTR, &[], false);
        match self.m.add_function(&name, ty) {
            Ok(f) => Ok(f),
            Err(e) => err(None, format!("cannot declare {name}: {e:?}")),
        }
    }

    /// clang `getMultiversionLinkage`: internal for an internal function,
    /// else `weak_odr`.
    fn mv_linkage(&self, first: DeclId) -> Linkage {
        if mangle::has_internal_linkage(self.ast, first) {
            Linkage::Internal
        } else {
            Linkage::WeakOdr
        }
    }

    /// Sets the linkage, visibility and `dso_local` of the `ifunc` or
    /// alias `a` of `first` (clang `SetCommonAttributes`).
    fn mv_common(&mut self, first: DeclId, a: AliasId) {
        let linkage = self.mv_linkage(first);
        let vis = self.decl_visibility(first, true);
        let al = self.m.alias_mut(a);
        al.linkage = linkage;
        al.visibility = vis;
        al.dso_local = linkage != Linkage::Internal;
    }

    /// An `ifunc` `name` of type `fty` and resolver `resolver` for
    /// `first`, through a declaration it replaces (returned, with it).
    fn mv_ifunc(
        &mut self,
        first: DeclId,
        name: &str,
        fty: LTy,
        resolver: FuncId,
    ) -> R<(FuncId, AliasId)> {
        let f = match self.m.add_function(name, fty) {
            Ok(f) => f,
            Err(e) => return err(None, format!("cannot declare {name}: {e:?}")),
        };
        let a = self.mv_replace(first, f, fty, resolver)?;
        Ok((f, a))
    }

    /// Replaces declaration `f` of `first` by an `ifunc` of `resolver`.
    fn mv_replace(&mut self, first: DeclId, f: FuncId, fty: LTy, resolver: FuncId) -> R<AliasId> {
        let a = match self.m.replace_function_with_ifunc(f, fty, Value::func(resolver)) {
            Ok(a) => a,
            Err(e) => return err(None, format!("cannot create an ifunc: {e:?}")),
        };
        self.mv_common(first, a);
        Ok(a)
    }

    /// clang `EmitGlobal` for a multiversioned function's declaration
    /// `d`: a `cpu_dispatch` emits its resolver; a definition that must be
    /// emitted is (`EmitMultiVersionFunctionDefinition`); another waits
    /// for a resolver to need it, or is emitted at the end when a version
    /// of it was already declared (clang `DeferredDecls`).
    pub(crate) fn emit_mv_global(&mut self, d: DeclId) -> R<()> {
        let ast = self.ast;
        if ast.mv_kind(d) == MvKind::CpuDispatch {
            return self.emit_cpu_dispatch(d);
        }
        let Some(fd) = ast.decls.function(d) else { return Ok(()) };
        if fd.body.is_none() || fd.spec.is_immediate() {
            return Ok(());
        }
        if !self.discardable_fn(d) || self.has_fn_attr(d, AttrKind::USED) {
            return self.emit_mv_def(d);
        }
        let n = match ast.mv_kind(d) {
            MvKind::Target => 1,
            _ => self.mv_list(d).len(),
        };
        for i in 0..n {
            let name = self.mv_version_name(d, i);
            if self.m.get_function(&name).is_some_and(|f| self.m.func(f).is_declaration()) {
                self.to_emit.push(Def::Decl(d));
                break;
            }
        }
        Ok(())
    }

    /// clang `EmitMultiVersionFunctionDefinition` for the definition `d`:
    /// every CPU of a `cpu_specific`, every distinct clone (then the
    /// resolver), or the one `target` version.
    pub(crate) fn emit_mv_def(&mut self, d: DeclId) -> R<()> {
        let first = self.ast.first(d);
        match self.ast.mv_kind(first) {
            MvKind::CpuDispatch => self.emit_cpu_dispatch(d),
            MvKind::CpuSpecific => {
                for i in 0..self.mv_list(d).len() {
                    self.emit_mv_version(d, i)?;
                }
                Ok(())
            }
            MvKind::Clones => {
                let list = self.mv_list(d);
                for i in 0..list.len() {
                    if first_of_version(&list, i) {
                        self.emit_mv_version(d, i)?;
                    }
                }
                self.mv_ref(first).map(|_| ())
            }
            _ => self.emit_mv_version(d, 0),
        }
    }

    /// Emits version `idx` of the definition `d` unless it was.
    fn emit_mv_version(&mut self, d: DeclId, idx: usize) -> R<()> {
        let name = self.mv_version_name(d, idx);
        let first = self.ast.first(d);
        let cur = self.mv_version_target(d, idx);
        let saved = self.mv.cur.replace(cur);
        let r = self.emit_mv_version_in(d, first, &name);
        self.mv.cur = saved;
        r
    }

    fn emit_mv_version_in(&mut self, d: DeclId, first: DeclId, name: &str) -> R<()> {
        let f = match self.m.get_function(name) {
            Some(f) if !self.m.func(f).is_declaration() => return Ok(()),
            Some(f) => f,
            None => {
                let sig = self.decl_sig(first)?;
                self.declare_fn(name, first, &sig)?
            }
        };
        self.gen_code(d, f, None)
    }

    /// clang `createFunction` of `emitMultiVersionFunctions`: version
    /// `idx` of `v`, emitted if `v` is defined, else declared.
    fn mv_version_fn(&mut self, v: DeclId, idx: usize) -> R<FuncId> {
        let name = self.mv_version_name(v, idx);
        if let Some(f) = self.m.get_function(&name) {
            return Ok(f);
        }
        if let Some(def) = self.ast.definition(v) {
            self.emit_mv_version(def, idx)?;
            if let Some(f) = self.m.get_function(&name) {
                return Ok(f);
            }
        }
        let first = self.ast.first(v);
        let cur = self.mv_version_target(v, idx);
        self.mv_declare(first, &name, cur)
    }

    /// Declares version `name` of `first` with the attributes of `cur`.
    fn mv_declare(&mut self, first: DeclId, name: &str, cur: MvCur) -> R<FuncId> {
        let saved = self.mv.cur.replace(cur);
        let r = self.decl_sig(first).and_then(|sig| self.declare_fn(name, first, &sig));
        self.mv.cur = saved;
        r
    }

    /// clang `emitMultiVersionFunctions`: the resolvers of the queued
    /// `target` / `target_clones` functions, then what their versions
    /// deferred, until none is left.
    pub(crate) fn emit_mv_resolvers(&mut self) -> R<()> {
        loop {
            let batch = std::mem::take(&mut self.mv.queue);
            if batch.is_empty() {
                return Ok(());
            }
            for first in batch {
                self.emit_mv_resolver(first)?;
            }
            self.emit_deferred()?;
        }
    }

    /// The resolver of `target` / `target_clones` function `first`.
    fn emit_mv_resolver(&mut self, first: DeclId) -> R<()> {
        let mut opts: Vec<MvOption> = Vec::new();
        for v in self.ast.mv_versions(first) {
            match self.mv_node(v) {
                Some((MvKind::Target, n)) => {
                    let text = self.ast.target_attr_text(&n);
                    let f = self.mv_version_fn(v, 0)?;
                    let (arch, features) =
                        if text == "default" { (None, Vec::new()) } else { parse_target(&text) };
                    opts.push(Self::mv_option(f, arch, features));
                }
                Some((MvKind::Clones, n)) => {
                    let list = self.ast.mv_attr_list(&n);
                    for i in 0..list.len() {
                        if !first_of_version(&list, i) {
                            continue;
                        }
                        let f = self.mv_version_fn(v, i)?;
                        let (arch, features) = clone_option(&list[i]);
                        opts.push(Self::mv_option(f, arch, features));
                    }
                }
                _ => {}
            }
        }
        let kind = self.ast.mv_kind(first);
        let f = self.mv_ref(first)?;
        let base = self.mv_base(first);
        if kind == MvKind::Clones
            && let ValueRef::Alias(ifunc) = self.m.resolve(Value::func(f)).get()
        {
            // clang 19+: the `.ifunc` alias of a clone's `ifunc` (its
            // name before clang 19), "deprecated, and may be removed".
            let name = format!("{base}.ifunc");
            if self.m.live_symbol(&name).is_none() {
                let fty = self.m.func(f).ty();
                match self.m.add_alias(&name, fty, Value::alias(ifunc)) {
                    Ok(a) => self.mv_common(first, a),
                    Err(e) => return err(None, format!("cannot alias {name}: {e:?}")),
                }
            }
        }
        // clang `getFMVPriority`: the highest of the option's features
        // and `arch=` CPU, descending, stable.
        let prio = |o: &MvOption| {
            o.arch.iter().chain(&o.features).map(|s| x86_fmv::fmv_priority(s)).max().unwrap_or(0)
        };
        opts.sort_by_key(|o| std::cmp::Reverse(prio(o)));
        let resolver = self.mv_resolver_fn(&base)?;
        self.mv_resolver_linkage(first, resolver, &base);
        self.emit_x86_resolver(resolver, &opts)
    }

    fn mv_option(f: FuncId, arch: Option<String>, features: Vec<String>) -> MvOption {
        let mask = x86_fmv::cpu_supports_mask(features.iter().map(String::as_str));
        MvOption { f, arch, features, mask }
    }

    /// The linkage, comdat and attributes of resolver `r` of `first`
    /// (clang `setMultiVersionResolverAttributes`).
    fn mv_resolver_linkage(&mut self, first: DeclId, r: FuncId, base: &str) {
        let linkage = self.mv_linkage(first);
        let comdat = (linkage != Linkage::Internal)
            .then(|| self.m.comdat(&format!("{base}.resolver"), ComdatKind::Any));
        let attrs = self.m.attr_set(&[Attr::Flag(LAttr::DisableSanitizerInstrumentation)]);
        let fm = self.m.func_mut(r);
        fm.linkage = linkage;
        fm.comdat = comdat;
        fm.dso_local = linkage != Linkage::Internal;
        fm.attrs.func = attrs;
    }

    /// clang `emitCPUDispatchDefinition` for `cpu_dispatch` function `d`:
    /// its resolver `.resolver` over the versions of its CPUs (emitted if
    /// defined, else declared), the `ifunc` `.ifunc` and the alias of the
    /// plain name. Clang emits it for each declaration: a later one only
    /// appends a second copy of the resolver's body, unreachable.
    fn emit_cpu_dispatch(&mut self, d: DeclId) -> R<()> {
        let first = self.ast.first(d);
        let base = self.mv_base(first);
        if !self.mv.dispatch_decls.insert(d) {
            return Ok(());
        }
        let again = !self.mv.dispatched.insert(base.clone());
        let sig = self.decl_sig(first)?;
        let resolver = self.mv_resolver_fn(&base)?;
        if !again {
            self.mv_resolver_linkage(first, resolver, &base);
            // clang gives a `cpu_dispatch` resolver its comdat whatever
            // its linkage.
            let c = self.m.comdat(&format!("{base}.resolver"), ComdatKind::Any);
            self.m.func_mut(resolver).comdat = Some(c);
        }
        let mut opts: Vec<MvOption> = Vec::new();
        let cpus = self.mv_list(d);
        for cpu in &cpus {
            let Some(p) = x86_fmv::processor(cpu) else { continue };
            let name = format!("{base}.{}", p.mangling);
            let f = match self.m.get_function(&name) {
                Some(f) => f,
                None => self.mv_dispatch_version(first, &name, p.mangling)?,
            };
            // The CPU's `__builtin_cpu_supports` features: none for a
            // zero mask (clang `validateCpuSupports`).
            let features = if p.mask == [0; 4] { Vec::new() } else { vec![p.name.to_owned()] };
            opts.push(MvOption { f, arch: None, features, mask: p.mask });
        }
        opts.sort_by_key(|o| std::cmp::Reverse(o.mask));
        while opts.len() > 1 && opts[opts.len() - 2].mask == [0; 4] {
            let n = opts.len();
            let (l, r) = (self.m.func(opts[n - 2].f).name(), self.m.func(opts[n - 1].f).name());
            let drop = if l < r { n - 2 } else { n - 1 };
            opts.remove(drop);
        }
        self.emit_x86_resolver(resolver, &opts)?;
        if again {
            return Ok(());
        }
        let a = match self.mv.refs.get(&base).copied() {
            Some(f) if self.m.resolve(Value::func(f)) == Value::func(f) => {
                self.mv_replace(first, f, sig.fn_ty, resolver)?
            }
            Some(f) => match self.m.resolve(Value::func(f)).get() {
                ValueRef::Alias(a) => a,
                _ => return Ok(()),
            },
            None => {
                let (f, a) = self.mv_ifunc(first, &format!("{base}.ifunc"), sig.fn_ty, resolver)?;
                self.mv.refs.insert(base.clone(), f);
                a
            }
        };
        if self.m.live_symbol(&base).is_none() {
            match self.m.add_alias(&base, sig.fn_ty, Value::alias(a)) {
                Ok(al) => self.mv_common(first, al),
                Err(e) => return err(None, format!("cannot alias {base}: {e:?}")),
            }
        }
        Ok(())
    }

    /// The version `name` (CPU mangling letter `c`) of `cpu_dispatch`
    /// function `first` that no function holds yet (clang
    /// `emitCPUDispatchDefinition` and its `Manglings` lookup): the
    /// `cpu_specific` declaration whose first CPU it is, if clang named
    /// it already (referenced, or a deferred definition), is emitted if
    /// defined, else declared with its attributes; otherwise a declaration
    /// with the module's.
    fn mv_dispatch_version(&mut self, first: DeclId, name: &str, c: char) -> R<FuncId> {
        let named = self.ast.mv_versions(first).into_iter().find(|&v| {
            self.ast.mv_kind(v) == MvKind::CpuSpecific
                && self.mv_list(v).first().and_then(|n| x86_fmv::processor(n)).map(|p| p.mangling)
                    == Some(c)
                && (self.mv.named.contains(&self.ast.first(v))
                    || self.ast.definition(v).is_some_and(|d| self.discardable_fn(d)))
        });
        let Some(v) = named else {
            return self.mv_declare(
                first,
                name,
                MvCur { first, target: None, tune: None, keep_cpu: false },
            );
        };
        if let Some(def) = self.ast.definition(v) {
            self.emit_mv_version(def, 0)?;
            if let Some(f) = self.m.get_function(name) {
                return Ok(f);
            }
        }
        let cur = self.mv_version_target(v, 0);
        self.mv_declare(self.ast.first(v), name, cur)
    }

    /// `__cpu_model` (`{ i32, i32, i32, [1 x i32] }`) or `__cpu_features2`
    /// (`[3 x i32]`), declared `dso_local` if new (clang
    /// `CreateRuntimeVariable` then `setDSOLocal(true)`).
    fn cpu_global(&mut self, model: bool) -> (GlobalId, LTy) {
        let ty = if model {
            let one = self.m.types.array(LTy::I32, 1);
            self.m.types.struct_literal(&[LTy::I32, LTy::I32, LTy::I32, one], false)
        } else {
            self.m.types.array(LTy::I32, 3)
        };
        let name = if model { "__cpu_model" } else { "__cpu_features2" };
        let g = self.m.get_global(name).unwrap_or_else(|| {
            let g = self.m.add_global(name, ty);
            self.m.global_mut(g).dso_local = true;
            g
        });
        (g, ty)
    }

    /// The constant address of word `field` (`sub`: of its array) of
    /// global `g` of type `ty`; all-zero indices fold to `g`.
    fn cpu_field(&mut self, g: GlobalId, ty: LTy, idx: &[i128]) -> R<Value> {
        if idx.iter().all(|&i| i == 0) {
            return Ok(Value::global(g));
        }
        let idx: Vec<Value> = idx.iter().map(|&i| self.m.const_int(LTy::I32, i)).collect();
        match self.m.const_gep(ty, Value::global(g), &idx, GepFlags::INBOUNDS, None) {
            Ok(v) => Ok(v),
            Err(e) => err(None, format!("bad cpu model address: {e:?}")),
        }
    }

    /// The loads one condition makes, in clang's order: `(word address,
    /// mask)` pairs of `EmitX86CpuSupports` (mask 0 for none), or the
    /// field and value of `EmitX86CpuIs`.
    fn cpu_supports_steps(&mut self, mask: [u32; 4]) -> R<Vec<(Value, u32)>> {
        let mut steps = Vec::new();
        if mask[0] != 0 {
            let (g, ty) = self.cpu_global(true);
            steps.push((self.cpu_field(g, ty, &[0, 3, 0])?, mask[0]));
        }
        let (g, ty) = self.cpu_global(false);
        for (i, &m) in mask.iter().enumerate().skip(1) {
            if m != 0 {
                steps.push((self.cpu_field(g, ty, &[0, i as i128 - 1])?, m));
            }
        }
        Ok(steps)
    }

    /// clang `EmitX86MultiVersionResolver` into `resolver`: after
    /// `__cpu_indicator_init`, each option's condition branches to a
    /// block returning it; one without a condition is returned last;
    /// without one, `llvm.trap`.
    fn emit_x86_resolver(&mut self, resolver: FuncId, opts: &[MvOption]) -> R<()> {
        let init_ty = self.m.types.function(LTy::VOID, &[], false);
        let init = match self.m.get_function("__cpu_indicator_init") {
            Some(f) => f,
            None => match self.m.add_function("__cpu_indicator_init", init_ty) {
                Ok(f) => {
                    self.m.func_mut(f).dso_local = true;
                    f
                }
                Err(e) => return err(None, format!("cannot declare __cpu_indicator_init: {e:?}")),
            },
        };
        // The conditions' operands, created in clang's order.
        let mut conds: Vec<Vec<Step>> = Vec::with_capacity(opts.len());
        for o in opts {
            let mut c = Vec::new();
            if let Some(a) = &o.arch {
                if a.starts_with("x86-64") {
                    let m = x86_fmv::cpu_supports_mask([a.as_str()]);
                    c.push(Step::Supports(self.cpu_supports_steps(m)?));
                } else {
                    let (g, ty) = self.cpu_global(true);
                    let (field, value) = x86_fmv::cpu_is(a).unwrap_or((0, 0));
                    let at = self.cpu_field(g, ty, &[0, i128::from(field)])?;
                    c.push(Step::Is(at, value));
                }
            }
            if !o.features.is_empty() {
                c.push(Step::Supports(self.cpu_supports_steps(o.mask)?));
            }
            conds.push(c);
        }
        // clang `EmitTrapCall` only without a default.
        let trap = if conds.iter().any(Vec::is_empty) {
            None
        } else {
            let t = kiln_llvm::intrinsics::trap(&mut self.m);
            let a = self.m.attr_set(&[Attr::Flag(LAttr::NoReturn), Attr::Flag(LAttr::NoUnwind)]);
            Some((t, self.m.func(t).ty(), a))
        };
        let mut b = IrBuilder::new(&mut self.m, resolver);
        let r = (|| -> kiln_llvm::IrResult<()> {
            let entry = b.append_block("");
            b.position_at_end(entry);
            b.call(init, &[])?;
            for (o, c) in opts.iter().zip(conds) {
                let mut cond: Option<Value> = None;
                for step in c {
                    let v = match step {
                        Step::Is(at, value) => {
                            let x = b.load(LTy::I32, at, 4)?;
                            let k = b.module().const_int(LTy::I32, i128::from(value));
                            b.icmp(IntPredicate::Eq, x, k)?
                        }
                        Step::Supports(words) => {
                            let mut res = b.module().const_bool(true);
                            for (at, m) in words {
                                let x = b.load(LTy::I32, at, 4)?;
                                let k = b.module().const_int(LTy::I32, i128::from(m));
                                let bits = b.binop(BinOp::And, x, k)?;
                                let eq = b.icmp(IntPredicate::Eq, bits, k)?;
                                res = b.binop(BinOp::And, res, eq)?;
                            }
                            res
                        }
                    };
                    cond = Some(match cond {
                        Some(c) => b.binop(BinOp::And, c, v)?,
                        None => v,
                    });
                }
                let Some(cond) = cond else {
                    b.ret(Some(Value::func(o.f)))?;
                    return Ok(());
                };
                let ret = b.append_block("");
                let other = b.append_block("");
                b.cond_br(cond, ret, other)?;
                b.position_at_end(ret);
                b.ret(Some(Value::func(o.f)))?;
                b.position_at_end(other);
            }
            if let Some((t, ty, a)) = trap {
                let o = CallOpts {
                    attrs: AttrList { func: a, ..AttrList::default() },
                    ..CallOpts::default()
                };
                b.call_ex(ty, Value::func(t), &[], &o)?;
            }
            b.unreachable()?;
            Ok(())
        })();
        match r {
            Ok(()) => Ok(()),
            Err(e) => err(None, format!("bad multiversion resolver: {e:?}")),
        }
    }
}
