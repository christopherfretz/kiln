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
pub mod overload;
mod rtti;
pub mod scope;
mod side_effects;
pub mod stack_guard;
pub mod stmt;
pub mod target;
pub mod templ;
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
    /// The operands of the `decltype`s being parsed, innermost last: the
    /// unevaluated depth of each and the calls with an incomplete class
    /// prvalue result it deferred ([dcl.type.decltype]/2).
    pub(crate) decltype_calls: Vec<(u32, Vec<deduce::DeferredCall>)>,
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
    /// Taken out for the duration of an evaluation, so the bridge can
    /// reach the rest of `Sema` (`ensure_body` instantiates through it,
    /// spec/sema-templates.md §4.11); `None` only while one runs.
    pub(crate) oven: Option<Oven>,
    /// The implicitly declared global allocation functions.
    pub(crate) global_ops: new_delete::GlobalOps,
    /// Braced-init-list call arguments awaiting their parameter types
    /// (placeholders of [`Sema::act_on_braced_arg`]).
    pub(crate) braced_args: Vec<(ExprId, InitList)>,
    /// The parameters declared so far in each enclosing
    /// parameter-declaration-clause being parsed
    /// ([`Sema::act_on_start_prototype`]).
    pub(crate) prototype_params: Vec<decl::PrototypeFrame>,
    /// Parameters created by [`Sema::act_on_prototype_param`] that no
    /// function has adopted yet ([`Sema::create_params`]).
    pub(crate) unowned_params: std::collections::HashSet<DeclId>,
    /// The declared (placeholder) return type of each function whose
    /// return type was deduced, by first declaration: redeclarations are
    /// matched against it ([dcl.spec.auto.general]/13).
    pub(crate) declared_returns: kiln_source::fx::FxHashMap<DeclId, QualType>,
    /// The declared placeholder type and decl-specifiers' type location
    /// of each variable of a multi-declarator group in a template
    /// pattern: instantiation checks the group's deductions again
    /// ([dcl.spec.auto.general]/8).
    pub(crate) group_placeholders: kiln_source::fx::FxHashMap<DeclId, (QualType, Loc)>,
    /// Set while [`Sema::act_on_call_name`] builds an unqualified callee:
    /// a function it names is not marked referenced until the call is
    /// known to be non-dependent ([temp.dep.candidate]), and ADL applies
    /// (spec/sema-overload.md §2.2).
    pub(crate) deferring_call_ref: bool,
    /// Overload resolution's pooled candidate sets and counters
    /// (spec/sema-overload.md §1.4).
    pub(crate) ovl: overload::OverloadScratch,
    /// Notes the next error takes (the user-declared candidates of an
    /// operator whose built-in fallback fails, spec/sema-overload.md
    /// §6.2): set and cleared around the fallback.
    pub(crate) trailing_notes: std::cell::RefCell<Vec<Diag>>,
    /// Callees whose function is not marked referenced yet (see
    /// `deferring_call_ref`), resolved by [`Sema::act_on_call`].
    pub(crate) deferred_call_refs: Vec<(ExprId, DeclId)>,
    /// Classes being defined, innermost last (spec/sema-class.md §8.3).
    pub(crate) classes: Vec<class::ClassBuild>,
    /// Which implicit special members each class has declared
    /// (`SpecialKind::bit`s; spec/sema-class.md §4.1).
    pub(crate) implicit_declared: kiln_source::fx::FxHashMap<DeclId, u8>,
    /// Special-member state (spec/sema-class.md §8.3).
    pub(crate) special: class::SpecialState,
    /// Access checks delayed until their declaration exists
    /// (spec/sema-class.md §2.4).
    pub(crate) delayed_access: Vec<class::AccessCheck>,
    /// `delayed_access.len()` at each open delay point, innermost last.
    pub(crate) delayed_access_depth: Vec<usize>,
    /// The current full-expression's cleanup state (spec/sema-class.md
    /// §5.3).
    pub(crate) cleanup: class::CleanupState,
    /// The cleanup states of the enclosing full-expressions around
    /// unevaluated operands, innermost last.
    pub(crate) cleanup_saved: Vec<class::CleanupState>,
    /// Implicit special members being defined (spec/sema-class.md §4.5),
    /// innermost last: diagnostics emitted meanwhile get the note `in
    /// implicit ... for 'T' first required here`.
    pub(crate) synth: Vec<class::SynthContext>,
    /// The depth of `synth` whose notes were last printed (clang
    /// `LastEmittedCodeSynthesisContextDepth`).
    pub(crate) synth_noted: std::cell::Cell<usize>,
    /// The internal `__builtin_memcpy` declaration (spec/sema-class.md
    /// §4.5), once declared.
    pub(crate) memcpy_decl: Option<DeclId>,
    /// Delegating constructors and their targets, checked for cycles at
    /// the end of the translation unit ([class.base.init]/6).
    pub(crate) delegating: Vec<(DeclId, DeclId, Loc)>,
    /// The subobject the next constructor call initializes, set by a
    /// mem-initializer and consumed by the outermost constructor call it
    /// builds (clang's `InitializedEntity` for bases, members and
    /// delegation; spec/sema-class.md §4.9).
    pub(crate) ctor_entity: Option<(class::InitEntity, kiln_ast::expr::ConstructionKind)>,
    /// The mem-initializers of the constructor definition being parsed
    /// (spec/sema-class.md §4.9).
    pub(crate) mem_inits: class::MemInitState,
    /// Materialized temporaries collected for P2718 range-for lifetime
    /// extension (spec/sema-class.md §5.4).
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
    /// Template heads and parse-time template state (WP-069,
    /// spec/sema-templates.md §11.2).
    pub(crate) templ: templ::TemplState,
    /// The instantiation engine (spec/sema-templates.md §11.2; WP-070a).
    pub inst: templ::InstState,
    /// The abstract classes whose unimplemented pure virtual functions
    /// were already noted (clang notes them on the first abstract-type
    /// diagnostic per class only; WP-078a).
    pub(crate) abstract_noted: kiln_source::fx::FxHashSet<DeclId>,
    /// State of the source-location builtins (`builtins.rs`).
    pub(crate) srcloc: builtins::SrcLocCache,
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

    fn with_abi(
        mut ast: Ast,
        diags: &'a DiagEngine<'sm>,
        abi: Target,
        target: TargetTypes,
    ) -> Self {
        let tu = ast.tu;
        templ::builtins::declare_builtin_templates(&mut ast);
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
            decltype_calls: Vec::new(),
            immediate: 0,
            pending_odr: Vec::new(),
            defining: Vec::new(),
            enums: Vec::new(),
            local_externs: Vec::new(),
            oven: Some(Oven::new(Budget::default())),
            global_ops: new_delete::GlobalOps::default(),
            braced_args: Vec::new(),
            prototype_params: Vec::new(),
            unowned_params: std::collections::HashSet::new(),
            declared_returns: kiln_source::fx::FxHashMap::default(),
            group_placeholders: kiln_source::fx::FxHashMap::default(),
            deferring_call_ref: false,
            ovl: overload::OverloadScratch::default(),
            trailing_notes: std::cell::RefCell::new(Vec::new()),
            deferred_call_refs: Vec::new(),
            classes: Vec::new(),
            implicit_declared: kiln_source::fx::FxHashMap::default(),
            special: class::SpecialState::default(),
            delayed_access: Vec::new(),
            delayed_access_depth: Vec::new(),
            cleanup: class::CleanupState::default(),
            cleanup_saved: Vec::new(),
            synth: Vec::new(),
            synth_noted: std::cell::Cell::new(0),
            memcpy_decl: None,
            delegating: Vec::new(),
            ctor_entity: None,
            mem_inits: class::MemInitState::default(),
            extend_collector: None,
            unparsed_exc: kiln_source::fx::FxHashSet::default(),
            fn_saved: Vec::new(),
            member_refs: kiln_source::fx::FxHashMap::default(),
            friend_tag: None,
            nsdmi_this: Vec::new(),
            initializing: None,
            templ: templ::TemplState::default(),
            inst: templ::InstState::default(),
            abstract_noted: kiln_source::fx::FxHashSet::default(),
            srcloc: builtins::SrcLocCache::default(),
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

    /// Emits `d`. Inside an instantiation (spec/sema-templates.md §10.2)
    /// the context notes are added to a diagnostic that shows, and a
    /// SFINAE context captures errors instead (§5.7). Inside an implicit
    /// special member definition the `in implicit ... first required
    /// here` note (spec/sema-class.md §4.5) goes first, as the innermost
    /// code-synthesis context.
    pub fn emit(&self, mut d: Diag) {
        if self.inst.suppress_next_error.replace(false) {
            return;
        }
        if !self.trailing_notes.borrow().is_empty()
            && d.id().is_some_and(|id| self.diags.severity(id) >= kiln_diag::Severity::Error)
        {
            for n in self.trailing_notes.take() {
                d = d.with_note(n);
            }
        }
        if !self.inst.stack.is_empty()
            && let Some(id) = d.id()
        {
            // Dedup is per instantiation (spec/diag.md §7.4).
            d = d.context(self.inst.stack.context_key());
            let sev = self.diags.severity(id);
            if self.inst.stack.is_sfinae() {
                if sev >= kiln_diag::Severity::Error
                    && let Some(s) = self.inst.sfinae.borrow_mut().last_mut()
                {
                    s.errors += 1;
                    if s.first.is_none() {
                        s.first = Some(d);
                    }
                }
                return;
            }
            // Only a diagnostic that will print takes the context notes,
            // which print once per depth (§10.2).
            if self.diags.would_emit(&d) {
                d.prepend_notes(self.inst_context_notes());
            }
        }
        let d = self.with_synth_note(d);
        d.emit(self.diags);
    }

    /// Emits a diagnostic with no arguments at `loc`.
    pub fn error(&self, id: DiagId, loc: Loc) {
        self.emit(self.diag(id, loc));
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
        self.decl_name_str(self.ast.decls.get(d).name())
    }

    /// `name` as clang prints it in a diagnostic: identifiers, `operator+`,
    /// the class name for constructors, `~X`, `operator int`.
    pub fn decl_name_str(&self, name: kiln_ast::names::DeclName) -> String {
        self.ast.names.simple_spelling(name, self.ast.symbols()).unwrap_or_else(|| {
            let mut s = String::new();
            kiln_ast::dump::AstNames::new(&self.ast).write_decl_name_value(name, &mut s);
            s
        })
    }
}
