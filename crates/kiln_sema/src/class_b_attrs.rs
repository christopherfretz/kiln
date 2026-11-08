//! WP-813: clang's class B attributes (inlining and call shape, pointer
//! facts, instrumentation and hardening; spec/sema.md "Class B
//! attributes", spec/landed/codegen/WP-813.md). Each is spelled, checked
//! and diagnosed as clang 19 does (`Attr.td` spellings, `SemaDeclAttr.cpp`
//! handlers) and kept on its declaration for codegen.
//!
//! [`kind_of`] maps a spelling to its kind; [`Sema::class_b_attribute`]
//! checks the arguments (clang's handler before the subject is known);
//! [`Sema::check_class_b_attr`] the subject and what needs the
//! declaration (parameter indices, the return type).

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax, ZERO_CALL_USED_REGS_KINDS};
use kiln_ast::decl::{DeclKind, VarStorage};
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::RawId;
use kiln_ast::sanitize::SanitizerSet;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{AttrId, DeclId, ExprId, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::decl::AttrArg;

/// How an attribute is spelled (clang `Attr.td`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Spell {
    /// `GCC<"x">`: `__attribute__((x))` and `[[gnu::x]]`.
    Gcc,
    /// `Clang<"x">`: `__attribute__((x))` and `[[clang::x]]`.
    Clang,
    /// `GCC<"x">` and `CXX11<"clang", "x">` (`no_stack_protector`).
    GccClang,
    /// `GNU<"x">` only.
    Gnu,
    /// `CXX11<"omp", "x">` only.
    Omp,
    /// `CXX11<"gsl", "x">` only (WP-674).
    Gsl,
    /// `Clang<"x">`, plus `[[gnu::x]]` from clang 22 (`no_sanitize`).
    ClangGcc22,
}

/// Which targets know the attribute (clang `TargetSpecificAttr`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Any,
    X86,
}

/// One class B spelling: the name, its kind and the preset `a` word.
struct Entry {
    name: &'static str,
    kind: AttrKind,
    a: u32,
    spell: Spell,
    target: Target,
    /// The clang release that added it.
    since: u32,
}

const fn e(name: &'static str, kind: AttrKind, spell: Spell) -> Entry {
    Entry { name, kind, a: 0, spell, target: Target::Any, since: 19 }
}

const fn x86(name: &'static str, kind: AttrKind) -> Entry {
    Entry { name, kind, a: 0, spell: Gcc, target: Target::X86, since: 19 }
}

use Spell::{Clang, ClangGcc22, Gcc, GccClang, Gnu, Gsl, Omp};

/// The class B spellings, sorted by name.
const TABLE: &[Entry] = &[
    e("Owner", AttrKind::GSL_OWNER, Gsl),
    e("Pointer", AttrKind::GSL_POINTER, Gsl),
    e("align_value", AttrKind::ALIGN_VALUE, Gnu),
    e("assume", AttrKind::OMP_ASSUME, Omp),
    e("assume_aligned", AttrKind::ASSUME_ALIGNED, Gcc),
    e("callback", AttrKind::CALLBACK, Clang),
    e("cfi_canonical_jump_table", AttrKind::CFI_CANONICAL_JUMP_TABLE, Clang),
    e("code_align", AttrKind::CODE_ALIGN, Clang),
    e("convergent", AttrKind::CONVERGENT, Clang),
    Entry { since: 22, ..e("coro_await_elidable", AttrKind::CORO_AWAIT_ELIDABLE, Clang) },
    Entry {
        since: 22,
        ..e("coro_await_elidable_argument", AttrKind::CORO_AWAIT_ELIDABLE_ARGUMENT, Clang)
    },
    e("disable_sanitizer_instrumentation", AttrKind::DISABLE_SANITIZER_INSTRUMENTATION, Clang),
    e("flatten", AttrKind::FLATTEN, Gcc),
    x86("force_align_arg_pointer", AttrKind::FORCE_ALIGN_ARG_POINTER),
    x86("function_return", AttrKind::FUNCTION_RETURN),
    e("leaf", AttrKind::LEAF, Gcc),
    e("lifetimebound", AttrKind::LIFETIMEBOUND, Clang),
    e("loader_uninitialized", AttrKind::LOADER_UNINITIALIZED, Clang),
    e("lto_visibility_public", AttrKind::LTO_VISIBILITY_PUBLIC, Clang),
    e("maybe_undef", AttrKind::MAYBE_UNDEF, Clang),
    // The `no_sanitize*` family: `a` is the spelling's index in
    // `NO_SANITIZE_SPELLINGS` (WP-675).
    Entry { a: 2, ..e("no_address_safety_analysis", AttrKind::NO_SANITIZE, Gcc) },
    e("no_instrument_function", AttrKind::NO_INSTRUMENT_FUNCTION, Gcc),
    e("no_profile_instrument_function", AttrKind::NO_PROFILE_INSTRUMENT_FUNCTION, Gcc),
    e("no_sanitize", AttrKind::NO_SANITIZE, ClangGcc22),
    Entry { a: 1, ..e("no_sanitize_address", AttrKind::NO_SANITIZE, Gcc) },
    Entry { a: 3, ..e("no_sanitize_memory", AttrKind::NO_SANITIZE, Clang) },
    Entry { a: 4, ..e("no_sanitize_thread", AttrKind::NO_SANITIZE, Gcc) },
    e("no_speculative_load_hardening", AttrKind::NO_SPECULATIVE_LOAD_HARDENING, Clang),
    e("no_split_stack", AttrKind::NO_SPLIT_STACK, Gcc),
    e("no_stack_protector", AttrKind::NO_STACK_PROTECTOR, GccClang),
    // Known on every target: without `-fcf-protection` (an error off
    // x86) clang 19 warns it ignored before its target check.
    e("nocf_check", AttrKind::NOCF_CHECK, Gcc),
    e("noduplicate", AttrKind::NODUPLICATE, Clang),
    e("noescape", AttrKind::NOESCAPE, Clang),
    e("nomerge", AttrKind::NOMERGE, Clang),
    e("not_tail_called", AttrKind::NOT_TAIL_CALLED, Clang),
    e("nouwtable", AttrKind::NOUWTABLE, Clang),
    e("patchable_function_entry", AttrKind::PATCHABLE_FUNCTION_ENTRY, Gcc),
    e("speculative_load_hardening", AttrKind::SPECULATIVE_LOAD_HARDENING, Clang),
    e("standalone_debug", AttrKind::STANDALONE_DEBUG, Clang),
    Entry { a: 1, ..e("xray_always_instrument", AttrKind::XRAY_INSTRUMENT, Clang) },
    e("xray_log_args", AttrKind::XRAY_LOG_ARGS, Clang),
    e("xray_never_instrument", AttrKind::XRAY_INSTRUMENT, Clang),
    e("zero_call_used_regs", AttrKind::ZERO_CALL_USED_REGS, Gcc),
];

/// The class B kind (and preset `a`) of attribute `n` (no `__` affixes)
/// written with `syntax` and normalized scope `scope`, on an x86-64
/// target or not, at clang release `compat`.
pub(crate) fn kind_of(
    n: &str,
    syntax: AttrSyntax,
    scope: Option<&str>,
    x86: bool,
    compat: u32,
) -> Option<(AttrKind, u32)> {
    let i = TABLE.binary_search_by(|e| e.name.cmp(n)).ok()?;
    let e = &TABLE[i];
    let gnu = syntax == AttrSyntax::Gnu;
    let cxx = syntax == AttrSyntax::Cxx11;
    let ok = match e.spell {
        Gcc => gnu || (cxx && scope == Some("gnu")),
        Clang => gnu || (cxx && scope == Some("clang")),
        GccClang => gnu || (cxx && matches!(scope, Some("gnu" | "clang"))),
        Gnu => gnu,
        Omp => cxx && scope == Some("omp"),
        Gsl => cxx && scope == Some("gsl"),
        ClangGcc22 => {
            gnu || (cxx && (scope == Some("clang") || (scope == Some("gnu") && compat >= 22)))
        }
    };
    (ok && (e.target == Target::Any || x86) && compat >= e.since).then_some((e.kind, e.a))
}

/// Whether `n` is one of the C-only attributes clang knows and ignores
/// in C++ with `'X' attribute ignored` (`randomize_layout`,
/// `no_randomize_layout`: `GCC<>`, `LangOpts<"COnly">`; `btf_decl_tag`,
/// `btf_type_tag`: `Clang<>`).
pub(crate) fn is_c_only(n: &str, syntax: AttrSyntax, scope: Option<&str>) -> bool {
    let gnu = syntax == AttrSyntax::Gnu;
    let cxx = syntax == AttrSyntax::Cxx11;
    match n {
        "randomize_layout" | "no_randomize_layout" => gnu || (cxx && scope == Some("gnu")),
        "btf_decl_tag" | "btf_type_tag" => gnu || (cxx && scope == Some("clang")),
        _ => false,
    }
}

/// The name clang prints for class B attribute `node`.
pub(crate) fn name(node: &AttrNode) -> Option<&'static str> {
    if node.kind == AttrKind::XRAY_INSTRUMENT {
        return Some(if node.a == 1 { "xray_always_instrument" } else { "xray_never_instrument" });
    }
    if node.kind == AttrKind::OMP_ASSUME {
        return Some("assume");
    }
    if node.kind == AttrKind::NO_SANITIZE {
        let tag = kiln_ast::sanitize::no_sanitize_tag(node.b);
        return kiln_ast::sanitize::NO_SANITIZE_SPELLINGS.get(tag).map(|s| s.0);
    }
    TABLE.iter().find(|e| e.kind == node.kind).map(|e| e.name)
}

/// clang's `KnownAssumptionStrings` (`llvm/Frontend/OpenMP`).
const KNOWN_ASSUMPTIONS: [&str; 5] = [
    "omp_no_openmp",
    "omp_no_openmp_routines",
    "ompx_spmd_amenable",
    "ompx_no_call_asm",
    "ompx_aligned_barrier",
];

/// clang's `Sema::MaximumAlignment` (2^32).
const MAX_ALIGN: u64 = 1 << 32;

impl Sema<'_, '_> {
    /// Whether the target is x86-64.
    pub(crate) fn is_x86_target(&self) -> bool {
        self.abi.triple.arch == kiln_abi::target::Arch::X86_64
    }

    /// The source location of attribute argument `a` (`loc`: the
    /// attribute's).
    fn arg_loc(&self, a: &AttrArg, loc: Loc) -> Loc {
        match a {
            AttrArg::Expr(e) => self.begin_loc(*e),
            AttrArg::Ident(_, at) => *at,
            AttrArg::Type(_) => loc,
        }
    }

    /// The integer value of argument `a`, `Err(true)` when it is not an
    /// integer constant expression, `Err(false)` when it is dependent or
    /// already diagnosed.
    fn arg_int(&mut self, a: &AttrArg) -> Result<i128, bool> {
        let AttrArg::Expr(e) = a else { return Err(true) };
        let n = self.ast.exprs.get(*e);
        if self.is_err_expr(*e) {
            return Err(false);
        }
        if n.deps().intersects(
            kiln_ast::expr::ExprDeps::TYPE
                | kiln_ast::expr::ExprDeps::VALUE
                | kiln_ast::expr::ExprDeps::UNEXPANDED_PACK,
        ) {
            return Err(true);
        }
        if self.string_lit_of(*e).is_some() {
            return Err(true);
        }
        self.fold_int(*e).ok_or(true)
    }

    /// A class B attribute `kind` (preset word `a`) as written: its
    /// arguments checked as clang's handler checks them (`arg_name`: the
    /// name `parse_attribute_wrong_arg_count` prints, `shown` the one
    /// the `sema_*` diagnostics quote). `None` after an error.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn class_b_attribute(
        &mut self,
        kind: AttrKind,
        preset: u32,
        syntax: AttrSyntax,
        arg_name: &str,
        shown: &str,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let q = format!("'{shown}'");
        let count = |s: &mut Self, n: &str| {
            let msg = s.diag(DiagId::parse_attribute_wrong_arg_count, loc).arg(arg_name).arg(n);
            s.emit(msg);
            None
        };
        let (mut a, mut b, mut flags) = (preset, 0u32, 0u8);
        match kind {
            AttrKind::PATCHABLE_FUNCTION_ENTRY | AttrKind::ASSUME_ALIGNED => {
                if args.is_empty() {
                    let msg = self.diag(DiagId::sema_attribute_too_few_arguments_one, loc).arg(q);
                    self.emit(msg);
                    return None;
                }
                if args.len() > 2 {
                    let msg =
                        self.diag(DiagId::sema_attribute_too_many_arguments, loc).arg(q).arg("2");
                    self.emit(msg);
                    return None;
                }
                let mut vals = [0u64; 2];
                for (i, arg) in args.iter().enumerate() {
                    let v = match self.arg_int(arg) {
                        Ok(v) => v,
                        Err(false) => return None,
                        Err(true) => {
                            // clang `checkUInt32Argument` names the first
                            // argument's index (0); `AddAssumeAlignedAttr`
                            // names both (from 1) only when there are two.
                            let msg = if kind == AttrKind::PATCHABLE_FUNCTION_ENTRY && i == 0 {
                                self.diag(DiagId::sema_attribute_param_not_ice, loc)
                                    .arg(q.clone())
                                    .arg("0")
                            } else if kind == AttrKind::ASSUME_ALIGNED && args.len() == 2 {
                                self.diag(DiagId::sema_attribute_param_not_ice, loc)
                                    .arg(q.clone())
                                    .arg((i + 1).to_string())
                            } else {
                                self.diag(DiagId::sema_attribute_requires_integer_constant, loc)
                                    .arg(q.clone())
                            };
                            self.emit(msg);
                            return None;
                        }
                    };
                    if v < 0 {
                        if kind == AttrKind::PATCHABLE_FUNCTION_ENTRY {
                            let msg = self
                                .diag(DiagId::sema_attribute_requires_positive_integer, loc)
                                .arg(q.clone())
                                .arg("1");
                            self.emit(msg);
                            return None;
                        }
                        if i == 0 {
                            self.error(DiagId::sema_alignment_not_power_of_two, loc);
                            return None;
                        }
                    }
                    let mut u = u64::try_from(v.max(0)).unwrap_or(u64::MAX);
                    if kind == AttrKind::ASSUME_ALIGNED && i == 0 {
                        if !u.is_power_of_two() {
                            self.error(DiagId::sema_alignment_not_power_of_two, loc);
                            return None;
                        }
                        if u > MAX_ALIGN {
                            let msg = self
                                .diag(DiagId::sema_builtin_assume_aligned_too_big, loc)
                                .arg(MAX_ALIGN.to_string());
                            self.emit(msg);
                            u = MAX_ALIGN;
                        }
                    }
                    vals[i] = u;
                }
                if kind == AttrKind::PATCHABLE_FUNCTION_ENTRY && vals[1] > vals[0] {
                    let msg = self
                        .diag(DiagId::sema_attribute_argument_out_of_range, loc)
                        .arg(q)
                        .arg("0")
                        .arg(vals[0].to_string());
                    self.emit(msg);
                    return None;
                }
                // 2^32 does not fit `a`: kept as 0 (the IR prints the
                // exponent form, see codegen).
                a = u32::try_from(vals[0]).unwrap_or(0);
                b = u32::try_from(vals[1]).unwrap_or(u32::MAX);
                if kind == AttrKind::ASSUME_ALIGNED && vals[0] == MAX_ALIGN {
                    a = 0;
                }
            }
            AttrKind::ZERO_CALL_USED_REGS | AttrKind::FUNCTION_RETURN => {
                let [arg] = args else { return count(self, "1") };
                let at = self.arg_loc(arg, loc);
                let lit = match arg {
                    AttrArg::Expr(e) if self.is_err_expr(*e) => return None,
                    AttrArg::Expr(e) => self.string_lit_of(*e),
                    _ => None,
                };
                let Some(lit) = lit else {
                    let msg = if matches!(arg, AttrArg::Ident(..)) {
                        self.diag(DiagId::sema_attribute_requires_string, at).arg(q)
                    } else {
                        self.diag(DiagId::parse_attribute_requires_string, at).arg(arg_name)
                    };
                    self.emit(msg);
                    return None;
                };
                let id = self.ast.exprs.string_lit(lit);
                let text = String::from_utf8_lossy(self.ast.strings.bytes(id)).into_owned();
                let pos = if kind == AttrKind::ZERO_CALL_USED_REGS {
                    ZERO_CALL_USED_REGS_KINDS.iter().position(|k| *k == text)
                } else {
                    ["keep", "thunk-extern"].iter().position(|k| *k == text)
                };
                let Some(pos) = pos else {
                    let msg =
                        self.diag(DiagId::sema_attribute_arg_not_supported, at).arg(q).arg(text);
                    self.emit(msg);
                    return None;
                };
                a = u32::try_from(pos).unwrap_or(0);
            }
            AttrKind::XRAY_LOG_ARGS | AttrKind::ALIGN_VALUE | AttrKind::CODE_ALIGN => {
                let [arg] = args else { return count(self, "1") };
                let v = match self.arg_int(arg) {
                    Ok(v) => v,
                    Err(false) => return None,
                    // A dependent `code_align` is checked at
                    // instantiation (kiln: kept without effect).
                    Err(true) if kind == AttrKind::CODE_ALIGN && self.is_dependent_arg(arg) => 0,
                    Err(true) => {
                        let msg = if kind == AttrKind::XRAY_LOG_ARGS {
                            self.diag(DiagId::sema_attribute_param_not_ice, loc).arg(q).arg("1")
                        } else {
                            self.diag(DiagId::sema_attribute_requires_integer_constant, loc).arg(q)
                        };
                        self.emit(msg);
                        return None;
                    }
                };
                match kind {
                    AttrKind::ALIGN_VALUE => {
                        let u = u64::try_from(v).unwrap_or(0);
                        if !u.is_power_of_two() {
                            self.error(DiagId::sema_alignment_not_power_of_two, loc);
                            return None;
                        }
                        a = u32::try_from(u).unwrap_or(0);
                    }
                    AttrKind::CODE_ALIGN if !self.is_dependent_arg(arg) => {
                        let u = u64::try_from(v).unwrap_or(0);
                        if !u.is_power_of_two() || u > 4096 {
                            let msg = self
                                .diag(DiagId::sema_attribute_power_of_two_in_range, loc)
                                .arg(q)
                                .arg("1")
                                .arg("4096")
                                .arg(v.to_string());
                            self.emit(msg);
                            return None;
                        }
                        a = u32::try_from(u).unwrap_or(0);
                    }
                    AttrKind::CODE_ALIGN => a = 0,
                    _ => a = u32::try_from(v).unwrap_or(0),
                }
            }
            AttrKind::OMP_ASSUME => {
                let [arg] = args else { return count(self, "1") };
                let at = self.arg_loc(arg, loc);
                let lit = match arg {
                    AttrArg::Expr(e) if self.is_err_expr(*e) => return None,
                    AttrArg::Expr(e) => self.string_lit_of(*e),
                    _ => None,
                };
                let Some(lit) = lit else {
                    let msg = self.diag(DiagId::parse_attribute_requires_string, at).arg(arg_name);
                    self.emit(msg);
                    return None;
                };
                let id = self.ast.exprs.string_lit(lit);
                let text = String::from_utf8_lossy(self.ast.strings.bytes(id)).into_owned();
                for s in text.split(',') {
                    if !KNOWN_ASSUMPTIONS.contains(&s) && !s.starts_with("ompx_") {
                        let msg = self.diag(DiagId::sema_unknown_assumption, at).arg(s.to_owned());
                        self.emit(msg);
                    }
                }
                a = id.raw();
            }
            AttrKind::CALLBACK => {
                // The arguments as written, two words each: 0 and the
                // value of an integer, 1 and the symbol of an identifier;
                // resolved against the parameters by
                // [`Self::check_callback_attr`].
                if args.is_empty() {
                    self.error(DiagId::sema_callback_no_callee, loc);
                    return None;
                }
                let mut words = Vec::with_capacity(args.len() * 2);
                for arg in args {
                    match arg {
                        AttrArg::Ident(s, at) => {
                            words.extend([1, s.index()]);
                            // The location rides in a third word.
                            words.push(at.to_raw());
                        }
                        _ => match self.arg_int(arg) {
                            Ok(v) => words.extend([0, u32::try_from(v).unwrap_or(u32::MAX), 0]),
                            Err(false) => return None,
                            Err(true) => {
                                let msg = self
                                    .diag(DiagId::sema_attribute_requires_integer_constant, loc)
                                    .arg(q);
                                self.emit(msg);
                                return None;
                            }
                        },
                    }
                }
                a = self.ast.lists.push_raw(&words).raw();
            }
            AttrKind::LIFETIMEBOUND if !args.is_empty() => flags = AttrNode::LIFETIMEBOUND_ARGS,
            AttrKind::GSL_OWNER | AttrKind::GSL_POINTER => {
                // clang `handleLifetimeCategoryAttr`: an optional type-id
                // (the parser reports a missing one); a reference or array
                // type is rejected before the subject is looked at.
                match args {
                    [] => {}
                    [AttrArg::Type(t)] => {
                        let canon = self.ast.types.canonical(*t);
                        let bad = if self.ast.types.is_reference(canon) {
                            Some("0")
                        } else if self.ast.types.is_array(canon) {
                            Some("1")
                        } else {
                            None
                        };
                        if let Some(sel) = bad {
                            let msg = self
                                .diag(DiagId::sema_attribute_invalid_argument, loc)
                                .arg(sel)
                                .arg(q);
                            self.emit(msg);
                            return None;
                        }
                        flags = AttrNode::GSL_HAS_TYPE;
                        b = t.to_raw();
                    }
                    _ => return count(self, "1"),
                }
            }
            AttrKind::NO_SANITIZE if preset == 0 => {
                // clang `handleNoSanitizeAttr`.
                if args.is_empty() {
                    let msg = self.diag(DiagId::sema_attribute_too_few_arguments_one, loc).arg(q);
                    self.emit(msg);
                    return None;
                }
                let mut set = SanitizerSet::NONE;
                let mut not_global = Vec::new();
                for arg in args {
                    let at = self.arg_loc(arg, loc);
                    let lit = match arg {
                        AttrArg::Expr(e) if self.is_err_expr(*e) => return None,
                        AttrArg::Expr(e) => self.string_lit_of(*e),
                        _ => None,
                    };
                    let Some(lit) = lit else {
                        let msg = if matches!(arg, AttrArg::Ident(..)) {
                            self.diag(DiagId::sema_attribute_requires_string, at).arg(q)
                        } else {
                            self.diag(DiagId::parse_attribute_requires_string, at).arg(arg_name)
                        };
                        self.emit(msg);
                        return None;
                    };
                    let id = self.ast.exprs.string_lit(lit);
                    let text = String::from_utf8_lossy(self.ast.strings.bytes(id)).into_owned();
                    match kiln_ast::sanitize::parse_name(&text, true) {
                        None if text != "coverage" => {
                            let msg =
                                self.diag(DiagId::sema_unknown_sanitizer_ignored, at).arg(text);
                            self.emit(msg);
                        }
                        s => {
                            set = set.union(s.unwrap_or(SanitizerSet::NONE));
                            if !matches!(text.as_str(), "address" | "hwaddress" | "memtag") {
                                not_global.push(text);
                            }
                        }
                    }
                }
                (a, b) = kiln_ast::sanitize::no_sanitize_words(set, 0);
                let id = self.ast.attrs.push(AttrNode { kind, syntax, flags: 0, loc, a, b });
                if !not_global.is_empty() {
                    self.no_sanitize_not_global.insert(id, not_global);
                }
                return Some(id);
            }
            AttrKind::NO_SANITIZE => {
                if !args.is_empty() {
                    return count(self, "0");
                }
                let set = kiln_ast::sanitize::NO_SANITIZE_SPELLINGS
                    .get(preset as usize)
                    .map_or(SanitizerSet::NONE, |s| s.1);
                (a, b) = kiln_ast::sanitize::no_sanitize_words(set, preset);
            }
            _ if !args.is_empty() => return count(self, "0"),
            _ => {}
        }
        Some(self.ast.attrs.push(AttrNode { kind, syntax, flags, loc, a, b }))
    }

    fn is_dependent_arg(&self, a: &AttrArg) -> bool {
        matches!(a, AttrArg::Expr(e) if self.ast.exprs.get(*e).deps().intersects(
            kiln_ast::expr::ExprDeps::TYPE | kiln_ast::expr::ExprDeps::VALUE))
    }

    /// The subject check of class B attribute `node` on declaration `d`
    /// of kind `kind` (clang's `SubjectList` diagnostics and the checks
    /// that need the declaration); false drops it.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn check_class_b_attr(
        &mut self,
        d: DeclId,
        kind: DeclKind,
        a: AttrId,
        node: AttrNode,
    ) -> bool {
        const FNS_METHODS: &str = "functions and Objective-C methods";
        let n = name(&node).unwrap_or_default();
        let wrong = |s: &mut Self, error: bool, what: &str| {
            let shown = s.attr_shown(&node, n);
            let id = if error {
                DiagId::sema_attribute_wrong_decl_type_error
            } else {
                DiagId::sema_attribute_wrong_decl_type
            };
            let msg = s.diag(id, node.loc).arg(format!("'{shown}'")).arg(what.to_owned());
            s.emit(msg);
            false
        };
        let q = format!("'{}'", self.attr_shown(&node, n));
        let func = kind.is_function();
        let var = matches!(kind, DeclKind::Var | DeclKind::VarTemplateSpecialization);
        let record = matches!(
            kind,
            DeclKind::CXXRecord
                | DeclKind::ClassTemplateSpecialization
                | DeclKind::ClassTemplatePartialSpecialization
        );
        match node.kind {
            // The type-position form (`a` = 1, clang's `AttributedType`
            // checked by `checkAttributesAfterMerging`): ignored on a
            // declarator that does not declare a function.
            AttrKind::LIFETIMEBOUND if node.a == 1 => {
                if !func {
                    return false;
                }
                let (id, sel) = match kind {
                    DeclKind::CXXConstructor => (DiagId::sema_lifetimebound_ctor_dtor, "0"),
                    DeclKind::CXXDestructor => (DiagId::sema_lifetimebound_ctor_dtor, "1"),
                    DeclKind::CXXMethod | DeclKind::CXXConversion if !self.is_static_method(d) => {
                        return true;
                    }
                    DeclKind::CXXMethod => (DiagId::sema_lifetimebound_no_object_param, "0"),
                    _ => (DiagId::sema_lifetimebound_no_object_param, "1"),
                };
                let msg = self.diag(id, node.loc).arg(sel);
                self.emit(msg);
                false
            }
            AttrKind::LIFETIMEBOUND if node.flags & AttrNode::LIFETIMEBOUND_ARGS != 0 => {
                let msg = self
                    .diag(DiagId::parse_attribute_wrong_arg_count, node.loc)
                    .arg("lifetimebound")
                    .arg("0");
                self.emit(msg);
                false
            }
            AttrKind::LIFETIMEBOUND => {
                kind == DeclKind::ParmVar
                    || wrong(self, true, "parameters and implicit object parameters")
            }
            AttrKind::GSL_OWNER | AttrKind::GSL_POINTER => {
                let union = self
                    .ast
                    .decls
                    .record(d)
                    .is_some_and(|x| x.tag == kiln_ast::decl::TagKind::Union);
                (record && !union) || wrong(self, false, "structs")
            }
            AttrKind::NOCF_CHECK if !self.cf_protection_branch && (func || var) => {
                let msg = self.diag(DiagId::sema_nocf_check_ignored, node.loc);
                self.emit(msg);
                false
            }
            // Off x86 it is otherwise an unknown attribute.
            AttrKind::NOCF_CHECK if !self.is_x86_target() => {
                let msg = self
                    .diag(DiagId::parse_unknown_attribute, node.loc)
                    .arg(self.attr_shown(&node, n));
                self.emit(msg);
                false
            }
            AttrKind::NOCF_CHECK => {
                func || self.is_fn_pointer_decl(d)
                    || wrong(self, false, "functions and function pointers")
            }
            AttrKind::NO_STACK_PROTECTOR
            | AttrKind::NO_PROFILE_INSTRUMENT_FUNCTION
            | AttrKind::FUNCTION_RETURN
            | AttrKind::FORCE_ALIGN_ARG_POINTER
            | AttrKind::LEAF
            | AttrKind::NOT_TAIL_CALLED
            | AttrKind::NODUPLICATE
            | AttrKind::CONVERGENT
            | AttrKind::CALLBACK
                if !func =>
            {
                wrong(self, false, "functions")
            }
            AttrKind::CALLBACK => self.check_callback_attr(d, a, node),
            AttrKind::ZERO_CALL_USED_REGS
            | AttrKind::NO_SPLIT_STACK
            | AttrKind::CFI_CANONICAL_JUMP_TABLE
            | AttrKind::FLATTEN
                if !func =>
            {
                wrong(self, true, "functions")
            }
            AttrKind::SPECULATIVE_LOAD_HARDENING | AttrKind::NO_SPECULATIVE_LOAD_HARDENING
                if !func =>
            {
                wrong(self, true, FNS_METHODS)
            }
            AttrKind::PATCHABLE_FUNCTION_ENTRY
            | AttrKind::NO_INSTRUMENT_FUNCTION
            | AttrKind::XRAY_INSTRUMENT
            | AttrKind::XRAY_LOG_ARGS
            | AttrKind::OMP_ASSUME
                if !func =>
            {
                wrong(self, false, FNS_METHODS)
            }
            AttrKind::XRAY_LOG_ARGS => {
                let nparams = self.fn_param_count(d);
                let this = u32::from(self.is_implicit_object_member(d));
                if node.a < 1 || node.a > nparams + this {
                    let msg = self
                        .diag(DiagId::sema_attribute_param_out_of_bounds, node.loc)
                        .arg(q)
                        .arg("1");
                    self.emit(msg);
                    return false;
                }
                true
            }
            AttrKind::NO_SANITIZE => self.check_no_sanitize(d, kind, a, node),
            AttrKind::DISABLE_SANITIZER_INSTRUMENTATION if !func && !var => {
                wrong(self, false, "functions, Objective-C methods, and global variables")
            }
            AttrKind::NOUWTABLE if !func && !self.is_fn_pointer_decl(d) => {
                wrong(self, false, "functions and function pointers")
            }
            AttrKind::NOMERGE if func || self.is_fn_pointer_decl(d) => true,
            AttrKind::NOMERGE if var || kind == DeclKind::ParmVar || kind == DeclKind::Field => {
                let msg = self
                    .diag(DiagId::sema_attribute_not_function_pointer, node.loc)
                    .arg(q)
                    .arg(self.decl_arg(d));
                self.emit(msg);
                false
            }
            AttrKind::NOMERGE => wrong(self, true, "functions, statements and variables"),
            AttrKind::ASSUME_ALIGNED if !func => {
                wrong(self, false, "Objective-C methods and functions")
            }
            AttrKind::ASSUME_ALIGNED => {
                let r = self.fn_ret(d);
                let t = &self.ast.types;
                if r != kiln_ast::QualType::ERROR
                    && !(t.is_pointer(r) || t.is_reference(r) || t.is_dependent(r))
                {
                    let msg = self.diag(DiagId::sema_attribute_pointers_refs_only, node.loc).arg(q);
                    self.emit(msg);
                    return false;
                }
                true
            }
            AttrKind::ALIGN_VALUE => {
                let typedef = matches!(kind, DeclKind::Typedef | DeclKind::TypeAlias);
                if !(var || kind == DeclKind::ParmVar || kind == DeclKind::Field || typedef) {
                    return wrong(self, false, "variables and typedefs");
                }
                // A typedef's underlying type (clang `TypedefNameDecl::
                // getUnderlyingType`), as the message shows it.
                let ty = if typedef {
                    self.ast.decls.typedef(d).map(|t| t.underlying)
                } else {
                    self.ast.decls.get(d).ty()
                };
                let t = &self.ast.types;
                if let Some(ty) = ty
                    && !(t.is_pointer(ty) || t.is_reference(ty) || t.is_dependent(ty))
                {
                    let msg = self
                        .diag(DiagId::sema_attribute_pointer_or_reference_only, node.loc)
                        .arg(q)
                        .arg(self.ty_arg(ty));
                    self.emit(msg);
                    return false;
                }
                true
            }
            AttrKind::NOESCAPE | AttrKind::MAYBE_UNDEF | AttrKind::CORO_AWAIT_ELIDABLE_ARGUMENT
                if kind != DeclKind::ParmVar =>
            {
                wrong(self, false, "parameters")
            }
            AttrKind::NOESCAPE => {
                let ty = self.ast.decls.get(d).ty();
                let t = &self.ast.types;
                if let Some(ty) = ty
                    && !(t.is_pointer(ty) || t.is_reference(ty) || t.is_dependent(ty))
                {
                    let msg = self.diag(DiagId::sema_attribute_pointer_args_only, node.loc).arg(q);
                    self.emit(msg);
                    return false;
                }
                true
            }
            AttrKind::LOADER_UNINITIALIZED => self.check_loader_uninitialized(d, kind, node),
            AttrKind::LTO_VISIBILITY_PUBLIC if !record => {
                wrong(self, false, "structs, unions, and classes")
            }
            AttrKind::STANDALONE_DEBUG | AttrKind::CORO_AWAIT_ELIDABLE if !record => {
                wrong(self, false, "classes")
            }
            AttrKind::CODE_ALIGN => {
                let msg = self
                    .diag(DiagId::sema_attribute_wrong_stmt_type, node.loc)
                    .arg(self.attr_shown(&node, n))
                    .arg("'for', 'while', and 'do' statements");
                self.emit(msg);
                false
            }
            _ => true,
        }
    }

    /// The class B `MutualExclusions` on function `d` (clang
    /// Attr.td): `speculative_load_hardening` against
    /// `no_speculative_load_hardening`, and `always_inline` against
    /// `not_tail_called`; on one declaration or across redeclarations
    /// the later is an error and dropped ([`Sema::check_exclusive_pair`]).
    pub(crate) fn check_class_b_exclusions(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        self.check_exclusive_pair(
            d,
            attrs,
            [
                (AttrKind::SPECULATIVE_LOAD_HARDENING, "speculative_load_hardening"),
                (AttrKind::NO_SPECULATIVE_LOAD_HARDENING, "no_speculative_load_hardening"),
            ],
        );
        self.check_exclusive_pair(
            d,
            attrs,
            [
                (AttrKind::ALWAYS_INLINE, "always_inline"),
                (AttrKind::NOT_TAIL_CALLED, "not_tail_called"),
            ],
        );
        self.check_gsl_attrs(d, attrs);
    }

    /// clang `handleLifetimeCategoryAttr`: `gsl::Owner` and `gsl::Pointer`
    /// exclude each other, and a repeat of one must name the same type
    /// (as written: a typedef differs from its target) or none, across
    /// the redeclarations of the class and the attributes before it.
    fn check_gsl_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        let is_gsl = |k: AttrKind| k == AttrKind::GSL_OWNER || k == AttrKind::GSL_POINTER;
        if !attrs.iter().any(|&a| is_gsl(self.ast.attrs.get(a).kind)) {
            return;
        }
        let prior: Vec<AttrNode> = self
            .ast
            .redecls_of(d)
            .filter(|&r| r != d)
            .flat_map(|r| self.ast.lists.iter::<AttrId>(self.ast.attrs.of_decl(r)))
            .map(|a| *self.ast.attrs.get(a))
            .filter(|n| is_gsl(n.kind))
            .collect();
        let deref = |n: &AttrNode| {
            (n.flags & AttrNode::GSL_HAS_TYPE != 0)
                .then(|| kiln_ast::QualType::from_raw(n.b).map(kiln_ast::QualType::without_quals))
                .flatten()
        };
        let mut kept: Vec<AttrNode> = Vec::new();
        let mut drop: Vec<AttrId> = Vec::new();
        for &a in attrs.iter() {
            let n = *self.ast.attrs.get(a);
            if !is_gsl(n.kind) {
                continue;
            }
            let seen = || prior.iter().chain(kept.iter());
            let other = seen().find(|e| e.kind != n.kind).copied();
            let same = seen().find(|e| e.kind == n.kind).copied();
            let clash = match (other, same) {
                (Some(e), _) => Some(e),
                (None, Some(e)) if deref(&e) != deref(&n) => Some(e),
                (None, Some(_)) => {
                    drop.push(a);
                    continue;
                }
                (None, None) => None,
            };
            if let Some(e) = clash {
                let shown =
                    |k: AttrKind| if k == AttrKind::GSL_OWNER { "Owner" } else { "Pointer" };
                let note = self.diag(DiagId::note_conflicting_attribute, e.loc);
                let msg = self
                    .diag(DiagId::sema_attributes_not_compatible, n.loc)
                    .arg(shown(n.kind))
                    .arg(shown(e.kind))
                    .with_note(note);
                self.emit(msg);
                drop.push(a);
            } else {
                kept.push(n);
            }
        }
        attrs.retain(|a| !drop.contains(a));
    }

    /// Whether `d` is a variable, parameter or field of (reference to)
    /// pointer-to-function type.
    /// The subject of a `no_sanitize*` attribute (clang: functions,
    /// Objective-C methods and global variables for `no_sanitize`,
    /// functions and global variables for the others, functions only for
    /// `no_sanitize_thread` and `no_sanitize_memory`), and the
    /// `no_sanitize("...")` arguments a global variable ignores.
    fn check_no_sanitize(&mut self, d: DeclId, kind: DeclKind, a: AttrId, node: AttrNode) -> bool {
        let tag = kiln_ast::sanitize::no_sanitize_tag(node.b);
        let n = name(&node).unwrap_or_default();
        let q = format!("'{}'", self.attr_shown(&node, n));
        if kind.is_function() {
            return true;
        }
        let global = matches!(kind, DeclKind::Var | DeclKind::VarTemplateSpecialization)
            && self.ast.decls.var(d).is_some_and(|v| {
                let local = self
                    .ast
                    .decls
                    .get(d)
                    .ctx()
                    .is_some_and(|c| self.ast.decls.get(c).kind().is_function());
                !local || v.spec.storage() == VarStorage::Static
            });
        let at = self.ast.decls.get(d).loc();
        if !global {
            let what = if tag == 0 {
                "functions, Objective-C methods, and global variables"
            } else {
                "functions and global variables"
            };
            let msg =
                self.diag(DiagId::sema_attribute_wrong_decl_type_error, node.loc).arg(q).arg(what);
            self.emit(msg);
            return false;
        }
        if tag >= 3 {
            // clang 19 points at the variable, clang 22 at the attribute.
            let loc = if self.clang_compat >= 22 { node.loc } else { at };
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type_error, loc)
                .arg(q)
                .arg("functions");
            self.emit(msg);
            return false;
        }
        if let Some(names) = self.no_sanitize_not_global.get(&a).cloned() {
            for t in names {
                let msg = self
                    .diag(DiagId::sema_attribute_type_not_supported_global, at)
                    .arg(q.clone())
                    .arg(t)
                    .allow_duplicate();
                self.emit(msg);
            }
        }
        true
    }

    fn is_fn_pointer_decl(&self, d: DeclId) -> bool {
        let t = &self.ast.types;
        self.ast.decls.get(d).ty().is_some_and(|ty| {
            let ty = t.pointee(ty).filter(|_| t.is_reference(ty)).unwrap_or(ty);
            t.is_pointer(ty) && t.pointee(ty).is_some_and(|p| t.is_function(p))
        })
    }

    fn fn_param_count(&self, d: DeclId) -> u32 {
        let t = &self.ast.types;
        self.ast
            .decls
            .get(d)
            .ty()
            .and_then(|q| t.proto(q))
            .map_or(0, |p| u32::try_from(t.params(p).len()).unwrap_or(0))
    }

    /// clang `handleCallbackAttr`: resolves the written arguments of
    /// `callback` on function `d` to LLVM argument numbers (`this` 0;
    /// `__` -1, an unknown payload) and checks the callee is a function
    /// pointer taking one argument per payload. The node's `a` becomes
    /// the raw `ListId` of the numbers (as `i32` bit patterns).
    fn check_callback_attr(&mut self, d: DeclId, a: AttrId, node: AttrNode) -> bool {
        let q = format!("'{}'", self.attr_shown(&node, "callback"));
        let Some(list) = kiln_ast::ListId::from_raw(node.a) else { return false };
        let words = self.ast.lists.get(list).to_vec();
        let this = self.is_implicit_object_member(d);
        let params: Vec<DeclId> = self
            .ast
            .decls
            .function(d)
            .map(|f| self.ast.lists.iter::<DeclId>(f.params).collect())
            .unwrap_or_default();
        let ptys: Vec<kiln_ast::QualType> = {
            let t = &self.ast.types;
            self.ast
                .decls
                .get(d)
                .ty()
                .and_then(|q| t.proto(q))
                .map(|p| t.params(p).to_vec())
                .unwrap_or_default()
        };
        let mut enc: Vec<i32> = Vec::new();
        for (i, w) in words.chunks(3).enumerate() {
            let idx: i64 = if w[0] == 1 {
                let sym = Some(kiln_source::symbol::Symbol::from_index(w[1]));
                let text = sym.map(|s| self.ast.symbols().as_str(s).to_owned()).unwrap_or_default();
                if text == "__" {
                    -1
                } else if let Some(p) = params.iter().position(|&p| {
                    sym.is_some_and(|s| self.ast.decls.get(p).name().as_ident() == Some(s))
                }) {
                    i64::try_from(p).unwrap_or(0) + 1 + i64::from(this)
                } else {
                    let msg = self.diag(DiagId::sema_callback_argument_unknown, node.loc).arg(text);
                    self.emit(msg);
                    return false;
                }
            } else {
                i64::from(w[1])
            };
            let max = i64::try_from(ptys.len()).unwrap_or(0) + i64::from(this);
            if idx < -1 || idx > max || (idx == 0 && !this) {
                let msg = self
                    .diag(DiagId::sema_attribute_param_out_of_bounds, node.loc)
                    .arg(q)
                    .arg((i + 1).to_string());
                self.emit(msg);
                return false;
            }
            // LLVM numbers the arguments from 0; `this` is the first.
            let llvm = if idx > 0 && !this { idx - 1 } else { idx };
            enc.push(i32::try_from(llvm).unwrap_or(-1));
        }
        let callee = enc[0];
        let cty = usize::try_from(callee - i32::from(this)).ok().and_then(|k| ptys.get(k).copied());
        let t = &self.ast.types;
        let fproto =
            cty.and_then(|c| t.pointee(c).filter(|_| t.is_pointer(c))).and_then(|p| t.proto(p));
        let Some(fproto) = fproto else {
            self.error(DiagId::sema_callback_callee_no_function_type, node.loc);
            return false;
        };
        let want = t.params(fproto).len();
        // clang names the payload count written, not the one wanted.
        let have = enc.len() - 1;
        if want != have {
            let msg = if have <= 1 {
                self.diag(DiagId::parse_attribute_wrong_arg_count, node.loc)
                    .arg(self.attr_shown(&node, "callback"))
                    .arg(have.to_string())
            } else {
                self.diag(DiagId::sema_attribute_wrong_number_arguments, node.loc)
                    .arg(q)
                    .arg(have.to_string())
            };
            self.emit(msg);
            return false;
        }
        let raw: Vec<u32> = enc.iter().map(|&v| v.cast_unsigned()).collect();
        let l = self.ast.lists.push_raw(&raw);
        self.ast.attrs.set_args(a, l.raw(), 0);
        true
    }

    /// clang `handleLoaderUninitializedAttr` and the checks of
    /// `ActOnUninitializedDecl`: a variable with static storage duration,
    /// not `extern`, without an initializer.
    fn check_loader_uninitialized(&mut self, d: DeclId, kind: DeclKind, node: AttrNode) -> bool {
        let Some(v) = (matches!(kind, DeclKind::Var | DeclKind::VarTemplateSpecialization))
            .then(|| self.ast.decls.var(d))
            .flatten()
        else {
            let shown = format!("'{}'", self.attr_shown(&node, "loader_uninitialized"));
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                .arg(shown)
                .arg("global variables");
            self.emit(msg);
            return false;
        };
        let storage = v.spec.storage();
        let local =
            self.ast.decls.get(d).ctx().is_some_and(|c| self.ast.decls.get(c).kind().is_function());
        if local && storage != VarStorage::Static {
            let shown = format!("'{}'", self.attr_shown(&node, "loader_uninitialized"));
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                .arg(shown)
                .arg("global variables");
            self.emit(msg);
            return false;
        }
        if storage == VarStorage::Extern {
            let at = self.ast.decls.get(d).loc();
            let msg = self.diag(DiagId::sema_loader_uninitialized_extern, at).arg(self.decl_arg(d));
            self.emit(msg);
            return false;
        }
        true
    }

    /// Whether variable `d` carries `loader_uninitialized`.
    pub(crate) fn has_loader_uninitialized(&self, d: DeclId) -> bool {
        self.ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(d))
            .any(|a| self.ast.attrs.get(a).kind == AttrKind::LOADER_UNINITIALIZED)
    }

    /// clang `ActOnUninitializedDecl`: a `loader_uninitialized` variable
    /// of class type needs a trivial default constructor.
    pub(crate) fn check_loader_uninitialized_ctor(&mut self, d: DeclId) {
        let Some(ty) = self.ast.decls.get(d).ty() else { return };
        let e = self.base_element(ty);
        let Some(r) = self.ast.types.as_record(e) else { return };
        if !self.trivial_default_ctor(r) {
            let at = self.ast.decls.get(d).loc();
            self.error(DiagId::sema_loader_uninitialized_trivial_ctor, at);
            self.ast.decls.set_flags(d, kiln_ast::decl::DeclFlags::INVALID);
        }
    }

    /// The class B statement attributes of [`Sema::act_on_attributed_stmt`]
    /// (clang `ProcessStmtAttribute`): `nomerge`, `noinline` and
    /// `always_inline` (the `[[clang::...]]` spellings) mark the calls in
    /// `sub`; `code_align` applies to a loop. Returns whether `a` is kept.
    pub(crate) fn class_b_stmt_attr(
        &mut self,
        a: AttrId,
        sub: StmtId,
        kept: &[AttrId],
    ) -> Option<bool> {
        let node = *self.ast.attrs.get(a);
        let sub_kind = self.ast.stmts.get(sub).kind();
        let begin = self.stmt_begin(sub);
        match node.kind {
            AttrKind::NOINLINE | AttrKind::ALWAYS_INLINE => {
                let canonical =
                    if node.kind == AttrKind::NOINLINE { "noinline" } else { "always_inline" };
                let clang =
                    self.attr_spellings.get(&node.loc).and_then(|&(s, _)| s).is_some_and(|s| {
                        crate::decl::norm_attr_scope(self.ast.symbols().as_str(s)) == "clang"
                    });
                if !clang || node.syntax != AttrSyntax::Cxx11 {
                    let msg = self
                        .diag(DiagId::sema_function_attribute_ignored_in_stmt, begin)
                        .arg(format!("[[clang::{canonical}]]"));
                    self.emit(msg);
                    return Some(false);
                }
                let other = if node.kind == AttrKind::NOINLINE {
                    AttrKind::ALWAYS_INLINE
                } else {
                    AttrKind::NOINLINE
                };
                Some(self.check_stmt_calls(sub, node, canonical, Some(other)))
            }
            AttrKind::NOMERGE => {
                if sub_kind == StmtKind::DeclStmt {
                    return Some(false);
                }
                Some(self.check_stmt_calls(sub, node, "nomerge", None))
            }
            AttrKind::CODE_ALIGN => {
                let shown = self.attr_shown(&node, "code_align");
                if !matches!(
                    sub_kind,
                    StmtKind::ForStmt
                        | StmtKind::WhileStmt
                        | StmtKind::DoStmt
                        | StmtKind::CXXForRangeStmt
                ) {
                    let msg = self
                        .diag(DiagId::sema_attribute_wrong_stmt_type, node.loc)
                        .arg(shown)
                        .arg("'for', 'while', and 'do' statements");
                    self.emit(msg);
                    return Some(false);
                }
                if let Some(&prev) =
                    kept.iter().find(|&&k| self.ast.attrs.get(k).kind == AttrKind::CODE_ALIGN)
                {
                    let prev = *self.ast.attrs.get(prev);
                    if prev.a != node.a {
                        let msg = self
                            .diag(DiagId::sema_loop_attr_conflict, node.loc)
                            .arg(format!("'{shown}'"))
                            .with_note(self.diag(DiagId::note_previous_attribute, prev.loc));
                        self.emit(msg);
                    }
                    return Some(false);
                }
                Some(true)
            }
            _ => None,
        }
    }

    /// clang `CheckStmtInlineAttr` / `handleNoMergeAttr`: `sub` must
    /// contain a call (else the attribute is ignored with a warning); a
    /// called function with the `other` attribute (or `flatten`) is
    /// overridden, with a warning.
    fn check_stmt_calls(
        &mut self,
        sub: StmtId,
        node: AttrNode,
        canonical: &str,
        other: Option<AttrKind>,
    ) -> bool {
        let begin = self.stmt_begin(sub);
        let shown = format!("'{}'", self.attr_shown(&node, canonical));
        let mut calls = Vec::new();
        let complete = self.stmt_calls(sub, &mut calls);
        if calls.is_empty() {
            if complete {
                let msg = self.diag(DiagId::sema_attribute_no_calls_in_stmt, begin).arg(shown);
                self.emit(msg);
                return false;
            }
            return true;
        }
        let Some(other) = other else { return true };
        for c in calls {
            let Some(callee) = self.callee_decl(c) else { continue };
            let found = self.ast.redecls_of(callee).find_map(|r| {
                self.ast
                    .lists
                    .iter::<AttrId>(self.ast.attrs.of_decl(r))
                    .map(|a| *self.ast.attrs.get(a))
                    .find(|n| n.kind == other || n.kind == AttrKind::FLATTEN)
                    .map(|n| (n, r))
            });
            if let Some((fa, r)) = found {
                let fname = if fa.kind == AttrKind::FLATTEN {
                    "flatten"
                } else if fa.kind == AttrKind::NOINLINE {
                    "noinline"
                } else {
                    "always_inline"
                };
                let fshown = format!("'{}'", self.attr_shown(&fa, fname));
                let at = self
                    .inline_fn_begin_locs
                    .get(&r)
                    .copied()
                    .unwrap_or_else(|| self.ast.decls.get(r).loc());
                let msg = self
                    .diag(DiagId::sema_stmt_attribute_precedence, begin)
                    .arg(shown.clone())
                    .arg(fshown)
                    .with_note(self.diag(DiagId::note_conflicting_attribute, at));
                self.emit(msg);
            }
        }
        true
    }

    /// Where statement `s` begins (an expression statement at its
    /// expression's first token).
    fn stmt_begin(&self, s: StmtId) -> Loc {
        if self.ast.stmts.get(s).kind() == StmtKind::ExprStmt {
            return self.begin_loc(self.ast.stmts.expr_stmt_expr(s));
        }
        self.ast.stmts.get(s).loc()
    }

    /// The call expressions in statement `s` (clang `CallExprFinder`,
    /// not into unevaluated operands); false when `s` holds a statement
    /// kind not searched (its calls are unknown).
    fn stmt_calls(&self, s: StmtId, out: &mut Vec<ExprId>) -> bool {
        let stmts = &self.ast.stmts;
        match stmts.get(s).kind() {
            StmtKind::ExprStmt => self.expr_calls(stmts.expr_stmt_expr(s), out),
            StmtKind::ReturnStmt => {
                if let Some(e) = stmts.return_value(s) {
                    self.expr_calls(e, out);
                }
            }
            StmtKind::NullStmt => {}
            StmtKind::CompoundStmt => {
                let mut ok = true;
                for c in self.ast.lists.iter::<StmtId>(stmts.compound_body(s)) {
                    ok &= self.stmt_calls(c, out);
                }
                return ok;
            }
            StmtKind::AttributedStmt => return self.stmt_calls(stmts.sub_stmt(s), out),
            StmtKind::DeclStmt => {
                for d in self.ast.lists.iter::<DeclId>(stmts.decl_stmt_decls(s)) {
                    if let Some(i) = self.ast.decls.var(d).and_then(|v| v.init) {
                        self.expr_calls(i, out);
                    }
                }
            }
            _ => return false,
        }
        true
    }

    fn expr_calls(&self, e: ExprId, out: &mut Vec<ExprId>) {
        let exprs = &self.ast.exprs;
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            match exprs.get(x).kind() {
                ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXOperatorCallExpr => {
                    out.push(x);
                }
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXTypeidExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::RequiresExpr
                | ExprKind::LambdaExpr => continue,
                _ => {}
            }
            exprs.children_into(x, &self.ast.lists, &mut stack);
        }
        out.reverse();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn table_is_sorted_for_binary_search() {
        assert!(super::TABLE.windows(2).all(|w| w[0].name < w[1].name));
    }
}
