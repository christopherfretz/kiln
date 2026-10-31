//! Pack expansion (spec/sema-templates.md §7): the expansion of a
//! pattern into its elements ([temp.variadic]/5–8), the length check,
//! and the per-list helpers the substituter's expansion sites call.
//!
//! The packs of a pattern and their lengths are found by a *probe*: the
//! pattern is substituted with no pack index; the first pack argument (or
//! instantiated function parameter pack) the substitution reaches records
//! its length and name and stops the probe. Each element is then
//! substituted with its index; a pack of another length reached while
//! expanding is the length-mismatch error. A probe that reaches no pack
//! (every pack belongs to a retained level) yields the substituted
//! pattern, still an expansion.

use kiln_ast::ExprId;
use kiln_ast::QualType;
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::RawId;
use kiln_ast::types::{ArgView, TemplateArg, TypeView};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::templ::subst::{SubstFail, SubstResult, Substituter};

/// The state of the innermost expansion in progress.
#[derive(Clone, Debug, Default)]
pub struct PackState {
    /// Probing for the length: the first pack reached stops the probe.
    pub probing: bool,
    /// What the probe found: the length and the pack's name.
    pub found: Option<(u32, String)>,
    /// The length of the expansion and the first pack's name.
    pub expanding: Option<(u32, String)>,
    /// The `...` of the expansion, where a length mismatch is reported.
    pub ellipsis: Option<Loc>,
}

/// The result of expanding one pattern.
#[derive(Debug)]
pub enum Expansion<T> {
    /// The elements, in order.
    Elements(Vec<T>),
    /// The pattern substituted but not expandable yet (its packs belong
    /// to a retained level).
    Unexpanded(T),
}

/// The pattern of a pack-expansion type.
pub(crate) fn type_pattern(s: &Sema<'_, '_>, q: QualType) -> Option<QualType> {
    match s.ast.types.view(q.ty()) {
        TypeView::PackExpansion { pattern, .. } => Some(pattern),
        _ => None,
    }
}

/// The pattern of a `PackExpansionExpr`.
pub(crate) fn expr_pattern(s: &Sema<'_, '_>, e: ExprId) -> Option<ExprId> {
    let n = s.ast.exprs.get(e);
    (n.kind() == ExprKind::PackExpansionExpr).then(|| ExprId::new(n.a()))
}

impl Substituter<'_, '_, '_> {
    /// A pack of length `n` named `name` reached during substitution:
    /// the index of the element to use. Probing records the pack and
    /// stops; expanding checks the length ([temp.variadic]/8).
    pub(crate) fn pack_reached(
        &mut self,
        n: u32,
        name: impl FnOnce(&Self) -> String,
        loc: Loc,
    ) -> SubstResult<u32> {
        if self.pack.probing {
            self.pack.found = Some((n, name(self)));
            return Err(SubstFail);
        }
        let Some(i) = self.pack_index else {
            return self.unimplemented("pack expansion in this context", loc);
        };
        if let Some((len, first)) = self.pack.expanding.clone()
            && len != n
        {
            let second = name(self);
            let d = self
                .sema
                .diag(DiagId::sema_pack_length_mismatch, self.pack.ellipsis.unwrap_or(loc))
                .arg(first)
                .arg(second)
                .arg(len)
                .arg(n);
            self.sema.emit(d);
            return Err(SubstFail);
        }
        if i >= n {
            return Err(SubstFail);
        }
        Ok(i)
    }

    /// The element of pack argument `a` for the current index (`a`
    /// itself when it is not a pack).
    pub(crate) fn pack_element(
        &mut self,
        a: TemplateArg,
        name: impl FnOnce(&Self) -> String,
        loc: Loc,
    ) -> SubstResult<TemplateArg> {
        match a.view() {
            ArgView::Pack(l) => {
                let n = u32::try_from(self.sema.ast.types.args(l).len()).unwrap_or(u32::MAX);
                let i = self.pack_reached(n, name, loc)?;
                Ok(self.sema.ast.types.args(l)[i as usize])
            }
            _ => Ok(a),
        }
    }

    /// Expands a pattern (§7.2): `f` substitutes the pattern once, under
    /// the current pack index.
    pub fn expand_pattern<T>(
        &mut self,
        f: impl FnMut(&mut Self) -> SubstResult<T>,
    ) -> SubstResult<Expansion<T>> {
        self.expand_pattern_at(None, f)
    }

    /// [`Self::expand_pattern`] for the expansion whose `...` is at
    /// `ellipsis`.
    pub fn expand_pattern_at<T>(
        &mut self,
        ellipsis: Option<Loc>,
        mut f: impl FnMut(&mut Self) -> SubstResult<T>,
    ) -> SubstResult<Expansion<T>> {
        let saved_state = std::mem::take(&mut self.pack);
        self.pack.ellipsis = ellipsis;
        let saved_index = self.pack_index.take();
        self.pack.probing = true;
        let probe = f(self);
        let found = self.pack.found.take();
        self.pack.probing = false;
        let r = match (probe, found) {
            (Ok(x), _) => Ok(Expansion::Unexpanded(x)),
            (Err(e), None) => Err(e),
            (Err(_), Some((n, name))) => {
                self.pack.expanding = Some((n, name));
                let mut out = Vec::with_capacity(n as usize);
                let mut res = Ok(());
                for k in 0..n {
                    self.pack_index = Some(k);
                    match f(self) {
                        Ok(x) => out.push(x),
                        Err(e) => {
                            res = Err(e);
                            break;
                        }
                    }
                }
                self.sema.templ.pack_elements += u64::from(n);
                res.map(|()| Expansion::Elements(out))
            }
        };
        self.pack = saved_state;
        self.pack_index = saved_index;
        r
    }

    /// Expands a list of pattern elements (§12 rule 2): an element that
    /// is a pack expansion (`pattern_of` gives its pattern) is replaced
    /// by its elements, each substituted by `each`; an expansion that
    /// cannot be expanded yet is rebuilt by `rewrap`.
    pub fn expand_list<T: Copy>(
        &mut self,
        items: &[T],
        pattern_of: impl Fn(&Sema<'_, '_>, T) -> Option<T>,
        mut each: impl FnMut(&mut Self, T) -> SubstResult<T>,
        rewrap: impl Fn(&mut Self, T) -> T,
    ) -> SubstResult<Vec<T>> {
        let mut out = Vec::with_capacity(items.len());
        for &x in items {
            if let Some(p) = pattern_of(self.sema, x) {
                match self.expand_pattern(|s| each(s, p))? {
                    Expansion::Elements(v) => out.extend(v),
                    Expansion::Unexpanded(y) => out.push(rewrap(self, y)),
                }
            } else {
                out.push(each(self, x)?);
            }
        }
        Ok(out)
    }

    /// Substitutes into a list of types that may contain pack
    /// expansions.
    pub fn expand_types(&mut self, tys: &[QualType], loc: Loc) -> SubstResult<Vec<QualType>> {
        self.expand_list(
            tys,
            type_pattern,
            |s, q| s.element_ty(q, loc),
            |s, q| s.sema.ast.types.pack_expansion(q, None),
        )
    }

    /// One element of a type pack expansion: an element that still names
    /// an unexpanded pack (of a level not substituted, `tl<Ts, Us>...`
    /// with only `Ts` known) needs clang's partially-substituted packs.
    pub(crate) fn element_ty(&mut self, q: QualType, loc: Loc) -> SubstResult<QualType> {
        let t = self.ty(q, loc)?;
        if self.pack_index.is_some()
            && self.sema.ast.types.qflags(t).contains(kiln_ast::types::TypeFlags::UNEXPANDED_PACK)
        {
            return self.unimplemented("partially substituted pack expansion", loc);
        }
        Ok(t)
    }

    /// Substitutes into a list of expressions that may contain pack
    /// expansions.
    pub fn expand_exprs(&mut self, es: &[ExprId]) -> SubstResult<Vec<ExprId>> {
        let mut out = Vec::with_capacity(es.len());
        for &x in es {
            let Some(p) = expr_pattern(self.sema, x) else {
                out.push(self.expr(x)?);
                continue;
            };
            // A `PackExpansionExpr` is located at its `...`.
            let ellipsis = self.sema.ast.exprs.get(x).loc();
            match self.expand_pattern_at(Some(ellipsis), |s| s.expr(p))? {
                Expansion::Elements(v) => out.extend(v),
                Expansion::Unexpanded(y) => out.push(self.sema.act_pack_expansion(y, ellipsis)),
            }
        }
        Ok(out)
    }

    /// Maps the parameters `pp` of a pattern function to the parameters
    /// `fp` of its instantiation: a function parameter pack takes the
    /// parameters its expansion produced (§7.1).
    pub(crate) fn map_params(&mut self, pp: &[kiln_ast::DeclId], fp: &[kiln_ast::DeclId]) {
        use crate::templ::subst::DeclOrPack;
        let is_pack = |s: &Self, p: kiln_ast::DeclId| {
            s.sema
                .ast
                .decls
                .param(p)
                .is_some_and(|x| x.flags & kiln_ast::decl::ParamData::PACK != 0)
        };
        let packs = pp.iter().filter(|&&p| is_pack(self, p)).count();
        let extra = (fp.len() + packs).saturating_sub(pp.len());
        let mut j = 0;
        for &a in pp {
            if is_pack(self, a) && packs == 1 {
                let end = (j + extra).min(fp.len());
                let l = self.sema.ast.lists.push(&fp[j..end]);
                self.locals.insert(a, DeclOrPack::Pack(l));
                j = end;
            } else if let Some(&b) = fp.get(j) {
                self.locals.insert(a, DeclOrPack::Decl(b));
                j += 1;
            }
        }
    }

    /// A fold-expression (§7.3, [temp.variadic]/11): the elements of its
    /// pattern combined by `act_on_binary` in a loop (no recursion per
    /// element, R-WALK), wrapped in a `ParenExpr`.
    pub(crate) fn fold(&mut self, e: ExprId, loc: Loc, end: Loc) -> SubstResult<ExprId> {
        let n = *self.sema.ast.exprs.get(e);
        let op = kiln_ast::expr::BinaryOp::from_u8(n.aux()).ok_or(SubstFail)?;
        let lhs = ExprId::from_raw(n.a());
        let rhs = ExprId::from_raw(n.b());
        let has_pack = |s: &Self, x: Option<ExprId>| {
            x.is_some_and(|x| {
                s.sema.ast.exprs.get(x).deps().contains(kiln_ast::expr::ExprDeps::UNEXPANDED_PACK)
            })
        };
        // A right fold has its pattern on the left: `(E op ...)`.
        let right = has_pack(self, lhs);
        let (pattern, init) = if right { (lhs, rhs) } else { (rhs, lhs) };
        let pattern = pattern.ok_or(SubstFail)?;
        let ellipsis = self.sema.templ.fold_ellipsis.get(&e).copied().unwrap_or(loc);
        let init = self.opt_expr(init)?;
        let elems = match self.expand_pattern_at(Some(ellipsis), |s| s.expr(pattern))? {
            Expansion::Elements(v) => v,
            Expansion::Unexpanded(p) => {
                let (l, r) = if right { (Some(p), init) } else { (init, Some(p)) };
                return Ok(self.sema.act_fold(l, op, r, loc, ellipsis, end));
            }
        };
        let mut it: Box<dyn Iterator<Item = ExprId>> =
            if right { Box::new(elems.into_iter().rev()) } else { Box::new(elems.into_iter()) };
        let Some(mut acc) = init.or_else(|| it.next()) else {
            use kiln_ast::expr::BinaryOp;
            return match op {
                BinaryOp::LAnd => Ok(self.sema.act_on_bool_literal(true, ellipsis)),
                BinaryOp::LOr => Ok(self.sema.act_on_bool_literal(false, ellipsis)),
                BinaryOp::Comma => {
                    Ok(self.sema.act_on_value_init_cast(QualType::VOID, ellipsis, ellipsis))
                }
                _ => {
                    let d = self.sema.diag(DiagId::sema_fold_empty, loc).arg(op.spelling());
                    self.sema.emit(d);
                    Err(SubstFail)
                }
            };
        };
        for x in it {
            self.check_stack(|_| loc)?;
            acc = if right {
                self.sema.act_on_binary(op, x, acc, ellipsis)
            } else {
                self.sema.act_on_binary(op, acc, x, ellipsis)
            };
            if self.sema.is_err_expr(acc) {
                return Err(SubstFail);
            }
        }
        Ok(self.sema.act_on_paren(acc, loc, end))
    }

    /// `sizeof...(P)` (§7.4): the length of the substituted pack, a
    /// `SizeOfPackExpr` that is no longer dependent.
    pub(crate) fn size_of_pack(&mut self, e: ExprId, loc: Loc, end: Loc) -> SubstResult<ExprId> {
        use crate::templ::args::ArgLookup;
        use crate::templ::subst::DeclOrPack;
        let n = *self.sema.ast.exprs.get(e);
        let d = kiln_ast::DeclId::from_raw(n.a()).ok_or(SubstFail)?;
        let len = if let Some(&DeclOrPack::Pack(l)) = self.locals.get(&d) {
            Some(self.sema.ast.lists.len(l))
        } else if let Some(info) = self.sema.ast.template_parm(d).copied() {
            match self.lookup(info.depth, info.index) {
                ArgLookup::Arg(a) => match a.view() {
                    ArgView::Pack(l) => Some(self.sema.ast.types.args(l).len()),
                    _ => None,
                },
                ArgLookup::Retain(_) => return Ok(e),
                ArgLookup::Missing => return Err(SubstFail),
            }
        } else {
            return Ok(e);
        };
        let Some(len) = len else { return self.unimplemented("sizeof... of this pack", loc) };
        // Gap (spec §7.4): an `IntegerLiteral` of type `size_t` stands
        // for the substituted `SizeOfPackExpr` (its dump differs).
        let _ = end;
        let st = self.sema.size_type();
        Ok(self.int_literal(st, len as i128, loc))
    }
}
