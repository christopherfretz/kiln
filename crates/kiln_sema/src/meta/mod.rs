//! The sema half of the `std::meta` intrinsics (spec/reflection.md §3):
//! recognizing a declaration as one ([`Sema::meta_intrinsic_of`]) and
//! running one ([`Sema::meta`], reached through
//! [`kiln_oven::SemaBridge::meta`]). One file per op family; an op no WP
//! has implemented yet fails with a note naming its owner.

mod access;
mod annot;
mod define;
mod ext;
mod gcc_names;
pub(crate) mod member;
pub(crate) mod names;
mod query;
mod recipe;
mod templ;
mod traits;
mod value;

pub(crate) use templ::SubstMemo;

use kiln_ast::decl::DeclKind;
use kiln_ast::types::BuiltinType;
use kiln_ast::{DeclId, ReflectionId};
use kiln_oven::{AnnotOp, DefineOp, MetaCtx, MetaError, MetaOp, MetaOut, Oven};
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
            MetaOp::Query(q) => self.meta_query(q, args, out, ctx, oven),
            // [meta.reflection.queries] lists these two with the
            // predicates; only the reflectee kind decides them.
            MetaOp::Annot(AnnotOp::IsAnnotation) | MetaOp::Define(DefineOp::IsDataMemberSpec) => {
                self.meta_kind_pred(op, args, out)
            }
            MetaOp::Define(d) => self.meta_define(d, args, out, ctx, oven),
            MetaOp::Ext(x) => self.meta_ext(x, args, out, ctx, oven),
            MetaOp::Template(t) => self.meta_templ(t, args, out, ctx, oven),
            MetaOp::Trait(t) => self.meta_trait(t, args, out, ctx, oven),
            MetaOp::Member(m) => self.meta_member(m, args, out, ctx, oven),
            MetaOp::Access(a) => self.meta_access(a, args, out, ctx, oven),
            MetaOp::Annot(a) => self.meta_annot(a, args, out, ctx, oven),
            MetaOp::Value(v) => self.meta_value(v, args, out, ctx, oven),
        }
    }

    /// Reading the variable `r` designates (a variable, or the object of
    /// one: `object_of`) through a reflection, at `loc`: its definition is
    /// instantiated as an odr-use by name would instantiate it
    /// ([temp.inst]/5 and /8, [basic.def.odr]/4; spec/reflection.md §3
    /// "Reading a variable through a reflection", WP-638). `extract`,
    /// `constant_of` and `object_of` come through here, before they read
    /// the variable's value or take its address, so a static data member
    /// of a specialization `substitute` produced (or one no expression
    /// names) has its initializer. The use is potentially evaluated
    /// whatever context the evaluation runs in: the reflection runtime
    /// reads the value. Queries of the declaration (`type_of`,
    /// `is_const`, ...) do not come here, so an initializer that would be
    /// ill-formed is not instantiated by them.
    pub(super) fn use_reflected_var(&mut self, r: ReflectionId, loc: Loc, oven: &mut Oven) {
        use kiln_ast::consts::{PtrBase, Scalar};
        use kiln_ast::reflect::Reflectee;
        let d = match self.ast.reflections.get(r) {
            Reflectee::Decl(d) => d,
            Reflectee::Object(cv) => match self.ast.consts.scalar(cv) {
                Some(Scalar::Pointer { base: PtrBase::Decl(d), .. }) => d,
                _ => return,
            },
            _ => return,
        };
        // Only a template's specialization or member has a definition
        // left to instantiate.
        if self.kind_of(d) != DeclKind::Var
            || (self.ast.instantiated_from(d).is_none() && self.ast.spec_info(d).is_none())
        {
            return;
        }
        let d = self.ast.first(d);
        self.lend_oven(oven, |s| {
            let unevaluated = std::mem::take(&mut s.unevaluated);
            let discarded = s.func.as_mut().map(|f| std::mem::take(&mut f.discarded));
            s.note_var_referenced(d, loc);
            s.unevaluated = unevaluated;
            if let (Some(f), Some(n)) = (s.func.as_mut(), discarded) {
                f.discarded = n;
            }
        });
    }

    /// Forming the address of (or pointer to member) function `d` through
    /// a reflection, at `loc`: `extract` to a function pointer or a
    /// pointer to member function odr-uses `d` as `&C::f` would
    /// ([basic.def.odr]/4, [temp.inst]/5 and /8; spec/reflection.md §3
    /// "Using a function through a reflection", WP-1244), so a member of a
    /// class template specialization (or a function template
    /// specialization) has its definition instantiated, at once when it
    /// is constexpr (needed for constant evaluation). Potentially
    /// evaluated whatever context the evaluation runs in, as
    /// [`Self::use_reflected_var`]. Declaration queries (`type_of`,
    /// `members_of`, ...) do not come here.
    pub(super) fn use_reflected_function(&mut self, d: DeclId, loc: Loc, oven: &mut Oven) {
        let d = self.ast.first(d);
        self.lend_oven(oven, |s| {
            let unevaluated = std::mem::take(&mut s.unevaluated);
            let discarded = s.func.as_mut().map(|f| std::mem::take(&mut f.discarded));
            s.mark_used_at(d, loc);
            s.unevaluated = unevaluated;
            if let (Some(f), Some(n)) = (s.func.as_mut(), discarded) {
                f.discarded = n;
            }
        });
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
        // A function the library defines is a library function, whatever
        // its name (`tests/support/kiln_meta.h` defines
        // `reflect_constant_string` over an intrinsic; libstdc++ declares
        // it without a body; §3.1).
        if self.defined_body(self.function_pattern(f)).is_some() {
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
