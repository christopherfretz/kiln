//! Semantic analysis: lookup, types, templates, overloads, conversions, initialization, reflection.
//!
//! See `docs/ARCHITECTURE.md` and `spec/sema.md` for the contract
//! this crate implements.
//!
//! The parser (`kiln_parse`) drives a [`Sema`] through `act_on_*` entry
//! points, clang's Parser/Sema split: the parser owns the grammar and the
//! token stream, `Sema` owns scopes, lookup, declarations, types and the
//! typed AST it builds through [`kiln_ast::AstBuilder`]. Conversions live
//! in [`conv`] (`spec/sema-conv.md`) and literal typing in [`literal`].
#![forbid(unsafe_code)]
#![allow(
    clippy::many_single_char_names,
    reason = "sema code uses the standard's short names (e, t, d, l, r)"
)]

mod builtins;
pub mod class;
pub mod constfold;
pub mod conv;
pub mod decl;
pub mod declspec;
mod deduce;
pub mod except;
pub mod expr;
mod forms;
mod gnu;
pub mod init;
pub mod literal;
pub mod lookup;
pub mod names;
pub mod new_delete;
mod oven_bridge;
mod rtti;
pub mod scope;
pub mod stmt;
pub mod target;
pub mod using;

use kiln_abi::Target;
use kiln_ast::types::{PrintOptions, TargetTypes, aka, print_type};
use kiln_ast::{Ast, AstBuilder, DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_source::{Loc, Span};

pub use class::{BaseSpecInfo, MemInitId, MemInitInfo};
pub use decl::{NameClass, TagUse};
pub use declspec::{
    Chunk, DeclSpec, Declarator, DeclaratorContext, DefaultArg, ParamInfo, Sign, SpecialName,
    StorageSpec, TypeSpecKind, Width,
};
pub use gnu::{OffsetofComponent, StmtExprScope};
pub use init::{Designator, InitItem, InitList, InitValue, Initializer};
pub use kiln_oven::{Budget, EvalContext, EvalFailure, EvalKind, Oven};
pub use scope::ScopeKind;

/// The semantic-analysis state of one translation unit (spec/sema.md §1).
///
/// `'a` is the borrow of the diagnostics engine and `'sm` the source
/// manager it reports against (two lifetimes: `DiagEngine` is invariant
/// in `'sm` and has drop glue, spec/sema.md §1).
#[derive(Debug)]
pub struct Sema<'a, 'sm> {
    /// The AST being built.
    pub ast: Ast,
    /// Where diagnostics go.
    pub diags: &'a DiagEngine<'sm>,
    /// The target's builtin type properties (the conversion machinery's
    /// view of the target, spec/sema-conv.md).
    pub target: TargetTypes,
    /// The full target description (layout for `sizeof`/`alignof`,
    /// spec/abi.md).
    pub abi: Target,
    /// The `#pragma pack` directives seen, in order (location and the
    /// spelling of the argument list).
    pub pragma_packs: Vec<(Loc, String)>,
    /// C++ exceptions are enabled (`-f[no-]exceptions`; default on).
    pub exceptions: bool,
    /// Run-time type information is enabled (`-f[no-]rtti`; default on):
    /// `typeid` is an error otherwise.
    pub rtti: bool,
    /// The `#pragma pack` state (clang's `AlignPackStack`): the current
    /// maximum field alignment in bytes (0: none) and the pushed
    /// `(label, value)` entries. Applied to each class definition as it
    /// begins (`record_pack`).
    pub(crate) pack: (u16, Vec<(Option<String>, u16)>),
    pub(crate) scopes: Vec<scope::Scope>,
    /// The current semantic declaration context.
    pub(crate) ctx: DeclId,
    pub(crate) func: Option<stmt::FnState>,
    /// Depth of unevaluated operands (`sizeof`, `alignof`).
    pub(crate) unevaluated: u32,
    /// Depth of immediate function contexts ([expr.const]/16) entered
    /// by `if consteval`.
    pub(crate) immediate: u32,
    /// `DeclRefExpr`s of the current full-expression that are potential
    /// odr-uses ([basic.def.odr]/4): resolved at the end of the
    /// full-expression (spec/sema.md §5.2).
    pub(crate) pending_odr: Vec<ExprId>,
    /// Classes whose definitions are being processed (incomplete until
    /// their closing brace).
    pub(crate) defining: Vec<DeclId>,
    /// Enumerations whose definitions are being processed.
    pub(crate) enums: Vec<decl::EnumBuild>,
    /// Block-scope function and `extern` variable declarations, by
    /// enclosing namespace and name: a later namespace-scope declaration
    /// redeclares them ([basic.link]/6, clang's local extern decls).
    pub(crate) local_externs: Vec<(DeclId, kiln_source::symbol::Symbol, DeclId)>,
    /// The constant evaluator of the translation unit (spec/sema.md §8).
    pub(crate) oven: Oven,
    /// The implicitly declared global allocation functions.
    pub(crate) global_ops: new_delete::GlobalOps,
    /// Braced-init-list call arguments awaiting their parameter types
    /// (placeholders of [`Sema::act_on_braced_arg`]).
    pub(crate) braced_args: Vec<(ExprId, InitList)>,
    /// The names of the parameters declared so far in each enclosing
    /// parameter-declaration-clause being parsed ([`Sema::act_on_start_prototype`]).
    pub(crate) prototype_params: Vec<Vec<kiln_source::symbol::Symbol>>,
    /// Classes being defined, innermost last (spec/sema-class.md §8.3).
    pub(crate) classes: Vec<class::ClassBuild>,
    /// Which implicit special members each class has declared
    /// ([`class::implicit`] bits; spec/sema-class.md §4.1).
    #[expect(dead_code, reason = "filled by WP-067a's implicit special members")]
    pub(crate) implicit_declared: kiln_source::fx::FxHashMap<DeclId, u8>,
    /// Access checks delayed until their declaration exists
    /// (spec/sema-class.md §2.4).
    pub(crate) delayed_access: Vec<class::AccessCheck>,
    /// `delayed_access.len()` at each open delay point, innermost last.
    pub(crate) delayed_access_depth: Vec<usize>,
    /// The current full-expression's cleanup state (spec/sema-class.md
    /// §5.3).
    #[expect(dead_code, reason = "WP-066a interface; read from WP-067b (cleanups)")]
    pub(crate) cleanup: class::CleanupState,
    /// Materialized temporaries collected for P2718 range-for lifetime
    /// extension (spec/sema-class.md §5.4).
    #[expect(dead_code, reason = "WP-066a interface; read from WP-067b (P2718)")]
    pub(crate) extend_collector: Option<Vec<ExprId>>,
    /// Member functions whose *noexcept-specifier* awaits replay
    /// (`ExcSpec::Deferred`, spec/sema-class.md §2.7).
    pub(crate) unparsed_exc: kiln_source::fx::FxHashSet<DeclId>,
    /// The enclosing function's state and the scopes pushed for a member
    /// function definition, saved by `act_on_start_function_def` while a
    /// (local class's or out-of-line) member function body is analysed.
    pub(crate) fn_saved: Vec<class::SavedFn>,
    /// Names of non-static members whose meaning depends on the operand
    /// they appear in (spec/sema-class.md §2.5/§2.6): `&C::m` forms a
    /// pointer to member, anything else needs an object.
    pub(crate) member_refs: kiln_source::fx::FxHashMap<ExprId, class::MemberRef>,
    /// The class the last `friend class X;` declared ([`TagUse::Friend`]),
    /// taken by [`Sema::act_on_friend`] as the `FriendDecl`'s owned tag.
    pub(crate) friend_tag: Option<DeclId>,
    /// The `this` type of the default member initializers being parsed,
    /// innermost last, with their fields (`C*`, spec/sema-class.md §2.5).
    pub(crate) nsdmi_this: Vec<(DeclId, QualType)>,
    /// The static data member whose out-of-line initializer is being
    /// analysed: its class is the access context ([class.access]/7).
    pub(crate) initializing: Option<DeclId>,
    /// The abstract classes whose unimplemented pure virtual functions
    /// were already noted (clang notes them on the first abstract-type
    /// diagnostic per class only; WP-078a).
    pub(crate) abstract_noted: kiln_source::fx::FxHashSet<DeclId>,
}

impl<'a, 'sm> Sema<'a, 'sm> {
    /// A `Sema` over `ast` reporting to `diags`, for the target whose
    /// builtin types are `target` (the layout target is picked from its
    /// `long double` format, [`target::target_for_types`]).
    pub fn new(ast: Ast, diags: &'a DiagEngine<'sm>, target: TargetTypes) -> Self {
        let abi = target::target_for_types(&target);
        Self::with_abi(ast, diags, abi, target)
    }

    /// A `Sema` over `ast` reporting to `diags` for `abi`.
    pub fn for_target(ast: Ast, diags: &'a DiagEngine<'sm>, abi: Target) -> Self {
        let types = abi.types.clone();
        Self::with_abi(ast, diags, abi, types)
    }

    fn with_abi(ast: Ast, diags: &'a DiagEngine<'sm>, abi: Target, target: TargetTypes) -> Self {
        let tu = ast.tu;
        Sema {
            ast,
            diags,
            target,
            abi,
            pragma_packs: Vec::new(),
            exceptions: true,
            rtti: true,
            pack: (0, Vec::new()),
            scopes: vec![scope::Scope::new(ScopeKind::TranslationUnit, Some(tu))],
            ctx: tu,
            func: None,
            unevaluated: 0,
            immediate: 0,
            pending_odr: Vec::new(),
            defining: Vec::new(),
            enums: Vec::new(),
            local_externs: Vec::new(),
            oven: Oven::new(Budget::default()),
            global_ops: new_delete::GlobalOps::default(),
            braced_args: Vec::new(),
            prototype_params: Vec::new(),
            classes: Vec::new(),
            implicit_declared: kiln_source::fx::FxHashMap::default(),
            delayed_access: Vec::new(),
            delayed_access_depth: Vec::new(),
            cleanup: class::CleanupState::default(),
            extend_collector: None,
            unparsed_exc: kiln_source::fx::FxHashSet::default(),
            fn_saved: Vec::new(),
            member_refs: kiln_source::fx::FxHashMap::default(),
            friend_tag: None,
            nsdmi_this: Vec::new(),
            initializing: None,
            abstract_noted: kiln_source::fx::FxHashSet::default(),
        }
    }

    /// The AST, mutably.
    pub fn ast_mut(&mut self) -> &mut Ast {
        &mut self.ast
    }

    /// A builder over the AST.
    pub fn builder(&mut self) -> AstBuilder<'_> {
        self.ast.builder()
    }

    /// The target's builtin type properties.
    pub fn target(&self) -> &TargetTypes {
        &self.target
    }

    /// The full target description.
    pub fn abi(&self) -> &Target {
        &self.abi
    }

    /// The current semantic declaration context.
    pub fn context(&self) -> DeclId {
        self.ctx
    }

    /// Consumes the `Sema`, returning the AST.
    pub fn into_ast(self) -> Ast {
        self.ast
    }

    /// True if any error has been reported.
    pub fn has_errors(&self) -> bool {
        self.diags.error_count() > 0
    }

    // -- diagnostics helpers ---------------------------------------------

    /// A diagnostic `id` with its caret at `loc`.
    pub fn diag(&self, id: DiagId, loc: Loc) -> Diag {
        Diag::new(id, Span::empty(loc))
    }

    /// Emits `d`.
    pub fn emit(&self, d: Diag) {
        d.emit(self.diags);
    }

    /// Emits a diagnostic with no arguments at `loc`.
    pub fn error(&self, id: DiagId, loc: Loc) {
        self.diag(id, loc).emit(self.diags);
    }

    /// `q` as clang prints a type argument: `'T'`, or `'T' (aka 'int')`
    /// when the canonical spelling differs.
    pub fn ty_arg(&self, q: QualType) -> String {
        let names = names::SemaNames::new(self);
        let s = print_type(&self.ast.types, &names, q, PrintOptions::SUGARED);
        match aka(&self.ast.types, &names, q) {
            Some(a) => format!("'{s}' (aka '{a}')"),
            None => format!("'{s}'"),
        }
    }

    /// `q` printed without quotes.
    pub fn ty_str(&self, q: QualType) -> String {
        let names = names::SemaNames::new(self);
        print_type(&self.ast.types, &names, q, PrintOptions::SUGARED)
    }

    /// The qualified name of `d`, as clang prints a declaration argument.
    pub fn qualified_decl_name(&self, d: DeclId) -> String {
        use kiln_ast::types::TypeNames as _;
        let mut s = String::new();
        names::SemaNames::new(self).write_decl_name(d, true, &mut s);
        s
    }

    /// A nested-name-specifier as written (`N::M::`, `::`).
    pub fn nns_str(&self, n: kiln_ast::ids::NnsId) -> String {
        let mut s = String::new();
        kiln_ast::types::write_nns(&self.ast.types, &names::SemaNames::new(self), n, &mut s);
        s
    }

    /// The simple name of `d` (empty when unnamed).
    pub fn decl_name(&self, d: DeclId) -> String {
        let name = self.ast.decls.get(d).name();
        self.ast.names.simple_spelling(name, self.ast.symbols()).unwrap_or_default()
    }
}
