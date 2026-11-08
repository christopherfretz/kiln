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
  `di_omit`, `debuginfo/types.rs:600`).
- `-g0`, the default, builds no debug metadata and records no locations
  (see "Cost at `-g0`").
- Since WP-910 `-g` is total: at any level it compiles every program kiln
  compiles without it. A construct kiln cannot describe degrades the
  way clang's does (spec/landed/codegen/WP-910.md); the differences
  from clang that remain are listed under "Gaps".

## Evidence

- `tests/codegen/debug_*.cpp`: 32 cases, byte-exact against clang 19 on
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
  | `reflection.cpp` (WP-910) | a `define_aggregate` class, a `Holder<^^int>` specialization, a `constexpr std::meta::info` | pass | skip (no P2996) |

  Since WP-816 `tests/debuginfo` is a default root of `cargo xtask test`
  and of both `ci` corpus runs, and GitHub CI runs it on aarch64 and
  x86_64: the corpus jobs install gdb and lldb-19 and set
  `KILN_REQUIRE_DEBUGGERS=1`, so a missing debugger is an ERROR there.
  On a machine without them the tests SKIP with the reason
  (`requires `gdb` on PATH`).

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
(`debuginfo/mod.rs:433`, one `Option` check on the function's debug
state), and the module's `Cg::di` (`module.rs:263`) stays `None`. `cargo xtask bench` on the
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

The table above is WP-814's measurement. WP-817 below re-measured the
`-g` rows after cutting the front end's share.

### WP-817: the front-end cost of `-g`

WP-814 found that `-g` cost kiln's front end about +47% (`-S
-emit-llvm`, `-O0`: format-test 3.30 to 4.86 s, templates_wide 0.42 to
0.75 s), against clang 19's whole `-g` increment of under 0.3 s on
format-test. WP-817 split that increment and cut the front-end part.
The IR is byte-identical to before for every TU measured (format-test
and templates_wide, with and without `-g`) and for every
`tests/codegen/debug_*` and `tests/debuginfo` test. The design is in
spec/landed/codegen/WP-817.md.

**Commands.** The TUs are the ones listed above: fmt 12.1.0
`test/format-test.cc` with the flags above, and
`bench/frontend/templates_wide.cpp` with `-std=c++26`, plus the new
`bench/frontend/format-like-g.cpp` with `-std=c++26` (the host's
libstdc++ 14, as the bench compiles it). Each measured command is

- front end only: `kiln++ -O0 [-g] -Xclang -disable-llvm-passes -S
  -emit-llvm -o /dev/null <TU flags>` (no `opt-19` run);
- whole `-S -emit-llvm`: `kiln++ -O0 [-g] -S -emit-llvm -o /dev/null
  <TU flags>`; for format-test this runs `opt-19 -passes=default<O0>`
  and `opt-19 -discard-value-names -S`, because `<format>`-style code
  defines `always_inline` functions (spec/driver.md);
- object: `kiln++ -O0 [-g] -c -o tu.o <TU flags>`;
- the same command lines with `clang++-19`.

kiln is a release build of the branch (base: main 5d62a654e), run from a
tmpfs copy. Times are medians of 7 interleaved before/after runs (front
end) or 3 (whole and object), with user CPU time next to wall time
because the shared 24-core aarch64 host is noisy. RSS is the largest
peak of the runs (`/usr/bin/time -f "%e %M %U"`).

**Where the `-g` increment went (before).** Targeted timers in a
throw-away instrumented build, one run each:

- format-test front end, +0.35 to +0.41 s user: codegen +0.28 s, of it
  0.11 s in metadata interning (197,582 `md_intern` calls without `-g`
  14,753); printing and dropping the module +0.20 s.
- templates_wide front end, +0.30 s user: codegen +0.20 s (interning
  0.075 s); printing +0.13 s, most of it dropping the module's 120,656
  metadata nodes, each with a boxed name and a boxed key per field.
- LLVM, not kiln: `llc-19 -O0 -filetype=obj` on the printed IR goes from
  1.36 to 2.28 s on format-test and from 0.02 to 0.36 s on
  templates_wide; `llvm-as-19` alone (parsing the text) goes from 0.44
  to 1.13 s and from 0.02 to 0.39 s. On format-test's `-S -emit-llvm`
  path the two `opt-19` runs add about 1.1 s with `-g`. That is the
  text round trip (perf-reference-workload.md F4), not front-end work.

**What changed.** Static node names and field keys (`&'static str`), so
a node allocates only its field list; hash-once interning into a
`hashbrown::HashTable<(u64, MetadataId)>` that never clones a node;
allocation-free printing of node names, keys and integers; and the
driver's `opt` decision scans only the code part of the IR, not the
metadata.

**After.** The same instrumented build: templates_wide codegen 0.079
to 0.242 s with `-g` (interning 0.05 s for 140,563 calls), printing
0.030 s in all, dropping 0.003 s; format-test codegen 0.357 to 0.583 s
(interning 0.040 s, `DILocation`s 0.044 s for 166 K calls, `DIFile`s
0.020 s for 32 K calls), printing 0.040 to 0.111 s.

| TU | command | before, no `-g` | before, `-g` | after, no `-g` | after, `-g` | clang 19, no `-g` | clang 19, `-g` |
|---|---|---|---|---|---|---|---|
| format-test | front end | 2.61 s (2.27 user), 142 MiB | 3.08 s (2.66), 250 MiB | 2.67 s (2.28), 142 MiB | 2.80 s (2.45), 181 MiB | 2.91 s (2.74), 322 MiB | 3.34 s (3.14), 355 MiB |
| format-test | `-S -emit-llvm` | 3.62 s (3.09), 143 MiB | 5.44 s (4.81), 247 MiB | 3.59 s (3.09), 143 MiB | 5.21 s (4.56), 206 MiB | 3.11 s (2.90), 322 MiB | 3.76 s (3.45), 355 MiB |
| format-test | `-c` | 4.17 s (3.67), 163 MiB | 6.22 s (5.42), 247 MiB | 4.21 s (3.74), 163 MiB | 5.71 s (5.18), 230 MiB | 3.58 s (3.40), 322 MiB | 4.45 s (4.23), 355 MiB |
| templates_wide | front end = `-S -emit-llvm` | 0.39 s (0.36), 89 MiB | 0.73 s (0.67), 182 MiB | 0.37 s (0.35), 90 MiB | 0.53 s (0.49), 117 MiB | 0.64 s (0.58), 231 MiB | 0.82 s (0.76), 253 MiB |
| templates_wide | `-c` | 0.40 s (0.35), 90 MiB | 1.13 s (0.98), 180 MiB | 0.41 s (0.37), 90 MiB | 0.88 s (0.80), 117 MiB | 0.65 s (0.57), 231 MiB | 0.80 s (0.72), 254 MiB |
| format-like bench | front end | 0.74 s user, 51 MiB | 0.77 s user, 78 MiB | 0.74 s user, 51 MiB | 0.80 s user, 62 MiB | | |

Against the WP-817 target (the front-end `-g` increment at most 15% of
the no-`-g` time):

- **format-test: met.** +7.5% user (2.28 to 2.45 s; before +17%), +5%
  wall. Peak RSS with `-g` 250 to 181 MiB (clang 355 MiB).
- **templates_wide: not met, inherent.** +40% user (0.35 to 0.49 s;
  before +86%). The `-g` module is 14 MB of IR against 1.1 MB, 13x: one
  global of the outermost type makes codegen describe all 20,100
  specializations, about six nodes and two mangled or printed names
  each. The remaining 0.14 s is building and printing those 120,656
  nodes, 1.2 microseconds a node; 15% would be 0.05 s, less than
  printing the 14 MB alone. clang 19's own `-g` increment on this TU is
  larger (0.58 to 0.76 s user, +0.18 s), and LLVM needs 0.37 s more to
  parse the same metadata. Peak RSS with `-g` 182 to 117 MiB (clang
  253 MiB).
- **format-like bench** (`bench/frontend/format-like-g.cpp`): front end
  +5 to +8% user, within the noise of 5 runs; peak RSS with `-g` 78 to
  62 MiB.

Without `-g` nothing changed: the IR is byte-identical and the times
and RSS are within noise. What remains of kiln's whole `-g` cost over
clang's is on the LLVM side: `-c` on format-test is +1.44 s user for
kiln and +0.83 s for clang, and templates_wide `-c` +0.43 s against
+0.15 s, because LLVM parses the metadata as text (once for `-c`,
twice through `opt` for format-test's `-S -emit-llvm`).

The bench entries `bench/frontend/templates-wide-g.cpp` and
`bench/frontend/format-like-g.cpp` (`mode: emit-llvm`, `args: -g`)
track the front-end cost from now on.

## Gaps

Every gap is a documented difference from clang 19 (D). Since WP-910
none is a hard error (E): the three E items WP-814 listed (template
value arguments with no IR constant, template template arguments that
are not a plain name, SVE types) now degrade as in clang
(spec/landed/codegen/WP-910.md). The list matches
spec/landed/codegen/WP-814.md "Deviations" and WP-910.md.

- D: `std::meta::info` has no clang counterpart. It is a pointer-sized
  `DW_ATE_unsigned` base type named `std::meta::info`, so gdb prints a
  value as a number, and a template value argument of the type has
  `value: null` (no IR constant). lldb names `Holder<^^int>` as
  `Holder<unsigned long>`.
- D: a floating-point template argument in a class name prints as
  `T<1.5e0>` (clang: `T<1.500000e+00>`), at
  `crates/kiln_codegen/src/debuginfo/types.rs:549`.
- D: no `defaulted: true` on template parameters (`types.rs:1164`).
- D: no `DIImportedEntity` for `using` declarations or directives
  (`debuginfo/mod.rs:417`).
- D: no `DISubprogram` for `__cxx_global_var_init`, `__tls_init` or
  atexit stubs.
- D: no `DILocalVariable` for structured bindings or variable-length
  arrays. A VLA type has `count: -1` (`types.rs:394`).
- D: no `DILabel`.
- D: statement end locations exist only for compound and loop statements
  (`debuginfo/stmt.rs:82`).
- D: alias template specializations are unwrapped (`types.rs:122`), so
  `C<int>` for an alias template `C` loses its `Alias<int>` typedef.
- D: `di_const_ref` approximates `DeclMustBeEmitted`
  (`debuginfo/func.rs:388`), and floating-point static members get no
  `extraData`.
- D: no call-site declaration subprograms above `-O0` (the call site
  DIE gap above).

## Follow-ups

- The LLVM side of the `-g` cost (WP-817): the metadata is parsed as
  text by `llc-19`, and twice by the two `opt-19` runs of an
  `always_inline` module's `-S -emit-llvm`
  (`kiln_driver/src/codegen_cmd.rs` `ir_passes`); a bitcode writer or a
  single `opt` run would cut it (perf-reference-workload.md F4).
- Call-site declaration subprograms above `-O0` (the call site DIE gap).
- The D items under "Gaps", in roughly this order: structured bindings,
  `DIImportedEntity`, global-initializer subprograms, alias templates,
  then the rest.
