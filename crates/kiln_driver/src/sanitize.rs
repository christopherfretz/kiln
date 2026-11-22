//! The sanitizer options (spec/driver.md "Sanitizers", WP-675;
//! docs/design/sanitizers.md §2): `-f[no-]sanitize=`, the recover and
//! trap lists, the ignorelists, the address and memory sanitizer
//! options and the runtime choice, resolved the way
//! clang 19's `SanitizerArgs` does.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;

use kiln_ast::sanitize::{GROUPS, SanitizerSet as S, parse_name};
use kiln_codegen::SanitizeOptions;
use kiln_codegen::ignorelist::{CoverageLists, IgnorelistError, SanitizerIgnorelist};

use crate::args::ArgError;

/// Which list an option is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum List {
    Add,
    Remove,
    Recover,
    NoRecover,
    Trap,
    NoTrap,
    Merge,
    NoMerge,
}

/// One list option as written.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ListArg {
    list: List,
    /// The option with its `=` (`-fsanitize=`).
    opt: &'static str,
    values: Vec<String>,
}

/// The sanitizer options of a command line, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SanitizeFlags {
    lists: Vec<ListArg>,
    /// `-fsanitize-ignorelist=` / `-fsanitize-blacklist=` files since the
    /// last `-fno-sanitize-ignorelist`.
    pub ignorelists: Vec<String>,
    /// The working directory the lists' relative paths open against
    /// ([`crate::args::Args::work_dir`]; spec/driver.md "Working
    /// directory"). Not a command-line option.
    pub work_dir: kiln_source::WorkDir,
    /// Some `-fno-sanitize-ignorelist` was given: no default ignorelist
    /// from the resource directory (clang's `hasArgNoClaim`, wherever it
    /// stands).
    pub no_default_ignorelists: bool,
    /// The last of `-f[no-]sanitize-address-use-after-scope`.
    pub use_after_scope: Option<bool>,
    /// `-fsanitize-address-use-after-return=<mode>`.
    pub use_after_return: Option<&'static str>,
    /// `-f[no-]sanitize-memory-track-origins[=N]`.
    pub track_origins: Option<u32>,
    /// The last of `-shared-libsan` (true) / `-static-libsan`.
    pub shared_libsan: Option<bool>,
    /// The `-fsanitize-coverage=` (`false`) and `-fno-sanitize-coverage=`
    /// (`true`) values, in order (WP-1863).
    coverage: Vec<(bool, String)>,
    /// The `-fsanitize-coverage-allowlist=` (`true`) and `-ignorelist=`
    /// files, in order.
    pub coverage_lists: Vec<(bool, String)>,
    /// Each `-fsanitize-coverage-stack-depth-callback-min=` (clang 22's),
    /// as written, and its value; the last one counts.
    pub stack_depth_callback_min: Vec<(String, String)>,
    /// `AArch64` x18 is reserved: `-ffixed-x18` was given or the
    /// `--target=` OS reserves it ([`x18_reserved_by_default`]; set by
    /// the argument parser, WP-1151).
    pub x18_reserved: bool,
}

/// The resolved sanitizer state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sanitize {
    /// What codegen sees.
    pub opts: SanitizeOptions,
    /// `-fsanitize-address-use-after-scope` (default on).
    pub use_after_scope: bool,
    /// `-fsanitize-address-use-after-return=`.
    pub use_after_return: Option<&'static str>,
    /// `-fsanitize-memory-track-origins=` (0 when off).
    pub track_origins: u32,
    /// clang would enable `memtag-stack` (named or by the `memtag`
    /// group, not removed, and the target supports it), which kiln does
    /// not implement but checks for the `mte` feature
    /// ([`Sanitize::hardware_errors`]).
    pub memtag_stack: bool,
    /// The `-fsanitize-coverage=` features ([`cov`] bits; WP-1863), with
    /// `-fsanitize=fuzzer`'s and the implied insertion point.
    pub coverage: u32,
    /// `-fsanitize-coverage-stack-depth-callback-min=` (0: not given or
    /// invalid): `stack-depth` calls `__sanitizer_cov_stack_depth` only
    /// in functions with a frame at least this large.
    pub stack_depth_callback_min: u32,
}

/// clang's `CoverageFeature` bits (`SanitizerArgs.cpp`; WP-1863).
pub mod cov {
    /// `func`: function entry blocks.
    pub const FUNC: u32 = 1 << 0;
    /// `bb`: every block.
    pub const BB: u32 = 1 << 1;
    /// `edge`: blocks and critical edges.
    pub const EDGE: u32 = 1 << 2;
    /// `indirect-calls`.
    pub const INDIRECT_CALLS: u32 = 1 << 3;
    /// `trace-bb` (deprecated; LLVM ignores it).
    pub const TRACE_BB: u32 = 1 << 4;
    /// `trace-cmp`.
    pub const TRACE_CMP: u32 = 1 << 5;
    /// `trace-div`.
    pub const TRACE_DIV: u32 = 1 << 6;
    /// `trace-gep`.
    pub const TRACE_GEP: u32 = 1 << 7;
    /// `8bit-counters` (deprecated; LLVM ignores it).
    pub const COUNTERS_8BIT: u32 = 1 << 8;
    /// `trace-pc`.
    pub const TRACE_PC: u32 = 1 << 9;
    /// `trace-pc-guard`.
    pub const TRACE_PC_GUARD: u32 = 1 << 10;
    /// `no-prune`.
    pub const NO_PRUNE: u32 = 1 << 11;
    /// `inline-8bit-counters`.
    pub const INLINE_8BIT_COUNTERS: u32 = 1 << 12;
    /// `pc-table`.
    pub const PC_TABLE: u32 = 1 << 13;
    /// `stack-depth`.
    pub const STACK_DEPTH: u32 = 1 << 14;
    /// `inline-bool-flag`.
    pub const INLINE_BOOL_FLAG: u32 = 1 << 15;
    /// `trace-loads`.
    pub const TRACE_LOADS: u32 = 1 << 16;
    /// `trace-stores`.
    pub const TRACE_STORES: u32 = 1 << 17;
    /// `control-flow`.
    pub const CONTROL_FLOW: u32 = 1 << 18;

    /// The `-fsanitize-coverage=` values, in clang's order.
    pub const NAMES: [(&str, u32); 19] = [
        ("func", FUNC),
        ("bb", BB),
        ("edge", EDGE),
        ("indirect-calls", INDIRECT_CALLS),
        ("trace-bb", TRACE_BB),
        ("trace-cmp", TRACE_CMP),
        ("trace-div", TRACE_DIV),
        ("trace-gep", TRACE_GEP),
        ("8bit-counters", COUNTERS_8BIT),
        ("trace-pc", TRACE_PC),
        ("trace-pc-guard", TRACE_PC_GUARD),
        ("no-prune", NO_PRUNE),
        ("inline-8bit-counters", INLINE_8BIT_COUNTERS),
        ("inline-bool-flag", INLINE_BOOL_FLAG),
        ("pc-table", PC_TABLE),
        ("stack-depth", STACK_DEPTH),
        ("trace-loads", TRACE_LOADS),
        ("trace-stores", TRACE_STORES),
        ("control-flow", CONTROL_FLOW),
    ];

    /// The insertion points (`-fsanitize-coverage-type=` 1, 2, 3).
    pub const INSERTION_POINTS: u32 = FUNC | BB | EDGE;

    /// What `-fsanitize=fuzzer[-no-link]` turns on (Linux: with
    /// `stack-depth`).
    pub const FUZZER: u32 =
        INLINE_8BIT_COUNTERS | INDIRECT_CALLS | TRACE_CMP | PC_TABLE | STACK_DEPTH;
}

/// The pass sanitizers' runtimes, named on the link line.
const RUNTIMES: S = S(S::ADDRESS.0
    | S::HWADDRESS.0
    | S::THREAD.0
    | S::MEMORY.0
    | S::LEAK.0
    | S::FUZZER.0
    | S::FUZZER_NO_LINK.0);

/// The frontend-check sanitizers (the UB runtime).
const UB_CHECKS: S = S(S::UNDEFINED.0 | S::UNSIGNED_INTEGER_OVERFLOW.0);

/// clang's `IncompatibleGroups`, in its order.
const INCOMPATIBLE: &[(S, S)] = &[
    (S::ADDRESS, S(S::THREAD.0 | S::MEMORY.0)),
    (S::THREAD, S::MEMORY),
    (S::LEAK, S(S::THREAD.0 | S::MEMORY.0)),
    (S::KERNEL_ADDRESS, S(S::ADDRESS.0 | S::LEAK.0 | S::THREAD.0 | S::MEMORY.0)),
    (S::HWADDRESS, S(S::ADDRESS.0 | S::THREAD.0 | S::MEMORY.0 | S::KERNEL_ADDRESS.0)),
    // WP-1151: the rest of clang's table, over [`CLANG_CHECKED`] (Linux:
    // `safe-stack` with `leak` too, as off Fuchsia).
    (
        S::SCUDO,
        S(S::ADDRESS.0
            | S::HWADDRESS.0
            | S::LEAK.0
            | S::THREAD.0
            | S::MEMORY.0
            | S::KERNEL_ADDRESS.0),
    ),
    (
        S::SAFE_STACK,
        S(S::LEAK.0
            | S::ADDRESS.0
            | S::HWADDRESS.0
            | S::THREAD.0
            | S::MEMORY.0
            | S::KERNEL_ADDRESS.0),
    ),
    (
        S::KERNEL_HWADDRESS,
        S(S::ADDRESS.0
            | S::HWADDRESS.0
            | S::LEAK.0
            | S::THREAD.0
            | S::MEMORY.0
            | S::KERNEL_ADDRESS.0
            | S::SAFE_STACK.0),
    ),
    (
        S::KERNEL_MEMORY,
        S(S::ADDRESS.0
            | S::HWADDRESS.0
            | S::LEAK.0
            | S::THREAD.0
            | S::MEMORY.0
            | S::KERNEL_ADDRESS.0
            | S::SCUDO.0
            | S::SAFE_STACK.0),
    ),
    (S::MEMTAG, S(S::ADDRESS.0 | S::KERNEL_ADDRESS.0 | S::HWADDRESS.0 | S::KERNEL_HWADDRESS.0)),
    (S::KCFI, S::FUNCTION),
];

/// The sanitizers kiln does not implement that clang's driver checks
/// further (WP-1151): the [`INCOMPATIBLE`] pairs and the `AArch64`
/// `shadow-call-stack` / `-ffixed-x18` rule.
const CLANG_CHECKED: S = S(S::MEMTAG.0
    | S::SHADOW_CALL_STACK.0
    | S::KERNEL_ADDRESS.0
    | S::KERNEL_HWADDRESS.0
    | S::KERNEL_MEMORY.0
    | S::SCUDO.0
    | S::SAFE_STACK.0
    | S::KCFI.0);

/// LLVM's `AArch64::isX18ReservedByDefault` of a `--target=` triple (of
/// those kiln accepts: Android and `OpenHarmony`; also Darwin, Fuchsia and
/// Windows): `-fsanitize=shadow-call-stack` then needs no `-ffixed-x18`.
#[must_use]
pub fn x18_reserved_by_default(triple: &str) -> bool {
    let mut parts = triple.split('-').skip(1);
    parts.any(|p| {
        ["android", "ohos", "fuchsia", "windows", "darwin", "macos", "ios", "tvos", "watchos"]
            .iter()
            .any(|os| p.starts_with(os))
            || p == "apple"
    })
}

fn split(v: &str) -> Vec<String> {
    v.split(',').map(str::to_owned).collect()
}

impl SanitizeFlags {
    /// Takes `arg` if it is a sanitizer option: `None` if it is not, else
    /// whether it is accepted.
    pub fn parse_arg(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        let mut list = |list: List, opt: &'static str, v: &str| {
            self.lists.push(ListArg { list, opt, values: split(v) });
        };
        let lists: [(&'static str, List); 8] = [
            ("-fsanitize=", List::Add),
            ("-fno-sanitize=", List::Remove),
            ("-fsanitize-recover=", List::Recover),
            ("-fno-sanitize-recover=", List::NoRecover),
            ("-fsanitize-trap=", List::Trap),
            ("-fno-sanitize-trap=", List::NoTrap),
            ("-fsanitize-merge=", List::Merge),
            ("-fno-sanitize-merge=", List::NoMerge),
        ];
        for (opt, l) in lists {
            if let Some(v) = arg.strip_prefix(opt) {
                list(l, opt, v);
                return Some(Ok(()));
            }
        }
        match arg {
            // clang's aliases.
            "-fsanitize-recover" => list(List::Recover, "-fsanitize-recover=", "all"),
            "-fno-sanitize-recover" => list(List::NoRecover, "-fno-sanitize-recover=", "all"),
            "-fsanitize-trap" => list(List::Trap, "-fsanitize-trap=", "all"),
            "-fno-sanitize-trap" => list(List::NoTrap, "-fno-sanitize-trap=", "all"),
            "-fsanitize-merge" => list(List::Merge, "-fsanitize-merge=", "all"),
            "-fno-sanitize-merge" => list(List::NoMerge, "-fno-sanitize-merge=", "all"),
            "-fsanitize-undefined-trap-on-error" => {
                list(List::Trap, "-fsanitize-trap=", "undefined");
            }
            "-fno-sanitize-undefined-trap-on-error" => {
                list(List::NoTrap, "-fno-sanitize-trap=", "undefined");
            }
            "-fno-sanitize-ignorelist" | "-fno-sanitize-blacklist" => {
                self.ignorelists.clear();
                self.no_default_ignorelists = true;
            }
            "-fsanitize-address-use-after-scope" => self.use_after_scope = Some(true),
            "-fno-sanitize-address-use-after-scope" => self.use_after_scope = Some(false),
            "-fsanitize-memory-track-origins" => self.track_origins = Some(2),
            "-fno-sanitize-memory-track-origins" => self.track_origins = Some(0),
            "-shared-libsan" | "-shared-libasan" => self.shared_libsan = Some(true),
            "-static-libsan" => self.shared_libsan = Some(false),
            _ => return self.valued(arg),
        }
        Some(Ok(()))
    }

    fn valued(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        if let Some(f) = arg
            .strip_prefix("-fsanitize-ignorelist=")
            .or_else(|| arg.strip_prefix("-fsanitize-blacklist="))
        {
            self.ignorelists.push(f.to_owned());
        } else if let Some(v) = arg.strip_prefix("-fsanitize-address-use-after-return=") {
            self.use_after_return = Some(match v {
                "never" => "never",
                "runtime" => "runtime",
                "always" => "always",
                _ => {
                    return Some(Err(ArgError::UnsupportedValue(
                        "-fsanitize-address-use-after-return=".into(),
                        v.into(),
                    )));
                }
            });
        } else if let Some(v) = arg.strip_prefix("-fsanitize-memory-track-origins=") {
            match v {
                "0" | "1" | "2" => self.track_origins = v.parse().ok(),
                _ => return Some(Err(ArgError::InvalidFlagValue(arg.into(), v.into()))),
            }
        } else if let Some(v) = arg.strip_prefix("-fsanitize-coverage=") {
            self.coverage.push((false, v.to_owned()));
        } else if let Some(v) = arg.strip_prefix("-fno-sanitize-coverage=") {
            self.coverage.push((true, v.to_owned()));
        } else if let Some(f) = arg.strip_prefix("-fsanitize-coverage-allowlist=") {
            self.coverage_lists.push((true, f.to_owned()));
        } else if let Some(f) = arg.strip_prefix("-fsanitize-coverage-ignorelist=") {
            self.coverage_lists.push((false, f.to_owned()));
        } else if let Some(v) = arg.strip_prefix("-fsanitize-coverage-stack-depth-callback-min=") {
            self.stack_depth_callback_min.push((arg.to_owned(), v.to_owned()));
        } else if arg.starts_with("-fsanitize-coverage-")
            || arg.starts_with("-fno-sanitize-coverage-")
        {
            // clang's driver has no other: the retired `-whitelist=` /
            // `-blacklist=` spellings and the `-cc1` options are unknown.
            return Some(Err(ArgError::Unknown(arg.to_owned())));
        } else if arg.starts_with("-fsanitize-") || arg.starts_with("-fno-sanitize-") {
            // The minimal-runtime, CFI, … options.
            return Some(Err(ArgError::UnsupportedByKiln(arg.to_owned())));
        } else {
            return None;
        }
        Some(Ok(()))
    }

    /// Whether any sanitizer option was given.
    #[must_use]
    pub fn any(&self) -> bool {
        self.lists.iter().any(|l| l.list == List::Add)
    }

    /// The resolved sanitizers and the errors (clang `SanitizerArgs`):
    /// `rtti` is `-frtti`'s state, `opt0` whether optimization is off,
    /// `x86` the target's architecture and `triple` its name.
    #[must_use]
    #[allow(clippy::too_many_lines, reason = "clang's single resolution pass")]
    pub fn resolve(
        &self,
        rtti: bool,
        opt0: bool,
        x86: bool,
        triple: &str,
    ) -> (Sanitize, Vec<ArgError>) {
        let mut errors = Vec::new();
        let trap = self.trap_kinds(&mut errors);
        // clang's `getSupportedSanitizers` complement for the Linux
        // targets (WP-1138: x86-64 Linux has `shadow-call-stack`; no Linux
        // target has `objc-cast`).
        let target_unsupported = if x86 {
            S(S::MEMTAG.0 | S::OBJC_CAST.0)
        } else {
            S(S::NUMERICAL.0 | S::KERNEL_MEMORY.0 | S::OBJC_CAST.0)
        };
        let mut kinds = S::NONE;
        let mut all_remove = if opt0 { S::OBJECT_SIZE } else { S::NONE };
        let mut diagnosed = S::NONE;
        // clang's group bits: the groups a later `-fno-sanitize=` names
        // (`all` names every one) and the ones already diagnosed.
        let mut removed_groups: Vec<&str> = Vec::new();
        let mut diagnosed_groups: Vec<&str> = Vec::new();
        let mut memtag_stack = false;
        let mut clang_checked = S::NONE;
        for l in self.lists.iter().rev() {
            match l.list {
                List::Add => {
                    let (explicit, expanded) = parse_values(l, false, &mut errors);
                    let explicit = explicit.without(all_remove);
                    memtag_stack |= expanded.without(all_remove).has(S::MEMTAG_STACK)
                        && !target_unsupported.has(S::MEMTAG_STACK);
                    clang_checked = clang_checked.union(
                        expanded
                            .without(all_remove)
                            .without(target_unsupported)
                            .intersect(CLANG_CHECKED),
                    );
                    if let Some(k) = Some(explicit.intersect(S::VPTR).intersect(trap))
                        .filter(|k| !k.is_empty() && !diagnosed.has_any(*k))
                    {
                        errors.push(ArgError::NotAllowedWith(
                            describe(l, k),
                            "-fsanitize-trap=undefined".into(),
                        ));
                        diagnosed = diagnosed.union(k);
                    }
                    // clang `setGroupBits`: a group is supported when one
                    // of its members is, so a named group none of whose
                    // members the target has is diagnosed as a single
                    // sanitizer is (WP-1138: `-fsanitize=memtag` on x86-64).
                    let groups: Vec<&str> = l
                        .values
                        .iter()
                        .map(String::as_str)
                        .filter(|v| {
                            GROUPS.iter().any(|(n, s)| {
                                n == v
                                    && target_unsupported.has(*s)
                                    && !removed_groups.contains(n)
                                    && !diagnosed_groups.contains(n)
                            })
                        })
                        .collect();
                    let k = explicit.intersect(target_unsupported).without(diagnosed);
                    if !k.is_empty() || !groups.is_empty() {
                        let vals: Vec<&str> = l
                            .values
                            .iter()
                            .map(String::as_str)
                            .filter(|v| {
                                groups.contains(v)
                                    || parse_name(v, true).is_some_and(|s| s.has_any(k))
                            })
                            .collect();
                        errors.push(ArgError::UnsupportedForTarget(
                            format!("{}{}", l.opt, vals.join(",")),
                            triple.into(),
                        ));
                        diagnosed = diagnosed.union(k);
                        diagnosed_groups.extend(groups);
                    }
                    if explicit.has_any(S::VPTR) && !rtti {
                        errors.push(ArgError::NotAllowedWith(
                            "-fsanitize=vptr".into(),
                            "-fno-rtti".into(),
                        ));
                        all_remove = all_remove.union(S::VPTR);
                    }
                    let k = expanded
                        .without(all_remove)
                        .without(S::KILN_SUPPORTED)
                        .without(target_unsupported)
                        .without(diagnosed);
                    if !k.is_empty() {
                        errors.push(ArgError::UnsupportedByKiln(describe(l, k)));
                        diagnosed = diagnosed.union(k);
                    }
                    let mut add = expanded
                        .without(all_remove)
                        .without(trap.intersect(S::VPTR))
                        .intersect(S::KILN_SUPPORTED);
                    if !rtti {
                        add = add.without(S::VPTR);
                    }
                    if add.has_any(S::FUZZER) {
                        add = add.union(S::FUZZER_NO_LINK);
                    }
                    kinds = kinds.union(add);
                }
                List::Remove => {
                    all_remove = all_remove.union(parse_values(l, true, &mut errors).1);
                    for v in &l.values {
                        removed_groups.extend(
                            GROUPS.iter().filter(|(n, _)| v == "all" || n == v).map(|g| g.0),
                        );
                    }
                }
                _ => {}
            }
        }
        // WP-1151: clang checks the AArch64 shadow call stack's register
        // after the per-option errors, before the incompatible pairs.
        if !x86 && clang_checked.has(S::SHADOW_CALL_STACK) && !self.x18_reserved {
            errors.push(ArgError::OnlyAllowedWith(
                self.last_arg_for(S::SHADOW_CALL_STACK),
                "-ffixed-x18".into(),
            ));
        }
        // The pairs over what clang enables, kiln's sanitizers and the
        // checked others (WP-1151).
        let mut enabled = kinds.union(clang_checked);
        for &(group, incompatible) in INCOMPATIBLE {
            if enabled.has_any(group) {
                let bad = enabled.intersect(incompatible);
                if !bad.is_empty() {
                    errors.push(ArgError::NotAllowedWith(
                        self.last_arg_for(group),
                        self.last_arg_for(bad),
                    ));
                    enabled = enabled.without(bad);
                }
            }
        }
        kinds = kinds.intersect(enabled);
        // clang's `parseSanitizeArgs` for the recover lists.
        let mut recover = S::RECOVERABLE_BY_DEFAULT;
        for l in &self.lists {
            match l.list {
                List::Recover => {
                    let (explicit, expanded) = parse_values(l, true, &mut errors);
                    let bad = explicit.intersect(S::UNRECOVERABLE);
                    if !bad.is_empty() {
                        let names: Vec<_> = bad.names().collect();
                        errors.push(ArgError::UnsupportedValue(l.opt.into(), names.join(",")));
                    }
                    recover = recover.union(expanded);
                }
                List::NoRecover => recover = recover.without(parse_values(l, true, &mut errors).1),
                _ => {}
            }
        }
        // clang 20+ `parseSanitizeArgs` for the merge lists (default: the
        // `undefined` group).
        let mut merge = None;
        for l in &self.lists {
            match l.list {
                List::Merge => {
                    let add = parse_values(l, true, &mut errors).1;
                    merge = Some(merge.unwrap_or(S::UNDEFINED).union(add));
                }
                List::NoMerge => {
                    let remove = parse_values(l, true, &mut errors).1;
                    merge = Some(merge.unwrap_or(S::UNDEFINED).without(remove));
                }
                _ => {}
            }
        }
        let trap = trap.intersect(kinds);
        let recover = recover.intersect(kinds).without(S::UNRECOVERABLE).without(trap);
        let use_after_scope = self.use_after_scope.unwrap_or(true);
        let coverage = self.coverage_features(kinds.has_any(S::FUZZER_NO_LINK)).0;
        let s = Sanitize {
            opts: SanitizeOptions {
                kinds,
                recover,
                trap,
                merge: merge.map(|m| m.intersect(kinds)),
                use_after_scope: use_after_scope
                    && kinds.has_any(S::ADDRESS.union(S::KERNEL_ADDRESS)),
                coverage: coverage_on(coverage),
                ..SanitizeOptions::default()
            },
            use_after_scope,
            use_after_return: self.use_after_return,
            track_origins: self.track_origins.unwrap_or(0),
            memtag_stack,
            coverage,
            stack_depth_callback_min: self.callback_min().unwrap_or(0),
        };
        (s, errors)
    }

    /// The `-fsanitize-coverage-stack-depth-callback-min=` value, 0 when
    /// not given; `None` for one clang rejects (not an `int` by
    /// `getAsInteger(0)`, or negative).
    fn callback_min(&self) -> Option<u32> {
        self.stack_depth_callback_min
            .last()
            .map_or(Some(0), |(_, v)| legacy_int(v).and_then(|n| u32::try_from(n).ok()))
    }

    /// clang's `-f[no-]sanitize-coverage=` resolution (WP-1863): the
    /// features, starting from `-fsanitize=fuzzer`'s when `fuzzer`, then
    /// the options in order (a single number is the legacy form: it
    /// clears them, deprecated unless 0), the insertion point a feature
    /// implies (`edge`; `func` for `stack-depth`), and the diagnostics in
    /// clang's order.
    fn coverage_features(&self, fuzzer: bool) -> (u32, Vec<ArgError>) {
        let mut errors = Vec::new();
        let mut f = if fuzzer { cov::FUZZER } else { 0 };
        for (no, v) in &self.coverage {
            let opt = if *no { "-fno-sanitize-coverage=" } else { "-fsanitize-coverage=" };
            // clang's `CommaJoined` values: empty ones are dropped.
            let vals: Vec<&str> = v.split(',').filter(|v| !v.is_empty()).collect();
            if !*no
                && let [n] = vals[..]
                && let Some(n) = legacy_int(n)
            {
                f = 0;
                if n != 0 {
                    errors.push(ArgError::DeprecatedArg(
                        format!("{opt}{v}"),
                        "-fsanitize-coverage=trace-pc-guard".into(),
                    ));
                }
                continue;
            }
            let mut parsed = 0;
            for v in vals {
                match cov::NAMES.iter().find(|(n, _)| *n == v) {
                    Some((_, b)) => parsed |= b,
                    None => errors.push(ArgError::UnsupportedValue(opt.into(), v.into())),
                }
            }
            if *no {
                f &= !parsed;
            } else {
                f |= parsed;
            }
        }
        for (a, b) in [(cov::FUNC, cov::BB), (cov::FUNC, cov::EDGE), (cov::BB, cov::EDGE)] {
            if f & a != 0 && f & b != 0 {
                let name = |x| cov::NAMES.iter().find(|(_, b)| *b == x).map_or("", |n| n.0);
                errors.push(ArgError::NotAllowedWith(
                    format!("-fsanitize-coverage={}", name(a)),
                    format!("-fsanitize-coverage={}", name(b)),
                ));
            }
        }
        for (b, n) in [(cov::TRACE_BB, "trace-bb"), (cov::COUNTERS_8BIT, "8bit-counters")] {
            if f & b != 0 {
                errors.push(ArgError::DeprecatedArg(
                    format!("-fsanitize-coverage={n}"),
                    "-fsanitize-coverage=trace-pc-guard".into(),
                ));
            }
        }
        let instrumentation = cov::TRACE_PC
            | cov::TRACE_PC_GUARD
            | cov::INLINE_8BIT_COUNTERS
            | cov::TRACE_LOADS
            | cov::TRACE_STORES
            | cov::INLINE_BOOL_FLAG
            | cov::CONTROL_FLOW;
        if f & cov::INSERTION_POINTS != 0 && f & instrumentation == 0 {
            errors.push(ArgError::DeprecatedArg(
                "-fsanitize-coverage=[func|bb|edge]".into(),
                "-fsanitize-coverage=[func|bb|edge],[trace-pc-guard|trace-pc],[control-flow]"
                    .into(),
            ));
        }
        if f & cov::INSERTION_POINTS == 0 {
            if f & (cov::TRACE_PC
                | cov::TRACE_PC_GUARD
                | cov::INLINE_8BIT_COUNTERS
                | cov::INLINE_BOOL_FLAG
                | cov::CONTROL_FLOW)
                != 0
            {
                f |= cov::EDGE;
            }
            if f & cov::STACK_DEPTH != 0 {
                f |= cov::FUNC;
            }
        }
        (f, errors)
    }

    /// The `-fsanitize-coverage=` diagnostics and those of its lists
    /// (WP-1863), in clang's order: the options', then, with coverage
    /// on, each allowlist then ignorelist file that does not exist and
    /// the lists LLVM's `SpecialCaseList` rejects (read as clang 22 or 19
    /// does). `kinds` are the resolved sanitizers.
    #[must_use]
    pub fn coverage_errors(&self, kinds: S, clang22: bool) -> Vec<ArgError> {
        let mut errors = Vec::new();
        // clang 22 checks the callback minimum first (clang 19 does not
        // know the option: `Args::unknown_at_compat_19`).
        if let Some((a, v)) = self.stack_depth_callback_min.last()
            && clang22
            && self.callback_min().is_none()
        {
            errors.push(ArgError::InvalidFlagValue(a.clone(), v.clone()));
        }
        let (f, e) = self.coverage_features(kinds.has_any(S::FUZZER_NO_LINK));
        errors.extend(e);
        if f == 0 {
            return errors;
        }
        for allow in [true, false] {
            let mut existing = Vec::new();
            for (_, p) in self.coverage_lists.iter().filter(|l| l.0 == allow) {
                if self.work_dir.join_str(p).exists() {
                    existing.push(p.clone());
                } else {
                    errors.push(ArgError::NoSuchFile(p.clone()));
                }
            }
            if existing.is_empty() {
                continue;
            }
            let parsed = read_ignorelists(&existing, &self.work_dir)
                .and_then(|t| parse_ignorelists(&t, clang22));
            if let Err(ArgError::MalformedIgnorelist(m)) = parsed {
                errors.push(ArgError::MalformedCoverageList(allow, m));
            }
        }
        errors
    }

    /// The coverage lists codegen applies (WP-1863): `None` without
    /// coverage or lists, or when one is malformed
    /// ([`Self::coverage_errors`] reported it).
    #[must_use]
    pub fn coverage_lists(&self, s: &Sanitize, clang22: bool) -> Option<Arc<CoverageLists>> {
        if s.coverage == 0 {
            return None;
        }
        let read = |allow: bool| -> Option<Option<SanitizerIgnorelist>> {
            let existing: Vec<String> = self
                .coverage_lists
                .iter()
                .filter(|(a, p)| *a == allow && self.work_dir.join_str(p).exists())
                .map(|l| l.1.clone())
                .collect();
            if existing.is_empty() {
                return Some(None);
            }
            read_ignorelists(&existing, &self.work_dir)
                .and_then(|t| parse_ignorelists(&t, clang22))
                .ok()
                .map(Some)
        };
        let allow = read(true)?;
        let ignore = read(false)?;
        (allow.is_some() || ignore.is_some()).then(|| Arc::new(CoverageLists { allow, ignore }))
    }

    /// The coverage list options clang leaves unclaimed, as written:
    /// all of them when no coverage is on (WP-1863; "argument unused
    /// during compilation").
    #[must_use]
    pub fn unused_coverage_lists(&self, s: &Sanitize) -> Vec<String> {
        if s.coverage != 0 {
            return Vec::new();
        }
        self.coverage_lists
            .iter()
            .map(|(allow, f)| {
                let kind = if *allow { "allowlist" } else { "ignorelist" };
                format!("-fsanitize-coverage-{kind}={f}")
            })
            .collect()
    }

    /// clang's `parseSanitizeTrapArgs`: the trap lists, last first.
    fn trap_kinds(&self, errors: &mut Vec<ArgError>) -> S {
        let mut remove = S::NONE;
        let mut kinds = S::NONE;
        for l in self.lists.iter().rev() {
            match l.list {
                List::Trap => {
                    let mut invalid = Vec::new();
                    let mut add = S::NONE;
                    for v in &l.values {
                        match parse_name(v, true) {
                            None => {
                                errors.push(ArgError::UnsupportedValue(l.opt.into(), v.clone()));
                            }
                            Some(s) => {
                                let single = s.0.is_power_of_two();
                                let ok = if single {
                                    S::TRAPPING_SUPPORTED.has(s)
                                } else {
                                    S::TRAPPING_SUPPORTED.has_any(s)
                                };
                                if !ok && !remove.has(s) {
                                    invalid.push(v.as_str());
                                }
                                add = add.union(s);
                            }
                        }
                    }
                    if !invalid.is_empty() {
                        errors.push(ArgError::UnsupportedValue(l.opt.into(), invalid.join(",")));
                    }
                    kinds = kinds.union(add.intersect(S::TRAPPING_SUPPORTED).without(remove));
                }
                List::NoTrap => remove = remove.union(parse_values(l, true, errors).1),
                _ => {}
            }
        }
        kinds
    }

    /// clang's `lastArgumentForMask`: the last `-fsanitize=` that enables
    /// part of `mask`, shown with only those values.
    fn last_arg_for(&self, mut mask: S) -> String {
        for l in self.lists.iter().rev() {
            match l.list {
                List::Add => {
                    let set = values_set(l);
                    if set.has_any(mask) {
                        return describe(l, mask);
                    }
                }
                List::Remove => mask = mask.without(values_set(l)),
                _ => {}
            }
        }
        String::new()
    }
}

/// clang's `addDefaultIgnorelists`: the resource directory's
/// `share/<file>` lists and the sanitizers each one is for (kiln has no
/// CFI, whose missing list clang reports).
const DEFAULT_IGNORELISTS: &[(&str, S)] = &[
    ("asan_ignorelist.txt", S::ADDRESS),
    ("hwasan_ignorelist.txt", S::HWADDRESS),
    ("memtag_ignorelist.txt", S::MEMTAG),
    ("msan_ignorelist.txt", S::MEMORY),
    ("nsan_ignorelist.txt", S::NUMERICAL),
    ("tsan_ignorelist.txt", S::THREAD),
    ("dfsan_abilist.txt", S::DATAFLOW),
    (
        "ubsan_ignorelist.txt",
        S(S::UNDEFINED.0 | S::VPTR.0 | S::INTEGER.0 | S::NULLABILITY.0 | S::FLOAT_DIVIDE_BY_ZERO.0),
    ),
];

impl SanitizeFlags {
    /// The ignorelists that apply (spec/driver.md "Sanitizers", WP-1032):
    /// the existing `-fsanitize-ignorelist=` files, then the default
    /// lists of the enabled `kinds` under `resource_dir`.
    #[must_use]
    pub fn ignorelist_paths(&self, kinds: S, resource_dir: &str) -> (Vec<String>, Vec<String>) {
        let user = self
            .ignorelists
            .iter()
            .filter(|f| self.work_dir.join_str(f).exists())
            .cloned()
            .collect();
        let mut system = Vec::new();
        if !self.no_default_ignorelists {
            for &(file, mask) in DEFAULT_IGNORELISTS {
                let p = Path::new(resource_dir).join("share").join(file);
                if kinds.has_any(mask) && p.exists() {
                    system.push(p.display().to_string());
                }
            }
        }
        (user, system)
    }

    /// clang's `validateSpecialCaseListFormat`, of the user lists and of
    /// the default lists apart: "malformed sanitizer ignorelist" with
    /// LLVM's message (list versions read as clang 22 or 19 does).
    #[must_use]
    pub fn ignorelist_errors(&self, kinds: S, resource_dir: &str, clang22: bool) -> Vec<ArgError> {
        let (user, system) = self.ignorelist_paths(kinds, resource_dir);
        [system, user]
            .iter()
            .filter(|l| !l.is_empty())
            .filter_map(|l| {
                read_ignorelists(l, &self.work_dir)
                    .and_then(|t| parse_ignorelists(&t, clang22))
                    .err()
            })
            .collect()
    }

    /// The parsed ignorelists codegen applies, `None` when there is none
    /// (or one is malformed: [`Self::ignorelist_errors`] reported it).
    #[must_use]
    pub fn ignorelist(
        &self,
        kinds: S,
        resource_dir: &str,
        clang22: bool,
    ) -> Option<Arc<SanitizerIgnorelist>> {
        let (mut paths, system) = self.ignorelist_paths(kinds, resource_dir);
        paths.extend(system);
        if paths.is_empty() {
            return None;
        }
        let l = read_ignorelists(&paths, &self.work_dir)
            .and_then(|t| parse_ignorelists(&t, clang22))
            .ok()?;
        (!l.is_empty()).then(|| Arc::new(l))
    }
}

/// The lists' text, or LLVM's `can't open file` message.
fn read_ignorelists(
    paths: &[String],
    work_dir: &kiln_source::WorkDir,
) -> Result<Vec<(String, String)>, ArgError> {
    paths
        .iter()
        .map(|p| match std::fs::read(work_dir.join_str(p)) {
            Ok(b) => Ok((p.clone(), String::from_utf8_lossy(&b).into_owned())),
            Err(e) => {
                let msg = e.to_string();
                let msg = msg.split(" (os error").next().unwrap_or(&msg).to_owned();
                Err(ArgError::MalformedIgnorelist(format!("can't open file '{p}': {msg}")))
            }
        })
        .collect()
}

fn parse_ignorelists(
    files: &[(String, String)],
    clang22: bool,
) -> Result<SanitizerIgnorelist, ArgError> {
    SanitizerIgnorelist::parse(files, clang22)
        .map_err(|IgnorelistError::Malformed(m)| ArgError::MalformedIgnorelist(m))
}

/// clang `CodeGenOptions::hasSanitizeCoverage` of features `c`
/// ([`cov`]).
fn coverage_on(c: u32) -> bool {
    c & (cov::INSERTION_POINTS
        | cov::INDIRECT_CALLS
        | cov::TRACE_CMP
        | cov::TRACE_LOADS
        | cov::TRACE_STORES
        | cov::CONTROL_FLOW)
        != 0
}

/// LLVM `StringRef::getAsInteger(0, int)`: an optional `-`, then
/// digits in the base a `0x`, `0b`, `0o` or `0` prefix picks (else
/// decimal), the whole string, in `int`'s range.
fn legacy_int(s: &str) -> Option<i64> {
    let (neg, s) = s.strip_prefix('-').map_or((false, s), |r| (true, r));
    let lower = s.to_ascii_lowercase();
    let (radix, digits) = if let Some(r) = lower.strip_prefix("0x") {
        (16, r)
    } else if let Some(r) = lower.strip_prefix("0b") {
        (2, r)
    } else if let Some(r) = lower.strip_prefix("0o") {
        (8, r)
    } else if lower.len() > 1 && lower.starts_with('0') && lower.as_bytes()[1].is_ascii_digit() {
        (8, &lower[1..])
    } else {
        (10, lower.as_str())
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    let v = i64::from_str_radix(digits, radix).ok()?;
    let v = if neg { -v } else { v };
    i32::try_from(v).ok().map(i64::from)
}

/// The values' union, unknown names ignored.
fn values_set(l: &ListArg) -> S {
    l.values.iter().filter_map(|v| parse_name(v, l.list != List::Add)).fold(S::NONE, S::union)
}

/// The single sanitizers named outright and the expanded union, with an
/// error for each unknown name.
fn parse_values(l: &ListArg, allow_all: bool, errors: &mut Vec<ArgError>) -> (S, S) {
    let mut explicit = S::NONE;
    let mut expanded = S::NONE;
    for v in &l.values {
        match parse_name(v, allow_all) {
            Some(s) => {
                if kiln_ast::sanitize::SANITIZERS.iter().any(|(n, _)| n == v) {
                    explicit = explicit.union(s);
                }
                expanded = expanded.union(s);
            }
            None => errors.push(ArgError::UnsupportedValue(l.opt.into(), v.clone())),
        }
    }
    (explicit, expanded)
}

/// clang's `describeSanitizeArg`: the option with the values that enable
/// part of `mask`.
fn describe(l: &ListArg, mask: S) -> String {
    let vals: Vec<&str> = l
        .values
        .iter()
        .filter(|v| parse_name(v, true).is_some_and(|s| s.has_any(mask)))
        .map(String::as_str)
        .collect();
    format!("{}{}", l.opt, vals.join(","))
}

impl Sanitize {
    /// clang `SanitizerArgs::addArgs`'s check after the option errors:
    /// `memtag-stack` on `AArch64` needs the `mte` target feature
    /// (`-march=...+memtag`; WP-1138). `mte` is whether the target
    /// features finally enable it.
    #[must_use]
    pub fn hardware_errors(&self, aarch64: bool, mte: bool) -> Vec<ArgError> {
        if aarch64 && self.memtag_stack && !mte {
            vec![ArgError::MemtagStackRequiresMte]
        } else {
            Vec::new()
        }
    }

    /// The `-fsanitize=` the linking `clang++` gets, so that it adds the
    /// runtimes (docs/design/sanitizers.md §2): the pass sanitizers, and
    /// `undefined` when a UB check reports through the runtime.
    #[must_use]
    pub fn link_list(&self) -> Option<String> {
        let k = self.opts.kinds;
        let mut names: Vec<&str> = k.intersect(RUNTIMES).names().collect();
        if !k.intersect(UB_CHECKS).without(self.opts.trap).is_empty() {
            names.push("undefined");
        }
        (!names.is_empty()).then(|| format!("-fsanitize={}", names.join(",")))
    }

    /// The instrumentation passes clang's `addSanitizers` puts at the
    /// `OptimizerLast` extension point, and the `cl::opt` flags it sets
    /// (docs/design/sanitizers.md §1.3); `None` without a pass sanitizer.
    #[must_use]
    pub fn optimizer_last(&self, opt_level: u8) -> Option<(String, Vec<String>)> {
        let k = self.opts.kinds;
        let rec = self.opts.recover;
        let mut passes: Vec<String> = Vec::new();
        let mut cl: Vec<String> = Vec::new();
        // WP-1863: `SanitizerCoveragePass` with clang's options, through
        // the pass's `cl::opt`s (`OverrideFromCL`). It instruments nothing
        // without an insertion point, so it runs only with one.
        let c = self.coverage;
        let level = [(cov::EDGE, 3), (cov::BB, 2), (cov::FUNC, 1)]
            .iter()
            .find(|(b, _)| c & b != 0)
            .map_or(0, |l| l.1);
        if level != 0 {
            passes.push("sancov-module".into());
            // Level 4 is `edge` with indirect calls; the `cl::opt`s cannot
            // ask indirect calls with `func` or `bb`.
            let level = if level == 3 && c & cov::INDIRECT_CALLS != 0 { 4 } else { level };
            cl.push(format!("-sanitizer-coverage-level={level}"));
            for (b, flag) in [
                (cov::TRACE_CMP, "-sanitizer-coverage-trace-compares"),
                (cov::TRACE_DIV, "-sanitizer-coverage-trace-divs"),
                (cov::TRACE_GEP, "-sanitizer-coverage-trace-geps"),
                (cov::TRACE_PC, "-sanitizer-coverage-trace-pc"),
                (cov::TRACE_PC_GUARD, "-sanitizer-coverage-trace-pc-guard"),
                (cov::INLINE_8BIT_COUNTERS, "-sanitizer-coverage-inline-8bit-counters"),
                (cov::INLINE_BOOL_FLAG, "-sanitizer-coverage-inline-bool-flag"),
                (cov::PC_TABLE, "-sanitizer-coverage-pc-table"),
                (cov::NO_PRUNE, "-sanitizer-coverage-prune-blocks=0"),
                (cov::STACK_DEPTH, "-sanitizer-coverage-stack-depth"),
                (cov::TRACE_LOADS, "-sanitizer-coverage-trace-loads"),
                (cov::TRACE_STORES, "-sanitizer-coverage-trace-stores"),
                (cov::CONTROL_FLOW, "-sanitizer-coverage-control-flow"),
            ] {
                if c & b != 0 {
                    cl.push(flag.into());
                }
            }
            if self.stack_depth_callback_min != 0 {
                cl.push(format!(
                    "-sanitizer-coverage-stack-depth-callback-min={}",
                    self.stack_depth_callback_min
                ));
            }
        }
        if k.has_any(S::MEMORY) {
            let mut p = String::new();
            if rec.has_any(S::MEMORY) {
                p.push_str("recover;");
            }
            if self.track_origins != 0 {
                let _ = write!(p, "track-origins={};", self.track_origins);
            }
            p.push_str("eager-checks");
            passes.push(format!("msan<{p}>"));
            if opt_level > 0 {
                passes.push(
                    "function(early-cse<>,instcombine,jump-threading,gvn<>,instcombine)".into(),
                );
            }
        }
        if k.has_any(S::THREAD) {
            passes.push("tsan-module,function(tsan)".into());
        }
        if k.has_any(S::ADDRESS) {
            passes.push("asan<>".into());
            if self.use_after_scope {
                cl.push("-asan-use-after-scope".into());
            }
            if rec.has_any(S::ADDRESS) {
                cl.push("-asan-recover".into());
            }
            if let Some(m) = self.use_after_return {
                cl.push(format!("-asan-use-after-return={m}"));
            }
        }
        if k.has_any(S::HWADDRESS) {
            passes.push(
                if rec.has_any(S::HWADDRESS) { "hwasan<recover>" } else { "hwasan<>" }.into(),
            );
            if opt_level == 0 {
                cl.push("-hwasan-use-stack-safety=0".into());
            }
        }
        (!passes.is_empty()).then(|| (passes.join(","), cl))
    }

    /// The `-fsanitize=` options `ClangFeatures::from_args` and the
    /// predefines see: one per feature group the set touches (clang's
    /// `__has_feature(undefined_behavior_sanitizer)` is any `undefined`
    /// check), and `-fsanitize-coverage=on` for [`Self::has_coverage`]
    /// (`__has_feature(coverage_sanitizer)`).
    #[must_use]
    pub fn feature_args(&self) -> Vec<String> {
        let k = self.opts.kinds;
        [
            "address",
            "hwaddress",
            "thread",
            "memory",
            "undefined",
            "dataflow",
            "safe-stack",
            "leak",
            "scudo",
            "kcfi",
            "cfi",
            "function",
            "numerical",
        ]
        .iter()
        .filter(|n| parse_name(n, false).is_some_and(|s| k.has_any(s)))
        .map(|n| format!("-fsanitize={n}"))
        .chain(self.has_coverage().then(|| "-fsanitize-coverage=on".to_owned()))
        .collect()
    }

    /// clang `CodeGenOptions::hasSanitizeCoverage` (WP-1863): an insertion
    /// point, `indirect-calls`, `trace-cmp`, `trace-loads`, `trace-stores`
    /// or `control-flow` (`__has_feature(coverage_sanitizer)`, and
    /// `no_sanitize("coverage")` makes `nosanitize_coverage`).
    #[must_use]
    pub fn has_coverage(&self) -> bool {
        coverage_on(self.coverage)
    }

    /// The predefined macros clang 22 adds (`__SANITIZE_ADDRESS__`,
    /// `__SANITIZE_THREAD__`, `__SANITIZE_HWADDRESS__`); clang 19 has none.
    #[must_use]
    pub fn macros(&self, clang22: bool) -> Vec<(String, Option<String>)> {
        let k = self.opts.kinds;
        let mut v = Vec::new();
        if clang22 {
            for (s, m) in [
                (S(S::ADDRESS.0 | S::KERNEL_ADDRESS.0), "__SANITIZE_ADDRESS__"),
                (S(S::HWADDRESS.0 | S::KERNEL_HWADDRESS.0), "__SANITIZE_HWADDRESS__"),
                (S::THREAD, "__SANITIZE_THREAD__"),
            ] {
                if k.has_any(s) {
                    v.push((m.to_owned(), Some("1".to_owned())));
                }
            }
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(s: &str) -> (SanitizeFlags, Vec<ArgError>) {
        let mut f = SanitizeFlags::default();
        let mut e = Vec::new();
        for a in s.split_whitespace() {
            match f.parse_arg(a) {
                Some(Err(x)) => e.push(x),
                Some(Ok(())) => {}
                None => panic!("not a sanitizer flag: {a}"),
            }
        }
        (f, e)
    }

    fn resolve(s: &str) -> (Sanitize, Vec<ArgError>) {
        let (f, mut e) = flags(s);
        let (r, e2) = f.resolve(true, false, false, "aarch64-unknown-linux-gnu");
        e.extend(e2);
        (r, e)
    }

    fn uv(o: &str, v: &str) -> ArgError {
        ArgError::UnsupportedValue(o.into(), v.into())
    }

    #[test]
    fn groups_and_removal() {
        let (r, e) = resolve("-fsanitize=undefined -fno-sanitize=vptr,function");
        assert_eq!(e, []);
        assert!(r.opts.kinds.has(S::SIGNED_INTEGER_OVERFLOW));
        assert!(!r.opts.kinds.has_any(S(S::VPTR.0 | S::FUNCTION.0)));
        assert!(r.opts.recover.has(S::SHIFT_BASE));
        assert!(!r.opts.recover.has_any(S::RETURN));
        assert_eq!(r.link_list().as_deref(), Some("-fsanitize=undefined"));
        // A later `-fsanitize=` re-enables.
        let (r, _) = resolve("-fno-sanitize=shift -fsanitize=shift-base");
        assert_eq!(r.opts.kinds, S::SHIFT_BASE);
    }

    #[test]
    fn unknown_values() {
        let (_, e) = resolve("-fsanitize=foo,address -fsanitize=all -fsanitize-recover=bar");
        assert_eq!(
            e,
            [uv("-fsanitize=", "all"), uv("-fsanitize=", "foo"), uv("-fsanitize-recover=", "bar")]
        );
        let (_, e) = resolve("-fsanitize=undefined -fsanitize-trap=address,undefined");
        assert_eq!(e, [uv("-fsanitize-trap=", "address")]);
        let (_, e) = resolve("-fsanitize=undefined -fsanitize-recover=unreachable,address,thread");
        assert_eq!(e, [uv("-fsanitize-recover=", "unreachable")]);
    }

    #[test]
    fn incompatible() {
        let (r, e) = resolve("-fsanitize=address,undefined -fsanitize=thread");
        assert_eq!(
            e,
            [ArgError::NotAllowedWith("-fsanitize=address".into(), "-fsanitize=thread".into())]
        );
        assert!(r.opts.kinds.has(S::ADDRESS) && !r.opts.kinds.has_any(S::THREAD));
        let (_, e) = resolve("-fsanitize=memory,leak");
        assert_eq!(
            e,
            [ArgError::NotAllowedWith("-fsanitize=leak".into(), "-fsanitize=memory".into())]
        );
    }

    #[test]
    fn unsupported() {
        let (r, e) = resolve("-fsanitize=unsigned-integer-overflow,unsigned-shift-base");
        assert_eq!(e, []);
        assert!(
            r.opts.kinds.has(S::UNSIGNED_SHIFT_BASE) && r.opts.recover.has(S::UNSIGNED_SHIFT_BASE)
        );
        let (_, e) = resolve("-fsanitize=cfi-icall,address");
        assert_eq!(e, [ArgError::UnsupportedByKiln("-fsanitize=cfi-icall".into())]);
        let (_, e) = resolve("-fsanitize=cfi -fno-sanitize=cfi");
        assert_eq!(e, []);
        let (_, e) = resolve("-fsanitize=numerical");
        assert_eq!(
            e,
            [ArgError::UnsupportedForTarget(
                "-fsanitize=numerical".into(),
                "aarch64-unknown-linux-gnu".into()
            )]
        );
        let (f, _) = flags("-fsanitize=vptr");
        let (_, e) = f.resolve(false, false, false, "t");
        assert_eq!(e, [ArgError::NotAllowedWith("-fsanitize=vptr".into(), "-fno-rtti".into())]);
        let (f, _) = flags("-fsanitize=undefined");
        let (r, e) = f.resolve(false, true, false, "t");
        assert_eq!(e, []);
        assert!(!r.opts.kinds.has_any(S(S::VPTR.0 | S::OBJECT_SIZE.0)));
        let (_, e) = flags("-fsanitize-minimal-runtime");
        assert_eq!(e, [ArgError::UnsupportedByKiln("-fsanitize-minimal-runtime".into())]);
    }

    /// clang's `-f[no-]sanitize-coverage=` resolution and the `opt`
    /// flags it maps to (WP-1863).
    #[test]
    fn coverage() {
        let feats = |s: &str, fuzzer: bool| flags(s).0.coverage_features(fuzzer);
        let (f, e) = feats("-fsanitize-coverage=trace-pc-guard,trace-cmp", false);
        assert_eq!((f, e), (cov::TRACE_PC_GUARD | cov::TRACE_CMP | cov::EDGE, vec![]));
        let (f, _) = feats("-fsanitize-coverage=stack-depth", false);
        assert_eq!(f, cov::STACK_DEPTH | cov::FUNC);
        let (f, _) = feats("-fno-sanitize-coverage=trace-cmp", true);
        // Both implied points: the counters' `edge`, `stack-depth`'s `func`.
        assert_eq!(f, (cov::FUZZER & !cov::TRACE_CMP) | cov::EDGE | cov::FUNC);
        // The legacy integers clear the features.
        let (f, e) = feats("-fsanitize-coverage=trace-pc -fsanitize-coverage=0x2", false);
        let dep = |a: &str| {
            ArgError::DeprecatedArg(a.into(), "-fsanitize-coverage=trace-pc-guard".into())
        };
        assert_eq!((f, e), (0, vec![dep("-fsanitize-coverage=0x2")]));
        assert_eq!(feats("-fsanitize-coverage=0", false), (0, vec![]));
        assert_eq!(
            (legacy_int("010"), legacy_int("0b11"), legacy_int("x")),
            (Some(8), Some(3), None)
        );
        let (_, e) = feats("-fsanitize-coverage=func,edge,trace-bb,bogus", false);
        assert_eq!(
            e,
            [
                uv("-fsanitize-coverage=", "bogus"),
                ArgError::NotAllowedWith(
                    "-fsanitize-coverage=func".into(),
                    "-fsanitize-coverage=edge".into()
                ),
                dep("-fsanitize-coverage=trace-bb"),
                ArgError::DeprecatedArg(
                    "-fsanitize-coverage=[func|bb|edge]".into(),
                    "-fsanitize-coverage=[func|bb|edge],[trace-pc-guard|trace-pc],[control-flow]"
                        .into()
                ),
            ]
        );
        let (r, e) = resolve("-fsanitize-coverage=trace-pc-guard,indirect-calls,no-prune");
        assert_eq!(e, []);
        assert!(r.has_coverage() && r.opts.coverage);
        let (p, cl) = r.optimizer_last(0).unwrap();
        assert_eq!(p, "sancov-module");
        assert_eq!(
            cl,
            [
                "-sanitizer-coverage-level=4",
                "-sanitizer-coverage-trace-pc-guard",
                "-sanitizer-coverage-prune-blocks=0"
            ]
        );
        let (r, e) = resolve(
            "-fsanitize-coverage=stack-depth -fsanitize-coverage-stack-depth-callback-min=0x20",
        );
        assert_eq!(e, []);
        assert_eq!(
            r.optimizer_last(0).unwrap().1,
            [
                "-sanitizer-coverage-level=1",
                "-sanitizer-coverage-stack-depth",
                "-sanitizer-coverage-stack-depth-callback-min=32"
            ]
        );
        let (f, _) = flags("-fsanitize-coverage-stack-depth-callback-min=-1");
        assert_eq!(
            f.coverage_errors(S::NONE, true),
            [ArgError::InvalidFlagValue(
                "-fsanitize-coverage-stack-depth-callback-min=-1".into(),
                "-1".into()
            )]
        );
        assert_eq!(f.coverage_errors(S::NONE, false), []);
        // No insertion point: nothing to instrument.
        let (r, _) = resolve("-fsanitize-coverage=trace-cmp");
        assert!(r.has_coverage() && r.optimizer_last(0).is_none());
        let (f, _) = flags("-fsanitize-coverage-allowlist=a.txt");
        assert_eq!(
            f.unused_coverage_lists(&resolve("").0),
            ["-fsanitize-coverage-allowlist=a.txt"]
        );
    }

    /// clang 19 and 22 `SanitizerArgs` on x86-64 and `AArch64` Linux
    /// (WP-1138): a group none of whose members the target supports is
    /// "unsupported option ... for target" as a single sanitizer is, and
    /// `memtag-stack` on `AArch64` needs the `mte` feature.
    #[test]
    fn memtag_target_support() {
        const X86: &str = "x86_64-unknown-linux-gnu";
        let x86 = |s: &str| {
            let (f, mut e) = flags(s);
            let (r, e2) = f.resolve(true, false, true, X86);
            e.extend(e2);
            (r, e)
        };
        let t = |o: &str| ArgError::UnsupportedForTarget(o.into(), X86.into());
        for v in ["memtag", "memtag-stack", "memtag-heap", "memtag-globals"] {
            let o = format!("-fsanitize={v}");
            let (r, e) = x86(&o);
            assert_eq!(e, [t(&o)], "{o}");
            assert!(!r.memtag_stack && r.hardware_errors(false, false).is_empty());
        }
        // `describeSanitizeArg`: the values naming the diagnosed kinds;
        // the group bit is diagnosed apart from its members.
        assert_eq!(x86("-fsanitize=memtag-stack,memtag").1, [t("-fsanitize=memtag-stack,memtag")]);
        assert_eq!(
            x86("-fsanitize=memtag-heap,memtag-stack -fsanitize=address,memtag").1,
            [t("-fsanitize=memtag"), t("-fsanitize=memtag-heap,memtag-stack")]
        );
        // A later `-fno-sanitize=` of the group (or `all`) removes it; of
        // its members only, not.
        assert_eq!(x86("-fsanitize=memtag -fno-sanitize=memtag").1, []);
        assert_eq!(x86("-fsanitize=memtag -fno-sanitize=all").1, []);
        assert_eq!(
            x86("-fsanitize=memtag -fno-sanitize=memtag-stack,memtag-heap,memtag-globals").1,
            [t("-fsanitize=memtag")]
        );
        assert_eq!(x86("-fno-sanitize=memtag -fsanitize=memtag").1, [t("-fsanitize=memtag")]);
        // x86-64 Linux has `shadow-call-stack` (kiln does not implement
        // it); no Linux target has `objc-cast`.
        assert_eq!(
            x86("-fsanitize=shadow-call-stack").1,
            [ArgError::UnsupportedByKiln("-fsanitize=shadow-call-stack".into())]
        );
        assert_eq!(x86("-fsanitize=objc-cast").1, [t("-fsanitize=objc-cast")]);
        // AArch64: memtag is the target's, not kiln's; `memtag-stack`
        // also needs `+memtag`.
        let (r, e) = resolve("-fsanitize=memtag");
        assert_eq!(e, [ArgError::UnsupportedByKiln("-fsanitize=memtag".into())]);
        assert!(r.memtag_stack);
        assert_eq!(r.hardware_errors(true, false), [ArgError::MemtagStackRequiresMte]);
        assert_eq!(r.hardware_errors(true, true), []);
        assert!(resolve("-fsanitize=memtag-stack").0.memtag_stack);
        assert!(!resolve("-fsanitize=memtag-heap,memtag-globals").0.memtag_stack);
        assert!(!resolve("-fsanitize=memtag -fno-sanitize=memtag-stack").0.memtag_stack);
    }

    /// spec/driver.md "Sanitizers" (WP-1151): clang's `IncompatibleGroups`
    /// over the sanitizers kiln does not implement (memtag, the kernel
    /// ones) and the `AArch64` shadow call stack's `-ffixed-x18`.
    #[test]
    fn clang_checked_pairs() {
        let errs = |s: &str| {
            resolve(s)
                .1
                .into_iter()
                .filter(|e| !matches!(e, ArgError::UnsupportedByKiln(_)))
                .collect::<Vec<_>>()
        };
        let na = |a: &str, b: &str| ArgError::NotAllowedWith(a.into(), b.into());
        let x18 =
            ArgError::OnlyAllowedWith("-fsanitize=shadow-call-stack".into(), "-ffixed-x18".into());
        assert_eq!(
            errs("-fsanitize=memtag,address"),
            [na("-fsanitize=memtag", "-fsanitize=address")]
        );
        assert_eq!(
            errs("-fsanitize=address -fsanitize=memtag-stack"),
            [na("-fsanitize=memtag-stack", "-fsanitize=address")]
        );
        assert_eq!(
            errs("-fsanitize=memtag-heap,kernel-hwaddress"),
            [na("-fsanitize=memtag-heap", "-fsanitize=kernel-hwaddress")]
        );
        // `KernelHWAddress` comes first and takes `address` away.
        assert_eq!(
            errs("-fsanitize=memtag,kernel-hwaddress,address"),
            [
                na("-fsanitize=kernel-hwaddress", "-fsanitize=address"),
                na("-fsanitize=memtag", "-fsanitize=kernel-hwaddress"),
            ]
        );
        assert_eq!(errs("-fsanitize=memtag,address -fno-sanitize=memtag").len(), 0);
        // The kiln sanitizer of a bad pair is dropped.
        assert!(!resolve("-fsanitize=memtag,address").0.opts.kinds.has_any(S::ADDRESS));
        assert_eq!(errs("-fsanitize=shadow-call-stack"), std::slice::from_ref(&x18));
        assert_eq!(errs("-fsanitize=shadow-call-stack -fno-sanitize=shadow-call-stack"), []);
        // Before the pairs.
        assert_eq!(
            errs("-fsanitize=address,thread,shadow-call-stack"),
            [x18, na("-fsanitize=address", "-fsanitize=thread")]
        );
        let (mut f, _) = flags("-fsanitize=shadow-call-stack");
        f.x18_reserved = true;
        assert!(
            f.resolve(true, false, false, "aarch64-unknown-linux-gnu")
                .1
                .iter()
                .all(|e| matches!(e, ArgError::UnsupportedByKiln(_)))
        );
        // x86-64 needs no `-ffixed-x18`.
        let (f, _) = flags("-fsanitize=shadow-call-stack");
        assert_eq!(f.resolve(true, false, true, "x86_64-unknown-linux-gnu").1.len(), 1);
        for (t, r) in [
            ("aarch64-linux-gnu", false),
            ("aarch64-unknown-linux-android", true),
            ("aarch64-linux-android21", true),
            ("aarch64-linux-ohos", true),
            ("aarch64-unknown-fuchsia", true),
            ("arm64-apple-darwin", true),
            ("aarch64-pc-windows-msvc", true),
            ("aarch64-none-elf", false),
        ] {
            assert_eq!(x18_reserved_by_default(t), r, "{t}");
        }
    }

    #[test]
    fn trap_and_recover() {
        let (r, e) = resolve(
            "-fsanitize=undefined,address -fsanitize-trap=undefined -fno-sanitize-trap=shift",
        );
        assert_eq!(e, []);
        assert!(r.opts.trap.has(S::SIGNED_INTEGER_OVERFLOW));
        assert!(!r.opts.trap.has_any(S::SHIFT));
        assert!(
            r.opts.recover.has(S::SHIFT) && !r.opts.recover.has_any(S::SIGNED_INTEGER_OVERFLOW)
        );
        let (r, _) = resolve("-fsanitize=signed-integer-overflow -fsanitize-trap=all");
        assert_eq!(r.link_list(), None);
        let (r, _) = resolve("-fsanitize=address -fsanitize-recover=address");
        let (p, cl) = r.optimizer_last(0).unwrap();
        assert_eq!(p, "asan<>");
        assert_eq!(cl, ["-asan-use-after-scope", "-asan-recover"]);
        let (r, _) = resolve("-fsanitize=memory -fsanitize-memory-track-origins");
        assert_eq!(r.optimizer_last(0).unwrap().0, "msan<track-origins=2;eager-checks>");
        let (r, _) = resolve("-fsanitize=hwaddress");
        assert_eq!(
            r.optimizer_last(0),
            Some(("hwasan<>".into(), vec!["-hwasan-use-stack-safety=0".to_owned()]))
        );
        assert_eq!(r.link_list().as_deref(), Some("-fsanitize=hwaddress"));
    }

    #[test]
    fn features_and_macros() {
        let (r, _) = resolve("-fsanitize=shift,address");
        assert_eq!(r.feature_args(), ["-fsanitize=address", "-fsanitize=undefined"]);
        assert_eq!(r.macros(false), []);
        assert_eq!(r.macros(true), [("__SANITIZE_ADDRESS__".to_owned(), Some("1".to_owned()))]);
    }
}
