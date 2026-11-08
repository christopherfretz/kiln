//! `AArch64` branch protection (spec/driver.md "Codegen flags", WP-846):
//! the `-mbranch-protection=` / `target("branch-protection=...")` spec
//! of LLVM 19 `ARM::parseBranchProtection` and what follows from it: the
//! function attributes of clang `TargetCodeGenInfo::
//! initBranchProtectionFnAttributes`, the module flags of
//! `CodeGenModule::Release` and the `__ARM_FEATURE_*_DEFAULT` macros of
//! `AArch64TargetInfo::getTargetDefines`.

/// Which functions sign their return address (`-msign-return-address=`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SignScope {
    /// No function (`none`, the default).
    #[default]
    None,
    /// Functions that spill the link register (`non-leaf`).
    NonLeaf,
    /// Every function (`all`, `pac-ret+leaf`).
    All,
}

impl SignScope {
    /// The `-msign-return-address=` value naming the scope.
    pub fn name(self) -> &'static str {
        match self {
            SignScope::None => "none",
            SignScope::NonLeaf => "non-leaf",
            SignScope::All => "all",
        }
    }

    /// The scope a `-msign-return-address=` value names.
    pub fn from_name(s: &str) -> Option<SignScope> {
        ["none", "non-leaf", "all"].iter().position(|n| *n == s).map(|i| {
            [SignScope::None, SignScope::NonLeaf, SignScope::All][i]
        })
    }
}

/// A parsed branch protection spec (clang `BranchProtectionInfo`).
#[allow(clippy::struct_excessive_bools, reason = "independent protections")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BranchProtection {
    /// Return address signing.
    pub scope: SignScope,
    /// Signing uses the B key (`b-key`), else the A key.
    pub b_key: bool,
    /// Branch target identification (`bti`).
    pub bti: bool,
    /// PAuth-LR (`pac-ret+pc`).
    pub pauth_lr: bool,
    /// The guarded control stack (`gcs`).
    pub gcs: bool,
}

impl BranchProtection {
    /// LLVM `ARM::parseBranchProtection`: `none`, `standard`, or a
    /// `+`-separated list of `bti`, `pac-ret` (followed by any of
    /// `leaf`, `b-key`, `pc`) and `gcs`, each trimmed. `standard` is
    /// `pac-ret+bti+gcs`, with `pc` when `pauth_lr` (the `-march=`
    /// enables PAuth-LR). The error is the offending item (`<empty>` for
    /// an empty one).
    pub fn parse(spec: &str, pauth_lr: bool) -> Result<BranchProtection, String> {
        let mut p = BranchProtection::default();
        if spec == "none" {
            return Ok(p);
        }
        if spec == "standard" {
            p.scope = SignScope::NonLeaf;
            p.bti = true;
            p.gcs = true;
            p.pauth_lr = pauth_lr;
            return Ok(p);
        }
        let opts: Vec<&str> = spec.split('+').map(str::trim).collect();
        let mut i = 0;
        while i < opts.len() {
            match opts[i] {
                "bti" => p.bti = true,
                "gcs" => p.gcs = true,
                "pac-ret" => {
                    p.scope = SignScope::NonLeaf;
                    while let Some(o) = opts.get(i + 1) {
                        match *o {
                            "leaf" => p.scope = SignScope::All,
                            "b-key" => p.b_key = true,
                            "pc" => p.pauth_lr = true,
                            _ => break,
                        }
                        i += 1;
                    }
                }
                "" => return Err("<empty>".to_owned()),
                o => return Err(o.to_owned()),
            }
            i += 1;
        }
        Ok(p)
    }

    /// The `-msign-return-address-key=` value: `a_key` / `b_key`.
    pub fn key_name(self) -> &'static str {
        if self.b_key { "b_key" } else { "a_key" }
    }

    /// The function attributes (clang `initBranchProtectionFnAttributes`)
    /// as `(name, value)`, `""` for a valueless one.
    pub fn fn_attrs(self) -> Vec<(&'static str, &'static str)> {
        let mut v = Vec::new();
        if self.scope != SignScope::None {
            v.push(("sign-return-address", self.scope.name()));
            v.push(("sign-return-address-key", self.key_name()));
        }
        if self.bti {
            v.push(("branch-target-enforcement", ""));
        }
        if self.pauth_lr {
            v.push(("branch-protection-pauth-lr", ""));
        }
        if self.gcs {
            v.push(("guarded-control-stack", ""));
        }
        v
    }

    /// The module flags (behaviour `Min`, value 1) of clang
    /// `CodeGenModule::Release`, in its order.
    pub fn module_flags(self) -> Vec<&'static str> {
        let signing = self.scope != SignScope::None;
        [
            (self.bti, "branch-target-enforcement"),
            (self.pauth_lr, "branch-protection-pauth-lr"),
            (self.gcs, "guarded-control-stack"),
            (signing, "sign-return-address"),
            (self.scope == SignScope::All, "sign-return-address-all"),
            (self.b_key, "sign-return-address-with-bkey"),
        ]
        .into_iter()
        .filter_map(|(on, n)| on.then_some(n))
        .collect()
    }

    /// The predefined macros (clang `AArch64TargetInfo::
    /// getTargetDefines`): `__ARM_FEATURE_PAC_DEFAULT` (bit 0 the A key,
    /// bit 1 the B key, bit 2 every function, bit 3 PAuth-LR) when
    /// signing, `__ARM_FEATURE_BTI_DEFAULT` and `__ARM_FEATURE_GCS_DEFAULT`.
    pub fn macros(self) -> Vec<(&'static str, String)> {
        let mut v = Vec::new();
        if self.scope != SignScope::None {
            let mut bits = if self.b_key { 2 } else { 1 };
            if self.scope == SignScope::All {
                bits |= 4;
            }
            if self.pauth_lr {
                bits |= 8;
            }
            v.push(("__ARM_FEATURE_PAC_DEFAULT", bits.to_string()));
        }
        if self.bti {
            v.push(("__ARM_FEATURE_BTI_DEFAULT", "1".to_owned()));
        }
        if self.gcs {
            v.push(("__ARM_FEATURE_GCS_DEFAULT", "1".to_owned()));
        }
        v
    }

    /// The last `branch-protection=` item of a `target("...")` string,
    /// trimmed (clang `AArch64TargetInfo::parseTargetAttr`), if any.
    pub fn of_target_attr(attr: &str) -> Option<&str> {
        attr.split(',')
            .map(str::trim)
            .filter_map(|f| f.strip_prefix("branch-protection="))
            .last()
            .map(str::trim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<BranchProtection, String> {
        BranchProtection::parse(s, false)
    }

    #[test]
    fn parse_specs() {
        assert_eq!(p("none"), Ok(BranchProtection::default()));
        let std = p("standard").unwrap();
        assert!(std.bti && std.gcs && !std.pauth_lr && std.scope == SignScope::NonLeaf);
        assert!(BranchProtection::parse("standard", true).unwrap().pauth_lr);
        let k = p("pac-ret+leaf+b-key+bti").unwrap();
        assert!(k.b_key && k.bti && k.scope == SignScope::All);
        assert!(p("pac-ret+pc").unwrap().pauth_lr);
        assert!(p(" bti + gcs ").unwrap().gcs);
        assert_eq!(p("x"), Err("x".to_owned()));
        assert_eq!(p("bti+"), Err("<empty>".to_owned()));
        assert_eq!(p(""), Err("<empty>".to_owned()));
        assert_eq!(p("pac-ret+bogus"), Err("bogus".to_owned()));
        assert_eq!(p("leaf"), Err("leaf".to_owned()));
    }

    #[test]
    fn derived() {
        let k = p("pac-ret+leaf+b-key+bti+gcs").unwrap();
        assert_eq!(
            k.module_flags(),
            [
                "branch-target-enforcement",
                "guarded-control-stack",
                "sign-return-address",
                "sign-return-address-all",
                "sign-return-address-with-bkey"
            ]
        );
        assert_eq!(k.macros()[0], ("__ARM_FEATURE_PAC_DEFAULT", "6".to_owned()));
        assert_eq!(p("pac-ret+pc").unwrap().macros()[0].1, "9");
        assert_eq!(k.fn_attrs()[1], ("sign-return-address-key", "b_key"));
        assert_eq!(
            BranchProtection::of_target_attr("arch=armv8.5-a, branch-protection= bti ,x"),
            Some("bti")
        );
        assert_eq!(SignScope::from_name("non-leaf"), Some(SignScope::NonLeaf));
    }
}
