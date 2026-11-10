//! Template type diffing for diagnostic argument pairs (spec/diag.md
//! §6.1; clang `TemplateDiff` in `ASTDiagnostic.cpp` with its default
//! `-felide-type` and no `-fdiagnostics-show-template-tree`).
//!
//! A clang diagnostic that formats two types with `%diff{$ ... $|...}`
//! prints them as a pair: when both are specializations of the same
//! template, the template arguments that are the same in both print as
//! `[...]` (a run of N as `[N * ...]`, all of them as `...`), and the
//! differing ones in full, recursively, with no `aka`. Anything else
//! prints as an ordinary type argument.

use kiln_ast::types::{
    ArgView, PrintOptions, Quals, TemplateArg, TypeKind, TypeView, print_type, write_template_arg,
};
use kiln_ast::{DeclId, QualType};
use std::fmt::Write as _;

use crate::Sema;
use crate::names::sema_names;

/// A template specialization as the diff sees it (clang's
/// `TemplateSpecializationType` with its sugared and desugared argument
/// iterators).
struct Spec {
    /// The (first declaration of the) template.
    template: DeclId,
    /// The arguments as written, packs expanded.
    written: Vec<TemplateArg>,
    /// The canonical (converted) arguments of the class template
    /// specialization, packs expanded; supplies defaulted arguments.
    canon: Option<Vec<TemplateArg>>,
    /// The aliased type of an alias template specialization.
    aliased: Option<QualType>,
}

/// One node of the diff tree.
enum Node {
    /// A template with per-argument children.
    Template { from: DeclId, from_quals: Quals, to_quals: Quals, children: Vec<Child> },
    /// Two type arguments.
    Type { from: Option<QualType>, to: Option<QualType> },
    /// Two non-type arguments (the written and canonical forms).
    Value { from: Option<(TemplateArg, TemplateArg)>, to: Option<(TemplateArg, TemplateArg)> },
    /// Two template template arguments.
    TemplateTemplate { from: Option<DeclId>, to: Option<DeclId> },
}

struct Child {
    node: Node,
    same: bool,
    from_default: bool,
}

fn flatten(ctx: &kiln_ast::types::TypeContext, args: &[TemplateArg], out: &mut Vec<TemplateArg>) {
    for &a in args {
        match a.view() {
            ArgView::Pack(l) => flatten(ctx, ctx.args(l), out),
            _ => out.push(a),
        }
    }
}

impl Sema<'_, '_> {
    /// The pair `(from, to)` as clang prints a `%diff` argument pair:
    /// quoted, abbreviated by the template diff when both are
    /// specializations of the same template, otherwise each as
    /// [`Sema::ty_arg`] (with its `aka`).
    pub fn ty_diff(&self, from: QualType, to: QualType) -> (String, String) {
        match (self.template_diff(from, to, true), self.template_diff(from, to, false)) {
            (Some(a), Some(b)) => (format!("'{a}'"), format!("'{b}'")),
            _ => (self.ty_arg(from), self.ty_arg(to)),
        }
    }

    /// clang's `FormatTemplateTypeDiff` (no tree): the `from` side when
    /// `print_from`, else the `to` side, or `None` when the types are not
    /// specializations of one template.
    fn template_diff(&self, from: QualType, to: QualType, print_from: bool) -> Option<String> {
        let (a, b) = if print_from { (from, to) } else { (to, from) };
        let (sa, sb) = self.common_specs(a, b)?;
        let ctx = &self.ast.types;
        let node = Node::Template {
            from: sa.template,
            from_quals: ctx.effective_quals(a),
            to_quals: ctx.effective_quals(b),
            children: self.diff_args(&sa, &sb),
        };
        let mut out = String::new();
        self.print_node(&node, false, &mut out);
        Some(out)
    }

    /// clang's `GetTemplateSpecializationType`: the first template
    /// specialization in `q`'s sugar, else the class template
    /// specialization `q` names.
    fn diff_spec(&self, q: QualType) -> Option<Spec> {
        let ctx = &self.ast.types;
        if ctx.is_dependent(q) {
            return None;
        }
        let mut cur = q;
        for _ in 0..64 {
            match ctx.view(cur.ty()) {
                TypeView::TemplateSpecialization { name, args, underlying, alias } => {
                    let template = self.ast.first(ctx.template_decl(name)?);
                    let mut written = Vec::new();
                    flatten(ctx, ctx.args(args), &mut written);
                    let canon = if alias {
                        None
                    } else {
                        underlying.and_then(|u| self.record_spec(u)).map(|s| s.written)
                    };
                    let aliased = if alias { underlying } else { None };
                    return Some(Spec { template, written, canon, aliased });
                }
                TypeView::SubstTemplateTypeParm { replacement, .. } => cur = replacement,
                _ => {
                    let next = ctx.desugar_once(cur);
                    if next == cur {
                        break;
                    }
                    cur = next;
                }
            }
        }
        self.record_spec(q)
    }

    /// The canonical specialization record `q` names, as a [`Spec`]
    /// whose written arguments are the canonical ones.
    fn record_spec(&self, q: QualType) -> Option<Spec> {
        let ctx = &self.ast.types;
        let rec = ctx.as_record(q)?;
        let (_, info) = self.ast.spec_info(rec)?;
        if self.ast.decls.get(rec).kind() != kiln_ast::decl::DeclKind::CXXRecord {
            return None;
        }
        let mut written = Vec::new();
        flatten(ctx, ctx.args(ctx.canonical_args(info.args)), &mut written);
        Some(Spec {
            template: self.ast.first(info.template),
            canon: Some(written.clone()),
            written,
            aliased: None,
        })
    }

    /// clang's `hasSameTemplate`: both specializations, of the same
    /// template, or of the highest alias templates their alias chains
    /// share from the bottom.
    fn common_specs(&self, a: QualType, b: QualType) -> Option<(Spec, Spec)> {
        let chain = |q: QualType| {
            let mut v = Vec::new();
            let mut s = self.diff_spec(q);
            while let Some(spec) = s {
                let next = spec.aliased.and_then(|t| self.diff_spec(t));
                v.push(spec);
                if v.len() > 64 {
                    break;
                }
                s = next;
            }
            v
        };
        let (mut ca, mut cb) = (chain(a), chain(b));
        let (fa, fb) = (ca.first()?, cb.first()?);
        if fa.template == fb.template {
            return Some((ca.swap_remove(0), cb.swap_remove(0)));
        }
        if ca.last()?.template != cb.last()?.template {
            return None;
        }
        // Climb from the bottom while the templates agree.
        let mut k = 1;
        while k < ca.len() && k < cb.len() {
            if ca[ca.len() - 1 - k].template != cb[cb.len() - 1 - k].template {
                break;
            }
            k += 1;
        }
        let (ia, ib) = (ca.len() - k, cb.len() - k);
        Some((ca.swap_remove(ia), cb.swap_remove(ib)))
    }

    /// clang's `DiffTemplate(FromTST, ToTST)`: one child per argument
    /// position written on either side.
    fn diff_args(&self, a: &Spec, b: &Spec) -> Vec<Child> {
        let ctx = &self.ast.types;
        let n = a.written.len().max(b.written.len());
        let side = |s: &Spec, i: usize| -> Option<(TemplateArg, bool, Option<TemplateArg>)> {
            let c = s.canon.as_ref().and_then(|c| c.get(i).copied());
            match s.written.get(i) {
                Some(&w) => Some((w, false, c)),
                None => c.map(|c| (c, true, Some(c))),
            }
        };
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let (fa, fb) = (side(a, i), side(b, i));
            let from_default = fa.is_some_and(|x| x.1);
            let kind = fa.or(fb).map(|x| x.0.view());
            let (node, same) = match kind {
                Some(ArgView::Type(_)) => {
                    let ty = |x: Option<(TemplateArg, bool, Option<TemplateArg>)>| match x?.0.view()
                    {
                        ArgView::Type(q) => Some(q),
                        _ => None,
                    };
                    let (ta, tb) = (ty(fa), ty(fb));
                    match (ta, tb) {
                        (Some(x), Some(y)) if ctx.same_type(x, y) => {
                            (Node::Type { from: ta, to: tb }, true)
                        }
                        (Some(x), Some(y)) => match self.common_specs(x, y) {
                            Some((sx, sy)) => (
                                Node::Template {
                                    from: sx.template,
                                    from_quals: ctx.effective_quals(x),
                                    to_quals: ctx.effective_quals(y),
                                    children: self.diff_args(&sx, &sy),
                                },
                                false,
                            ),
                            None => (Node::Type { from: ta, to: tb }, false),
                        },
                        _ => (Node::Type { from: ta, to: tb }, false),
                    }
                }
                Some(ArgView::Template(_) | ArgView::TemplateExpansion { .. }) => {
                    let td = |x: Option<(TemplateArg, bool, Option<TemplateArg>)>| match x?.0.view()
                    {
                        ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. } => {
                            ctx.template_decl(n).map(|d| self.ast.first(d))
                        }
                        _ => None,
                    };
                    let (da, db) = (td(fa), td(fb));
                    let same = da.is_some() && da == db;
                    (Node::TemplateTemplate { from: da, to: db }, same)
                }
                _ => {
                    let key = |x: (TemplateArg, bool, Option<TemplateArg>)| {
                        ctx.canonical_arg(x.2.unwrap_or(x.0))
                    };
                    let same = matches!((fa, fb), (Some(x), Some(y)) if key(x) == key(y));
                    let val = |x: Option<(TemplateArg, bool, Option<TemplateArg>)>| {
                        x.map(|x| (x.0, x.2.unwrap_or(x.0)))
                    };
                    (Node::Value { from: val(fa), to: val(fb) }, same)
                }
            };
            out.push(Child { node, same, from_default });
        }
        out
    }

    /// clang's `TreeToString` for the printed (`from`) side.
    fn print_node(&self, node: &Node, from_default: bool, out: &mut String) {
        let names = sema_names(self);
        let ctx = &self.ast.types;
        match node {
            Node::Template { from, from_quals, to_quals, children } => {
                if from_default {
                    out.push_str("(default) ");
                }
                print_quals(*from_quals, *to_quals, out);
                out.push_str(&self.decl_name(*from));
                if children.is_empty() {
                    out.push_str("<>");
                    return;
                }
                out.push('<');
                let mut elided = 0usize;
                let mut all_elided = true;
                for (i, c) in children.iter().enumerate() {
                    if c.same {
                        elided += 1;
                        continue;
                    }
                    all_elided = false;
                    if elided > 0 {
                        print_elided(elided, out);
                        elided = 0;
                        out.push_str(", ");
                    }
                    self.print_node(&c.node, c.from_default, out);
                    if i + 1 < children.len() {
                        out.push_str(", ");
                    }
                }
                if elided > 0 {
                    if all_elided {
                        out.push_str("...");
                    } else {
                        print_elided(elided, out);
                    }
                }
                out.push('>');
            }
            Node::Type { from, to } => self.print_type_leaf(*from, *to, from_default, out),
            Node::Value { from, to } => {
                let Some((written, canon)) = *from else {
                    out.push_str("(no argument)");
                    return;
                };
                if from_default {
                    out.push_str("(default) ");
                }
                let int_ty = |a: TemplateArg| match a.view() {
                    ArgView::Value { ty, .. }
                        if ctx.is_integral(ty) || ctx.canonical_kind(ty) == TypeKind::Enum =>
                    {
                        Some(ty)
                    }
                    _ => None,
                };
                if let (Some(fa), Some(ta)) = (int_ty(canon), to.and_then(|t| int_ty(t.1)))
                    && !ctx.same_type(fa, ta)
                {
                    out.push('(');
                    out.push_str(&print_type(ctx, &names, fa, PrintOptions::SUGARED));
                    out.push_str(") ");
                }
                let shown =
                    if matches!(written.view(), ArgView::Expr { .. }) { canon } else { written };
                write_template_arg(ctx, &names, shown, out);
            }
            Node::TemplateTemplate { from, to } => {
                out.push_str(if from_default { "(default) template " } else { "template " });
                let Some(f) = *from else {
                    out.push_str("(no argument)");
                    return;
                };
                let qualified =
                    to.is_some_and(|t| t != f && self.decl_name(t) == self.decl_name(f));
                if qualified {
                    out.push_str(&self.qualified_decl_name(f));
                } else {
                    out.push_str(&self.decl_name(f));
                }
            }
        }
    }

    /// A type leaf of [`Self::print_node`] (clang's `PrintTypeNames`).
    fn print_type_leaf(
        &self,
        from: Option<QualType>,
        to: Option<QualType>,
        from_default: bool,
        out: &mut String,
    ) {
        let names = sema_names(self);
        let ctx = &self.ast.types;
        let Some(f) = from else {
            out.push_str("(no argument)");
            return;
        };
        if from_default {
            out.push_str("(default) ");
        }
        if let Some(t) = to
            && f != t
            && f.without_quals() == t.without_quals()
        {
            // clang's `PrintTypeNames`: the same sugared type with
            // different local qualifiers prints them first.
            let fq = Quals::from_bits(f.quals_bits());
            let tq = Quals::from_bits(t.quals_bits());
            print_quals(fq, tq, out);
            out.push_str(&print_type(ctx, &names, f.without_quals(), PrintOptions::SUGARED));
            return;
        }
        let mut s = print_type(ctx, &names, f, PrintOptions::SUGARED);
        if let Some(t) = to
            && s == print_type(ctx, &names, t, PrintOptions::SUGARED)
        {
            // Elaborated sugar dropped, then the canonical type,
            // when that tells the two apart.
            let unelaborated = |q: QualType| match ctx.view(q.ty()) {
                TypeView::Elaborated { named, .. } => print_type(
                    ctx,
                    &names,
                    QualType::new(named.ty(), named.quals_bits() | q.quals_bits()),
                    PrintOptions::SUGARED,
                ),
                _ => print_type(ctx, &names, q, PrintOptions::SUGARED),
            };
            let (nf, nt) = (unelaborated(f), unelaborated(t));
            if nf == nt {
                let cf = print_type(ctx, &names, f, PrintOptions::CANONICAL);
                if cf != print_type(ctx, &names, t, PrintOptions::CANONICAL) {
                    s = cf;
                }
            } else {
                s = nf;
            }
        }
        out.push_str(&s);
    }
}

/// clang's `PrintElideArgs` (inline form).
fn print_elided(n: usize, out: &mut String) {
    if n == 1 {
        out.push_str("[...]");
    } else {
        let _ = write!(out, "[{n} * ...]");
    }
}

/// clang's `PrintQualifiers` (inline form): the common qualifiers, then
/// those only on the printed side, each followed by a space.
fn print_quals(from: Quals, to: Quals, out: &mut String) {
    if from.is_empty() && to.is_empty() {
        return;
    }
    let (common, only) = if from == to { (from, Quals::NONE) } else { (from & to, from - to) };
    for q in [common, only] {
        if !q.is_empty() {
            q.write(out);
            out.push(' ');
        }
    }
}

/// Formats a `%diff` type pair into a diagnostic (spec/diag.md §6.1).
pub(crate) trait DiffArgs {
    /// Appends `from` and `to` as two arguments, as [`Sema::ty_diff`].
    fn diff_args(self, s: &Sema<'_, '_>, from: QualType, to: QualType) -> Self;
}

impl DiffArgs for kiln_diag::Diag {
    fn diff_args(self, s: &Sema<'_, '_>, from: QualType, to: QualType) -> Self {
        let (a, b) = s.ty_diff(from, to);
        self.arg(a).arg(b)
    }
}
