# WP-184: oracle switch clang/LLVM 19 → 22 — measurement and plan

Status: write-up for the operator's go/no-go (2026-11-02). Nothing in the
tree has moved yet. Measured on main `b161fb06` in the development
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
