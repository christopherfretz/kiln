//! The remaining kinds of K-3d: statement expressions, coroutines,
//! `asm`, label addresses, the GNU, vector and atomic builtin forms,
//! `_Generic`, constraints. Each writes its tag, then its fixed layout.
//!
//! Coroutines: `co_await` and `co_yield` hash their written operand and
//! the resolved awaiter calls (`await_ready`, `await_suspend`,
//! `await_resume`, and the awaiter itself: the `operator co_await` and
//! `await_transform` sema selected); `co_return` its operand and the
//! promise call; a coroutine body the written body and, as implicit nodes
//! holding resolved callees, the promise variable, the initial and final
//! suspend points, `unhandled_exception`, the implicit `co_return;`, the
//! frame allocation and deallocation calls and `get_return_object`. The
//! parameter copies and the returned-object plumbing follow from those
//! and the declarator: not hashed.

use kiln_ast::expr::{ExprKind as K, GenericControl, OffsetofStep};
use kiln_ast::ids::ListId;
use kiln_ast::stmt::StmtKind as S;
use kiln_ast::{DeclId, ExprId, StmtId};

use super::{Area, DefHasher, Site, Work};

impl DefHasher<'_> {
    /// `co_await` / `co_yield`: implicit bit and suspend kind, then the
    /// operand, the awaiter and the three resolved calls.
    pub(super) fn coawait(&mut self, e: ExprId, k: K) {
        let ast = self.ast;
        let x = &ast.exprs;
        let l = ListId::new(x.get(e).a());
        if ast.lists.len(l) != 6 {
            return self.partial(Area::Dependent, Site::Expr(k));
        }
        self.expr_tag(k);
        self.byte(u8::from(x.coawait_is_implicit(e)) | (x.coawait_suspend_kind(e) as u8) << 1);
        // operand, common (the awaiter), ready, suspend, resume; the
        // opaque value is the awaiter again.
        self.count(5);
        self.stack.extend(ast.lists.iter::<ExprId>(l).take(5).map(Work::Expr));
    }

    /// The GNU and builtin expression forms, constraints, pseudo-objects.
    #[allow(
        clippy::too_many_lines,
        clippy::many_single_char_names,
        reason = "one arm per expression kind"
    )]
    pub(super) fn misc_expr(&mut self, e: ExprId, k: K) {
        let ast = self.ast;
        let x = &ast.exprs;
        let n = *x.get(e);
        let id = ExprId::try_new;
        match k {
            K::StmtExpr => {
                self.expr_tag(k);
                self.stack.push(Work::Stmt(x.stmt_expr_body(e)));
            }
            K::AddrLabelExpr => {
                self.expr_tag(k);
                self.label(x.addr_label(e));
            }
            // The (constant) condition and the operand it selected.
            K::ChooseExpr => {
                let [cond, _, _] = x.choose_operands(e);
                let Some(chosen) = x.choose_chosen(e) else {
                    return self.partial(Area::Dependent, Site::Expr(k));
                };
                self.expr_tag(k);
                self.stack.extend([Work::Expr(cond), Work::Expr(chosen)]);
            }
            // The controlling type and the selected association: the
            // others are never evaluated (and a unit could not tell).
            K::GenericSelectionExpr => {
                let Some(sel) = x.selected_operand(e) else {
                    return self.partial(Area::Dependent, Site::Expr(k));
                };
                self.expr_tag(k);
                match x.generic_control(e, &ast.lists) {
                    GenericControl::Type(q, _) => {
                        self.byte(1);
                        self.ty(q);
                    }
                    GenericControl::Expr(c) => {
                        self.byte(0);
                        self.ty(x.get(c).ty());
                    }
                }
                self.stack.push(Work::Expr(sel));
            }
            // `a ?: b`: the common operand, the condition over it, the
            // false arm.
            K::BinaryConditionalOperator => {
                let [common, cond, _, f] = x.binary_conditional_operands(e, &ast.lists);
                self.expr_tag(k);
                self.stack.extend([Work::Expr(common), Work::Expr(cond), Work::Expr(f)]);
            }
            K::AtomicExpr => {
                self.expr_tag(k);
                self.u32(x.atomic_op(e));
                self.exprs(x.atomic_subs(e));
            }
            // The vectors and the index expressions as written.
            K::ShuffleVectorExpr => {
                self.expr_tag(k);
                self.exprs(x.shuffle_subs(e));
            }
            K::ConvertVectorExpr | K::ImaginaryLiteral => {
                self.expr_tag(k);
                self.ty(n.ty());
                self.stack.push(Work::OptExpr(id(n.a())));
            }
            K::ExtVectorElementExpr => {
                self.expr_tag(k);
                let s = ast.symbols().as_str(x.ext_vector_accessor(e));
                self.str(s);
                self.stack.push(Work::OptExpr(id(n.a())));
            }
            K::VAArgExpr => {
                let (op, ty, _) = x.va_arg(e);
                self.expr_tag(k);
                self.ty(ty);
                self.stack.push(Work::Expr(op));
            }
            // The type, then the offset as computed (a layout difference
            // is a kind 3 difference too) or its steps.
            K::OffsetOfExpr => {
                self.expr_tag(k);
                self.ty(n.ty());
                match x.offsetof_layout(e) {
                    None => {
                        self.byte(0);
                        self.u64(x.offsetof_value(e));
                    }
                    Some(steps) => {
                        self.byte(1);
                        self.count(steps.len());
                        for s in steps {
                            match *s {
                                OffsetofStep::Offset(b) => {
                                    self.byte(0);
                                    self.u64(b);
                                }
                                OffsetofStep::Index { index, elem_size } => {
                                    self.byte(1);
                                    self.u64(elem_size);
                                    self.stack.push(Work::Expr(index));
                                }
                            }
                        }
                    }
                }
            }
            // A constraint outside a template is a constant: its value.
            K::ConceptSpecializationExpr | K::RequiresExpr => match x.satisfied(e) {
                Some(v) if n.deps().is_none() => {
                    self.expr_tag(k);
                    self.byte(u8::from(v));
                }
                // A dependent concept-id: the concept and its arguments
                // (signature.rs).
                _ if k == K::ConceptSpecializationExpr => self.dep_expr(e, k),
                _ => self.partial(Area::Dependent, Site::Expr(k)),
            },
            // The semantic result (what is evaluated).
            K::PseudoObjectExpr => {
                self.expr_tag(k);
                self.stack.push(Work::Expr(x.pseudo_result(e, &ast.lists)));
            }
            _ => self.partial(Area::Misc, Site::Expr(k)),
        }
    }

    /// Coroutine bodies, `co_return`, `asm`.
    pub(super) fn misc_stmt(&mut self, s: StmtId, k: S) {
        let ast = self.ast;
        let st = &ast.stmts;
        match k {
            S::CoroutineBodyStmt => {
                let d = *st.coroutine_body(s);
                self.stmt_tag(k);
                self.stack.extend([
                    Work::Stmt(d.promise_stmt),
                    Work::Expr(d.init_suspend),
                    Work::Stmt(d.body),
                    Work::Expr(d.final_suspend),
                    Work::OptStmt(d.on_exception),
                    Work::OptStmt(d.on_fallthrough),
                    Work::Expr(d.allocate),
                    Work::Expr(d.deallocate),
                    Work::Expr(d.return_value),
                    Work::OptStmt(d.return_on_alloc_failure),
                ]);
            }
            S::CoreturnStmt => {
                self.stmt_tag(k);
                self.byte(u8::from(st.coreturn_is_implicit(s)));
                self.stack.extend([
                    Work::OptExpr(st.coreturn_operand(s)),
                    Work::OptExpr(st.coreturn_promise_call(s)),
                ]);
            }
            // The flags, the template, each operand (name, constraint,
            // expression), the clobbers, the `asm goto` labels by index.
            S::GCCAsmStmt => {
                let d = *st.asm_data(s);
                self.stmt_tag(k);
                self.u32(u32::from(st.asm_bits(s)));
                self.u32(d.num_outputs);
                self.stack.push(Work::Str(d.string));
                let ops = st.asm_operands(s);
                self.count(ops.len());
                for op in ops {
                    let name = op.name.map_or("", |y| ast.symbols().as_str(y));
                    self.str(name);
                    self.stack.extend([Work::Str(op.constraint), Work::Expr(op.expr)]);
                }
                self.count(ast.lists.len(d.clobbers));
                self.stack.extend(
                    ast.lists.iter::<kiln_ast::ids::StringLitId>(d.clobbers).map(Work::Str),
                );
                self.count(ast.lists.len(d.labels));
                for l in ast.lists.iter::<DeclId>(d.labels) {
                    self.label(l);
                }
            }
            _ => self.partial(Area::Misc, Site::Stmt(k)),
        }
    }
}
