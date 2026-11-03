//! The `AArch64` target features of `-march=`, `-mcpu=` and
//! `-target-feature` (spec/driver.md "Target features").
//!
//! kiln models the features clang's driver derives from the base
//! architecture `armv8-a` and the CPU `generic`, plus the `sve` extension:
//! `+sve` turns on `+fullfp16` and `+sve`, `+nosve` turns `sve` off again
//! only if an earlier extension turned it on. `-march=` wins over
//! `-mcpu=` (whose extensions are then ignored, as clang's are); the
//! `-target-feature` list applies after both, the last naming a feature
//! winning. Any other architecture, CPU or extension is an unsupported
//! value.

use crate::args::{ArgError, Args};

/// The resolved target features beyond the target's base set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetFeatures {
    /// `sve` is finally on: the SVE types are usable and function
    /// definitions carry `vscale_range(1,16)`.
    pub sve: bool,
    /// The extra `+name` / `-name` entries for the functions'
    /// `target-features`, one per name (the last wins), in the order the
    /// names first appeared.
    pub extra: Vec<String>,
}

impl TargetFeatures {
    fn set(&mut self, name: &str, on: bool) {
        let entry = format!("{}{name}", if on { '+' } else { '-' });
        match self.extra.iter_mut().find(|e| &e[1..] == name) {
            Some(e) => *e = entry,
            None => self.extra.push(entry),
        }
        if name == "sve" {
            self.sve = on;
        }
    }

    fn is_on(&self, name: &str) -> bool {
        self.extra.iter().any(|e| e.starts_with('+') && &e[1..] == name)
    }

    /// Applies the `+ext` list of `-march=`/`-mcpu=` (case-insensitive,
    /// as clang's), or returns `None` for an extension kiln does not know.
    fn extensions(&mut self, exts: &str) -> Option<()> {
        for e in exts.split('+') {
            match e.to_ascii_lowercase().as_str() {
                "sve" => {
                    self.set("fullfp16", true);
                    self.set("sve", true);
                }
                "nosve" if self.is_on("sve") => self.set("sve", false),
                "nosve" => {}
                _ => return None,
            }
        }
        Some(())
    }
}

/// Resolves the target features of `args` for a target that is `AArch64`
/// (`aarch64`) or not. On a non-`AArch64` target `-march=`/`-mcpu=` are
/// unsupported values and `-target-feature` is ignored. Every error is
/// returned; the features are those of the valid options.
pub fn resolve(args: &Args, aarch64: bool) -> (TargetFeatures, Vec<ArgError>) {
    let mut f = TargetFeatures::default();
    let mut errors = Vec::new();
    let unsupported = |opt: &str, v: &str| ArgError::UnsupportedValue(opt.to_owned(), v.to_owned());
    if !aarch64 {
        if let Some(v) = &args.march {
            errors.push(unsupported("-march=", v));
        }
        if let Some(v) = &args.mcpu {
            errors.push(unsupported("-mcpu=", v));
        }
        return (f, errors);
    }
    if let Some(v) = &args.march {
        let (base, exts) = v.split_once('+').map_or((v.as_str(), None), |(b, e)| (b, Some(e)));
        let ok = base.eq_ignore_ascii_case("armv8-a")
            && exts.is_none_or(|e| f.extensions(e).is_some());
        if !ok {
            f = TargetFeatures::default();
            errors.push(unsupported("-march=", v));
        }
    } else if let Some(v) = &args.mcpu {
        let (base, exts) = v.split_once('+').map_or((v.as_str(), None), |(b, e)| (b, Some(e)));
        // `generic` is clang's `-mcpu=` default CPU plus the `ete` and
        // `trbe` features it names explicitly.
        f.set("ete", true);
        f.set("trbe", true);
        let ok = base.eq_ignore_ascii_case("generic")
            && exts.is_none_or(|e| f.extensions(e).is_some());
        if !ok {
            f = TargetFeatures::default();
            errors.push(unsupported("-mcpu=", v));
        }
    }
    for t in &args.target_features {
        match t.split_at_checked(1) {
            Some(("+", name)) if !name.is_empty() => f.set(name, true),
            Some(("-", name)) if !name.is_empty() => f.set(name, false),
            _ => errors.push(unsupported("-target-feature", t)),
        }
    }
    (f, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res(s: &str, aarch64: bool) -> (TargetFeatures, Vec<ArgError>) {
        let a = Args::parse(&s.split_whitespace().map(str::to_owned).collect::<Vec<_>>());
        assert_eq!(a.errors, []);
        resolve(&a, aarch64)
    }

    /// `"<sve> <extra,...>"`.
    fn feats(s: &str) -> String {
        let (f, e) = res(s, true);
        assert_eq!(e, []);
        format!("{} {}", f.sve, f.extra.join(","))
    }

    #[test]
    fn march_sve() {
        assert_eq!(feats("a.cpp"), "false ");
        assert_eq!(feats("-march=armv8-a a.cpp"), "false ");
        assert_eq!(feats("-march=armv8-a+sve a.cpp"), "true +fullfp16,+sve");
        assert_eq!(feats("-march=ARMv8-A+SVE a.cpp"), "true +fullfp16,+sve");
        assert_eq!(feats("-march=armv8-a+nosve a.cpp"), "false ");
        assert_eq!(feats("-march=armv8-a+sve+nosve a.cpp"), "false +fullfp16,-sve");
        // The last `-march=` wins.
        assert_eq!(feats("-march=armv8-a+sve -march=armv8-a a.cpp"), "false ");
    }

    #[test]
    fn mcpu_and_march() {
        assert_eq!(feats("-mcpu=generic a.cpp"), "false +ete,+trbe");
        assert_eq!(
            feats("-mcpu=generic+sve a.cpp"),
            "true +ete,+trbe,+fullfp16,+sve"
        );
        // `-march=` wins and `-mcpu=`'s extensions are ignored.
        assert_eq!(feats("-mcpu=generic+sve -march=armv8-a a.cpp"), "false ");
        assert_eq!(feats("-march=armv8-a+sve -mcpu=generic a.cpp"), "true +fullfp16,+sve");
    }

    #[test]
    fn target_feature_last_wins() {
        assert_eq!(feats("-Xclang -target-feature -Xclang +sve a.cpp"), "true +sve");
        assert_eq!(feats("-target-feature -sve a.cpp"), "false -sve");
        assert_eq!(
            feats("-march=armv8-a+sve -Xclang -target-feature -Xclang -sve a.cpp"),
            "false +fullfp16,-sve"
        );
        assert_eq!(feats("-target-feature +sve -target-feature -sve a.cpp"), "false -sve");
    }

    #[test]
    fn unsupported_values() {
        let u = |o: &str, v: &str| ArgError::UnsupportedValue(o.into(), v.into());
        assert_eq!(res("-march=armv8-a+foo a.cpp", true).1, [u("-march=", "armv8-a+foo")]);
        assert_eq!(res("-march=armv9-a a.cpp", true).1, [u("-march=", "armv9-a")]);
        assert_eq!(res("-mcpu=cortex-a57 a.cpp", true).1, [u("-mcpu=", "cortex-a57")]);
        assert_eq!(res("-target-feature sve a.cpp", true).1, [u("-target-feature", "sve")]);
        assert_eq!(res("-march=armv8-a+sve a.cpp", false).1, [u("-march=", "armv8-a+sve")]);
        // `-target-feature` is ignored on a non-`AArch64` target.
        assert_eq!(res("-target-feature +sve a.cpp", false), (TargetFeatures::default(), vec![]));
        assert_eq!(res("-march=armv8-a+foo a.cpp", true).0, TargetFeatures::default());
    }
}
