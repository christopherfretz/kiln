//! x86-64 function multiversioning (WP-1745; clang
//! `CheckMultiVersionFunction` and its helpers in `SemaDecl.cpp`).
//!
//! A function declaration whose multiversioning attribute (`target`,
//! `target_clones`, `cpu_specific`, `cpu_dispatch`) makes it a new version
//! of an earlier function of its name and type is a function of its own,
//! not a redeclaration ([`Sema::fmv_x86_redecl`] answers which, before the
//! declaration is built); [`kiln_ast::Ast::is_multiversion`] then tells
//! the versions apart. Overload resolution takes no non-default `target`
//! version ([`Sema::is_nonviable_multiversion`]), and codegen emits the
//! versions and their `ifunc` resolver. spec/sema.md "Function
//! multiversioning".

use kiln_abi::x86_fmv;
use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::decl::{ConstexprKind, DeclFlags, FnSpec, FnStorage};
use kiln_ast::multiversion::MvKind;
use kiln_ast::types::{ExcSpec, NoexceptValue};
use kiln_ast::{AttrId, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::decl::AttrArg;

/// What [`Sema::fmv_x86_redecl`] knows of the function being declared.
pub(crate) struct FmvNew {
    /// Its type.
    pub ty: QualType,
    /// Its name's location.
    pub loc: Loc,
    /// Its specifiers.
    pub spec: FnSpec,
    /// It is `::main`.
    pub is_main: bool,
    /// It is a member function (`static` does not give it internal
    /// linkage).
    pub member: bool,
    /// It is a function template.
    pub template: bool,
    /// A constructor (3) or destructor (4), as clang's `DoesntSupport`.
    pub structor: Option<u8>,
}

/// A parsed `target` string (clang `ParsedTargetAttr`, compared whole).
#[derive(PartialEq, Eq)]
struct ParsedTarget {
    cpu: Option<String>,
    tune: Option<String>,
    features: Vec<String>,
}

/// clang `X86TargetInfo::parseTargetAttr`: `arch=` / `tune=` items, and
/// `no-<f>` / `<f>` as `-<f>` / `+<f>`, sorted.
fn parse_target(s: &str) -> ParsedTarget {
    let mut p = ParsedTarget { cpu: None, tune: None, features: Vec::new() };
    for item in s.split(',').map(str::trim) {
        if let Some(c) = item.strip_prefix("arch=") {
            p.cpu = Some(c.trim().to_owned());
        } else if let Some(t) = item.strip_prefix("tune=") {
            p.tune = Some(t.trim().to_owned());
        } else if let Some(f) = item.strip_prefix("no-") {
            p.features.push(format!("-{f}"));
        } else if !item.is_empty() && !item.starts_with("fpmath=") {
            p.features.push(format!("+{item}"));
        }
    }
    p.features.sort();
    p
}

/// The `DoesntSupport` reasons of `err_multiversion_doesnt_support`.
const TEMPLATES: u8 = 0;
const VIRTUAL: u8 = 1;
const DEDUCED: u8 = 2;
const CONSTEXPR: u8 = 7;
const CONSTEVAL: u8 = 8;

/// The `Different` aspects of `err_multiversion_diff`.
const RETURN_TYPE: u8 = 1;
const CONSTEXPR_SPEC: u8 = 2;
const INLINE_SPEC: u8 = 3;
const LINKAGE: u8 = 4;

/// The attribute name of multiversioning kind `k`.
fn kind_name(k: MvKind) -> &'static str {
    match k {
        MvKind::None => "",
        MvKind::Target => "target",
        MvKind::Clones => "target_clones",
        MvKind::CpuSpecific => "cpu_specific",
        MvKind::CpuDispatch => "cpu_dispatch",
    }
}

/// The outcome of [`Sema::fmv_x86_redecl`].
pub(crate) enum FmvRedecl {
    /// Redeclares this function (or declares a new one).
    Prev(Option<DeclId>),
    /// Diagnosed: the declaration is invalid.
    Invalid,
}

impl Sema<'_, '_> {
    /// Whether the target multiversions on x86-64 rules.
    pub(crate) fn is_x86_mv(&self) -> bool {
        self.abi.triple.arch == kiln_abi::target::Arch::X86_64
    }

    /// The multiversioning attributes of a declarator's `attrs` that
    /// attach ([`Sema::fmv_attached`], undiagnosed) and its kind.
    fn new_mv(&mut self, attrs: &[AttrId]) -> (MvKind, Vec<AttrNode>) {
        let kept: Vec<AttrNode> = self
            .fmv_attached(true, attrs, false)
            .into_iter()
            .map(|a| *self.ast.attrs.get(a))
            .filter(|n| n.kind != AttrKind::TARGET_VERSION)
            .collect();
        let has = |k| kept.iter().any(|n| n.kind == k);
        let kind = if has(AttrKind::CPU_DISPATCH) {
            MvKind::CpuDispatch
        } else if has(AttrKind::CPU_SPECIFIC) {
            MvKind::CpuSpecific
        } else if has(AttrKind::TARGET_CLONES) {
            MvKind::Clones
        } else if has(AttrKind::TARGET) {
            MvKind::Target
        } else {
            MvKind::None
        };
        (kind, kept)
    }

    /// The comma-separated list a `target_clones` / `cpu_*` node keeps.
    fn mv_list(&self, n: &AttrNode) -> Vec<String> {
        self.ast.mv_attr_list(n)
    }

    /// clang `CheckMultiVersionFunction` for a function declaration with
    /// attributes `attrs` whose redeclaration lookup found `prev`: the
    /// declaration it redeclares (`prev`, another version of the same
    /// multiversioned function, or none: a new version), or invalid
    /// after an error.
    pub(crate) fn fmv_x86_redecl(
        &mut self,
        prev: Option<DeclId>,
        attrs: &[AttrId],
        new: &FmvNew,
    ) -> FmvRedecl {
        let (kind, kept) = self.new_mv(attrs);
        let target = kept.iter().find(|n| n.kind == AttrKind::TARGET).map(|n| self.fmv_text(n));
        let is_default = target.as_deref() == Some("default");
        if new.is_main {
            if kind != MvKind::None && (kind != MvKind::Target || is_default) {
                self.error(DiagId::sema_multiversion_not_allowed_on_main, new.loc);
                return FmvRedecl::Invalid;
            }
            return FmvRedecl::Prev(prev);
        }
        let old = prev.filter(|&p| self.ast.decls.get(p).kind().is_function());
        let Some(old) = old else {
            if kind == MvKind::None || (kind == MvKind::Target && !is_default) {
                return FmvRedecl::Prev(prev);
            }
            // clang `CheckMultiVersionFirstFunction`.
            if kind == MvKind::Target && self.bad_mv_value(target.as_deref(), new.loc) {
                return FmvRedecl::Invalid;
            }
            if self.mv_rules(None, attrs, new, kind, true) {
                return FmvRedecl::Invalid;
            }
            return FmvRedecl::Prev(prev);
        };
        let old_mv = self.ast.is_multiversion(old);
        let old_kind = self.ast.mv_kind(old);
        if !old_mv && kind == MvKind::None {
            return FmvRedecl::Prev(prev);
        }
        if old_mv && kind == MvKind::None && old_kind != MvKind::Clones {
            let which = u8::from(old_kind != MvKind::Target);
            let msg = self.diag(DiagId::sema_multiversion_required_in_redecl, new.loc).arg(which);
            self.emit(msg);
            return FmvRedecl::Invalid;
        }
        if !old_mv {
            match kind {
                MvKind::Target => {
                    return self.target_causes_mv(old, attrs, new, target.as_deref());
                }
                MvKind::Clones if self.ast.redecls_of(old).any(|r| self.is_used(r)) => {
                    self.error(DiagId::sema_multiversion_after_used, new.loc);
                    return FmvRedecl::Invalid;
                }
                _ => {}
            }
        }
        self.mv_additional_decl(old, old_mv, attrs, new, kind, &kept, target.as_deref())
    }

    /// Whether declaration `d` was odr-used (clang `isUsed(false)`).
    fn is_used(&self, d: DeclId) -> bool {
        self.ast.decls.get(d).has(DeclFlags::USED)
    }

    /// clang `CheckDeclarationCausesMultiVersioning` for a `target`
    /// declaration of the not yet multiversioned function `old`.
    fn target_causes_mv(
        &mut self,
        old: DeclId,
        attrs: &[AttrId],
        new: &FmvNew,
        target: Option<&str>,
    ) -> FmvRedecl {
        let s = target.unwrap_or_default();
        let old_t = self.ast.mv_attr(old, AttrKind::TARGET).map(|n| self.fmv_text(&n));
        if s != "default" && old_t.as_deref().is_none_or(|o| o == s) {
            return FmvRedecl::Prev(Some(old));
        }
        if self.mv_rules(Some(old), attrs, new, MvKind::Target, true) {
            return FmvRedecl::Invalid;
        }
        if self.bad_mv_value(Some(s), new.loc) {
            return FmvRedecl::Invalid;
        }
        let Some(old_t) = old_t else {
            // A `target("default")` redeclaration makes `old` the default.
            return FmvRedecl::Prev(Some(old));
        };
        if self.bad_mv_value(Some(&old_t), self.ast.decls.get(old).loc()) {
            let note = self.diag(DiagId::note_multiversioning_caused_here, new.loc);
            self.emit(note);
            return FmvRedecl::Invalid;
        }
        if parse_target(&old_t) == parse_target(s) {
            let msg = self.diag(DiagId::sema_multiversion_duplicate, new.loc);
            let msg = self.note_prev(msg, old, false);
            self.emit(msg);
            return FmvRedecl::Invalid;
        }
        // Each declaration of `old` after its first `target` one has one.
        let chain: Vec<DeclId> = self.ast.redecls_of(old).collect();
        for (i, &r) in chain.iter().enumerate().rev() {
            let earlier = chain[i + 1..].iter().any(|&e| {
                [AttrKind::TARGET, AttrKind::TARGET_CLONES, AttrKind::CPU_SPECIFIC]
                    .into_iter()
                    .chain([AttrKind::CPU_DISPATCH])
                    .any(|k| self.has_own_attr(e, k))
            });
            if earlier && !self.has_own_attr(r, AttrKind::TARGET) {
                let loc = self.ast.decls.get(r).loc();
                let caused = self.diag(DiagId::note_multiversioning_caused_here, new.loc);
                let msg = self
                    .diag(DiagId::sema_multiversion_required_in_redecl, loc)
                    .arg(0u8)
                    .with_note(caused);
                self.emit(msg);
                return FmvRedecl::Invalid;
            }
        }
        FmvRedecl::Prev(None)
    }

    /// Whether declaration `d` itself carries an attribute of kind `k`
    /// (not one inherited from an earlier declaration).
    fn has_own_attr(&self, d: DeclId, k: AttrKind) -> bool {
        self.ast.lists.iter::<AttrId>(self.ast.attrs.of_decl(d)).any(|a| {
            let n = self.ast.attrs.get(a);
            n.kind == k && n.flags & 2 == 0
        })
    }

    /// clang `CheckMultiVersionAdditionalDecl`.
    #[allow(clippy::too_many_arguments, reason = "clang's helper, one value each")]
    fn mv_additional_decl(
        &mut self,
        old: DeclId,
        old_mv: bool,
        attrs: &[AttrId],
        new: &FmvNew,
        kind: MvKind,
        kept: &[AttrNode],
        target: Option<&str>,
    ) -> FmvRedecl {
        let old_kind = self.ast.mv_kind(old);
        let compatible = old_kind == kind
            || old_kind == MvKind::None
            || kind == MvKind::None
            || matches!(
                (old_kind, kind),
                (MvKind::CpuDispatch, MvKind::CpuSpecific)
                    | (MvKind::CpuSpecific, MvKind::CpuDispatch)
            );
        if !compatible {
            let msg = self.diag(DiagId::sema_multiversion_types_mixed, new.loc);
            let msg = self.note_prev(msg, old, false);
            self.emit(msg);
            return FmvRedecl::Invalid;
        }
        let own = |k| kept.iter().find(|n| n.kind == k).copied();
        let new_list = own(AttrKind::TARGET_CLONES)
            .or_else(|| own(AttrKind::CPU_DISPATCH))
            .or_else(|| own(AttrKind::CPU_SPECIFIC))
            .map(|n| self.mv_list(&n))
            .unwrap_or_default();
        let new_parsed = target.map(parse_target);
        for cur in self.ast.same_signature_fns(old) {
            if self.ast.decls.get(cur).has(DeclFlags::INVALID) {
                continue;
            }
            match kind {
                MvKind::None => {}
                MvKind::Target => {
                    let Some(ct) = self.ast.mv_attr(cur, AttrKind::TARGET) else { continue };
                    let ct = self.fmv_text(&ct);
                    if Some(ct.as_str()) == target {
                        return FmvRedecl::Prev(Some(cur));
                    }
                    if new_parsed.as_ref() == Some(&parse_target(&ct)) {
                        let msg = self.diag(DiagId::sema_multiversion_duplicate, new.loc);
                        let msg = self.note_prev(msg, cur, false);
                        self.emit(msg);
                        return FmvRedecl::Invalid;
                    }
                }
                MvKind::Clones => {
                    if let Some(c) = self.ast.mv_attr(cur, AttrKind::TARGET_CLONES)
                        && self.mv_list(&c) != new_list
                    {
                        let msg = self.diag(DiagId::sema_target_clone_doesnt_match, new.loc);
                        let msg = self.note_prev(msg, cur, false);
                        self.emit(msg);
                        return FmvRedecl::Invalid;
                    }
                    return FmvRedecl::Prev(Some(cur));
                }
                MvKind::CpuDispatch | MvKind::CpuSpecific => {
                    let ak = if kind == MvKind::CpuDispatch {
                        AttrKind::CPU_DISPATCH
                    } else {
                        AttrKind::CPU_SPECIFIC
                    };
                    let Some(c) = self.ast.mv_attr(cur, ak) else { continue };
                    let cur_list = self.mv_list(&c);
                    if cur_list == new_list {
                        return FmvRedecl::Prev(Some(cur));
                    }
                    if kind == MvKind::CpuDispatch {
                        let msg = self.diag(DiagId::sema_cpu_dispatch_mismatch, new.loc);
                        let msg = self.note_prev(msg, cur, false);
                        self.emit(msg);
                        return FmvRedecl::Invalid;
                    }
                    if let Some(cpu) = new_list.iter().find(|c| cur_list.contains(c)) {
                        let msg = self
                            .diag(DiagId::sema_cpu_specific_multiple_defs, new.loc)
                            .arg(cpu.clone());
                        let msg = self.note_prev(msg, cur, false);
                        self.emit(msg);
                        return FmvRedecl::Invalid;
                    }
                }
            }
        }
        if kind == MvKind::Target && self.bad_mv_value(target, new.loc) {
            return FmvRedecl::Invalid;
        }
        if self.mv_rules(Some(old), attrs, new, kind, !old_mv) {
            return FmvRedecl::Invalid;
        }
        if old_mv { FmvRedecl::Prev(None) } else { FmvRedecl::Prev(Some(old)) }
    }

    /// clang `CheckMultiVersionValue` for `target` string `s`: an `arch=`
    /// CPU must be a `__builtin_cpu_is` name, and each feature one with
    /// an FMV priority; else the error at `loc`.
    fn bad_mv_value(&mut self, s: Option<&str>, loc: Loc) -> bool {
        let Some(s) = s.filter(|s| *s != "default") else { return false };
        let p = parse_target(s);
        let bad = if let Some(c) = p.cpu.filter(|c| x86_fmv::cpu_is(c).is_none()) {
            Some((1u8, c))
        } else {
            let mut items = s
                .split(',')
                .map(str::trim)
                .filter(|i| !i.is_empty() && !i.starts_with("arch=") && !i.starts_with("tune="));
            items
                .find(|i| {
                    i.starts_with("no-")
                        || x86_fmv::fmv_priority(i) == 0
                        || x86_fmv::cpu_supports(i).is_none()
                })
                .map(|i| (0u8, i.to_owned()))
        };
        let Some((which, name)) = bad else { return false };
        let msg = self.diag(DiagId::sema_bad_multiversion_option, loc).arg(which).arg(name);
        self.emit(msg);
        true
    }

    /// clang `CheckMultiVersionAdditionalRules` (with
    /// `checkNonMultiVersionCompatAttributes` and
    /// `areMultiversionVariantFunctionsCompatible`) for the new
    /// declaration (attributes `attrs`) of kind `kind` beside `old`;
    /// `causes`: it makes `old` multiversioned.
    fn mv_rules(
        &mut self,
        old: Option<DeclId>,
        attrs: &[AttrId],
        new: &FmvNew,
        kind: MvKind,
        causes: bool,
    ) -> bool {
        if causes && let Some(o) = old {
            let old_attrs: Vec<AttrId> = self
                .ast
                .redecls_of(o)
                .flat_map(|r| {
                    self.ast.lists.iter::<AttrId>(self.ast.attrs.of_decl(r)).collect::<Vec<_>>()
                })
                .collect();
            let at = self.ast.decls.get(o).loc();
            if self.other_attr_error(&old_attrs, kind, at, Some(new.loc)) {
                return true;
            }
        }
        if self.other_attr_error(attrs, kind, new.loc, None) {
            return true;
        }
        if causes
            && let Some(o) = old
            && self.ast.redecls_of(o).any(|r| self.is_used(r))
        {
            let msg = self.diag(DiagId::sema_multiversion_after_used, new.loc);
            // Clang 19 has no note.
            let msg = if self.clang_compat >= 22 { self.note_prev(msg, o, false) } else { msg };
            self.emit(msg);
            return true;
        }
        let cpu = matches!(kind, MvKind::CpuDispatch | MvKind::CpuSpecific);
        let ck = new.spec.constexpr_kind();
        let reason = if new.template {
            Some(TEMPLATES)
        } else if new.spec.is_virtual() {
            Some(VIRTUAL)
        } else if let Some(s) = new.structor {
            Some(s)
        } else if cpu && ck != ConstexprKind::None {
            Some(if ck == ConstexprKind::Consteval { CONSTEVAL } else { CONSTEXPR })
        } else if self
            .ast
            .types
            .proto(new.ty)
            .is_some_and(|p| self.contains_undeduced_auto(p.ret()))
        {
            Some(DEDUCED)
        } else {
            None
        };
        if let Some(r) = reason {
            let msg = self
                .diag(DiagId::sema_multiversion_doesnt_support, new.loc)
                .arg(kind_name(kind))
                .arg(r);
            self.emit(msg);
            return true;
        }
        let Some(o) = old else { return false };
        let ospec = self.fn_spec(o);
        let ret = |s: &Self, t: Option<QualType>| {
            t.and_then(|t| s.ast.types.proto(t)).map(|p| s.ast.types.canonical(p.ret()))
        };
        let old_internal = self.has_internal_linkage(o);
        let ctx_internal = old_internal && ospec.storage() != FnStorage::Static;
        let new_internal = if new.member {
            old_internal
        } else {
            new.spec.storage() == FnStorage::Static || ctx_internal
        };
        let diff = if ret(self, self.ast.decls.get(o).ty()) != ret(self, Some(new.ty)) {
            Some(RETURN_TYPE)
        } else if ospec.constexpr_kind() != ck {
            Some(CONSTEXPR_SPEC)
        } else if ospec.is_inline() != new.spec.is_inline() {
            Some(INLINE_SPEC)
        } else if old_internal != new_internal {
            Some(LINKAGE)
        } else {
            None
        };
        if let Some(d) = diff {
            let msg = self.diag(DiagId::sema_multiversion_diff, new.loc).arg(d);
            self.emit(msg);
            return true;
        }
        // clang `CheckEquivalentExceptionSpec` on the two versions' types.
        if let Some(ot) = self.ast.decls.get(o).ty() {
            let (oe, ne) = (self.written_exc_spec(ot), self.written_exc_spec(new.ty));
            let known = |e: ExcSpec| {
                !matches!(
                    e,
                    ExcSpec::Deferred(_)
                        | ExcSpec::DependentNoexcept(_)
                        | ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(_), .. }
                )
            };
            if known(oe) && known(ne) && oe.is_nothrow() != ne.is_nothrow() {
                let msg = self.diag(DiagId::sema_exception_spec_mismatch, new.loc);
                let msg = self.note_prev(msg, o, false);
                self.emit(msg);
                return true;
            }
        }
        false
    }

    /// clang `checkNonMultiVersionCompatAttributes`: an attribute among
    /// `attrs` other than `used` (with `target`), `nonnull`, `nothrow` and
    /// the multiversioning attributes of kind `kind` is an error at `at`
    /// (noting `caused`).
    fn other_attr_error(
        &mut self,
        attrs: &[AttrId],
        kind: MvKind,
        at: Loc,
        caused: Option<Loc>,
    ) -> bool {
        let cpu = matches!(kind, MvKind::CpuDispatch | MvKind::CpuSpecific);
        for &a in attrs {
            let n = *self.ast.attrs.get(a);
            if n.flags & AttrNode::IMPLICIT != 0 {
                continue;
            }
            let ok = match n.kind {
                AttrKind::CPU_DISPATCH | AttrKind::CPU_SPECIFIC => cpu,
                AttrKind::TARGET | AttrKind::USED => kind == MvKind::Target,
                AttrKind::TARGET_CLONES | AttrKind::TARGET_VERSION => kind == MvKind::Clones,
                AttrKind::NONNULL | AttrKind::NOTHROW => true,
                _ => false,
            };
            if ok {
                continue;
            }
            let Some(name) = self.attr_spelled(&n) else { continue };
            let mut msg = self
                .diag(DiagId::sema_multiversion_disallowed_other_attr, at)
                .arg(kind_name(kind))
                .arg(name);
            if let Some(c) = caused {
                msg = msg.with_note(self.diag(DiagId::note_multiversioning_caused_here, c));
            }
            self.emit(msg);
            return true;
        }
        false
    }

    /// The name of attribute `n` as written (no scope), if known.
    fn attr_spelled(&self, n: &AttrNode) -> Option<String> {
        if let Some(&(_, name)) = self.attr_spellings.get(&n.loc) {
            let raw = self.ast.symbols().as_str(name);
            return Some(kiln_ast::attr::normalize_attr_name(raw).to_owned());
        }
        crate::decl_attrs::attr_name(n).map(str::to_owned)
    }

    /// clang `isNonViableMultiVersionOverload` on x86-64: a non-default
    /// `target` version of a multiversioned function is not a candidate
    /// (the call goes to the default one, which dispatches).
    pub(crate) fn is_nonviable_multiversion(&self, f: DeclId) -> bool {
        self.is_x86_mv()
            && self.ast.attrs.has_kind(AttrKind::TARGET)
            && self.ast.mv_kind(f) == MvKind::Target
            && !self.ast.is_target_default(f)
            && self.ast.is_multiversion(f)
    }

    /// Whether `f` is an x86-64 `cpu_dispatch` function, whose body is
    /// ignored: clang does not check that it returns a value.
    pub(crate) fn is_x86_cpu_dispatch(&self, f: DeclId) -> bool {
        self.is_x86_mv()
            && self.ast.attrs.has_kind(AttrKind::CPU_DISPATCH)
            && self.ast.mv_kind(f) == MvKind::CpuDispatch
    }

    /// clang `ActOnFinishFunctionBody`: the body of a `cpu_dispatch`
    /// function is ignored, a warning at its first statement.
    pub(crate) fn check_dispatch_body(&mut self, f: DeclId, body: kiln_ast::StmtId) {
        use kiln_ast::stmt::StmtKind;
        if !self.is_x86_cpu_dispatch(f) || self.ast.stmts.get(body).kind() != StmtKind::CompoundStmt
        {
            return;
        }
        let l = self.ast.stmts.compound_body(body);
        if let Some(s) = self.ast.lists.iter::<kiln_ast::StmtId>(l).next() {
            let at = self.stmt_begin_loc(s);
            self.error(DiagId::sema_dispatch_body_ignored, at);
        }
    }

    /// clang `checkTargetClonesAttrString` (x86-64) over the string
    /// literals `lits` (text, location) of a `target_clones` attribute at
    /// `loc`, then `handleTargetClonesAttr`'s checks: whether the
    /// attribute stays. A piece's location is its literal's plus its
    /// byte offset, as clang 22 reports it (clang 19: one byte later).
    pub(crate) fn x86_target_clones_ok(&mut self, lits: &[(String, Loc)], loc: Loc) -> bool {
        let (mut has_default, mut has_commas) = (false, false);
        let mut seen: Vec<String> = Vec::new();
        let valid_arch = |a: &str| {
            x86_fmv::FMV_ARCHES.iter().any(|e| e.0 == a)
                || matches!(a, "x86-64" | "x86-64-v2" | "x86-64-v3" | "x86-64-v4")
        };
        for (text, at) in lits {
            has_commas |= text.contains(',');
            if text.is_empty() {
                let msg = self
                    .diag(DiagId::sema_unsupported_target_clones_feature, *at)
                    .arg(String::new());
                self.emit(msg);
                return false;
            }
            let mut rest = Some((text.as_str(), 0usize));
            while let Some((r, off)) = rest.filter(|(r, _)| !r.is_empty()) {
                let (part, next) = match r.find(',') {
                    Some(i) => (&r[..i], Some((&r[i + 1..], off + i + 1))),
                    None => (r, None),
                };
                rest = next;
                let cur = part.trim();
                let lead = part.len() - part.trim_start().len();
                // Clang 19 reports a piece at its first byte, clang 22
                // one byte earlier (the literal's start for the first).
                let shift = usize::from(self.clang_compat < 22);
                let cur_loc = at.with_offset(u32::try_from(off + lead + shift).unwrap_or(0));
                let bad = if cur.is_empty() {
                    Some((DiagId::sema_unsupported_target_clones_feature, String::new()))
                } else if let Some(a) = cur.strip_prefix("arch=") {
                    (!valid_arch(a)).then(|| (DiagId::sema_unknown_target_clones_cpu, a.to_owned()))
                } else if cur == "default" {
                    None
                } else {
                    (x86_fmv::fmv_priority(cur) == 0)
                        .then(|| (DiagId::sema_unsupported_target_clones_feature, cur.to_owned()))
                };
                if let Some((id, what)) = bad {
                    let msg = self.diag(id, cur_loc).arg(what);
                    self.emit(msg);
                    return false;
                }
                let default_dupe = cur == "default" && has_default;
                has_default |= cur == "default";
                if default_dupe || seen.iter().any(|x| x == cur) {
                    self.error(DiagId::sema_target_clone_duplicate_options, cur_loc);
                }
                seen.push(cur.to_owned());
            }
            if text.trim_end().ends_with(',') {
                let msg = self
                    .diag(DiagId::sema_unsupported_target_clones_feature, *at)
                    .arg(String::new());
                self.emit(msg);
                return false;
            }
        }
        if has_commas && lits.len() > 1 {
            // Clang 19 reports it at the attribute.
            let at = if self.clang_compat < 22 { loc } else { lits[0].1 };
            self.error(DiagId::sema_target_clone_mixed_values, at);
        }
        if !has_default {
            self.error(DiagId::sema_target_clone_must_have_default, loc);
            return false;
        }
        true
    }

    /// clang `handleCPUSpecificAttr` on x86-64: every CPU of a
    /// `cpu_specific` / `cpu_dispatch` (named `name`) at `loc` is a
    /// processor, no two of one mangling letter. The list's `Symbol`
    /// index (comma-joined), or `None` once diagnosed.
    pub(crate) fn x86_cpu_list(&mut self, args: &[AttrArg], name: &str, loc: Loc) -> Option<u32> {
        let mut cpus: Vec<String> = Vec::new();
        let mut chars: Vec<char> = Vec::new();
        for a in args {
            let AttrArg::Ident(s, at) = a else { continue };
            let cpu = self.ast.symbols().as_str(*s).trim().to_owned();
            let Some(p) = x86_fmv::processor(&cpu) else {
                let msg = self
                    .diag(DiagId::sema_invalid_cpu_specific_dispatch_value, *at)
                    .arg(cpu)
                    .arg(name);
                self.emit(msg);
                return None;
            };
            if chars.contains(&p.mangling) {
                let msg = self.diag(DiagId::sema_multiversion_duplicate_entries, loc);
                self.emit(msg);
                return None;
            }
            chars.push(p.mangling);
            cpus.push(cpu);
        }
        Some(self.intern(&cpus.join(",")).index())
    }

    /// The text of a `target` node's string.
    fn fmv_text(&self, n: &AttrNode) -> String {
        self.ast.target_attr_text(n)
    }

    /// clang `isBetterMultiversionCandidate`: between two versions of a
    /// `cpu_dispatch` / `cpu_specific` function, the `cpu_dispatch` one,
    /// then the `cpu_specific` one with fewer CPUs, then the one whose
    /// first differing CPU name is less. `None`: no preference.
    pub(crate) fn better_multiversion(&self, f1: DeclId, f2: DeclId) -> Option<bool> {
        if !self.is_x86_mv() || !self.ast.attrs.has_kind(AttrKind::CPU_SPECIFIC) {
            return None;
        }
        let (k1, k2) = (self.ast.mv_kind(f1), self.ast.mv_kind(f2));
        let cpu = |k| matches!(k, MvKind::CpuDispatch | MvKind::CpuSpecific);
        if !cpu(k1) || !cpu(k2) || self.ast.first(f1) == self.ast.first(f2) {
            return None;
        }
        match (k1, k2) {
            (MvKind::CpuDispatch, MvKind::CpuSpecific) => return Some(true),
            (MvKind::CpuSpecific, MvKind::CpuDispatch) => return Some(false),
            (MvKind::CpuSpecific, MvKind::CpuSpecific) => {}
            _ => return None,
        }
        let l1 = self.mv_list(&self.ast.mv_attr(f1, AttrKind::CPU_SPECIFIC)?);
        let l2 = self.mv_list(&self.ast.mv_attr(f2, AttrKind::CPU_SPECIFIC)?);
        if l1.len() != l2.len() {
            return Some(l1.len() < l2.len());
        }
        let (a, b) = l1.iter().zip(&l2).find(|(a, b)| a != b)?;
        Some(a < b)
    }
}
