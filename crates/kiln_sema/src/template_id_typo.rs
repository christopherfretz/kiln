//! An unqualified template-id whose name lookup finds nothing or only
//! functions ([temp.names]/3) and that is not called: the template-id
//! names nothing (spec/sema.md "Undeclared template-ids", WP-372,
//! WP-391). clang 19 reports it through delayed typo correction; clang 22
//! typo-corrects a name that lookup found functions for to the same name
//! qualified by its namespace (`val<int> + 1` with `int val(int)`:
//! `use of undeclared identifier 'val'; did you mean '::val'?`).

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ValueCategory;
use kiln_ast::names::DeclName;
use kiln_ast::types::TemplateArg;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{DiagId, FixIt};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use kiln_ast::decl::{DeclFlags, NamespaceData};

use crate::Sema;
use crate::decl::sym_str;
use crate::lookup::LookupKind;

/// What clang 22's typo correction made of a template-id that names
/// nothing ([`Sema::correct_template_id_typo`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateIdTypo {
    /// Corrected with recovery: the expression the enclosing expression
    /// goes on with.
    Recovered(ExprId),
    /// Corrected without recovery (a type or keyword suggestion):
    /// diagnosed, no expression.
    Suggested,
    /// No correction; nothing diagnosed.
    Uncorrected,
}

/// One typo-correction candidate (clang's `TypoCorrection`).
struct Candidate {
    /// clang's unnormalized distance: 100 per character edit, 110 per
    /// qualifier component.
    ed: usize,
    /// The correction as spelled (`template`, `vat`, `::N::fn`).
    text: String,
    /// The declarations found (none for a keyword).
    decls: Vec<DeclId>,
    /// Parallel to `decls`: what lookup found (a using-shadow).
    found: Vec<DeclId>,
    /// Found by qualified lookup.
    qualified: bool,
    /// The scope qualified lookup searched.
    scope: Option<DeclId>,
}

/// A clang 19 correction kiln goes on with
/// ([`Sema::speculate_template_id_typo`]).
struct Speculation {
    expr: ExprId,
    suggest: kiln_diag::Diag,
    fallback: Option<kiln_diag::Diag>,
    ambiguous: bool,
    tie: Option<(QualType, ValueCategory)>,
}

/// What a declaration correction recovers as.
enum Correction {
    Function,
    Member(DeclId),
    Value,
    Quiet,
    Type,
}

impl Sema<'_, '_> {
    /// clang 19's undeclared template-id `name<args>` at `loc` (not
    /// called, not a nested-name-specifier): "use of undeclared
    /// identifier" through delayed typo correction, on an erroneous
    /// expression the enclosing expression goes on parsing around
    /// (clang 19's `TypoExpr`; [`Sema::correct_delayed_typos_in`] finds it,
    /// so a call missing its `)` skips to it quietly). A near name the
    /// expression rebuilt on is valid at the correction point is the
    /// correction ([`Sema::speculate_template_id_typo`]).
    pub fn act_on_undeclared_template_id_typo(&mut self, name: Symbol, loc: Loc) -> ExprId {
        let text = sym_str(self, name);
        let msg = self.diag(DiagId::sema_undeclared_var_use, loc).arg(text);
        if let Some(s) = self.speculate_template_id_typo(name, loc) {
            self.hold_typo_spec(msg, s.suggest, s.expr, loc, s.fallback, s.ambiguous);
            self.set_typo_tie(loc, s.tie);
            return s.expr;
        }
        let e = self.recover_err(&[], loc);
        self.emit_delayed_typo(msg, e);
        e
    }

    /// clang 19's keyword correction of the undeclared template-id
    /// `name<...>` at `loc` (`ActOnIdExpression`'s `KeywordReplacement`):
    /// when the nearest candidates ([`Sema::template_id_typo_candidates`])
    /// are keywords only, "use of undeclared identifier 'x'; did you mean
    /// 'kw'?" is issued at once and the keyword returned; the parser then
    /// reads the keyword where the template-id was (`breakk<int>` is
    /// `break`: "expected unqualified-id").
    pub fn template_id_keyword_typo(&mut self, name: Symbol, loc: Loc) -> Option<&'static str> {
        if !self.inst.stack.is_empty() {
            return None;
        }
        let cands = self.template_id_typo_candidates(name);
        let first = cands.first()?;
        if cands.iter().take_while(|c| c.ed == first.ed).any(|c| !c.decls.is_empty()) {
            return None;
        }
        let kw = self.typo_keywords().into_iter().find(|&k| k == first.text)?;
        let msg = self.diag(DiagId::sema_undeclared_var_use_suggest, loc).arg(sym_str(self, name));
        let msg = self.typo_fixes(msg.arg(kw), loc, kw, true);
        self.emit(msg);
        Some(kw)
    }

    /// clang 19's correction of the undeclared template-id `name<args>`
    /// (`CorrectTypoDelayed`; the template arguments are dropped): the
    /// candidates by distance ([`Sema::template_id_typo_candidates`]),
    /// each rebuilt into the expression at the correction point
    /// (`TransformTypos`); the first valid one is the correction unless
    /// another at its distance is valid too (no correction). Keywords,
    /// types, templates other than function templates and non-static
    /// member functions never rebuild into a valid expression; a value
    /// (variable, parameter, enumerator, static data member, a field
    /// under an implicit `this`) is assumed valid, a function is valid
    /// where a function designator is. kiln goes on with one candidate
    /// and decides at the correction point ([`TypoSpec`]): the one
    /// value of the nearest distance, or -- when that distance has
    /// functions -- its first function, valid making a tie with the value
    /// (or another function) ambiguous, invalid passing the correction to
    /// the value (or to the one value of the next distance with any).
    /// Only inside a delayed-typo scope outside an instantiation
    /// (otherwise there is no correction point to decide at).
    fn speculate_template_id_typo(&mut self, name: Symbol, loc: Loc) -> Option<Speculation> {
        if self.typo_delay == 0 || !self.inst.stack.is_empty() {
            return None;
        }
        let mut groups: Vec<(usize, Vec<Candidate>, Vec<Candidate>)> = Vec::new();
        for c in self.template_id_typo_candidates(name) {
            let kind = self.typo_kind_19(&c);
            if groups.last().is_none_or(|g| g.0 != c.ed) {
                groups.push((c.ed, Vec::new(), Vec::new()));
            }
            let Some(g) = groups.last_mut() else { continue };
            if g.1.iter().chain(&g.2).any(|o| self.same_typo_entity(o, &c)) {
                continue;
            }
            match kind {
                Some(true) => g.1.push(c),
                Some(false) => g.2.push(c),
                None => {}
            }
        }
        let at = groups.iter().position(|g| !g.1.is_empty() || !g.2.is_empty())?;
        let (_, values, funcs) = groups.swap_remove(at);
        let rest = groups.into_iter().skip(at);
        let lone_func = funcs.len() == 1;
        let (probe, ambiguous, fallback) = match (values.len(), funcs.len()) {
            (1, 0) => (values.into_iter().next()?, false, None),
            (1, _) => (funcs.into_iter().next()?, true, values.into_iter().next()),
            (0, n) => {
                // Past the functions, the next distance with a value.
                let next = rest.map(|g| g.1).find(|v| !v.is_empty());
                let fallback = next.filter(|v| v.len() == 1).and_then(|v| v.into_iter().next());
                (funcs.into_iter().next()?, n > 1, fallback)
            }
            _ => return None,
        };
        let typo = sym_str(self, name);
        let suggest = self.template_id_typo_suggest(&typo, &probe, loc);
        // A lone function tied with a value: an initializer the value
        // cannot initialize leaves the function the one valid candidate
        // ([`Sema::settle_typo_tie`]).
        let tie = fallback
            .as_ref()
            .filter(|_| ambiguous && lone_func)
            .and_then(|c| self.typo_value_type(c));
        let fallback = fallback.map(|c| self.template_id_typo_suggest(&typo, &c, loc));
        let expr = self.template_id_typo_expr(&probe, loc)?;
        Some(Speculation { expr, suggest, fallback, ambiguous, tie })
    }

    /// What candidate `c` rebuilds into for clang 19
    /// ([`Sema::speculate_template_id_typo`]): `Some(true)` a value,
    /// `Some(false)` a function, `None` never a valid expression.
    fn typo_kind_19(&self, c: &Candidate) -> Option<bool> {
        if c.decls.is_empty() {
            return None;
        }
        let d = self.ast.strip_shadow(c.decls[self.typo_correction_decl(c)]);
        match self.kind_of(d) {
            DeclKind::Var
            | DeclKind::ParmVar
            | DeclKind::EnumConstant
            | DeclKind::Binding
            | DeclKind::NonTypeTemplateParm => Some(true),
            DeclKind::Field | DeclKind::IndirectField => {
                self.implicit_this_available().then_some(true)
            }
            DeclKind::Function | DeclKind::FunctionTemplate | DeclKind::CXXMethod => {
                let all_static = c.decls.iter().all(|&x| {
                    let x = self.ast.strip_shadow(x);
                    let m = match self.kind_of(x) {
                        DeclKind::FunctionTemplate => self.templated_function(x),
                        _ => Some(x),
                    };
                    m.is_none_or(|m| {
                        self.kind_of(m) != DeclKind::CXXMethod || self.is_static_method(m)
                    })
                });
                all_static.then_some(false)
            }
            _ => None,
        }
    }

    /// The type and category of the expression value candidate `c`
    /// rebuilds into ([`Sema::typo_kind_19`] `Some(true)`).
    fn typo_value_type(&self, c: &Candidate) -> Option<(QualType, ValueCategory)> {
        let d = self.ast.strip_shadow(*c.decls.get(self.typo_correction_decl(c))?);
        let ty = self.ast.decls.get(d).ty()?;
        let vk = match self.kind_of(d) {
            DeclKind::EnumConstant | DeclKind::NonTypeTemplateParm
                if !self.ast.types.is_reference(ty) =>
            {
                ValueCategory::PRValue
            }
            _ => ValueCategory::LValue,
        };
        Some((self.non_reference(ty), vk))
    }

    /// "use of undeclared identifier 'typo'; did you mean 'c'?" with
    /// "'c' declared here".
    fn template_id_typo_suggest(&self, typo: &str, c: &Candidate, loc: Loc) -> kiln_diag::Diag {
        let shown = c.found[self.typo_correction_decl(c)];
        let note = self
            .diag(DiagId::note_entity_declared_here, self.typo_note_loc(shown))
            .arg(format!("'{}'", c.text));
        let d = self
            .diag(DiagId::sema_undeclared_var_use_suggest, loc)
            .arg(typo.to_owned())
            .arg(c.text.clone())
            .with_note(note);
        self.typo_fixes(d, loc, &c.text, false)
    }

    /// A "did you mean" `d` for the name at `loc`: under clang 22 the
    /// name's range and, for an entity, the fix-it replacing it with
    /// `text`; under clang 19 the fix-it alone (WP-420).
    fn typo_fixes(
        &self,
        d: kiln_diag::Diag,
        loc: Loc,
        text: &str,
        keyword: bool,
    ) -> kiln_diag::Diag {
        let name = kiln_source::Span::new(loc, self.token_end(loc).unwrap_or(loc));
        let d = if keyword && self.clang_compat >= 22 {
            d
        } else {
            d.fixit(FixIt::replace(name, text))
        };
        if self.clang_compat >= 22 { d.range(name) } else { d }
    }

    /// The expression clang 19 rebuilds candidate `c` into: a reference
    /// to the one function or value (an implicit member access for a
    /// field), else an overload set of the functions, named as corrected.
    fn template_id_typo_expr(&mut self, c: &Candidate, loc: Loc) -> Option<ExprId> {
        let nns = self.typo_nns(c);
        if let &[decl] = c.decls.as_slice() {
            let d = self.ast.strip_shadow(decl);
            if self.kind_of(d) != DeclKind::FunctionTemplate {
                return Some(self.build_decl_ref_qualified(
                    d,
                    loc,
                    (decl != d).then_some(decl),
                    nns,
                ));
            }
        }
        let first = self.ast.strip_shadow(*c.decls.first()?);
        let dn = self.ast.decls.get(first).name();
        Some(self.builder().unresolved_lookup(dn, &c.decls, false, nns, None, loc))
    }

    /// clang 22's typo correction (`CorrectTypo` from `DiagnoseEmptyLookup`)
    /// of the template-id `name<args>` at `loc` that is not called and
    /// whose ordinary lookup found nothing, or only functions
    /// ([temp.names]/3; spec/sema.md "Undeclared template-ids"). The
    /// candidates are every identifier ordinary lookup finds within
    /// `(len+2)/3` character edits, the keywords clang offers here
    /// ([`Sema::typo_keywords`]), and -- for `name` itself and the near
    /// identifiers ordinary lookup does not find -- the same name
    /// qualified by any namespace or complete class that declares it
    /// ([`Sema::typo_scopes`]). Distances: 100 per character edit, 110 per
    /// qualifier component; the least wins, a tie with a different
    /// candidate means no correction, and so does a winner whose rounded
    /// distance exceeds a third of a name of three or more characters.
    /// What the winner is decides the outcome:
    /// - a function, function template or static member function:
    ///   "use of undeclared identifier 'x'; did you mean 'y'?" with
    ///   "'y' declared here", recovered as its overload set;
    /// - a non-static member function: the suggestion and clang's
    ///   "must be called" / "without an object argument";
    /// - a variable, parameter or enumerator: the suggestion and, outside
    ///   a template, clang's "is expected to be a non-type template"
    ///   pair; a data member or template parameter: the suggestion;
    /// - a type or keyword: the suggestion only ([`TemplateIdTypo::Suggested`]);
    /// - a namespace, concept or variable template: no correction.
    pub fn correct_template_id_typo(
        &mut self,
        name: Symbol,
        args: &[TemplateArg],
        loc: Loc,
    ) -> TemplateIdTypo {
        if let Some(r) = self.template_id_class_member(name, loc) {
            return r;
        }
        let Some(c) = self.best_template_id_typo(name) else {
            return TemplateIdTypo::Uncorrected;
        };
        let typo = sym_str(self, name);
        if c.decls.is_empty() {
            // A keyword: no note, no recovery.
            let msg = self.diag(DiagId::sema_undeclared_var_use_suggest, loc).arg(typo);
            let msg = self.typo_fixes(msg.arg(c.text.clone()), loc, &c.text, true);
            self.emit(msg);
            return TemplateIdTypo::Suggested;
        }
        let i = self.typo_correction_decl(&c);
        let (decl, shown) = (c.decls[i], c.found[i]);
        let d = self.ast.strip_shadow(decl);
        let pattern = |s: &Self| match s.kind_of(d) {
            DeclKind::FunctionTemplate => s.templated_function(d),
            _ => Some(d),
        };
        let kind = match self.kind_of(d) {
            DeclKind::Function | DeclKind::FunctionTemplate | DeclKind::CXXMethod => {
                let m = pattern(self).filter(|&m| {
                    self.kind_of(m) == DeclKind::CXXMethod && !self.is_static_method(m)
                });
                match m {
                    Some(m) => Correction::Member(m),
                    None => Correction::Function,
                }
            }
            DeclKind::Var | DeclKind::ParmVar | DeclKind::EnumConstant | DeclKind::Binding => {
                Correction::Value
            }
            DeclKind::Field | DeclKind::IndirectField | DeclKind::NonTypeTemplateParm => {
                Correction::Quiet
            }
            DeclKind::Typedef
            | DeclKind::TypeAlias
            | DeclKind::Enum
            | DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
            | DeclKind::ClassTemplate
            | DeclKind::TypeAliasTemplate
            | DeclKind::BuiltinTemplate
            | DeclKind::TemplateTypeParm
            | DeclKind::TemplateTemplateParm => Correction::Type,
            _ => return TemplateIdTypo::Uncorrected,
        };
        let note = self
            .diag(DiagId::note_entity_declared_here, self.typo_note_loc(shown))
            .arg(format!("'{}'", c.text));
        let msg = self
            .diag(DiagId::sema_undeclared_var_use_suggest, loc)
            .arg(typo)
            .arg(c.text.clone())
            .with_note(note);
        let msg = self.typo_fixes(msg, loc, &c.text, false);
        self.emit(msg);
        match kind {
            Correction::Type => return TemplateIdTypo::Suggested,
            Correction::Member(m) => self.typo_member_not_called(m, !c.qualified, loc),
            Correction::Value if !self.in_template() => {
                // clang re-forms the template-id on the variable and finds
                // no template kind for it.
                let note =
                    self.diag(DiagId::note_referenced_type_template, self.decl_loc(d)).arg(0u8);
                let msg = self
                    .diag(DiagId::sema_template_kw_refers_to_type_template, loc)
                    .arg(self.decl_name(d))
                    .arg(0u8)
                    .with_note(note);
                self.emit(msg);
            }
            Correction::Value | Correction::Quiet => {}
            Correction::Function => {
                if args.iter().any(|&a| Self::dependent_flags(self.ast.types.arg_flags(a))) {
                    return TemplateIdTypo::Recovered(self.recover_err(&[], loc));
                }
                // clang's recovery: an overload set of the one declaration
                // with the written template arguments, which a placeholder
                // context resolves to a non-template ignoring them
                // ([`Sema::resolve_single_overload_candidate`]) and a target
                // type does not ([`Sema::resolve_address_of_overloaded`]).
                let template = self.kind_of(d) == DeclKind::FunctionTemplate;
                let targs = Some(self.ast.types.arg_list(args));
                let dn = self.ast.decls.get(d).name();
                let nns = self.typo_nns(&c);
                let e = self.builder().unresolved_lookup(dn, &[decl], false, nns, targs, loc);
                return TemplateIdTypo::Recovered(if template {
                    self.resolve_single_template_id(e)
                } else {
                    e
                });
            }
        }
        TemplateIdTypo::Recovered(self.recover_err(&[], loc))
    }

    /// clang 22's `DiagnoseEmptyLookup` before typo correction of the
    /// template-id `name<...>` not called: `name` declared in a class
    /// enclosing the current context (a member function whose
    /// template-id ordinary lookup set aside) is diagnosed
    /// ([`Sema::diagnose_member_found_in_class`]) and recovered as a typo
    /// correction to those functions would be.
    fn template_id_class_member(&mut self, name: Symbol, loc: Loc) -> Option<TemplateIdTypo> {
        let decls = self.diagnose_member_found_in_class(DeclName::ident(name), loc, true)?;
        let member = decls
            .iter()
            .map(|&d| self.ast.strip_shadow(d))
            .find(|&d| self.kind_of(d) == DeclKind::CXXMethod && !self.is_static_method(d));
        if let Some(m) = member {
            self.typo_member_not_called(m, true, loc);
            return Some(TemplateIdTypo::Recovered(self.recover_err(&[], loc)));
        }
        let dn = self.ast.decls.get(self.ast.strip_shadow(decls[0])).name();
        Some(TemplateIdTypo::Recovered(
            self.builder().unresolved_lookup(dn, &decls, false, None, None, loc),
        ))
    }

    /// clang's `DiagnoseEmptyLookup` class walk and
    /// `DiagnoseDependentMemberLookup` for the unqualified `name` at `loc`
    /// whose lookup came up empty: the first class enclosing the current
    /// context that declares or inherits `name` gets "member 'x' used
    /// before its declaration" when it declares it, "explicit
    /// qualification required to use member 'x' from dependent base
    /// class" when it inherits it, with a "member is declared here" per
    /// declaration and, in a non-static member function of that class, a
    /// `this->` fix-it. `pick` (no explicit template arguments reached
    /// clang's overload check): of several declarations, the one function
    /// callable with no arguments alone is named, and as the lookup then
    /// has no naming class it counts as inherited. The declarations
    /// diagnosed; `None` (nothing diagnosed) when no class declares
    /// `name`.
    pub(crate) fn diagnose_member_found_in_class(
        &mut self,
        name: DeclName,
        loc: Loc,
        pick: bool,
    ) -> Option<Vec<DeclId>> {
        let mut dc = Some(self.ctx);
        let mut hit = None;
        for _ in 0..10_000 {
            let Some(d) = dc else { break };
            if self.kind_of(d) == DeclKind::CXXRecord {
                let found = self.lookup_qualified_dn(d, name, LookupKind::Ordinary);
                if !found.decls.is_empty() {
                    hit = Some((d, found.decls));
                    break;
                }
            }
            dc = self.ast.decls.get(d).ctx();
        }
        let (class, mut decls) = hit?;
        let callable: Vec<DeclId> = decls
            .iter()
            .copied()
            .filter(|&d| {
                let d = self.ast.strip_shadow(d);
                matches!(self.kind_of(d), DeclKind::Function | DeclKind::CXXMethod)
                    && self.fn_min_args(d) == 0
            })
            .collect();
        let picked = pick && callable.len() == 1;
        if picked {
            decls = callable;
        }
        let class = self.ast.first(class);
        let first = self.ast.strip_shadow(decls[0]);
        let own =
            !picked && self.ast.decls.get(first).ctx().is_some_and(|p| self.ast.first(p) == class);
        let method = self.ast.decls.get(self.ctx).ctx().map(|p| self.ast.first(p));
        let instance = !picked
            && self.func.as_ref().is_some_and(|f| f.this_ty.is_some())
            && self.kind_of(self.ctx) == DeclKind::CXXMethod
            && method == Some(class);
        let id = if own {
            DiagId::sema_found_later_in_class
        } else {
            DiagId::sema_found_in_dependent_base
        };
        let mut msg = self.diag(id, loc).arg(self.decl_name_str(name));
        if instance {
            msg = msg.fixit(FixIt::insert(loc, "this->"));
        }
        for &d in &decls {
            let at = self.decl_loc(self.ast.strip_shadow(d));
            msg = msg.with_note(self.diag(DiagId::note_member_declared_at, at));
        }
        self.emit(msg);
        Some(decls)
    }

    /// Whether an unqualified member name here can be an implicit member
    /// access: in a non-static member function, or outside any function
    /// in a default member initializer.
    pub(crate) fn implicit_this_available(&self) -> bool {
        match &self.func {
            Some(f) => f.this_ty.is_some(),
            None => !self.nsdmi_this.is_empty(),
        }
    }

    /// Where clang's "declared here" note for `d` points: its name, or
    /// for an alias template its `using` keyword.
    pub(crate) fn typo_note_loc(&self, d: DeclId) -> Loc {
        let first = self.ast.first(self.ast.strip_shadow(d));
        (self.kind_of(first) == DeclKind::TypeAliasTemplate)
            .then(|| self.ast.template_info(first).and_then(|i| i.pattern))
            .flatten()
            .and_then(|p| self.templ.alias_using_loc.get(&p).copied())
            .unwrap_or_else(|| self.decl_loc(d))
    }

    /// clang's recovery of a non-static member function `m` named
    /// without an object: "reference to non-static member function must
    /// be called" in a non-static member function (`implicit`: the name
    /// was unqualified), else "call to non-static member function without
    /// an object argument".
    fn typo_member_not_called(&mut self, m: DeclId, implicit: bool, loc: Loc) {
        let in_method = implicit && self.implicit_this_available();
        let msg = if in_method {
            let no_args =
                self.ast.decls.function(m).is_some_and(|f| self.ast.lists.get(f.params).is_empty());
            self.diag(DiagId::sema_bound_member_not_called, loc).arg(u8::from(no_args))
        } else {
            self.diag(DiagId::sema_member_call_without_object, loc)
        };
        self.emit(msg);
    }

    /// The candidate clang's `CorrectTypo` settles on for `name` (see
    /// [`Sema::correct_template_id_typo`]); `None` when there is none,
    /// it is too distant, or it ties with another.
    fn best_template_id_typo(&self, name: Symbol) -> Option<Candidate> {
        let len = sym_str(self, name).len();
        let cands = self.template_id_typo_candidates(name);
        let mut best: Option<Candidate> = None;
        for c in cands {
            match &best {
                None => best = Some(c),
                Some(b) if b.ed < c.ed => break,
                Some(b) => {
                    if self.same_typo_entity(b, &c) {
                        continue;
                    }
                    return None;
                }
            }
        }
        let best = best?;
        let ed = (best.ed + 50) / 100;
        (len < 3 || ed == 0 || len / ed >= 3).then_some(best)
    }

    /// Whether candidates `a` and `b` name the same entity (the same
    /// entity at the same distance counts once, as the lesser spelling).
    fn same_typo_entity(&self, a: &Candidate, b: &Candidate) -> bool {
        let key =
            |c: &Candidate| c.decls.iter().map(|&d| self.ast.first(self.ast.strip_shadow(d))).min();
        !a.decls.is_empty() && key(a) == key(b)
    }

    /// Every typo-correction candidate for `name`, by distance and then
    /// spelling (clang's `TypoCorrectionConsumer` before it picks; see
    /// [`Sema::correct_template_id_typo`] for what is a candidate).
    fn template_id_typo_candidates(&self, name: Symbol) -> Vec<Candidate> {
        let typo = sym_str(self, name);
        let len = typo.len();
        let bound = len.div_ceil(3);
        // `TypoCorrectionConsumer::addName`: the length difference and
        // the edit distance bound the candidates.
        let near = |s: &str| {
            let diff = s.len().abs_diff(len);
            if diff > 0 && len / diff < 3 {
                return None;
            }
            let ed = crate::local_friend::edit_distance(&typo, s);
            (ed <= bound).then_some(ed)
        };
        let mut cands: Vec<Candidate> = Vec::new();
        for k in self.typo_keywords() {
            if let Some(ed) = near(k).filter(|&ed| ed > 0) {
                cands.push(Candidate {
                    ed: 100 * ed,
                    text: k.to_owned(),
                    decls: Vec::new(),
                    found: Vec::new(),
                    qualified: false,
                    scope: None,
                });
            }
        }
        // Every identifier seen (clang walks its identifier table, kiln
        // its interner): visible ones are candidates as written, the
        // others (and `name` itself) are looked for in every scope.
        let mut unfound: Vec<(Symbol, usize)> = vec![(name, 0)];
        let syms = self.ast.symbols();
        let n = u32::try_from(syms.len()).unwrap_or(u32::MAX);
        for s in (0..n).map(Symbol::from_index) {
            if s == name {
                continue;
            }
            let Some(ed) = near(syms.as_str(s)) else { continue };
            let found = self.lookup_name(s, LookupKind::Ordinary);
            if found.decls.is_empty() || found.ambiguity.is_some() {
                unfound.push((s, ed));
                continue;
            }
            cands.push(Candidate {
                ed: 100 * ed,
                text: syms.as_str(s).to_owned(),
                decls: found.decls,
                found: found.found,
                qualified: false,
                scope: None,
            });
        }
        let cur = self.typo_context_chain();
        for scope in self.typo_scopes() {
            let record = self.kind_of(scope) != DeclKind::Namespace
                && self.kind_of(scope) != DeclKind::TranslationUnit;
            for &(s, ed) in &unfound {
                if record && self.ast.decls.get(scope).name().as_ident() == Some(s) {
                    continue;
                }
                let found = self.lookup_qualified(scope, s, LookupKind::Ordinary);
                if found.decls.is_empty() || found.ambiguity.is_some() {
                    continue;
                }
                // A data member of another class is no correction.
                if found
                    .decls
                    .iter()
                    .any(|&d| matches!(self.kind_of(d), DeclKind::Field | DeclKind::IndirectField))
                {
                    continue;
                }
                let (qual, q) = self.typo_qualifier(scope, &cur);
                cands.push(Candidate {
                    ed: 100 * ed + 110 * q,
                    text: format!("{qual}{}", syms.as_str(s)),
                    decls: found.decls,
                    found: found.found,
                    qualified: true,
                    scope: Some(scope),
                });
            }
        }
        // `addCorrection`: a typo shorter than three characters is only
        // corrected to itself, qualified, within its length.
        if len < 3 {
            cands.retain(|c| c.text.ends_with(&*typo) && c.qualified && (c.ed + 50) / 100 <= len);
        }
        cands.sort_by(|a, b| a.ed.cmp(&b.ed).then_with(|| a.text.cmp(&b.text)));
        cands
    }

    /// The declaration of correction `c` clang names in the note and
    /// recovers with: of an overload set, the one function callable with
    /// no arguments, else the first found (by qualified lookup, the first
    /// declared; by unqualified lookup, the last).
    fn typo_correction_decl(&self, c: &Candidate) -> usize {
        let order = |i: &usize| self.ast.first(c.decls[*i]);
        if c.decls.len() > 1 {
            let mut viable = (0..c.decls.len()).filter(|&i| {
                let d = self.ast.strip_shadow(c.decls[i]);
                matches!(self.kind_of(d), DeclKind::Function | DeclKind::CXXMethod)
                    && self.fn_min_args(d) == 0
            });
            if let (Some(i), None) = (viable.next(), viable.next()) {
                return i;
            }
        }
        let all = 0..c.decls.len();
        if c.qualified { all.min_by_key(order) } else { all.max_by_key(order) }.unwrap_or(0)
    }

    /// The keywords clang's typo correction offers in an expression
    /// (`AddKeywordsToConsumer`, every `Want*` flag, C++ mode): type and
    /// storage specifiers, the named casts, expression keywords (`this`
    /// in a non-static member function); in a function the statement
    /// keywords (`break` / `continue` in a loop or switch / loop, `case`
    /// and `default` in a switch), elsewhere `namespace` and `template`
    /// (and the member specifiers at class scope); `using` and
    /// `static_assert` always.
    fn typo_keywords(&self) -> Vec<&'static str> {
        let mut k = vec![
            "char",
            "const",
            "double",
            "enum",
            "float",
            "int",
            "long",
            "short",
            "signed",
            "struct",
            "union",
            "unsigned",
            "void",
            "volatile",
            "_Complex",
            "_Imaginary",
            "extern",
            "inline",
            "static",
            "typedef",
            "bool",
            "class",
            "typename",
            "wchar_t",
            "char16_t",
            "char32_t",
            "constexpr",
            "decltype",
            "thread_local",
            "const_cast",
            "dynamic_cast",
            "reinterpret_cast",
            "static_cast",
            "sizeof",
            "false",
            "true",
            "delete",
            "new",
            "operator",
            "throw",
            "typeid",
            "alignof",
            "nullptr",
        ];
        if let Some(f) = self.func.as_ref() {
            if f.this_ty.is_some() {
                k.push("this");
            }
            k.extend(["do", "else", "for", "goto", "if", "return", "switch", "while"]);
            k.extend(["catch", "try"]);
            if !f.breakables.is_empty() {
                k.push("break");
            }
            if f.breakables.contains(&true) {
                k.push("continue");
            }
            if !f.switches.is_empty() {
                k.extend(["case", "default"]);
            }
        } else {
            k.extend(["namespace", "template"]);
            if matches!(
                self.kind_of(self.ctx),
                DeclKind::CXXRecord
                    | DeclKind::ClassTemplateSpecialization
                    | DeclKind::ClassTemplatePartialSpecialization
            ) {
                k.extend(["explicit", "friend", "mutable", "private", "protected"]);
                k.extend(["public", "virtual"]);
            }
        }
        k.extend(["using", "static_assert"]);
        k
    }

    /// The scopes clang's typo correction looks a name up in for a
    /// qualified correction: every namespace, and every complete named
    /// class that is neither a union nor a template or member of one
    /// (`TypoCorrectionConsumer::addNamespaces`); nested ones included.
    fn typo_scopes(&self) -> Vec<DeclId> {
        let mut root = self.ctx;
        while let Some(p) = self.ast.decls.get(root).ctx() {
            root = p;
        }
        let mut out = vec![root];
        let mut stack = vec![root];
        while let Some(c) = stack.pop() {
            for m in self.ast.members(c) {
                match self.kind_of(m) {
                    DeclKind::Namespace => {
                        let f = self.ast.first(m);
                        if !out.contains(&f) {
                            out.push(f);
                        }
                        stack.push(m);
                    }
                    DeclKind::LinkageSpec | DeclKind::Export => stack.push(m),
                    DeclKind::CXXRecord
                        if self.ast.definition(m) == Some(m)
                            && self.ast.decls.get(m).name().as_ident().is_some()
                            && !self.ast.decls.get(m).has(DeclFlags::DEPENDENT_CTX)
                            && self.ast.described_template(m).is_none()
                            && !self.is_union(m) =>
                    {
                        out.push(m);
                        stack.push(m);
                    }
                    _ => {}
                }
            }
        }
        out
    }

    /// The named contexts enclosing the current one, outermost first
    /// (clang's `buildContextChain`: inline and unnamed namespaces and
    /// transparent contexts left out).
    fn typo_context_chain(&self) -> Vec<DeclId> {
        self.typo_chain(self.ctx)
    }

    fn typo_chain(&self, mut d: DeclId) -> Vec<DeclId> {
        let mut chain = Vec::new();
        for _ in 0..10_000 {
            let keep = match self.kind_of(d) {
                DeclKind::Namespace => self.ast.decls.namespace(d).is_some_and(|n| {
                    n.flags & (NamespaceData::INLINE | NamespaceData::ANONYMOUS) == 0
                }),
                DeclKind::TranslationUnit
                | DeclKind::LinkageSpec
                | DeclKind::Export
                | DeclKind::Enum => false,
                _ => self.ast.decls.get(d).name().as_ident().is_some(),
            };
            if keep {
                chain.push(self.ast.first(d));
            }
            match self.ast.decls.get(d).ctx() {
                Some(p) => d = p,
                None => break,
            }
        }
        chain.reverse();
        chain
    }

    /// The qualifier clang's typo correction spells a member of `scope`
    /// with from the contexts `cur`, and its component count
    /// (`NamespaceSpecifierSet::addNameSpecifier`): `::` for the
    /// translation unit (one component); the path from the innermost
    /// common context, or `::` and the full path when that is empty or
    /// starts with a name of an enclosing context.
    /// The nested-name-specifier of correction `c` as spelled (`::val`,
    /// `N::fn`): its namespaces, from the global one when the spelling
    /// starts with `::`. `None` when it is unqualified, or names a class
    /// (kiln then keeps the bare name).
    fn typo_nns(&mut self, c: &Candidate) -> Option<kiln_ast::ids::NnsId> {
        use kiln_ast::types::Nns;
        let scope = c.scope?;
        let chain = self.typo_chain(scope);
        if chain.iter().any(|&n| self.kind_of(n) != DeclKind::Namespace) {
            return None;
        }
        let parts = c.text.split("::").count().checked_sub(1)?;
        let global = c.text.starts_with("::");
        let take = if global { chain.len() } else { parts.min(chain.len()) };
        let mut nns = global.then(|| self.ast.types.nns(Nns::Global));
        for &ns in &chain[chain.len() - take..] {
            nns = Some(self.ast.types.nns(Nns::Namespace { prefix: nns, ns }));
        }
        let n = nns?;
        let name = c.text.rsplit("::").next().unwrap_or_default();
        (format!("{}{name}", self.nns_str(n)) == c.text).then_some(n)
    }

    fn typo_qualifier(&self, scope: DeclId, cur: &[DeclId]) -> (String, usize) {
        let chain = self.typo_chain(scope);
        if chain.is_empty() {
            return ("::".to_owned(), 1);
        }
        let common = chain.iter().zip(cur).take_while(|(a, b)| a == b).count();
        let spell = |part: &[DeclId]| {
            part.iter().fold(String::new(), |mut s, &n| {
                s.push_str(&self.decl_name(n));
                s.push_str("::");
                s
            })
        };
        let rel = &chain[common..];
        let global = rel.first().is_none_or(|&r| {
            let n = self.ast.decls.get(r).name();
            cur.iter().any(|&c| self.ast.decls.get(c).name() == n)
        });
        if global { (format!("::{}", spell(&chain)), chain.len()) } else { (spell(rel), rel.len()) }
    }
}
