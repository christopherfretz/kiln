//! Initialization ([dcl.init]): copy-, direct- and list-initialization
//! of scalars, references, arrays and aggregates (spec/sema.md §6).

use kiln_ast::decl::{DeclKind, DefinitionBits, FieldData, TagKind};
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::class::{ConstructSite, DeletedAs, InitStyle};
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

impl Designator {
    /// The location of the designator (the name of `.name`, the `[`).
    pub fn loc(&self) -> Loc {
        match *self {
            Designator::Field(_, l) | Designator::Index(_, l) => l,
        }
    }

    /// The AST form (a `DesignatedInitExpr` designator).
    pub fn to_ast(&self) -> kiln_ast::expr::Designator {
        match *self {
            Designator::Field(name, loc) => kiln_ast::expr::Designator::Field { name, loc },
            Designator::Index(index, loc) => kiln_ast::expr::Designator::Index { index, loc },
        }
    }
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

    pub(crate) fn is_char_type(&self, t: QualType) -> bool {
        use kiln_ast::types::BuiltinType as B;
        matches!(
            self.ast.types.builtin(self.ast.types.canonical(t).without_quals()),
            Some(B::Char | B::SChar | B::UChar | B::Char8 | B::Char16 | B::Char32 | B::WChar)
        )
    }

    /// True for array types and aggregate classes ([dcl.init.aggr]/1);
    /// an incomplete class counts as one (its initialization is
    /// diagnosed as incomplete).
    pub(crate) fn is_aggregate(&self, t: QualType) -> bool {
        if self.ast.types.is_array(t) {
            return true;
        }
        let Some(r) = self.ast.types.as_record(t) else { return false };
        match self.ast.definition(r) {
            Some(d) => {
                let bits = self.defn_bits(d);
                !bits.contains(DefinitionBits::COMPLETE) || bits.contains(DefinitionBits::AGGREGATE)
            }
            None => true,
        }
    }

    /// List-initialization of a non-aggregate class `ty` by a constructor
    /// ([dcl.init.list]/3.5, /3.7): the elements are the arguments (an
    /// empty list value-initializes). `None` when `ty` is not such a
    /// class. `at` is the entity; `temp_end` makes the call a
    /// `CXXTemporaryObjectExpr` (`T{...}`).
    pub(crate) fn list_construct(
        &mut self,
        ty: QualType,
        l: &InitList,
        style: InitStyle,
        at: Loc,
        temp_end: Option<Loc>,
    ) -> Option<ExprId> {
        if let Some(r) = self.ast.types.as_record(ty) {
            // Whether it is an aggregate needs its definition.
            self.instantiate_for_init(r, at);
        }
        if self.is_error_type(ty) || self.ast.types.as_record(ty).is_none() || self.is_aggregate(ty)
        {
            return None;
        }
        // [dcl.init.list]/3.1: a designated-initializer-list needs an
        // aggregate class, before any constructor is considered (clang
        // `FK_DesignatedInitForNonAggregate`, ahead of phase 1).
        if l.items.iter().any(|i| !i.designator.is_empty()) {
            return Some(self.designated_non_aggregate(ty, l, at));
        }
        if let [InitItem { value: InitValue::Expr(x), designator }] = l.items.as_slice()
            && designator.is_empty()
            && !self.expr_is_type_dependent(*x)
            && self.is_class_or_derived(*x, ty)
        {
            // [dcl.init.list]/3.2 as clang applies DR1467 to every class
            // (not only aggregates, CWG2137): a single element of the class
            // or a derived class initializes the object, copy-initialized
            // for copy-list-initialization (explicit constructors are not
            // candidates) and direct-initialized for direct-list; no
            // initializer-list constructor is considered.
            let ctx = if style == InitStyle::DirectList {
                ConvCtx::DirectInitialization
            } else {
                ConvCtx::Initialization
            };
            return Some(self.init_from_expr(ty, *x, ctx, at));
        }
        if !l.items.is_empty()
            && let Some(e) = self.std_initializer_list_init(ty, l, at)
        {
            return Some(e);
        }
        if let Some(e) = self.init_list_ctor_init(ty, l, style, at, temp_end) {
            return Some(e);
        }
        let mut args = Vec::with_capacity(l.items.len());
        for item in &l.items {
            match &item.value {
                InitValue::Expr(e) => args.push(*e),
                // A nested braced-init-list is an argument converted by
                // [over.ics.list] (WP-152): the call-argument placeholder,
                // as for `T x({...})`.
                InitValue::List(n) => args.push(self.act_on_braced_arg(n.clone())),
            }
        }
        if args.iter().any(|&a| self.expr_is_type_dependent(a)) {
            for &a in &args {
                let _ = self.take_braced_arg(a);
            }
            return None;
        }
        let mut site = ConstructSite::at(at);
        site.temp_end = temp_end;
        self.construct_class(ty, &args, style, site)
    }

    /// "initialization of non-aggregate type 'T' with a designated
    /// initializer list" at the entity `at`, the list underlined
    /// ([dcl.init.list]/3.1); the result is a recovery expression.
    pub(crate) fn designated_non_aggregate(
        &mut self,
        ty: QualType,
        l: &InitList,
        at: Loc,
    ) -> ExprId {
        let end = self.token_end(l.rbrace).unwrap_or(l.rbrace);
        let msg = self
            .diag(DiagId::sema_designated_init_non_aggregate, at)
            .arg(self.ty_arg(ty))
            .range(kiln_source::Span::new(l.lbrace, end));
        self.emit(msg);
        let flat = Self::list_exprs(l);
        self.recover(ty, &flat, l.lbrace)
    }

    /// The C++ checks on the designators of a braced-init-list as
    /// written (clang `ActOnInitList`): `-Wc99-designator` for nested
    /// designators, array designators and a mix of designated and
    /// non-designated clauses, each at most once per list. Run once,
    /// when the list is parsed (not again at instantiation).
    pub fn act_on_init_list_syntax(&mut self, l: &InitList) {
        let Some(first) = l.items.first() else { return };
        // A designated pack expansion `.m = e...` is a
        // `PackExpansionExpr`, not a `DesignatedInitExpr`, to these
        // checks (clang `ActOnInitList`).
        let heads: Vec<Option<Designator>> = l
            .items
            .iter()
            .map(|item| match item.value {
                InitValue::Expr(e) if crate::templ::pack::expr_pattern(self, e).is_some() => None,
                _ => item.designator.first().copied(),
            })
            .collect();
        let first_designator = heads[0];
        let (mut nested, mut array, mut mixed) = (false, false, false);
        for (item, head) in l.items.iter().zip(&heads) {
            if let Some(d) = head {
                if !nested && item.designator.len() > 1 {
                    nested = true;
                    let msg =
                        self.diag(DiagId::sema_designated_init_nested, self.designator_begin(d));
                    self.emit(msg);
                }
                for d in &item.designator {
                    if let Designator::Index(_, at) = *d
                        && !array
                    {
                        array = true;
                        let msg = self.diag(DiagId::sema_designated_init_array, at);
                        self.emit(msg);
                    }
                }
                if !mixed && first_designator.is_none() {
                    mixed = true;
                    let msg =
                        self.diag(DiagId::sema_designated_init_mixed, self.designator_begin(d));
                    let note =
                        self.diag(DiagId::note_designated_init_mixed, self.item_begin(first));
                    self.emit(msg.with_note(note));
                }
            } else if !mixed && let Some(d) = first_designator.as_ref() {
                mixed = true;
                let msg = self.diag(DiagId::sema_designated_init_mixed, self.designator_begin(d));
                let note = self.diag(DiagId::note_designated_init_mixed, self.item_begin(item));
                self.emit(msg.with_note(note));
            }
        }
    }

    /// Where a designator begins: the `.` of `.name`, the `[` of `[i]`.
    fn designator_begin(&self, d: &Designator) -> Loc {
        match *d {
            Designator::Field(_, at) => self.period_before(at),
            Designator::Index(_, at) => at,
        }
    }

    /// Where an initializer-clause begins (its first designator if any).
    fn item_begin(&self, item: &InitItem) -> Loc {
        match (item.designator.first(), &item.value) {
            (Some(d), _) => self.designator_begin(d),
            (None, InitValue::List(l)) => l.lbrace,
            (None, InitValue::Expr(e)) => self.begin_loc(*e),
        }
    }

    /// Whether the token before `at` is `=` (from the source text).
    fn preceded_by_equal(&self, at: Loc) -> bool {
        let sm = self.diags.sm();
        let Some((file, pos)) = sm.offset_of(at) else { return false };
        let text = sm.text(file).as_bytes();
        let before = text.get(..pos as usize).unwrap_or_default();
        before.iter().rev().find(|c| !c.is_ascii_whitespace()) == Some(&b'=')
    }

    /// The `.` of the field designator whose name is at `name`
    /// (`name` itself when the source text has none).
    fn period_before(&self, name: Loc) -> Loc {
        let sm = self.diags.sm();
        let Some((file, pos)) = sm.offset_of(name) else { return name };
        let text = sm.text(file).as_bytes();
        let before = text.get(..pos as usize).unwrap_or_default();
        match before.iter().rposition(|c| !c.is_ascii_whitespace()) {
            Some(i) if before[i] == b'.' => u32::try_from(i).map_or(name, |o| sm.loc(file, o)),
            _ => name,
        }
    }

    /// [over.match.list]/1.1 (phase 1): the initializer-list
    /// constructors of class `ty` with the whole list `l` as their one
    /// argument ([dcl.init.list]/3.7). `None` when phase 1 does not apply
    /// (no initializer-list constructor, an empty list and a default
    /// constructor, a type-dependent element) or selects nothing (phase 2
    /// follows). The argument is built as list-initialization of the
    /// parameter (a `CXXStdInitializerListExpr`, bound to a temporary for
    /// a reference parameter).
    fn init_list_ctor_init(
        &mut self,
        ty: QualType,
        l: &InitList,
        style: InitStyle,
        at: Loc,
        temp_end: Option<Loc>,
    ) -> Option<ExprId> {
        use crate::class::{CtorCall, Selected, SpecialKind};
        use crate::overload::{ArgInfo, IcsOptions, SetKind};
        use kiln_ast::expr::ConstructInfo;
        let target = self.ast.types.as_record(ty)?;
        let class = self.ast.definition(target)?;
        let class = conv::bases::complete_class(self, class)?;
        if !self.has_init_list_ctor(class)
            || (l.items.is_empty() && self.has_default_ctor_cheap(class))
            || l.items.iter().any(|i| match i.value {
                InitValue::Expr(e) => self.expr_is_type_dependent(e),
                InitValue::List(_) => false,
            })
        {
            return None;
        }
        let idx = u32::try_from(self.ovl.lists.len()).unwrap_or(u32::MAX);
        self.ovl.lists.push(l.clone());
        let mut set = self.begin_set(SetKind::ListCtor { phase: 1 }, at);
        set.args.push(ArgInfo {
            list: Some(idx),
            ..ArgInfo::of_type(QualType::VOID, ValueCategory::PRValue)
        });
        self.add_ctor_candidates(&mut set, class, IcsOptions::NONE, true);
        if set.viable().next().is_none() && !set.skipped_templates && !set.uncertain {
            self.end_set(set);
            self.ovl.lists.pop();
            return None;
        }
        let out = self.best_viable(&mut set);
        let sel = self.selected_of(set, out);
        self.ovl.lists.pop();
        let (Selected::Found(f) | Selected::Deleted(f)) = sel else {
            self.unsupported("this initializer-list constructor selection", l.lbrace);
            let flat = Self::list_exprs(l);
            return Some(self.recover(ty, &flat, l.lbrace));
        };
        let (entity, ck) = self.take_ctor_entity();
        let p = self.param_decls(f)[0];
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        let pty = if self.ast.types.is_reference(pty) { pty } else { conv::strip_cv(self, pty) };
        let arg = self.init_from_list_at(pty, l, Some(l.lbrace));
        if self.is_err_expr(arg) {
            return Some(self.recover(ty, &[arg], l.lbrace));
        }
        let call = CtorCall {
            info: ConstructInfo {
                kind: ck,
                list: style.is_list(),
                std_init_list: true,
                ..ConstructInfo::COMPLETE
            },
            temp_end,
            reject_explicit: style == InitStyle::CopyList,
            ..CtorCall::new(std::slice::from_ref(&arg))
        };
        let site = ConstructSite::at(at);
        let built =
            self.use_ctor(sel, ty, class, SpecialKind::CopyCtor, call, at, entity, site.sites);
        Some(built.unwrap_or_else(|| self.recover(ty, &[arg], at)))
    }

    /// [dcl.init.list]/3.6, /5: list-initialization of
    /// `std::initializer_list<E>` from a non-empty list: a backing array
    /// `const E[N]` is copy-initialized from the elements and the object
    /// refers to it (`CXXStdInitializerListExpr` over the materialized
    /// array; the array's lifetime is extended with the object's,
    /// [dcl.init.list]/6). `None` for other types and while an element is
    /// type-dependent.
    pub(crate) fn std_initializer_list_init(
        &mut self,
        ty: QualType,
        l: &InitList,
        at: Loc,
    ) -> Option<ExprId> {
        let elem = self.initializer_list_element(ty)?;
        let dependent = l.items.iter().any(|i| match i.value {
            InitValue::Expr(e) => self.expr_is_type_dependent(e),
            InitValue::List(_) => false,
        });
        if dependent || self.ast.types.is_dependent(elem) {
            return None;
        }
        let n = u64::try_from(l.items.len()).unwrap_or(u64::MAX);
        let celem = self.ast.types.qualify(elem, kiln_ast::types::Quals::CONST);
        let arr = self.ast.types.constant_array(celem, n, None);
        let init = self.init_from_list_at(arr, l, Some(at));
        let mte = self.materialize_temporary(init, arr, false);
        Some(self.builder().std_initializer_list(mte, ty))
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
            Initializer::Paren(v, l, r) => {
                let settled = !v
                    .iter()
                    .any(|&a| self.expr_is_type_dependent(a) || self.is_error_type(self.ety(a)));
                if settled
                    && self.ast.types.is_array(ty)
                    && !(v.len() == 1 && self.string_lit_of(v[0]).is_some())
                {
                    // [dcl.init.general]/16.5: `T a[N](x, y)`.
                    return self.paren_aggregate_init(ty, &v, loc, r);
                }
                if settled && self.ast.types.as_record(ty).is_some() {
                    if v.len() == 1
                        && let Some(x) = self.construct_from_class(
                            v[0],
                            ty,
                            ConvCtx::DirectInitialization,
                            loc,
                            DeletedAs::Call,
                        )
                    {
                        return x;
                    }
                    let mut site = ConstructSite::at(loc);
                    site.rparen = Some(r);
                    if let Some(x) = self.construct_class(ty, &v, InitStyle::Direct, site) {
                        return x;
                    }
                }
                if v.len() == 1 {
                    self.init_from_expr(ty, v[0], ConvCtx::DirectInitialization, loc)
                } else if !self.is_aggregate(ty) && !self.ast.types.is_reference(ty) {
                    // [dcl.init.general]/16.9: a scalar takes one expression.
                    let d = self.diag(DiagId::sema_excess_initializers, loc).arg("2");
                    self.emit(d);
                    self.recover(ty, &v, l)
                } else {
                    self.unsupported("parenthesized initialization with multiple expressions", l);
                    self.recover(ty, &v, l)
                }
            }
            Initializer::List(l) => {
                if direct_list && let Some(e) = self.fixed_enum_list_init(ty, &l) {
                    return e;
                }
                let style = if direct_list { InitStyle::DirectList } else { InitStyle::CopyList };
                let ctx = if direct_list {
                    ConvCtx::DirectInitialization
                } else {
                    ConvCtx::Initialization
                };
                if let Some(e) = self.class_element_list_init(ty, &l, ctx, loc, None) {
                    return e;
                }
                if self.ast.types.is_reference(ty) {
                    return self.reference_list_init(ty, &l, Some(loc), style);
                }
                if let Some(e) = self.list_construct(ty, &l, style, loc, None) {
                    return e;
                }
                self.init_from_list_at(ty, &l, Some(loc))
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
        if let Some(x) = self.atomic_class_init(ty, e, ctx, at) {
            return x;
        }
        if let Some(x) = self.construct_from_class(e, ty, ctx, at, DeletedAs::Call) {
            return x;
        }
        if self.ast.types.as_record(ty).is_some() && !self.expr_is_type_dependent(e) {
            if ctx == ConvCtx::DirectInitialization {
                let site = ConstructSite::at(at);
                if let Some(x) = self.construct_class(ty, &[e], InitStyle::Direct, site) {
                    return x;
                }
            } else if let Some(x) = self.convert_to_class(e, ty, ctx, at, DeletedAs::Call) {
                return x;
            }
        }
        let class_src = self.ast.types.as_record(ety).is_some()
            && !self.ast.types.is_reference(ty)
            && !self.expr_is_type_dependent(e)
            && !self.ast.types.is_dependent(ty);
        let r = if class_src {
            conv::user_conversion(self, e, ty, ctx, at)
        } else {
            conv::implicit_convert(self, e, ty, ctx)
        };
        match r {
            Ok(x) => x,
            Err(err) => {
                self.emit(err.diag_at(self, at));
                let loc = self.ast.exprs.get(e).loc();
                self.recover(ty, &[e], loc)
            }
        }
    }

    /// Initialization of `_Atomic(T)`, `T` a class, from `e` of class
    /// type `T` or derived from `T` (clang `InitializeFrom`
    /// `NeedAtomicConversion`): `T` is initialized from `e`, then
    /// converted by `NonAtomicToAtomic` (WP-165).
    fn atomic_class_init(
        &mut self,
        ty: QualType,
        e: ExprId,
        ctx: ConvCtx,
        at: Loc,
    ) -> Option<ExprId> {
        let v = self.ast.types.as_atomic(ty)?;
        let vr = self.ast.types.as_record(v)?;
        let er = self.ast.types.as_record(self.ety(e))?;
        if !self.derived_inclusive(er, vr) {
            return None;
        }
        let v = conv::strip_cv(self, v);
        let x = self.init_from_expr(v, e, ctx, at);
        let to = conv::strip_cv(self, ty);
        let ck = kiln_ast::expr::CastKind::NonAtomicToAtomic;
        Some(conv::cast_to(self, ck, x, to, None))
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

    /// [dcl.init.list]/3.9 for a single class-type element of a non-class
    /// `ty` (or `nullptr` for `bool`): clang initializes from the element
    /// itself (`TryListInitialization`'s core-24034 rewrite), so a direct
    /// list may use an explicit conversion function, a failure is reported
    /// at the entity `at` (with the "passing argument" note `param`) and not
    /// as a braced scalar, and the user conversion's second standard
    /// conversion must not narrow. `None` if the rule does not apply.
    pub(crate) fn class_element_list_init(
        &mut self,
        ty: QualType,
        l: &InitList,
        ctx: ConvCtx,
        at: Loc,
        param: Option<kiln_diag::Diag>,
    ) -> Option<ExprId> {
        use kiln_ast::types::BuiltinType;
        let [InitItem { value: InitValue::Expr(e), designator }] = l.items.as_slice() else {
            return None;
        };
        let e = *e;
        if !designator.is_empty()
            || self.ast.types.as_record(ty).is_some()
            // `_Atomic(T)` from a `T` of class type is `atomic_class_init`
            // (WP-165), not a conversion function call.
            || self.ast.types.as_atomic(ty).is_some()
            || self.ast.types.is_reference(ty)
            || self.ast.types.is_array(ty)
            || self.ast.types.is_dependent(ty)
            || self.is_error_type(ty)
            || self.expr_is_type_dependent(e)
        {
            return None;
        }
        let ety = self.ety(e);
        let nullptr_bool = self.ast.types.builtin(ety) == Some(BuiltinType::NullPtr)
            && self.ast.types.builtin(conv::strip_cv(self, ty)) == Some(BuiltinType::Bool);
        if self.ast.types.as_record(ety).is_none() && !nullptr_bool {
            return None;
        }
        let r = if nullptr_bool {
            conv::implicit_convert(self, e, ty, ctx)
        } else {
            conv::user_conversion(self, e, ty, ctx, at)
        };
        let x = match r {
            Ok(x) => {
                if !nullptr_bool {
                    let inner = self.ignore_paren_imp_casts(x);
                    self.check_narrowing(inner, ty, ty);
                }
                x
            }
            Err(err) => {
                let mut d = err.diag_at(self, at);
                if let Some(n) = param {
                    d = d.with_note(n);
                }
                self.emit(d);
                let loc = self.ast.exprs.get(e).loc();
                self.recover(ty, &[e], loc)
            }
        };
        Some(self.builder().init_list(ty, &[x], None, None, l.lbrace, l.rbrace))
    }

    /// List-initialization of a parameter (`f({1})`, `int x = {1}` as a
    /// default argument) or a return value (`return {1};`): a single
    /// braced scalar warns `-Wbraced-scalar-init` ([dcl.init.list]/3.9).
    /// `ctx` is the entity's conversion context and `param` the
    /// "passing argument" note of a failure.
    pub(crate) fn init_list_entity(
        &mut self,
        ty: QualType,
        l: &InitList,
        entity: Loc,
        ctx: ConvCtx,
        param: Option<kiln_diag::Diag>,
    ) -> ExprId {
        if self.is_error_type(ty) {
            return self.perform_init(ty, Initializer::List(l.clone()), entity, false);
        }
        if let Some(e) = self.class_element_list_init(ty, l, ctx, entity, param) {
            return e;
        }
        // Scalars only: a class (aggregate or not, complete or not yet
        // instantiated) takes its braces.
        if !self.is_aggregate(ty)
            && self.ast.types.as_record(ty).is_none()
            && !self.ast.types.is_reference(ty)
            && let [InitItem { value: InitValue::Expr(_), designator }] = l.items.as_slice()
            && designator.is_empty()
        {
            let msg = self.diag(DiagId::sema_braces_around_init, l.lbrace);
            self.emit(msg);
        }
        self.init_from_list_at(ty, l, Some(entity))
    }

    /// A value-initialized member with no initializer: a nested list for a
    /// class or an array of classes, `ImplicitValueInitExpr` otherwise
    /// (clang's `InitListChecker` fillers).
    pub(crate) fn filler(&mut self, ty: QualType, at: Loc) -> ExprId {
        let mut base = ty;
        while self.ast.types.array_size(base).is_some()
            && let Some(e) = self.ast.types.element(base)
        {
            base = e;
        }
        if self.ast.types.as_record(base).is_some() {
            let empty = InitList { items: Vec::new(), lbrace: at, rbrace: at };
            return self.init_from_list(ty, &empty);
        }
        self.builder().implicit_value_init(ty)
    }

    /// A field with no initializer in an aggregate initialization: its
    /// default member initializer ([dcl.init.aggr]/5.1, a
    /// `CXXDefaultInitExpr`), else the value-initializing [`Sema::filler`].
    pub(crate) fn field_filler(&mut self, f: DeclId, at: Loc) -> ExprId {
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        if self.field_has_nsdmi(f) {
            let ctx = self.context();
            if let Some(e) = self.default_member_init(f, ctx, at) {
                self.note_default_use(e, f, at);
                return e;
            }
            return self.recover(fty, &[], at);
        }
        self.filler(fty, at)
    }

    /// Parenthesized aggregate initialization ([dcl.init.general]/16.5,
    /// /16.6.2.2, P0960) of the array or class `ty` from `args`: each
    /// element is copy-initialized from its argument (no brace elision,
    /// narrowing allowed); the rest take their default member initializer
    /// or are value-initialized. `at` is the entity (where an excess
    /// argument is reported), `rparen` the `)`.
    pub(crate) fn paren_aggregate_init(
        &mut self,
        ty: QualType,
        args: &[ExprId],
        at: Loc,
        rparen: Loc,
    ) -> ExprId {
        if self.ast.types.is_array(ty) {
            let elem = self.ast.types.element(ty).unwrap_or(QualType::ERROR);
            let bound = self.ast.types.array_size(ty);
            if bound.is_some_and(|n| args.len() as u64 > n) {
                let msg = self.diag(DiagId::sema_excess_initializers, at).arg("0");
                self.emit(msg);
                return self.recover(ty, args, at);
            }
            let inits: Vec<ExprId> = args
                .iter()
                .map(|&a| {
                    let loc = self.begin_loc(a);
                    self.init_from_expr(elem, a, ConvCtx::Initialization, loc)
                })
                .collect();
            if inits.iter().any(|&e| self.is_err_expr(e)) {
                return self.recover(ty, args, at);
            }
            let n = bound.unwrap_or(args.len() as u64);
            let filler = if n > args.len() as u64 {
                if self.ast.types.as_record(elem).is_some() {
                    // Value-initialized class elements: clang's filler is
                    // the default construction (`zeroing` unless the
                    // constructor is user-provided).
                    Some(self.paren_value_init(elem, at))
                } else if self.ast.types.as_record(self.base_element(elem)).is_some() {
                    // todo(WP-067b follow-up): value-initialized arrays of
                    // classes inside a parenthesized array initialization.
                    self.unsupported("parenthesized initialization of an array of classes", at);
                    return self.recover(ty, args, at);
                } else {
                    Some(self.builder().implicit_value_init(elem))
                }
            } else {
                None
            };
            let ty =
                if bound.is_none() { self.ast.types.constant_array(elem, n, None) } else { ty };
            return self.builder().paren_list_init(ty, &inits, filler, at, rparen);
        }
        let Some(r) = self.ast.types.as_record(ty) else {
            return self.recover(ty, args, at);
        };
        let def = self.ast.definition(r).unwrap_or(r);
        if self.ast.decls.record(def).is_some_and(|x| x.tag == TagKind::Union) {
            return self.paren_union_init(ty, def, args, at, rparen);
        }
        let bases = self.init_bases(def);
        let fields = self.init_fields(def);
        let nb = bases.len();
        if args.len() > nb + fields.len() {
            let msg = self.diag(DiagId::sema_excess_initializers, at).arg("4");
            self.emit(msg);
            return self.recover(ty, args, at);
        }
        let mut inits = Vec::with_capacity(nb + fields.len());
        for i in 0..nb + fields.len() {
            let ety = if i < nb {
                bases[i]
            } else {
                self.ast.decls.get(fields[i - nb]).ty().unwrap_or(QualType::ERROR)
            };
            let e = match args.get(i) {
                Some(&a) if i < nb => self.init_base_element(ety, a),
                Some(&a) => {
                    let loc = self.begin_loc(a);
                    self.init_from_expr(ety, a, ConvCtx::Initialization, loc)
                }
                None if i < nb => self.as_base_subobject(|s| s.paren_value_init(ety, at)),
                None => {
                    let f = fields[i - nb];
                    if self.field_has_nsdmi(f) {
                        self.field_filler(f, rparen)
                    } else if self.ast.types.is_reference(ety) {
                        self.uninit_reference_member(f, ety, rparen);
                        return self.recover(ty, args, at);
                    } else {
                        self.paren_value_init(ety, at)
                    }
                }
            };
            inits.push(e);
        }
        if inits.iter().any(|&e| self.is_err_expr(e)) {
            return self.recover(ty, args, at);
        }
        self.builder().paren_list_init(ty, &inits, None, at, rparen)
    }

    /// Parenthesized aggregate initialization of the union `ty` (definition
    /// `def`): the single argument copy-initializes the first member
    /// ([dcl.init.general]/16.6.2.2); a second argument is excess.
    fn paren_union_init(
        &mut self,
        ty: QualType,
        def: DeclId,
        args: &[ExprId],
        at: Loc,
        rparen: Loc,
    ) -> ExprId {
        let fields = self.init_fields(def);
        let (Some(&f), Some(&a)) = (fields.first(), args.first()) else {
            return self.recover(ty, args, at);
        };
        if args.len() > 1 {
            let msg = self.diag(DiagId::sema_excess_initializers, at).arg("3");
            self.emit(msg);
            return self.recover(ty, args, at);
        }
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let loc = self.begin_loc(a);
        let init = self.init_from_expr(fty, a, ConvCtx::Initialization, loc);
        if self.is_err_expr(init) {
            return self.recover(ty, args, at);
        }
        self.builder().paren_list_init_union(ty, init, f, at, rparen)
    }

    /// clang's `err_init_reference_member_uninitialized`: the reference
    /// member `f` of type `ty` takes no argument of a parenthesized
    /// aggregate initialization ([dcl.init.general]/16.6.2.3).
    fn uninit_reference_member(&mut self, f: DeclId, ty: QualType, at: Loc) {
        let t = self.ty_arg(ty);
        let floc = self.ast.decls.get(f).loc();
        let note = self.diag(DiagId::note_uninit_reference_member, floc);
        let msg =
            self.diag(DiagId::sema_init_reference_member_uninitialized, at).arg(t).with_note(note);
        self.emit(msg);
    }

    /// A value-initialized element of a parenthesized aggregate
    /// initialization ([dcl.init.general]/16.6.2.2, /9): a class by its
    /// default constructor (zero-initialized first unless that is
    /// user-provided), anything else as an `ImplicitValueInitExpr`.
    pub(crate) fn paren_value_init(&mut self, ty: QualType, at: Loc) -> ExprId {
        if self.ast.types.as_record(ty).is_some()
            && let Some(e) = self.value_init_class(ty, at)
        {
            return e;
        }
        self.builder().implicit_value_init(ty)
    }

    /// Whether `e` is a prvalue of the class type `ty`.
    fn is_prvalue_of_class(&self, e: ExprId, ty: QualType) -> bool {
        self.ast.exprs.get(e).value_category() == ValueCategory::PRValue
            && self
                .ast
                .types
                .as_record(self.ety(e))
                .zip(self.ast.types.as_record(ty))
                .is_some_and(|(a, b)| self.ast.first(a) == self.ast.first(b))
    }

    /// Whether `e` has the class type `ty` or a class derived from it.
    fn is_class_or_derived(&self, e: ExprId, ty: QualType) -> bool {
        self.ast.types.as_record(self.ety(e)).zip(self.ast.types.as_record(ty)).is_some_and(
            |(a, b)| self.ast.first(a) == self.ast.first(b) || conv::is_derived_from(self, a, b),
        )
    }

    /// Runs `f` with the outermost constructor call built for a base
    /// subobject (clang `EK_Base`): the base-object variant is called
    /// (Itanium C2; spec/sema-class.md §5.6). `f` must reach a
    /// constructor of the base itself.
    fn as_base_subobject<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.ctor_entity = Some((
            crate::class::InitEntity::Base { inherited_virtual: false },
            kiln_ast::expr::ConstructionKind::NonVirtualBase,
        ));
        let r = f(self);
        self.ctor_entity = None;
        r
    }

    /// Whether a braced or value-initialization of the base `bty` is by a
    /// constructor of `bty` itself (not an aggregate initialization,
    /// whose elements are complete objects).
    fn base_inits_by_ctor(&self, bty: QualType) -> bool {
        self.ast.types.as_record(bty).is_some() && !self.is_aggregate(bty)
    }

    /// A value-initialized or braced base element: a constructor call
    /// builds the base subobject.
    fn init_base_list(&mut self, bty: QualType, f: impl FnOnce(&mut Self) -> ExprId) -> ExprId {
        if self.base_inits_by_ctor(bty) { self.as_base_subobject(f) } else { f(self) }
    }

    /// A base element initialized from the expression `e`. A source of
    /// the base class or a class derived from it is copied or moved by
    /// the base-object constructor; a prvalue of the base class is
    /// materialized first (clang does not elide into a base subobject,
    /// which may overlap other members). Anything else converts as a
    /// complete object (a converting constructor's temporary).
    fn init_base_element(&mut self, bty: QualType, e: ExprId) -> ExprId {
        let ety = self.ety(e);
        let loc = self.begin_loc(e);
        if self.is_class_or_derived(e, bty) {
            let m = if self.is_prvalue_of_class(e, bty) {
                self.materialize_temporary(e, ety, false)
            } else {
                e
            };
            let x = self.as_base_subobject(|s| {
                s.construct_from_class(m, bty, ConvCtx::Initialization, loc, DeletedAs::Call)
            });
            if let Some(x) = x {
                return x;
            }
        }
        self.init_from_expr(bty, e, ConvCtx::Initialization, loc)
    }

    /// The direct base classes of the aggregate `def`, as the types of
    /// its first elements ([dcl.init.aggr]/2).
    pub(crate) fn init_bases(&self, def: DeclId) -> Vec<QualType> {
        self.ast.decls.bases(def, &self.ast.lists).map(|b| b.ty).collect()
    }

    /// The members of a class that take part in aggregate initialization:
    /// fields, without unnamed bit-fields ([dcl.init.aggr]/2).
    pub(crate) fn init_fields(&self, r: DeclId) -> Vec<DeclId> {
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
        self.init_from_list_at(ty, l, None)
    }

    /// List-initialization of an entity at `entity` (its name, or the
    /// start of the expression for temporaries, return values and
    /// assignments): too many elements for a scalar are reported there
    /// (clang's `FK_TooManyInitsForScalar`; a scalar member's nested list
    /// passes its `{`); nested aggregate lists report at the excess
    /// element.
    pub(crate) fn init_from_list_at(
        &mut self,
        ty: QualType,
        l: &InitList,
        entity: Option<Loc>,
    ) -> ExprId {
        // `_Atomic(T)` takes no braces (clang `InitListChecker`), but a
        // single element of class type initializes it (WP-165).
        if self.ast.types.as_atomic(ty).is_some() {
            if let [InitItem { value: InitValue::Expr(x), designator }] = l.items.as_slice()
                && designator.is_empty()
                && self.ast.types.as_record(self.ety(*x)).is_some()
            {
                let at = entity.unwrap_or(l.lbrace);
                return self.init_from_expr(ty, *x, ConvCtx::Initialization, at);
            }
            let msg =
                self.diag(DiagId::sema_illegal_initializer_type, l.lbrace).arg(self.ty_arg(ty));
            self.emit(msg);
            return self.recover(ty, &[], l.lbrace);
        }
        if let Some(r) = self.ast.types.as_record(ty) {
            // Whether it is an aggregate needs its definition.
            self.instantiate_for_init(r, l.lbrace);
        }
        if self.is_aggregate(ty) {
            // [dcl.init.list]/3.2: an aggregate class initialized from a
            // single element of the same or a derived class type is
            // initialized from that element.
            if let Some(r) = self.ast.types.as_record(ty)
                && let [InitItem { value: InitValue::Expr(x), designator }] = l.items.as_slice()
                && designator.is_empty()
                && let Some(u) = self.ast.types.as_record(self.ety(*x))
                && self.derived_inclusive(u, r)
            {
                return self.init_from_expr(ty, *x, ConvCtx::Initialization, l.lbrace);
            }
            if self.ast.types.as_record(ty).is_some() && !self.require_complete(ty, l.lbrace) {
                let msg = self
                    .diag(DiagId::sema_typecheck_decl_incomplete_type, l.lbrace)
                    .arg(self.ty_arg(ty));
                self.emit(self.incomplete_note(msg, ty));
                return self.recover(ty, &[], l.lbrace);
            }
            let mut pos = 0;
            let e = self.check_aggregate(ty, &l.items, &mut pos, true, l.lbrace, l.rbrace);
            if pos < l.items.len() {
                self.excess(ty, &l.items[pos]);
            }
            if self.in_template() && l.items.iter().any(|i| !i.designator.is_empty()) {
                // Instantiation rebuilds a designated list as written.
                self.note_syntactic_list(e, l);
            }
            return e;
        }
        if self.is_vector_type(ty) {
            return self.vector_list_init(ty, l);
        }
        if let Some(e) =
            self.list_construct(ty, l, InitStyle::CopyList, entity.unwrap_or(l.lbrace), None)
        {
            return e;
        }
        let designated = l.items.iter().any(|i| !i.designator.is_empty());
        if self.ast.types.is_reference(ty) {
            return self.reference_list_init(ty, l, entity, InitStyle::CopyList);
        }
        // Scalar ([dcl.init.list]/3.9-10).
        let at = entity.unwrap_or(l.lbrace);
        if designated && !self.is_error_type(ty) {
            return self.designated_non_aggregate(ty, l, at);
        }
        if let Some(e) = self.class_element_list_init(ty, l, ConvCtx::Initialization, at, None) {
            return e;
        }
        let mut inits = Vec::new();
        // (A designated list was rejected above.)
        match l.items.first() {
            Some(InitItem { value: InitValue::Expr(e), designator }) if designator.is_empty() => {
                self.check_narrowing(*e, ty, ty);
                inits.push(self.convert_for_init(*e, ty, ConvCtx::Initialization));
            }
            Some(InitItem { value: InitValue::List(inner), designator })
                if designator.is_empty() =>
            {
                // `int x = {{1}};` ([dcl.init.list]/3.9 with a nested
                // list): clang warns and initializes from the inner list.
                let msg = self.diag(DiagId::sema_many_braces_around_scalar, inner.lbrace);
                self.emit(msg);
                let e = self.init_from_list_at(ty, inner, entity);
                let list = self.ast.exprs.init_list_inits(e);
                inits.extend(self.ast.lists.iter::<ExprId>(list));
            }
            _ => {}
        }
        if let Some(extra) = l.items.get(1) {
            let at = entity.unwrap_or_else(|| extra.value.loc(self));
            let msg = self.diag(DiagId::sema_excess_initializers, at).arg("2");
            self.emit(self.ref_temp_noted(msg));
        }
        self.builder().init_list(ty, &inits, None, None, l.lbrace, l.rbrace)
    }

    /// List-initialization of a vector (clang `InitListChecker::
    /// CheckVectorType`, C++ rules): a leading non-list vector element
    /// copy-initializes the whole vector; otherwise each element
    /// initializes one vector element, the rest are zero (no filler in
    /// the list, as clang), and an element past the last is "excess
    /// elements in vector initializer".
    fn vector_list_init(&mut self, ty: QualType, l: &InitList) -> ExprId {
        let top = self.ast.types.desugar_top(ty);
        let (elem, count) = match self.ast.types.view(top.ty()) {
            kiln_ast::types::TypeView::Vector { elem, count, .. } => (elem, count as usize),
            _ => return self.recover(ty, &[], l.lbrace),
        };
        let mut inits = Vec::new();
        let mut pos = 0;
        if let Some(InitItem { value: InitValue::Expr(e), designator }) = l.items.first()
            && designator.is_empty()
            && self.is_vector_type(self.ety(*e))
        {
            let at = self.begin_loc(*e);
            let x = self.init_from_expr(ty, *e, ConvCtx::Initialization, at);
            if self.is_err_expr(x) {
                return self.recover(ty, &[x], l.lbrace);
            }
            inits.push(x);
            pos = 1;
        } else {
            let Some(xs) = self.vector_elements(elem, count, &l.items, &mut pos, true) else {
                return self.recover(ty, &[], l.lbrace);
            };
            inits = xs;
        }
        if let Some(extra) = l.items.get(pos) {
            let msg = self.diag(DiagId::sema_excess_initializers, extra.value.loc(self)).arg("1");
            self.emit(self.ref_temp_noted(msg));
        }
        self.builder().init_list(ty, &inits, None, None, l.lbrace, l.rbrace)
    }

    /// Up to `count` vector elements of type `elem` from `items[*pos..]`
    /// (clang `CheckVectorType`), one item each. A designator is not
    /// supported in a braced vector list (`None`, diagnosed) and ends a
    /// brace-elided one after its first item.
    fn vector_elements(
        &mut self,
        elem: QualType,
        count: usize,
        items: &[InitItem],
        pos: &mut usize,
        braced: bool,
    ) -> Option<Vec<ExprId>> {
        let mut inits = Vec::new();
        while inits.len() < count && *pos < items.len() {
            let item = &items[*pos];
            // The designator of the item that started a brace-elided
            // list names this vector, not one of its elements.
            if !item.designator.is_empty() && (braced || !inits.is_empty()) {
                if braced {
                    self.unsupported("a designator in a vector initializer", item.value.loc(self));
                    return None;
                }
                break;
            }
            let x = match &item.value {
                InitValue::Expr(e) => self.vector_element(*e, elem),
                InitValue::List(inner) => self.init_from_list(elem, inner),
            };
            inits.push(x);
            *pos += 1;
        }
        Some(inits)
    }

    /// One vector element of type `elem` from `e` (clang
    /// `CheckScalarType` with a vector-element entity): a vector operand
    /// is "cannot initialize a vector element".
    fn vector_element(&mut self, e: ExprId, elem: QualType) -> ExprId {
        let et = self.ety(e);
        if self.is_vector_type(et) && !self.expr_is_type_dependent(e) {
            let lv = self.ast.exprs.get(e).value_category() == ValueCategory::LValue;
            let at = self.begin_loc(e);
            let msg = self
                .diag(DiagId::sema_init_vector_element_failed, at)
                .arg(self.ty_arg(elem))
                .arg(if lv { "1" } else { "0" })
                .arg(self.ty_arg(et));
            self.emit(msg);
            return self.recover(elem, &[e], at);
        }
        self.check_narrowing(e, elem, elem);
        self.convert_for_init(e, elem, ConvCtx::Initialization)
    }

    /// A brace-elided GNU vector member ([dcl.init.aggr]/16; clang
    /// `CheckImplicitInitList` of a vector): up to N scalar items.
    fn elided_vector_init(
        &mut self,
        ty: QualType,
        items: &[InitItem],
        pos: &mut usize,
        at: Loc,
    ) -> ExprId {
        let top = self.ast.types.desugar_top(ty);
        let (elem, count) = match self.ast.types.view(top.ty()) {
            kiln_ast::types::TypeView::Vector { elem, count, .. } => (elem, count as usize),
            _ => return self.recover(ty, &[], at),
        };
        let inits = self.vector_elements(elem, count, items, pos, false).unwrap_or_default();
        self.builder().init_list(ty, &inits, None, None, at, at)
    }

    /// List-initialization of reference `ty` ([dcl.init.list]/3.9-10):
    /// a single element the referee is reference-related to initializes
    /// the reference directly; otherwise a prvalue of the referenced type
    /// is copy-list-initialized and the reference binds to it (a
    /// non-const lvalue reference cannot).
    fn reference_list_init(
        &mut self,
        ty: QualType,
        l: &InitList,
        entity: Option<Loc>,
        style: InitStyle,
    ) -> ExprId {
        let r = self.pointee_sugared(ty).unwrap_or(QualType::ERROR);
        let ur = conv::strip_cv(self, r);
        let cr = self.ast.types.canonical(ur);
        let at = entity.unwrap_or(l.lbrace);
        // A reference to an aggregate takes a designated list (as clang
        // and GCC); otherwise the reference type is named.
        if l.items.iter().any(|i| !i.designator.is_empty())
            && !self.is_aggregate(ur)
            && !self.is_error_type(ur)
        {
            return self.designated_non_aggregate(ty, l, at);
        }
        let lref =
            matches!(self.ast.types.canonical_view(ty), kiln_ast::types::TypeView::LValueRef(_));
        let q = self.ast.types.effective_quals(r);
        // A non-const (or volatile) lvalue reference binds no temporary.
        let no_temp = lref && (!q.is_const() || q.is_volatile());
        if let [InitItem { value: InitValue::Expr(e), designator }] = l.items.as_slice()
            && designator.is_empty()
        {
            let e = *e;
            let et = conv::strip_cv(self, self.ety(e));
            let ce = self.ast.types.canonical(et);
            let lvalue = self.ast.exprs.get(e).value_category() == ValueCategory::LValue;
            if self.expr_is_type_dependent(e)
                || (self.ref_related(cr, ce).0 && (lvalue || !no_temp))
            {
                return self.init_from_expr(ty, e, ConvCtx::Initialization, at);
            }
        }
        if no_temp {
            // clang's `FK_NonConstLValueReferenceBindingToTemporary` with
            // an initializer list argument.
            let msg = self
                .diag(DiagId::sema_lvalue_ref_bind_to_initlist, at)
                .arg(u8::from(q.is_const()))
                .arg(self.ty_arg(r));
            self.emit(msg);
            let flat = Self::list_exprs(l);
            return self.recover(ty, &flat, l.lbrace);
        }
        // The temporary is list-initialized as the reference is
        // ([dcl.init.list]/3.10: copy- or direct-), and a failed
        // constructor call names the reference type.
        self.ctor_diag_ty = Some(ty);
        let tmp = self.list_construct(r, l, style, at, None);
        self.ctor_diag_ty = None;
        let tmp = if let Some(t) = tmp {
            t
        } else {
            // An aggregate temporary with too many elements gets
            // clang's `diagnoseListInit` reference note.
            let agg = self.ast.types.is_array(r) || self.ast.types.as_record(r).is_some();
            let saved = self.ref_temp_note.replace(agg.then_some((r, at)));
            let t = self.init_from_list_at(r, l, entity);
            self.ref_temp_note.set(saved);
            t
        };
        self.init_from_expr(ty, tmp, ConvCtx::Initialization, at)
    }

    /// `E e{v};` for an enumeration `E` with a fixed underlying type `U`
    /// and `v` implicitly convertible to `U` ([dcl.init.list]/3.8): `v`
    /// is converted to `U` (narrowing rejected), then to `E`.
    pub(crate) fn fixed_enum_list_init(&mut self, ty: QualType, l: &InitList) -> Option<ExprId> {
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
        self.check_narrowing_impl(e, to, entity);
    }

    /// [`Sema::check_narrowing`] of a constructor argument from a list.
    pub(crate) fn check_list_narrowing(&mut self, e: ExprId, to: QualType) {
        self.check_narrowing_impl(e, to, to);
    }

    fn check_narrowing_impl(&mut self, e: ExprId, to: QualType, entity: QualType) {
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
        self.emit(self.ref_temp_noted(msg));
    }

    /// Adds the `in initialization of temporary ... created to
    /// list-initialize this reference` note while a reference's
    /// aggregate temporary is list-initialized (once).
    fn ref_temp_noted(&self, d: kiln_diag::Diag) -> kiln_diag::Diag {
        match self.ref_temp_note.take() {
            Some((t, at)) => {
                let n =
                    self.diag(DiagId::note_reference_temporary_list_init, at).arg(self.ty_arg(t));
                d.with_note(n)
            }
            None => d,
        }
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
                // `.m{...}` (no `=`) is located at the designator, as
                // clang; `.m = {...}` and `{...}` at the brace.
                let entity = match items[*pos].designator.first() {
                    Some(Designator::Field(_, d)) if !self.preceded_by_equal(sub.lbrace) => {
                        Some(self.period_before(*d))
                    }
                    _ => None,
                };
                *pos += 1;
                // A scalar member's nested list reports too many
                // elements at its `{` (clang's `FK_TooManyInitsForScalar`
                // for the member entity, whose location is the list),
                // unless a `.m{...}` designator located it.
                let scalar = self.is_scalar(ty);
                let at = entity.or(scalar.then_some(sub.lbrace));
                self.init_from_list_at(ty, sub, at)
            }
            InitValue::Expr(e) => {
                let e = *e;
                let ety = self.ast.exprs.get(e).ty();
                let elide = if self.ast.types.is_array(ty) {
                    let elem = self.ast.types.element(ty).unwrap_or(QualType::ERROR);
                    !(self.is_char_type(elem) && self.string_lit_of(e).is_some())
                } else if let Some(r) = self.ast.types.as_record(ty) {
                    // Only an aggregate takes brace elision; a class with
                    // a user constructor is copy-initialized from the
                    // element ([dcl.init.aggr]/16).
                    self.is_aggregate(ty)
                        && self.ast.types.as_record(ety).is_none_or(|x| {
                            self.ast.first(x) != self.ast.first(r)
                                && !conv::is_derived_from(self, x, r)
                        })
                        && !self.is_error_type(ety)
                } else if let Some(p) = self.vector_parts(ty) {
                    // A GNU vector is not initialized from a scalar (no
                    // implicit splat), so brace elision is assumed; an
                    // ext vector splats the scalar (clang
                    // `CheckSubElementType`).
                    !p.is_ext()
                        && !self.is_vector_type(ety)
                        && self.ast.types.as_record(ety).is_none()
                        && !self.is_error_type(ety)
                        && !self.expr_is_type_dependent(e)
                } else {
                    false
                };
                if elide && self.is_vector_type(ty) {
                    self.elided_vector_init(ty, items, pos, at)
                } else if elide {
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
        let sub = Sub { braced, entry: None, prior: None };
        self.check_aggregate_in(ty, items, pos, sub, lbrace, rbrace)
    }

    /// [`Sema::check_aggregate`] entering the (sub)aggregate as `sub`
    /// says: a designator path descends into a subobject, where the
    /// following non-designated clauses continue (clang
    /// `CheckDesignatedInitializer`).
    #[allow(clippy::too_many_lines)]
    fn check_aggregate_in(
        &mut self,
        ty: QualType,
        items: &[InitItem],
        pos: &mut usize,
        sub: Sub,
        lbrace: Loc,
        rbrace: Loc,
    ) -> ExprId {
        if self.ast.types.is_array(ty) {
            return self.check_array(ty, items, pos, sub, lbrace, rbrace);
        }
        let record = self.ast.types.as_record(ty);
        if let Some(k) = sub.entry
            && record.is_none_or(|_| !self.is_aggregate(ty))
        {
            match record {
                Some(_) => self.unsupported("a designator into a non-aggregate member", lbrace),
                None => self.designator_mismatch(ty, &items[*pos].designator[k]),
            }
            *pos += 1;
            return self.recover(ty, &[], lbrace);
        }
        let Some(r) = record else {
            return self.recover(ty, &[], lbrace);
        };
        let fields = self.init_fields(r);
        let def = self.ast.definition(r).unwrap_or(r);
        let union = self.ast.decls.record(def).is_some_and(|x| x.tag == TagKind::Union);
        // [dcl.init.aggr]/2: the direct bases, in declaration order, then
        // the fields (a union has no bases).
        let bases = self.init_bases(def);
        let nb = bases.len();
        let total = nb + fields.len();
        let mut slots: Vec<Option<ExprId>> = vec![None; total];
        // Whether a slot was initialized by a clause (not a filler).
        let mut explicit = vec![false; total];
        let mut union_field = None;
        if let Some(p) = self.prior_list(sub.prior) {
            let inits: Vec<ExprId> =
                self.ast.lists.iter(self.ast.exprs.init_list_inits(p)).collect();
            let uf = self.ast.exprs.init_list(p).union_field;
            if union {
                if let (Some(f), Some(&e)) = (uf, inits.first())
                    && let Some(i) = fields.iter().position(|&x| x == f)
                {
                    slots[i] = Some(e);
                    explicit[i] = !self.is_filler_expr(e);
                    union_field = Some(f);
                }
            } else if inits.len() == total {
                for (i, &e) in inits.iter().enumerate() {
                    slots[i] = Some(e);
                    explicit[i] = !self.is_filler_expr(e);
                }
            }
        }
        let mut entry = sub.entry;
        let mut cur = 0usize;
        while *pos < items.len() {
            let item = &items[*pos];
            let k = match entry.take() {
                Some(k) => Some(k),
                None if item.designator.is_empty() => None,
                None if !sub.braced => break,
                None => Some(0),
            };
            let mut descend = None;
            if let Some(k) = k {
                let (name, dloc) = match item.designator[k] {
                    Designator::Field(n, l) => (n, l),
                    ref d @ Designator::Index(..) => {
                        self.designator_mismatch(ty, d);
                        *pos += 1;
                        continue;
                    }
                };
                let Some((idx, direct)) = self.designated_field(&fields, name) else {
                    let msg = self
                        .diag(DiagId::sema_field_designator_unknown, dloc)
                        .arg(self.ast.symbols().as_str(name).to_string())
                        .arg(self.ty_arg(ty));
                    self.emit(msg);
                    *pos += 1;
                    continue;
                };
                // A member of an anonymous struct or union is designated
                // through it (clang's implicit designator for an
                // `IndirectFieldDecl`).
                if !direct {
                    descend = Some(k);
                } else if k + 1 < item.designator.len() {
                    descend = Some(k + 1);
                }
                // [dcl.init.aggr]/3.1: designators in declaration order
                // (clang `ext_designated_init_reordered`, checked for the
                // designators of the list itself).
                if sub.braced && cur > nb && cur - nb - 1 > idx {
                    let prev = fields[cur - nb - 1];
                    let msg = self
                        .diag(DiagId::sema_designated_init_reordered, self.value_begin(item))
                        .arg(self.field_arg(fields[idx]))
                        .arg(self.field_arg(prev));
                    let msg = match slots[cur - 1].filter(|_| explicit[cur - 1]) {
                        Some(p) => msg.with_note(
                            self.diag(DiagId::note_previous_field_init, self.begin_loc(p))
                                .arg(self.field_arg(prev)),
                        ),
                        None => msg,
                    };
                    self.emit(msg);
                }
                cur = nb + idx;
            }
            if cur >= total || (union && union_field.is_some() && item.designator.is_empty()) {
                if sub.braced {
                    self.excess(ty, item);
                    *pos = items.len();
                }
                break;
            }
            if union
                && let Some(g) = union_field
                && g != fields[cur]
            {
                // Another member of the union was initialized.
                if let Some(old) = slots.iter().zip(&explicit).find_map(|(s, &x)| s.filter(|_| x)) {
                    let msg = self
                        .diag(DiagId::sema_initializer_union_overrides, self.item_begin(item))
                        .arg("0");
                    let note = self.diag(DiagId::note_init_previous, self.begin_loc(old));
                    self.emit(msg.with_note(note));
                }
                slots = vec![None; total];
                explicit = vec![false; total];
                union_field = None;
            }
            let ety = if cur < nb {
                bases[cur]
            } else {
                self.ast.decls.get(fields[cur - nb]).ty().unwrap_or(QualType::ERROR)
            };
            let e = if let Some(k2) = descend {
                let prior = self.descend_prior(slots[cur], explicit[cur], item);
                let at = self.designator_begin(&item.designator[0]);
                let s = Sub { braced: false, entry: Some(k2), prior };
                self.check_aggregate_in(ety, items, pos, s, at, at)
            } else {
                if explicit[cur]
                    && let Some(old) = slots[cur]
                {
                    self.override_warning(item, old);
                }
                match items[*pos].value {
                    InitValue::Expr(x) if cur < nb && self.is_class_or_derived(x, ety) => {
                        *pos += 1;
                        self.init_base_element(ety, x)
                    }
                    InitValue::List(_) if cur < nb => {
                        self.init_base_list(ety, |s| s.init_element(ety, items, pos, lbrace))
                    }
                    _ => self.init_element(ety, items, pos, lbrace),
                }
            };
            slots[cur] = Some(e);
            explicit[cur] = true;
            if union {
                union_field = Some(fields[cur]);
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
                let e = match slot {
                    Some(e) => e,
                    None if i < nb => {
                        let b = bases[i];
                        self.init_base_list(b, |s| s.filler(b, rbrace))
                    }
                    None => self.field_filler(fields[i - nb], rbrace),
                };
                inits.push(e);
            }
        }
        self.builder().init_list(ty, &inits, None, union_field, lbrace, rbrace)
    }

    #[allow(clippy::too_many_lines)]
    fn check_array(
        &mut self,
        ty: QualType,
        items: &[InitItem],
        pos: &mut usize,
        sub: Sub,
        lbrace: Loc,
        rbrace: Loc,
    ) -> ExprId {
        let elem = self.ast.types.element(ty).unwrap_or(QualType::ERROR);
        let bound = self.ast.types.array_size(ty);
        let mut slots: Vec<Option<ExprId>> = Vec::new();
        let mut explicit: Vec<bool> = Vec::new();
        if let Some(p) = self.prior_list(sub.prior) {
            for e in
                self.ast.lists.iter::<ExprId>(self.ast.exprs.init_list_inits(p)).collect::<Vec<_>>()
            {
                slots.push(Some(e));
                explicit.push(!self.is_filler_expr(e));
            }
        }
        let mut entry = sub.entry;
        let mut cur: u64 = 0;
        while *pos < items.len() {
            let item = &items[*pos];
            let k = match entry.take() {
                Some(k) => Some(k),
                None if item.designator.is_empty() => None,
                None if !sub.braced => break,
                None => Some(0),
            };
            let mut descend = None;
            if let Some(k) = k {
                let (ie, dloc) = match item.designator[k] {
                    Designator::Index(ie, l) => (ie, l),
                    ref d @ Designator::Field(..) => {
                        self.designator_mismatch(ty, d);
                        *pos += 1;
                        continue;
                    }
                };
                if k + 1 < item.designator.len() {
                    descend = Some(k + 1);
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
                if sub.braced {
                    self.excess(ty, item);
                    *pos = items.len();
                }
                break;
            }
            let i = usize::try_from(cur).unwrap_or(usize::MAX);
            if slots.len() <= i {
                slots.resize(i + 1, None);
                explicit.resize(i + 1, false);
            }
            let e = if let Some(k2) = descend {
                let prior = self.descend_prior(slots[i], explicit[i], item);
                let at = self.designator_begin(&item.designator[0]);
                let s = Sub { braced: false, entry: Some(k2), prior };
                self.check_aggregate_in(elem, items, pos, s, at, at)
            } else {
                if explicit[i]
                    && let Some(old) = slots[i]
                {
                    self.override_warning(item, old);
                }
                self.init_element(elem, items, pos, lbrace)
            };
            slots[i] = Some(e);
            explicit[i] = true;
            cur += 1;
        }
        let n = bound.unwrap_or(slots.len() as u64);
        let mut inits = Vec::with_capacity(slots.len());
        // A hole left by array designators also gives the list a filler
        // (clang `FillInEmptyInitializations`).
        let hole = slots.iter().any(Option::is_none);
        for slot in slots {
            let e = match slot {
                Some(e) => e,
                None => self.filler(elem, rbrace),
            };
            inits.push(e);
        }
        let filler =
            if hole || (inits.len() as u64) < n { Some(self.filler(elem, rbrace)) } else { None };
        let lty = if bound.is_none() {
            let complete = self.ast.types.constant_array(elem, n, None);
            let q = self.ast.types.effective_quals(ty);
            if q.is_empty() { complete } else { self.ast.types.qualify(complete, q) }
        } else {
            ty
        };
        self.builder().init_list(lty, &inits, filler, None, lbrace, rbrace)
    }

    /// "field designator cannot initialize a non-class type" /
    /// "array designator cannot initialize non-array type" for the
    /// designator `d` applied to `ty`.
    fn designator_mismatch(&mut self, ty: QualType, d: &Designator) {
        if self.is_error_type(ty) {
            return;
        }
        let (id, at) = match *d {
            Designator::Field(_, l) => {
                (DiagId::sema_field_designator_non_record, self.period_before(l))
            }
            Designator::Index(_, l) => (DiagId::sema_array_designator_non_array, l),
        };
        let msg = self.diag(id, at).arg(self.ty_arg(ty));
        self.emit(msg);
    }

    /// The field of `fields` the designator `.name` designates: its index
    /// and whether it is the field itself (false: an anonymous struct or
    /// union member containing it).
    fn designated_field(&self, fields: &[DeclId], name: Symbol) -> Option<(usize, bool)> {
        if let Some(i) =
            fields.iter().position(|&f| self.ast.decls.get(f).name().as_ident() == Some(name))
        {
            return Some((i, true));
        }
        fields
            .iter()
            .position(|&f| self.is_anonymous_member(f) && self.anonymous_contains(f, name))
            .map(|i| (i, false))
    }

    /// Whether anonymous member `f` has a member `name` (directly or in
    /// a nested anonymous member).
    fn anonymous_contains(&self, f: DeclId, name: Symbol) -> bool {
        let Some(r) = self.ast.decls.get(f).ty().and_then(|t| self.ast.types.as_record(t)) else {
            return false;
        };
        self.init_fields(r).into_iter().any(|g| {
            self.ast.decls.get(g).name().as_ident() == Some(name)
                || (self.is_anonymous_member(g) && self.anonymous_contains(g, name))
        })
    }

    /// A field as a diagnostic argument (an anonymous member by type).
    fn field_arg(&self, f: DeclId) -> String {
        if self.ast.decls.get(f).name().is_empty() {
            // clang names an anonymous member by its type, without `aka`.
            let names = crate::names::SemaNames::new(self);
            let t = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            let s = kiln_ast::types::print_type(
                &self.ast.types,
                &names,
                t,
                kiln_ast::types::PrintOptions::SUGARED,
            );
            format!("'{s}'")
        } else {
            self.decl_arg(f)
        }
    }

    /// Where the initializer of a clause begins (after its designators).
    fn value_begin(&self, item: &InitItem) -> Loc {
        match &item.value {
            InitValue::List(l) => l.lbrace,
            InitValue::Expr(e) => self.begin_loc(*e),
        }
    }

    /// A filler a later designator may replace silently.
    fn is_filler_expr(&self, e: ExprId) -> bool {
        matches!(
            self.ast.exprs.get(e).kind(),
            ExprKind::ImplicitValueInitExpr | ExprKind::CXXDefaultInitExpr
        )
    }

    /// `prior` if it is an initializer list (a subaggregate to merge into).
    fn prior_list(&self, prior: Option<ExprId>) -> Option<ExprId> {
        prior.filter(|&p| self.ast.exprs.get(p).kind() == ExprKind::InitListExpr)
    }

    /// The prior initializer a designator path descending into a slot
    /// merges into. A subobject initialized by an expression would need
    /// clang's `DesignatedInitUpdateExpr`.
    fn descend_prior(
        &self,
        slot: Option<ExprId>,
        explicit: bool,
        item: &InitItem,
    ) -> Option<ExprId> {
        let old = slot?;
        if explicit && self.ast.exprs.get(old).kind() != ExprKind::InitListExpr {
            self.unsupported(
                "a designator into a subobject initialized by an expression",
                self.value_begin(item),
            );
            return None;
        }
        Some(old)
    }

    /// "initializer overrides prior initialization of this subobject"
    /// ([dcl.init.aggr]/3.1 forbids it; clang `ext_initializer_overrides`).
    fn override_warning(&mut self, item: &InitItem, old: ExprId) {
        let msg = self.diag(DiagId::sema_initializer_overrides, self.value_begin(item)).arg("1");
        let note = self.diag(DiagId::note_init_previous, self.begin_loc(old));
        self.emit(msg.with_note(note));
    }
}

/// How [`Sema::check_aggregate_in`] enters a (sub)aggregate.
#[derive(Clone, Copy)]
struct Sub {
    /// Whether the subaggregate has its own braces; under brace elision
    /// or a designator path a designated clause ends it.
    braced: bool,
    /// The designator of the current clause that applies within this
    /// subaggregate (a designator path entering it).
    entry: Option<usize>,
    /// The subaggregate's initializer so far, when a designator path
    /// re-enters it: its elements are kept.
    prior: Option<ExprId>,
}
