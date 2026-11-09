//! The IR effects of the class B attributes and their driver flags
//! (WP-813, spec/landed/codegen/WP-813.md): hardening, instrumentation,
//! inlining and pointer facts, placed where clang places them.
//!
//! - `ConstructAttributeList`'s declaration attributes, on a declaration,
//!   a definition and a direct call alike: [`Cg::class_b_fn_attrs`] and
//!   [`Cg::class_b_params`];
//! - `SetLLVMFunctionAttributesForDefinition` and `StartFunction` on a
//!   definition: [`Cg::class_b_def_attrs`];
//! - the flag defaults of `getDefaultFunctionAttributes` (no call site):
//!   [`Cg::class_b_default_attrs`];
//! - the call-site-only ones of `EmitCall`: [`Cg::class_b_call_attrs`]
//!   and [`Cg::stmt_call_attrs`];
//! - `EmitFunctionProlog`'s parameter facts:
//!   [`Cg::class_b_prolog_params`].

use kiln_abi::call::ArgKind;
use kiln_abi::irtype::IrType;
use kiln_ast::DeclId;
use kiln_ast::attr::{AttrKind as K, AttrNode};
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::RawId;
use kiln_ast::sanitize::{SanitizerSet as San, no_sanitize_set};
use kiln_llvm::{Attr, AttrKind, AttrSetId, GlobalSanitizer, TypeId as LTy};
use kiln_source::Loc;

use crate::module::{Cg, Sig};

/// clang `ZeroCallUsedRegsAttr::ConvertKindToStr`, indexed by the
/// attribute node's `a`.
const ZERO_CALL_KINDS: [&str; 9] = [
    "skip",
    "used-gpr-arg",
    "used-gpr",
    "used-arg",
    "used",
    "all-gpr-arg",
    "all-gpr",
    "all-arg",
    "all",
];

/// What `StartFunction` knows of the function it starts (its
/// `GlobalDecl` and `CurFuncDecl`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StartDecl {
    /// No declaration: a thunk, `_GLOBAL__sub_I_*`, `__tls_init`.
    None,
    /// A variable's initialization function (a declaration, no
    /// function).
    Var,
    /// A function definition (its `CurFuncDecl`).
    Fn(DeclId),
    /// A kiln-outlined piece of function `d` (WP-641): `d`'s attributes,
    /// no instrumentation of its own.
    Outlined(DeclId),
}

impl StartDecl {
    fn decl(self) -> Option<DeclId> {
        match self {
            StartDecl::Fn(d) | StartDecl::Outlined(d) => Some(d),
            _ => None,
        }
    }
}

impl Cg<'_> {
    /// The sanitizers the `no_sanitize*` attributes of `d` exclude (clang
    /// `NoSanitizeAttr::getMask`, with `address` and `kernel-address`,
    /// `hwaddress` and `kernel-hwaddress` excluding each other); all of
    /// them under `disable_sanitizer_instrumentation`.
    pub(crate) fn no_sanitize_mask(&self, d: DeclId) -> San {
        if self.has_fn_attr(d, K::DISABLE_SANITIZER_INSTRUMENTATION) {
            return San::ALL;
        }
        let mut m = San::NONE;
        for n in self.fn_attrs_of(d, K::NO_SANITIZE) {
            m = m.union(no_sanitize_set(n.a, n.b));
        }
        // A variable template's instantiation has its pattern's
        // attributes (clang `InstantiateAttrs`; kiln keeps them on the
        // pattern).
        if let Some(p) = self.var_template_pattern(d) {
            for n in self.fn_attrs_of(p, K::NO_SANITIZE) {
                m = m.union(no_sanitize_set(n.a, n.b));
            }
        }
        for pair in
            [San::ADDRESS.union(San::KERNEL_ADDRESS), San::HWADDRESS.union(San::KERNEL_HWADDRESS)]
        {
            if m.has_any(pair) {
                m = m.union(pair);
            }
        }
        m
    }

    /// The pattern implicitly instantiated variable template
    /// specialization `d` was instantiated from.
    fn var_template_pattern(&self, d: DeclId) -> Option<DeclId> {
        let ast = self.ast;
        ast.decls.var(d)?;
        let (_, info) = ast.spec_info(d)?;
        if info.kind == kiln_ast::templates::SpecKind::ExplicitSpecialization {
            return None;
        }
        ast.template_info(info.template)?.pattern
    }

    /// The `SanOpts` of clang `StartFunction` for `start`: the module's
    /// sanitizers less what the function's attributes exclude. A
    /// variable's initialization function has the module's.
    pub(crate) fn fn_sanitizers(&self, start: StartDecl) -> San {
        let kinds = self.opts.sanitize.kinds;
        match start.decl() {
            Some(d) if !kinds.is_empty() => kinds.without(self.no_sanitize_mask(d)),
            _ => kinds,
        }
    }

    /// clang `CodeGenModule::isInNoSanitizeList(kind, Fn, Loc)` for each
    /// sanitizer of `kinds`: those an ignorelist lists the function `name`
    /// under, by `fun:` (its mangled name), `mainfile:`, or `src:` (the
    /// file of `loc`'s expansion, the main file without one).
    pub(crate) fn ignored_sanitizers(&self, kinds: San, name: &str, loc: Option<Loc>) -> San {
        let Some(list) = self.opts.ignorelist.as_deref() else { return San::NONE };
        let main = self.opts.source_filename.as_str();
        let file = match (self.sm, loc) {
            (Some(sm), Some(l)) => {
                sm.offset_of(sm.expansion_loc(l)).map_or(main, |(fid, _)| sm.file_name(fid))
            }
            _ => main,
        };
        let newer = self.opts.clang_compat >= 22;
        let mut out = San::NONE;
        for &(_, k) in kiln_ast::sanitize::SANITIZERS {
            if !kinds.has_any(k) {
                continue;
            }
            if list.contains(k, "fun", name, "", newer)
                || list.contains(k, "mainfile", main, "", newer)
                || list.contains(k, "src", file, "", newer)
            {
                out = out.union(k);
            }
        }
        out
    }

    /// `StartFunction`: the attributes of the sanitizers `s` of a
    /// function's `SanOpts`.
    pub(crate) fn sanitizer_fn_attrs(a: &mut Vec<Attr>, s: San) {
        for (k, f) in [
            (San::ADDRESS.union(San::KERNEL_ADDRESS), AttrKind::SanitizeAddress),
            (San::HWADDRESS.union(San::KERNEL_HWADDRESS), AttrKind::SanitizeHWAddress),
            (San::MEMTAG_STACK, AttrKind::SanitizeMemTag),
            (San::THREAD, AttrKind::SanitizeThread),
            (San::MEMORY.union(San::KERNEL_MEMORY), AttrKind::SanitizeMemory),
            (San::FUZZER.union(San::FUZZER_NO_LINK), AttrKind::OptForFuzzing),
        ] {
            if s.has_any(k) {
                a.push(Attr::Flag(f));
            }
        }
    }

    /// Takes the sanitizers an ignorelist lists the function `f` (declared
    /// at `loc`) under out of the function being emitted (clang
    /// `StartFunction` and `CreateGlobalInitOrCleanUpFunction`).
    pub(crate) fn ignore_sanitizers(&mut self, f: kiln_llvm::FuncId, loc: Option<Loc>) {
        if self.opts.ignorelist.is_none() {
            return;
        }
        let name = self.m.func(f).name().to_owned();
        let ignored = self.ignored_sanitizers(self.opts.sanitize.kinds, &name, loc);
        self.f.san_ignored = ignored;
        self.f.san = self.f.san.without(ignored);
    }

    /// The sanitizer metadata of the global of variable `d` (clang
    /// `SanitizerMetadata::reportGlobal`), `dyn_init` when it has a
    /// dynamic initializer; nothing unless an address, hardware-address
    /// or memory-tagging sanitizer is on.
    pub(crate) fn global_sanitizer(
        &self,
        d: DeclId,
        g: kiln_llvm::GlobalId,
        dyn_init: bool,
    ) -> GlobalSanitizer {
        let loc = self.ast.decls.get(d).loc();
        self.report_global(Some(d), Some(loc), g, dyn_init)
    }

    /// The sanitizer metadata of the new global `g` of a string literal
    /// at `loc` (clang `GetAddrOfConstantStringFromLiteral`'s
    /// `reportGlobal`: no type, no `no_sanitize`). WP-1043.
    pub(crate) fn string_lit_sanitizer(
        &self,
        g: kiln_llvm::GlobalId,
        loc: Option<Loc>,
    ) -> GlobalSanitizer {
        self.report_global(None, loc, g, false)
    }

    /// clang `SanitizerMetadata::reportGlobal` for the global `g` of
    /// variable `d` (none for a string literal) at `loc`.
    fn report_global(
        &self,
        d: Option<DeclId>,
        loc: Option<Loc>,
        g: kiln_llvm::GlobalId,
        dyn_init: bool,
    ) -> GlobalSanitizer {
        let kinds = expand_kernel(self.opts.sanitize.kinds);
        if !kinds.has_any(San::ADDRESS.union(San::HWADDRESS).union(San::MEMTAG)) {
            return GlobalSanitizer::default();
        }
        let no = d.map_or(San::NONE, |d| expand_kernel(self.no_sanitize_mask(d)).intersect(kinds));
        let listed = |k: San, cat: &str| self.global_ignored(k, d, loc, g, cat);
        let no_address = no.has_any(San::ADDRESS) || listed(kinds.intersect(San::ADDRESS), "");
        GlobalSanitizer {
            no_address,
            no_hwaddress: no.has_any(San::HWADDRESS) || listed(kinds.intersect(San::HWADDRESS), ""),
            memtag: kinds.has_any(San::MEMTAG_GLOBALS)
                && !no.has_any(San::MEMTAG)
                && !listed(kinds.intersect(San::MEMTAG), ""),
            dyninit: dyn_init
                && !no_address
                && kinds.has_any(San::ADDRESS)
                && !listed(San::ADDRESS.union(San::KERNEL_ADDRESS), "init"),
        }
    }

    /// clang `CodeGenModule::isInNoSanitizeList(kind, GV, Loc, Ty,
    /// category)` for the global `g` of variable `d` (none: a string
    /// literal) at `loc`: `global:` (its name), `mainfile:`, `src:` (the
    /// file of `loc`'s expansion) or `type:` (`d`'s class type, arrays
    /// drilled to their element, printed canonically with full scope).
    fn global_ignored(
        &self,
        k: San,
        d: Option<DeclId>,
        loc: Option<Loc>,
        g: kiln_llvm::GlobalId,
        cat: &str,
    ) -> bool {
        let Some(list) = self.opts.ignorelist.as_deref() else { return false };
        if k.is_empty() {
            return false;
        }
        let newer = self.opts.clang_compat >= 22;
        let main = self.opts.source_filename.as_str();
        if list.contains(k, "global", self.m.global(g).name(), cat, newer)
            || list.contains(k, "mainfile", main, cat, newer)
        {
            return true;
        }
        let ast = self.ast;
        if let Some(sm) = self.sm
            && let Some(loc) = loc
            && let Some((fid, _)) = sm.offset_of(sm.expansion_loc(loc))
            && list.contains(k, "src", sm.file_name(fid), cat, newer)
        {
            return true;
        }
        let Some(mut q) = d.and_then(|d| ast.decls.get(d).ty()) else { return false };
        while ast.types.is_array(q) {
            match ast.types.element(q) {
                Some(e) => q = e,
                None => break,
            }
        }
        let q = ast.types.canonical(q);
        if ast.types.as_record(q).is_none() {
            return false;
        }
        let names = match self.sm {
            Some(sm) => kiln_ast::dump::AstNames::with_sources(ast, sm),
            None => kiln_ast::dump::AstNames::new(ast),
        };
        let opts = kiln_ast::types::PrintOptions {
            full_scope: true,
            ..kiln_ast::types::PrintOptions::SUGARED
        };
        let s = kiln_ast::types::print_type(&ast.types, &names, q, opts);
        let mut s = s.as_str();
        while let Some(r) = ["const ", "volatile "].iter().find_map(|p| s.strip_prefix(p)) {
            s = r;
        }
        list.contains(k, "type", s, cat, newer)
    }

    /// The class B attribute nodes of kind `k` on the redeclarations of
    /// `d`, in declaration order.
    pub(crate) fn fn_attrs_of(&self, d: DeclId, k: K) -> Vec<AttrNode> {
        let ast = self.ast;
        let mut v: Vec<AttrNode> = Vec::new();
        for r in ast.redecls_of(d) {
            for a in ast.lists.iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r)) {
                let n = *ast.attrs.get(a);
                if n.kind == k && !v.iter().any(|x| x.loc == n.loc) {
                    v.push(n);
                }
            }
        }
        v.sort_by_key(|n| n.loc.to_raw());
        v
    }

    /// The `"zero-call-used-regs"` value of `zero_call_used_regs` on `d`.
    fn zero_call_attr(&self, d: DeclId) -> Option<&'static str> {
        let n = self.fn_attr(d, K::ZERO_CALL_USED_REGS)?;
        ZERO_CALL_KINDS.get(n.a as usize).copied()
    }

    /// clang `ConstructAttributeList`'s declaration attributes of `d`
    /// (on its declaration, definition and direct calls): the
    /// `zero_call_used_regs` kind (over the flag's),
    /// `speculative_load_hardening`, `nocf_check`, `leaf` as
    /// `nocallback`, `noduplicate`, `convergent`, the `assume`
    /// strings as `"llvm.assume"`, and `returns_nonnull`'s `nonnull`.
    pub(crate) fn class_b_fn_attrs(&self, d: DeclId, fa: &mut Vec<Attr>, ra: &mut Vec<Attr>) {
        if let Some(z) = self.zero_call_attr(d) {
            fa.retain(|a| !matches!(a, Attr::Str(k, _) if &**k == "zero-call-used-regs"));
            fa.push(Attr::string("zero-call-used-regs", z));
        }
        if self.has_fn_attr(d, K::SPECULATIVE_LOAD_HARDENING) {
            fa.push(Attr::Flag(AttrKind::SpeculativeLoadHardening));
        }
        for (k, a) in [
            (K::NOCF_CHECK, AttrKind::NoCfCheck),
            (K::LEAF, AttrKind::NoCallback),
            (K::NODUPLICATE, AttrKind::NoDuplicate),
            (K::CONVERGENT, AttrKind::Convergent),
        ] {
            if self.has_fn_attr(d, k) {
                fa.push(Attr::Flag(a));
            }
        }
        let assumes = self.fn_attrs_of(d, K::OMP_ASSUME);
        if !assumes.is_empty() {
            let ast = self.ast;
            let s: Vec<String> = assumes
                .iter()
                .filter_map(|n| kiln_ast::StringLitId::from_raw(n.a))
                .map(|l| String::from_utf8_lossy(ast.strings.bytes(l)).into_owned())
                .collect();
            fa.push(Attr::string("llvm.assume", &s.join(",")));
        }
        if self.has_fn_attr(d, K::RETURNS_NONNULL) && !self.opts.null_pointer_is_valid {
            ra.push(Attr::Flag(AttrKind::NonNull));
        }
    }

    /// The flag defaults of clang `getDefaultFunctionAttributes` on the
    /// function `d` (`None`: a helper): `-fsplit-stack`'s `"split-stack"`
    /// (not on a `no_split_stack` function), `-mspeculative-load-
    /// hardening` (not on a `no_speculative_load_hardening` one) and
    /// `-fzero-call-used-regs=` (a `zero_call_used_regs` one's own
    /// kind wins, [`Self::class_b_fn_attrs`]).
    pub(crate) fn class_b_default_attrs(&self, out: &mut Vec<Attr>, d: Option<DeclId>) {
        let h = &self.opts.hardening;
        let has = |k| d.is_some_and(|d| self.has_fn_attr(d, k));
        if h.split_stack && !has(K::NO_SPLIT_STACK) {
            out.push(Attr::string("split-stack", ""));
        }
        if h.speculative_load_hardening && !has(K::NO_SPECULATIVE_LOAD_HARDENING) {
            out.push(Attr::Flag(AttrKind::SpeculativeLoadHardening));
        }
        if let Some(z) = h.zero_call_used_regs
            && z != "skip"
            && !has(K::ZERO_CALL_USED_REGS)
        {
            out.push(Attr::string("zero-call-used-regs", z));
        }
    }

    /// The definition attributes of clang
    /// `SetLLVMFunctionAttributesForDefinition` and `StartFunction` for
    /// `start`: `no_stack_protector` drops `ssp*`, `nouwtable` drops
    /// `uwtable`; `cfi_canonical_jump_table`, `force_align_arg_pointer`
    /// (`"stackrealign"`), `function_return` (or
    /// `-mfunction-return=thunk-extern`), `no_profile_instrument_function`
    /// (`noprofile`) and `disable_sanitizer_instrumentation`; the
    /// patchable entry (`patchable_function_entry` over the flag), `XRay`
    /// and `-finstrument-functions*` attributes.
    pub(crate) fn class_b_def_attrs(&self, a: &mut Vec<Attr>, start: StartDecl) {
        let d = start.decl();
        let has = |k| d.is_some_and(|d| self.has_fn_attr(d, k));
        if has(K::NO_STACK_PROTECTOR) {
            a.retain(|x| {
                !matches!(
                    x,
                    Attr::Flag(
                        AttrKind::StackProtect
                            | AttrKind::StackProtectStrong
                            | AttrKind::StackProtectReq
                    )
                )
            });
        }
        if has(K::NOUWTABLE) {
            a.retain(|x| !matches!(x, Attr::UwTable(_)));
        }
        if has(K::CFI_CANONICAL_JUMP_TABLE) {
            a.push(Attr::string("cfi-canonical-jump-table", ""));
        }
        if has(K::FORCE_ALIGN_ARG_POINTER) {
            a.push(Attr::string("stackrealign", ""));
        }
        let h = &self.opts.hardening;
        let thunk_extern = match d.and_then(|d| self.fn_attr(d, K::FUNCTION_RETURN)) {
            Some(n) => n.a == 1,
            None => h.fn_return_thunk_extern && start != StartDecl::None,
        };
        if thunk_extern {
            a.push(Attr::Flag(AttrKind::FnRetThunkExtern));
        }
        if has(K::NO_PROFILE_INSTRUMENT_FUNCTION) {
            a.push(Attr::Flag(AttrKind::NoProfile));
        }
        // `StartFunction`: the sanitizer attributes of its `SanOpts`
        // (`CreateGlobalInitOrCleanUpFunction` adds the module's to an
        // initialization function whatever its variable excludes).
        if has(K::DISABLE_SANITIZER_INSTRUMENTATION) {
            a.push(Attr::Flag(AttrKind::DisableSanitizerInstrumentation));
        } else if self.opts.sanitize.any() {
            let all = self.opts.sanitize.kinds;
            let s = self.fn_sanitizers(start).without(self.f.san_ignored);
            if all.has_any(San::BOUNDS) && !s.has_any(San::BOUNDS) {
                a.push(Attr::Flag(AttrKind::NoSanitizeBounds));
            }
            Self::sanitizer_fn_attrs(a, s);
        }
        // `StartFunction`: XRay.
        let xray = h.xray.as_ref();
        match d.and_then(|d| self.fn_attr(d, K::XRAY_INSTRUMENT)) {
            Some(n) => {
                if n.a == 1 && xray.is_some() {
                    a.push(Attr::string("function-instrument", "xray-always"));
                }
                if n.a == 0 {
                    a.push(Attr::string("function-instrument", "xray-never"));
                }
                if let Some(l) = d.and_then(|d| self.fn_attr(d, K::XRAY_LOG_ARGS))
                    && xray.is_some()
                {
                    a.push(Attr::string("xray-log-args", &l.a.to_string()));
                }
            }
            None => {
                if let Some(x) = xray {
                    a.push(Attr::string("xray-instruction-threshold", &x.threshold.to_string()));
                }
            }
        }
        if xray.is_some_and(|x| x.ignore_loops) {
            a.push(Attr::string("xray-ignore-loops", ""));
        }
        // `-finstrument-functions*` (`ShouldInstrumentFunction`: a
        // function without `no_instrument_function`).
        if let StartDecl::Fn(f) = start
            && !self.has_fn_attr(f, K::NO_INSTRUMENT_FUNCTION)
        {
            if h.instrument_functions {
                a.push(Attr::string("instrument-function-entry", "__cyg_profile_func_enter"));
                a.push(Attr::string("instrument-function-exit", "__cyg_profile_func_exit"));
            }
            if h.instrument_functions_after_inlining {
                a.push(Attr::string(
                    "instrument-function-entry-inlined",
                    "__cyg_profile_func_enter",
                ));
                a.push(Attr::string("instrument-function-exit-inlined", "__cyg_profile_func_exit"));
            }
            if h.instrument_function_entry_bare {
                a.push(Attr::string(
                    "instrument-function-entry-inlined",
                    "__cyg_profile_func_enter_bare",
                ));
            }
        }
        // The patchable entry: the attribute's, else the flag's.
        let (count, offset) = match d.and_then(|d| self.fn_attr(d, K::PATCHABLE_FUNCTION_ENTRY)) {
            Some(n) => (n.a, n.b),
            None => h.patchable_function_entry.unwrap_or((0, 0)),
        };
        if count != 0 && offset <= count {
            a.push(Attr::string("patchable-function-entry", &(count - offset).to_string()));
            if offset != 0 {
                a.push(Attr::string("patchable-function-prefix", &offset.to_string()));
            }
        }
    }

    /// The IR parameter index of each argument of `sig` (`None`: an
    /// ignored one) and whether it is a direct pointer (clang
    /// `EmitFunctionProlog`'s `ABIArgInfo::Direct` case with a pointer
    /// coercion at offset 0).
    fn arg_slots(sig: &Sig) -> Vec<(Option<usize>, bool)> {
        let mut at =
            usize::from(sig.sret()) + usize::from(sig.this.is_some()) + usize::from(sig.vtt);
        let mut out = Vec::with_capacity(sig.info.args.len());
        for a in &sig.info.args {
            match &a.kind {
                ArgKind::Ignore => out.push((None, false)),
                ArgKind::Direct { coerce: IrType::Struct(fs, _), flatten: true, .. } => {
                    out.push((Some(at), false));
                    at += fs.len().max(1);
                }
                ArgKind::Direct { coerce, offset, .. } => {
                    out.push((Some(at), *offset == 0 && *coerce == IrType::Ptr));
                    at += 1;
                }
                _ => {
                    out.push((Some(at), false));
                    at += 1;
                }
            }
        }
        out
    }

    /// The typedef declaration of `q` (clang `getAs<TypedefType>`).
    fn typedef_of(&self, mut q: kiln_ast::QualType) -> Option<DeclId> {
        let t = &self.ast.types;
        for _ in 0..64 {
            if let kiln_ast::types::TypeView::Typedef { decl, .. } = t.view(q.ty()) {
                return Some(decl);
            }
            let n = t.desugar_once(q);
            if n == q {
                return None;
            }
            q = n;
        }
        None
    }

    /// The parameter declarations of the redeclarations of `d`, by
    /// position.
    pub(crate) fn redecl_params(&self, d: DeclId) -> Vec<Vec<DeclId>> {
        let ast = self.ast;
        ast.redecls_of(d)
            .filter_map(|r| ast.decls.function(r))
            .map(|f| ast.lists.iter::<DeclId>(f.params).collect())
            .collect()
    }

    /// Whether parameter `i` of some redeclaration of `d` carries `k`.
    fn param_has(&self, params: &[Vec<DeclId>], i: usize, k: K) -> bool {
        params.iter().filter_map(|p| p.get(i)).any(|&p| self.has_fn_attr(p, k))
    }

    /// Adds `attr` to IR parameter `at` of `params`.
    fn add_param_attr(&mut self, params: &mut [AttrSetId], at: usize, attr: Attr) {
        let Some(s) = params.get(at).copied() else { return };
        let mut v = self.m.attrs_of(s).to_vec();
        v.push(attr);
        params[at] = self.m.attr_set(&v);
    }

    /// `noescape` parameters as `nocapture` (clang
    /// `ConstructAttributeList` from the prototype's
    /// `ExtParameterInfo`), on a declaration, a definition and a direct
    /// call of `d` with signature `sig`.
    pub(crate) fn class_b_params(&mut self, d: DeclId, sig: &Sig, params: &mut [AttrSetId]) {
        let ps = self.redecl_params(d);
        if ps.is_empty() {
            return;
        }
        for (i, (slot, _)) in Self::arg_slots(sig).into_iter().enumerate() {
            if let Some(at) = slot
                && self.param_has(&ps, i, K::NOESCAPE)
            {
                self.add_param_attr(params, at, Attr::Flag(AttrKind::NoCapture));
            }
        }
    }

    /// Whether function attribute `nonnull` `n` (clang
    /// `NonNullAttr::isNonNull`) names parameter `i` (zero-based, after
    /// the implicit object parameter, which `this` counts): it has no
    /// index list, or its one-based list names `i`.
    pub(crate) fn nonnull_covers(&self, n: &AttrNode, i: usize, this: u32) -> bool {
        let ast = self.ast;
        let idx: Vec<u32> = kiln_ast::ListId::from_raw(n.a)
            .map(|l| {
                ast.lists
                    .iter::<kiln_ast::ExprId>(l)
                    .filter_map(|e| ast.exprs.int_value(e))
                    .filter_map(|v| u32::try_from(v).ok())
                    .collect()
            })
            .unwrap_or_default();
        idx.is_empty() || idx.iter().any(|&v| v.checked_sub(1 + this) == Some(i as u32))
    }

    /// clang `EmitFunctionProlog`'s parameter facts of definition `d`:
    /// `nonnull` on a pointer parameter named by `nonnull` (the
    /// function's, by one-based index, or none for every pointer; or the
    /// parameter's own), unless null pointers are valid; `align N` on one
    /// with `align_value(N)` (its own, or its declared type's typedef's).
    pub(crate) fn class_b_prolog_params(&mut self, d: DeclId, sig: &Sig, params: &mut [AttrSetId]) {
        let ast = self.ast;
        let Some(f) = ast.decls.function(d) else { return };
        let own: Vec<DeclId> = ast.lists.iter::<DeclId>(f.params).collect();
        let fn_nonnull = self.fn_attrs_of(d, K::NONNULL);
        let this = u32::from(
            ast.decls.get(ast.first(d)).kind() == DeclKind::CXXMethod
                && !self.is_static_method(ast.first(d)),
        );
        for (i, (slot, direct_ptr)) in Self::arg_slots(sig).into_iter().enumerate() {
            let (Some(at), true) = (slot, direct_ptr) else { continue };
            let Some(&p) = own.get(i) else { continue };
            let pty = ast.decls.get(p).ty();
            let is_ptr = pty.is_some_and(|t| ast.types.is_pointer(t));
            let fn_nn = is_ptr && fn_nonnull.iter().any(|n| self.nonnull_covers(n, i, this));
            if (fn_nn || self.has_fn_attr(p, K::NONNULL)) && !self.opts.null_pointer_is_valid {
                self.add_param_attr(params, at, Attr::Flag(AttrKind::NonNull));
            }
            let av = self.fn_attr(p, K::ALIGN_VALUE).or_else(|| {
                let td = self.typedef_of(ast.decls.get(p).ty()?)?;
                self.fn_attr(td, K::ALIGN_VALUE)
            });
            if let Some(n) = av {
                self.add_param_attr(params, at, Attr::Align(u64::from(n.a)));
            }
        }
    }

    /// [`Self::class_b_fn_attrs`] and [`Self::class_b_params`] on the
    /// attribute sets of a declaration or definition of `d`, and its
    /// `!callback` (clang `SetFunctionAttributes`).
    pub(crate) fn class_b_fn_sets(
        &mut self,
        f: kiln_llvm::FuncId,
        d: DeclId,
        sig: &Sig,
        fa: AttrSetId,
        ret: AttrSetId,
        params: &mut [AttrSetId],
    ) -> (AttrSetId, AttrSetId) {
        let mut fv = self.m.attrs_of(fa).to_vec();
        let mut rv = self.m.attrs_of(ret).to_vec();
        let (nf, nr) = (fv.len(), rv.len());
        self.class_b_fn_attrs(d, &mut fv, &mut rv);
        let fa = if fv.len() == nf && !self.has_fn_attr(d, K::ZERO_CALL_USED_REGS) {
            fa
        } else {
            self.m.attr_set(&fv)
        };
        let ret = if rv.len() == nr { ret } else { self.m.attr_set(&rv) };
        self.class_b_params(d, sig, params);
        self.callback_md(f, d);
        (fa, ret)
    }

    /// The call-site-only attributes clang `EmitCall` gives a direct call
    /// of `d`: `nomerge`, and `assume_aligned(N)`'s result `align N`
    /// (without an offset; with one it is
    /// [`Self::assume_aligned_assume`] after the call).
    pub(crate) fn class_b_call_attrs(&self, d: DeclId, fa: &mut Vec<Attr>, ra: &mut Vec<Attr>) {
        self.class_b_fn_attrs(d, fa, ra);
        if self.has_fn_attr(d, K::NOMERGE) {
            fa.push(Attr::Flag(AttrKind::NoMerge));
        }
        if let Some(n) = self.fn_attr(d, K::ASSUME_ALIGNED)
            && n.b == 0
        {
            ra.push(Attr::Align(if n.a == 0 { 1 << 32 } else { u64::from(n.a) }));
        }
    }

    /// The call-site attributes of the enclosing statement and function
    /// (clang `EmitCall`): `[[clang::nomerge]]`, `[[clang::noinline]]`
    /// and `[[clang::always_inline]]` statements mark every call in
    /// them; in a `flatten` function every call but one in a
    /// `[[clang::noinline]]` statement or of a `noinline` function is
    /// `alwaysinline`.
    pub(crate) fn stmt_call_attrs(&self, callee: Option<DeclId>, fa: &mut Vec<Attr>) {
        let s = self.f.stmt_calls;
        if s.nomerge {
            fa.push(Attr::Flag(AttrKind::NoMerge));
        }
        if s.noinline {
            fa.push(Attr::Flag(AttrKind::NoInline));
        }
        if s.always_inline {
            fa.push(Attr::Flag(AttrKind::AlwaysInline));
        }
        if self.f.flatten
            && !s.noinline
            && !callee.is_some_and(|d| self.has_fn_attr(d, K::NOINLINE))
            && !s.always_inline
        {
            fa.push(Attr::Flag(AttrKind::AlwaysInline));
        }
    }

    /// The call-site attributes of `AttributedStmt` `s`'s own
    /// statement attributes.
    pub(crate) fn stmt_calls_of(&self, s: kiln_ast::StmtId) -> StmtCalls {
        let ast = self.ast;
        let mut c = StmtCalls::default();
        for a in ast.lists.iter::<kiln_ast::AttrId>(ast.stmts.stmt_attrs(s)) {
            match ast.attrs.get(a).kind {
                K::NOMERGE => c.nomerge = true,
                K::NOINLINE => c.noinline = true,
                K::ALWAYS_INLINE => c.always_inline = true,
                _ => {}
            }
        }
        c
    }

    /// The call of `d` is `notail` (clang `EmitCall`: `not_tail_called`).
    pub(crate) fn not_tail_called(&self, d: DeclId) -> bool {
        self.has_fn_attr(d, K::NOT_TAIL_CALLED)
    }

    /// Whether parameter `i` of `d` is `maybe_undef` (clang `EmitCall`
    /// freezes the argument).
    pub(crate) fn maybe_undef_param(&self, d: DeclId, i: usize) -> bool {
        let ps = self.redecl_params(d);
        self.param_has(&ps, i, K::MAYBE_UNDEF)
    }

    /// clang `AssumeAlignedAttrEmitter`: after a call of `d` returning
    /// `r`, `assume_aligned(N, O)` with an offset becomes
    /// `llvm.assume(i1 true) ["align"(ptr r, i64 N, i64 O)]`.
    pub(crate) fn assume_aligned_assume(&mut self, d: DeclId, r: crate::fir::V) {
        let Some(n) = self.fn_attr(d, K::ASSUME_ALIGNED) else { return };
        if n.b == 0 || self.f.fir.cur().is_none() {
            return;
        }
        let align = if n.a == 0 { 1i128 << 32 } else { i128::from(n.a) };
        let a = crate::fir::V::C(self.m.const_int(LTy::I64, align));
        let o = crate::fir::V::C(self.m.const_int(LTy::I64, i128::from(n.b)));
        let f = kiln_llvm::intrinsics::assume(&mut self.m);
        let fn_ty = self.m.func(f).ty();
        let cond = self.m.const_bool(true);
        self.f.fir.push(
            crate::fir::Op::AssumeBundle {
                fn_ty,
                callee: kiln_llvm::Value::func(f),
                cond,
                tag: "align",
                args: vec![r, a, o],
            },
            LTy::VOID,
        );
    }

    /// clang `SetFunctionAttributes`: `callback` as `!callback` on the
    /// function `f` for `d`: `!{!{i64 callee, i64 payload..., i1 false}}`.
    pub(crate) fn callback_md(&mut self, f: kiln_llvm::FuncId, d: DeclId) {
        let Some(n) = self.fn_attr(d, K::CALLBACK) else { return };
        let Some(l) = kiln_ast::ListId::from_raw(n.a) else { return };
        let words: Vec<u32> = self.ast.lists.get(l).to_vec();
        let mut ops = Vec::with_capacity(words.len() + 1);
        for w in words {
            let v = self.m.const_int(LTy::I64, i128::from(w.cast_signed()));
            ops.push(self.m.md_const(v).ok());
        }
        let f0 = self.m.const_int(LTy::I1, 0);
        ops.push(self.m.md_const(f0).ok());
        let enc = self.m.md_tuple(&ops);
        let md = self.m.md_tuple(&[Some(enc)]);
        let kind = self.m.md_kind("callback");
        self.m.func_mut(f).add_metadata(kind, md);
    }
}

/// The call-site attributes of the statements being emitted
/// ([`Cg::stmt_call_attrs`]).
#[derive(Clone, Copy, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent statement attributes")]
pub(crate) struct StmtCalls {
    pub(crate) nomerge: bool,
    pub(crate) noinline: bool,
    pub(crate) always_inline: bool,
}

/// clang `expandKernelSanitizerMasks`: a kernel sanitizer counts as its
/// user-space counterpart.
fn expand_kernel(m: San) -> San {
    let mut m = m;
    for (k, u) in [(San::KERNEL_ADDRESS, San::ADDRESS), (San::KERNEL_HWADDRESS, San::HWADDRESS)] {
        if m.has_any(k) {
            m = m.union(u);
        }
    }
    m
}
