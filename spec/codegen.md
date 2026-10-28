# spec: kiln_codegen — typed AST to LLVM IR (M2)

Status: **in progress** (WP-052). This spec is normative for `crates/kiln_codegen`.
Reference: for every construct, the reference output is clang 19's
`-S -emit-llvm -O0 -std=c++26 --target=<triple>`. Targets are
`x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`.
Companions: `spec/llvm.md` (the IR model), `spec/abi.md` (signature
classification), `spec/oven.md` (constant evaluation).

## 1. Scope
The M2 subset covers the following:
- fundamental types, arithmetic and conversions;
- pointers, arrays and enums;
- structs without member functions;
- functions and all of C's control flow;
- constexpr functions and variables, and `static_assert`;
- string literals and `extern "C"`.

A construct outside M2 is a `CodegenError` ("not supported in M2"), never a
panic and never silently wrong IR.

## 2. Architecture
- `lib.rs`: `emit(ast, oven, target, opts) -> Result<Module, CodegenError>`.
- `fold.rs`: a pre-pass that asks the Oven to fold initializers and
  constant-reference loads (the `Folds` table). A const var is foldable
  only when it is non-volatile and either static storage, integral or enum
  (this mirrors clang's `isUsableInConstantExpressions` + `tryEmitAsConstant`).
- `consts.rs`: turns Oven byte images into `kiln_llvm` constants. It has a
  memory form (bool is `i8`) and a scalar form (bool is `i1`), plus string
  literal globals.
- `fir.rs`: `Fir`, a mutable per-function instruction buffer. It allows
  erasing, RAUW, block reordering and forwarding-block simplification,
  which clang does at -O0 but the append-only `kiln_llvm` builder cannot.
  `Fir::lower` writes the result into the module in one pass.
- `module.rs`: `Cg`, the module-level state. It owns type lowering,
  signatures (`kiln_abi::call`), mangling, globals, linkage and function
  attribute sets.
- `func.rs`: per-function state (`Fx`) and the prolog and epilog: the
  return slot, the main `retval` store of 0, parameter spills, the
  fall-off-the-end trap/unreachable, mustprogress, and helpers for
  memcpy/memset/GEP/casts.
- `expr.rs`: scalar, lvalue and aggregate expression emission (§3).
- `stmt.rs`: statement emission (§4).

## 3. Expressions (clang CGExprScalar/CGExprAgg/CGDecl rules mirrored)
- Conversions follow `EmitScalarConversion`:
  - to bool: `icmp ne 0`, `fcmp une 0.0` or `icmp ne null`;
  - integer to integer: cast by source signedness;
  - integer to float: `sitofp` or `uitofp`; float to integer: `fptosi` or
    `fptoui`, chosen by destination signedness;
  - float to float: `fptrunc` or `fpext`, chosen by rank;
  - pointer to and from integer: through `i64`.
- Arithmetic:
  - signed add, sub and mul get `nsw`;
  - pointer arithmetic: `gep inbounds` (`gep i8` for void and function
    pointers); pointer difference: `ptrtoint`, `sub`, then `sdiv exact`.
- Calls:
  - a call site gets `nounwind` only when the callee's prototype is nothrow;
  - a variadic call prints its function type.
- Local initialization (`EmitAutoVarInit`):
  - a constant aggregate goes through `emitStoresForConstant`. That means
    a single store, memset-0 plus a few stores, a bytewise memset, or a
    memcpy from a `@__const.<fn>.<var>` private global, in that order of
    preference. Constants are never promoted to globals at -O0;
  - a non-constant aggregate goes through `EmitAggExpr`, with
    `CheckAggExprForMemSetUse` (size > 16 and nonzero*4 <= size gives a
    memset);
  - `EmitArrayInit`: element 0 is stored at the base, element i at
    `gep inbounds elem, base, i64 i`, and a phi filler loop covers
    elements without an initializer.

## 4. Statements
- All of C's control flow is covered: if (including constexpr/consteval
  if), switch, while, do, for, break, continue, return, goto and labels.
- Block layout, forwarding-block removal and `!llvm.loop` metadata follow
  clang -O0.
- A loop with a constant-true or absent condition and an empty body
  clears `mustprogress` on the function and gets no loop metadata.

## 5. Driver (`kiln_driver::codegen_cmd`, `link`)
- `-S -emit-llvm` writes a `.ll` file.
- `-S` runs `llc-19`.
- `-c` runs `llc-19 -filetype=obj`.
- Linking runs `clang++-19` and passes `-fuse-ld=`, `-l`, `-L`, `-Wl,`,
  `-static` and `-nostdlib` through.
- `-O<n>` is passed through, `-g` is ignored, and `-###` and `-v` print the
  subcommands.
- Every job calls `ConstexprOpts::write_report`.

## 6. Validation
- `crates/kiln_codegen/tests/ir_vs_clang.rs` compiles each
  `tests/codegen/*.cpp` with kiln and with the clang oracle on both
  triples. Both outputs are normalized with `opt-19 -S -passes=verify`,
  with `source_filename`, `ModuleID` and `ident` stripped, and the results
  must match exactly.
- The `run` conformance tests (`basic.start.main`, `stmt.ranged`, printf
  tests) run through kiln.

## Status and what remains
Written as a resume point for a fresh agent. Branch `wp/052`, worktree
`kiln/.worktrees/wp-052`.

**Done and committed (it builds, nothing calls it end to end yet):**
- `fir.rs`, the Fir buffer, complete;
- `module.rs`: types, signatures, attributes, globals, linkage,
  `private_const`, string globals;
- `func.rs`: function prolog and epilog, ret slot, params, main retval,
  trap, branch helpers, memcpy/memset/gep/cast helpers;
- `consts.rs`: constants from Oven byte images;
- `fold.rs`: the fold pre-pass. This commit restricts const-var folding to
  static, integral and enum vars, as clang does.

**Not written yet, in order:**
1. `expr.rs` (currently empty) holds the core of the lowering, still to
   write:
   - scalar emission and conversions;
   - `eval_bool` and branch-on-bool;
   - lvalues (DeclRef local/global/static-local/function/enum, Member,
     ArraySubscript, Deref, string literal, assignment and pre-inc
     lvalues);
   - calls, `emit_ignored`, all unary, binary and compound operators,
     `?:` and comma;
   - aggregate init (EmitArrayInit, struct and union InitList, the memset
     check, null init) and local var init (`emitStoresForConstant`).

   The rules are in §3, and clang probes confirmed them. Bitfields are an
   M2 error for now. On x86_64, local arrays of 16 bytes or more need
   align 16; aarch64 is not yet verified.
2. `stmt.rs` (currently a stub returning "todo"): every statement kind in
   §4. `CXXForRangeStmt` is never produced by sema yet, so it is out of
   scope.
3. Driver: `crates/kiln_driver/src/codegen_cmd.rs` and `link.rs` (§5).
   Edits to `lib.rs` and `args.rs` must be additive only. The pipeline
   mirrors `syntax_cmd.rs`: `Job::build_preprocessor`, then
   `parse_preprocessed`, then `kiln_codegen::emit`, then
   `constexpr.write_report`.
4. Tests:
   - `tests/codegen/*.cpp`, about 40 programs, plus `ir_vs_clang.rs`;
   - about 30 printf `run` tests;
   - `//@ xfail(kiln)` markers for the header-dependent tests in
     basic.start.main (argv, environment, return-value, stdin) and for
     stmt.ranged/constexpr-sum.
5. `cargo xtask ci` green.
