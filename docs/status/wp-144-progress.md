# WP-144 progress: WP-140 follow-ups (libm builtins, source_location, asm extras)

Branch `wp/144-asm-followups`, worktree `kiln/.worktrees/wp-144`.

## Done
- **libm builtins.** Constant evaluation follows clang 19:
  - Evaluated in constant expressions: fabs, copysign, fmax/fmin, inf,
    isinf_sign, fpclassify, issubnormal/iszero/issignaling, expect, assume.
  - Not constant, as in clang: abs, sqrt, rounding, fma, other libm calls,
    trap, abort.
  - Code:
    - Oven: `kiln_oven/src/compile/builtins.rs`, `vm/builtins.rs`.
    - Sema: `kiln_sema/src/builtins.rs:427` (fpclassify).
  - `-O0` codegen is byte-identical to clang: llvm intrinsics, fpclassify
    CFG (`kiln_codegen/src/builtins.rs:470`), noreturn abort plus
    `unreachable`, libm calls with clang's attributes.
- **`std::source_location`.**
  - `current()` as a default argument (including on a function template's
    own parameter) is folded per use with that use's location
    (`Oven::fold_immediate_in`, `kiln_oven/src/lib.rs:396`).
  - A deduced return type's `function_name()` is refined once the type is
    deduced.
- **asm goto** (`kiln_codegen/src/asm.rs:291`):
  - Lowers to `callbr` with one `!i` per label (`Op::CallBr`, llvm
    `callbr_ex` at `kiln_llvm/src/builder.rs:999`).
  - With register outputs, each indirect edge gets its own block that
    stores them.
  - `%l[name]` numbering counts the "+" halves (`kiln_abi::named_operand`).
  - Parse/sema diagnostics follow clang: expected ':', expected
    identifier, undeclared label, duplicate names (`kiln_sema/src/asm.rs:42`).
- **`unwind` clobber.** Inside a try block the asm is an `invoke`.
- **`=@cc<cond>` flag outputs.**
  - Constraints follow clang's `matchAsmCCConstraint` for aarch64 and
    x86_64 (`kiln_abi/src/asm.rs:100`).
  - Each flag output gets `icmp ult 2` plus `llvm.assume` before its store
    (`kiln_codegen/src/asm.rs:120`, `asm_stores`).
- **Register-variable operands.**
  - clang `AddVariableConstraints`: `register T v asm("reg")` operands
    become `{reg}` (normalized; `&{reg}` when early-clobber), and the input
    half of a "+" operand repeats `{reg}` (`kiln_codegen/src/asm.rs:338`).
  - Sema now records the `register` storage class (`var_spec`).
- **MMX: N/A.** kiln's targets are aarch64 and x86_64. The x86 MMX
  `adjustInlineAsmType` path (`y` constraints mapping to `x86_mmx`) only
  matters for 64-bit vector operands with `y` constraints. LLVM 19 removed
  `x86_mmx` from IR inputs, and nothing in the corpus or libc++ uses it.
  Not implemented.

## Tests
- Codegen IR-diff tests:
  - `tests/codegen/builtins_math.cpp`
  - `tests/codegen/srcloc_immediate.cpp`
  - `tests/codegen/asm_goto.cpp`
  - `tests/codegen/asm_flag_output.cpp`
  - `tests/codegen/asm_register_var.cpp`
- Conformance tests:
  - `tests/conformance/expr.const/builtins-math{,-errors}.cpp`
  - `tests/conformance/expr.const/builtins-srcloc-deduced-run.cpp`
  - `tests/conformance/dcl.asm/asm-goto{,-errors}.cpp`
  - `tests/conformance/dcl.asm/flag-and-register-operands.cpp`
- `basic.start.main/environment.cpp` is no longer xfail.

## Divergences from clang and leftovers (minimal repros)
- A default member initializer's `__builtin_LINE()` uses the declaration
  site. `struct M { int v = __builtin_LINE(); };` used at line 4 gives
  kiln 1, clang 4.
- `[[noreturn]]` user functions get no `noreturn` attribute and no
  `unreachable` after the call. Repro: `[[noreturn]] void die(); void l(){die();}`.
- No `-Wassume`. `__builtin_isfpclass` and `__builtin_expect_with_probability`
  are missing.
- asm goto:
  - Labels and goto bits are not substituted in templates (pre-existing).
  - The AST dump omits the label children (clang shows `AddrLabelExpr`).
- `!srcloc` for an asm string from a macro expansion: clang encodes the
  macro SourceLocation (high bit set), kiln uses the spelling offset.
- Jump-scope checks are missing (pre-existing): no "cannot jump from this
  asm goto statement / goto into protected scope" diagnostics
  (`sema_goto_into_protected_scope` is unused).
- **Bug:** goto/label in a function with a destructible local drops the
  destructor call and EH. Repro:
  `struct D{~D();}; void g(); int h(bool b){ D d0; if(b) goto l1; g(); l1: return 3; }`
  emits no `~D` call.
