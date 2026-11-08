//! GNU `cleanup(f)` on a local variable (spec/sema.md §4.5; clang
//! `handleCleanupAttr`, AttrDocs "cleanup"): `f(&v)` runs when `v` goes
//! out of scope, along every exit from its scope (codegen's
//! `Cleanup::CleanupFn`, before the variable's destructor).
//!
//! `act_on_attribute` keeps the argument as written and the argument
//! count; [`Sema::check_cleanup_attrs`] checks them in clang's order (the
//! subject, the count, the function the argument names, its parameter
//! count, then the parameter type against `T *` under C's assignment
//! rules) and replaces the node with one whose `a` is the function. The
//! constant evaluator ignores it, as clang's does.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::RawId;
use kiln_ast::types::BuiltinType;
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::decl::AttrArg;

impl Sema<'_, '_> {
    /// `cleanup` as written: `a` = the raw `ExprId` of the first argument
    /// (0: none; `u32::MAX`: not an expression), `b` = the argument count.
    pub(crate) fn cleanup_attribute(
        &mut self,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let a = match args.first() {
            Some(AttrArg::Expr(e)) => e.raw(),
            Some(_) => u32::MAX,
            None => 0,
        };
        let b = u32::try_from(args.len()).unwrap_or(u32::MAX);
        let kind = AttrKind::CLEANUP;
        Some(self.ast.attrs.push(AttrNode { kind, syntax, flags: 0, loc, a, b }))
    }

    /// Checks the `cleanup` attributes among `attrs` of declaration `d`,
    /// dropping the invalid ones and replacing each valid one by a node
    /// whose `a` is the raw `DeclId` of the cleanup function and `b` the raw
    /// location of its argument.
    pub(crate) fn check_cleanup_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if node.kind != AttrKind::CLEANUP {
                i += 1;
                continue;
            }
            match self.cleanup_function(d, node) {
                Some(f) => {
                    let b = self.cleanup_arg_loc(node).to_raw();
                    attrs[i] = self.ast.attrs.push(AttrNode { a: f.raw(), b, ..node });
                    i += 1;
                }
                None => {
                    attrs.remove(i);
                }
            }
        }
    }

    /// The location of the argument of `cleanup` attribute `node` as
    /// written (clang `getExprLoc`: a qualified name's is its qualifier's).
    fn cleanup_arg_loc(&self, node: AttrNode) -> Loc {
        ExprId::from_raw(node.a).map_or(node.loc, |e| self.begin_loc(e))
    }

    /// The function `cleanup` attribute `node` on `d` names, or `None`
    /// after a diagnostic.
    fn cleanup_function(&mut self, d: DeclId, node: AttrNode) -> Option<DeclId> {
        let shown = self.attr_shown(&node, "cleanup");
        let name = format!("'{shown}'");
        // `SubjectList<[LocalVar]>`: a variable with automatic storage
        // that is not a parameter; else ignored with a warning.
        if self.kind_of(d) != DeclKind::Var || !self.has_local_storage(d) {
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                .arg(name)
                .arg("local variables");
            self.emit(msg);
            return None;
        }
        if node.b != 1 {
            let msg =
                self.diag(DiagId::parse_attribute_wrong_arg_count, node.loc).arg(shown).arg("1");
            self.emit(msg);
            return None;
        }
        let e = ExprId::from_raw(node.a).filter(|_| node.a != u32::MAX)?;
        if self.is_err_expr(e) {
            return None;
        }
        let at = self.cleanup_arg_loc(node);
        let (f, fname) = self.cleanup_callee(e, at)?;
        // A variadic `f(...)` has no parameter.
        let params = self.param_decls(f);
        let [p] = params[..] else {
            let msg = self.diag(DiagId::sema_cleanup_func_must_take_one_arg, at).arg(fname);
            self.emit(msg);
            return None;
        };
        // clang 22 checks the type once it is deduced
        // (`ActOnCleanupAttr`, from `CheckAttributesOnDeducedType`).
        if self.clang_compat < 22 && !self.cleanup_type_ok(d, f, p, at) {
            return None;
        }
        self.mark_function_referenced(f, at);
        self.mark_used_at(f, at);
        Some(f)
    }

    /// Checks the type of `cleanup` function `f`'s parameter `p` against
    /// the address of variable `d` (diagnosed at `at`, the argument).
    fn cleanup_type_ok(&mut self, d: DeclId, f: DeclId, p: DeclId, at: Loc) -> bool {
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::INT);
        let vty = self.ast.decls.get(d).ty().unwrap_or(QualType::INT);
        if self.cleanup_param_compatible(pty, vty) {
            return true;
        }
        let shown = self.ty_arg(vty);
        // clang prints `T *` for the address of `T`; for a reference
        // `int &*` (a pointer to it cannot be formed).
        let vptr = if self.ast.types.is_reference(vty) {
            format!("{}*'", shown.strip_suffix('\'').unwrap_or(&shown))
        } else {
            let q = self.ast.types.pointer_to(vty);
            self.ty_arg(q)
        };
        let fname = format!("'{}'", self.decl_name_str(self.ast.decls.get(f).name()));
        let msg = self
            .diag(DiagId::sema_cleanup_func_arg_incompatible_type, at)
            .arg(fname)
            .arg(self.ty_arg(pty))
            .arg(vptr);
        self.emit(msg);
        false
    }

    /// clang 22 `ActOnCleanupAttr`: once variable `v`'s type is deduced
    /// (its initializer attached, or none), the `cleanup` function's
    /// parameter type is checked against `v`'s address; a mismatch drops
    /// the attribute. A dependent `v` waits for its instantiation.
    pub(crate) fn check_cleanup_deduced(&mut self, v: DeclId) {
        if self.clang_compat < 22 || !self.ast.decls.get(v).has(DeclFlags::HAS_ATTRS) {
            return;
        }
        let list = self.ast.attrs.of_decl(v);
        let attrs: Vec<AttrId> = self.ast.lists.iter(list).collect();
        let Some(i) = attrs.iter().position(|&a| self.ast.attrs.get(a).kind == AttrKind::CLEANUP)
        else {
            return;
        };
        let node = *self.ast.attrs.get(attrs[i]);
        let vty = self.ast.decls.get(v).ty().unwrap_or(QualType::INT);
        let Some(f) = DeclId::from_raw(node.a) else { return };
        if self.ast.types.is_dependent(vty) {
            return;
        }
        let at = Loc::from_raw(node.b);
        let Some(&p) = self.param_decls(f).first() else { return };
        if self.cleanup_type_ok(v, f, p, at) {
            return;
        }
        let mut kept = attrs;
        kept.remove(i);
        let list = self.ast.lists.push(&kept);
        let _ = self.ast.attrs.decl_attrs.refine(v, list);
    }

    /// The function the `cleanup` argument `e` (at `at`) names, with its
    /// name as printed; `None` after a diagnostic. A qualified name or a
    /// template-id is accepted with a GCC-compatibility warning.
    fn cleanup_callee(&mut self, e: ExprId, at: Loc) -> Option<(DeclId, String)> {
        let kind = self.ast.exprs.get(e).kind();
        // A template-id kiln resolved to its one specialization early is
        // still the overload set clang sees.
        let ule = if kind == ExprKind::UnresolvedLookupExpr {
            Some(e)
        } else {
            self.ovl.template_id_refs.get(&e).copied()
        };
        if let Some(u) = ule {
            let fname = format!("'{}'", self.decl_name_str(self.ast.unresolved_name(u)));
            if self.ast.templates.expr_targs.get(u).is_some() {
                self.error(DiagId::sema_cleanup_ext, at);
            }
            let spec = if u == e {
                self.single_template_spec(u).map(|(s, _)| s)
            } else {
                Some(self.ast.exprs.decl(e))
            };
            let Some(f) = spec else {
                let msg = self.diag(DiagId::sema_cleanup_arg_not_single_function, at).arg(fname);
                let notes: Vec<_> = self
                    .ule_candidates(u)
                    .into_iter()
                    .map(|(c, _)| self.plain_candidate_note(c))
                    .collect();
                self.emit(notes.into_iter().fold(msg, kiln_diag::Diag::with_note));
                return None;
            };
            return Some((f, fname));
        }
        if kind == ExprKind::DeclRefExpr {
            let d = self.ast.exprs.decl(e);
            let fname = format!("'{}'", self.decl_name_str(self.ast.decls.get(d).name()));
            if self.ast.templates.expr_begin.get(e).is_some() {
                self.error(DiagId::sema_cleanup_ext, at);
            }
            if !self.kind_of(d).is_function() {
                let msg = self.diag(DiagId::sema_cleanup_arg_named_not_function, at).arg(fname);
                self.emit(msg);
                return None;
            }
            return Some((d, fname));
        }
        self.error(DiagId::sema_cleanup_arg_not_function, at);
        None
    }

    /// clang `CheckAssignmentConstraints(param, T *) == Compatible` for a
    /// `cleanup` function parameter of type `param` and a variable of
    /// type `var` (C rules: `bool`, or a pointer whose pointee is `T` or
    /// `void` with at least `T`'s qualifiers (any, under clang 22), or a
    /// reference to exactly `T *`). A dependent, undeduced or reference
    /// `var` never matches.
    fn cleanup_param_compatible(&self, param: QualType, var: QualType) -> bool {
        let t = &self.ast.types;
        if t.is_dependent(var) || self.is_undeduced_auto(var) || t.is_reference(var) {
            return false;
        }
        let rp = t.canonical(var);
        let p = t.canonical(param);
        let pointee = |q: QualType| t.pointee(q).map(|x| t.canonical(x));
        if t.is_reference(p) {
            return pointee(p).is_some_and(|pe| {
                pe.quals().is_empty() && t.is_pointer(pe) && pointee(pe) == Some(rp)
            });
        }
        let p = p.without_quals();
        if t.builtin(p) == Some(BuiltinType::Bool) {
            return true;
        }
        if !t.is_pointer(p) {
            return false;
        }
        let Some(lpe) = pointee(p) else { return false };
        // clang 22 `IsAssignConvertCompatible` also accepts
        // `CompatiblePointerDiscardsQualifiers`.
        if self.clang_compat < 22 && !lpe.quals().contains(rp.quals()) {
            return false;
        }
        t.is_void(lpe) || lpe.without_quals() == rp.without_quals()
    }
}
