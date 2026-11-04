//! Symbol visibility and `dso_local` (spec/codegen.md "Symbol
//! visibility"): clang's `CodeGenModule::setGlobalVisibility`,
//! `getLVForDecl` and `shouldAssumeDSOLocal`.

use kiln_llvm::{Linkage, Module, Visibility};

use crate::RelocModel;
use crate::module::Cg;

/// The inputs of clang's `shouldAssumeDSOLocal` for one global value.
struct GvFacts<'n> {
    name: &'n str,
    linkage: Linkage,
    visibility: Visibility,
    /// A declaration for the linker (`available_externally` counts).
    decl: bool,
    function: bool,
    tls: bool,
    comdat: bool,
}

/// The `dso_local` decision of clang 19's `shouldAssumeDSOLocal` (ELF):
/// `None` keeps what emission chose (a definition under `-fpie`).
fn assume_dso_local(
    g: &GvFacts<'_>,
    reloc: RelocModel,
    semantic_interposition: Option<bool>,
    direct_access: bool,
) -> Option<bool> {
    if g.linkage.is_local()
        || (g.visibility != Visibility::Default && g.linkage != Linkage::ExternWeak)
    {
        // Implied by the linkage or visibility; the printer omits it.
        return None;
    }
    // An intrinsic, or the `extern_weak` `_ZTH` declaration (clang
    // creates it without `setGVProperties`; kiln has no other
    // `extern_weak` symbol).
    if g.name.starts_with("llvm.") || g.linkage == Linkage::ExternWeak {
        return Some(false);
    }
    match reloc {
        // Not an executable: only a function definition that can use a
        // local alias, under `-fno-semantic-interposition`
        // (`GlobalValue::canBenefitFromLocalAlias`).
        RelocModel::Pic(_) => Some(
            semantic_interposition == Some(false)
                && g.function
                && !g.decl
                && g.linkage == Linkage::External
                && !g.comdat,
        ),
        // A definition cannot be preempted from an executable. Under
        // `-fpie` clang decides when it creates the global, which may be
        // before it has an initializer (`_ZGR` of a dynamically
        // initialized temporary): emission made that choice already.
        RelocModel::Static if !g.decl => Some(true),
        RelocModel::Pie(_) if !g.decl => None,
        _ => Some(
            direct_access
                && ((!g.function && !g.tls)
                    || (g.function && reloc == RelocModel::Static)),
        ),
    }
}

impl Cg<'_> {
    /// Whether non-TLS variable declarations are directly accessed
    /// (`-f[no-]direct-access-external-data`; on by default only for
    /// `-fno-pic`).
    pub(crate) fn direct_access_external_data(&self) -> bool {
        self.opts.direct_access_external_data.unwrap_or(self.opts.reloc == RelocModel::Static)
    }

    /// The last emission step: every global value of local linkage has
    /// default visibility (LLVM's verifier rule, clang
    /// `setGlobalVisibility`), and its `dso_local` follows the relocation
    /// model ([`assume_dso_local`]).
    pub(crate) fn finish_dso_local(&mut self) {
        let (reloc, si) = (self.opts.reloc, self.opts.semantic_interposition);
        let da = self.direct_access_external_data();
        finish_module(&mut self.m, reloc, si, da);
    }
}

fn finish_module(m: &mut Module, reloc: RelocModel, si: Option<bool>, da: bool) {
    let avail = |l: Linkage| l == Linkage::AvailableExternally;
    let fids: Vec<_> = m.functions().map(|(id, _)| id).collect();
    for id in fids {
        let f = m.func(id);
        let facts = GvFacts {
            name: f.name(),
            linkage: f.linkage,
            visibility: f.visibility,
            decl: f.is_declaration() || avail(f.linkage),
            function: true,
            tls: false,
            comdat: f.comdat.is_some(),
        };
        let d = assume_dso_local(&facts, reloc, si, da);
        let f = m.func_mut(id);
        if f.linkage.is_local() {
            f.visibility = Visibility::Default;
        }
        if let Some(d) = d {
            f.dso_local = d;
        }
    }
    let gids: Vec<_> = m.global_ids().collect();
    for id in gids {
        let g = m.global(id);
        let facts = GvFacts {
            name: g.name(),
            linkage: g.linkage,
            visibility: g.visibility,
            decl: g.initializer().is_none() || avail(g.linkage),
            function: false,
            tls: g.thread_local.is_some(),
            comdat: g.comdat.is_some(),
        };
        let d = assume_dso_local(&facts, reloc, si, da);
        let g = m.global_mut(id);
        if g.linkage.is_local() {
            g.visibility = Visibility::Default;
        }
        if let Some(d) = d {
            g.dso_local = d;
        }
    }
    let aids: Vec<_> = m.alias_ids().collect();
    for id in aids {
        let a = m.alias(id);
        let facts = GvFacts {
            name: a.name(),
            linkage: a.linkage,
            visibility: a.visibility,
            decl: false,
            function: false,
            tls: a.thread_local.is_some(),
            comdat: false,
        };
        let d = assume_dso_local(&facts, reloc, si, da);
        let a = m.alias_mut(id);
        if a.linkage.is_local() {
            a.visibility = Visibility::Default;
        }
        if let Some(d) = d {
            a.dso_local = d;
        }
    }
}
