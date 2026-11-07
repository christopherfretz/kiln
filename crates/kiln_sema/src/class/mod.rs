//! Classes (spec/sema-class.md): the parser-facing entry points of class
//! definitions, complete-class contexts and constructors, and the sema
//! state they share.
//!
//! WP-066a lands the interface only: every entry point that would accept
//! a class feature reports `{what} is not supported yet` (never a panic)
//! and the state is initialised but not yet read. The implementing work
//! packages are named on each item (spec/sema-class.md §9); they fill in
//! the bodies without changing these signatures. Submodules
//! (`access`, `special`, `ctorinit`, `temporary`, `virtual`) arrive with
//! those packages.

mod access;
mod compare;
mod construct;
mod ctor_select;
mod ctorinit;
pub(crate) mod friend;
pub(crate) mod inhctor;
mod member_expr;
pub(crate) mod members;
mod special;
mod temporary;
mod r#virtual;

use kiln_ast::decl::{Access, DeclFlags, DeclKind, DefinitionBits, TagKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::types::ExcSpec;
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::declspec::{DeclSpec, Declarator};
use crate::init::Initializer;
use crate::scope::{Scope, ScopeKind};

/// One base-specifier as parsed ([class.derived.general]/1), the input of
/// [`Sema::act_on_base_specifiers`] (spec/sema-class.md §3.1).
#[derive(Clone, Debug)]
pub struct BaseSpecInfo {
    /// The base type as written (a class-or-decltype).
    pub ty: QualType,
    /// Where the base-specifier starts.
    pub loc: Loc,
    /// `virtual` was written.
    pub is_virtual: bool,
    /// The access-specifier as written; [`Access::None`] when absent
    /// (`private` for `class`, else `public`).
    pub access: Access,
    /// The specifier is a pack expansion `B...` (WP-070).
    pub pack_expansion: bool,
    /// Attributes appertaining to the base-specifier.
    pub attrs: Vec<AttrId>,
}

/// The mem-initializer-id of one mem-initializer ([class.base.init]/2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemInitId {
    /// An identifier: a non-static data member, else a base or the class
    /// itself named by an injected-class-name or typedef (lookup decides).
    Ident(Symbol),
    /// A class-or-decltype the parser already resolved to a type (a
    /// qualified name, a template-id, `decltype(...)`).
    Type(QualType),
}

/// One mem-initializer as parsed, the input of
/// [`Sema::act_on_mem_initializers`] (spec/sema-class.md §4.9).
#[derive(Clone, Debug)]
pub struct MemInitInfo {
    /// What is initialized.
    pub id: MemInitId,
    /// Where the mem-initializer-id was written.
    pub loc: Loc,
    /// `( expression-list )` ([`Initializer::Paren`], possibly empty) or a
    /// braced-init-list ([`Initializer::List`]).
    pub init: Initializer,
    /// `B(args)...` (WP-070).
    pub pack_expansion: bool,
}

/// Per class being defined, innermost last in [`Sema`]'s `classes`
/// (spec/sema-class.md §8.3). Pushed by `act_on_start_record_definition`,
/// popped by `act_on_finish_record_definition`.
#[derive(Clone, Debug)]
pub struct ClassBuild {
    /// The class.
    pub record: DeclId,
    /// The access of the next member ([class.access.spec]): `private` for
    /// `class`, else `public`, until an access-specifier changes it.
    pub access: Access,
    /// Definition bits accumulated as members are declared (§2.9); merged
    /// into `RecordData.defn` at `}`.
    pub bits: DefinitionBits,
    /// The first virtual member function declared (diagnostic anchor,
    /// key-function candidate order).
    pub first_virtual: Option<DeclId>,
    /// Fields of this class whose NSDMI is saved for replay (§2.7).
    pub pending_nsdmi: u32,
    /// Parameters of member functions whose default argument is saved for
    /// replay (§2.7).
    pub pending_default_args: u32,
    /// The first declarations befriended by this class, in order; stored
    /// in `RecordCold.friends` at `}`.
    pub friends: Vec<DeclId>,
    /// The context current where the definition began, current again at
    /// `}` (an out-of-line definition of a member partial specialization
    /// declared in its class is lexically outside it).
    pub outer: DeclId,
}

/// The function being defined when a nested function definition starts
/// (a member function defined inside a local class), and the number of
/// class/namespace scopes entered for an out-of-line member definition.
#[derive(Debug)]
pub(crate) struct SavedFn {
    pub(crate) func: Option<crate::stmt::FnState>,
    pub(crate) pushed: usize,
}

impl ClassBuild {
    /// A fresh entry for `record` with the default access of `tag`.
    pub fn new(record: DeclId, tag: TagKind, outer: DeclId) -> Self {
        let access = if tag == TagKind::Class { Access::Private } else { Access::Public };
        ClassBuild {
            record,
            access,
            bits: DefinitionBits::default(),
            first_virtual: None,
            pending_nsdmi: 0,
            pending_default_args: 0,
            friends: Vec::new(),
            outer,
        }
    }
}

pub(crate) use construct::ConstructSite;
pub(crate) use ctor_select::InitStyle;
pub(crate) use ctorinit::MemInitState;
pub(crate) use member_expr::MemberRef;
pub(crate) use special::{CtorCall, DeletedAs, DiagSites, InitEntity, SpecialState, SynthContext};
// WP-074: the builtin type traits select special members.
pub(crate) use access::AccessScope;
pub(crate) use special::{Selected, SpecialKind};
pub(crate) use r#virtual::{AbstractUse, VirtualSite, ensure_virtuals, is_virtual};

/// Bits of [`Sema`]'s `implicit_declared` map: which implicit special
/// members of a class have been declared (spec/sema-class.md §4.1).
pub mod implicit {
    /// The default constructor.
    pub const DEFAULT_CTOR: u8 = 1;
    /// The copy constructor.
    pub const COPY_CTOR: u8 = 1 << 1;
    /// The move constructor.
    pub const MOVE_CTOR: u8 = 1 << 2;
    /// The copy assignment operator.
    pub const COPY_ASSIGN: u8 = 1 << 3;
    /// The move assignment operator.
    pub const MOVE_ASSIGN: u8 = 1 << 4;
    /// The destructor.
    pub const DTOR: u8 = 1 << 5;
}

/// One access check delayed until the declaration it belongs to exists
/// (spec/sema-class.md §2.4, [class.access]/7; WP-066b).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessCheck {
    /// The member found.
    pub member: DeclId,
    /// The naming class ([class.access.base]/5).
    pub naming_class: DeclId,
    /// The class of the object expression, for [class.protected]
    /// (`None` for a static member or no object).
    pub object_class: Option<DeclId>,
    /// Where the name was written.
    pub loc: Loc,
    /// Made substituting an alias template: reported at every use, even
    /// at the same location (clang re-substitutes the alias each time).
    pub repeat: bool,
}

/// Whether the current full-expression needs an `ExprWithCleanups`
/// (spec/sema-class.md §5.3; WP-067b). Saved and reset on entering each
/// full-expression context, restored on exit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CleanupState {
    /// A temporary with a non-trivial destructor was bound.
    pub needs: bool,
    /// The cleanups have side effects (the `ExprWithCleanups` aux bit).
    pub side_effects: bool,
}

/// The NRVO state of a local scope (spec/sema-class.md §5.5; clang 19
/// `Scope::ReturnSlots` / `Scope::NRVO`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NrvoState {
    /// The variables declared in this scope that may still occupy the
    /// return slot (clang `ReturnSlots`; parameters are never added).
    pub slots: Vec<DeclId>,
    /// `None`: no `return` reached this scope; `Some(None)`: NRVO is
    /// disabled through it; `Some(Some(x))`: `x` is the candidate.
    pub nrvo: Option<Option<DeclId>>,
}

impl Sema<'_, '_> {
    /// The innermost class being defined, if any.
    pub fn current_class(&self) -> Option<&ClassBuild> {
        self.classes.last()
    }

    /// The class-virt-specifier `final` at `loc` of class `r`
    /// ([class.pre]/1, spec/sema-class.md §2.1 step 2a): called after
    /// `act_on_start_record_definition` and before
    /// [`Sema::act_on_base_specifiers`]. Sets `DefinitionBits::FINAL`,
    /// merged into the definition at `}`.
    pub fn act_on_class_final(&mut self, r: DeclId, loc: Loc) {
        let _ = loc;
        if let Some(c) = self.classes.last_mut()
            && c.record == r
        {
            c.bits = c.bits.with(DefinitionBits::FINAL);
        }
    }

    /// The base-clause of class `r` (spec/sema-class.md §3.1; WP-066b,
    /// virtual bases WP-078a). Called once per class definition, after
    /// `act_on_start_record_definition` and before any member, with an
    /// empty slice when there is no base-clause. Invalid specifiers are
    /// diagnosed and dropped; one that fails clang's `CheckBaseSpecifier`
    /// (not a class, a union, incomplete, `final`, or itself invalid)
    /// makes the class invalid (spec/sema-class.md §2.3), a duplicate or
    /// unresolved base name does not.
    pub fn act_on_base_specifiers(&mut self, r: DeclId, specs: &[BaseSpecInfo]) {
        // [dcl.attr.annotation]/1 allows annotations on a base-specifier;
        // kiln has no key for one in the annotation table yet
        // (spec/annotations.md §9).
        for b in specs {
            for &a in &b.attrs {
                let n = *self.ast.attrs.get(a);
                if n.kind == kiln_ast::attr::AttrKind::ANNOTATION {
                    self.unsupported("annotations on a base-specifier", n.loc);
                }
            }
        }
        let _ = self.attach_bases(r, specs);
    }

    /// `public:` / `protected:` / `private:` at `loc` in the class being
    /// defined ([class.access.spec]; WP-066b): changes the current access
    /// and returns the `AccessSpec` declaration (kept for the dump).
    pub fn act_on_access_spec(&mut self, access: Access, loc: Loc) -> Option<DeclId> {
        let c = self.classes.last_mut()?;
        c.access = access;
        let r = c.record;
        Some(self.builder().access_spec(r, access, loc))
    }

    /// A friend declaration in the class being defined ([class.friend];
    /// WP-066b). `d` is the declarator of a friend function declaration
    /// (`friend void f();`, one call per declarator), or `None` for a
    /// friend type (`friend class X;`, `friend T;`) whose type is `base`
    /// (an elaborated `class X` was first passed to `act_on_tag` with
    /// [`crate::TagUse::Friend`]). `base` is the type of the
    /// decl-specifier-seq, as for [`Sema::act_on_declarator`]. Returns the
    /// `Friend` declaration; a friend function's own declaration is its
    /// target, entered in the enclosing namespace with
    /// `LookupFlags::ADL_ONLY` when first declared here.
    pub fn act_on_friend(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: Option<&Declarator>,
        friend_loc: Loc,
    ) -> Option<DeclId> {
        let base = self.ctad_friend_check(ds, base, d);
        self.declare_friend(ds, base, d, friend_loc)
    }

    /// One friend-type-specifier of a friend-type-specifier-list
    /// (`friend A, struct B, Ts...;`, [class.friend]/3, P2893), whose type
    /// is `base`; `ellipsis` is its `...`, making it a pack expansion
    /// ([temp.variadic]/5.6) that befriends each element when its class
    /// template is instantiated. A `...` after a type that names no pack
    /// is "pack expansion does not contain any unexpanded parameter
    /// packs" at the `...` (nothing is declared). Returns the `Friend`
    /// declaration.
    pub fn act_on_friend_type(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        ellipsis: Option<Loc>,
        friend_loc: Loc,
    ) -> Option<DeclId> {
        let base = match ellipsis {
            Some(at) if !self.is_error_type(base) && !self.has_unexpanded_pack(base) => {
                self.friend_tag = None;
                self.error(DiagId::sema_pack_expansion_without_packs, at);
                return None;
            }
            Some(_) if self.is_error_type(base) => base,
            Some(_) => self.ast.types.pack_expansion(base, None),
            None => base,
        };
        self.act_on_friend(ds, base, None, friend_loc)
    }

    /// `= default;` after the declarator of function `f`
    /// ([dcl.fct.def.default], spec/sema-class.md §4.4; WP-067a). `loc` is
    /// the `default` keyword.
    pub fn act_on_defaulted(&mut self, f: DeclId, loc: Loc) {
        self.defaulted_impl(f, loc);
    }

    /// `= delete;` / `= delete("msg");` after the declarator of function
    /// `f` ([dcl.fct.def.delete], spec/sema-class.md §4.4; WP-067a). `msg`
    /// is the C++26 string-literal message.
    pub fn act_on_deleted(&mut self, f: DeclId, loc: Loc, msg: Option<ExprId>) {
        self.deleted_impl(f, loc, msg);
    }

    /// The default argument `e` of parameter `p` ([dcl.fct.default]),
    /// write-once: called by the parser when it replays a saved one in a
    /// complete-class context (spec/sema-class.md §2.7, the parameter then
    /// carries `ParamData::DEFAULT_ARG_PENDING`); one parsed in place goes
    /// through [`Sema::act_on_param_default_init`]. WP-066b (temporaries in
    /// defaults: WP-067b).
    pub fn act_on_param_default_arg(&mut self, p: DeclId, e: ExprId, eq: Loc) {
        self.unparsed_default_args.remove(&p);
        // [temp.variadic]/12, as `act_on_param_default_init`.
        let at = self.begin_loc(e);
        let op = crate::templ::pack::PackOperand::Expr(e);
        if self.lambdas.is_empty() && self.diagnose_unexpanded_pack(op, "default argument", at) {
            self.ast.decls.set_flags(p, DeclFlags::INVALID);
            let _ = self.ast.decls.set_default_arg(p, e);
            return;
        }
        // [dcl.fct.default]/3: dropped from a parameter pack (`eq` is
        // the replayed `=`; invalid for an instantiated argument).
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        if eq.is_valid() && !self.has_errors_in(e) && self.reject_param_pack_default(pty, eq) {
            return;
        }
        self.attach_default_arg(p, e);
    }

    /// A call at `callee` to `f` that uses the default argument of `p`
    /// before its complete-class context was parsed ([class.mem.general]/7;
    /// clang `CheckCXXDefaultArgExpr`): "use of default argument to
    /// function ... that is declared later in class ...", noted at the
    /// argument. Returns true then.
    pub(crate) fn diagnose_unparsed_default_arg(
        &mut self,
        f: DeclId,
        p: DeclId,
        callee: ExprId,
    ) -> bool {
        let Some(&at) = self.unparsed_default_args.get(&p) else { return false };
        if self.ast.decls.param(p).is_none_or(|x| x.default_arg.is_some()) {
            return false;
        }
        let class = self.ast.decls.get(f).ctx().map(|c| self.decl_arg(c)).unwrap_or_default();
        let note = self.diag(DiagId::note_default_arg_declared_here, at);
        let fname = self.decl_arg(f);
        let msg = self
            .diag(DiagId::sema_default_arg_declared_later, self.begin_loc(callee))
            .arg(fname)
            .arg(class);
        self.emit(msg.with_note(note));
        true
    }

    /// The braced default argument `= { ... }` of parameter `p`
    /// ([dcl.fct.default]/1, [dcl.init.list]): copy-list-initializes
    /// the parameter's type, as one parsed in place does. Called by the
    /// parser for a replayed member function default argument, and by
    /// instantiation for a pattern's dependent braced default argument
    /// (WP-136). Write-once, like [`Sema::act_on_param_default_arg`].
    pub fn act_on_param_default_list(&mut self, p: DeclId, l: &crate::init::InitList) {
        self.unparsed_default_args.remove(&p);
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        if self.ast.types.is_dependent(pty) || self.init_list_type_dependent(l) {
            // Kept as written (clang's dependent `InitializationSequence`):
            // instantiation list-initializes the substituted type, as
            // `act_on_param_default_init` does for one parsed in place
            // (`const O (&)[N] = {}` binds to a `const O[N]`, WP-782).
            let e = self.dependent_init_list(l);
            self.note_default_init_full_expr();
            let e = self.act_on_full_expr(e);
            let _ = self.ast.decls.set_default_arg(p, e);
            return;
        }
        let loc = self.ast.decls.get(p).loc();
        let note = Some(self.param_decl_note(p));
        let e = self.defer_default_arg_uses(|s| {
            let e = s.init_list_entity(pty, l, loc, crate::conv::ConvCtx::Argument, note);
            s.note_default_init_full_expr();
            s.act_on_full_expr(e)
        });
        let _ = self.ast.decls.set_default_arg(p, e);
    }

    /// The replayed *noexcept-specifier* of member function `f`, whose
    /// type held `ExcSpec::Deferred(f)` and which sat in `unparsed_exc`
    /// (spec/sema-class.md §2.7; WP-066b). `spec` comes from
    /// [`Sema::act_on_noexcept_spec`] or
    /// [`Sema::act_on_dynamic_exception_spec`].
    pub fn act_on_exception_spec(&mut self, f: DeclId, spec: ExcSpec) {
        if self.is_static_method(f)
            && let ExcSpec::NoexceptExpr { expr, .. } = spec
        {
            self.check_this_in_static_exception_spec(expr);
        }
        self.set_member_exception_spec(f, spec);
    }

    /// [expr.prim.this]/3: `this` shall not appear in a static member
    /// function's declaration. Its replayed *noexcept-specifier* was
    /// parsed with `this` in scope (so a member named in it is found as
    /// `this->m`, clang's `CXXThisScopeRAII`), and the first `this`,
    /// written or implicit, is diagnosed: `'this' cannot be implicitly
    /// used in a static member function declaration` (clang
    /// `checkThisInStaticMemberFunctionExceptionSpec`).
    pub(crate) fn check_this_in_static_exception_spec(&mut self, e: ExprId) {
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            if self.ast.exprs.get(x).kind() == ExprKind::CXXThisExpr {
                let implicit = self.ast.exprs.this_is_implicit(x);
                let loc = self.ast.exprs.get(x).loc();
                // Repeated in each instantiation of the class, as clang.
                let msg = self
                    .diag(DiagId::sema_this_static_member_func, loc)
                    .arg(u32::from(implicit))
                    .per_instantiation();
                self.emit(msg);
                return;
            }
            let mut kids = self.ast.exprs.children(x, &self.ast.lists);
            kids.reverse();
            stack.extend(kids);
        }
    }

    /// Before the replayed *noexcept-specifier* of member function `f` is
    /// parsed: `this` may appear in it, with `f`'s cv-qualifiers
    /// ([expr.prim.this]/2). A static member function's gets one too, as
    /// in clang, so that a member it names is diagnosed by
    /// [`Sema::act_on_exception_spec`] rather than as a call without an
    /// object. Returns whether a `this` scope was entered, for
    /// [`Sema::act_on_end_this_scope`].
    pub fn act_on_start_delayed_exception_spec(&mut self, f: DeclId) -> bool {
        if self.kind_of(f) == DeclKind::Function {
            return false;
        }
        if self.is_static_method(f) {
            return self.act_on_start_this_scope(true, kiln_ast::types::Quals::NONE);
        }
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let quals = self
            .ast
            .types
            .proto(ty)
            .map_or(kiln_ast::types::Quals::NONE, |p| p.info().this_quals());
        self.act_on_start_this_scope(true, quals)
    }

    /// The replayed default member initializer of `field`
    /// ([class.mem.general]/11, spec/sema-class.md §2.7): `this` is usable
    /// and the initializer is its own full-expression. WP-066b (temporaries
    /// WP-067b).
    pub fn act_on_field_init(&mut self, field: DeclId, init: Initializer) {
        let fty = self.ast.decls.get(field).ty().unwrap_or(QualType::ERROR);
        let loc = match &init {
            Initializer::Expr(e) => self.begin_loc(*e),
            // clang's in-class initializer kind is at the initializer.
            Initializer::List(l) => l.lbrace,
            Initializer::Paren(..) => self.ast.decls.get(field).loc(),
        };
        // `= {..}` is copy-list-initialization, `{..}` direct
        // ([dcl.init.general]/15; the style is kept on the field).
        let direct = match &init {
            Initializer::Expr(_) => false,
            Initializer::Paren(..) => true,
            Initializer::List(_) => self
                .ast
                .decls
                .field(field)
                .is_some_and(|f| f.init_style() == kiln_ast::decl::InitStyle::List),
        };
        if self.is_error_type(fty) {
            if let Initializer::Expr(e) = init {
                self.act_on_full_expr(e);
            }
        } else if let Initializer::List(l) = &init
            && (self.ast.types.is_dependent(fty) || self.init_list_type_dependent(l))
        {
            // Initialized when the class is instantiated ([temp.dep.expr]),
            // the list kept as written (as for a variable).
            let e = self.dependent_init_list(l);
            let e = self.act_on_full_expr(e);
            let _ = self.ast.decls.set_field_init(field, e);
        } else {
            let e = self.with_init_entity(crate::init::entity::MEMBER, |s| {
                s.perform_init(fty, init, loc, direct)
            });
            self.check_bit_field_init(field, e, loc);
            self.note_default_init_full_expr();
            let e = self.act_on_full_expr(e);
            // clang `ActOnFinishCXXInClassMemberInitializer`: a field whose
            // initializer failed is invalid, so no use of it (a
            // constructor, aggregate initialization, an instantiation)
            // reports again.
            if self.is_err_expr(e) {
                self.ast.decls.set_flags(field, DeclFlags::INVALID);
            }
            let _ = self.ast.decls.set_field_init(field, e);
        }
        if self.nsdmi_this.last().is_some_and(|&(f, _)| f == field) {
            self.nsdmi_this.pop();
        }
    }

    /// Before the replayed default member initializer of `field` is
    /// parsed (spec/sema-class.md §2.7): `this` has type `C*`
    /// ([expr.prim.this]/2).
    pub fn act_on_start_field_init(&mut self, field: DeclId) {
        let class = self.ast.decls.get(field).ctx().unwrap_or(self.ctx);
        let t = self.this_class_type(class);
        let p = self.ast.types.pointer_to(t);
        self.nsdmi_this.push((field, p));
    }

    /// After the outermost class `r` (not nested in a class being defined)
    /// and all its complete-class contexts are parsed (spec/sema-class.md
    /// §2.1 step 6; WP-066b/067a): resolves the deferred exception
    /// specifications of explicitly defaulted members and runs clang's
    /// `ActOnFinishCXXNonNestedClass` checks.
    pub fn act_on_finish_class_contexts(&mut self, r: DeclId) {
        self.act_on_virtual_class_contexts(r, 0);
    }

    /// Re-enters the scope of class `r` to replay its saved complete-class
    /// contexts after the outermost class's `}` (spec/sema-class.md §2.7,
    /// clang `ActOnStartDelayedMemberDeclarations`). Paired with
    /// [`Sema::act_on_finish_delayed_class`]; nests for nested classes.
    pub fn act_on_start_delayed_class(&mut self, r: DeclId) {
        self.scopes.push(Scope::new(ScopeKind::Record, Some(r)));
        self.ctx = r;
    }

    /// Leaves the scope entered by [`Sema::act_on_start_delayed_class`].
    pub fn act_on_finish_delayed_class(&mut self, r: DeclId) {
        if self.scopes.last().is_some_and(|s| s.kind == ScopeKind::Record && s.entity == Some(r)) {
            self.scopes.pop();
        }
        self.ctx = self
            .ast
            .decls
            .lexical_ctx(r)
            .or_else(|| self.ast.decls.get(r).ctx())
            .unwrap_or(self.ast.tu);
    }

    /// The body of function `f` defined in a class is saved for replay
    /// after the outermost class ([class.mem.general]/7): `f` will be a
    /// definition (`Sema::delayed_bodies`).
    pub fn act_on_delayed_body(&mut self, f: DeclId) {
        self.delayed_bodies.insert(f);
    }

    /// Re-enters the parameter scope of member function `f` to replay its
    /// default arguments and *noexcept-specifier* (spec/sema-class.md §2.7,
    /// clang `ActOnReenterCXXMethodParameter`): the earlier parameters are
    /// visible ([basic.scope.param]). Paired with
    /// [`Sema::act_on_finish_delayed_method`].
    pub fn act_on_start_delayed_method(&mut self, f: DeclId) {
        self.scopes.push(Scope::new(ScopeKind::Prototype, None));
        let params: Vec<DeclId> = self
            .ast
            .decls
            .function(f)
            .map(|x| self.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        for p in params {
            if let Some(sym) = self.ast.decls.get(p).name().as_ident() {
                self.push_local(sym, p);
            }
        }
    }

    /// Leaves the scope entered by [`Sema::act_on_start_delayed_method`].
    pub fn act_on_finish_delayed_method(&mut self, f: DeclId) {
        let _ = f;
        if self.scopes.last().is_some_and(|s| s.kind == ScopeKind::Prototype) {
            self.scopes.pop();
        }
    }

    /// `this` at `loc` ([expr.prim.this], spec/sema-class.md §2.5;
    /// WP-066b).
    pub fn act_on_this(&mut self, loc: Loc) -> ExprId {
        if let Some(t) = self.current_this() {
            self.note_this_use(loc);
            self.builder().this_expr(t, false, loc)
        } else {
            let explicit_obj = self.func.as_ref().is_some_and(|f| self.has_explicit_object(f.decl));
            let id = if explicit_obj {
                DiagId::sema_explicit_object_this
            } else {
                DiagId::sema_this_outside_member
            };
            self.error(id, loc);
            self.act_on_error_expr(&[], loc)
        }
    }

    /// Whether a member access into class `r` may treat it as complete
    /// although it is still being defined: `this` may appear here outside
    /// a member function body (a trailing return type or
    /// noexcept-specifier, [expr.prim.this]/2) and `r` is being defined
    /// (clang `isThisOutsideMemberFunctionBody`). Elsewhere before the
    /// class's `}` (a member's type, a static member's initializer, an
    /// array bound), [expr.ref]/4 needs a complete class:
    /// `member access into incomplete type`.
    pub(crate) fn this_outside_member_body(&self, r: DeclId) -> bool {
        let first = self.ast.first(r);
        let this_scope = !self.nsdmi_this.is_empty()
            || self.func.as_ref().is_some_and(|f| f.this_override.is_some());
        this_scope && self.defining.iter().any(|&d| self.ast.first(d) == first)
    }

    /// The type of `this` where an expression is being analysed, if
    /// `this` may appear there ([expr.prim.this]/2-3).
    pub(crate) fn current_this(&self) -> Option<QualType> {
        let t = if let Some(t) = self.func.as_ref().and_then(|f| f.this_override) {
            Some(t)
        } else if let Some(&(_, t)) = self.nsdmi_this.last() {
            Some(t)
        } else {
            self.func.as_ref().and_then(|f| f.this_ty)
        };
        // [expr.prim.this]/2: a pointer to const in a contract predicate.
        if self.contract_ctx.is_empty() { t } else { t.map(|t| self.contract_this(t)) }
    }
}
