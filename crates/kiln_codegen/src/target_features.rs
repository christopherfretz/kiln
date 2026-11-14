//! Per-function target features: the `target("...")` attribute and
//! clang's `checkTargetFeatures` (spec/landed/codegen/WP-558.md "Target
//! features", WP-558, for `AArch64`; spec/landed/codegen/WP-630.md, WP-630,
//! for x86-64).

use std::collections::BTreeMap;

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::builtin::{arm, neon, x86};
use kiln_ast::ids::RawId;
use kiln_ast::{DeclId, ExprId, StringLitId};
use kiln_diag::{Diag, DiagId};
use kiln_source::Span;

use crate::module::Cg;

/// A feature name to whether it is on.
pub(crate) type FeatureMap = BTreeMap<String, bool>;

impl Cg<'_> {
    /// The `target("...")` string of function `d`: the first redeclaration
    /// carrying one (an instantiation carries its pattern's attributes).
    pub(crate) fn target_attr_str(&self, d: DeclId) -> Option<String> {
        let ast = self.ast;
        ast.redecls_of(d).find_map(|r| {
            ast.lists.iter::<kiln_ast::AttrId>(ast.attrs.of_decl(r)).find_map(|a| {
                let n = ast.attrs.get(a);
                let l = StringLitId::from_raw(n.a).filter(|_| n.kind == AttrKind::TARGET)?;
                Some(String::from_utf8_lossy(ast.strings.bytes(l)).into_owned())
            })
        })
    }

    /// The features of `d`'s `target("...")` attribute, if it has one and
    /// the driver gave a parser ([`crate::CodegenOptions::target_attr`]).
    pub(crate) fn target_attr_features(&self, d: DeclId) -> Option<crate::TargetAttrFeatures> {
        let parse = self.opts.target_attr?;
        let s = self.target_attr_str(d)?;
        let module: Vec<String> = self
            .feature_map(None)
            .into_iter()
            .map(|(n, on)| format!("{}{n}", if on { '+' } else { '-' }))
            .collect();
        let m = crate::TargetAttrModule {
            features: &module,
            cpu_features: &self.opts.target_cpu_features,
            written: &self.opts.target_features_written,
            v22: self.opts.clang_compat >= 22,
        };
        Some(parse(&s, &m))
    }

    /// The `AArch64` features of function `d` (`None`: the module's): the
    /// base `fp-armv8`, `neon`, `outline-atomics` (per
    /// [`crate::CodegenOptions::outline_atomics`]), `v8a` and `-fmv`, then
    /// [`crate::CodegenOptions::target_features`], then the features of
    /// `d`'s `target("...")` attribute (clang `getFunctionFeatureMap`).
    pub(crate) fn feature_map(&self, d: Option<DeclId>) -> FeatureMap {
        if self.is_x86() {
            return self.x86_feature_map(d);
        }
        let mut f: FeatureMap = [("fp-armv8", true), ("neon", true), ("v8a", true), ("fmv", false)]
            .into_iter()
            .map(|(n, on)| (n.to_owned(), on))
            .collect();
        if let Some(on) = self.opts.outline_atomics {
            f.insert("outline-atomics".to_owned(), on);
        }
        let attr = d.and_then(|d| self.target_attr_features(d));
        let attr = attr.iter().flat_map(|a| &a.features);
        for e in self.opts.target_features.iter().chain(attr) {
            // clang's driver adds its `-fmv` after any `-Xclang
            // -target-feature -Xclang +fmv`, so `fmv` stays off (WP-810,
            // src/fmv.rs in `kiln_sema`).
            if let Some((sign, name)) = e.split_at_checked(1)
                && name != "fmv"
            {
                f.insert(name.to_owned(), sign == "+");
            }
        }
        f
    }

    /// The x86-64 features of function `d` (`None`: the module's): the
    /// module's map ([`crate::CodegenOptions::target_features`];
    /// `x86-64`'s without one), or the whole map of `d`'s `target("...")`
    /// attribute (clang `getFunctionFeatureMap`: the CPU's features, the
    /// command line's as written, then the attribute's; WP-1000).
    fn x86_feature_map(&self, d: Option<DeclId>) -> FeatureMap {
        const X86_64: [&str; 7] = ["cmov", "cx8", "fxsr", "mmx", "sse", "sse2", "x87"];
        // The command-line features that are no CPU's (WP-813: the
        // retpoline thunks of `-mspeculative-load-hardening`).
        const OPTION_FEATURES: [&str; 1] = ["+retpoline-indirect-calls"];
        let mut f: FeatureMap = FeatureMap::new();
        let attr = d.and_then(|d| self.target_attr_features(d));
        let features = if let Some(a) = &attr {
            &a.features
        } else {
            if self.opts.target_features.iter().all(|e| OPTION_FEATURES.contains(&&**e)) {
                f.extend(X86_64.iter().map(|n| ((*n).to_owned(), true)));
            }
            &self.opts.target_features
        };
        for e in features {
            if let Some((sign, name)) = e.split_at_checked(1) {
                f.insert(name.to_owned(), sign == "+");
            }
        }
        f
    }

    /// The name clang's `checkTargetFeatures` diagnostics print for `d`
    /// (`FunctionDecl::getDeclName`, unqualified): an identifier,
    /// `operator==`, `operator()` for a lambda's call operator, the class
    /// name for a constructor, `~S`, `operator int`.
    fn feature_diag_name(&self, d: DeclId) -> String {
        let mut out = String::new();
        kiln_ast::dump::AstNames::new(self.ast).write_name(d, &mut out);
        out
    }

    /// clang `checkTargetFeatures` for the call `e` of NEON builtin `b`
    /// from the current function: its required-feature expression must
    /// hold for the function's features, else `err_builtin_needs_feature`
    /// at the call. Returns whether it holds.
    pub(crate) fn neon_builtin_features_ok(&mut self, b: u32, e: ExprId) -> bool {
        let Some(row) = neon::row(b) else { return true };
        let on = self.feature_map(self.f.decl);
        if neon::features_hold(row.features, |n| on.get(n) == Some(&true)) {
            return true;
        }
        let d = Diag::new(DiagId::cg_builtin_needs_feature, Span::empty(self.ast.begin_loc(e)))
            .arg(row.info.name.to_owned())
            .arg(row.features.to_owned());
        self.diags.push(d);
        false
    }

    /// clang `checkTargetFeatures` for the call `e` of ACLE builtin `row`
    /// (WP-724), as [`Self::neon_builtin_features_ok`].
    pub(crate) fn arm_builtin_features_ok(&mut self, row: &arm::Row, e: ExprId) -> bool {
        if row.features.is_empty() {
            return true;
        }
        let on = self.feature_map(self.f.decl);
        if neon::features_hold(row.features, |n| on.get(n) == Some(&true)) {
            return true;
        }
        let d = Diag::new(DiagId::cg_builtin_needs_feature, Span::empty(self.ast.begin_loc(e)))
            .arg(row.info.name.to_owned())
            .arg(row.features.to_owned());
        self.diags.push(d);
        false
    }

    /// clang `checkTargetFeatures` for the call `e` (arguments `args`) of
    /// x86 builtin `row` from the current function: its required-feature
    /// expression must hold for the function's features (an SSE compare
    /// builtin's predicate above 7 needs `avx` instead), else
    /// `err_builtin_needs_feature` at the call. Returns whether it holds.
    pub(crate) fn x86_builtin_features_ok(
        &mut self,
        row: &x86::Row,
        e: ExprId,
        args: &[ExprId],
    ) -> bool {
        let name = row.info.name;
        let mut req = row.features;
        if matches!(
            name,
            "__builtin_ia32_cmpps"
                | "__builtin_ia32_cmppd"
                | "__builtin_ia32_cmpss"
                | "__builtin_ia32_cmpsd"
        ) && args.get(2).and_then(|&a| self.fold_int(a)).is_some_and(|v| v > 7)
        {
            req = "avx";
        }
        if req.is_empty() {
            return true;
        }
        // clang 22 has no `evex512` feature, and its builtin table no
        // `,evex512` term (kiln's table is clang 19's; WP-1242).
        let req = if self.opts.clang_compat >= 22 {
            std::borrow::Cow::Owned(req.replace(",evex512", ""))
        } else {
            std::borrow::Cow::Borrowed(req)
        };
        let on = self.feature_map(self.f.decl);
        if neon::features_hold(&req, |n| on.get(n) == Some(&true)) {
            return true;
        }
        let d = Diag::new(DiagId::cg_builtin_needs_feature, Span::empty(self.ast.begin_loc(e)))
            .arg(name.to_owned())
            .arg(req.into_owned());
        self.diags.push(d);
        false
    }

    /// clang `EmitCall` + `checkTargetFeatures` for the direct call `e` of
    /// `callee` (`AArch64`): an `always_inline` callee, when it or the
    /// current function has a `target("...")` attribute, needs every
    /// feature it is compiled with to be on in the caller, else
    /// `err_function_needs_feature` at the call naming the first missing
    /// one: the callee attribute's `+` features in order, then the rest
    /// of its features (sorted; clang walks a hash map). A callee without
    /// the attribute is checked feature by feature, one error each.
    pub(crate) fn target_feature_call_check(&mut self, e: ExprId, callee: DeclId) {
        if self.opts.target_attr.is_none() || !self.has_always_inline_attr(callee) {
            return;
        }
        let Some(from) = self.f.decl else { return };
        let callee_attr = self.target_attr_features(callee);
        if callee_attr.is_none() && self.target_attr_str(from).is_none() {
            return;
        }
        let have = self.feature_map(Some(from));
        let need = self.feature_map(Some(callee));
        let missing: Vec<String> = match &callee_attr {
            Some(a) => {
                let direct = a.features.iter().filter_map(|f| f.strip_prefix('+'));
                let all = need.iter().filter(|e| *e.1).map(|e| e.0.as_str());
                direct
                    .filter(|f| need.get(*f) == Some(&true))
                    .chain(all)
                    .find(|f| have.get(*f) != Some(&true))
                    .map(str::to_owned)
                    .into_iter()
                    .collect()
            }
            None => need
                .iter()
                .filter(|e| *e.1 && have.get(e.0) != Some(&true))
                .map(|e| e.0.clone())
                .collect(),
        };
        let span = Span::empty(self.ast.begin_loc(e));
        for f in missing {
            let d = Diag::new(DiagId::cg_function_needs_feature, span)
                .arg(self.feature_diag_name(from))
                .arg(self.feature_diag_name(callee))
                .arg(f);
            self.diags.push(d);
        }
    }
}
