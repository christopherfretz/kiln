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

mod aggval;
pub mod constfold;
pub mod conv;
pub mod decl;
pub mod declspec;
pub mod expr;
pub mod init;
pub mod literal;
pub mod lookup;
pub mod names;
pub mod scope;
pub mod stmt;
pub mod target;

use kiln_abi::Target;
use kiln_ast::types::{PrintOptions, TargetTypes, aka, print_type};
use kiln_ast::{Ast, AstBuilder, DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_source::{Loc, Span};

pub use constfold::{ConstEvaluator, ConstVal, NotConstant};
pub use decl::{NameClass, TagUse};
pub use declspec::{
    BaseSpec, Chunk, DeclSpec, Declarator, DeclaratorContext, ParamInfo, Sign, StorageSpec, Width,
};
pub use init::{Designator, InitItem, InitList, InitValue, Initializer};
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
    pub(crate) scopes: Vec<scope::Scope>,
    /// The current semantic declaration context.
    pub(crate) ctx: DeclId,
    pub(crate) func: Option<stmt::FnState>,
    /// Depth of unevaluated operands (`sizeof`, `alignof`).
    pub(crate) unevaluated: u32,
    /// `DeclRefExpr`s of the current full-expression that are potential
    /// odr-uses ([basic.def.odr]/4): resolved at the end of the
    /// full-expression (spec/sema.md §5.2).
    pub(crate) pending_odr: Vec<ExprId>,
    /// Classes whose definitions are being processed (incomplete until
    /// their closing brace).
    pub(crate) defining: Vec<DeclId>,
    /// Enumerations whose definitions are being processed.
    pub(crate) enums: Vec<decl::EnumBuild>,
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
            scopes: vec![scope::Scope::new(ScopeKind::TranslationUnit, Some(tu))],
            ctx: tu,
            func: None,
            unevaluated: 0,
            pending_odr: Vec::new(),
            defining: Vec::new(),
            enums: Vec::new(),
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

    /// The simple name of `d` (empty when unnamed).
    pub fn decl_name(&self, d: DeclId) -> String {
        let name = self.ast.decls.get(d).name();
        self.ast.names.simple_spelling(name, self.ast.symbols()).unwrap_or_default()
    }
}
