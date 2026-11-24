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
    /// A precondition of a constructor (0) or a postcondition of a
    /// destructor (1) of the class: an implicit `(*this).` there is
    /// ill-formed ([expr.prim.id.general]/2, [over.call.func]/3.1).
    pub(crate) no_implicit_this: Option<(DeclId, u8)>,
}

/// What [`Sema::act_on_start_contract`] opened, for
/// [`Sema::act_on_end_contract`].
#[derive(Clone, Copy, Debug)]
pub struct ContractScope {
    kind: ContractKind,
    result: Option<DeclId>,
    /// The function a precondition or postcondition belongs to.
    func: Option<DeclId>,
    /// The context to restore after the predicate, when it was analysed
    /// in the function's (see [`Sema::act_on_start_contract`]).
    saved_ctx: Option<DeclId>,
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
        self.has_contracts = true;
        let watermark = self.ast.decls.next_id().index();
        self.push_scope(ScopeKind::Block);
        let result = result.map(|(name, loc)| {
            let ty = match f {
                Some(f) => self.contract_result_type(f, loc, definition),
                None => QualType::ERROR,
            };
            let ctx = f.map_or(self.ctx, |f| self.contract_fn(f));
            // The result name of a templated function is in a dependent
            // context, so a predicate naming it is rebuilt for each
            // specialization even when it is not type- or value-dependent
            // (`template<class T> int g(int a) post(r: r > 0)`; WP-1463).
            let mut flags =
                if ty == QualType::ERROR { DeclFlags::INVALID } else { DeclFlags::empty() };
            if self.in_template() || self.ast.decls.get(ctx).has(DeclFlags::DEPENDENT_CTX) {
                flags = flags.with(DeclFlags::DEPENDENT_CTX);
            }
            let d = self.ast.decls.push(
                NewDecl {
                    kind: DeclKind::ContractResultName,
                    flags,
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
            if let Some(f) = f {
                self.check_result_name_conflict(self.contract_fn(f), name, loc);
            }
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
        let no_implicit_this = f.and_then(|f| {
            let f = self.contract_fn(f);
            let which = match (kind, self.kind_of(f)) {
                (ContractKind::Pre, DeclKind::CXXConstructor) => 0,
                (ContractKind::Post, DeclKind::CXXDestructor) => 1,
                _ => return None,
            };
            Some((self.ast.decls.get(f).ctx()?, which))
        });
        self.contract_ctx.push(ContractCtx {
            watermark,
            lambdas: self.lambdas.len(),
            this,
            no_implicit_this,
        });
        // A precondition or postcondition is in the function parameter
        // scope of `f` ([basic.scope.contract]): a lambda in its predicate
        // is in `f` and may capture `f`'s parameters
        // ([expr.prim.lambda.capture]/7, spec/contracts.md §5). The seq is
        // replayed outside `f`, so its context is entered for the
        // predicate.
        let saved_ctx = f
            .map(|f| self.contract_fn(f))
            .filter(|&f| f != self.ctx && self.kind_of(f).is_function())
            .map(|f| {
                let saved = self.ctx;
                self.ctx = f;
                saved
            });
        ContractScope { kind, result, func: f, saved_ctx }
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
        let site_text = cond.map(|c| self.expr_range(c));
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
        if let Some(c) = sc.saved_ctx {
            self.ctx = c;
        }
        let s = self.builder().contract_stmt(sc.kind, cond, sc.result, loc);
        if !self.in_template() {
            let ctx = sc.func.map_or(self.ctx, |f| self.contract_fn(f));
            self.record_contract_site(s, loc, ctx, site_text);
        }
        s
    }

    /// Records the `contract_violation` data of the contract assertion
    /// `s` whose keyword is at `loc`, belonging to function `ctx`, with
    /// the predicate spelled at `text` (spec/contracts.md §6.3): GCC's
    /// presumed file, line and keyword column, `__PRETTY_FUNCTION__`, and
    /// the predicate's source text as the comment.
    fn record_contract_site(
        &mut self,
        s: StmtId,
        loc: Loc,
        ctx: DeclId,
        text: Option<kiln_source::Span>,
    ) {
        let sm = self.diags.sm();
        let (file, line, column) = match sm.presumed_line_col(loc) {
            Some(p) => (sm.remap_macro_path(&sm.presumed_file_name(&p, loc)), p.line, p.col),
            None => (String::new(), 0, 0),
        };
        let comment = text.and_then(|t| predicate_text(sm, t)).unwrap_or_default().to_owned();
        let (_, pretty) = self.predefined_function_names(ctx);
        let (Some(file), Some(function), Some(comment)) = (
            self.contract_string(&file),
            self.contract_string(&pretty),
            self.contract_string(&comment),
        ) else {
            return;
        };
        let site = kiln_ast::stmt::ContractSite { file, function, comment, line, column };
        let _ = self.ast.contract_sites.insert(s, site);
    }

    /// The function a contract of `f` belongs to: a function template's
    /// pattern.
    /// [basic.scope.contract]/2.1: a result name that is not
    /// name-independent (`_`) and potentially conflicts with a parameter of
    /// its function makes the program ill-formed (diagnosed as a
    /// redefinition of the parameter; WP-1463).
    fn check_result_name_conflict(&mut self, f: DeclId, name: Symbol, loc: Loc) {
        let spelled = crate::decl::sym_str(self, name);
        if spelled == "_" {
            return;
        }
        let Some(params) = self.ast.decls.function(f).map(|x| x.params) else { return };
        let clash = self
            .ast
            .lists
            .iter::<DeclId>(params)
            .any(|p| self.ast.decls.get(p).name().as_ident() == Some(name));
        if clash {
            let d = self.diag(DiagId::sema_param_redefinition, loc).arg(spelled);
            self.emit(d);
        }
    }

    /// [expr.prim.id.general]/2, [over.call.func]/3.1: in the predicate
    /// of a precondition of a constructor or a postcondition of a
    /// destructor of class X, a potentially-evaluated id-expression naming
    /// a member of X (or a base) through an implicit `(*this).`, and an
    /// unqualified call selecting a non-static member function, are
    /// ill-formed (WP-1463).
    pub(crate) fn check_contract_implicit_this(&mut self, loc: Loc) {
        let Some(&ContractCtx { no_implicit_this: Some((x, which)), .. }) =
            self.contract_ctx.last()
        else {
            return;
        };
        if self.unevaluated > 0
            || self.this_class().is_none_or(|(_, c)| self.ast.first(c) != self.ast.first(x))
        {
            return;
        }
        let d = self.diag(DiagId::sema_contract_implicit_this, loc).arg(which);
        self.emit(d);
    }

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
    /// ([dcl.contract.res]/1-2): its return type, as written (a
    /// template's pattern dumps `T`, not `type-parameter-0-0`, as clang
    /// keeps a declaration's written types).
    fn contract_result_type(&mut self, f: DeclId, loc: Loc, definition: bool) -> QualType {
        let f = self.contract_fn(f);
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let t = &self.ast.types;
        let Some(ret) =
            t.proto_of(ty.ty()).or_else(|| t.proto(ty)).map(kiln_ast::types::FuncProto::ret)
        else {
            return QualType::ERROR;
        };
        if self.ast.types.is_void(ret) {
            self.error(DiagId::sema_contract_result_void, loc);
            return QualType::ERROR;
        }
        if self.contains_undeduced_auto(ret) {
            // [dcl.contract.res]/2 constrains non-templated functions only:
            // a templated one (an abbreviated function template, a member
            // of a class template) deduces its type on instantiation
            // (WP-1485; found running GCC's C++ testsuite).
            if !definition && !self.in_template() {
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
    /// A splice-expression `[: r :]` designating such a variable or
    /// structured binding is built through the same reference and gets
    /// the same type (P3598R0, [expr.prim.splice]/2.5); a splice of an
    /// object reflection is not (`splice_object_expr`).
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
        let mut first = self.ast.contract_first(f);
        if first == f
            && stmts.is_empty()
            && let Some(&p) = self.friend_pattern_prev.get(&f)
        {
            // A non-dependent friend of a templated class: checked on the
            // pattern against the function it will redeclare.
            first = self.ast.contract_first(p);
        }
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

    /// [dcl.contract.func]/6 (the DIS): a virtual, deleted, or
    /// first-declaration-defaulted function has no seq; whether the seq
    /// may appear on `f` (diagnosed otherwise). A function is virtual
    /// when its first declaration is (spec/contracts.md §9: P3097 lifts
    /// the virtual rule only for C++29).
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

    /// [dcl.contract.func]/7 on `f`, a friend function declaration just
    /// instantiated from a templated class without a seq of its own
    /// (`template<class T> struct S { friend int m(T a); };` redeclaring
    /// `int m(const int a) post(r: r == a)`): its parameters, of
    /// dependent type in the pattern, are checked against the first
    /// declaration's postconditions now that their types are known.
    pub(crate) fn check_friend_post_params(&mut self, f: DeclId) {
        let first = self.ast.contract_first(f);
        if first == f || self.ast.decls.cold.contracts.get(f).is_some() {
            return;
        }
        let Some(l) = self.ast.decls.cold.contracts.get(first).copied() else { return };
        let posts: Vec<StmtId> = self.ast.lists.iter::<StmtId>(l).collect();
        self.check_post_params(f, first, &posts);
    }

    /// [dcl.contract.func]/7: a non-reference parameter of `owner` that a
    /// postcondition among `stmts` (of declaration `of`) odr-uses is
    /// `const` on declaration `decl`, and not of array or function type.
    /// In a template a parameter of non-dependent type is checked at
    /// definition (a member or friend of a class template, a function
    /// template), once for the pattern; a dependent one at instantiation.
    fn check_post_params(&mut self, decl: DeclId, of: DeclId, stmts: &[StmtId]) {
        let in_template = self.in_template();
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
            if self.ast.types.is_reference(t)
                || n.has(DeclFlags::INVALID)
                || (in_template && self.is_dependent_type(t))
                || self.post_param_pattern_locs.contains(&n.loc())
            {
                continue;
            }
            // An array or function parameter was adjusted to a pointer
            // ([dcl.fct]/5): `const int a[2]` is no const parameter.
            let is_const = self.ast.types.effective_quals(t).contains(Quals::CONST);
            if !is_const && self.post_param_diagnosed.insert(p) {
                if in_template {
                    self.post_param_pattern_locs.insert(n.loc());
                }
                let msg = self
                    .diag(DiagId::sema_contract_post_nonconst_param, n.loc())
                    .with_note(self.diag(DiagId::note_declared_at, n.loc()));
                self.emit(msg);
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
        let owner = if self.ast.decls.cold.contracts.get(f).is_some() {
            f
        } else {
            self.ast.contract_first(f)
        };
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
            if self.ast.types.is_reference(t)
                || n.has(DeclFlags::INVALID)
                || self.post_param_diagnosed.contains(&p)
                || self.post_param_pattern_locs.contains(&n.loc())
            {
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

/// The predicate spelled at `t` as written (the `contract_violation`
/// comment, spec/contracts.md §6.3): a bound in a macro argument is where
/// the argument is written, one in a macro body widens to that
/// invocation, level by level; a range that does not come out balanced
/// widens both bounds to the outermost invocations.
fn predicate_text(sm: &kiln_source::SourceManager, t: kiln_source::Span) -> Option<&str> {
    let to_file = |mut l: Loc, hi: bool| {
        for _ in 0..64 {
            let Some(e) = sm.expansion_info(l) else { break };
            l = match e.kind {
                kiln_source::ExpansionKind::MacroArg => {
                    e.spelling.with_offset(l.to_raw() - e.base.to_raw())
                }
                kiln_source::ExpansionKind::Macro { .. } | kiln_source::ExpansionKind::Embed
                    if hi =>
                {
                    e.expansion.hi
                }
                kiln_source::ExpansionKind::Macro { .. } | kiln_source::ExpansionKind::Embed => {
                    e.expansion.lo
                }
            };
        }
        l
    };
    let balanced = |s: &str| {
        let mut depth = 0i32;
        for c in s.bytes() {
            match c {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                _ => {}
            }
            if depth < 0 {
                return false;
            }
        }
        depth == 0
    };
    let (lo, hi) = (to_file(t.lo, false), to_file(t.hi, true));
    if lo.is_file()
        && hi.is_file()
        && sm.same_entry_distance(lo, hi).is_some()
        && let Some(s) = sm.span_text(kiln_source::Span::new(lo, hi)).filter(|s| balanced(s))
    {
        return Some(s);
    }
    let lo = if t.lo.is_macro() { sm.expansion_span(t.lo).lo } else { t.lo };
    let hi = if t.hi.is_macro() { sm.expansion_span(t.hi).hi } else { t.hi };
    sm.span_text(kiln_source::Span::new(lo, hi))
}
