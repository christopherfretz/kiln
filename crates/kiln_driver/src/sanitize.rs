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
use kiln_codegen::ignorelist::{IgnorelistError, SanitizerIgnorelist};

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

/// clang's `IncompatibleGroups`, the ones kiln can reach.
const INCOMPATIBLE: &[(S, S)] = &[
    (S::ADDRESS, S(S::THREAD.0 | S::MEMORY.0)),
    (S::THREAD, S::MEMORY),
    (S::LEAK, S(S::THREAD.0 | S::MEMORY.0)),
    (S::KERNEL_ADDRESS, S(S::ADDRESS.0 | S::LEAK.0 | S::THREAD.0 | S::MEMORY.0)),
    (S::HWADDRESS, S(S::ADDRESS.0 | S::THREAD.0 | S::MEMORY.0 | S::KERNEL_ADDRESS.0)),
    // WP-1151.
    (
        S::KERNEL_HWADDRESS,
        S(S::ADDRESS.0
            | S::HWADDRESS.0
            | S::LEAK.0
            | S::THREAD.0
            | S::MEMORY.0
            | S::KERNEL_ADDRESS.0),
    ),
    (S::MEMTAG, S(S::ADDRESS.0 | S::KERNEL_ADDRESS.0 | S::HWADDRESS.0 | S::KERNEL_HWADDRESS.0)),
];

/// The sanitizers kiln does not implement that clang's driver checks
/// further (WP-1151): the [`INCOMPATIBLE`] pairs and the AArch64
/// `shadow-call-stack` / `-ffixed-x18` rule.
const CLANG_CHECKED: S =
    S(S::MEMTAG.0 | S::SHADOW_CALL_STACK.0 | S::KERNEL_ADDRESS.0 | S::KERNEL_HWADDRESS.0);

/// LLVM's `AArch64::isX18ReservedByDefault` of a `--target=` triple (of
/// those kiln accepts: Android and OpenHarmony; also Darwin, Fuchsia and
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
        } else if arg.starts_with("-fsanitize-") || arg.starts_with("-fno-sanitize-") {
            // The coverage, minimal-runtime, CFI, … options.
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
        let s = Sanitize {
            opts: SanitizeOptions {
                kinds,
                recover,
                trap,
                merge: merge.map(|m| m.intersect(kinds)),
                use_after_scope: use_after_scope
                    && kinds.has_any(S::ADDRESS.union(S::KERNEL_ADDRESS)),
                ..SanitizeOptions::default()
            },
            use_after_scope,
            use_after_return: self.use_after_return,
            track_origins: self.track_origins.unwrap_or(0),
            memtag_stack,
        };
        (s, errors)
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
        let user = self.ignorelists.iter().filter(|f| Path::new(f).exists()).cloned().collect();
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
    /// LLVM's message.
    #[must_use]
    pub fn ignorelist_errors(&self, kinds: S, resource_dir: &str) -> Vec<ArgError> {
        let (user, system) = self.ignorelist_paths(kinds, resource_dir);
        [system, user]
            .iter()
            .filter(|l| !l.is_empty())
            .filter_map(|l| read_ignorelists(l).and_then(|t| parse_ignorelists(&t)).err())
            .collect()
    }

    /// The parsed ignorelists codegen applies, `None` when there is none
    /// (or one is malformed: [`Self::ignorelist_errors`] reported it).
    #[must_use]
    pub fn ignorelist(&self, kinds: S, resource_dir: &str) -> Option<Arc<SanitizerIgnorelist>> {
        let (mut paths, system) = self.ignorelist_paths(kinds, resource_dir);
        paths.extend(system);
        if paths.is_empty() {
            return None;
        }
        let l = read_ignorelists(&paths).and_then(|t| parse_ignorelists(&t)).ok()?;
        (!l.is_empty()).then(|| Arc::new(l))
    }
}

/// The lists' text, or LLVM's `can't open file` message.
fn read_ignorelists(paths: &[String]) -> Result<Vec<(String, String)>, ArgError> {
    paths
        .iter()
        .map(|p| match std::fs::read(p) {
            Ok(b) => Ok((p.clone(), String::from_utf8_lossy(&b).into_owned())),
            Err(e) => {
                let msg = e.to_string();
                let msg = msg.split(" (os error").next().unwrap_or(&msg).to_owned();
                Err(ArgError::MalformedIgnorelist(format!("can't open file '{p}': {msg}")))
            }
        })
        .collect()
}

fn parse_ignorelists(files: &[(String, String)]) -> Result<SanitizerIgnorelist, ArgError> {
    SanitizerIgnorelist::parse(files).map_err(|e| match e {
        IgnorelistError::Malformed(m) => ArgError::MalformedIgnorelist(m),
        IgnorelistError::Regex(p) => ArgError::RegexIgnorelist(p),
    })
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
        if k.has_any(S::FUZZER_NO_LINK) {
            passes.push("sancov-module".into());
            cl.extend(
                [
                    "-sanitizer-coverage-level=3",
                    "-sanitizer-coverage-inline-8bit-counters",
                    "-sanitizer-coverage-pc-table",
                    "-sanitizer-coverage-trace-compares",
                    "-sanitizer-coverage-stack-depth",
                ]
                .map(str::to_owned),
            );
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
    /// check).
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
        .collect()
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
        let (_, e) = flags("-fsanitize-coverage=trace-pc");
        assert_eq!(e, [ArgError::UnsupportedByKiln("-fsanitize-coverage=trace-pc".into())]);
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
        assert_eq!(errs("-fsanitize=shadow-call-stack"), [x18.clone()]);
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
