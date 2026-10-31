//! clang's `DefaultedComparisonAnalyzer`: whether a defaulted comparison
//! is deleted, whether it is `constexpr`, and the common comparison
//! category of a defaulted `operator<=>` ([class.compare.default]/2,
//! [class.eq]/2, [class.spaceship]/1–2, [class.compare.secondary]/2), with
//! the notes explaining a deletion.

use kiln_ast::class::CmpCategory;
use kiln_ast::decl::{ConstexprKind, DefinitionBits, FieldData, TagKind};
use kiln_ast::expr::ValueCategory;
use kiln_ast::names::OverloadedOperator as OO;
use kiln_ast::types::Quals;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use super::CmpKind;
use crate::Sema;
use crate::overload::ics::Ics;
use crate::overload::oper::{OperForm, binary_of};
use crate::overload::{CandidateKind, OverloadCandidateSet, OvlOutcome, Rewrite};

/// The result of analyzing a defaulted comparison (clang
/// `DefaultedComparisonAnalyzer::Result`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct CmpAnalysis {
    /// Defined as deleted.
    pub deleted: bool,
    /// Every comparison it performs calls a `constexpr` function (or a
    /// built-in operator).
    pub constexpr: bool,
    /// The common comparison category (a defaulted `<=>` with return type
    /// `auto`).
    pub category: CmpCategory,
}

impl CmpAnalysis {
    const OK: CmpAnalysis =
        CmpAnalysis { deleted: false, constexpr: true, category: CmpCategory::Strong };
    const DELETED: CmpAnalysis =
        CmpAnalysis { deleted: true, constexpr: true, category: CmpCategory::Strong };

    /// Merges `r` in; true once deleted (clang `Result::add`).
    fn add(&mut self, r: CmpAnalysis) -> bool {
        self.deleted |= r.deleted;
        self.constexpr &= r.constexpr;
        self.category = self.category.min(r.category);
        self.deleted
    }
}

/// A subobject compared (clang `DefaultedComparisonSubobject`): the
/// `{0:|member |base class }` select, its diagnostic argument and
/// location.
#[derive(Clone)]
struct Subobj {
    kind: u8,
    decl: String,
    loc: Loc,
}

/// The state of one analysis.
struct Cx {
    f: DeclId,
    k: CmpKind,
    rd: DeclId,
    from: DeclId,
    diagnose: bool,
    notes: Vec<Diag>,
    floc: Loc,
    /// The name of `f` (`operator==`), unquoted.
    fname: String,
}

impl Sema<'_, '_> {
    /// Analyzes defaulted comparison `f` (kind `k`) of class `rd`, its
    /// subobject comparisons resolved as in context `from` (for access).
    /// With `diagnose`, the notes explaining a deletion.
    pub(crate) fn analyze_cmp(
        &mut self,
        f: DeclId,
        k: CmpKind,
        rd: DeclId,
        from: DeclId,
        diagnose: bool,
    ) -> (CmpAnalysis, Vec<Diag>) {
        let first = self.ast.first(f);
        let saved = self.special.cmp.exclude.replace((first, self.ctx));
        let floc = self.ast.decls.get(f).loc();
        let fname = self.decl_name(f);
        let mut cx = Cx { f, k, rd, from, diagnose, notes: Vec::new(), floc, fname };
        let r = self.analyze_top(&mut cx);
        self.special.cmp.exclude = saved;
        (r, cx.notes)
    }

    fn analyze_top(&mut self, cx: &mut Cx) -> CmpAnalysis {
        let rd = cx.rd;
        let is_union = self.ast.decls.record(rd).is_some_and(|x| x.tag == TagKind::Union);
        if cx.k.is_primary()
            && (is_union || self.defn_bits(rd).contains(DefinitionBits::HAS_VARIANT_MEMBERS))
        {
            if cx.diagnose {
                let d = self
                    .diag(DiagId::note_defaulted_cmp_union, cx.floc)
                    .arg(&cx.fname)
                    .arg(u8::from(is_union))
                    .arg(self.decl_arg(rd));
                cx.notes.push(d);
            }
            return CmpAnalysis::DELETED;
        }
        let params = self.param_decls(cx.f);
        let Some(&last) = params.last() else { return CmpAnalysis::DELETED };
        let pty = self.ast.decls.get(last).ty().unwrap_or(QualType::ERROR);
        let plval = self.non_reference(pty);
        let mut res = CmpAnalysis::OK;
        if cx.k.is_primary() {
            let quals = self.ast.types.effective_quals(plval);
            self.visit_subobjects(cx, &mut res, rd, quals);
        } else {
            let sub = Subobj { kind: 0, decl: self.decl_arg(rd), loc: cx.floc };
            let r = self.visit_expanded(cx, plval, &sub);
            let _ = res.add(r);
        }
        res
    }

    /// clang `visitSubobjects`: the direct bases in declaration order, then
    /// the non-static data members ([class.compare.default]/5);
    /// true once deleted.
    fn visit_subobjects(
        &mut self,
        cx: &mut Cx,
        res: &mut CmpAnalysis,
        record: DeclId,
        quals: Quals,
    ) -> bool {
        let bases: Vec<(QualType, Loc)> =
            self.ast.decls.bases(record, &self.ast.lists).map(|b| (b.ty, b.loc)).collect();
        for (bty, bloc) in bases {
            let t = self.ast.types.qualify(bty, quals);
            let decl = match self.ast.types.as_record(bty) {
                Some(b) => self.decl_arg(b),
                None => self.ty_arg(bty),
            };
            let sub = Subobj { kind: 2, decl, loc: bloc };
            let r = self.visit_subobject(cx, t, &sub);
            if res.add(r) {
                return true;
            }
        }
        let fields: Vec<DeclId> = self
            .ast
            .decls
            .members(record)
            .filter(|&m| self.ast.decls.get(m).kind() == kiln_ast::decl::DeclKind::Field)
            .collect();
        for fd in fields {
            let data = self.ast.decls.field(fd).copied();
            let n = self.ast.decls.get(fd);
            let fty = n.ty().unwrap_or(QualType::ERROR);
            if data.is_some_and(|x| x.bit_width.is_some()) && n.name().is_empty() {
                continue;
            }
            if let Some(anon) = self.ast.types.as_record(fty)
                && self.defn_bits(anon).contains(DefinitionBits::ANONYMOUS)
            {
                let anon = self.class_def(anon);
                if self.visit_subobjects(cx, res, anon, quals) {
                    return true;
                }
                continue;
            }
            let mut q = quals;
            if data.is_some_and(|x| x.flags & FieldData::MUTABLE != 0) {
                q = Quals::from_bits(q.bits() & !Quals::CONST.bits());
            }
            let t = self.ast.types.qualify(fty, q);
            let sub = Subobj { kind: 1, decl: self.decl_arg(fd), loc: n.loc() };
            let r = self.visit_subobject(cx, t, &sub);
            if res.add(r) {
                return true;
            }
        }
        false
    }

    /// clang `visitSubobject`: an array is expanded to its elements
    /// ([class.compare.default]/6).
    fn visit_subobject(&mut self, cx: &mut Cx, t: QualType, sub: &Subobj) -> CmpAnalysis {
        let mut t = t;
        for _ in 0..64 {
            if self.ast.types.array_size(t).is_none() {
                break;
            }
            let q = self.ast.types.effective_quals(t);
            let Some(e) = self.ast.types.element(t) else { break };
            t = self.ast.types.qualify(e, q);
        }
        self.visit_expanded(cx, t, sub)
    }

    /// clang `visitExpandedSubobject`: `xi @ xi` for an lvalue `xi` of
    /// type `t` ([class.compare.default]/2: a reference member deletes).
    fn visit_expanded(&mut self, cx: &mut Cx, t: QualType, sub: &Subobj) -> CmpAnalysis {
        if self.ast.types.is_reference(t) {
            if cx.diagnose {
                let d = self
                    .diag(DiagId::note_defaulted_cmp_ref_member, sub.loc)
                    .arg(&cx.fname)
                    .arg(self.decl_arg(cx.rd));
                cx.notes.push(d);
            }
            return CmpAnalysis::DELETED;
        }
        let xi = self.builder().opaque_of(t, ValueCategory::LValue, cx.floc);
        let Some(op) = self.fn_operator(cx.f) else { return CmpAnalysis::DELETED };
        self.visit_binary(cx, op, xi, sub, None)
    }

    /// clang `visitBinaryOperator`: resolves `xi op xi` (without
    /// rewritten candidates when synthesizing `<=>` from `==` and `<`,
    /// whose `<=>` candidate set is `ss`).
    #[allow(clippy::too_many_lines, reason = "each overload resolution outcome, as clang")]
    fn visit_binary(
        &mut self,
        cx: &mut Cx,
        op: OO,
        xi: ExprId,
        sub: &Subobj,
        ss: Option<&OverloadCandidateSet>,
    ) -> CmpAnalysis {
        let Some(bop) = binary_of(op) else { return CmpAnalysis::DELETED };
        let t = self.ety(xi);
        let arg_class = self.ast.types.as_record(t);
        let overloadable = arg_class.is_some() || self.ast.types.as_enum(t).is_some();
        let (mut set, out, _, _) = self.oper_set(
            OperForm::Binary(bop),
            op,
            &[xi, xi],
            &[None, None],
            2,
            cx.floc,
            ss.is_none(),
            overloadable,
        );
        let secondary = !cx.k.is_primary();
        let mut r = CmpAnalysis::OK;
        if set.uncertain || set.skipped_templates {
            // todo(WP-071/073): undecided candidates; assumed usable.
            self.end_set(set);
            return r;
        }
        match out {
            OvlOutcome::Success(c) => {
                let cand = set.cand(c).clone();
                if secondary && cand.rewrite == Rewrite::None {
                    if cx.diagnose {
                        if let Some(fd) = cand.decl {
                            let d = self
                                .diag(
                                    DiagId::note_defaulted_cmp_not_rewritten_callee,
                                    self.ast.decls.get(fd).loc(),
                                )
                                .arg(&cx.fname);
                            cx.notes.push(d);
                        } else if let Some(Ics::UserDefined(u)) = set.ics_of(c).first()
                            && let Some(conv) = u.found.or(u.func)
                        {
                            let d = self
                                .diag(
                                    DiagId::note_defaulted_cmp_not_rewritten_conversion,
                                    self.ast.decls.get(conv).loc(),
                                )
                                .arg(&cx.fname);
                            cx.notes.push(d);
                        }
                    }
                    self.end_set(set);
                    return CmpAnalysis::DELETED;
                }
                if let Some(ac) = arg_class
                    && cand.kind != CandidateKind::Builtin
                    && self.declaring_class(cand.found).is_some()
                {
                    let object = if sub.kind == 1 { ac } else { cx.rd };
                    let callee_name = cand.decl.map(|d| self.decl_name(d)).unwrap_or_default();
                    let fname = cx.fname.clone();
                    let (kind, decl) = (sub.kind, sub.decl.clone());
                    let loc = sub.loc;
                    // clang appends the declaring class of the callee.
                    let owner = cand
                        .decl
                        .and_then(|d| self.declaring_class(d))
                        .or_else(|| self.declaring_class(cand.found))
                        .unwrap_or(ac);
                    let ac_arg = self.decl_arg(owner);
                    if let Some(d) =
                        self.cmp_member_access(cand.found, ac, object, cx.from, |s, prot| {
                            s.diag(DiagId::note_defaulted_cmp_inaccessible, loc)
                                .arg(&fname)
                                .arg(u8::from(prot))
                                .arg(&callee_name)
                                .arg(kind)
                                .arg(&ac_arg)
                                .arg(&decl)
                        })
                    {
                        if cx.diagnose {
                            cx.notes.push(d);
                        }
                        self.end_set(set);
                        return CmpAnalysis::DELETED;
                    }
                }
                let deducing =
                    op == OO::Spaceship && self.contains_deduced(self.declared_ret(cx.f));
                if let Some(fd) = cand.decl {
                    r.constexpr &= self.fn_spec(fd).constexpr_kind() != ConstexprKind::None;
                    if deducing {
                        let mut ret = self.fn_ret(fd);
                        if self.contains_deduced(ret) && self.ast.instantiated_from(fd).is_some() {
                            self.instantiate_function_definition(fd, cx.floc);
                            ret = self.fn_ret(fd);
                        }
                        if self.contains_deduced(ret) {
                            // todo: clang's `cannot_deduce_undeduced_auto`
                            // error.
                            self.end_set(set);
                            return CmpAnalysis::DELETED;
                        }
                        let rt = self.non_reference(ret);
                        match self.cmp_category_of(rt) {
                            Some(cat) => r.category = cat,
                            None => {
                                if cx.diagnose {
                                    let shown = self.ast.types.unqualified(rt);
                                    let d = self
                                        .diag(DiagId::note_defaulted_cmp_cannot_deduce, sub.loc)
                                        .arg(self.ty_arg(shown))
                                        .arg(sub.kind)
                                        .arg(&sub.decl);
                                    cx.notes.push(d);
                                    let d = self
                                        .diag(
                                            DiagId::note_defaulted_cmp_cannot_deduce_callee,
                                            self.ast.decls.get(fd).loc(),
                                        )
                                        .arg(sub.kind)
                                        .arg(&sub.decl);
                                    cx.notes.push(d);
                                }
                                self.end_set(set);
                                return CmpAnalysis::DELETED;
                            }
                        }
                    }
                } else if deducing {
                    let bt = self.non_reference(cand.builtin[0]);
                    let bt = self.ast.types.unqualified(bt);
                    if let Some(cat) = self.builtin_cmp_category(bt) {
                        r.category = cat;
                    }
                }
            }
            OvlOutcome::Ambiguous => {
                if cx.diagnose {
                    let kk: u8 = if cx.k == CmpKind::ThreeWay && op != OO::Spaceship {
                        if op == OO::EqualEqual { 1 } else { 2 }
                    } else {
                        0
                    };
                    let d = self
                        .diag(DiagId::note_defaulted_cmp_ambiguous, sub.loc)
                        .arg(&cx.fname)
                        .arg(kk)
                        .arg(sub.kind)
                        .arg(&sub.decl);
                    cx.notes.push(d);
                    // clang notes these candidates with no operator
                    // spelling and an invalid location.
                    set.builtin_op = None;
                    set.loc = Loc::INVALID;
                    let ns = self.ovl_notes(&set, true);
                    cx.notes.extend(ns);
                }
                r = CmpAnalysis::DELETED;
            }
            OvlOutcome::Deleted(c) => {
                if cx.diagnose {
                    let cand = set.cand(c).clone();
                    if secondary && cand.rewrite == Rewrite::None {
                        if let Some(fd) = cand.decl {
                            let d = self
                                .diag(
                                    DiagId::note_defaulted_cmp_not_rewritten_callee,
                                    self.ast.decls.get(fd).loc(),
                                )
                                .arg(&cx.fname);
                            cx.notes.push(d);
                        }
                    } else {
                        let d = self
                            .diag(DiagId::note_defaulted_cmp_calls_deleted, sub.loc)
                            .arg(&cx.fname)
                            .arg(sub.kind)
                            .arg(&sub.decl);
                        cx.notes.push(d);
                        if let Some(fd) = cand.decl {
                            let ns = self.deleted_function_notes(fd, 0);
                            cx.notes.extend(ns);
                        }
                    }
                }
                r = CmpAnalysis::DELETED;
            }
            OvlOutcome::NoViable => {
                let declared = self.declared_ret(cx.f);
                let declared = self.non_reference(declared);
                if op == OO::Spaceship && self.cmp_category_of(declared).is_some() {
                    let a = self.visit_binary(cx, OO::EqualEqual, xi, sub, Some(&set));
                    if !r.add(a) {
                        let b = self.visit_binary(cx, OO::Less, xi, sub, Some(&set));
                        let _ = r.add(b);
                    }
                } else {
                    if cx.diagnose {
                        let d = self
                            .diag(DiagId::note_defaulted_cmp_no_viable, sub.loc)
                            .arg(&cx.fname)
                            .arg(u8::from(matches!(op, OO::EqualEqual | OO::ExclaimEqual)))
                            .arg(sub.kind)
                            .arg(&sub.decl);
                        cx.notes.push(d);
                        if let Some(s) = ss {
                            let ns = self.ovl_notes(s, false);
                            cx.notes.extend(ns);
                            let d = self
                                .diag(DiagId::note_defaulted_cmp_no_viable_synth, sub.loc)
                                .arg(u8::from(op != OO::EqualEqual));
                            cx.notes.push(d);
                        }
                        let ns = self.ovl_notes(&set, false);
                        cx.notes.extend(ns);
                    }
                    r = CmpAnalysis::DELETED;
                }
            }
        }
        self.end_set(set);
        r
    }

    /// The declared (or deduced) return type of function `f`.
    pub(crate) fn fn_ret(&self, f: DeclId) -> QualType {
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        self.sugared_proto(ty).map_or(QualType::ERROR, |p| p.ret())
    }
}
