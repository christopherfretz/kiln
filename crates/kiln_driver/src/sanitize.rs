//! The sanitizer options (spec/driver.md "Sanitizers", WP-675;
//! docs/design/sanitizers.md §2): `-f[no-]sanitize=`, the recover and
//! trap lists, the ignorelists, the AddressSanitizer and
//! MemorySanitizer options and the runtime choice, resolved the way
//! clang 19's `SanitizerArgs` does.

use kiln_ast::sanitize::{SanitizerSet as S, parse_name};
use kiln_codegen::SanitizeOptions;

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
    /// The last of `-f[no-]sanitize-address-use-after-scope`.
    pub use_after_scope: Option<bool>,
    /// `-fsanitize-address-use-after-return=<mode>`.
    pub use_after_return: Option<&'static str>,
    /// `-f[no-]sanitize-memory-track-origins[=N]`.
    pub track_origins: Option<u32>,
    /// The last of `-shared-libsan` (true) / `-static-libsan`.
    pub shared_libsan: Option<bool>,
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
];

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
        let lists: [(&'static str, List); 6] = [
            ("-fsanitize=", List::Add),
            ("-fno-sanitize=", List::Remove),
            ("-fsanitize-recover=", List::Recover),
            ("-fno-sanitize-recover=", List::NoRecover),
            ("-fsanitize-trap=", List::Trap),
            ("-fno-sanitize-trap=", List::NoTrap),
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
            "-fsanitize-undefined-trap-on-error" => {
                list(List::Trap, "-fsanitize-trap=", "undefined");
            }
            "-fno-sanitize-undefined-trap-on-error" => {
                list(List::NoTrap, "-fno-sanitize-trap=", "undefined");
            }
            "-fno-sanitize-ignorelist" | "-fno-sanitize-blacklist" => self.ignorelists.clear(),
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
    pub fn resolve(&self, rtti: bool, opt0: bool, x86: bool, triple: &str) -> (Sanitize, Vec<ArgError>) {
        let mut errors = Vec::new();
        let trap = self.trap_kinds(&mut errors);
        let target_unsupported = if x86 {
            S(S::SHADOW_CALL_STACK.0 | S::MEMTAG.0)
        } else {
            S(S::NUMERICAL.0 | S::KERNEL_MEMORY.0)
        };
        let mut kinds = S::NONE;
        let mut all_remove = if opt0 { S::OBJECT_SIZE } else { S::NONE };
        let mut diagnosed = S::NONE;
        for l in self.lists.iter().rev() {
            match l.list {
                List::Add => {
                    let (explicit, expanded) = parse_values(l, false, &mut errors);
                    let explicit = explicit.without(all_remove);
                    if let Some(k) = Some(explicit.intersect(S::VPTR).intersect(trap))
                        .filter(|k| !k.is_empty() && !diagnosed.has_any(*k))
                    {
                        errors.push(ArgError::NotAllowedWith(
                            describe(l, k),
                            "-fsanitize-trap=undefined".into(),
                        ));
                        diagnosed = diagnosed.union(k);
                    }
                    let k = explicit.intersect(target_unsupported).without(diagnosed);
                    if !k.is_empty() {
                        errors.push(ArgError::UnsupportedForTarget(describe(l, k), triple.into()));
                        diagnosed = diagnosed.union(k);
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
                }
                _ => {}
            }
        }
        for &(group, incompatible) in INCOMPATIBLE {
            if kinds.has_any(group) {
                let bad = kinds.intersect(incompatible);
                if !bad.is_empty() {
                    errors.push(ArgError::NotAllowedWith(
                        self.last_arg_for(group),
                        self.last_arg_for(bad),
                    ));
                    kinds = kinds.without(bad);
                }
            }
        }
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
        let trap = trap.intersect(kinds);
        let recover = recover.intersect(kinds).without(S::UNRECOVERABLE).without(trap);
        let s = Sanitize {
            opts: SanitizeOptions { kinds, recover, trap },
            use_after_scope: self.use_after_scope.unwrap_or(true),
            use_after_return: self.use_after_return,
            track_origins: self.track_origins.unwrap_or(0),
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
                            None => errors.push(ArgError::UnsupportedValue(l.opt.into(), v.clone())),
                            Some(s) => {
                                let single = s.0.count_ones() == 1;
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

/// The values' union, unknown names ignored.
fn values_set(l: &ListArg) -> S {
    l.values
        .iter()
        .filter_map(|v| parse_name(v, l.list != List::Add))
        .fold(S::NONE, S::union)
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
                p.push_str(&format!("track-origins={};", self.track_origins));
            }
            p.push_str("eager-checks");
            passes.push(format!("msan<{p}>"));
            if opt_level > 0 {
                passes.push("function(early-cse<>,instcombine,jump-threading,gvn<>,instcombine)".into());
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
            passes.push(if rec.has_any(S::HWADDRESS) { "hwasan<recover>" } else { "hwasan<>" }.into());
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

    #[test]
    fn trap_and_recover() {
        let (r, e) = resolve("-fsanitize=undefined,address -fsanitize-trap=undefined -fno-sanitize-trap=shift");
        assert_eq!(e, []);
        assert!(r.opts.trap.has(S::SIGNED_INTEGER_OVERFLOW));
        assert!(!r.opts.trap.has_any(S::SHIFT));
        assert!(r.opts.recover.has(S::SHIFT) && !r.opts.recover.has_any(S::SIGNED_INTEGER_OVERFLOW));
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
