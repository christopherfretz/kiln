//! Semantic analysis: lookup, types, templates, overloads, conversions, initialization, reflection.
//!
//! See `docs/ARCHITECTURE.md` and `spec/sema.md` for the contract
//! this crate implements.
//!
//! This crate currently holds the conversion machinery ([`conv`],
//! `spec/sema-conv.md`) and literal typing ([`literal`]). The [`Sema`]
//! struct here is the minimal shim agreed with the parser package
//! (WP-048), whose `Sema` is a superset of it.
#![forbid(unsafe_code)]

pub mod conv;
pub mod literal;
pub mod names;

use kiln_ast::types::{PrintOptions, TargetTypes, aka, print_type};
use kiln_ast::{Ast, AstBuilder, QualType};
use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_source::{Loc, Span};

/// The semantic-analysis state of one translation unit.
///
/// `'a` is the borrow of the diagnostics engine and `'sm` the source
/// manager it reports against.
#[derive(Debug)]
pub struct Sema<'a, 'sm> {
    /// The AST being built.
    pub ast: Ast,
    /// Where diagnostics go.
    pub diags: &'a DiagEngine<'sm>,
    /// The target's builtin type properties.
    pub target: TargetTypes,
}

impl<'a, 'sm> Sema<'a, 'sm> {
    /// A `Sema` over `ast` reporting to `diags`.
    pub fn new(ast: Ast, diags: &'a DiagEngine<'sm>, target: TargetTypes) -> Self {
        Sema { ast, diags, target }
    }

    /// The AST, mutably.
    pub fn ast_mut(&mut self) -> &mut Ast {
        &mut self.ast
    }

    /// A builder appending to the AST.
    pub fn builder(&mut self) -> AstBuilder<'_> {
        self.ast.builder()
    }

    /// The target's builtin type properties.
    pub fn target(&self) -> &TargetTypes {
        &self.target
    }

    /// Consumes the `Sema`, returning the AST.
    pub fn into_ast(self) -> Ast {
        self.ast
    }

    /// A diagnostic `id` with its caret at `loc`.
    pub fn diag(&self, id: DiagId, loc: Loc) -> Diag {
        Diag::new(id, Span::empty(loc))
    }

    /// Emits `d`.
    pub fn emit(&self, d: Diag) {
        d.emit(self.diags);
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
}
