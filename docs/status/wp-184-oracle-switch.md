# WP-184: oracle switch clang/LLVM 19 → 22 — measurement and plan

Status: **done** (steps A–B in WP-184/185/263, C–E in WP-355,
2026-11-02). The oracle is clang/LLVM 22; clang 19 stays installed as the
second oracle and the frozen M3 gate. "Outcome" below is the result and
the residual list; the rest of this page is the original measurement
(2026-11-02, before the switch). Measured on main `b161fb06` in the development
container (Debian 13, aarch64; `clang++-22` = Debian clang 22.1.8
`1~deb13u4`, same apt source as clang 19, so the CI image can install it
with one more package line).

## What the corpus does under clang 22 today

2824 tests (`cargo xtask test tests`, `-std=c++26`):

| Run | pass | fail | error | xfail | skip |
|---|---|---|---|---|---|
| clang++-19 self-check (today's gate) | all | 0 | 0 | 4 | 36 |
| clang++-22 as the compiler under test | 2457 | 271 | 56 | 4 | 36 |
| kiln under test, clang++-22 as oracle | 2662 | 109 | 42 | 10 | 1 |

Logs: `.build/wp184-selfcheck22.log`, `.build/wp184-oracle22.log`. Every
failure falls into one of seven buckets.

### 1. AST-dump schema drift (47 errors + ~65 fails; xtask + goldens)
clang 22's `-ast-dump=json` carries attributes the normalizer
(`xtask/src/clang_ast/table.rs`) does not know: `TemplateInstantiationPattern`
on `ClassTemplateSpecializationDecl` (41 `templates/` tests error out),
`hasExplicitParameters` on `LambdaExpr` (6 `expr.prim.lambda`). Those are
table additions. The fails are type-sugar changes in clang's printer that
the `.expected` goldens (written from clang 19) do not have: `sizeof`/`new`
bounds print `'__size_t':'unsigned long'`; alias templates keep their sugar
(`'Ptr<T> *'` where 19 printed `'type-parameter-0-0 **'`); `ParmVarDecl`
loses `destroyed`; `ArrayInitIndexExpr` and `_CmpUnspecifiedParam`
conversions print differently. kiln's own dump matches 19 here, so the
goldens are regenerated under 22 and kiln's printer follows where the
change is sugar retention (desirable: it is what users see in diagnostics).

### 2. Preprocessor builtin and predefined-macro drift (83 `differential/pp`; kiln_pp)
Not a preprocessor bug: 78 of the 83 are libc++ 19 headers taking a
different `#if` branch because clang 22 answers
`__has_builtin(__is_referenceable)` = 0 (removed) and adds
`__builtin_is_virtual_base_of`, `__builtin_is_cpp_trivially_relocatable`
and friends; `-dM` differs in 37 lines (`__cpp_*` values from WP-185's
"Macro gaps", `__clang_*` version macros, new `__FP*`/`__GCC_*` entries).
kiln's `__has_builtin` and predef tables mirror clang 19. The switch means
kiln claims clang 22's builtin set, which obliges kiln_sema to implement
the type traits it newly claims (small: ~6 traits) and to drop the removed
ones. 1 torture case (`lex.name-ucn-paste`) is a real lexer difference
(clang 22 pastes UCN-spelled identifiers differently); investigate.

### 3. Diagnostic wording changes (~60 self-check fails; test directives)
Clang 22 renamed or re-ordered diagnostics the `//@ diag:` directives pin:
`'__size_t' (aka 'unsigned long')` in conversion errors (expr.new, ast),
consteval-escalation notes (P2564 follow-ups, `dcl.constexpr/consteval-*`),
`relational-unrelated`/`string-literals` constant-evaluation notes, the
structured-binding access errors (`dcl.struct.bind/access.cpp`: clang 22
no longer rejects the private/protected cases, CWG 2386-adjacent).
Policy: each becomes the clang-22 wording; kiln follows the wording. Where
22's behaviour changed rather than its wording, the test documents which
standard the new behaviour comes from.

### 4. C++26 features clang 22 now accepts (16 xfail/skip(clang) + ~20 fails)
Tests written against clang 19's rejection now compile under 22:
constexpr placement new (`expr.new/`), structured-binding packs
(`dcl.struct.bind/`, P1061), frozen string literals (clang 22 accepts
`expr.const/5.24/frozen-string-literals.cpp`, so the test's expectation was
a 19-ism), consteval escalation fixes, `requires:`-less contract and P2996
probes. These flip to real tests (and kiln's xfail(kiln) rows become the
M4 todo list, which matches WP-185's table).

### 5. Run-test output (3 fails; kiln follows 22)
`__PRETTY_FUNCTION__` of a lambda prints `(lambda)` in clang 22 where 19
and kiln print `(anonymous class)` (2 tests; kiln changes).
`ext.c11.atomic/class-run.cpp` prints a different evaluation order:
investigate before assuming clang is right (C11 atomic class member order).

### 6. IR-diff corpus (`crates/kiln_codegen/tests/ir_vs_clang.rs`, 230 cases × 2 triples)
Proxy measurement (`.build/wp184-ir/results2.txt`): clang 19 vs clang 22
`-S -emit-llvm -O0 -std=c++26` on the 230 cases × 2 triples, both
normalized by `opt-22 -passes=verify` as `ir_vs_clang` would. 401 of 460
pairs differ, 55 are identical, 4 fail to build. Almost every diff is
boilerplate, not structure:
- the x86_64 and aarch64 `datalayout` strings gained address-space entries
  (`p270/p271/p272`) — every x86 case;
- new parameter attributes `dead_on_return` on indirect by-value arguments,
  `nuw` on struct `getelementptr`s, `"frame-pointer"="non-leaf-no-reserve"`
  (LLVM 22 only: **opt-19 rejects that attribute**, so the normalizer must
  run on opt-22 for both sides; clang 19 IR auto-upgrades fine);
- `!llvm.linker.options` dropped, module-flag numbering shifts.
Median diff is 8 lines; the outliers are `int128_ops` (641 lines, x86)
and `complex_ops` (~330 lines, both triples), where clang 22 lowers
differently (i128 by-value passing, complex arithmetic helpers) and the
IR-diff policy in spec/codegen.md §6 decides case by case whether kiln
follows. This confirms the BACKLOG row's plan: a structural comparison
mode that strips attribute sets, alignment metadata, datalayout and gep
flags keeps ~95% of the corpus exact across oracles; the ABI-sensitive
slice stays strict on the pinned oracle.

### 7. libc++ 22 is not installed (gate + headers)
`clang++-22` in Debian 13 defaults to libstdc++ 14; `libc++-22-dev` exists
in the same apt source but is not in the container or the CI action.
Today kiln hardcodes `/usr/lib/llvm-19` for headers, `llc`, `opt` and the
link driver (`kiln_driver/src/toolchain.rs:253`, override `KILN_LLVM_DIR`).
The libc++ 22 test checkout (`scripts/fetch-libcxx-tests.sh`) is a tag
argument away. The M3 gate stays frozen on 19 (ROADMAP), so both libc++
versions must coexist: 19 under `.toolchain/` as now, 22 beside it.

## Plan (one WP per step, parallel where marked)

| Step | Scope | Size | Parallel |
|---|---|---|---|
| A | xtask: `KILN_ORACLE=19\|22` (default 19 until E lands) selecting `clang++-N`/`opt-N`/`llc-N`/`FileCheck-N`; `--oracle` flag already exists; `requires: clang>=22` honoured by the harness; kiln_driver `KILN_LLVM_DIR` follows; clang_ast table additions (bucket 1 errors) | S | — |
| B | kiln_pp: `__has_builtin`/predef tables to clang 22 + the ~6 new type traits in kiln_sema; drop `__is_referenceable` (bucket 2) | M | with A |
| C | AST goldens regenerated under 22, kiln printer sugar retention (bucket 1 fails) | M | after A |
| D | diag wording + feature flips (buckets 3, 4, 5) | M | after A, with C |
| E | IR-diff re-baseline on 22 per spec/codegen.md §6 policy: structural mode (attributes/align/gep flags stripped) + strict ABI slice (bucket 6); flip default `KILN_ORACLE` to 22; CI action installs clang-22/llvm-22/libc++-22-dev alongside 19; `libcxx --gate m4` list on libc++ 22 | M | after B, C, D |

Both oracles stay installed; the M3 gate (`libcxx --gate m3`) keeps
running on 19 so its numbers remain comparable. Deviations and wording
changes are recorded per test (`requires: clang>=22`, `xfail(clang)` with
the 19 reason) so the corpus runs green under either oracle during the
transition.

## Decisions wanted from the operator
1. Go on the plan and the two-oracle CI image (adds ~1–2 min to the Debian
   apt step; both `check` jobs).
2. Bucket 2 policy: kiln mirrors clang 22's `__has_builtin` set exactly
   (recommended; libc++/libstdc++ headers key on it) versus kiln advertising
   only what it implements and accepting pp diffs in `differential/pp`.
3. Bucket 1 policy: regenerate goldens from clang 22 and make kiln print
   alias sugar like 22 (recommended) versus normalizing both sides to 19's
   desugared form.

## Outcome (WP-355)
- `KILN_ORACLE` defaults to 22 (`xtask/src/oracle.rs`); the harness passes
  `-fclang-compat=<oracle major>` to kiln, and a test whose `requires:
  clang>=N` / `clang<N` the oracle does not meet runs kiln alone at the
  nearest supported level (spec/harness.md "Two-oracle policy").
- CI (`.github/actions/kiln-ci`) installs clang-22/llvm-22/lld-22 beside
  19 and runs `cargo xtask ci` on 22 plus `KILN_ORACLE=19 cargo xtask ci
  --quick`. libc++ 22 is an extracted tree under
  `.toolchain/libcxx-22-root` (`libc++-22-dev` conflicts with
  `libc++-19-dev`; spec/libcxx.md "Toolchain"); `libcxx --gate m4` runs on
  it, baseline in docs/status/libcxx.md.
- Goldens (step C) are clang 22's; kiln's dumper follows at level 22
  (spec/ast-dump.md §2). IR diff (step E): structural mode on 22, strict
  ABI slice (spec/codegen.md §6).
- kiln follows clang 22 at `-fclang-compat=22` (spec/sema.md §1.1):
  `(lambda)` closure names, `__size_t`/`__ptrdiff_t` sugar, alias sugar,
  stmt-expr trailing null statement, structured-binding index literal and
  captured-binding `const`, `-Wdeprecated-literal-operator` (CWG2521),
  `__is_trivially_relocatable` deprecation, codegen atomic text.
- Codegen follows the backend LLVM where the two reject each other's IR:
  coroutines use LLVM 22's `void` `llvm.coro.end` +
  `llvm.coro.is_in_ramp` when `KILN_LLVM_DIR` is an `llvm-22` tree
  (`Toolchain::llvm_major`; spec/codegen.md "LLVM 22 `coro.end`").
  Padded `_Atomic` operands are zeroed and a failed compare-exchange
  writes `*expected` back at every level (clang 22's behaviour; an
  oracle-19 `ir_vs_clang` deviation).
- Tests that skipped clang 19 now `requires: clang>=22` (with
  `xfail(kiln)` where kiln lacks the feature); docs/status/cxx-features.md (now docs/STANDARD_SUPPORT.md).

### Residuals: burn-down (WP-377)
WP-355 pinned 152 tests `//@ requires: clang<22` with `// clang 22 emits
<text>; WP-355 residual`, and marked 5 AST goldens `xfail(kiln)`. Each
pinned test still runs kiln at level 19 under either oracle. WP-377 made
kiln follow clang 22 at `-fclang-compat=22` (`_19` diag rows,
spec/diag.md §2.5; compat-keyed rules, spec/sema.md §1.1; the level in
the Oven's options, spec/oven.md). Each fixed test was either unpinned,
when its directives use substrings that both clangs share, or split
into a clang 19 copy and a `-clang22.cpp` copy (`requires: clang>=22`).

To list what is left: `git grep -l "WP-355 residual" tests`.

| Bucket | Before | After | What remains, and why |
|---|---|---|---|
| Constant-evaluation and `static_assert` operand notes | ~32 | 19 | These are not wording changes. clang 22's evaluator reaches different verdicts or notes: virtual `typeid`/`dynamic_cast`, `std::allocator`, vbase ctors, enum range, frozen string literals, stmt-expr, vector element access, recovery expressions, `__int128` in-class init, a compound literal at namespace scope, consteval address, quiet contexts, extra notes on `__is_trivially_copyable`. Each needs Oven work. |
| Scoped names in `aka`/args, out-of-line mismatch, "refers into" | ~22 | 13 | This needs clang 22's type printer. A tag type keeps its written qualifier and is not sugar, and a canonical tag prints fully scoped, inside template arguments too (`'O::J<SA>' (aka 'O::J<ns::S>')`, `'const J<ns::S> &'`). `kiln_ast/src/types/print.rs` (`aka`, `template_id`, `in_tst_args`) has no compat input yet. These include item 6's two tests (`explicit-arg-sugar-nondependent-param`, `deduced-binding-sugar`): they fail at 22 on `aka` scope, not on substitution. |
| GNU/vendor extensions | ~11 | 9 | ext_vector component columns/wording (4), `__builtin_assume_aligned` on a non-pointer (2), `va_start`, atomic memory order, `%s` with `void *`. |
| Attribute names scoped, `nodiscard` | ~9 | 2 | `leading-attribute-placement-2`: clang 22 recovers "misplaced attributes" differently. `namespace-deprecated`: clang 22 rejects code kiln accepts. |
| CTAD guide notes | 8 | 1 | `dcl.type.class.deduct-compound-diag`: "cannot form pointer to deduced class template specialization type" is new clang 22 checking. |
| Structured binding wording | ~10 | 0 | |
| `-Wswitch`, array and self-comparison | 7 | 1 | `hidden-friend-of-member-class-unsatisfied`: clang 22's self-comparison warning in an instantiated hidden friend. |
| Local-class friends, digit separators, typeid | 11 | 0 | |
| Template template notes | 3 | 3 | Not attempted: clang 22 notes the mismatching parameter of a template template argument differently (new note texts and locations). |
| Single cases | ~39 | 21 | Parser recovery after a missing `;` or `)` (3), NSDMI-needed (2), lazy default arguments (2), DR1467, designated update, `across-namespaces`, noexcept reachability, unused-value reachability, explicit specialization after instantiation, access in substitution, substitution order before constraints, invalid-body substitution, missing `typename`, a macro-argument note, `__reference_constructs_from_temporary`, `meta.unary.comp`, and `mangle/constraints` (WP-384 owns mangling). |
| **Pinned total** | 152 | 69 | |
| AST goldens `xfail(kiln)` | 5 | 5 | Not started (item 7): inheriting-ctor shadows, a `::B` base cast as written, redeclaration parameter names, an alias specialization's qualifier, CTAD specialization order. |

**Not done** (at WP-377; the flip is done in WP-394, below): kiln++'s own `-fclang-compat` default was still 19 (item 8),
because the residual count is well above the exit target. Only the
harness passes 22. Codegen at level 22 (`dead_on_return`, `nuw` gep,
datalayout) is covered only structurally (spec/codegen.md §6; WP-386).

### Residuals: part 2 (WP-394)
WP-394 followed clang 22 in the remaining buckets and flipped kiln++'s
default to `-fclang-compat=22` (spec/driver.md; the library-level
`ClangCompat` default, used by `Sema`, `DiagOptions` and unit tests,
stays 19). `xtask libcxx` now always passes the level. 58 more tests
were split into a clang 19 copy and a `-clang22.cpp` copy, and a few
were unpinned outright (`designated-update-diags`, `meta.unary.comp`).

The rules added are listed in spec/sema.md §1.1 (type printing, note
locations, checks clang 22 adds or drops, template template arguments,
member access, builtins), spec/diag.md §3 (level-keyed group nesting:
`c99-designator` ⊃ `initializer-overrides`, `reorder-init-list`),
spec/oven.md (enum range fault; no non-literal local check in C++23) and
spec/ast-dump.md (the shadow filter).

| Bucket | WP-377 | WP-394 | What remains, and why |
|---|---|---|---|
| Constant-evaluation and `static_assert` operand notes | 19 | 4 + 2 | WP-413 fixed the four: `consteval-address` (P2564 escalation of a specialization), `non-literal-argument` (no by-value argument literal check), `namespace-scope-diagnostics` (element locations, dependent literals checked at definition) and `static-assert-concept-atom-expr` (type-trait explainer notes, `is_trivially_copyable` only). Pinned `// WP-388` (vbase construction): `class.inhctor.init/constexpr-vbase`, `expr.const/non-literal-local` — WP-388 has landed and both are only the clang 19 halves of splits whose clang 22 behaviour is tested (`constexpr-vbase-22.cpp`, `non-literal-local-22.cpp`), so nothing is left to burn down. |
| Scoped names in `aka`/args | 13 | 0 | |
| GNU/vendor extensions | 9 | 0 | |
| Attribute names, `nodiscard` | 2 | 0 | |
| CTAD guide notes | 1 | 0 | |
| `-Wswitch`, array and self-comparison | 1 | 0 | |
| Template template notes | 3 | 0 | |
| Single cases | 21 | 2 + 3 | WP-413 fixed `dr1467-single-element` (CWG2137 at clang 20+) and `access-in-substitution` (no access checks while deducing explicit-specialization candidates). Pinned `// WP-399` (typo correction, clang 22 has no delayed typo correction): the three `stmt.expr/delayed-*` tests. |
| **Pinned total** | 69 | 13 (8 `WP-355 residual`, 2 `WP-388`, 3 `WP-399`); 5 after WP-413 (no `WP-355 residual` left) | |
| AST goldens `xfail(kiln)` | 5 | 2; 0 after WP-413 | Fixed: inheriting-ctor shadows (harness filter), `::B` cast type as written, redeclaration parameters; WP-413: `temp.alias-dependent-template-template-arg-ast` (the first-written `QualifiedTemplateName` is kept) and `temp.deduct.guide-ast-dump` (CTAD resolves non-template guides first). |

The exit target (15 or fewer) is met, so the default is flipped. The
stratum A sample at the flip is in docs/status/libcxx.md.
