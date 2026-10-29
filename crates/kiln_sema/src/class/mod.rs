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

use kiln_ast::decl::{Access, DefinitionBits, TagKind};
use kiln_ast::types::ExcSpec;
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
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
}

impl ClassBuild {
    /// A fresh entry for `record` with the default access of `tag`.
    pub fn new(record: DeclId, tag: TagKind) -> Self {
        let access = if tag == TagKind::Class { Access::Private } else { Access::Public };
        ClassBuild {
            record,
            access,
            bits: DefinitionBits::default(),
            first_virtual: None,
            pending_nsdmi: 0,
            pending_default_args: 0,
        }
    }
}

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

/// The NRVO candidate of a block scope (spec/sema-class.md §5.5;
/// WP-067b, clang `Scope::updateNRVOCandidate`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NrvoState {
    /// No `return` seen in this scope.
    #[default]
    None,
    /// Every `return` so far returned this candidate variable.
    Candidate(DeclId),
    /// Some `return` returned something else: no NRVO through this scope.
    Disabled,
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
    /// diagnosed and dropped; the class stays valid.
    pub fn act_on_base_specifiers(&mut self, r: DeclId, specs: &[BaseSpecInfo]) {
        let _ = r;
        if let Some(first) = specs.first() {
            self.unsupported("base classes", first.loc);
        }
    }

    /// `public:` / `protected:` / `private:` at `loc` in the class being
    /// defined ([class.access.spec]; WP-066b): changes the current access
    /// and returns the `AccessSpec` declaration (kept for the dump).
    pub fn act_on_access_spec(&mut self, access: Access, loc: Loc) -> Option<DeclId> {
        let _ = access;
        self.unsupported("access specifiers", loc);
        None
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
        let _ = (ds, base, d);
        self.unsupported("friend declarations", friend_loc);
        None
    }

    /// `= default;` after the declarator of function `f`
    /// ([dcl.fct.def.default], spec/sema-class.md §4.4; WP-067a). `loc` is
    /// the `default` keyword.
    pub fn act_on_defaulted(&mut self, f: DeclId, loc: Loc) {
        let _ = f;
        self.unsupported("defaulted functions", loc);
    }

    /// `= delete;` / `= delete("msg");` after the declarator of function
    /// `f` ([dcl.fct.def.delete], spec/sema-class.md §4.4; WP-067a). `msg`
    /// is the C++26 string-literal message.
    pub fn act_on_deleted(&mut self, f: DeclId, loc: Loc, msg: Option<ExprId>) {
        let _ = (f, msg);
        self.unsupported("deleted functions", loc);
    }

    /// The default argument `e` of parameter `p` ([dcl.fct.default]),
    /// write-once: called by [`Sema::act_on_declarator`] for a default
    /// argument parsed in place, and by the parser when it replays a saved
    /// one in a complete-class context (spec/sema-class.md §2.7, the
    /// parameter then carries `ParamData::DEFAULT_ARG_PENDING`). WP-066b
    /// (temporaries in defaults: WP-067b).
    pub fn act_on_param_default_arg(&mut self, p: DeclId, e: ExprId) {
        let _ = p;
        let loc = self.ast.exprs.get(e).loc();
        self.unsupported("default arguments", loc);
    }

    /// The replayed *noexcept-specifier* of member function `f`, whose
    /// type held `ExcSpec::Deferred(f)` and which sat in `unparsed_exc`
    /// (spec/sema-class.md §2.7; WP-066b). `spec` comes from
    /// [`Sema::act_on_noexcept_spec`] or
    /// [`Sema::act_on_dynamic_exception_spec`].
    pub fn act_on_exception_spec(&mut self, f: DeclId, spec: ExcSpec) {
        let _ = spec;
        self.unparsed_exc.remove(&f);
        let loc = self.ast.decls.get(f).loc();
        self.unsupported("exception specifications of member functions", loc);
    }

    /// The replayed default member initializer of `field`
    /// ([class.mem.general]/11, spec/sema-class.md §2.7): `this` is usable
    /// and the initializer is its own full-expression. WP-066b (temporaries
    /// WP-067b).
    #[expect(clippy::needless_pass_by_value, reason = "the WP-066b body consumes it")]
    pub fn act_on_field_init(&mut self, field: DeclId, init: Initializer) {
        let _ = init;
        let loc = self.ast.decls.get(field).loc();
        self.unsupported("default member initializers", loc);
    }

    /// The ctor-initializer of constructor definition `ctor`
    /// ([class.base.init], spec/sema-class.md §4.9; WP-067b), between
    /// `act_on_start_function_def` and the body. The parser calls it for
    /// every constructor definition: with the mem-initializers and the
    /// `:` location, or with an empty slice and the body's `{` when there
    /// is no ctor-initializer, so the implicit initializers are always
    /// completed here.
    pub fn act_on_mem_initializers(&mut self, ctor: DeclId, inits: &[MemInitInfo], colon_loc: Loc) {
        let _ = ctor;
        if !inits.is_empty() {
            self.unsupported("constructor initializers", colon_loc);
        }
    }

    /// After the outermost class `r` (not nested in a class being defined)
    /// and all its complete-class contexts are parsed (spec/sema-class.md
    /// §2.1 step 6; WP-066b/067a): resolves the deferred exception
    /// specifications of explicitly defaulted members and runs clang's
    /// `ActOnFinishCXXNonNestedClass` checks.
    pub fn act_on_finish_class_contexts(&mut self, r: DeclId) {
        let _ = r;
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

    /// Before the replayed default member initializer of `field` is parsed
    /// (spec/sema-class.md §2.7): the hook where WP-066b makes `this`
    /// usable and opens the initializer's full-expression context. The
    /// parser follows it with [`Sema::act_on_field_init`] when the
    /// initializer parsed (after a syntax error there is no second call).
    pub fn act_on_start_field_init(&mut self, field: DeclId) {
        let _ = field;
    }

    /// `this` at `loc` ([expr.prim.this], spec/sema-class.md §2.5;
    /// WP-066b).
    pub fn act_on_this(&mut self, loc: Loc) -> ExprId {
        self.unsupported("'this'", loc);
        self.act_on_error_expr(&[], loc)
    }
}
