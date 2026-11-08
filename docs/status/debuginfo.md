# Debug information (`-g`) status

WP-814 added DWARF debug information. The contract is in
spec/codegen.md §5.1, spec/driver.md "Debug information flags" and
spec/landed/codegen/WP-814.md. This page records the state measured when
it landed (2026-11-08, aarch64 Linux host, LLVM 19 back end).

## What `-g` produces

- `-g` / `-g2` and the other levels produce the metadata clang 19 produces
  for the same program. A `-S -emit-llvm -g` output equals clang 19's after
  `opt-19`, apart from the `producer:` string. `llc-19` turns it into
  DWARF 5 with `-debugger-tune=gdb`.
- `-gline-tables-only`, `-gline-directives-only`, `-gsplit-dwarf`, `-gz`,
  the path-remapping flags and `-fstandalone-debug` behave as in clang. A
  debug flag kiln does not implement is a hard error naming it.
- `nodebug`, `artificial` and `[[clang::standalone_debug]]` shape the
  output as in clang (`standalone_debug` keeps a class's full definition,
  `di_omit`, `debuginfo/types.rs:569`).
- `-g0`, the default, builds no debug metadata and records no locations
  (see "Cost at `-g0`").
- A construct kiln cannot describe yet is a hard error (see "Gaps"). Debug
  information is never silently dropped.

## Evidence

- `tests/codegen/debug_*.cpp`: 30 cases, byte-exact against clang 19 on
  aarch64 and x86_64 (`crates/kiln_codegen/tests/ir_vs_clang.rs`).
- `llvm-dwarfdump-19 --verify` is clean on all 60 `-g` objects from
  `tests/codegen` and `tests/debuginfo`.
- `llvm-symbolizer-19` gives the same function, file, line and inlining
  stack for kiln and clang 19 objects of the debugger tests.
- Debugger tests (`tests/debuginfo`, spec/harness.md "Debugger tests"):
  each one compiles at `-O0 -g`, then runs the same script under gdb and
  lldb-19.

  | test | covers | kiln | clang++-19 |
  |---|---|---|---|
  | `locals.cpp` | file:line breakpoint, `bt`, a parameter, a local, a struct member, a scoped enumerator, a reference, a class template member | pass | pass |
  | `step_finish.cpp` | `step` into a method, `finish` with its return value, `next` over a call | pass | pass |
  | `globals_lambda.cpp` | a namespace-scope variable, a static data member, a lambda's by-copy and by-reference captures, `up` | pass | pass |

  The debugger tests are not among `cargo xtask test`'s default roots,
  because CI has no gdb. Name the directory to run them:
  `cargo xtask test tests/debuginfo`.

## Statistics against clang 19 (OSS builds)

These are fmt 12.1.0 and nlohmann/json 3.12.0, built through
`scripts/oss/oss-build.sh <proj> --compiler kiln|clang --no-test` with
`CXXFLAGS=-g`, so each build uses its own `-O3` (Release). The numbers are
`llvm-dwarfdump-19 --statistics`, summed over every object in the build.

| statistic | fmt kiln | fmt clang | json kiln | json clang |
|---|---|---|---|---|
| objects | 50 | 50 | 93 | 93 |
| source variables | 1004520 | 1043136 | 3134981 | 3072445 |
| ... with a location | 688649 | 715100 | 2092817 | 1981007 |
| scope bytes covered | 71.06% | 69.71% | 72.37% | 69.33% |
| inlined functions | 453800 | 469803 | 1443928 | 1447806 |
| call site DIEs | 51114 | 74509 | 193274 | 257098 |
| DIEs | 1834602 | 2085094 | 6208684 | 6711181 |
| `.debug_info` bytes | 17733478 | 19696632 | 57914371 | 61817781 |

- Variables with a location are 96.3% (fmt) and 105.6% (json) of clang's.
  Scope coverage is 1.4 and 3.0 points higher. The optimizer (`opt-19
  -O3`) decides most of this, and the two front ends hand it different
  IR. Kiln is within 6% of clang on every variable count, so it does not
  lose variables that clang keeps.
- Call site DIEs are 69% and 75% of clang's. Above `-O0`, kiln does not
  emit clang's declaration subprograms for calls to external functions
  (`EmitFuncDeclForCallSite`). This is a follow-up (see "Gaps").
- There are fewer DIEs (88% and 93%) mostly for the same reason. Each
  missing declaration subprogram carries its parameter DIEs.

## Cost at `-g0`

Every debug-information path is behind `Cg::dbg_on()`
(`debuginfo/mod.rs:403`, one `Option` check on the function's debug
state), and the module's `Cg::di` (`module.rs:258`) stays `None`. `cargo xtask bench` on the
whole corpus (46 entries, 5 runs each) compared this branch with main
(fb89f25f5). Both were built with `--profile harness`. Main was recorded
into a scratch baseline (`--record --baseline <dir>`), then this branch
was compared against it (`--baseline <dir>`):

- Constexpr `steps`, `bytes` and instantiation counts: +0.0% on every
  entry.
- Wall time: the median entry's median is 3.4% faster (range -24.6% to
  +7.9%) on this shared host. No entry is past the 5% time threshold,
  which compares the fastest run with the baseline median.
- Peak RSS: two entries under 10 MB trip the 2% memory threshold,
  `oven/fib` (+0.3 MB) and `oven/churn-vector` (+0.3 MB). A rerun with
  `--runs 10` clears `churn-vector` (+0.5%). `fib` measured directly
  (8 interleaved runs of `-fsyntax-only bench/oven/fib.cpp`) is 9764 KB
  against 9848 KB, +84 KB. An empty TU shows the same +50 to +85 KB. This
  is the larger binary's startup pages (about 0.8 MB more text), not work
  done per declaration or per instruction. Neither entry runs code
  generation.

## Cost of `-g` against clang 19

The TUs are listed below. kiln is the `--profile harness` build of this
branch; the comparison is `clang++-19`. Every cell is the fastest of 3
interleaved runs and the largest peak RSS (`/usr/bin/time -f "%e %M"`),
on a shared 24-core aarch64 host. `-c -o tu.o` is appended to each
command. The `-g0` rows compare this branch with main (fb89f25f5); the
`-g` rows compare kiln with clang 19.

- **fmt 12.1.0** `test/format-test.cc`:
  `-DGTEST_HAS_STD_WSTRING=1 -I$SRC/include -isystem $SRC/test/gtest/.
  -std=gnu++20 -fvisibility=hidden -fvisibility-inlines-hidden
  $SRC/test/format-test.cc`
- **nlohmann/json 3.12.0** `tests/src/unit-msgpack.cpp`:
  `-DDOCTEST_CONFIG_SUPER_FAST_ASSERTS -DJSON_TEST_KEEP_MACROS
  -DJSON_TEST_USING_MULTIPLE_HEADERS=1 -I$SRC/tests/thirdparty/doctest
  -I$SRC/tests/thirdparty/fifo_map -I$SRC/include -I<build>/include
  -std=gnu++11 -Wno-deprecated -Wno-float-equal
  $SRC/tests/src/unit-msgpack.cpp`. `<build>/include` holds the
  CMake-generated `test_data.hpp`.
- **bench** `bench/frontend/templates_wide.cpp`: `-std=c++26`.

| TU | flags | main | kiln | clang 19 | kiln / other |
|---|---|---|---|---|---|
| format-test | `-O0` | 3.62 s, 162 MiB | 3.60 s, 162 MiB | | 0.99x time, 1.00x RSS |
| format-test | `-O0 -g` | | 4.96 s, 247 MiB | 3.52 s, 354 MiB | **1.41x** time, 0.70x RSS |
| format-test | `-O3` | 8.63 s, 258 MiB | 8.66 s, 258 MiB | | 1.00x, 1.00x |
| format-test | `-O3 -g` | | 12.77 s, 421 MiB | 11.67 s, 555 MiB | 1.09x, 0.76x |
| unit-msgpack | `-O0` | 3.09 s, 140 MiB | 3.18 s, 140 MiB | | 1.03x, 1.00x |
| unit-msgpack | `-O0 -g` | | 4.66 s, 227 MiB | 3.12 s, 315 MiB | **1.49x**, 0.72x |
| unit-msgpack | `-O3` | 8.47 s, 233 MiB | 8.54 s, 233 MiB | | 1.01x, 1.00x |
| unit-msgpack | `-O3 -g` | | 12.99 s, 392 MiB | 11.92 s, 602 MiB | 1.09x, 0.65x |
| templates_wide | `-O0` | 0.45 s, 89 MiB | 0.44 s, 88 MiB | | 0.98x, 0.99x |
| templates_wide | `-O0 -g` | | 1.00 s, 180 MiB | 0.70 s, 252 MiB | **1.43x**, 0.72x |
| templates_wide | `-O3 -g` | | 1.19 s, 180 MiB | 0.71 s, 253 MiB | **1.68x**, 0.71x |

The `-g0` objects are byte-identical between main and this branch.

**Perf finding (not fixed here): `-g` costs kiln's front end about
+47%.** At `-O0`, `kiln++ -S -emit-llvm` on format-test takes 3.30 s
without `-g` and 4.86 s with it. The IR grows from 16 MB to 31 MB. For
templates_wide, the times are 0.42 s and 0.75 s, and the IR grows from
1.1 MB to 14 MB. clang 19's whole `-g` increment on format-test is
under 0.3 s (`-O0`: 3.23 to 3.48 s without `-g`, 3.52 s with it). kiln's
peak RSS stays 25 to 35% below clang's. The time has not been profiled
(`perf_event_open` is blocked on the dev host). Two candidates:

- Each DI node allocates its class name and every field key as a
  `Box<str>`, and is hashed for interning (`kiln_llvm` `md_specialized`,
  `module.rs:1459`; `debuginfo::Fields`, `debuginfo/mod.rs:124`). Keys
  could be `&'static str`.
- The metadata is printed as IR text and parsed again by `llc-19`.

## Gaps

Each gap is either a hard error (E) or a documented difference from
clang 19 (D). The list matches spec/landed/codegen/WP-814.md
"Deviations".

- E: a non-integral template value argument that does not lower to an IR
  constant (`crates/kiln_codegen/src/debuginfo/types.rs:1175`).
- E: a template template argument other than a plain template name
  (`types.rs:1207`).
- E: SVE and other target builtin types (`types.rs:340`).
- D: a floating-point template argument in a class name prints as
  `T<1.5e0>` (clang: `T<1.500000e+00>`), at `types.rs:535`.
- D: no `defaulted: true` on template parameters (`types.rs:1128`).
- D: no `DIImportedEntity` for `using` declarations or directives
  (`debuginfo/mod.rs:382`).
- D: no `DISubprogram` for `__cxx_global_var_init`, `__tls_init` or
  atexit stubs.
- D: no `DILocalVariable` for structured bindings or variable-length
  arrays. A VLA type has `count: -1` (`types.rs:363`).
- D: no `DILabel`.
- D: statement end locations exist only for compound and loop statements
  (`debuginfo/stmt.rs:82`).
- D: alias template specializations are unwrapped (`types.rs:134`).
- D: `di_const_ref` approximates `DeclMustBeEmitted`
  (`debuginfo/func.rs:383`), and floating-point static members get no
  `extraData`.
- D: no call-site declaration subprograms above `-O0` (the call site
  DIE gap above).
- `tests/debuginfo` is not run in CI (no gdb in the CI image).

## Follow-ups

- The `-g` front-end cost above (`kiln_llvm/src/module.rs:1459`,
  `kiln_codegen/src/debuginfo/mod.rs:124`).
- Call-site declaration subprograms above `-O0` (the call site DIE gap).
- A CI image with gdb, so `tests/debuginfo` joins the default roots
  (`xtask/src/test/runner.rs` `debugger_skip`).
- The D items under "Gaps", in roughly this order: structured bindings,
  `DIImportedEntity`, global-initializer subprograms, alias templates,
  then the rest.
