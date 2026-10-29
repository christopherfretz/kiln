//! Initialization ([dcl.init]): copy-, direct- and list-initialization
//! of scalars, references, arrays and aggregates (spec/sema.md §6).

use kiln_ast::decl::{DeclKind, FieldData, TagKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::conv::{self, ConvCtx};

/// An initializer as written ([dcl.init.general]/1).
#[derive(Clone, Debug)]
pub enum Initializer {
    /// `= expr`, or a single expression.
    Expr(ExprId),
    /// `( expr-list )`.
    Paren(Vec<ExprId>, Loc, Loc),
    /// `{ ... }` or `= { ... }`.
    List(InitList),
}

/// A braced-init-list.
#[derive(Clone, Debug)]
pub struct InitList {
    /// The initializer-clauses.
    pub items: Vec<InitItem>,
    /// `{`.
    pub lbrace: Loc,
    /// `}`.
    pub rbrace: Loc,
}

/// One initializer-clause, possibly designated ([dcl.init.aggr]/3).
#[derive(Clone, Debug)]
pub struct InitItem {
    /// `.x` / `[i]` designators (outermost first); empty when not
    /// designated.
    pub designator: Vec<Designator>,
    /// The clause.
    pub value: InitValue,
}

/// A designator.
#[derive(Copy, Clone, Debug)]
pub enum Designator {
    /// `.name`.
    Field(Symbol, Loc),
    /// `[index]` (a GNU extension in C++).
    Index(ExprId, Loc),
}

/// An initializer-clause.
#[derive(Clone, Debug)]
pub enum InitValue {
    /// An assignment-expression.
    Expr(ExprId),
    /// A nested braced-init-list.
    List(InitList),
}

impl InitValue {
    fn loc(&self, s: &Sema<'_, '_>) -> Loc {
        match self {
            InitValue::Expr(e) => s.ast.exprs.get(*e).loc(),
            InitValue::List(l) => l.lbrace,
        }
    }
}

impl Sema<'_, '_> {
    /// The `StringLiteral` `e` is (through parentheses), if any.
    pub(crate) fn string_lit_of(&self, mut e: ExprId) -> Option<ExprId> {
        for _ in 0..64 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::StringLiteral => return Some(e),
                ExprKind::ParenExpr => e = self.ast.exprs.operand(e),
                _ => return None,
            }
        }
        None
    }

    fn is_char_type(&self, t: QualType) -> bool {
        use kiln_ast::types::BuiltinType as B;
        matches!(
            self.ast.types.builtin(self.ast.types.canonical(t).without_quals()),
            Some(B::Char | B::SChar | B::UChar | B::Char8 | B::Char16 | B::Char32 | B::WChar)
        )
    }

    /// True for class and array types (aggregates in M2, [dcl.init.aggr]/1).
    fn is_aggregate(&self, t: QualType) -> bool {
        self.ast.types.is_array(t) || self.ast.types.as_record(t).is_some()
    }

    /// The initialization of an object of type `ty` by `init`
    /// ([dcl.init.general]/16); `direct_list` for `T x{...}`
    /// (direct-list-initialization, [dcl.init.list]/1).
    pub fn perform_init(
        &mut self,
        ty: QualType,
        init: Initializer,
        loc: Loc,
        direct_list: bool,
    ) -> ExprId {
        if self.is_error_type(ty) {
            return match init {
                Initializer::Expr(e) => e,
                Initializer::Paren(v, l, _) => self.recover(QualType::ERROR, &v, l),
                Initializer::List(l) => {
                    let items: Vec<ExprId> = l
                        .items
                        .iter()
                        .filter_map(
                            |i| if let InitValue::Expr(e) = i.value { Some(e) } else { None },
                        )
                        .collect();
                    self.recover(QualType::ERROR, &items, l.lbrace)
                }
            };
        }
        match init {
            Initializer::Expr(e) => self.init_from_expr(ty, e, ConvCtx::Initialization, loc),
            Initializer::Paren(v, l, _) => {
                if v.len() == 1 {
                    self.init_from_expr(ty, v[0], ConvCtx::DirectInitialization, loc)
                } else {
                    self.unsupported("parenthesized initialization with multiple expressions", l);
                    self.recover(ty, &v, l)
                }
            }
            Initializer::List(l) => {
                if direct_list && let Some(e) = self.fixed_enum_list_init(ty, &l) {
                    return e;
                }
                self.init_from_list(ty, &l)
            }
        }
    }

    /// A failed conversion is reported at `at`, the entity's location
    /// (clang's `InitializationKind::getLocation`).
    fn init_from_expr(&mut self, ty: QualType, e: ExprId, ctx: ConvCtx, at: Loc) -> ExprId {
        let ety = self.ast.exprs.get(e).ty();
        if self.is_error_type(ety) {
            return e;
        }
        if self.ast.types.is_array(ty) {
            let elem = self.ast.types.element(ty).unwrap_or(QualType::ERROR);
            if let Some(s) = self.string_lit_of(e)
                && self.is_char_type(elem)
            {
                return self.check_string_init(ty, s, e);
            }
            let which = if self.is_char_type(elem) { "1" } else { "0" };
            let msg = self
                .diag(DiagId::sema_array_init_not_init_list, self.ast.exprs.get(e).loc())
                .arg(which);
            self.emit(msg);
            return e;
        }
        match conv::implicit_convert(self, e, ty, ctx) {
            Ok(x) => x,
            Err(err) => {
                let d = err.args.into_iter().fold(self.diag(err.id, at), kiln_diag::Diag::arg);
                self.emit(d);
                let loc = self.ast.exprs.get(e).loc();
                self.recover(ty, &[e], loc)
            }
        }
    }

    /// `char s[N] = "..."` ([dcl.init.string]).
    fn check_string_init(&mut self, ty: QualType, s: ExprId, e: ExprId) -> ExprId {
        let sty = self.ast.exprs.get(s).ty();
        let len = self.ast.types.array_size(sty).unwrap_or(0);
        if let Some(n) = self.ast.types.array_size(ty) {
            // `len` counts the terminating NUL, which must fit in C++
            // ([dcl.init.string]/2).
            if len > n {
                let msg = self
                    .diag(DiagId::sema_init_string_too_long, self.ast.exprs.get(s).loc())
                    .arg(n.to_string())
                    .arg(len.to_string());
                self.emit(msg);
            }
        }
        // The literal takes the (completed) array type as a prvalue.
        let target = if self.ast.types.array_size(ty).is_some() {
            ty
        } else {
            let elem = self.ast.types.element(ty).unwrap_or(QualType::ERROR);
            self.ast.types.constant_array(elem, len, None)
        };
        self.ast.exprs.update_string_init_type(e, target);
        e
    }

    /// A value-initialized member with no initializer: a nested list for a
    /// class, `ImplicitValueInitExpr` otherwise (clang's
    /// `InitListChecker` fillers).
    fn filler(&mut self, ty: QualType, at: Loc) -> ExprId {
        if self.ast.types.as_record(ty).is_some() {
            let empty = InitList { items: Vec::new(), lbrace: at, rbrace: at };
            return self.init_from_list(ty, &empty);
        }
        self.builder().implicit_value_init(ty)
    }

    /// The members of a class that take part in aggregate initialization:
    /// fields, without unnamed bit-fields ([dcl.init.aggr]/2).
    fn init_fields(&self, r: DeclId) -> Vec<DeclId> {
        let def = self.ast.definition(r).unwrap_or(r);
        self.ast
            .decls
            .members(def)
            .filter(|&f| {
                let n = self.ast.decls.get(f);
                n.kind() == DeclKind::Field
                    && !(n.name().is_empty()
                        && self
                            .ast
                            .decls
                            .field(f)
                            .is_some_and(|x: &FieldData| x.bit_width.is_some()))
            })
            .collect()
    }

    /// List-initialization ([dcl.init.list]/3).
    pub fn init_from_list(&mut self, ty: QualType, l: &InitList) -> ExprId {
        if self.is_aggregate(ty) {
            if self.ast.types.as_record(ty).is_some() && !self.is_complete(ty) {
                let msg = self
                    .diag(DiagId::sema_typecheck_decl_incomplete_type, l.lbrace)
                    .arg(self.ty_arg(ty));
                self.emit(self.incomplete_note(msg, ty));
                return self.recover(ty, &[], l.lbrace);
            }
            if let Some(def) = self.ast.types.as_record(ty).and_then(|r| self.ast.definition(r))
                && self.ast.decls.bases(def, &self.ast.lists).next().is_some()
            {
                self.unsupported("aggregate initialization of a class with base classes", l.lbrace);
                let items: Vec<ExprId> = l
                    .items
                    .iter()
                    .filter_map(|i| if let InitValue::Expr(e) = i.value { Some(e) } else { None })
                    .collect();
                return self.recover(ty, &items, l.lbrace);
            }
            let mut pos = 0;
            let e = self.check_aggregate(ty, &l.items, &mut pos, true, l.lbrace, l.rbrace);
            if pos < l.items.len() {
                self.excess(ty, &l.items[pos]);
            }
            return e;
        }
        if self.ast.types.is_reference(ty) {
            self.unsupported("list-initialization of a reference", l.lbrace);
            return self.recover(ty, &[], l.lbrace);
        }
        // Scalar ([dcl.init.list]/3.9-10).
        let mut inits = Vec::new();
        match l.items.first() {
            None => {}
            Some(InitItem { value: InitValue::Expr(e), designator }) if designator.is_empty() => {
                self.check_narrowing(*e, ty, ty);
                inits.push(self.convert_for_init(*e, ty, ConvCtx::Initialization));
            }
            Some(item) => {
                let what = if matches!(item.value, InitValue::List(_)) {
                    "nested braces"
                } else {
                    "a designator"
                };
                self.unsupported(&format!("{what} in a scalar initializer"), item.value.loc(self));
            }
        }
        if let Some(extra) = l.items.get(1) {
            let msg = self.diag(DiagId::sema_excess_initializers, extra.value.loc(self)).arg("2");
            self.emit(msg);
        }
        self.builder().init_list(ty, &inits, None, None, l.lbrace, l.rbrace)
    }

    /// `E e{v};` for an enumeration `E` with a fixed underlying type `U`
    /// and `v` implicitly convertible to `U` ([dcl.init.list]/3.8): `v`
    /// is converted to `U` (narrowing rejected), then to `E`.
    fn fixed_enum_list_init(&mut self, ty: QualType, l: &InitList) -> Option<ExprId> {
        let [InitItem { value: InitValue::Expr(e), designator }] = l.items.as_slice() else {
            return None;
        };
        let e = *e;
        if !designator.is_empty() {
            return None;
        }
        let t = conv::strip_cv(self, ty);
        let d = self.ast.types.as_enum(t)?;
        let def = self.ast.definition(d).unwrap_or(d);
        let data = self.ast.decls.enumeration(def)?;
        if data.flags & kiln_ast::decl::EnumData::FIXED == 0 {
            return None;
        }
        let u = data.int_type?;
        let from = conv::strip_cv(self, self.ast.exprs.get(e).ty());
        let unscoped_enum = self.ast.types.as_enum(from).is_some_and(|fd| {
            let fd = self.ast.definition(fd).unwrap_or(fd);
            self.ast.first(fd) != self.ast.first(def)
                && self
                    .ast
                    .decls
                    .enumeration(fd)
                    .is_some_and(|x| x.flags & kiln_ast::decl::EnumData::SCOPED == 0)
        });
        if !self.ast.types.is_integral(from) && !unscoped_enum {
            return None;
        }
        self.check_narrowing(e, u, ty);
        let r = conv::lvalue_to_rvalue(self, e);
        let c = conv::cast_to(self, kiln_ast::expr::CastKind::IntegralCast, r, t, None);
        Some(self.builder().init_list(ty, &[c], None, None, l.lbrace, l.rbrace))
    }

    /// Rejects a narrowing conversion of the list element `e` to `to`
    /// ([dcl.init.list]/7, clang's `-Wc++11-narrowing` errors).
    /// `entity` is the initialized type as diagnosed (`to` itself, or
    /// the enumeration for [dcl.init.list]/3.8).
    fn check_narrowing(&mut self, e: ExprId, to: QualType, entity: QualType) {
        let from = self.ast.exprs.get(e).ty();
        if self.is_error_type(from) || self.is_error_type(to) {
            return;
        }
        let t = conv::strip_cv(self, to);
        if !self.ast.types.is_arithmetic(t) && self.ast.types.as_enum(t).is_none() {
            return;
        }
        let Some(scs) = conv::standard_conversion(self, e, to, ConvCtx::Initialization) else {
            return;
        };
        let value = self.try_fold(e);
        let n = conv::narrowing_kind(self, &scs, e, value);
        if let Some(d) = conv::narrowing_diag(self, &n, e, entity) {
            self.emit(d);
        }
    }

    fn excess(&self, ty: QualType, item: &InitItem) {
        let which = if self.ast.types.is_array(ty) {
            "0"
        } else if self.ast.types.as_record(ty).is_some_and(|r| {
            self.ast
                .decls
                .record(self.ast.definition(r).unwrap_or(r))
                .is_some_and(|x| x.tag == TagKind::Union)
        }) {
            "3"
        } else {
            "4"
        };
        let msg = self.diag(DiagId::sema_excess_initializers, item.value.loc(self)).arg(which);
        self.emit(msg);
    }

    /// One element of an aggregate: a nested list, a brace-elided
    /// sub-aggregate ([dcl.init.aggr]/16) or an expression.
    fn init_element(
        &mut self,
        ty: QualType,
        items: &[InitItem],
        pos: &mut usize,
        at: Loc,
    ) -> ExprId {
        match &items[*pos].value {
            InitValue::List(sub) => {
                *pos += 1;
                self.init_from_list(ty, sub)
            }
            InitValue::Expr(e) => {
                let e = *e;
                let ety = self.ast.exprs.get(e).ty();
                let elide = if self.ast.types.is_array(ty) {
                    let elem = self.ast.types.element(ty).unwrap_or(QualType::ERROR);
                    !(self.is_char_type(elem) && self.string_lit_of(e).is_some())
                } else if let Some(r) = self.ast.types.as_record(ty) {
                    self.ast
                        .types
                        .as_record(ety)
                        .is_none_or(|x| self.ast.first(x) != self.ast.first(r))
                        && !self.is_error_type(ety)
                } else {
                    false
                };
                if elide {
                    self.check_aggregate(ty, items, pos, false, at, at)
                } else {
                    *pos += 1;
                    let at = self.begin_loc(e);
                    self.check_narrowing(e, ty, ty);
                    self.init_from_expr(ty, e, ConvCtx::Initialization, at)
                }
            }
        }
    }

    /// Initializes an aggregate of type `ty` from `items[*pos..]`.
    /// `braced` is false under brace elision, where designators end the
    /// sub-aggregate.
    fn check_aggregate(
        &mut self,
        ty: QualType,
        items: &[InitItem],
        pos: &mut usize,
        braced: bool,
        lbrace: Loc,
        rbrace: Loc,
    ) -> ExprId {
        if self.ast.types.is_array(ty) {
            return self.check_array(ty, items, pos, braced, lbrace, rbrace);
        }
        let Some(r) = self.ast.types.as_record(ty) else {
            return self.recover(ty, &[], lbrace);
        };
        let fields = self.init_fields(r);
        let def = self.ast.definition(r).unwrap_or(r);
        let union = self.ast.decls.record(def).is_some_and(|x| x.tag == TagKind::Union);
        let mut slots: Vec<Option<ExprId>> = vec![None; fields.len()];
        let mut cur = 0usize;
        let mut union_field = None;
        while *pos < items.len() {
            let item = &items[*pos];
            if let Some(d) = item.designator.first() {
                if !braced {
                    break;
                }
                let Designator::Field(name, dloc) = *d else {
                    self.unsupported("array designator for a class", item.value.loc(self));
                    *pos += 1;
                    continue;
                };
                if item.designator.len() > 1 {
                    self.unsupported("nested designators", dloc);
                    *pos += 1;
                    continue;
                }
                let Some(idx) = fields
                    .iter()
                    .position(|&f| self.ast.decls.get(f).name().as_ident() == Some(name))
                else {
                    let msg = self
                        .diag(DiagId::sema_no_member, dloc)
                        .arg(self.ast.symbols().as_str(name).to_string())
                        .arg(self.ty_arg(ty));
                    self.emit(msg);
                    *pos += 1;
                    continue;
                };
                cur = idx;
            }
            if cur >= fields.len() || (union && union_field.is_some() && item.designator.is_empty())
            {
                if braced {
                    self.excess(ty, item);
                    *pos = items.len();
                }
                break;
            }
            let f = fields[cur];
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            let e = self.init_element(fty, items, pos, lbrace);
            slots[cur] = Some(e);
            if union {
                union_field = Some(f);
                slots = vec![None; fields.len()];
                slots[cur] = Some(e);
            }
            cur += 1;
        }
        let mut inits = Vec::new();
        if union {
            if let Some(f) = union_field {
                let i = fields.iter().position(|&x| x == f).unwrap_or(0);
                inits.extend(slots[i]);
            } else if let Some(&f) = fields.first() {
                let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                inits.push(self.filler(fty, rbrace));
                union_field = Some(f);
            }
        } else {
            for (i, slot) in slots.into_iter().enumerate() {
                let e = if let Some(e) = slot {
                    e
                } else {
                    let fty = self.ast.decls.get(fields[i]).ty().unwrap_or(QualType::ERROR);
                    self.filler(fty, rbrace)
                };
                inits.push(e);
            }
        }
        self.builder().init_list(ty, &inits, None, union_field, lbrace, rbrace)
    }

    fn check_array(
        &mut self,
        ty: QualType,
        items: &[InitItem],
        pos: &mut usize,
        braced: bool,
        lbrace: Loc,
        rbrace: Loc,
    ) -> ExprId {
        let elem = self.ast.types.element(ty).unwrap_or(QualType::ERROR);
        let bound = self.ast.types.array_size(ty);
        let mut slots: Vec<Option<ExprId>> = Vec::new();
        let mut cur: u64 = 0;
        while *pos < items.len() {
            let item = &items[*pos];
            if let Some(d) = item.designator.first() {
                if !braced {
                    break;
                }
                let Designator::Index(ie, dloc) = *d else {
                    let msg = self
                        .diag(DiagId::sema_no_member, item.value.loc(self))
                        .arg("field")
                        .arg(self.ty_arg(ty));
                    self.emit(msg);
                    *pos += 1;
                    continue;
                };
                if item.designator.len() > 1 {
                    self.unsupported("nested designators", dloc);
                    *pos += 1;
                    continue;
                }
                let c = conv::lvalue_to_rvalue(self, ie);
                let c = self.act_on_full_expr(c);
                match self
                    .eval_int(c, kiln_oven::EvalContext::new(kiln_oven::EvalKind::Other, dloc))
                {
                    Ok(v) if v >= 0 => {
                        cur = u64::try_from(v).unwrap_or(u64::MAX);
                    }
                    r => {
                        let d = self.diag(DiagId::sema_expr_not_ice, dloc);
                        let d = match r {
                            Err(f) => Self::with_failure_notes(d, f),
                            Ok(_) => d,
                        };
                        self.emit(d);
                        *pos += 1;
                        continue;
                    }
                }
            }
            if bound.is_some_and(|b| cur >= b) {
                if braced {
                    self.excess(ty, item);
                    *pos = items.len();
                }
                break;
            }
            let e = self.init_element(elem, items, pos, lbrace);
            let i = usize::try_from(cur).unwrap_or(usize::MAX);
            if slots.len() <= i {
                slots.resize(i + 1, None);
            }
            slots[i] = Some(e);
            cur += 1;
        }
        let n = bound.unwrap_or(slots.len() as u64);
        let mut inits = Vec::with_capacity(slots.len());
        for slot in slots {
            let e = match slot {
                Some(e) => e,
                None => self.filler(elem, rbrace),
            };
            inits.push(e);
        }
        let filler = if (inits.len() as u64) < n { Some(self.filler(elem, rbrace)) } else { None };
        let lty = if bound.is_none() {
            let complete = self.ast.types.constant_array(elem, n, None);
            let q = self.ast.types.effective_quals(ty);
            if q.is_empty() { complete } else { self.ast.types.qualify(complete, q) }
        } else {
            ty
        };
        self.builder().init_list(lty, &inits, filler, None, lbrace, rbrace)
    }
}
