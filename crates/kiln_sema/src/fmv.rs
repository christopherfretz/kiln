//! Function multiversioning attributes (WP-810; clang `AttrDocs`
//! `target_version`, `target_clones`, `cpu_dispatch`, `cpu_specific`).
//!
//! On `AArch64` kiln does what clang does without the `fmv` target
//! feature, which clang's driver turns off unless the runtime library is
//! compiler-rt (`-fmv`, `--rtlib=compiler-rt`): a `target_version`
//! default version and a `target_clones` function are ordinary
//! functions, a non-default `target_version` is hidden (not declared, its
//! body skipped unparsed), and `cpu_dispatch` / `cpu_specific` name no
//! valid CPU. On x86-64, where clang always multiversions, every
//! multiversioning form is the hard error "function multiversioning with
//! 'A' is not supported yet": kiln never silently picks one version.
//! spec/landed/codegen/WP-810.md.

use kiln_abi::target::Arch;
use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, DeclId, StringLitId};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::decl::AttrArg;

/// The FMV extension names `target_version` accepts on `AArch64` at
/// clang 19 (`AArch64TargetInfo::validateCpuSupports`, LLVM 19
/// `AArch64TargetParser.h` `Extensions` with an FMV priority).
const FMV_19: &[&str] = &[
    "rng",
    "flagm",
    "flagm2",
    "fp16fml",
    "dotprod",
    "sm4",
    "rdm",
    "lse",
    "fp",
    "simd",
    "crc",
    "sha1",
    "sha2",
    "sha3",
    "aes",
    "pmull",
    "fp16",
    "dit",
    "dpb",
    "dpb2",
    "jscvt",
    "fcma",
    "rcpc",
    "rcpc2",
    "rcpc3",
    "frintts",
    "dgh",
    "i8mm",
    "bf16",
    "ebf16",
    "rpres",
    "sve",
    "sve-bf16",
    "sve-ebf16",
    "sve-i8mm",
    "f32mm",
    "f64mm",
    "sve2",
    "sve2-aes",
    "sve2-pmull128",
    "sve2-bitperm",
    "sve2-sha3",
    "sve2-sm4",
    "sme",
    "memtag",
    "memtag2",
    "memtag3",
    "sb",
    "predres",
    "ssbs",
    "ssbs2",
    "bti",
    "ls64",
    "ls64_v",
    "ls64_accdata",
    "wfxt",
    "sme-f64f64",
    "sme-i16i64",
    "sme2",
    "mops",
];

/// Likewise at clang 22 (LLVM 22 `AArch64FMV.td`).
const FMV_22: &[&str] = &[
    "rng",
    "flagm",
    "flagm2",
    "fp16fml",
    "dotprod",
    "sm4",
    "rdm",
    "lse",
    "fp",
    "simd",
    "crc",
    "sha2",
    "sha3",
    "aes",
    "fp16",
    "dit",
    "dpb",
    "dpb2",
    "jscvt",
    "fcma",
    "rcpc",
    "rcpc2",
    "rcpc3",
    "frintts",
    "i8mm",
    "bf16",
    "sve",
    "f32mm",
    "f64mm",
    "sve2",
    "sve2-aes",
    "sve2-bitperm",
    "sve2-sha3",
    "sve2-sm4",
    "sme",
    "memtag",
    "sb",
    "ssbs",
    "bti",
    "wfxt",
    "sme-f64f64",
    "sme-i16i64",
    "sme2",
    "mops",
    "cssc",
];

/// Whether `k` is one of the mutually exclusive multiversioning kinds.
fn is_fmv_kind(k: AttrKind) -> bool {
    matches!(
        k,
        AttrKind::TARGET
            | AttrKind::TARGET_VERSION
            | AttrKind::TARGET_CLONES
            | AttrKind::CPU_DISPATCH
            | AttrKind::CPU_SPECIFIC
    )
}

/// The canonical name of multiversioning kind `k`.
fn fmv_name(k: AttrKind) -> &'static str {
    match k {
        AttrKind::TARGET_VERSION => "target_version",
        AttrKind::TARGET_CLONES => "target_clones",
        AttrKind::CPU_DISPATCH => "cpu_dispatch",
        AttrKind::CPU_SPECIFIC => "cpu_specific",
        _ => "target",
    }
}

impl Sema<'_, '_> {
    fn is_aarch64(&self) -> bool {
        self.abi.triple.arch == Arch::AArch64
    }

    /// The text of the string literal `a` of a `target` /
    /// `target_version` attribute.
    fn fmv_string(&self, node: &AttrNode) -> String {
        StringLitId::from_raw(node.a).map_or_else(String::new, |l| {
            String::from_utf8_lossy(self.ast.strings.bytes(l)).into_owned()
        })
    }

    /// Whether a `target_version` node names the default version (clang
    /// `TargetVersionAttr::isDefaultVersion`: the trimmed string is
    /// `"default"`).
    fn is_default_version(&self, node: &AttrNode) -> bool {
        self.fmv_string(node).trim() == "default"
    }

    /// The attribute `scope::n` (`n` without `__` affixes) when it is a
    /// multiversioning attribute: clang `GCC<"target_version">` (an
    /// `AArch64`-only attribute from clang 20; clang 19 knows it on every
    /// target), `GCC<"target_clones">`, `Clang<"cpu_dispatch">`,
    /// `Clang<"cpu_specific">`. `None` for any other attribute; else the
    /// attribute, or `Some(None)` after an argument error (clang
    /// `handleTargetVersionAttr`, `handleTargetClonesAttr`,
    /// `handleCPUSpecificAttr` and the generic argument-count check).
    #[allow(clippy::option_option, reason = "not ours / dropped / attached, as the caller's match")]
    pub(crate) fn fmv_attribute(
        &mut self,
        n: &str,
        scope: Option<&str>,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<Option<AttrId>> {
        let plain = syntax == AttrSyntax::Gnu && scope.is_none();
        let gnu = plain || scope == Some("gnu");
        let clang = plain || scope == Some("clang");
        let kind = match n {
            "target_version" if gnu && (self.is_aarch64() || self.clang_compat < 22) => {
                AttrKind::TARGET_VERSION
            }
            "target_clones" if gnu => AttrKind::TARGET_CLONES,
            "cpu_dispatch" if clang => AttrKind::CPU_DISPATCH,
            "cpu_specific" if clang => AttrKind::CPU_SPECIFIC,
            _ => return None,
        };
        let shown = format!("'{}'", fmv_name(kind));
        let string_at = |s: &Self, arg: &AttrArg| match arg {
            AttrArg::Expr(e) => {
                s.string_lit_of(*e).map(|l| (l, s.begin_loc(*e))).ok_or(s.begin_loc(*e))
            }
            AttrArg::Ident(_, at) => Err(*at),
            AttrArg::Type(_) => Err(loc),
        };
        let mut node = AttrNode { kind, syntax, flags: 0, loc, a: 0, b: 0 };
        match kind {
            AttrKind::TARGET_VERSION => {
                let [arg] = args else {
                    let msg = self
                        .diag(DiagId::parse_attribute_wrong_arg_count, loc)
                        .arg("target_version")
                        .arg("1");
                    self.emit(msg);
                    return Some(None);
                };
                match string_at(self, arg) {
                    Ok((lit, at)) => {
                        node.a = self.ast.exprs.string_lit(lit).raw();
                        node.b = at.to_raw();
                    }
                    Err(at) => {
                        let msg = self
                            .diag(DiagId::parse_attribute_requires_string, at)
                            .arg("target_version");
                        self.emit(msg);
                        return Some(None);
                    }
                }
            }
            AttrKind::TARGET_CLONES => {
                for arg in args {
                    if let Err(at) = string_at(self, arg) {
                        let msg = self
                            .diag(DiagId::parse_attribute_requires_string, at)
                            .arg("target_clones");
                        self.emit(msg);
                        return Some(None);
                    }
                }
            }
            _ => {
                let Some(first) = args.first() else {
                    let msg =
                        self.diag(DiagId::sema_attribute_too_few_arguments_one, loc).arg(shown);
                    self.emit(msg);
                    return Some(None);
                };
                let AttrArg::Ident(sym, at) = *first else {
                    let msg = self.diag(DiagId::sema_attribute_requires_identifier, loc).arg(shown);
                    self.emit(msg);
                    return Some(None);
                };
                if args.iter().any(|a| !matches!(a, AttrArg::Ident(..))) {
                    let msg = self.diag(DiagId::sema_attribute_requires_identifier, loc).arg(shown);
                    self.emit(msg);
                    return Some(None);
                }
                node.a = sym.index();
                node.b = at.to_raw();
            }
        }
        Some(Some(self.ast.attrs.push(node)))
    }

    /// The multiversioning attributes among `attrs` that attach, in
    /// order, to a declaration (`is_fn`: a function); with `emit`, each
    /// dropped one is diagnosed. clang processes them in source order:
    /// the subject check (an error for `target`, `target_version` and
    /// `target_clones`; a warning for `cpu_*`), then the mutual
    /// exclusion (a new attribute conflicts with an earlier attached
    /// `target` or `target_version` of another kind on the same
    /// declaration; an earlier `target_clones` never conflicts), then the
    /// kind's own check.
    pub(crate) fn fmv_attached(
        &mut self,
        is_fn: bool,
        attrs: &[AttrId],
        emit: bool,
    ) -> Vec<AttrId> {
        let mut kept: Vec<AttrId> = Vec::new();
        for &a in attrs {
            let node = *self.ast.attrs.get(a);
            if !is_fmv_kind(node.kind) {
                continue;
            }
            let name = fmv_name(node.kind);
            if !is_fn {
                if emit {
                    let id = if matches!(node.kind, AttrKind::CPU_DISPATCH | AttrKind::CPU_SPECIFIC)
                    {
                        DiagId::sema_attribute_wrong_decl_type
                    } else {
                        DiagId::sema_attribute_wrong_decl_type_error
                    };
                    let shown = format!("'{}'", self.attr_shown(&node, name));
                    let msg = self.diag(id, node.loc).arg(shown).arg("functions");
                    self.emit(msg);
                }
                continue;
            }
            let earlier = kept.iter().map(|&k| *self.ast.attrs.get(k)).find(|e| {
                matches!(e.kind, AttrKind::TARGET | AttrKind::TARGET_VERSION) && e.kind != node.kind
            });
            if let Some(e) = earlier {
                if emit {
                    let conflict = self.diag(DiagId::note_conflicting_attribute, e.loc);
                    let msg = self
                        .diag(DiagId::sema_attributes_not_compatible, node.loc)
                        .arg(self.attr_shown(&node, name))
                        .arg(self.attr_shown(&e, fmv_name(e.kind)))
                        .with_note(conflict);
                    self.emit(msg);
                }
                continue;
            }
            let ok = match node.kind {
                AttrKind::TARGET => true,
                AttrKind::TARGET_VERSION if self.is_aarch64() => {
                    self.check_target_version_string(&node, emit)
                }
                AttrKind::TARGET_CLONES if self.is_aarch64() => true,
                AttrKind::CPU_DISPATCH | AttrKind::CPU_SPECIFIC if self.is_aarch64() => {
                    // clang `TargetInfo::validateCPUSpecificCPUDispatch`:
                    // no CPU name is valid off x86.
                    if emit {
                        let cpu = self.ast.symbols().as_str(Symbol::from_index(node.a)).to_owned();
                        let msg = self
                            .diag(
                                DiagId::sema_invalid_cpu_specific_dispatch_value,
                                Loc::from_raw(node.b),
                            )
                            .arg(cpu)
                            .arg(name);
                        self.emit(msg);
                    }
                    false
                }
                _ => {
                    if emit {
                        let msg =
                            self.diag(DiagId::sema_multiversion_not_supported, node.loc).arg(name);
                        self.emit(msg);
                    }
                    false
                }
            };
            if ok {
                kept.push(a);
            }
        }
        kept
    }

    /// clang `checkTargetVersionAttr`: each `+`-separated, trimmed part of
    /// the string is `default` or an FMV extension name; else the
    /// attribute is ignored with a warning at the literal.
    fn check_target_version_string(&mut self, node: &AttrNode, emit: bool) -> bool {
        let list = if self.clang_compat < 22 { FMV_19 } else { FMV_22 };
        let s = self.fmv_string(node);
        let Some(bad) = s.split('+').map(str::trim).find(|p| *p != "default" && !list.contains(p))
        else {
            return true;
        };
        if emit {
            let msg = self
                .diag(DiagId::sema_unsupported_fmv_feature, Loc::from_raw(node.b))
                .arg(bad.to_owned());
            self.emit(msg);
        }
        false
    }

    /// The multiversioning checks of declaration `d`
    /// ([`Sema::fmv_attached`]), dropping the attributes that do not
    /// attach. On x86-64 a `target("default")` or a `target` string that
    /// differs from an earlier declaration's makes the function
    /// multiversioned (clang `CheckTargetCausesMultiVersioning`): the hard
    /// error.
    pub(crate) fn check_fmv_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        if !attrs.iter().any(|&a| is_fmv_kind(self.ast.attrs.get(a).kind)) {
            return;
        }
        let is_fn = self.ast.decls.get(d).kind().is_function();
        let kept = self.fmv_attached(is_fn, attrs, true);
        attrs.retain(|a| kept.contains(a) || !is_fmv_kind(self.ast.attrs.get(*a).kind));
        if self.is_aarch64() {
            // clang 22 `checkNewAttributesAfterDef`: a default version
            // added after the definition is ignored with a warning.
            let def = self.ast.prev(d).and_then(|p| self.ast.definition(p)).filter(|&f| f != d);
            if let Some(def) = def.filter(|_| self.clang_compat >= 22) {
                let v = attrs
                    .iter()
                    .copied()
                    .find(|&a| self.ast.attrs.get(a).kind == AttrKind::TARGET_VERSION);
                if let Some(v) = v {
                    let loc = self.ast.attrs.get(v).loc;
                    let prev_def =
                        self.diag(DiagId::note_previous_definition, self.ast.decls.get(def).loc());
                    let msg = self
                        .diag(DiagId::sema_attribute_precede_definition, loc)
                        .with_note(prev_def);
                    self.emit(msg);
                    attrs.retain(|&a| a != v);
                }
            }
            return;
        }
        let Some(t) =
            attrs.iter().map(|&a| *self.ast.attrs.get(a)).find(|n| n.kind == AttrKind::TARGET)
        else {
            return;
        };
        let s = self.fmv_string(&t);
        let differs = self.ast.redecls_of(d).filter(|&r| r != d).any(|r| {
            self.ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(r))
                .map(|a| *self.ast.attrs.get(a))
                .any(|n| n.kind == AttrKind::TARGET && self.fmv_string(&n) != s)
        });
        if s == "default" || differs {
            let msg = self.diag(DiagId::sema_multiversion_not_supported, t.loc).arg("target");
            self.emit(msg);
            attrs.retain(|&a| self.ast.attrs.get(a).kind != AttrKind::TARGET);
        }
    }

    /// Whether the function declarator with attributes `attrs` is a
    /// non-default `target_version` version, which clang without `fmv`
    /// hides (`AArch64` only): the location of its `target_version`. The
    /// caller diagnoses the attributes ([`Sema::fmv_attached`]), as the
    /// declaration is never built.
    pub(crate) fn fmv_hidden_version(&mut self, attrs: &[AttrId]) -> Option<Loc> {
        if !self.is_aarch64()
            || !attrs.iter().any(|&a| self.ast.attrs.get(a).kind == AttrKind::TARGET_VERSION)
        {
            return None;
        }
        let kept = self.fmv_attached(true, attrs, false);
        let tv = kept
            .iter()
            .map(|&a| *self.ast.attrs.get(a))
            .find(|n| n.kind == AttrKind::TARGET_VERSION)?;
        (!self.is_default_version(&tv)).then_some(tv.loc)
    }
}
