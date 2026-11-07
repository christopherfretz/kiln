//! Contract assertions (P2900; spec/contracts.md §4-§5): the result-name
//! declaration of a postcondition, the const-ification of names in a
//! predicate ([expr.prim.id.unqual]/6-7, [expr.prim.this]/2), and the
//! checks on a function's *function-contract-specifier-seq*
//! ([dcl.contract.func], [dcl.contract.res]). Evaluation is WP-707's.

use kiln_ast::decl::{DeclFlags, DeclKind, DeclMisc, FnSpec, NewDecl, Payload};
use kiln_ast::expr::ExprKind;
use kiln_ast::names::DeclName;
use kiln_ast::stmt::ContractKind;
use kiln_ast::types::Quals;
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::conv;
use crate::scope::ScopeKind;

/// A contract assertion whose predicate is being analysed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ContractCtx {
    /// `decls.len()` when the assertion began: a declaration with a
    /// lower index was declared outside it ([expr.prim.id.unqual]/7).
    pub(crate) watermark: u32,
    /// `lambdas.len()` when the assertion began: deeper is a lambda in
    /// the predicate.
    pub(crate) lambdas: usize,
    /// The type of `this` there and its pointer-to-const version
    /// ([expr.prim.this]/2).
    pub(crate) this: Option<(QualType, QualType)>,
}

/// What [`Sema::act_on_start_contract`] opened, for
/// [`Sema::act_on_end_contract`].
#[derive(Debug)]
pub struct ContractScope {
    kind: ContractKind,
    result: Option<DeclId>,
}

impl ContractScope {
    /// The result name the assertion declares.
    pub fn result_name(&self) -> Option<DeclId> {
        self.result
    }
}

/// How the declaration carrying a *function-contract-specifier-seq* ends
/// ([dcl.contract.func]/6).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ContractDeclKind {
    /// A declaration that is no definition (or whose kind sema reads
    /// from the function, a member replayed after its class).
    Declaration,
    /// A definition with a body.
    Definition,
    /// `= delete`.
    Deleted,
    /// `= default`.
    Defaulted,
}

impl Sema<'_, '_> {
    /// Before the predicate of a contract assertion of `kind` at `loc`:
    /// a postcondition's result name (`r :`) of function `f` is declared
    /// in a scope of its own ([basic.scope.contract]); names declared
    /// outside become `const` ([expr.prim.id.unqual]/7). `definition`:
    /// `f`'s body was seen (or is being replayed after), so a deduced
    /// return type may be named ([dcl.contract.res]/2).
    pub fn act_on_start_contract(
        &mut self,
        kind: ContractKind,
        f: Option<DeclId>,
        result: Option<(Symbol, Loc)>,
        definition: bool,
    ) -> ContractScope {
        let watermark = self.ast.decls.next_id().index();
        self.push_scope(ScopeKind::Block);
        let result = result.map(|(name, loc)| {
            let ty = match f {
                Some(f) => self.contract_result_type(f, loc, definition),
                None => QualType::ERROR,
            };
            let ctx = f.map_or(self.ctx, |f| self.contract_fn(f));
            let d = self.ast.decls.push(
                NewDecl {
                    kind: DeclKind::ContractResultName,
                    flags: if ty == QualType::ERROR {
                        DeclFlags::INVALID
                    } else {
                        DeclFlags::empty()
                    },
                    misc: DeclMisc::default(),
                    name: DeclName::ident(name),
                    ctx: Some(ctx),
                    loc,
                    ty: Some(ty),
                },
                // ContractResultName payload: unused (0); spec/contracts.md §4.
                Payload::None(0),
            );
            self.push_local(name, d);
            d
        });
        let this = self.current_this().map(|t| {
            let c = match self.pointee_sugared(t) {
                Some(p) => {
                    let p = self.ast.types.qualify(p, Quals::CONST);
                    self.ast.types.pointer_to(p)
                }
                None => t,
            };
            (t, c)
        });
        self.contract_ctx.push(ContractCtx { watermark, lambdas: self.lambdas.len(), this });
        ContractScope { kind, result }
    }

    /// After the predicate `cond` (`None`: it failed to parse) of the
    /// contract assertion opened by `sc` at `loc`: the predicate is
    /// contextually converted to `bool` and is a full-expression
    /// ([basic.contract.general]/2). Returns the `ContractAssertStmt`.
    pub fn act_on_end_contract(
        &mut self,
        sc: ContractScope,
        cond: Option<ExprId>,
        loc: Loc,
    ) -> StmtId {
        let cond = match cond {
            Some(c) => {
                let c = match conv::contextually_convert_to_bool(self, c) {
                    Ok(b) => b,
                    Err(err) => {
                        self.emit_conv_error(err, self.ast.exprs.get(c).loc());
                        let at = self.begin_loc(c);
                        self.recover_err(&[c], at)
                    }
                };
                self.act_on_full_expr(c)
            }
            None => self.recover_err(&[], loc),
        };
        self.contract_ctx.pop();
        self.pop_scope();
        self.builder().contract_stmt(sc.kind, cond, sc.result, loc)
    }

    /// The function a contract of `f` belongs to: a function template's
    /// pattern.
    pub(crate) fn contract_fn(&self, f: DeclId) -> DeclId {
        if self.kind_of(f) == DeclKind::FunctionTemplate {
            self.pattern_of(f).unwrap_or(f)
        } else {
            f
        }
    }

    /// Whether the declared return type of `f` is still a placeholder
    /// (the parser then replays a definition's contracts after its body).
    pub fn contract_return_undeduced(&self, f: DeclId) -> bool {
        let f = self.contract_fn(f);
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        self.ast
            .types
            .proto(ty)
            .is_some_and(|p| self.contains_undeduced_auto(kiln_ast::types::FuncProto::ret(p)))
    }

    /// The type of the result name of a postcondition of `f`
    /// ([dcl.contract.res]/1-2): its return type.
    fn contract_result_type(&mut self, f: DeclId, loc: Loc, definition: bool) -> QualType {
        let f = self.contract_fn(f);
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let Some(ret) = self.ast.types.proto(ty).map(kiln_ast::types::FuncProto::ret) else {
            return QualType::ERROR;
        };
        if self.ast.types.is_void(ret) {
            self.error(DiagId::sema_contract_result_void, loc);
            return QualType::ERROR;
        }
        if self.contains_undeduced_auto(ret) {
            if !definition {
                self.error(DiagId::sema_contract_result_deduced, loc);
                return QualType::ERROR;
            }
            // A template's body deduces nothing: the name is
            // type-dependent until instantiation.
            return QualType::DEPENDENT;
        }
        ret
    }

    /// `contract_assert ( cond ) ;` at `loc` ([stmt.contract.assert]).
    pub fn act_on_start_contract_assert(&mut self) -> ContractScope {
        self.act_on_start_contract(ContractKind::Assert, None, None, true)
    }

    /// The type of an id-expression naming `d`, of type `ty`, inside the
    /// predicate of a contract assertion: `const` if `d` is a variable,
    /// a reference template parameter, or a structured binding declared
    /// outside the assertion, or a result name ([expr.prim.id.unqual]/6-7).
    /// A name that denotes a member of the closure of a lambda in the
    /// predicate (a capture by copy) is not const-ified.
    pub(crate) fn contract_constify(
        &mut self,
        d: DeclId,
        ty: QualType,
        enclosing: bool,
    ) -> QualType {
        let Some(&c) = self.contract_ctx.last() else { return ty };
        let n = *self.ast.decls.get(d);
        let applies = match n.kind() {
            DeclKind::ContractResultName => true,
            DeclKind::Var | DeclKind::ParmVar | DeclKind::Binding => d.index() < c.watermark,
            DeclKind::NonTypeTemplateParm => n.ty().is_some_and(|t| self.ast.types.is_reference(t)),
            _ => false,
        };
        if !applies || ty == QualType::ERROR || ty == QualType::DEPENDENT {
            return ty;
        }
        if enclosing && self.lambdas.len() > c.lambdas && self.captured_by_copy_since(d, c.lambdas)
        {
            return ty;
        }
        self.ast.types.qualify(ty, Quals::CONST)
    }

    /// Whether a lambda opened at or above `lambdas[from]` captures `d`
    /// by copy (explicitly or by its `=` default).
    fn captured_by_copy_since(&self, d: DeclId, from: usize) -> bool {
        use kiln_ast::lambda::{CaptureDefault, CaptureKind};
        self.lambdas.iter().skip(from).any(|l| {
            let data = self.ast.exprs.lambdas.get(l.id);
            match data.captures.iter().find(|x| x.var == Some(d)) {
                Some(x) => x.kind == CaptureKind::ByCopy,
                None => data.default == CaptureDefault::ByCopy,
            }
        })
    }

    /// `this` inside a contract predicate, also in a lambda there that
    /// captures it: a pointer to const ([expr.prim.this]/2). A `*this`
    /// capture copies the const object, so stays const (as `[*this]` in a
    /// const member function, clang and GCC).
    pub(crate) fn contract_this(&self, t: QualType) -> QualType {
        match self.contract_ctx.last().and_then(|c| c.this) {
            Some((orig, k)) if orig == t => k,
            _ => t,
        }
    }

    /// The *function-contract-specifier-seq* `stmts` (possibly empty) of
    /// the declaration `f` of a function, which ends as `how`
    /// ([dcl.contract.func]/4-7). Records it and checks it against the
    /// earlier declarations.
    pub fn act_on_function_contracts(
        &mut self,
        f: Option<DeclId>,
        stmts: &[StmtId],
        how: ContractDeclKind,
        loc: Loc,
    ) {
        let Some(f) = f else { return };
        let f = self.contract_fn(f);
        if self.ast.decls.function(f).is_none() {
            return;
        }
        let first = self.ast.first(f);
        let first_list =
            if first == f { None } else { self.ast.decls.cold.contracts.get(first).copied() };
        if stmts.is_empty() {
            // A later declaration without a seq: the const rule on the
            // postconditions' parameters holds on every declaration
            // ([dcl.contract.func]/7).
            if let Some(l) = first_list {
                let posts: Vec<StmtId> = self.ast.lists.iter::<StmtId>(l).collect();
                self.check_post_params(f, first, &posts);
            }
            return;
        }
        if !self.check_contract_placement(f, how, loc) {
            return;
        }
        let list = self.ast.lists.push(stmts);
        let _ = self.ast.decls.cold.contracts.insert(f, list);
        self.ast.decls.mark_function(f, FnSpec::new().with_has_contracts(true));
        if first != f {
            match first_list {
                // A function template specialization's first declaration
                // may be the one deduction made before its explicit
                // specialization was declared (spec/contracts.md §9).
                None if self.ast.spec_info(f).is_some() => {}
                None => {
                    let msg = self
                        .diag(DiagId::sema_contract_redecl_adds, loc)
                        .arg(self.decl_name(f))
                        .with_note(self.diag(
                            DiagId::note_previous_declaration,
                            self.ast.decls.get(first).loc(),
                        ));
                    self.emit(msg);
                }
                Some(l) => {
                    let prev: Vec<StmtId> = self.ast.lists.iter::<StmtId>(l).collect();
                    if !self.contract_seqs_equivalent(&prev, stmts) {
                        let msg = self
                            .diag(DiagId::sema_contract_redecl_mismatch, loc)
                            .arg(self.decl_name(f))
                            .with_note(self.diag(
                                DiagId::note_previous_declaration,
                                self.ast.decls.get(first).loc(),
                            ));
                        self.emit(msg);
                    }
                }
            }
        }
        self.check_post_params(f, f, stmts);
        if first != f {
            let posts = stmts.to_vec();
            self.check_post_params(first, f, &posts);
        }
    }

    /// [dcl.contract.func]/6 and the virtual-function rule: whether the
    /// seq may appear on `f` (diagnosed otherwise).
    fn check_contract_placement(&mut self, f: DeclId, how: ContractDeclKind, loc: Loc) -> bool {
        let spec = self.ast.decls.function(f).map(|x| x.spec).unwrap_or_default();
        let deleted = how == ContractDeclKind::Deleted || spec.is_deleted();
        let defaulted = how == ContractDeclKind::Defaulted
            || spec.defaulted() == kiln_ast::decl::Defaulted::Explicit && self.ast.first(f) == f;
        if deleted || defaulted {
            let msg =
                self.diag(DiagId::sema_contract_deleted_or_defaulted, loc).arg(u8::from(!deleted));
            self.emit(msg);
            return false;
        }
        if matches!(
            self.kind_of(f),
            DeclKind::CXXMethod | DeclKind::CXXDestructor | DeclKind::CXXConversion
        ) && self.ast.decls.function(self.ast.first(f)).is_some_and(|x| x.spec.is_virtual())
        {
            self.error(DiagId::sema_contract_virtual, loc);
            return false;
        }
        true
    }

    /// [dcl.contract.func]/5: the same contract assertions in the same
    /// order, with equivalent predicates and result names on both or
    /// neither.
    fn contract_seqs_equivalent(&self, a: &[StmtId], b: &[StmtId]) -> bool {
        a.len() == b.len()
            && a.iter().zip(b).all(|(&x, &y)| {
                let s = &self.ast.stmts;
                s.contract_kind(x) == s.contract_kind(y)
                    && s.contract_result_name(x).is_some() == s.contract_result_name(y).is_some()
                    && self.exprs_equivalent(s.contract_cond(x), s.contract_cond(y))
            })
    }

    /// [dcl.contract.func]/7: a non-reference parameter of `owner` that a
    /// postcondition among `stmts` (of declaration `of`) odr-uses is
    /// `const` on declaration `decl`, and not of array or function type.
    fn check_post_params(&mut self, decl: DeclId, of: DeclId, stmts: &[StmtId]) {
        if self.in_template() {
            return;
        }
        let of_params: Vec<DeclId> = self
            .ast
            .decls
            .function(of)
            .map(|x| self.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        let decl_params: Vec<DeclId> = self
            .ast
            .decls
            .function(decl)
            .map(|x| self.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        for i in self.post_odr_used_params(&of_params, stmts) {
            let Some(&p) = decl_params.get(i) else { continue };
            let n = *self.ast.decls.get(p);
            let Some(t) = n.ty() else { continue };
            if self.ast.types.is_reference(t) || n.has(DeclFlags::INVALID) {
                continue;
            }
            // An array or function parameter was adjusted to a pointer
            // ([dcl.fct]/5): `const int a[2]` is no const parameter.
            let is_const = self.ast.types.effective_quals(t).contains(Quals::CONST);
            if !is_const {
                let msg = self
                    .diag(DiagId::sema_contract_post_nonconst_param, n.loc())
                    .with_note(self.diag(DiagId::note_declared_at, n.loc()));
                self.emit(msg);
                self.ast.decls.set_flags(p, DeclFlags::INVALID);
            }
        }
    }

    /// The indices into `params` of the parameters a postcondition among
    /// `stmts` odr-uses, in first-use order.
    fn post_odr_used_params(&self, params: &[DeclId], stmts: &[StmtId]) -> Vec<usize> {
        let mut used = Vec::new();
        for &s in stmts {
            if self.ast.stmts.contract_kind(s) != ContractKind::Post {
                continue;
            }
            let mut work = vec![self.ast.stmts.contract_cond(s)];
            while let Some(e) = work.pop() {
                if self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr
                    && self.ast.exprs.non_odr_use(e) == kiln_ast::expr::NonOdrUse::None
                {
                    let d = self.ast.exprs.decl(e);
                    if let Some(i) = params.iter().position(|&p| p == d)
                        && !used.contains(&i)
                    {
                        used.push(i);
                    }
                }
                self.ast.exprs.children_into(e, &self.ast.lists, &mut work);
            }
        }
        used
    }

    /// [dcl.fct.def.coroutine]/6 (note), [dcl.contract.func]: a
    /// postcondition of the coroutine `f` odr-uses no non-reference
    /// parameter (it would read the parameter after the coroutine copied
    /// it into its frame). Checked when the body turns out a coroutine.
    pub(crate) fn check_coroutine_post_params(&mut self, f: DeclId) {
        // The seq is on `f` or, for a definition that omits it, on the
        // first declaration, whose parameters the predicates name.
        let owner =
            if self.ast.decls.cold.contracts.get(f).is_some() { f } else { self.ast.first(f) };
        let stmts = self.function_contracts(owner);
        if stmts.is_empty() {
            return;
        }
        let params_of = |s: &Self, d: DeclId| -> Vec<DeclId> {
            s.ast
                .decls
                .function(d)
                .map(|x| s.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default()
        };
        let owner_params = params_of(self, owner);
        let params = params_of(self, f);
        for i in self.post_odr_used_params(&owner_params, &stmts) {
            let Some(&p) = params.get(i) else { continue };
            let n = *self.ast.decls.get(p);
            let Some(t) = n.ty() else { continue };
            if self.ast.types.is_reference(t) || n.has(DeclFlags::INVALID) {
                continue;
            }
            let msg = self
                .diag(DiagId::sema_contract_coroutine_post_param, n.loc())
                .arg(self.decl_name(p));
            self.emit(msg);
        }
    }

    /// True if `d`'s own type is not const, so a const reference to it
    /// in a contract predicate was made const there.
    pub(crate) fn contract_made_const(&self, d: DeclId) -> bool {
        let Some(t) = self.ast.decls.get(d).ty() else { return false };
        let t = self.non_reference(t);
        !self.ast.types.effective_quals(t).is_const()
    }

    /// The non-static data member `e` (`m` or `this->m`) names, when it
    /// is const only because `this` points to const in a contract
    /// predicate ([expr.prim.this]/2): a member declared neither const
    /// nor of reference type, accessed through `this`.
    pub(crate) fn contract_this_member(&self, e: ExprId) -> Option<DeclId> {
        let x = self.ast.exprs.get(e);
        if x.kind() != ExprKind::MemberExpr || !x.has(kiln_ast::expr::ExprFlags::ARROW) {
            return None;
        }
        let obj = self.ignore_paren_imp_casts(self.ast.exprs.operand(e));
        if self.ast.exprs.get(obj).kind() != ExprKind::CXXThisExpr {
            return None;
        }
        let field = self.ast.exprs.decl(e);
        let ty = self.ast.decls.get(field).ty()?;
        (self.ast.decls.get(field).kind() == DeclKind::Field
            && !self.ast.types.is_reference(ty)
            && !self.ast.types.effective_quals(ty).is_const())
        .then_some(field)
    }

    /// The function contract assertions of pattern `pdef`, from the
    /// declaration of it that carries them.
    pub(crate) fn pattern_contracts(&self, pdef: DeclId) -> Option<(DeclId, Vec<StmtId>)> {
        let src = [pdef, self.ast.first(pdef)]
            .into_iter()
            .find(|&d| self.ast.decls.cold.contracts.contains(d))?;
        let l = *self.ast.decls.cold.contracts.get(src)?;
        Some((src, self.ast.lists.iter::<StmtId>(l).collect()))
    }

    /// Records the instantiated function contract assertions `stmts` of
    /// specialization `f`.
    pub(crate) fn install_instantiated_contracts(&mut self, f: DeclId, stmts: &[StmtId]) {
        if stmts.is_empty() {
            return;
        }
        let list = self.ast.lists.push(stmts);
        let _ = self.ast.decls.cold.contracts.insert(f, list);
        self.ast.decls.mark_function(f, FnSpec::new().with_has_contracts(true));
        // [dcl.contract.func]/7 on the instantiated parameter types.
        self.check_post_params(f, f, stmts);
    }

    /// The call operator of the innermost lambda whose body is open.
    pub fn current_lambda_call_op(&self) -> Option<DeclId> {
        self.lambdas.last().and_then(|l| l.call_op)
    }

    /// The function contract assertions of `f` (empty if none).
    pub fn function_contracts(&self, f: DeclId) -> Vec<StmtId> {
        self.ast
            .decls
            .cold
            .contracts
            .get(f)
            .map(|&l| self.ast.lists.iter::<StmtId>(l).collect())
            .unwrap_or_default()
    }
}
