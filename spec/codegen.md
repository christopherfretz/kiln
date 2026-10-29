# spec: kiln_codegen — typed AST to LLVM IR (M2)

Status: **in progress** (WP-052 part 1 done; driver and corpora are WP-052b). This spec is normative for `crates/kiln_codegen`.
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
`expr.rs` mirrors clang's `ScalarExprEmitter`, `EmitLValue`,
`AggExprEmitter`, `EmitBranchOnBoolExpr` and `EmitAutoVarDecl`.
- Conversions follow `EmitScalarConversion`:
  - to bool: `icmp ne 0`, `fcmp une 0.0` or `icmp ne null`;
  - integer to integer: cast by source signedness;
  - integer to float: `sitofp` or `uitofp`; float to integer: `fptosi` or
    `fptoui`, chosen by destination signedness;
  - float to float: `fptrunc` or `fpext`, chosen by rank;
  - pointer to and from integer: through `i64`;
  - casts of constants fold as LLVM's constant folder does, including
    integer to `fp128` (round to nearest even).
- Arithmetic:
  - signed add, sub and mul get `nsw`; shifts mask nothing at -O0;
  - pointer arithmetic: `gep inbounds` (`gep i8` for void and function
    pointers); pointer difference: `ptrtoint`, `sub`, then `sdiv exact`;
  - floating add/sub (also compound) contract a single-use `fmul`
    operand (looking through a single-use `fneg`) into
    `llvm.fmuladd.*`, as `tryEmitFMulAdd` does under `-ffp-contract=on`.
- Values:
  - an lvalue-to-rvalue conversion of a glvalue `?:` is emitted as a
    scalar `?:`: each arm loads in its own block and the result is a phi;
  - `&&`/`||` produce an `i1` phi; `EmitBranchOnBoolExpr` splits
    `&&`/`||`/`!`/`?:` conditions into branches and folds constants;
  - bit-fields load the storage unit, then `shl`/`ashr` (signed) or
    `lshr`/`and` (unsigned); stores mask, clear and `or` into the unit;
  - `++`/`--` on bool, integers, floats and pointers, including bit-fields.
- Calls:
  - a call site gets `nounwind` only when the callee's prototype is nothrow;
  - a variadic call prints its function type;
  - an indirect (sret) return writes into the destination slot when the
    call initializes an aggregate, otherwise into a temporary.
- Names (via `kiln_abi::mangle`): namespace-scope variables with internal
  linkage are mangled (`_ZL1h`); static locals use 0-based
  discriminators per (function, name): none, then `_0`, `__1_`, ...
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
- Constants (`consts.rs`): a union constant is its active member followed
  by `[N x i8] undef` padding, typed as the union when the element types
  match its IR type, else as a literal struct. Records with bit-fields and
  base classes are still M3 errors in constants.

## 4. Statements
- `stmt.rs` covers compound, declaration, expression, null, if (with
  init statement; constexpr and consteval if), switch (with case chains,
  default, fall-through), while, do, for, break, continue, return,
  goto and labels, attributed statements and `static_assert` (no code).
- A function's return goes through the `retval` slot and a single
  `return` block, as clang does; an sret return stores into the sret
  pointer.
- Block layout, forwarding-block removal and the dead-block cleanup
  follow clang -O0. A statement with no insertion point is skipped unless
  it contains a label (`ContainsLabel`).
- Each loop gets a distinct `!llvm.loop` node with
  `llvm.loop.mustprogress`. A loop with a constant-true or absent
  condition and an empty body clears `mustprogress` on the function and
  gets no loop metadata.
- Case ranges (GNU) are an error.

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
- **Unit corpus** (`crates/kiln_codegen/tests/lower.rs`, WP-052): 73
  inline C++ programs, lowered for `aarch64-unknown-linux-gnu` straight
  from `parse_tu` (no driver). Every case must pass
  `opt-19 -S -passes=verify`. `Exact` cases must also equal
  `clang++-19 -S -emit-llvm -O0 -std=c++26 --target=aarch64-unknown-linux-gnu`
  after normalization. A case that differs but is marked `VerifyOnly` is
  a documented deviation (below); one that becomes exact is reported so
  it can be promoted. `KILN_CASE=<name>` prints both texts.
- **Normalization:** both texts go through `opt-19 -S -passes=verify`,
  which renumbers values and attribute groups canonically. Then the
  `; ModuleID` line, `source_filename`, `!llvm.ident` and its
  `clang version ...`/`kiln` string node are dropped. Nothing else is
  normalized: attributes, metadata, alignment, names and declaration
  order must match.
- **IR-diff corpus** (WP-052b): `crates/kiln_codegen/tests/ir_vs_clang.rs`
  compiles each `tests/codegen/*.cpp` through the driver with kiln and
  with clang on both triples, under the same normalization.
- The `run` conformance tests (`basic.start.main`, `stmt.ranged`, printf
  tests) run through kiln.

### Deviations
- `bitfields_compound` (VerifyOnly; kiln_sema). The computation type of a
  compound assignment does not promote a bit-field LHS narrower than
  `int` (`kiln_sema` `conv/arith.rs`, `usual_arithmetic_conversions_kind`
  skips LHS promotion on the CompoundAssign path). So for
  `unsigned c : 9`, `c += 2` computes in `unsigned` (`add`) where clang
  computes in `int` (`add nsw`). The value is the same; only the flag
  differs. The fix belongs in kiln_sema.
- Parser gaps (kiln_parse), which keep cases out of the corpus rather
  than deviating: `noexcept` exception specifications and
  `if constexpr`/`if consteval` are not parsed yet. `stmt.rs` already
  lowers constexpr and consteval if, and call sites already honour a
  nothrow prototype; add cases when the parser accepts them.

## Status and what remains
Written as a resume point for a fresh agent. Branch `wp/052`, worktree
`kiln/.worktrees/wp-052`.

**Done (WP-052 part 1):** `fir.rs`, `module.rs`, `func.rs`, `consts.rs`,
`fold.rs`, `expr.rs` (§3) and `stmt.rs` (§4). The unit corpus is 72/73
exact against clang on aarch64; the one deviation is listed above.
Additive `kiln_abi` fixes came with it: internal-linkage variable mangling,
static-local discriminators, and indirect returns with `byval` (clang's
`getNaturalAlignIndirect`, so no `result.ptr` spill).

**Remaining for WP-052b, in order:**
1. Driver: `crates/kiln_driver/src/codegen_cmd.rs` and `link.rs` (§5).
   Edits to `lib.rs` and `args.rs` must be additive only. The pipeline
   mirrors `syntax_cmd.rs`: `Job::build_preprocessor`, then
   `parse_preprocessed`, then `kiln_codegen::emit`, then
   `constexpr.write_report`.
2. IR-diff corpus: about 40 `tests/codegen/*.cpp` programs plus
   `ir_vs_clang.rs`, on **both** triples. The unit corpus only checks
   aarch64. On x86_64 the ABI paths (`kiln_abi::call` x86 classification,
   align-16 local arrays of 16 bytes or more) are unverified against
   clang.
3. `run` tests: about 30 printf tests, plus `//@ xfail(kiln)` markers for
   the header-dependent tests in basic.start.main (argv, environment,
   return-value, stdin) and for stmt.ranged/constexpr-sum.
   `CXXForRangeStmt` is never produced by sema yet.
4. Out of M2 but noted: bit-field and base-class constants, case ranges,
   and the kiln_sema compound-assign promotion fix (then promote
   `bitfields_compound` to `Exact`).
