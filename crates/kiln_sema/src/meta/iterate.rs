//! Range arguments read by iterating them (spec/reflection.md §3.5): the
//! Oven reads a range argument's elements from its layout only for the
//! standard contiguous classes kiln knows ([`Sema::range_by_layout`]);
//! any other range (`views::transform`, `views::filter`, an input view,
//! a user's container) is a [`Shape::Iter`] argument. Its call is an
//! ordinary call of the intrinsic, whose body ([`Sema::meta_iter_thunk`])
//! copies the elements into a `std::vector` in one pass over the range
//! (`ranges::begin`, `ranges::end`, `==`, `*`, `++`, as a range-based
//! `for` does) and calls the intrinsic with it.

use kiln_ast::decl::{DeclFlags, DeclKind, VarSpec};
use kiln_ast::names::DeclName;
use kiln_ast::types::{ArgView, AutoKeyword, BuiltinType, TemplateArg};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType, StmtId};
use kiln_oven::{MetaOp, Shape};
use kiln_source::Loc;

use crate::Sema;
use crate::lookup::LookupKind;
use crate::range_for::RangeInit;
use crate::scope::{Scope, ScopeKind};
use crate::templ::actions::TemplateIdRef;

impl Sema<'_, '_> {
    /// Whether a range argument of type `q` is read by its layout
    /// (§3.5): a non-class, or one of the standard contiguous classes
    /// `array`, `basic_string_view`, `basic_string`, `initializer_list`,
    /// `vector` (not `vector<bool>`) and `span` of dynamic extent (or the
    /// one-parameter stand-in of `tests/support/kiln_meta.h`), declared
    /// in `std` (inline namespaces skipped). Another class is iterated.
    pub(crate) fn range_by_layout(&self, q: QualType) -> bool {
        let t = &self.ast.types;
        let c = t.canonical(q).without_quals();
        let Some(d) = t.as_record(c) else { return true };
        if !self.is_in_std_namespace(d) {
            return false;
        }
        let args: Vec<ArgView> = self
            .ast
            .spec_info(d)
            .map(|(_, i)| t.args(i.args).iter().map(TemplateArg::view).collect())
            .unwrap_or_default();
        match self.decl_name(d).as_str() {
            "array" | "basic_string_view" | "basic_string" | "initializer_list" => true,
            "vector" => !matches!(args.first(),
                Some(ArgView::Type(e)) if t.canonical(*e).without_quals() == QualType::BOOL),
            "span" => match args.get(1) {
                None => true,
                // `dynamic_extent`, `size_t(-1)`.
                Some(ArgView::Value { value, .. }) => self
                    .const_int(*value)
                    .is_some_and(|v| v == i128::from(u64::MAX) || v == i128::from(u32::MAX)),
                _ => false,
            },
            _ => false,
        }
    }

    /// The body of intrinsic `f` (a specialization of a function
    /// template) when a range parameter is read by iterating it: `{
    /// std::vector<V> __v; for (auto&& __x : p) __v.emplace_back(__x);
    /// return f(..., __v, ...); }` per such parameter `p`, the call
    /// resolving to the specialization for `std::vector<V>&` (§3.5). `V`
    /// is `info` for a `reflection_range`, else `ranges::range_value_t`
    /// of the range. None when `f` has no such parameter; `Some(Err(()))`
    /// when the body cannot be built (no `std::vector`, `vector<bool>`).
    pub(crate) fn meta_iter_thunk(&mut self, f: DeclId, op: MetaOp) -> Option<Result<StmtId, ()>> {
        let params = self.param_decls(f);
        let mut iter = Vec::new();
        for (i, &p) in params.iter().enumerate() {
            let nr = self.non_reference(self.ast.decls.get(p).ty()?);
            if let Ok(Some(sh @ (Shape::Range | Shape::Elems))) =
                self.meta_op_shape(op, Some(i), nr)
                && !self.range_by_layout(nr)
            {
                iter.push((i, sh));
            }
        }
        if iter.is_empty() {
            return None;
        }
        let Some((_, info)) = self.ast.spec_info(f) else { return Some(Err(())) };
        let ft = info.template;
        let n = *self.ast.decls.get(f);
        let (Some(ctx), Some(fty)) = (n.ctx(), n.ty()) else { return Some(Err(())) };
        let Some(ret) = self.ast.types.proto(fty).map(kiln_ast::types::FuncProto::ret) else {
            return Some(Err(()));
        };
        let loc = n.loc();
        let (body, stash) = self.in_inst_context(ctx, |s| {
            s.capturing(|s| {
                s.scopes.push(Scope::new(ScopeKind::Function, None));
                s.ctx = f;
                s.begin_function_body(f, ret);
                s.meta_iter_body(ft, &params, &iter, loc)
            })
        });
        if self.stash_has_error(&stash) {
            return Some(Err(()));
        }
        Some(body.ok_or(()))
    }

    fn meta_iter_body(
        &mut self,
        ft: DeclId,
        params: &[DeclId],
        iter: &[(usize, Shape)],
        loc: Loc,
    ) -> Option<StmtId> {
        let mut stmts = Vec::new();
        let mut args: Vec<ExprId> = params.iter().map(|&p| self.build_decl_ref(p, loc)).collect();
        for (k, &(i, sh)) in iter.iter().enumerate() {
            let p = params[i];
            let elem = if sh == Shape::Range {
                BuiltinType::MetaInfo.qual_type()
            } else {
                let r = self.ast.decls.get(p).ty()?;
                self.std_ranges_alias("range_value_t", r, loc)?
            };
            // A `bool` or array element is collected packed: its scalars
            // (a `bool` as an `unsigned char`) in a `vector`, which the
            // intrinsic reads as `V`s ([`Sema::meta_packed_elems`]):
            // `vector<bool>` is no contiguous range and an array is no
            // element a `vector` holds.
            let ec = self.ast.types.canonical(elem);
            let scalar = self.base_element(ec);
            let packed = scalar.without_quals() == QualType::BOOL || self.ast.types.is_array(ec);
            let store = if !packed {
                elem
            } else if scalar.without_quals() == QualType::BOOL {
                QualType::UCHAR
            } else {
                scalar.without_quals()
            };
            let (_, vt) = self.std_class_template("vector")?;
            let name = self.template_name_for(vt);
            let Ok(TemplateIdRef::Type(vty)) =
                self.act_template_id(name, &[TemplateArg::ty(store)], loc, loc)
            else {
                return None;
            };
            // std::vector<V> __vK;
            let v = self.implicit_local(&format!("__kiln_elems{k}"), vty, loc);
            self.act_on_uninit(v);
            stmts.push(self.builder().decl_stmt(&[v], loc));
            if packed {
                self.meta_packed_elems.insert(v, ec.without_quals());
            }
            // for (auto&& __xK : p) __vK.emplace_back(__xK); an array
            // element is walked down to its scalars by nested loops.
            let mut heads = Vec::new();
            let mut range = self.build_decl_ref(p, loc);
            let mut t = ec;
            let mut x;
            loop {
                let auto = self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false);
                let rref = self.ast.types.rvalue_ref_to(auto);
                x = self.implicit_local(&format!("__kiln_elem{k}_{}", heads.len()), rref, loc);
                self.ast.decls.mark_var(x, VarSpec::new().with_cxx_for_range(true));
                heads.push(self.act_on_for_range(
                    None,
                    x,
                    RangeInit::Expr(range),
                    Vec::new(),
                    loc,
                    loc,
                )?);
                if !packed || !self.ast.types.is_array(t) {
                    break;
                }
                t = self.ast.types.element(t)?;
                range = self.build_decl_ref(x, loc);
            }
            let vref = self.build_decl_ref(v, loc);
            let emplace = self.intern("emplace_back");
            let m = self.act_on_member(vref, emplace, loc, loc, false);
            let xref = self.build_decl_ref(x, loc);
            let push = self.act_on_call(m, &[xref], loc, loc);
            let push = self.act_on_full_expr(push);
            let mut body = self.builder().expr_stmt(push);
            while let Some(head) = heads.pop() {
                body = self.act_on_finish_for_range(head, body);
            }
            stmts.push(body);
            args[i] = self.build_decl_ref(v, loc);
        }
        let name = self.ast.decls.get(ft).name();
        let callee = self.build_overload_ref(name, &[ft], &[ft], None, false, None, loc);
        let call = self.act_on_call(callee, &args, loc, loc);
        let call = self.act_on_full_expr(call);
        stmts.push(self.builder().return_stmt(Some(call), None, loc));
        Some(self.builder().compound(&stmts, loc, loc))
    }

    /// An implicit local variable of the function being built.
    fn implicit_local(&mut self, name: &str, ty: QualType, loc: Loc) -> DeclId {
        let sym = self.intern(name);
        let mut h = DeclHead::new(self.ctx, DeclName::ident(sym), loc);
        h.flags = h.flags.with(DeclFlags::IMPLICIT);
        self.builder().var(h, ty, VarSpec::new(), None)
    }

    /// `std::ranges::NAME<r>`, an alias template of one type parameter.
    fn std_ranges_alias(&mut self, name: &str, r: QualType, loc: Loc) -> Option<QualType> {
        let ns = |s: &Self, ctx: DeclId, n: &str| {
            let sym = s.intern(n);
            let found = s.lookup_qualified(ctx, sym, LookupKind::Nested);
            found.decls.iter().copied().find(|&d| s.kind_of(d) == DeclKind::Namespace)
        };
        let std = ns(self, self.ast.tu, "std")?;
        let ranges = ns(self, std, "ranges")?;
        let sym = self.intern(name);
        let found = self.lookup_qualified(ranges, sym, LookupKind::Ordinary);
        let t = found
            .decls
            .iter()
            .copied()
            .find(|&d| self.kind_of(d) == DeclKind::TypeAliasTemplate)?;
        let tn = self.template_name_for(t);
        match self.act_template_id(tn, &[TemplateArg::ty(r)], loc, loc) {
            Ok(TemplateIdRef::Type(t)) => Some(t),
            _ => None,
        }
    }
}
