# Known issues

This page lists what kiln does not do yet, or does differently, as of
1.0. Each item names the work package that tracks it in
[BACKLOG.md](BACKLOG.md), where one exists. How to use the parts that do
work is in the [user guide](USER_GUIDE.md). Measured results for each
library and project are in [COMPATIBILITY.md](COMPATIBILITY.md).

If you hit something not listed here, please
[report it](REPORTING-BUGS.md).

## Platforms

- **Linux on aarch64 and x86_64 only.** Any other `--target` (macOS,
  Windows, 32-bit, other architectures) is `unsupported target`. Support
  for other platforms is not planned for 1.0.
- **glibc 2.39 or newer** is needed to run the release binaries. Building
  them on an older-glibc image is [WP-943](BACKLOG.md).
- **x86 function multiversioning** (`target_clones`, several
  `target("...")` versions of one function) is an error on x86-64:
  [WP-1059](BACKLOG.md). The `-m<feature>` flags and single
  `target("...")` functions work. `-mgeneral-regs-only`, `-msoft-float`,
  `-mretpoline` and a few other x86 code-generation flags are still
  `unknown argument` (spec/driver.md "x86-64 target features").
- **x86_64 sweep results are from before the last fixes.** The first
  x86_64 sweep ([status/x86_64-sweep.md](status/x86_64-sweep.md)) found
  that LLVM's `IRTests` did not build because glibc's x86-64 `*f128`
  functions were not usable as `__float128` ones. WP-1058 fixed that
  (kiln still keeps `_Float128` and `__float128` distinct, as GCC 13+
  does), and WP-1000 added the `-m<feature>` flags Bitcoin Core and
  OpenCV need, but the sweep has not been re-run since. The 1.0 release
  candidate's sweep re-measures them
  ([status/release-1.0.md](status/release-1.0.md)).
- **LLVM 19 and 22 only.** LLVM 20 and 21 are not supported.
- **Identification.** `--version` prints `kiln version <v>` without the
  git commit ([WP-942](BACKLOG.md)). The `-v` output of a link step
  repeats the backing clang's "Debian clang version" line
  ([WP-976](BACKLOG.md)).

## C mode

kiln compiles C++ only. `kiln` can preprocess C (`-E` on `.c`, `.h`,
`.i`, or `-x c`), but compiling C is `not implemented yet`. `kiln++`
compiles a `.c` file as C++, with a warning, as `clang++` does. C support
is [WP-243](BACKLOG.md) and is not part of 1.0.

## Uninitialized variables at C++26

kiln implements C++26 erroneous behaviour (P2795) and clang 22 does not.
At `-std=c++26`, an automatic variable of at most 4096 bytes that has no
initializer is zero-filled at its declaration, so a program that reads
it sees zero where the clang 22 binary sees whatever was on the stack.
The fill costs a store or a `memset` per declaration. To get clang's
behaviour, mark the variable `[[indeterminate]]` or compile with
`-ftrivial-auto-var-init=uninitialized`
(`-ftrivial-auto-var-init-max-size=N` moves the size limit). A variable
whose declaration a `goto` or `switch` jumps over is not written, as in
clang. This is by design, not a gap
([USER_GUIDE.md](USER_GUIDE.md#language-modes)).

## Build systems

- **Meson** requires the word `clang` in `--version` output and rejects
  kiln. Use the wrapper in the
  [user guide](USER_GUIDE.md#using-kiln-from-a-build-system). kiln does
  not ship one. CMake and autotools work unchanged
  ([COMPATIBILITY.md](COMPATIBILITY.md#build-systems)).
- **`cargo install`**: kiln is not on crates.io, and
  `cargo install --path crates/kiln_driver` installs the `kiln` binary
  without the `kiln++` symlink or the resource directory. Use the release
  archive or `cargo xtask dist`
  ([STABILITY.md](STABILITY.md#cratesio-not-published)).
- **LTO with an older linker**: with `-flto`, kiln's objects are LLVM
  bitcode of the selected LLVM (22 when it is installed), so the link
  needs the same LLVM's driver, linker plugin or `lld`. A target linked
  by an older driver, such as a C library linked by clang-19, fails with
  `Unknown attribute kind ... (Producer: 'LLVM22...' Reader: 'LLVM 19...')`.
  clang has the same rule. In a mixed C/C++ build, use a C compiler of
  the same LLVM ([USER_GUIDE.md](USER_GUIDE.md#targets-and-llvm-versions)).

## Modules

Module files (`--precompile`, `-fmodule-output`) work. They are kiln's
own format; see the [user guide](USER_GUIDE.md#modules). CMake's C++20
module support works through `kiln-scan-deps` (`FILE_SET CXX_MODULES`,
3.28+ with Ninja), and so does `import std;` (`CXX_MODULE_STD`). What is
missing (spec/modules.md §11.10):

- **`-fmodule-header`** and header-unit module files
  ([WP-983](BACKLOG.md)). Header units work from their headers in a
  direct compile, but the scanner rejects `import "h";` and
  `import <h>;`, so a CMake build cannot use them.
- **Module files hold the module's source, not a serialised AST**, so an
  importer re-parses the module's text. Since WP-985 the file also
  carries the module's constant-evaluation results, so the importer does
  not evaluate those again (spec/modules.md §11.11). The declaration
  image that replaces the re-parse (`-fkiln-bmi-image`) ships in 1.0 as
  an experimental opt-in: it is off by default and does not cover `std`
  yet. Making it the default is [WP-1070](BACKLOG.md). Importing
  libc++'s `std` module file costs about what compiling `std.cppm`
  in-process costs ([COMPATIBILITY.md](COMPATIBILITY.md#c20-modules)).
- In libc++ 22's suite, one test fails because `std.compat` imported over
  `std` does not keep the two global module fragments apart
  ([status/libcxx-m5.md](status/libcxx-m5.md)). WP-460 fixed it, but the
  full suite has not been re-run since.

## Sanitizers and coverage

- **Source-based coverage mapping** (`-fcoverage-mapping`) writes
  clang 19's mapping in every `-fclang-compat` mode, so against clang 20+
  `llvm-cov` shows fewer regions around `&&`/`||` and constant
  conditions. A function template specialization used only in constant
  evaluation is missing from the report instead of showing as 0%; a few
  generic-lambda and macro `#x` regions differ. Line and function
  counts match clang 19's on the comparison programs. Details and rows:
  [spec/landed/codegen/WP-1003.md](../spec/landed/codegen/WP-1003.md).
- **gcov filters** `-fprofile-filter-files` and `-fprofile-exclude-files`
  are rejected: [WP-1004](BACKLOG.md).
- **Profile-guided optimization is not part of 1.0.**
  `-fprofile-generate`, `-fcs-profile-generate`, `-fprofile-use`,
  `-fprofile-instr-use` and `-fprofile-sample-use` are rejected with an
  error that says so.
- **UBSan.** `-fsanitize=vptr` and `-fsanitize=function`
  ([WP-934](BACKLOG.md)), the nullability checks ([WP-935](BACKLOG.md))
  and `local-bounds` ([WP-936](BACKLOG.md)) are accepted but emit no
  check. Other small gaps: [WP-937](BACKLOG.md). The umbrella is
  [WP-852](BACKLOG.md).
- Smaller differences from clang's sanitizer IR (value names, ASan module
  name, constructor details) are [WP-858](BACKLOG.md),
  [WP-859](BACKLOG.md) and [WP-861](BACKLOG.md).
  They do not change what the sanitizers report.

## Flags and attributes

kiln rejects a clang flag it does not implement instead of ignoring it.
These flags are the exceptions: they are accepted and have no effect.

| Flag | What happens | WP |
|---|---|---|
| `-fno-fat-lto-objects` | accepted; with `-flto` objects are bitcode only anyway | — |
| `-f[no-]vectorize`, `-f[no-]slp-vectorize`, `-fstrict-vtable-pointers` | accepted; LLVM's defaults apply | [WP-900](BACKLOG.md) (deferred) |
| `-fdiagnostics-show-template-tree` | accepted; diagnostics are printed without the tree | [WP-901](BACKLOG.md) (deferred) |
| `-F <dir>` | warning; framework directories do not exist on Linux | — |
| `-f[no-]stack-clash-protection`, `-f[no-]common`, `-fstack-usage`, `-m64`, `-pipe` and a few more | accepted silently; listed in spec/driver.md, "Codegen flags" | — |

`-ffat-lto-objects` is rejected: fat objects are [WP-899](BACKLOG.md).

`kiln++ --help` lists the flags a user is likely to need, grouped as in
clang's help; spec/driver.md, "Options", is the complete list.

**Attributes.** Every attribute clang knows is in one of three classes
(`crates/kiln_sema/src/attr_triage.rs`, audit in
[status/silent-drops.md](status/silent-drops.md)):

- Class A attributes would change semantics, the ABI or layout. kiln does
  not implement a few of them, and those are a hard error,
  `'X' attribute is not supported by kiln yet`. That error is not
  silenced by `-w` or in system headers, and `__has_attribute` answers 0
  for them, so portable code takes its fallback path.
- Class B attributes steer only optimization, instrumentation or debug
  information. Class C attributes feed only diagnostics or analyses kiln
  does not run. Both are accepted. Most of class B is implemented
  (WP-813). Where it is not, the cost is a missed optimization or
  diagnostic, never wrong code.

## Diagnostics and accepts-invalid

kiln is measured on clang 22.1.8's own negative tests
([status/accepts-invalid.md](status/accepts-invalid.md), run on the 1.0
release candidate `e6199e693`):

- In 4.6% of test files (57 of 1250) where clang expects an error, kiln
  reports none in some configuration.
- 12.1% of `expected-error` lines (3384 of 28065) have no kiln error on
  them.
- 6.2% of `expected-no-diagnostics` configurations (26 of 418) get an
  error from kiln (rejects-valid).
- 0 crashes, timeouts or out-of-memory results in 2850 cases (2767 ran;
  the driver skipped 83 for clang flags it does not accept).

The first triage run (`321baa9de`) found mostly declaration rules: types
defined where the grammar forbids them, members named after their class,
union and static member rules, redeclarations, and attribute checks. The
clusters are [WP-945](BACKLOG.md) to [WP-960](BACKLOG.md), and most of
them are fixed in 1.0; the largest remaining cluster is
[WP-960](BACKLOG.md) (`-pedantic-errors` extension diagnostics). Valid
code is not affected.
Code that clang rejects may compile with kiln, so build with clang or GCC
in CI too if you rely on rejection.

Diagnostic wording follows clang's but is not always identical, and some
warnings clang has are missing. Wording is not covered by
[STABILITY.md](STABILITY.md).

## libc++ 22 test suite

kiln passes 99.4% of libc++ 22.1.8's suite (9247 tests, `-std=c++26`)
on the 1.0 release candidate ([status/libcxx-1.0.md](status/libcxx-1.0.md)).
The 9 correctness failures of the M5 run
([status/libcxx-m5.md](status/libcxx-m5.md); WP-460, WP-491) all pass.
What is left:

- **4 filesystem permission tests** that cannot pass when the suite runs
  as root (an environment limit, not a kiln failure).
- **Expected failures for missing or reworded warnings**, none of them
  wrong code:
  - `-Wnonnull` ×5 ([WP-433](BACKLOG.md))
  - `-Wthread-safety` ×2
  - deprecation warnings on `rel_ops` and the C++17 adaptor typedefs ×2
  - `alloc_align` power-of-two check ×1
  - constraint-note wording ×1

  The `std::hash<std::monostate>` wording test that was on this list now
  passes.
- **1 long test** that times out under heavy host load and passes at its
  full timeout.

## Open-source projects with gaps

Of the 47 projects in [COMPATIBILITY.md](COMPATIBILITY.md#libraries-and-applications),
46 pass everything clang passes in the 1.0 release run
([status/release-1.0.md](status/release-1.0.md) "(d)"). Among them,
folly (3772 of 3773, as clang), folly::coro (616 of 618, as clang),
simdjson (117 of 117; its one earlier failure needed `-Wuninitialized`,
WP-696), mp-units (25 of 25) and oneTBB (139 of 139, with a clang-22 C
driver for its LTO link; see [Build systems](#build-systems)) were
re-run at parity for the release. The other:

- **Boost metaprogramming libraries**: 2235 of 2253 TUs (clang 19:
  2240). Of the 18 TUs that fail, 13 also fail under clang 19; the other
  5 are rejected by clang 22 too, so kiln follows clang 22 there.

## Open expected-failure tests

Each gap below has a test in kiln's corpus marked `xfail(kiln)`: the test
passes under clang 19 (or GCC 16.1 for reflection) and the harness
expects it to fail under kiln until the fix lands. The list is every such
test on `main`, from `git grep -l 'xfail(kiln)' tests/`; the WP is the
one the test names (the work package it was found in, or the open row
that tracks it). Paths are under `tests/conformance/` unless shown.

Templates, lookup and overloading:

- `ast/class.implicit/eager.cpp`: injected-class-names are not declared
  as members yet (spec/sema-class.md §2.2). WP-171.
- `ast/namespace.udecl/instantiated-shadow-order.cpp`: the same cause.
  WP-171.
- `over.match.best/perfect-match-reversed-member.cpp`: a member
  `operator==` beside its own reversed form is a unique perfect match in
  clang 22.1. [WP-635](BACKLOG.md) (deferred).
- `temp.expl.spec/in-class-member-template-spec-of-class-template-run.cpp`:
  instantiating an in-class explicit specialization of a member class
  template redeclares the member as a different kind of symbol.
  WP-1178.
- `temp.deduct.call/explicit-args-trailing-return-sfinae-before-nondep-conversion.cpp`:
  a trailing return type is substituted only after the CWG1391
  conversion check. WP-804.
- `temp.dep.type/out-of-line-current-instantiation-var-template-unqualified-run.cpp`:
  a variable template-id keeps its written qualifier. WP-1169.
- `temp.spec.partial.order/typed-nttp-more-specialized-than-auto-run.cpp`:
  partial ordering of class partial specializations does not check a
  deduced non-type argument's type against its parameter's
  (`os<int E>` against `os<auto E>` is called ambiguous).
  [WP-1236](BACKLOG.md).

Constant evaluation and reflection:

- `class.union/constexpr-placement-same-type.cpp`: Oven pointers carry no
  union-member designator, so either member of one type at one offset
  may be the constructed one. [WP-517](BACKLOG.md).
- `ext.clang.pass.object.size/pass-object-size-lenient-xfail.cpp`: no
  lenient designator for object-size folds. WP-808.
- `meta.reflection.define.aggregate/define-run-template-for.cpp`: an
  expansion statement over a defined class's members. WP-474, WP-478,
  WP-479.

Attributes, calling conventions and extensions:

- `dcl.attr.cconv/ir-swiftcall.cpp`: `swiftcall`/`swiftasynccall` are a
  hard error. WP-808.
- `dcl.attr.cconv/override-cc-clang22.cpp`: a written calling convention
  prints after the type (clang 19's form). WP-808.
- `dcl.attr.cconv/x86-interrupt.cpp`: the x86 `interrupt` attribute is a
  hard error. WP-808.
- `dcl.attr.class-b/ir-likely-branch-weights-O1-19.cpp`: no branch
  weights for `[[likely]]`, `[[unlikely]]` or `__builtin_expect` when
  optimizing. WP-813.
- `dcl.attr.class-b/ir-noescape-function-pointer-19.cpp`: `noescape` on a
  function type's parameter is not kept in the type. WP-813.
- `dcl.attr.class-b/malloc-span-22.cpp`: `malloc_span` has no subject or
  argument checks. WP-813.
- `dcl.attr.grammar/gnu-attribute-before-statement.cpp`: GNU attributes
  before a statement parse only when a declaration or `;` follows.
  WP-805.
- `ext.arm.sve/target-attribute-sve-locals.cpp`: the SVE type check
  ignores a function's `target("sve")`. WP-910.
- `ext.clang.address_space/address-space-run.cpp`: `address_space(N)` is
  a hard error. WP-809.
- `ext.multiversion/target-clones-x86.cpp`: x86-64 function
  multiversioning. [WP-1059](BACKLOG.md).

Code generation, ABI and sanitizers:

- `basic.link/unnamed-enum-internal-name-run.cpp`: an internal unnamed
  enumeration mangles as `Ut_` instead of clang's `$_N`. WP-941.
- `cfenv.syn/fenv-access-fetestexcept-run.cpp`: `#pragma STDC
  FENV_ACCESS ON` is rejected (no constrained intrinsics). WP-806.
- `driver.sanitize/ubsan-desc-identity-ir.cpp`: UBSan type descriptors
  are keyed by name, not by the sugared type. WP-871.
- `driver.sanitize/ubsan-lambda-returns-nonnull-ir.cpp`: a lambda's
  trailing `returns_nonnull` lands on `operator()`. WP-871.
- `driver.sanitize/ubsan-noreturn-asan-ir.cpp`: value names and clang
  22's `alloca_name_altered` annotation under ASan. WP-871.

Diagnostics and dumps:

- `dcl.fct.def.delete/use-uncalled-argument-no-match.cpp`: a deleted
  function used as a call argument also draws "no matching function".
  WP-1170.
- `dcl.fct.def.delete/use-uncalled-comma-operand.cpp`: a deleted
  function as a comma operand also draws two `-Wunused-value` warnings.
  WP-1170.
- `dcl.array/vla-warning-suppressed-by-wvla-pragma.cpp`: ignoring
  `-Wvla` by pragma does not silence `-Wvla-cxx-extension`. WP-789.
- `tests/stress/dump-friend-template-cycle.cpp`: `-fdump-ast` prints a
  befriended class template's pattern where clang prints it
  instantiated. [WP-129](BACKLOG.md).

## Crashes

kiln reports an internal compiler error instead of crashing silently, and
writes a reproducer ([REPORTING-BUGS.md](REPORTING-BUGS.md)). The known
internal-error sites were turned into diagnostics (WP-971 to WP-973).
Testing the reproducer of a preprocessed `.i` input waits on C support:
[WP-974](BACKLOG.md).
