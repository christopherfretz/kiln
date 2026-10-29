//! Special member functions (WP-067a; [special], [class.ctor],
//! [class.dtor], [class.copy.ctor], [class.copy.assign],
//! [class.conv.fct], [dcl.fct.def.default], [dcl.fct.def.delete];
//! spec/sema-class.md §4).
//!
//! - `declare`: constructor, destructor and conversion-function
//!   declarators (the §4.7 checks), `explicit`, member `operator=`.
//! - `implicit`: which special members a class has, their implicit
//!   declarations (eager at `}` as clang does, else lazily when sema
//!   needs them), triviality, `constexpr` and the literal-type bits.
//! - `deletion`: `should_delete` ([class.default.ctor]/2,
//!   [class.copy.ctor]/10, [class.copy.assign]/7, [class.dtor]/7) and
//!   its notes, `= default` checks.
//! - `uses`: uses of deleted functions ([dcl.fct.def.delete]/2) and the
//!   default-initialization checks of variables of class type.

mod declare;
mod deletion;
mod implicit;
mod uses;

use kiln_ast::decl::{DeclFlags, DeclKind, Defaulted, DefinitionBits, FnSpec, ParamData};
use kiln_ast::names::DeclNameView;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::Sema;

use uses::PendingDeleted;

/// The six special member functions ([special]/1), in clang's
/// `CXXSpecialMemberKind` order, which is also the `%select` index of the
/// diagnostics naming them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpecialKind {
    /// [class.default.ctor].
    DefaultCtor = 0,
    /// [class.copy.ctor].
    CopyCtor = 1,
    /// [class.copy.ctor].
    MoveCtor = 2,
    /// [class.copy.assign].
    CopyAssign = 3,
    /// [class.copy.assign].
    MoveAssign = 4,
    /// [class.dtor].
    Dtor = 5,
}

impl SpecialKind {
    /// Every kind, in order.
    pub const ALL: [SpecialKind; 6] = [
        SpecialKind::DefaultCtor,
        SpecialKind::CopyCtor,
        SpecialKind::MoveCtor,
        SpecialKind::CopyAssign,
        SpecialKind::MoveAssign,
        SpecialKind::Dtor,
    ];

    /// The `%select` index.
    pub const fn index(self) -> u8 {
        self as u8
    }

    /// The `select` argument of a diagnostic.
    pub(crate) fn sel(self) -> String {
        self.index().to_string()
    }

    /// The [`super::implicit`] bit of the kind.
    pub const fn bit(self) -> u8 {
        1 << self.index()
    }

    /// A constructor.
    pub const fn is_ctor(self) -> bool {
        matches!(self, SpecialKind::DefaultCtor | SpecialKind::CopyCtor | SpecialKind::MoveCtor)
    }

    /// An assignment operator.
    pub const fn is_assign(self) -> bool {
        matches!(self, SpecialKind::CopyAssign | SpecialKind::MoveAssign)
    }

    /// A move operation.
    pub const fn is_move(self) -> bool {
        matches!(self, SpecialKind::MoveCtor | SpecialKind::MoveAssign)
    }

    /// The kind's name in diagnostics (`default constructor`, ...).
    pub const fn spelling(self) -> &'static str {
        match self {
            SpecialKind::DefaultCtor => "default constructor",
            SpecialKind::CopyCtor => "copy constructor",
            SpecialKind::MoveCtor => "move constructor",
            SpecialKind::CopyAssign => "copy assignment operator",
            SpecialKind::MoveAssign => "move assignment operator",
            SpecialKind::Dtor => "destructor",
        }
    }

    /// The `TRIVIAL_*` definition bit.
    pub const fn trivial_bit(self) -> DefinitionBits {
        match self {
            SpecialKind::DefaultCtor => DefinitionBits::TRIVIAL_DEFAULT_CTOR,
            SpecialKind::CopyCtor => DefinitionBits::TRIVIAL_COPY_CTOR,
            SpecialKind::MoveCtor => DefinitionBits::TRIVIAL_MOVE_CTOR,
            SpecialKind::CopyAssign => DefinitionBits::TRIVIAL_COPY_ASSIGN,
            SpecialKind::MoveAssign => DefinitionBits::TRIVIAL_MOVE_ASSIGN,
            SpecialKind::Dtor => DefinitionBits::TRIVIAL_DTOR,
        }
    }

    /// The `USER_DECLARED_*` definition bit (`USER_DECLARED_CTOR` for the
    /// default constructor: any user-declared constructor suppresses it,
    /// [class.default.ctor]/1).
    pub const fn user_declared_bit(self) -> DefinitionBits {
        match self {
            SpecialKind::DefaultCtor => DefinitionBits::USER_DECLARED_CTOR,
            SpecialKind::CopyCtor => DefinitionBits::USER_DECLARED_COPY_CTOR,
            SpecialKind::MoveCtor => DefinitionBits::USER_DECLARED_MOVE_CTOR,
            SpecialKind::CopyAssign => DefinitionBits::USER_DECLARED_COPY_ASSIGN,
            SpecialKind::MoveAssign => DefinitionBits::USER_DECLARED_MOVE_ASSIGN,
            SpecialKind::Dtor => DefinitionBits::USER_DECLARED_DTOR,
        }
    }

    /// The `NEEDS_OVL_*` definition bit (none for the default
    /// constructor).
    pub const fn needs_ovl_bit(self) -> Option<DefinitionBits> {
        match self {
            SpecialKind::DefaultCtor => None,
            SpecialKind::CopyCtor => Some(DefinitionBits::NEEDS_OVL_COPY_CTOR),
            SpecialKind::MoveCtor => Some(DefinitionBits::NEEDS_OVL_MOVE_CTOR),
            SpecialKind::CopyAssign => Some(DefinitionBits::NEEDS_OVL_COPY_ASSIGN),
            SpecialKind::MoveAssign => Some(DefinitionBits::NEEDS_OVL_MOVE_ASSIGN),
            SpecialKind::Dtor => Some(DefinitionBits::NEEDS_OVL_DTOR),
        }
    }

    /// The `DEFAULTED_*_DELETED` definition bit (constructors and the
    /// destructor; clang keeps none for assignment).
    pub const fn defaulted_deleted_bit(self) -> Option<DefinitionBits> {
        match self {
            SpecialKind::DefaultCtor => Some(DefinitionBits::DEFAULTED_DEFAULT_CTOR_DELETED),
            SpecialKind::CopyCtor => Some(DefinitionBits::DEFAULTED_COPY_CTOR_DELETED),
            SpecialKind::MoveCtor => Some(DefinitionBits::DEFAULTED_MOVE_CTOR_DELETED),
            SpecialKind::Dtor => Some(DefinitionBits::DEFAULTED_DTOR_DELETED),
            SpecialKind::CopyAssign | SpecialKind::MoveAssign => None,
        }
    }
}

/// The special-member state of [`Sema`] (spec/sema-class.md §8.3).
#[derive(Debug, Default)]
pub(crate) struct SpecialState {
    /// Explicitly defaulted special members declared in a class being
    /// defined, with the `default` keyword: checked at the class's `}`.
    pub defaulted: Vec<(DeclId, Loc)>,
    /// Deleted functions named by the expression being analysed whose
    /// diagnostic waits to see whether they are called.
    pub pending_deleted: Vec<PendingDeleted>,
    /// Implicit special members found deleted after their class's
    /// definition bits froze (lazy declarations; clang's
    /// `setImplicit*IsDeleted` and its `DefaultedCopyAssignmentIsDeleted`
    /// / `DefaultedMoveAssignmentIsDeleted`, which have no definition
    /// bit): class definition to [`SpecialKind::bit`]s.
    pub deleted_implicit: FxHashMap<DeclId, u8>,
    /// The definition bits of classes at their `}`, before
    /// `complete_record_definition` freezes them (innermost last).
    pub finishing: Vec<(DeclId, DefinitionBits)>,
    /// Fields declared with a default member initializer
    /// ([class.mem.general]/11), whose initializer is parsed later.
    pub nsdmi: FxHashSet<DeclId>,
}

impl Sema<'_, '_> {
    /// The class type of record `r` (shared by its redeclarations).
    pub(crate) fn class_type(&self, r: DeclId) -> QualType {
        self.tag_type(r)
    }

    /// Whether `q` (cv-qualifiers ignored) is the class `r`.
    pub(crate) fn is_class_type_of(&self, q: QualType, r: DeclId) -> bool {
        self.ast.types.as_record(q).is_some_and(|d| self.ast.first(d) == self.ast.first(r))
    }

    /// The parameters of function `f`.
    pub(crate) fn param_decls(&self, f: DeclId) -> Vec<DeclId> {
        self.ast
            .decls
            .function(f)
            .map(|x| self.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default()
    }

    /// Whether parameter `p` has a default argument (possibly not parsed
    /// yet).
    pub(crate) fn param_has_default(&self, p: DeclId) -> bool {
        self.ast.decls.param(p).is_some_and(|x| {
            x.default_arg.is_some() || x.flags & ParamData::DEFAULT_ARG_PENDING != 0
        })
    }

    /// Which special member function `f` is, if any ([special]/1;
    /// clang `getSpecialMember`). Function templates are never special
    /// members.
    pub(crate) fn special_kind_of(&self, f: DeclId) -> Option<SpecialKind> {
        let kind = self.kind_of(f);
        if !matches!(kind, DeclKind::CXXConstructor | DeclKind::CXXDestructor | DeclKind::CXXMethod)
        {
            return None;
        }
        if self.ast.described_template(f).is_some() {
            return None;
        }
        let class = self.ast.decls.get(f).ctx()?;
        if kind == DeclKind::CXXDestructor {
            return Some(SpecialKind::Dtor);
        }
        let params = self.param_decls(f);
        let pty = |i: usize| -> QualType {
            self.ast.decls.get(params[i]).ty().unwrap_or(QualType::ERROR)
        };
        if kind == DeclKind::CXXConstructor {
            if params.first().is_none_or(|&p| self.param_has_default(p)) {
                return Some(SpecialKind::DefaultCtor);
            }
            if !params[1..].iter().all(|&p| self.param_has_default(p)) {
                return None;
            }
            return match self.ast.types.canonical_view(pty(0)) {
                TypeView::LValueRef(t) if self.is_class_type_of(t, class) => {
                    Some(SpecialKind::CopyCtor)
                }
                TypeView::RValueRef(t) if self.is_class_type_of(t, class) => {
                    Some(SpecialKind::MoveCtor)
                }
                _ => None,
            };
        }
        // [class.copy.assign]/1: a non-template non-static `operator=`
        // with exactly one parameter of type X, X&, X&& (cv any).
        let name = self.ast.decls.get(f).name();
        if !matches!(
            self.ast.names.view(name),
            DeclNameView::Operator(kiln_ast::names::OverloadedOperator::Equal)
        ) || self.is_static_method(f)
            || params.len() != 1
        {
            return None;
        }
        match self.ast.types.canonical_view(pty(0)) {
            TypeView::LValueRef(t) if self.is_class_type_of(t, class) => {
                Some(SpecialKind::CopyAssign)
            }
            TypeView::RValueRef(t) if self.is_class_type_of(t, class) => {
                Some(SpecialKind::MoveAssign)
            }
            _ if self.is_class_type_of(pty(0), class) => Some(SpecialKind::CopyAssign),
            _ => None,
        }
    }

    /// Whether `f` was declared by the user (not implicitly).
    pub(crate) fn is_user_declared(&self, f: DeclId) -> bool {
        !self.ast.decls.get(f).flags().contains(DeclFlags::IMPLICIT)
    }

    /// clang `isUserProvided` ([dcl.fct.def.default]/5): user-declared
    /// and not defaulted or deleted on its first declaration.
    pub(crate) fn is_user_provided(&self, f: DeclId) -> bool {
        let first = self.ast.first(f);
        let spec = self.ast.decls.function(first).map(|x| x.spec).unwrap_or_default();
        self.is_user_declared(first) && spec.defaulted() == Defaulted::No && !spec.is_deleted()
    }

    /// `= default;` after the declarator of function `f`
    /// ([dcl.fct.def.default], spec/sema-class.md §4.4).
    pub(crate) fn defaulted_impl(&mut self, f: DeclId, loc: Loc) {
        if self.ast.decls.get(f).flags().contains(DeclFlags::INVALID) {
            return;
        }
        let Some(kind) = self.special_kind_of(f) else {
            let name = self.ast.decls.get(f).name();
            if let DeclNameView::Operator(op) = self.ast.names.view(name)
                && is_comparison(op)
            {
                // todo(WP-068b): defaulted comparison operators.
                self.unsupported("defaulted comparison operators", loc);
            } else {
                self.error(DiagId::sema_default_not_special, loc);
            }
            return;
        };
        let first = self.ast.prev(f).is_none();
        if first {
            // `constexpr`, the implicit exception specification and
            // triviality are decided once the class is complete
            // ([dcl.fct.def.default]/3, [except.spec]/8).
            self.ast.decls.mark_function(f, FnSpec::new().with_defaulted(Defaulted::Explicit));
            let class = self.ast.decls.get(f).ctx();
            if self.classes.iter().any(|c| Some(c.record) == class) {
                self.special.defaulted.push((f, loc));
            } else {
                self.check_defaulted(f, kind, loc);
            }
        } else {
            // An out-of-line `= default` is user-provided
            // ([dcl.fct.def.default]/5).
            self.ast.decls.mark_function(f, FnSpec::new().with_defaulted(Defaulted::Explicit));
            self.check_out_of_line_defaulted(f, kind);
        }
    }

    /// `= delete;` / `= delete("msg");` ([dcl.fct.def.delete]/4: the first
    /// declaration).
    pub(crate) fn deleted_impl(&mut self, f: DeclId, loc: Loc, msg: Option<ExprId>) {
        // A deleted definition is a definition ([dcl.fct.def.general]/1).
        let first = self.ast.first(f);
        if first != f && self.is_deleted_fn(first) {
            let m = self
                .diag(DiagId::sema_redefinition, self.ast.decls.get(f).loc())
                .arg(self.decl_name(f));
            let m = self.note_prev(m, first, true);
            self.emit(m);
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
            return;
        }
        if let Some(p) = self.ast.prev(f)
            && self.ast.definition(p).is_none_or(|d| d == f)
        {
            let m = self.diag(DiagId::sema_deleted_not_first, loc);
            let m = self.note_prev(m, p, false);
            self.emit(m);
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
            return;
        }
        self.ast.decls.mark_function(f, FnSpec::new().with_deleted(true));
        if let Some(m) = msg {
            let _ = self.ast.decls.cold.delete_message.insert(f, m);
        }
    }

    /// The `define_implicit_*` hooks of spec/sema-class.md §4.5: the
    /// implicit definitions (member-wise construction, copy, assignment
    /// and destruction) are a later work package; a use that needs one
    /// reports `{what} is not supported yet`.
    #[expect(dead_code, reason = "called once constructor calls are built (WP-067b)")]
    pub(crate) fn define_implicit_special(&mut self, f: DeclId, kind: SpecialKind, loc: Loc) {
        let _ = f;
        let what = format!("implicit definition of a {}", kind.spelling());
        self.unsupported(&what, loc);
    }
}

/// `==`, `!=`, `<`, `>`, `<=`, `>=`, `<=>` ([class.compare]).
fn is_comparison(op: kiln_ast::names::OverloadedOperator) -> bool {
    use kiln_ast::names::OverloadedOperator as O;
    matches!(
        op,
        O::EqualEqual
            | O::ExclaimEqual
            | O::Less
            | O::Greater
            | O::LessEqual
            | O::GreaterEqual
            | O::Spaceship
    )
}
