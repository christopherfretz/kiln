//! The sema half of the `std::meta` intrinsics (spec/reflection.md §3):
//! recognizing a declaration as one ([`Sema::meta_intrinsic_of`]) and
//! running one ([`Sema::meta`], reached through
//! [`kiln_oven::SemaBridge::meta`]). One file per op family; an op no WP
//! has implemented yet fails with a note naming its owner.

mod ext;
pub(crate) mod names;
mod query;
mod recipe;

use kiln_ast::decl::DeclKind;
use kiln_ast::types::BuiltinType;
use kiln_ast::{DeclId, ReflectionId};
use kiln_oven::{MetaCtx, MetaError, MetaOp, MetaOut, Oven};
use kiln_source::Loc;

use crate::Sema;

/// The reason note of a failed reflection intrinsic.
fn fail<T>(loc: Loc, msg: impl Into<String>) -> Result<T, MetaError> {
    Err(MetaError::note(loc, msg))
}

/// Whether `s` is an identifier ([lex.name]/1, basic source characters).
fn is_identifier(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next().is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && cs.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// How many inline namespaces a context walk skips at most (a bound on a
/// malformed chain).
const MAX_INLINE: usize = 64;

impl Sema<'_, '_> {
    /// Runs intrinsic `op` on `args` (the call's argument values, as the
    /// op's signature lays them out) into `out` (spec/reflection.md §3).
    ///
    /// # Errors
    /// The reasons the call is not a constant expression (none: a silent
    /// failure), or the Oven's fault reading an argument.
    pub(crate) fn meta(
        &mut self,
        op: MetaOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        match op {
            MetaOp::Query(q) if names::is_names_op(q) => self.meta_names(q, args, out, ctx, oven),
            MetaOp::Query(q) => self.meta_query(q, args, out, ctx),
            MetaOp::Ext(x) => self.meta_ext(x, args, out, ctx, oven),
            _ => Err(MetaError::not_implemented(op, ctx.loc)),
        }
    }

    /// The reflection intrinsic `f` is (spec/reflection.md §3.1): a
    /// function declared in `std::meta` (inline namespaces at any level
    /// skipped), or a member of a class declared there, whose name and
    /// parameter list (`info` or not, per parameter) are an op's; or a
    /// kiln extension declared at namespace scope. Only under
    /// `-freflection`.
    pub(crate) fn meta_intrinsic_of(&self, f: DeclId) -> Option<MetaOp> {
        if !self.reflection {
            return None;
        }
        let n = self.ast.decls.get(f);
        if !matches!(n.kind(), DeclKind::Function | DeclKind::CXXMethod) {
            return None;
        }
        let ctx = self.skip_inline(n.ctx()?)?;
        let (name, in_std_meta) = if self.kind_of(ctx) == DeclKind::TranslationUnit {
            (self.decl_name(f), false)
        } else if self.kind_of(ctx) == DeclKind::Namespace && self.decl_name(ctx) == "meta" {
            (self.decl_name(f), true)
        } else if self.kind_of(ctx) == DeclKind::CXXRecord
            && self.ast.decls.get(ctx).ctx().and_then(|m| self.skip_inline(m)).is_some_and(|m| {
                self.kind_of(m) == DeclKind::Namespace && self.decl_name(m) == "meta"
            })
        {
            (format!("{}::{}", self.decl_name(ctx), self.decl_name(f)), true)
        } else {
            return None;
        };
        if in_std_meta && !self.in_std_meta_ns(ctx) {
            return None;
        }
        let proto = self.ast.types.proto(n.ty()?)?;
        let info = BuiltinType::MetaInfo.qual_type();
        let mut params = [false; kiln_oven::MAX_META_ARGS];
        let ps = self.ast.types.params(proto);
        if ps.len() > params.len() {
            return None;
        }
        for (slot, &p) in params.iter_mut().zip(ps) {
            *slot = self.ast.types.canonical(p).without_quals() == info;
        }
        MetaOp::lookup(&name, &params[..ps.len()], in_std_meta)
    }

    /// `d` with the inline namespaces enclosing it skipped (`d` itself
    /// included): the first context that is not one.
    fn skip_inline(&self, mut d: DeclId) -> Option<DeclId> {
        for _ in 0..MAX_INLINE {
            match self.ast.decls.namespace(d) {
                Some(ns) if ns.is_inline() => d = self.ast.decls.get(d).ctx()?,
                _ => return Some(d),
            }
        }
        None
    }

    /// Whether `d` (a namespace `meta`, or a class in one) is in `std`,
    /// which is in the translation unit (inline namespaces skipped).
    fn in_std_meta_ns(&self, d: DeclId) -> bool {
        let meta = if self.kind_of(d) == DeclKind::Namespace {
            Some(d)
        } else {
            self.ast.decls.get(d).ctx().and_then(|m| self.skip_inline(m))
        };
        let Some(std) =
            meta.and_then(|m| self.ast.decls.get(m).ctx()).and_then(|s| self.skip_inline(s))
        else {
            return false;
        };
        self.kind_of(std) == DeclKind::Namespace
            && self.decl_name(std) == "std"
            && self.ast.decls.get(std).ctx().is_some_and(|t| {
                self.kind_of(self.ast.redecl_context(t)) == DeclKind::TranslationUnit
            })
    }

    /// `r` printed for a note.
    fn reflection_str(&self, r: ReflectionId) -> String {
        use kiln_ast::consts::PrintEnv as _;
        let mut s = String::new();
        self.ast.reflection(r, &mut s);
        s
    }
}
