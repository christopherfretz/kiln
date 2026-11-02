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

mod asm;
mod atomic;
mod builtins;
mod checked;
mod class;
mod cleanup;
mod complex;
mod consts;
mod coro;
mod dyncast;
mod eh;
mod expr;
mod fir;
mod fold;
mod func;
mod global_init;
mod lambda;
mod memptr;
mod module;
mod module_init;
mod new_delete;
mod null_init;
mod stmt;
mod tls;
mod try_catch;
mod va_arg;
mod vla;
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
    /// `AArch64` only: `+outline-atomics` in the functions'
    /// `target-features` (`-moutline-atomics`, the default), else
    /// `-outline-atomics` (`-mno-outline-atomics`). Ignored on x86-64.
    pub outline_atomics: bool,
    /// How `_Complex` floating multiply and divide are lowered
    /// (`-fcx-limited-range`, `-fcomplex-arithmetic=`; clang's
    /// `LangOptions::ComplexRange`).
    pub complex_range: ComplexRange,
}

/// clang's `LangOptions::ComplexRangeKind` (spec/abi.md §8.17): the
/// algorithm for `_Complex` floating `*` and `/` when both operands are
/// complex.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ComplexRange {
    /// `full` (the default, `-fno-cx-limited-range`): the algebraic
    /// product with a NaN check falling back to `__mul?c3`; division by
    /// `__div?c3`.
    #[default]
    Full,
    /// `improved` (`-fcx-fortran-rules`): the algebraic product; Smith's
    /// range-reducing division (`EmitRangeReductionDiv`).
    Improved,
    /// `basic` (`-fcx-limited-range`, `-ffast-math`): the algebraic
    /// product and quotient (`EmitAlgebraicDiv`).
    Basic,
}

impl ComplexRange {
    /// The `-fcomplex-arithmetic=` value kiln supports (`promoted` is
    /// not: it widens the operands, spec/abi.md §8.17).
    #[must_use]
    pub fn from_arg(v: &str) -> Option<ComplexRange> {
        match v {
            "full" => Some(ComplexRange::Full),
            "improved" => Some(ComplexRange::Improved),
            "basic" => Some(ComplexRange::Basic),
            _ => None,
        }
    }
}

impl Default for CodegenOptions {
    fn default() -> Self {
        CodegenOptions {
            source_filename: String::new(),
            exceptions: true,
            ident: String::new(),
            outline_atomics: true,
            complex_range: ComplexRange::Full,
        }
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
/// `oven` first (which may cache values in `ast`). Warnings codegen
/// reports itself are dropped; see [`emit_with_diags`].
pub fn emit(
    ast: &mut Ast,
    oven: &mut Oven,
    target: &Target,
    opts: &CodegenOptions,
) -> Result<Module, CodegenError> {
    emit_with_diags(ast, oven, target, opts).0
}

/// [`emit`], also returning the warnings codegen reports itself (clang
/// reports them from `CodeGenModule`, e.g. `-Watomic-alignment`), in
/// emission order, for the caller to emit to its engine.
pub fn emit_with_diags(
    ast: &mut Ast,
    oven: &mut Oven,
    target: &Target,
    opts: &CodegenOptions,
) -> (Result<Module, CodegenError>, Vec<kiln_diag::Diag>) {
    let folds = fold::prepass(ast, oven, target);
    let void_ptr = ast.types.pointer_to(kiln_ast::QualType::VOID);
    module::Cg::new(&*ast, &folds, target, opts, void_ptr).run()
}
