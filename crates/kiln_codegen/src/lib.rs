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
mod lifetime;
mod loop_md;
mod recursion;
mod memptr;
mod module;
mod module_init;
mod new_delete;
mod null_init;
mod psabi;
mod stmt;
mod tbaa;
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
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent command-line switches, as clang's CodeGenOptions"
)]
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
    /// The optimization level (clang `CodeGenOpts.OptimizationLevel`):
    /// 0 at `-O0`, 1 at `-O`/`-O1`/`-Og`, 2 at `-O2`/`-Os`/`-Oz`, 3 at
    /// `-O3` and above. Above 0 the IR will be optimized (see
    /// [`Self::optimize`]); at exactly 1 loops carry
    /// `llvm.loop.unroll.disable` (spec/codegen.md "Loop metadata").
    pub opt_level: u8,
    /// The size optimization level: 1 at `-Os`, 2 at `-Oz`, else 0
    /// (clang `CodeGenOpts.OptimizeSize`): functions and call sites get
    /// `optsize`, and `minsize` too at 2 (spec/codegen.md "Size
    /// optimization attributes").
    pub opt_size: u8,
    /// Type-based alias analysis applies (`-fstrict-aliasing`, the
    /// default; `-fno-strict-aliasing` is clang's `RelaxedAliasing`):
    /// with [`Self::optimize`], loads and stores carry `!tbaa`
    /// (spec/codegen.md "TBAA").
    pub strict_aliasing: bool,
    /// The clang version whose diagnostic text codegen's warnings follow
    /// (`-fclang-compat=`; spec/driver.md).
    pub clang_compat: u32,
    /// The major version of the LLVM that consumes the IR (`llvm-config
    /// --version` of the driver's `KILN_LLVM_DIR`; spec/driver.md): selects
    /// the IR that changed between the supported versions (19 and 22).
    pub llvm_major: u32,
    /// The last of `-fomit-frame-pointer` (`Some(true)`) and
    /// `-fno-omit-frame-pointer` (`Some(false)`); `None` is the target's
    /// default ([`FramePointer::resolve`]).
    pub omit_frame_pointer: Option<bool>,
    /// The last of `-momit-leaf-frame-pointer` (`Some(true)`) and
    /// `-mno-omit-leaf-frame-pointer` (`Some(false)`); `None` is the
    /// target's default.
    pub omit_leaf_frame_pointer: Option<bool>,
}

/// Which functions keep a frame pointer: the `"frame-pointer"` function
/// attribute and the `frame-pointer` module flag (clang's
/// `CodeGenOptions::FramePointerKind`; spec/codegen.md "Frame pointer").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePointer {
    /// No function keeps one: neither the attribute nor the flag is
    /// emitted.
    None,
    /// Every function but a leaf keeps one (`"non-leaf"`, flag 1).
    NonLeaf,
    /// clang 22's spelling of `NonLeaf`: the frame-pointer register is
    /// not reserved in leaf functions (`"non-leaf-no-reserve"`, flag 4).
    NonLeafNoReserve,
    /// Every function keeps one (`"all"`, flag 2).
    All,
}

impl FramePointer {
    /// clang's driver rule (`getFramePointerKind`): the frame pointer is
    /// omitted when `-fomit-frame-pointer` is last, kept when
    /// `-fno-omit-frame-pointer` is, and otherwise by the target's
    /// default: x86-64 Linux omits it when optimizing (any `-O` above
    /// `-O0`), `AArch64` Linux always keeps it. A kept frame pointer is
    /// omitted in leaf functions when `-momit-leaf-frame-pointer` is
    /// last, kept with `-mno-omit-leaf-frame-pointer`, and by default
    /// omitted on `AArch64` and kept on x86-64.
    #[must_use]
    pub fn resolve(
        x86_64: bool,
        optimize: bool,
        omit: Option<bool>,
        omit_leaf: Option<bool>,
    ) -> FramePointer {
        if omit.unwrap_or(x86_64 && optimize) {
            FramePointer::None
        } else if omit_leaf.unwrap_or(!x86_64) {
            FramePointer::NonLeaf
        } else {
            FramePointer::All
        }
    }

    /// The `"frame-pointer"` attribute value, `None` when none is
    /// emitted.
    #[must_use]
    pub fn attr(self) -> Option<&'static str> {
        match self {
            FramePointer::None => None,
            FramePointer::NonLeaf => Some("non-leaf"),
            FramePointer::NonLeafNoReserve => Some("non-leaf-no-reserve"),
            FramePointer::All => Some("all"),
        }
    }

    /// The `frame-pointer` module flag value, `None` when none is
    /// emitted.
    #[must_use]
    pub fn module_flag(self) -> Option<i32> {
        match self {
            FramePointer::None => None,
            FramePointer::NonLeaf => Some(1),
            FramePointer::NonLeafNoReserve => Some(4),
            FramePointer::All => Some(2),
        }
    }
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

impl CodegenOptions {
    /// The IR will be optimized (`-O1` and above): definitions are
    /// emitted without the `-O0` `noinline optnone` pair (clang
    /// `CodeGenOpts.OptimizationLevel > 0`; spec/codegen.md §7p).
    #[must_use]
    pub fn optimize(&self) -> bool {
        self.opt_level > 0
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
            opt_level: 0,
            opt_size: 0,
            strict_aliasing: true,
            clang_compat: 22,
            llvm_major: 19,
            omit_frame_pointer: None,
            omit_leaf_frame_pointer: None,
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

#[cfg(test)]
mod tests {
    use super::FramePointer::{self, All, NonLeaf, None as Omit};

    /// clang 19 and 22's `"frame-pointer"` for each target, `-O0`/`-O1+`
    /// and flag combination (probed with `--target=` each Linux triple;
    /// spec/codegen.md "Frame pointer").
    #[test]
    fn frame_pointer_matches_clang() {
        let r = FramePointer::resolve;
        // (x86-64, optimize, -f[no-]omit, -m[no-]omit-leaf) -> kind.
        let rows = [
            (true, false, None, None, All),
            (true, true, None, None, Omit),
            (true, true, Some(false), None, All),
            (true, false, Some(true), None, Omit),
            (true, false, None, Some(true), NonLeaf),
            (true, true, None, Some(true), Omit),
            (true, true, None, Some(false), Omit),
            (true, true, Some(false), Some(true), NonLeaf),
            (true, false, Some(true), Some(false), Omit),
            (false, false, None, None, NonLeaf),
            (false, true, None, None, NonLeaf),
            (false, true, Some(false), None, NonLeaf),
            (false, true, Some(true), None, Omit),
            (false, false, None, Some(false), All),
            (false, true, Some(true), Some(false), Omit),
        ];
        for (x86, opt, omit, leaf, want) in rows {
            assert_eq!(r(x86, opt, omit, leaf), want, "{x86} {opt} {omit:?} {leaf:?}");
        }
        assert_eq!((All.attr(), All.module_flag()), (Some("all"), Some(2)));
        assert_eq!((NonLeaf.attr(), NonLeaf.module_flag()), (Some("non-leaf"), Some(1)));
        assert_eq!((Omit.attr(), Omit.module_flag()), (None, None));
    }
}
