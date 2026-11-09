//! The lifetime analysis of an initialization, `return` or assignment:
//! which temporaries and locals the initialized entity keeps referring to
//! after they die (clang 19 `CheckExprLifetime.cpp`; spec/sema-class.md
//! §5.6).
//!
//! The initializer is walked the way clang's
//! `visitLocalsRetainedByReferenceBinding` (a glvalue bound by a
//! reference) and `visitLocalsRetainedByInitializer` (a prvalue) do,
//! recording a path of the steps taken: `&` and array decay, the
//! initializer of a reference or `const` variable, a default member
//! initializer, a lambda capture, an argument bound to a
//! `[[clang::lifetimebound]]` parameter (or the implicit object of a
//! `lifetimebound` member function), a copy of a temporary, and the
//! `[[gsl::Pointer]]` / `[[gsl::Owner]]` steps (a Pointer built from an
//! Owner, a Pointer or reference returned by an Owner's member). Each
//! local found (a `MaterializeTemporaryExpr`, an automatic variable or
//! parameter, a GNU `&&label`) is judged by the entity's lifetime:
//!
//! - a variable (or a subobject of one): a temporary on a path that does
//!   not extend it dangles (`-Wdangling`, `-Wdangling-gsl`);
//! - a mem-initializer: a temporary dangles (`-Wdangling-field`, the
//!   reference-member error), and so does a parameter;
//! - a `return`: a local or a temporary dangles (`-Wreturn-stack-address`);
//! - an assignment to a pointer or a gsl Pointer: a temporary dangles
//!   (`-Wdangling-assignment`, `-Wdangling-assignment-gsl`).
//!
//! The diagnostic is placed at the first variable, default member
//! initializer or capture on the path (else at the local), followed by a
//! note per such step. Lifetime extension itself is done by
//! [`Sema::extend_temporaries`]; a path that would extend a temporary is
//! not diagnosed here.

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{BinaryOp, CastKind, ExprFlags, ExprKind, StorageDuration, UnaryOp};
use kiln_ast::lambda::{CaptureKind, LambdaCapture};
use kiln_ast::names::OverloadedOperator as OO;
use kiln_ast::types::TypeKind;
use kiln_ast::{AttrId, DeclId, ExprId, ListId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use super::super::special::SpecialKind;
use super::gsl::Gsl;
use crate::Sema;

/// The recursion bound of the walk.
const MAX_DEPTH: u32 = 256;

/// The lifetime of the initialized entity (clang `LifetimeKind`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Lk {
    /// A variable, or a subobject of one: temporaries bound to it are
    /// extended.
    Extended,
    /// A mem-initializer.
    MemInit,
    /// A `return` operand.
    Return,
    /// The right operand of an assignment.
    Assign,
    /// The initializer of a new-expression (clang `LK_New`).
    New,
}

/// How a local is referred to (clang `ReferenceKind`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Rk {
    /// Bound by a reference.
    Ref = 0,
    /// The backing array of a `std::initializer_list`.
    InitList = 1,
}

/// A step from the initializer to a local (clang
/// `IndirectLocalPathEntry::EntryKind`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Ek {
    DefaultInit,
    AddressOf,
    VarInit,
    LValToRVal,
    LifetimeBoundCall,
    TemporaryCopy,
    LambdaCaptureInit,
    GslReferenceInit,
    GslPointerInit,
    GslPointerAssignment,
}

/// One path entry: the step's expression, its declaration, and for a
/// capture the capture and the place clang diagnoses it at.
#[derive(Copy, Clone)]
struct Entry {
    kind: Ek,
    e: ExprId,
    d: Option<DeclId>,
    cap: Option<(LambdaCapture, Loc)>,
}

impl Entry {
    fn new(kind: Ek, e: ExprId, d: Option<DeclId>) -> Entry {
        Entry { kind, e, d, cap: None }
    }
}

/// Whether the path extends the temporary at its end (clang
/// `PathLifetimeKind`).
#[derive(Copy, Clone, PartialEq, Eq)]
enum PathKind {
    Extend,
    ShouldExtend,
    NoExtend,
}

/// The state of one check.
struct Cx {
    lk: Lk,
    /// The extending variable, or the mem-initialized (outermost) field.
    extending: Option<DeclId>,
    /// The initialized entity is a subobject of `extending`.
    has_parent: bool,
    /// The initializer is a glvalue.
    init_glvalue: bool,
    /// The initialized entity is a reference.
    entity_is_ref: bool,
    /// The left operand of an assignment.
    lhs: Option<ExprId>,
    path: Vec<Entry>,
    depth: u32,
}

impl Cx {
    fn new(lk: Lk, extending: Option<DeclId>) -> Cx {
        Cx {
            lk,
            extending,
            has_parent: false,
            init_glvalue: false,
            entity_is_ref: false,
            lhs: None,
            path: Vec::new(),
            depth: 0,
        }
    }

    fn on_path(&self, v: DeclId) -> bool {
        self.path.iter().any(|p| p.kind == Ek::VarInit && p.d == Some(v))
    }

    /// clang `pathContainsInit`.
    fn contains_init(&self) -> bool {
        self.path.iter().any(|p| matches!(p.kind, Ek::VarInit | Ek::DefaultInit))
    }

    /// clang `pathOnlyHandlesGslPointer`.
    fn only_gsl(&self) -> bool {
        for p in self.path.iter().rev() {
            match p.kind {
                Ek::VarInit | Ek::AddressOf | Ek::LifetimeBoundCall => {}
                Ek::GslPointerInit | Ek::GslReferenceInit | Ek::GslPointerAssignment => {
                    return true;
                }
                _ => return false,
            }
        }
        false
    }

    /// clang `shouldLifetimeExtendThroughPath`.
    fn path_kind(&self) -> PathKind {
        let mut k = PathKind::Extend;
        for p in &self.path {
            match p.kind {
                Ek::DefaultInit => k = PathKind::ShouldExtend,
                Ek::LambdaCaptureInit => {}
                _ => return PathKind::NoExtend,
            }
        }
        k
    }
}

/// The visitor a local is handed to: the check's own, or the one an
/// lvalue-to-rvalue conversion installs to look through `const`
/// variables and temporaries (with the lifetime-warnings flag in force
/// where it was installed).
#[derive(Copy, Clone)]
enum Vis<'a> {
    Top,
    ConstInit(&'a Vis<'a>, bool),
}

/// clang `StdOwners`-free name tables of the gsl tracking rules.
const TRACK_PTR_METHODS: [&str; 15] = [
    "begin",
    "rbegin",
    "cbegin",
    "crbegin",
    "end",
    "rend",
    "cend",
    "crend",
    "c_str",
    "data",
    "get",
    "find",
    "equal_range",
    "lower_bound",
    "upper_bound",
];
const TRACK_REF_METHODS: [&str; 5] = ["front", "back", "at", "top", "value"];
const TRACK_PTR_FUNCTIONS: [&str; 9] =
    ["begin", "rbegin", "cbegin", "crbegin", "end", "rend", "cend", "crend", "data"];
const TRACK_REF_FUNCTIONS: [&str; 2] = ["get", "any_cast"];
/// The `std` functions clang 19 makes `lifetimebound` in their parameter
/// (`Sema::inferLifetimeBoundAttribute`).
const STD_LIFETIMEBOUND: [&str; 7] =
    ["addressof", "__addressof", "as_const", "forward", "forward_like", "move", "move_if_noexcept"];

impl Sema<'_, '_> {
    /// The lifetime check of variable `v` initialized by `init` (clang
    /// `checkExprLifetime` for an `EK_Variable` entity and the members
    /// and elements of its aggregate initializer). `v` may be implicit:
    /// a for-range `__range` variable, or a structured binding's holding
    /// variable.
    pub(crate) fn check_var_lifetime(&mut self, v: DeclId, init: ExprId) {
        if self.expr_is_type_dependent(init) {
            return;
        }
        // P2718: the temporaries of a for-range-initializer live as long
        // as `__range` (clang 22; clang 19 still warns).
        if self.range_lifetime_extension
            && self.clang_compat >= 22
            && self.ast.decls.get(v).flags().contains(DeclFlags::IMPLICIT)
            && self.ast.decls.var(v).is_some_and(|x| x.spec.is_cxx_for_range())
        {
            return;
        }
        // [class.temporary]/8 (CWG3043): an enumerating expansion
        // statement's element temporaries all live as long as `v`
        // (`extend_init_temporaries`).
        if self.expansion_element_init {
            return;
        }
        let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        // A dependent entity type is initialized at instantiation (clang's
        // dependent `InitializationSequence` checks nothing).
        if self.ast.types.is_dependent(ty) {
            return;
        }
        let cx = Cx::new(Lk::Extended, Some(v));
        self.check_entity(cx, init, ty, 0);
    }

    /// The lifetime diagnostics of `return c;` in a function returning
    /// `ret` (`c` is the converted operand).
    pub(crate) fn check_return_lifetime(&mut self, c: ExprId, ret: QualType) {
        if self.expr_is_type_dependent(c) || self.ast.types.is_dependent(ret) {
            return;
        }
        self.check_entity(Cx::new(Lk::Return, None), c, ret, 0);
    }

    /// The lifetime check of the mem-initializer of `field` by `init`.
    pub(crate) fn check_member_init_lifetime(&mut self, field: DeclId, init: ExprId) {
        if self.expr_is_type_dependent(init) {
            return;
        }
        let ty = self.ast.decls.get(field).ty().unwrap_or(QualType::ERROR);
        if self.ast.types.is_dependent(ty) {
            return;
        }
        self.check_entity(Cx::new(Lk::MemInit, Some(field)), init, ty, 0);
    }

    /// The lifetime check of a base-class mem-initializer `init` of type
    /// `ty` (clang `EK_Base`): only the members of an aggregate base
    /// initialized by braces are judged, each as its own mem-initialized
    /// field; a temporary bound by the base object as a whole is not
    /// diagnosed (clang's FIXME). A delegating initializer initializes a
    /// non-aggregate and is not checked at all.
    pub(crate) fn check_base_init_lifetime(&mut self, init: ExprId, ty: QualType) {
        if self.expr_is_type_dependent(init) || self.ast.types.is_dependent(ty) {
            return;
        }
        self.check_entity(Cx::new(Lk::MemInit, None), init, ty, 0);
    }

    /// The lifetime check of the initializer `init` of a new-expression
    /// allocating `ty` (clang `LK_New`): a temporary bound to a reference
    /// member or an `std::initializer_list` of the allocated object dies
    /// at the end of the full-expression.
    pub(crate) fn check_new_lifetime(&mut self, init: ExprId, ty: QualType) {
        if self.expr_is_type_dependent(init) || self.ast.types.is_dependent(ty) {
            return;
        }
        self.check_entity(Cx::new(Lk::New, None), init, ty, 0);
    }

    /// The lifetime check of the assignment `lhs = rhs` (built-in, or a
    /// class's `operator=` with `rhs` the converted argument; clang
    /// `checkExprLifetime` for an `AssignedEntity`).
    pub(crate) fn check_assignment_lifetime(&mut self, lhs: ExprId, rhs: ExprId) {
        let lty = self.ast.exprs.get(lhs).ty();
        let lw = self.diags.is_enabled(DiagId::sema_dangling_lifetime_pointer);
        let gsl_ptr = lw && self.type_gsl_kind(lty) == Some(Gsl::Pointer);
        if !(self.ast.types.is_pointer(lty) || gsl_ptr)
            || self.expr_is_type_dependent(lhs)
            || self.expr_is_type_dependent(rhs)
        {
            return;
        }
        let mut cx = Cx::new(Lk::Assign, None);
        cx.lhs = Some(lhs);
        if gsl_ptr {
            cx.path.push(Entry::new(Ek::GslPointerAssignment, rhs, None));
        }
        self.check_leaf(cx, rhs, lty);
    }

    /// The entity of type `ty` initialized by `init`: a braced aggregate
    /// initializer initializes each element as its own entity (clang
    /// checks each one as `InitListChecker` initializes it), braces around
    /// one scalar or same-type initializer initialize the entity itself.
    fn check_entity(&mut self, cx: Cx, init: ExprId, ty: QualType, depth: u32) {
        let e = self.skip_full_expr(init);
        // An invalid entity or initializer (clang marks the declaration
        // invalid and never builds the initialization) is not checked.
        if self.ast.types.is_error(ty) || self.ast.types.is_error(self.ast.exprs.get(e).ty()) {
            return;
        }
        if self.ast.exprs.get(e).kind() != ExprKind::InitListExpr || depth > MAX_DEPTH {
            self.check_leaf(cx, init, ty);
            return;
        }
        let lty = self.ast.exprs.get(e).ty();
        let inits: Vec<ExprId> =
            self.ast.lists.iter::<ExprId>(self.ast.exprs.init_list_inits(e)).collect();
        let aggregate = self.ast.types.is_array(lty)
            || (self.ast.types.as_record(lty).is_some() && !self.ile_transparent(e));
        if !aggregate {
            if let [x] = inits[..] {
                self.check_entity(cx, x, ty, depth + 1);
            }
            return;
        }
        // An element is a subobject of the entity; the members of a
        // base class's aggregate initializer are mem-initialized fields of
        // their own (clang `getEntityLifetime` for `EK_Base`).
        let elem = |cx: &Cx, field: Option<DeclId>| {
            let mut n = Cx::new(cx.lk, cx.extending);
            if cx.lk == Lk::MemInit && cx.extending.is_none() {
                n.extending = field;
            } else {
                n.has_parent = true;
            }
            n.lhs = cx.lhs;
            n
        };
        if self.ast.types.is_array(lty) {
            let ety = self.ast.types.element(lty).unwrap_or(QualType::ERROR);
            // The array filler initializes the remaining elements, once.
            let filler = self.ast.exprs.init_list(e).array_filler;
            for x in inits.into_iter().chain(filler) {
                let n = elem(&cx, None);
                self.check_element(n, x, ety, depth);
            }
            return;
        }
        let Some(r) = self.ast.types.as_record(lty) else { return };
        if let Some(f) = self.ast.exprs.init_list(e).union_field {
            if let Some(&x) = inits.first() {
                let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                let n = elem(&cx, Some(f));
                self.check_element(n, x, fty, depth);
            }
            return;
        }
        let def = self.ast.definition(r).unwrap_or(r);
        let bases: Vec<QualType> =
            self.ast.decls.bases(def, &self.ast.lists).map(|b| b.ty).collect();
        let fields = self.init_fields(r);
        for (i, &x) in inits.iter().enumerate() {
            let (ety, field) = if i < bases.len() {
                (bases[i], None)
            } else if let Some(&f) = fields.get(i - bases.len()) {
                (self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR), Some(f))
            } else {
                break;
            };
            let n = elem(&cx, field);
            self.check_element(n, x, ety, depth);
        }
    }

    /// One element of an aggregate initializer. A default member
    /// initializer used by a variable's initializer extends its
    /// temporaries in clang 20+ and is not checked there; clang 19 warns
    /// that the extension is unsupported (its `ShouldExtend` paths,
    /// spec/sema-class.md §5.6). One used by a mem-initializer or a
    /// new-expression is checked as any other element.
    fn check_element(&mut self, cx: Cx, x: ExprId, ety: QualType, depth: u32) {
        if cx.lk == Lk::Extended
            && self.clang_compat >= 20
            && self.ast.exprs.get(x).kind() == ExprKind::CXXDefaultInitExpr
        {
            return;
        }
        self.check_entity(cx, x, ety, depth + 1);
    }

    /// clang `checkExprLifetimeImpl` for the entity of type `ty`
    /// initialized (or assigned) by `init`.
    fn check_leaf(&mut self, mut cx: Cx, init: ExprId, ty: QualType) {
        let lw = self.diags.is_enabled(DiagId::sema_dangling_lifetime_pointer);
        cx.init_glvalue = self.ast.exprs.get(init).value_category().is_glvalue();
        cx.entity_is_ref = self.ast.types.is_reference(ty);
        if cx.init_glvalue {
            self.by_ref(&mut cx, init, Rk::Ref, &Vis::Top, lw);
        } else {
            let revisit = cx.lk == Lk::Assign;
            self.by_init(&mut cx, init, revisit, &Vis::Top, lw);
        }
    }

    /// `e` without its `ExprWithCleanups` / `ConstantExpr` wrappers
    /// (clang `FullExpr`).
    fn skip_full_expr(&self, mut e: ExprId) -> ExprId {
        for _ in 0..MAX_DEPTH {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ExprWithCleanups | ExprKind::ConstantExpr => {
                    e = self.ast.exprs.operand(e);
                }
                _ => break,
            }
        }
        e
    }

    /// clang `IgnoreParens`: parentheses, `__extension__` and the
    /// chosen operand of `__builtin_choose_expr`.
    fn lt_skip_parens(&self, mut e: ExprId) -> ExprId {
        let ex = &self.ast.exprs;
        for _ in 0..MAX_DEPTH {
            match ex.get(e).kind() {
                ExprKind::ParenExpr => e = ex.operand(e),
                ExprKind::ChooseExpr => match ex.choose_chosen(e) {
                    Some(c) => e = c,
                    None => break,
                },
                ExprKind::UnaryOperator if ex.unary_op(e) == UnaryOp::Extension => {
                    e = ex.operand(e);
                }
                _ => break,
            }
        }
        e
    }

    /// clang `Expr::skipRValueSubobjectAdjustments`.
    fn skip_rvalue_adjustments(&self, mut e: ExprId) -> ExprId {
        let ex = &self.ast.exprs;
        for _ in 0..MAX_DEPTH {
            e = self.lt_skip_parens(e);
            let n = *ex.get(e);
            let next = match n.kind() {
                k if k.is_cast() => match ex.cast_kind(e) {
                    CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase
                        if self.ast.types.as_record(n.ty()).is_some() =>
                    {
                        Some(ex.operand(e))
                    }
                    CastKind::NoOp => Some(ex.operand(e)),
                    _ => None,
                },
                ExprKind::MemberExpr if !n.has(ExprFlags::ARROW) => {
                    let d = ex.decl(e);
                    let fty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
                    let bitfield = self.ast.decls.field(d).is_some_and(|f| f.bit_width.is_some());
                    (self.kind_of(d) == DeclKind::Field
                        && !bitfield
                        && !self.ast.types.is_reference(fty))
                    .then(|| ex.operand(e))
                }
                ExprKind::BinaryOperator => match ex.binary_op(e) {
                    BinaryOp::PtrMemD => Some(ex.operands(e)[0]),
                    BinaryOp::Comma => Some(ex.operands(e)[1]),
                    _ => None,
                },
                _ => None,
            };
            match next {
                Some(x) => e = x,
                None => break,
            }
        }
        e
    }

    /// clang `InitListExpr::isTransparent`: braces around one initializer
    /// of the list's own type (or a glvalue list).
    fn ile_transparent(&self, e: ExprId) -> bool {
        let ex = &self.ast.exprs;
        let n = *ex.get(e);
        if n.value_category().is_glvalue() {
            return true;
        }
        let mut inits = self.ast.lists.iter::<ExprId>(ex.init_list_inits(e));
        let (Some(x), None) = (inits.next(), inits.next()) else { return false };
        let xn = *ex.get(x);
        if xn.value_category().is_glvalue() && self.ast.types.as_record(n.ty()).is_some() {
            return false;
        }
        self.ast.types.canonical(n.ty()) == self.ast.types.canonical(xn.ty())
    }

    /// Whether `d` is a parameter or a variable with automatic storage
    /// duration (clang `VarDecl::hasLocalStorage`).
    fn is_automatic_local(&self, d: DeclId) -> bool {
        match self.kind_of(d) {
            DeclKind::ParmVar => true,
            DeclKind::Var => self.var_storage(d) == StorageDuration::Automatic,
            _ => false,
        }
    }

    /// The true and false operands of a conditional operator; a GNU
    /// `?:`'s true operand is the opaque common operand, which is not
    /// followed (clang `AbstractConditionalOperator`).
    fn lt_cond_arms(&self, e: ExprId) -> [ExprId; 2] {
        if self.ast.exprs.get(e).kind() == ExprKind::BinaryConditionalOperator {
            let [_, _, t, f] = self.ast.exprs.binary_conditional_operands(e, &self.ast.lists);
            [t, f]
        } else {
            self.ast.exprs.cond_arms(e)
        }
    }

    fn is_glvalue(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).value_category().is_glvalue()
    }

    /// clang `visitLocalsRetainedByReferenceBinding`: the locals reachable
    /// through a reference bound to the glvalue `init`.
    fn by_ref(&mut self, cx: &mut Cx, init: ExprId, rk: Rk, vis: &Vis<'_>, lw: bool) {
        if cx.depth > MAX_DEPTH {
            return;
        }
        let len = cx.path.len();
        cx.depth += 1;
        self.by_ref_in(cx, init, rk, vis, lw);
        cx.depth -= 1;
        cx.path.truncate(len);
    }

    fn by_ref_in(&mut self, cx: &mut Cx, mut init: ExprId, rk: Rk, vis: &Vis<'_>, lw: bool) {
        for _ in 0..MAX_DEPTH {
            let old = init;
            init = self.skip_full_expr(init);
            if self.ast.exprs.get(init).kind() == ExprKind::InitListExpr
                && self.ile_transparent(init)
                && let Some(x) = self.first_of(self.ast.exprs.init_list_inits(init))
            {
                init = x;
            }
            init = self.skip_rvalue_adjustments(init);
            if self.ast.exprs.get(init).kind().is_cast() {
                let s = self.ast.exprs.operand(init);
                if self.is_glvalue(s) {
                    init = s;
                }
            }
            if self.ast.exprs.get(init).kind() == ExprKind::ArraySubscriptExpr {
                let [l, r] = self.ast.exprs.operands(init);
                let lt = self.ast.exprs.get(l).ty();
                let base = if self.ast.types.is_pointer(lt) || self.ast.types.is_array(lt) {
                    l
                } else {
                    r
                };
                let b = *self.ast.exprs.get(base);
                if b.kind() == ExprKind::ImplicitCastExpr
                    && self.ast.exprs.cast_kind(base) == CastKind::ArrayToPointerDecay
                {
                    init = self.ast.exprs.operand(base);
                } else {
                    self.by_init(cx, base, true, vis, lw);
                    return;
                }
            }
            if self.ast.exprs.get(init).kind() == ExprKind::CXXDefaultInitExpr {
                let f = self.ast.exprs.default_init_field(init);
                let Some(x) = self.ast.decls.field(f).and_then(|d| d.init) else { return };
                cx.path.push(Entry::new(Ek::DefaultInit, init, Some(f)));
                init = x;
            }
            if init == old {
                break;
            }
        }
        let n = *self.ast.exprs.get(init);
        match n.kind() {
            ExprKind::MaterializeTemporaryExpr => {
                if self.visit(cx, vis, init, rk) {
                    let s = self.ast.exprs.operand(init);
                    self.by_init(cx, s, true, vis, lw);
                }
            }
            ExprKind::MemberExpr => {
                let d = self.ast.exprs.decl(init);
                let fty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
                if self.kind_of(d) == DeclKind::Field && !self.ast.types.is_reference(fty) {
                    let b = self.ast.exprs.operand(init);
                    self.by_init(cx, b, true, vis, lw);
                }
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr => {
                if lw {
                    self.handle_gsl(cx, init, vis);
                }
                self.lifetimebound_args(cx, init, vis);
            }
            ExprKind::DeclRefExpr if !n.has(ExprFlags::REFERS_TO_ENCLOSING) => {
                let d = self.ast.exprs.decl(init);
                if !self.is_automatic_local(d) {
                    return;
                }
                let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
                if !self.ast.types.is_reference(ty) {
                    self.visit(cx, vis, init, rk);
                } else if self.kind_of(d) == DeclKind::Var
                    && let Some(x) = self.ast.decls.var(d).and_then(|v| v.init)
                    && !cx.on_path(d)
                {
                    // The lifetime of a reference parameter is unknown;
                    // a local reference denotes what it was bound to.
                    cx.path.push(Entry::new(Ek::VarInit, init, Some(d)));
                    self.by_ref(cx, x, Rk::Ref, vis, lw);
                }
            }
            ExprKind::UnaryOperator if self.ast.exprs.unary_op(init) == UnaryOp::Deref => {
                let s = self.ast.exprs.operand(init);
                self.by_init(cx, s, true, vis, lw);
            }
            ExprKind::ConditionalOperator | ExprKind::BinaryConditionalOperator => {
                for arm in self.lt_cond_arms(init) {
                    if !self.ast.types.is_void(self.ast.exprs.get(arm).ty()) {
                        self.by_ref(cx, arm, rk, vis, lw);
                    }
                }
            }
            _ => {}
        }
    }

    /// clang `visitLocalsRetainedByInitializer`: the locals reachable
    /// through an object initialized by the prvalue `init`. The elements
    /// of a braced list were checked as they were initialized and are
    /// walked again only if `revisit`.
    fn by_init(&mut self, cx: &mut Cx, init: ExprId, revisit: bool, vis: &Vis<'_>, lw: bool) {
        if cx.depth > MAX_DEPTH {
            return;
        }
        let len = cx.path.len();
        cx.depth += 1;
        self.by_init_in(cx, init, revisit, vis, lw);
        cx.depth -= 1;
        cx.path.truncate(len);
    }

    #[allow(
        clippy::too_many_lines,
        reason = "clang's visitLocalsRetainedByInitializer, one arm per form in its order"
    )]
    fn by_init_in(
        &mut self,
        cx: &mut Cx,
        mut init: ExprId,
        revisit: bool,
        vis: &Vis<'_>,
        lw: bool,
    ) {
        for _ in 0..MAX_DEPTH {
            let old = init;
            if self.ast.exprs.get(init).kind() == ExprKind::CXXDefaultInitExpr {
                let f = self.ast.exprs.default_init_field(init);
                let Some(x) = self.ast.decls.field(f).and_then(|d| d.init) else { return };
                cx.path.push(Entry::new(Ek::DefaultInit, init, Some(f)));
                init = x;
            }
            init = self.skip_full_expr(init);
            init = self.skip_rvalue_adjustments(init);
            if self.ast.exprs.get(init).kind() == ExprKind::CXXBindTemporaryExpr {
                init = self.ast.exprs.operand(init);
            }
            init = self.lt_skip_parens(init);
            if self.ast.exprs.get(init).kind().is_cast() {
                match self.ast.exprs.cast_kind(init) {
                    CastKind::LValueToRValue => {
                        // A `const` object's value is its initializer's.
                        cx.path.push(Entry::new(Ek::LValToRVal, init, None));
                        let v = Vis::ConstInit(vis, lw);
                        self.by_ref(cx, init, Rk::Ref, &v, lw);
                        return;
                    }
                    // Casts that keep the referred-to object (an integer
                    // may hold a pointer; `bool` and floating types not).
                    CastKind::NoOp
                    | CastKind::BitCast
                    | CastKind::BaseToDerived
                    | CastKind::DerivedToBase
                    | CastKind::UncheckedDerivedToBase
                    | CastKind::Dynamic
                    | CastKind::ToUnion
                    | CastKind::UserDefinedConversion
                    | CastKind::ConstructorConversion
                    | CastKind::IntegralToPointer
                    | CastKind::PointerToIntegral
                    | CastKind::VectorSplat
                    | CastKind::IntegralCast => {}
                    CastKind::ArrayToPointerDecay => {
                        cx.path.push(Entry::new(Ek::AddressOf, init, None));
                        let s = self.ast.exprs.operand(init);
                        self.by_ref(cx, s, Rk::Ref, vis, lw);
                        return;
                    }
                    _ => return,
                }
                init = self.ast.exprs.operand(init);
            }
            if init == old {
                break;
            }
        }
        let n = *self.ast.exprs.get(init);
        match n.kind() {
            // [dcl.init.list]/6: the backing array is bound like a
            // reference.
            ExprKind::CXXStdInitializerListExpr => {
                let s = self.ast.exprs.operand(init);
                self.by_ref(cx, s, Rk::InitList, vis, lw);
            }
            ExprKind::InitListExpr => self.by_init_list(cx, init, revisit, vis, lw),
            ExprKind::LambdaExpr => self.by_init_lambda(cx, init, vis, lw),
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                // A copy or move of a temporary refers to what the
                // temporary does.
                let ctor = self.ast.exprs.constructor(init);
                if matches!(
                    self.special_kind_of(ctor),
                    Some(SpecialKind::CopyCtor | SpecialKind::MoveCtor)
                ) && let Some(a) = self.first_of(self.ast.exprs.construct_args(init))
                    && self.ast.exprs.get(a).kind() == ExprKind::MaterializeTemporaryExpr
                {
                    let s = self.ast.exprs.operand(a);
                    cx.path.push(Entry::new(Ek::TemporaryCopy, s, Some(ctor)));
                    self.by_init(cx, s, true, vis, false);
                    cx.path.pop();
                }
                if lw {
                    self.handle_gsl(cx, init, vis);
                }
                self.lifetimebound_args(cx, init, vis);
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr | ExprKind::CXXOperatorCallExpr => {
                if lw {
                    self.handle_gsl(cx, init, vis);
                }
                self.lifetimebound_args(cx, init, vis);
            }
            ExprKind::UnaryOperator if self.ast.exprs.unary_op(init) == UnaryOp::AddrOf => {
                let s = self.ast.exprs.operand(init);
                // `&` of a prvalue was diagnosed already.
                if self.ast.exprs.get(s).kind() == ExprKind::MaterializeTemporaryExpr {
                    return;
                }
                cx.path.push(Entry::new(Ek::AddressOf, init, None));
                self.by_ref(cx, s, Rk::Ref, vis, lw);
            }
            ExprKind::BinaryOperator
                if matches!(self.ast.exprs.binary_op(init), BinaryOp::Add | BinaryOp::Sub)
                    && self.ast.types.is_pointer(n.ty()) =>
            {
                let [l, r] = self.ast.exprs.operands(init);
                if self.ast.types.is_pointer(self.ast.exprs.get(l).ty()) {
                    self.by_init(cx, l, true, vis, lw);
                } else if self.ast.types.is_pointer(self.ast.exprs.get(r).ty()) {
                    self.by_init(cx, r, true, vis, lw);
                }
            }
            ExprKind::ConditionalOperator | ExprKind::BinaryConditionalOperator => {
                for arm in self.lt_cond_arms(init) {
                    if !self.ast.types.is_void(self.ast.exprs.get(arm).ty()) {
                        self.by_init(cx, arm, true, vis, lw);
                    }
                }
            }
            ExprKind::AddrLabelExpr => {
                self.visit(cx, vis, init, Rk::Ref);
            }
            _ => {}
        }
    }

    /// A braced list initializing an object: the reference members bind
    /// their initializers, the other elements are walked as initializers.
    fn by_init_list(&mut self, cx: &mut Cx, e: ExprId, revisit: bool, vis: &Vis<'_>, lw: bool) {
        if !revisit {
            return;
        }
        let ty = self.ast.exprs.get(e).ty();
        let inits: Vec<ExprId> =
            self.ast.lists.iter::<ExprId>(self.ast.exprs.init_list_inits(e)).collect();
        if self.ile_transparent(e) {
            if let Some(&x) = inits.first() {
                self.by_init(cx, x, revisit, vis, lw);
            }
            return;
        }
        if self.ast.types.is_array(ty) {
            for x in inits {
                self.by_init(cx, x, revisit, vis, lw);
            }
            return;
        }
        let Some(r) = self.ast.types.as_record(ty) else { return };
        if let Some(f) = self.ast.exprs.init_list(e).union_field {
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            if let Some(&x) = inits.first() {
                if self.ast.types.is_reference(fty) {
                    self.by_ref(cx, x, Rk::Ref, vis, lw);
                } else {
                    self.by_init(cx, x, revisit, vis, lw);
                }
            }
            return;
        }
        let def = self.ast.definition(r).unwrap_or(r);
        let nbases = self.ast.decls.bases(def, &self.ast.lists).count();
        let fields = self.init_fields(r);
        for (i, &x) in inits.iter().enumerate() {
            if i < nbases {
                self.by_init(cx, x, revisit, vis, lw);
                continue;
            }
            let Some(&f) = fields.get(i - nbases) else { break };
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            if self.ast.types.is_reference(fty) {
                self.by_ref(cx, x, Rk::Ref, vis, lw);
            } else {
                self.by_init(cx, x, revisit, vis, lw);
            }
        }
    }

    /// The lifetime of an init-capture (or a capture's copy) is the
    /// closure's.
    fn by_init_lambda(&mut self, cx: &mut Cx, e: ExprId, vis: &Vis<'_>, lw: bool) {
        let data = self.ast.exprs.lambda(e);
        let Some(inits) = data.inits else { return };
        let default_loc = data.default_loc;
        let caps: Vec<LambdaCapture> = data.captures.clone();
        let inits: Vec<ExprId> = self.ast.lists.iter::<ExprId>(inits).collect();
        for (cap, init) in caps.into_iter().zip(inits) {
            let len = cx.path.len();
            if cap.var.is_some() {
                // clang diagnoses at the capture's initializer: the name
                // of an explicit capture, the capture-default of an
                // implicit one.
                let at = if cap.init {
                    self.begin_loc(init)
                } else if cap.implicit {
                    default_loc
                } else {
                    cap.loc
                };
                cx.path.push(Entry {
                    kind: Ek::LambdaCaptureInit,
                    e: init,
                    d: cap.var,
                    cap: Some((cap, at)),
                });
            }
            if self.is_glvalue(init) {
                self.by_ref(cx, init, Rk::Ref, vis, lw);
            } else {
                self.by_init(cx, init, true, vis, lw);
            }
            cx.path.truncate(len);
        }
    }

    /// Hands local `l` to the visitor; whether the walk goes on into a
    /// temporary's initializer.
    fn visit(&mut self, cx: &mut Cx, vis: &Vis<'_>, l: ExprId, rk: Rk) -> bool {
        match *vis {
            Vis::Top => self.visit_local(cx, l, rk),
            Vis::ConstInit(parent, lw) => {
                let n = *self.ast.exprs.get(l);
                match n.kind() {
                    ExprKind::DeclRefExpr => {
                        let d = self.ast.exprs.decl(l);
                        let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
                        if self.kind_of(d) == DeclKind::Var
                            && self.ast.types.canonical(ty).quals().is_const()
                            && let Some(x) = self.ast.decls.var(d).and_then(|v| v.init)
                            && !cx.on_path(d)
                        {
                            cx.path.push(Entry::new(Ek::VarInit, l, Some(d)));
                            self.by_init(cx, x, true, parent, lw);
                        }
                    }
                    ExprKind::MaterializeTemporaryExpr
                        if self.ast.types.canonical(n.ty()).quals().is_const() =>
                    {
                        let s = self.ast.exprs.operand(l);
                        self.by_init(cx, s, true, parent, lw);
                    }
                    _ => {}
                }
                false
            }
        }
    }

    /// clang `nextPathEntryRange`'s begin: the first variable, default
    /// member initializer or capture on `path` from `i`, else `l`.
    fn next_path_loc(&self, path: &[Entry], i: usize, l: ExprId) -> Loc {
        for p in path.iter().skip(i) {
            match p.kind {
                Ek::VarInit => {
                    let implicit = p.d.is_some_and(|d| {
                        self.ast.decls.get(d).flags().contains(DeclFlags::IMPLICIT)
                    });
                    return if implicit { Loc::INVALID } else { self.begin_loc(p.e) };
                }
                Ek::DefaultInit => return self.begin_loc(p.e),
                Ek::LambdaCaptureInit => {
                    if let Some((_, at)) = p.cap {
                        return at;
                    }
                }
                _ => {}
            }
        }
        self.begin_loc(l)
    }

    /// clang's `TemporaryVisitor`: judges local `l` by the entity's
    /// lifetime.
    #[allow(
        clippy::too_many_lines,
        reason = "clang's TemporaryVisitor, one judgement per entity kind in its order"
    )]
    fn visit_local(&mut self, cx: &mut Cx, l: ExprId, rk: Rk) -> bool {
        let at = self.next_path_loc(&cx.path, 0, l);
        let n = *self.ast.exprs.get(l);
        let is_mte = n.kind() == ExprKind::MaterializeTemporaryExpr;
        let mut gsl_temp = false;
        let mut local_owner = false;
        if cx.only_gsl() {
            if n.kind() == ExprKind::DeclRefExpr {
                // A Pointer into a local Owner is followed only to a
                // `return` (not through a variable's initializer).
                local_owner = self.type_gsl_kind(n.ty()) == Some(Gsl::Owner);
                if cx.contains_init() || !local_owner {
                    return false;
                }
            } else {
                gsl_temp = is_mte
                    && self.ast.exprs.lifetime_extended_temporary(l).is_none()
                    && self.type_gsl_kind(n.ty()) == Some(Gsl::Owner);
                // Skip a chain of gsl Pointers to their final source.
                if !gsl_temp {
                    return true;
                }
            }
        }
        let mut msg = match cx.lk {
            Lk::Extended => {
                if !is_mte {
                    return false;
                }
                if gsl_temp && at.is_valid() {
                    let msg = self.diag(DiagId::sema_dangling_lifetime_pointer, at);
                    self.emit(msg.per_instantiation());
                    return false;
                }
                match cx.path_kind() {
                    // Extended by `extend_temporaries`; its own
                    // temporaries are walked too.
                    PathKind::Extend => return true,
                    // A default member initializer's temporary: clang 20+
                    // extends it, clang 19 warns the extension is
                    // unsupported.
                    PathKind::ShouldExtend => {
                        if self.clang_compat >= 20 {
                            return false;
                        }
                        // Every member using one shares the location of
                        // the braces; each is diagnosed.
                        self.diag(DiagId::sema_unsupported_lifetime_extension, at)
                            .arg(rk as u8)
                            .allow_duplicate()
                    }
                    PathKind::NoExtend => {
                        if cx.contains_init() {
                            return false;
                        }
                        let Some(v) = cx.extending else { return false };
                        self.dangling_variable_diag(cx, v, rk, at)
                    }
                }
            }
            Lk::New => {
                if !is_mte {
                    // The allocation may outlive a local.
                    return false;
                }
                if gsl_temp {
                    self.diag(DiagId::sema_dangling_lifetime_pointer, at)
                } else {
                    match rk {
                        Rk::Ref => self.diag(DiagId::sema_new_dangling_reference, at),
                        Rk::InitList => self
                            .diag(DiagId::sema_new_dangling_initializer_list, at)
                            .arg(u8::from(!cx.has_parent)),
                    }
                }
            }
            Lk::Assign => {
                if !is_mte || cx.contains_init() {
                    return false;
                }
                let id = if gsl_temp {
                    DiagId::sema_dangling_lifetime_pointer_assignment
                } else {
                    DiagId::sema_dangling_pointer_assignment
                };
                let lhs = cx.lhs.map(|x| self.expr_text_as_written(x)).unwrap_or_default();
                let msg = self.diag(id, at).arg(lhs);
                self.emit(msg.per_instantiation());
                return false;
            }
            Lk::MemInit => {
                let Some(field) = cx.extending else { return false };
                let floc = self.ast.decls.get(field).loc();
                if is_mte {
                    if gsl_temp {
                        let msg = self
                            .diag(DiagId::sema_dangling_lifetime_pointer_member, at)
                            .arg(self.decl_arg(field))
                            .with_note(
                                self.diag(DiagId::note_ref_or_ptr_member_here, floc).arg(1u8),
                            );
                        self.emit(msg);
                        return false;
                    }
                    // [class.base.init]/8: a temporary bound to a
                    // reference member in a mem-initializer.
                    let id = if cx.path_kind() == PathKind::NoExtend {
                        DiagId::sema_dangling_member
                    } else {
                        DiagId::sema_temp_member_ref
                    };
                    let mut msg = self
                        .diag(id, at)
                        .arg(self.decl_arg(field))
                        .arg(u8::from(cx.has_parent))
                        .arg(rk as u8);
                    if cx.path.last().is_none_or(|p| p.kind != Ek::DefaultInit) {
                        msg = msg.with_note(
                            self.diag(DiagId::note_temp_member_ref_here, floc)
                                .arg(rk as u8)
                                .arg(u8::from(cx.has_parent)),
                        );
                    }
                    msg
                } else {
                    if cx.contains_init() || (local_owner && cx.only_gsl()) {
                        return false;
                    }
                    if n.kind() != ExprKind::DeclRefExpr {
                        return false;
                    }
                    let var = self.ast.exprs.decl(l);
                    if !matches!(self.kind_of(var), DeclKind::Var | DeclKind::ParmVar) {
                        return false;
                    }
                    let fty = self.ast.decls.get(field).ty().unwrap_or(QualType::ERROR);
                    let is_pointer = !self.ast.types.is_reference(fty);
                    let id = if is_pointer {
                        DiagId::sema_dangling_field_param_addr
                    } else {
                        DiagId::sema_dangling_field_param
                    };
                    self.diag(id, at)
                        .arg(self.decl_arg(field))
                        .arg(self.decl_arg(var))
                        .arg(u8::from(self.kind_of(var) == DeclKind::ParmVar))
                        .with_note(
                            self.diag(DiagId::note_ref_or_ptr_member_here, floc)
                                .arg(u8::from(is_pointer)),
                        )
                }
            }
            Lk::Return => match n.kind() {
                ExprKind::DeclRefExpr => {
                    let d = self.ast.exprs.decl(l);
                    self.diag(DiagId::sema_ret_stack_addr, at)
                        .arg(u8::from(cx.entity_is_ref))
                        .arg(u8::from(self.kind_of(d) == DeclKind::ParmVar))
                        .arg(self.decl_arg(d))
                }
                // "returning address of label, which is local".
                ExprKind::AddrLabelExpr => self.diag(DiagId::sema_ret_addr_label, at),
                ExprKind::MaterializeTemporaryExpr => {
                    // [stmt.return]/6 (P2748): ill-formed in C++26.
                    let id = if cx.entity_is_ref {
                        self.return_temp_ref_id()
                    } else {
                        DiagId::sema_return_local_temp_addr
                    };
                    self.diag(id, at)
                }
                _ => return false,
            },
        };
        for (i, p) in cx.path.iter().enumerate() {
            let note = match p.kind {
                Ek::DefaultInit => {
                    let Some(f) = p.d else { continue };
                    let _ = i;
                    self.diag(
                        DiagId::note_init_with_default_member_init,
                        self.ast.decls.get(f).loc(),
                    )
                    .arg(self.decl_arg(f))
                }
                Ek::VarInit => {
                    let Some(v) = p.d else { continue };
                    let d = self.ast.decls.get(v);
                    let is_ref = self.ast.types.is_reference(d.ty().unwrap_or(QualType::ERROR));
                    if d.flags().contains(DeclFlags::IMPLICIT) {
                        self.diag(DiagId::note_local_var_init_implicit, d.loc())
                            .arg(u8::from(is_ref))
                    } else {
                        self.diag(DiagId::note_local_var_init, d.loc())
                            .arg(u8::from(is_ref))
                            .arg(self.decl_arg(v))
                    }
                }
                Ek::LambdaCaptureInit => {
                    let Some((cap, _)) = p.cap else { continue };
                    let by_ref = cap.kind == CaptureKind::ByRef;
                    if cap.init {
                        let Some(v) = cap.var else { continue };
                        self.diag(DiagId::note_lambda_init_capture, cap.loc)
                            .arg(u8::from(by_ref))
                            .arg(self.decl_arg(v))
                    } else {
                        self.diag(DiagId::note_lambda_capture_here, cap.loc)
                            .arg(u8::from(by_ref) + 2 * u8::from(cap.implicit))
                    }
                }
                _ => continue,
            };
            msg = msg.with_note(note);
        }
        // clang re-runs the check on the initializer rebuilt in each
        // instantiation (WP-231). A mem-initializer is checked once:
        // kiln also checks the non-dependent ones of a template
        // definition, which clang does not.
        if cx.lk == Lk::MemInit {
            self.emit(msg);
        } else {
            self.emit(msg.per_instantiation());
        }
        false
    }

    /// clang `warn_dangling_variable` for a temporary bound to variable
    /// `v` (or a subobject of it); an implicit variable is not named.
    fn dangling_variable_diag(&self, cx: &Cx, v: DeclId, rk: Rk, at: Loc) -> kiln_diag::Diag {
        let implicit = self.ast.decls.get(v).flags().contains(DeclFlags::IMPLICIT);
        let id = match (rk, cx.has_parent, implicit) {
            (Rk::Ref, true, false) => DiagId::sema_dangling_variable_member,
            (Rk::Ref, false, false) => DiagId::sema_dangling_variable,
            (Rk::Ref, true, true) => DiagId::sema_dangling_variable_member_implicit,
            (Rk::Ref, false, true) => DiagId::sema_dangling_variable_implicit,
            (Rk::InitList, _, false) => DiagId::sema_dangling_init_list,
            (Rk::InitList, _, true) => DiagId::sema_dangling_init_list_implicit,
        };
        let sel = match rk {
            Rk::Ref => u8::from(cx.init_glvalue),
            Rk::InitList => u8::from(!cx.has_parent),
        };
        let msg = self.diag(id, at).arg(sel);
        if implicit { msg } else { msg.arg(self.decl_arg(v)) }
    }

    /// clang `visitLifetimeBoundArguments`: a call's result refers to
    /// what each argument bound to a `lifetimebound` parameter (or the
    /// implicit object of a `lifetimebound` member function) does.
    fn lifetimebound_args(&mut self, cx: &mut Cx, call: ExprId, vis: &Vis<'_>) {
        let kind = self.ast.exprs.get(call).kind();
        let (callee, list) = match kind {
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                (Some(self.ast.exprs.constructor(call)), self.ast.exprs.construct_args(call))
            }
            _ => (self.direct_callee(call), self.ast.exprs.call_args(call)),
        };
        let Some(callee) = callee else { return };
        let mut args: Vec<ExprId> = self.ast.lists.iter::<ExprId>(list).collect();
        let mut object = None;
        if kind == ExprKind::CXXOperatorCallExpr && self.is_instance_member(callee) {
            if args.is_empty() {
                return;
            }
            object = Some(args.remove(0));
        } else if kind == ExprKind::CXXMemberCallExpr {
            object = Some(self.ast.exprs.member_call_object(call));
        }
        if let Some(o) = object
            && self.implicit_object_lifetimebound(callee)
        {
            self.visit_lifetimebound_arg(cx, callee, o, vis);
        }
        let params = self.param_decls(callee);
        for (i, (&p, &a)) in params.iter().zip(&args).enumerate() {
            if self.param_lifetimebound(callee, i) {
                self.visit_lifetimebound_arg(cx, p, a, vis);
            }
        }
    }

    fn visit_lifetimebound_arg(&mut self, cx: &mut Cx, d: DeclId, arg: ExprId, vis: &Vis<'_>) {
        cx.path.push(Entry::new(Ek::LifetimeBoundCall, arg, Some(d)));
        if self.is_glvalue(arg) {
            self.by_ref(cx, arg, Rk::Ref, vis, false);
        } else {
            self.by_init(cx, arg, true, vis, false);
        }
        cx.path.pop();
    }

    /// The function a call names directly (clang `getDirectCallee`).
    fn direct_callee(&self, call: ExprId) -> Option<DeclId> {
        self.callee_decl(call).filter(|&d| self.kind_of(d).is_function())
    }

    /// Whether some declaration of `f` (or of its pattern) carries
    /// `[[clang::lifetimebound]]` with object flag `implicit` (`a == 1`:
    /// written on the function type, for the implicit object).
    fn has_lifetimebound(&self, d: DeclId, implicit: bool) -> bool {
        self.ast.redecls_of(d).any(|x| {
            self.ast.lists.iter::<AttrId>(self.ast.attrs.of_decl(x)).any(|a| {
                let a = self.ast.attrs.get(a);
                a.kind == AttrKind::LIFETIMEBOUND && (a.a == 1) == implicit
            })
        })
    }

    /// clang `implicitObjectParamIsLifetimeBound`: the attribute on the
    /// member function type, or an assignment operator returning an
    /// lvalue reference to its left operand's type.
    fn implicit_object_lifetimebound(&self, f: DeclId) -> bool {
        if self.has_lifetimebound(f, true) {
            return true;
        }
        let pattern = self.function_pattern(f);
        if pattern != f && self.has_lifetimebound(pattern, true) {
            return true;
        }
        let Some(op) = self.ast.decls.get(f).name().as_operator() else { return false };
        let assign = matches!(
            op,
            OO::Equal
                | OO::PlusEqual
                | OO::MinusEqual
                | OO::StarEqual
                | OO::SlashEqual
                | OO::PercentEqual
                | OO::CaretEqual
                | OO::AmpEqual
                | OO::PipeEqual
                | OO::LessLessEqual
                | OO::GreaterGreaterEqual
        );
        let ret = self.fn_ret(f);
        if !assign || self.ast.types.canonical_kind(ret) != TypeKind::LValueRef {
            return false;
        }
        let Some(pointee) = self.ast.types.pointee(ret) else { return false };
        if self.is_instance_member(f) {
            let Some(class) = self.ast.decls.get(f).ctx() else { return false };
            let (quals, _) = self.method_object_quals(f);
            let pc = self.ast.types.canonical(pointee);
            self.ast.types.as_record(pc).is_some_and(|r| self.ast.first(r) == self.ast.first(class))
                && pc.quals() == quals
        } else {
            let ty = self.fn_param_type(f, 0);
            ty.is_some_and(|t| self.ast.types.same_type(t, ret))
        }
    }

    /// The type of parameter `i` of function `f`.
    fn fn_param_type(&self, f: DeclId, i: usize) -> Option<QualType> {
        let ty = self.ast.decls.get(f).ty()?;
        let proto = self.ast.types.proto(ty)?;
        self.ast.types.params(proto).get(i).copied()
    }

    /// Whether parameter `i` of `f` is `lifetimebound`: written on it (on
    /// any declaration of `f` or its pattern), or implied for the
    /// parameter of `std::move` and the like.
    fn param_lifetimebound(&self, f: DeclId, i: usize) -> bool {
        let written = |g: DeclId| {
            self.ast.redecls_of(g).any(|x| {
                self.param_decls(x).get(i).is_some_and(|&p| self.has_lifetimebound(p, false))
            })
        };
        if written(f) {
            return true;
        }
        let pattern = self.function_pattern(f);
        if pattern != f && written(pattern) {
            return true;
        }
        if i != 0 || !self.is_in_std_namespace(f) {
            return false;
        }
        let Some(name) = self.ast.decls.get(f).name().as_ident() else { return false };
        if !STD_LIFETIMEBOUND.contains(&self.ast.symbols().as_str(name)) {
            return false;
        }
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        self.ast.types.proto(ty).is_some_and(|p| p.num_params() == 1 && !p.variadic())
    }

    /// clang `handleGslAnnotatedTypes`: a Pointer built from an Owner (or
    /// a Pointer or reference obtained from one) refers to what the Owner
    /// owns.
    fn handle_gsl(&mut self, cx: &mut Cx, call: ExprId, vis: &Vis<'_>) {
        match self.ast.exprs.get(call).kind() {
            ExprKind::CXXMemberCallExpr => {
                let Some(m) = self.direct_callee(call) else { return };
                if self.track_implicit_object_arg(m) {
                    let o = self.ast.exprs.member_call_object(call);
                    let value = !self.ast.types.is_reference(self.fn_ret(m));
                    self.visit_pointer_arg(cx, m, o, value, vis);
                }
            }
            ExprKind::CXXOperatorCallExpr => {
                let Some(m) = self.direct_callee(call) else { return };
                if self.is_instance_member(m)
                    && self.track_implicit_object_arg(m)
                    && let Some(o) = self.first_of(self.ast.exprs.call_args(call))
                {
                    let value = !self.ast.types.is_reference(self.fn_ret(m));
                    self.visit_pointer_arg(cx, m, o, value, vis);
                }
            }
            ExprKind::CallExpr => {
                let Some(f) = self.direct_callee(call) else { return };
                if self.track_first_argument(f)
                    && let Some(a) = self.first_of(self.ast.exprs.call_args(call))
                {
                    let value = !self.ast.types.is_reference(self.fn_ret(f));
                    self.visit_pointer_arg(cx, f, a, value, vis);
                }
            }
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                let ctor = self.ast.exprs.constructor(call);
                let Some(class) = self.ast.decls.get(ctor).ctx() else { return };
                let list: ListId = self.ast.exprs.construct_args(call);
                if let Some(a) = self.first_of(list)
                    && self.gsl_kind(class) == Some(Gsl::Pointer)
                {
                    let p = self.param_decls(ctor).first().copied().unwrap_or(ctor);
                    self.visit_pointer_arg(cx, p, a, true, vis);
                }
            }
            _ => {}
        }
    }

    fn visit_pointer_arg(
        &mut self,
        cx: &mut Cx,
        d: DeclId,
        arg: ExprId,
        value: bool,
        vis: &Vis<'_>,
    ) {
        // Not the temporary base object of a Pointer: `Temp().ptr` may
        // not dangle.
        if self.ast.exprs.get(self.ignore_imp_casts(arg)).kind() == ExprKind::MemberExpr {
            return;
        }
        // Once a value was initialized from a reference, it can no
        // longer dangle.
        if !value {
            for p in cx.path.iter().rev() {
                match p.kind {
                    Ek::GslReferenceInit => {}
                    Ek::GslPointerInit | Ek::GslPointerAssignment => return,
                    _ => break,
                }
            }
        }
        let kind = if value { Ek::GslPointerInit } else { Ek::GslReferenceInit };
        cx.path.push(Entry::new(kind, arg, Some(d)));
        if self.is_glvalue(arg) {
            self.by_ref(cx, arg, Rk::Ref, vis, true);
        } else {
            self.by_init(cx, arg, true, vis, true);
        }
        cx.path.pop();
    }

    fn first_of(&self, l: ListId) -> Option<ExprId> {
        self.ast.lists.iter::<ExprId>(l).next()
    }

    /// clang `isInStlNamespace`: declared directly in `std` (through
    /// inline namespaces) or in a namespace with a reserved name.
    fn is_in_stl_namespace(&self, d: DeclId) -> bool {
        if let Some(c) = self.ast.decls.get(d).ctx()
            && self.kind_of(c) == DeclKind::Namespace
            && let Some(n) = self.ast.decls.get(c).name().as_ident()
        {
            let b = self.ast.symbols().as_str(n).as_bytes();
            if b.len() >= 2 && b[0] == b'_' && (b[1] == b'_' || b[1].is_ascii_uppercase()) {
                return true;
            }
        }
        self.is_in_std_namespace(d)
    }

    /// clang `shouldTrackImplicitObjectArg`.
    fn track_implicit_object_arg(&self, m: DeclId) -> bool {
        let ret = self.fn_ret(m);
        if self.kind_of(m) == DeclKind::CXXConversion
            && self.type_gsl_kind(ret) == Some(Gsl::Pointer)
        {
            return true;
        }
        let Some(class) = self.ast.decls.get(m).ctx() else { return false };
        if !self.is_in_stl_namespace(class) || self.gsl_kind(class).is_none() {
            return false;
        }
        let name = self.ast.decls.get(m).name();
        let ident = name.as_ident().map(|s| self.ast.symbols().as_str(s));
        if self.ast.types.is_pointer(ret) || self.type_gsl_kind(ret) == Some(Gsl::Pointer) {
            ident.is_some_and(|n| TRACK_PTR_METHODS.contains(&n))
        } else if self.ast.types.is_reference(ret) {
            match ident {
                None => matches!(name.as_operator(), Some(OO::Subscript | OO::Star)),
                Some(n) => TRACK_REF_METHODS.contains(&n),
            }
        } else {
            false
        }
    }

    /// clang `shouldTrackFirstArgument`: `std::begin(owner)` and the like.
    fn track_first_argument(&self, f: DeclId) -> bool {
        let Some(name) = self.ast.decls.get(f).name().as_ident() else { return false };
        let params = self.param_decls(f);
        if params.len() != 1 {
            return false;
        }
        let pty = self.ast.decls.get(params[0]).ty().unwrap_or(QualType::ERROR);
        let Some(r) = self.ast.types.pointee(pty).and_then(|p| self.ast.types.as_record(p)) else {
            return false;
        };
        if !self.is_in_std_namespace(f)
            || !self.is_in_std_namespace(r)
            || self.gsl_kind(r).is_none()
        {
            return false;
        }
        let name = self.ast.symbols().as_str(name);
        let ret = self.fn_ret(f);
        if self.ast.types.is_pointer(ret) || self.type_gsl_kind(ret) == Some(Gsl::Pointer) {
            TRACK_PTR_FUNCTIONS.contains(&name)
        } else if self.ast.types.is_reference(ret) {
            TRACK_REF_FUNCTIONS.contains(&name)
        } else {
            false
        }
    }
}
