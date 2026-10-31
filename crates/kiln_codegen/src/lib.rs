//! Lowering of the typed AST to LLVM IR.
//!
//! The lowering mirrors clang 19 at `-O0` instruction for instruction so
//! that kiln's output can be diffed against the clang oracle; see
//! `spec/codegen.md` for the contract this crate implements.
#![forbid(unsafe_code)]
// The emitters mirror clang's CodeGen function by function: long
// per-construct dispatch functions, case arms kept 1:1 with clang's even
// when two bodies coincide, and the short operand names (`l`, `r`, `q`,
// `t`, `v`) of IR builder code. Emitters return `R<_>` uniformly even
// where the M2 subset cannot fail yet.
#![allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::match_same_arms,
    clippy::unnecessary_wraps
)]

mod builtins;
mod class;
mod cleanup;
mod consts;
mod dyncast;
mod expr;
mod fir;
mod fold;
mod func;
mod memptr;
mod module;
mod new_delete;
mod stmt;
mod vtable;

use kiln_abi::Target;
use kiln_ast::Ast;
use kiln_llvm::Module;
use kiln_oven::Oven;
use kiln_source::Loc;

/// Options controlling code generation.
#[derive(Clone, Debug)]
pub struct CodegenOptions {
    /// The module's `source_filename`.
    pub source_filename: String,
    /// C++ exceptions are enabled (`-fexceptions`, the default). When
    /// disabled every function is `nounwind`.
    pub exceptions: bool,
    /// The `llvm.ident` string; empty for none.
    pub ident: String,
}

impl Default for CodegenOptions {
    fn default() -> Self {
        CodegenOptions { source_filename: String::new(), exceptions: true, ident: String::new() }
    }
}

/// A construct codegen does not support (yet), or an internal failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodegenError {
    /// Where the offending construct is, when known.
    pub loc: Option<Loc>,
    /// What went wrong.
    pub msg: String,
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for CodegenError {}

/// Lowers a translation unit to an LLVM module.
///
/// `ast` must be free of errors. Constant expressions are folded with
/// `oven` first (which may cache values in `ast`).
pub fn emit(
    ast: &mut Ast,
    oven: &mut Oven,
    target: &Target,
    opts: &CodegenOptions,
) -> Result<Module, CodegenError> {
    let folds = fold::prepass(ast, oven, target);
    let void_ptr = ast.types.pointer_to(kiln_ast::QualType::VOID);
    module::Cg::new(&*ast, &folds, target, opts, void_ptr).run()
}
