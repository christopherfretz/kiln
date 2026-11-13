# Floating-point environment status

## `#pragma STDC FENV_ACCESS ON` (analysis, WP-1399)

Today the pragma is an error (`'#pragma STDC FENV_ACCESS ON' is not
supported yet`, `crates/kiln_parse/src/fp_pragma.rs`, WP-806), as are
`float_control(except, on)`, `clang fp exceptions(maytrap|strict)` and a
`FENV_ROUND` other than `FE_TONEAREST`. Nothing is silently dropped.

What clang does under `FENV_ACCESS ON` (C23 7.6.1, `Sema::ActOnPragmaFEnvAccess`,
`CodeGenFunction` with `CGFPOptionsRAII`):

- Sema: the pragma is only allowed at file scope or at the start of a
  compound statement (kiln already checks this placement for every FP
  pragma). It sets `AllowFEnvAccess` in the scope's `FPOptionsOverride`,
  which turns `-ffp-exception-behavior` to `strict` and the rounding mode
  to `dynamic`. A constant initializer of a static object is still
  folded at translation time, but in a function the operations are not
  folded (`Expr::EvaluateAsRValue` refuses `FPOptions::isFPConstrained`).
- Codegen: every FP operation in the scope becomes a constrained
  intrinsic (`llvm.experimental.constrained.fadd(..., metadata
  !"round.dynamic", metadata !"fpexcept.strict")`, and likewise `fsub`,
  `fmul`, `fdiv`, `frem`, `fma`/`fmuladd`, `fcmp`/`fcmps`, `fptosi`,
  `fptoui`, `sitofp`, `uitofp`, `fpext`, `fptrunc`, and the math
  intrinsics `sqrt`, `pow`, `sin`, ...). Every call in the function gets
  `strictfp`, and the function gets the `strictfp` attribute.

What kiln needs:

1. AST/sema: one `FpOptions` bit (`fenv_access`) carried in
   `FpOverride` like `contract` (`crates/kiln_ast/src/fp.rs`); the parser
   arm in `fp_pragma.rs` stops being an error. The constant evaluator
   (Oven) must refuse to fold FP arithmetic in a constrained scope
   outside a constant-expression context, and `constexpr` evaluation keeps
   the default environment ([expr.const] does not see the dynamic
   environment). About 60 lines.
2. Codegen: one lowering switch in `crates/kiln_codegen/src/fp.rs` (it
   already threads the scope's `FpOptions` to each instruction for
   fast-math flags). The sites are the 31 `fadd`/`fsub`/`fmul`/`fdiv`/
   `frem`/`fcmp`/`fneg` builder calls (`expr.rs` 10, `builtins.rs` 7,
   `complex.rs` 6, `x86_gen.rs` 3, `ubsan.rs` 2, `fir.rs`, `func.rs`,
   `x86.rs` 1 each) and the FP casts among the 160 `binop`/`cast` calls;
   `crates/kiln_llvm` needs a constrained-intrinsic helper and the
   `strictfp` function and call attributes. About 300 lines.
3. Tests: IR tests against clang for each operation class, the run test
   of `fesetround` + `fetestexcept` at -O2, C and C++. About 15 tests.

Estimate: about 400 lines of code plus tests, one work package of
medium size. The risk is in completeness (an FP operation emitted
outside the switch stays unconstrained and LLVM may move it across
`fesetround`), so the switch must sit in the builder wrapper rather than
at each call site. `FENV_ROUND` with a static mode and
`float_control(except, on)` come with it almost for free (other metadata
values on the same intrinsics).
