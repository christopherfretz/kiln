//! Function multiversioning queries (WP-1745; clang
//! `FunctionDecl::getMultiVersionKind`, `isMultiVersion`,
//! `ASTContext::forEachMultiversionedFunctionVersion`).
//!
//! On x86-64 the versions of a multiversioned function are separate
//! entities of one name, scope and type: sema keeps a declaration whose
//! multiversioning attribute differs from the earlier ones' out of their
//! redeclaration chain. Whether an entity is multiversioned follows from
//! its attributes and those of the entities beside it, so nothing is
//! stored for it. spec/sema.md "Function multiversioning".

use crate::ast::Ast;
use crate::attr::{AttrKind, AttrNode};
use crate::decl::DeclFlags;
use crate::ids::{AttrId, DeclId, RawId, StringLitId};

/// The multiversioning kind of a function entity (clang
/// `MultiVersionKind`, without `target_version`, which x86-64 does not
/// multiversion).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MvKind {
    /// No multiversioning attribute.
    None,
    /// `target("...")`.
    Target,
    /// `target_clones(...)`.
    Clones,
    /// `cpu_specific(...)`.
    CpuSpecific,
    /// `cpu_dispatch(...)`.
    CpuDispatch,
}

impl Ast {
    /// The latest attribute of kind `kind` on a declaration of `d`'s
    /// entity.
    pub fn mv_attr(&self, d: DeclId, kind: AttrKind) -> Option<AttrNode> {
        self.redecls_of(d).find_map(|r| {
            self.lists
                .iter::<AttrId>(self.attrs.of_decl(r))
                .map(|a| *self.attrs.get(a))
                .find(|n| n.kind == kind)
        })
    }

    /// The text of the string literal of a `target` attribute node.
    pub fn target_attr_text(&self, n: &AttrNode) -> String {
        StringLitId::from_raw(n.a).map_or_else(String::new, |l| {
            String::from_utf8_lossy(self.strings.bytes(l)).into_owned()
        })
    }

    /// The list a `target_clones` / `cpu_specific` / `cpu_dispatch` node
    /// keeps (its `Symbol`, comma-joined): clone strings or CPU names.
    pub fn mv_attr_list(&self, n: &AttrNode) -> Vec<String> {
        self.symbols()
            .as_str(kiln_source::symbol::Symbol::from_index(n.a))
            .split(',')
            .map(str::to_owned)
            .collect()
    }

    /// The multiversioning kind of `d`'s entity, from its attributes.
    pub fn mv_kind(&self, d: DeclId) -> MvKind {
        if !self.decls.get(d).kind().is_function() {
            return MvKind::None;
        }
        let has = |k| self.attrs.has_kind(k) && self.mv_attr(d, k).is_some();
        if has(AttrKind::CPU_DISPATCH) {
            MvKind::CpuDispatch
        } else if has(AttrKind::CPU_SPECIFIC) {
            MvKind::CpuSpecific
        } else if has(AttrKind::TARGET_CLONES) {
            MvKind::Clones
        } else if has(AttrKind::TARGET) {
            MvKind::Target
        } else {
            MvKind::None
        }
    }

    /// Whether `d`'s entity has a `target("default")` attribute.
    pub fn is_target_default(&self, d: DeclId) -> bool {
        self.mv_attr(d, AttrKind::TARGET).is_some_and(|n| self.target_attr_text(&n) == "default")
    }

    /// The other valid functions of `d`'s name, scope and type (their
    /// latest declarations), in order of first declaration, with `d`'s
    /// own entity among them.
    pub fn same_signature_fns(&self, d: DeclId) -> Vec<DeclId> {
        let node = self.decls.get(d);
        let (Some(ctx), Some(ty)) = (node.ctx(), node.ty()) else { return vec![self.latest(d)] };
        let first = self.first(d);
        let mut out: Vec<DeclId> = self
            .lookup_local(ctx, node.name())
            .iter()
            .filter(|&x| {
                let n = self.decls.get(x);
                n.kind().is_function()
                    && n.ty() == Some(ty)
                    && (self.first(x) == first || !n.has(DeclFlags::INVALID))
            })
            .collect();
        if !out.iter().any(|&x| self.first(x) == first) {
            out.push(self.latest(d));
        }
        out
    }

    /// The versions of `d`'s multiversioned function (clang
    /// `forEachMultiversionedFunctionVersion`): the entities of
    /// [`Ast::same_signature_fns`] with a multiversioning attribute.
    pub fn mv_versions(&self, d: DeclId) -> Vec<DeclId> {
        let mut v = self.same_signature_fns(d);
        v.retain(|&x| self.mv_kind(x) != MvKind::None);
        v
    }

    /// Whether `d`'s entity is a multiversioned function on x86-64
    /// (clang `FunctionDecl::isMultiVersion`): it has a `target_clones`,
    /// `cpu_specific` or `cpu_dispatch` attribute, a `target("default")`,
    /// or a `target` beside another version.
    pub fn is_multiversion(&self, d: DeclId) -> bool {
        match self.mv_kind(d) {
            MvKind::None => false,
            MvKind::Target => self.is_target_default(d) || self.mv_versions(d).len() > 1,
            _ => true,
        }
    }
}
