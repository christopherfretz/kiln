# Changelog

All notable changes to kiln are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and kiln adheres to [Semantic Versioning](https://semver.org/). What a
version number promises is in [docs/STABILITY.md](docs/STABILITY.md).

## [Unreleased]

Planned as 1.1.0.

### C mode

kiln compiles C. Its C behaviour is tested against clang (`-x c`),
diagnostic wording included. What is not done is in
[docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md#c-mode), and the
measurements are in [docs/status/c-mode.md](docs/status/c-mode.md).

- Drivers and dialects: `kiln` compiles `.c` (and `.h`, `.i`) as C, and
  `-x c` selects C under either driver. `kiln++` compiles a `.c` file as
  C++, with clang++'s deprecation warning. `-std=c89` (`c90`,
  `iso9899:1990`), `c99`, `c11`, `c17` (`c18`), `c23` (`c2x`), each
  with its `gnu` dialect; the default is `gnu17`, as in clang 19. C has
  its own lexing (no C++-only punctuators, user-defined literals or
  alternative tokens; no `//` comments in strict C89), C keywords
  (`_Imaginary` and `_Decimal32/64/128` are rejected as unsupported
  types, as in clang), each standard's identifier characters for UCNs,
  clang's C predefined macros and feature-test answers
  (`__has_c_attribute`; `__has_cpp_attribute` is C++ only), and C
  linkage (unmangled names, `fn.var` static locals),
  so C and C++ objects link together (WP-1275, WP-1290).
- C semantics, each where C differs from C++ (WP-1276, WP-1285 to
  WP-1287, WP-1301, WP-1311):
  - conversions: `void *` to and from any object pointer, integer and
    pointer conversions and incompatible pointer types diagnosed as
    clang does (`-Wint-conversion` is an error by default;
    `-Wincompatible-pointer-types` is an error by default, as in clang
    22, and a warning at `-fclang-compat=19`), pointer casts to narrower
    integers, `void *` and function-pointer arithmetic (GNU), C pointer
    comparisons and `?:` composite types, no C++ narrowing in brace
    initializers or case labels;
  - types: character literals are `int`, string literals are not
    `const`, `!`, comparisons and `&&`/`||` yield `int`, an enumeration
    with no negative enumerator promotes to `unsigned int`, `_Generic`
    matches by C compatibility;
  - declarations: the separate tag name space, tentative definitions
    (`-fcommon` and the `common` attribute), C99 and GNU89 `inline`
    (`-fgnu89-inline`), functions without a prototype, K&R
    definitions, implicit `int` and implicit function declarations
    (warnings in C89, errors from C99, as in clang), implicit
    declarations of the C library functions clang predeclares, no
    overloading (`__attribute__((overloadable))` keeps it), and
    prototype-scope tags and enumerators;
  - arrays and objects: variable length arrays, `[static N]`,
    `[restrict]` and `[*]` in array parameters, compound literals as
    lvalues (and file-scope compound literals as static objects),
    initialization of a `char` array by a string that exactly fills
    it, flexible array members initialized in static objects (GNU);
  - static initializers fold what clang folds (address constants
    through casts and offsets, pointer and label-address differences,
    IEEE floating results), and a C static is never initialized at run
    time: a non-constant initializer is clang's error;
  - statements: `goto` past an initialized declaration (but not into
    the scope of a VLA), and falling off the end of a non-void function
    without a trap (C only makes using the value undefined);
  - C99, C11 and C23 constructs: `_Bool`, `_Complex`, designated
    initializers, `_Static_assert`, `_Noreturn`, `_Alignas`, `_Alignof`,
    `_Thread_local`, `_Atomic`, anonymous structs and unions, C23
    `bool`/`true`/`false`, `nullptr`, `constexpr` objects, `auto`,
    `typeof`/`typeof_unqual`, `enum E : T`, `static_assert` without a
    message, `= {}`, `[[attributes]]` and `_BitInt(N)`.
- GNU C extensions, as clang implements them: statement expressions,
  `typeof`, `?:` with an omitted middle operand, case ranges and
  designator ranges, labels as values and computed `goto`, local labels
  (`__label__`), old-style `field:` designators, zero-length arrays,
  `transparent_union`, `cleanup`, `packed`, `__int128`, vector types,
  inline assembly and `__builtin_types_compatible_p` (WP-1276,
  WP-1287, WP-1288, WP-1290).
- What builds: five C projects with their own build systems pass their
  tests with kiln as the C compiler: zlib 1.3.2, sqlite 3.54.1, Lua
  5.4.8, c-ares 1.34.9 and zstd 1.5.7 (2026-11-11). GCC 16.1's
  `gcc.c-torture` suites at `-std=gnu23`, `-O0` and `-O2`: 6912 of 7572
  cases pass, against 6887 for clang 19 (2026-11-11, kiln at
  `-fclang-compat=19`). Every case kiln fails, clang 19 fails too
  (WP-1277, WP-1282, WP-1295, WP-1301, WP-1304).
- Diagnostics for the C-only rules are clang 22's, or clang 19's with
  `-fclang-compat=19`, including `-Wstrict-prototypes`,
  `-Wdeprecated-non-prototype`, `-Wincompatible-library-redeclaration`,
  `-Warray-parameter`, `-Wsizeof-array-argument` and
  `-Wsizeof-array-decay` (the last two in C++ too), C types printed as C
  (`struct S`, `_Bool`) (WP-1312).
- Linking: `kiln` links as `clang` does, so a C program does not pull
  in libstdc++ or libm; link C++ objects with `kiln++` (WP-1282).
- Precompiled C headers: `-x c-header` (or a `.h` input under `kiln`)
  writes a PCH that a C compile takes with `-include-pch` or by the
  `-include` probe (WP-1275, WP-1303).

### Added

- Precompiled headers: kiln's PCH works as in 1.0 (`-x c++-header`,
  `-emit-pch`, `-include-pch`, and now `-x c-header`): by default the
  consumer validates the PCH and then replays the header's tokens, so a
  PCH build costs what the same build with `-include` costs. 1.1.0 adds
  an experimental serialized form behind
  `-fkiln-pch-image[=write|check|load]` (`-fno-kiln-pch-image`), off by
  default: the producer also writes the header's preprocessor state
  and, when it can carry every declaration of the header, a
  declaration image; a `load` consumer then builds the declarations
  from the image instead of parsing the header, and `check` builds both
  ways and reports the first difference. A header holding anything the
  image cannot carry, and every C header, gets no image, and its
  consumers replay it; the two paths are never mixed. The image
  does not yet carry most standard library headers, so real prefixes
  still replay ([docs/status/pch.md](docs/status/pch.md)). It is
  planned as the default for 1.2.0 (spec/driver.md "The declaration
  image", WP-1280, WP-1036, WP-1293).
- GNU extensions in C++ too: local labels (`__label__`), old-style
  `field:` designators (`-Wgnu-designator`),
  `__builtin_extract_return_addr`, `__builtin_frob_return_addr` and
  `__builtin___clear_cache` (WP-1288).
- Driver: `-rpath <dir>` is passed to the link; `-Wa,<args>` and
  `-Xassembler <arg>` take the integrated assembler's options clang
  takes (`--noexecstack` and others) (WP-1288).
- Profile-guided build of kiln itself: `cargo xtask build --pgo` and
  `cargo xtask dist --pgo` build instrumented, train on a fixed corpus
  from `bench/`, merge with `llvm-profdata` (rustc's LLVM major, found in
  kiln's LLVM or rustup's `llvm-tools`) and rebuild with the profile. That
  is about three times the build time, for a faster compiler with the
  same memory use. `cargo xtask pgo-profile --out <file>` is the pre-step
  alone, for a packaging system that runs its own `cargo build --release`
  with `-Cprofile-use=<file>`. `cargo xtask pgo-train` is the training
  alone. Nothing changes without the switch (README.md "Profile-guided
  build", WP-1308).

### Changed

- The experimental declaration image of module files and PCHs
  (`-fkiln-bmi-image`, `-fkiln-pch-image`; both off by default) carries
  more of C++: locations in other files and in macro expansions,
  declaration attributes, linkage specifications, inline and unnamed
  namespaces, class, alias and variable templates with their
  specializations and member templates, friend templates, explicit
  instantiations, constraints, deduction guides, expressions in types
  and template arguments, block-scope `extern` declarations and
  `static_assert` in templates. Whatever it does not carry falls back
  to the 1.0 replay. `-fkiln-bmi-image-census` prints, per reason, what
  a unit's image would refuse (WP-1070, WP-1278, WP-1279, WP-1283,
  WP-1284, WP-1292, WP-1293, WP-1300, WP-1307, WP-1310).

### Fixed

- Crash: a GNU binary conditional whose condition is a logical,
  relational or `!` expression (`(a || b) ?: c`) no longer stops code
  generation with the internal error "phi operand %N is defined", in C
  and C++ (WP-1288).
- Crash: an `asm` statement with a `_Complex` operand no longer crashes
  the backend; the operand is passed indirectly, as clang does
  (WP-1288).
- `__builtin_constant_p` of a value that is not a constant expression
  is left to the optimizer (`llvm.is.constant`), as in clang, so it is
  1 once inlining makes the argument constant. It was always 0
  (WP-1288).
- AArch64: the NEON table builtins `vqtbl2` to `vqtbl4`, `vqtbx1` to
  `vqtbx4`, `vtbl3`, `vtbl4` and `vtbx1` to `vtbx4` were "not supported
  in code generation". They are lowered as clang 19 lowers them
  (WP-1288).
- Parsing: a later declarator of a group may begin with GNU attributes
  (`void f(void), __attribute__((noreturn)) g(void);`), in a member
  declaration too (WP-1301). A compound literal after `sizeof` or
  `alignof` (`sizeof (struct P){ 0 }`) is the operand expression, not a
  type-id followed by a stray brace (WP-1287).
- Accepts-invalid:
  - a `static` namespace-scope variable after a declaration with
    external linkage (a block-scope `extern` included), and a
    non-`static` one after a `static` first declaration, are errors
    ([dcl.stc]/4); a block-scope `extern` declaration now redeclares a
    later declaration in any block of its namespace ([basic.link]/6)
    (WP-1293);
  - a function whose first declaration lacks `[[noreturn]]` while a
    later one has it is an error ([dcl.attr.noreturn]/1) (WP-1311);
  - a `reinterpret_cast`, or a C-style cast performing one, in the
    initializer of a `constexpr` or `constinit` variable is an error
    ([expr.const]/5.15). The same initializer of a variable that is
    neither is still static data, with no guard or dynamic
    initialization (WP-1295);
  - pointer arithmetic on a pointer to a variable length array (the GNU
    extension in C++) is not a constant expression (WP-1287).
- Module and PCH outputs: `--precompile -o /dev/null` or
  `-emit-pch -o /dev/null` (any path that exists and is not a regular
  file) is written in place, as clang does. Run as root, kiln replaced
  `/dev/null` with a regular file (WP-1281).
- Diagnostics: an error in an `#if` expression has clang's caret
  ranges (WP-1290).
- **Source-compatibility note: previously accepted code is now
  rejected.** A run-time use of an expansion-statement (`template for`)
  element variable whose type holds a reflection was accepted:
  `template for (constexpr auto e : plan) std::puts(e.name);` with
  `struct E { std::meta::info r; const char* name; }`. It is ill-formed
  (P2996R13 [expr.const]: `e` is an expression of consteval-only type
  outside a constant-evaluated context), as the same use of a plain
  `constexpr E e = plan[0];` already was, and is now rejected with the
  same "consteval-only expressions are only allowed in a
  constant-evaluated context" in every expansion form, for `constexpr
  const auto&` and `constexpr auto&&` element variables, for a
  structured-binding element declaration, and in function templates
  (also for a `constexpr T x` of a function template instantiated with
  such a class, and a binding of a `constexpr` structured binding of
  one). GCC 16.1 rejects it too. Write `constexpr const char* name =
  e.name; std::puts(name);` instead: the read is then inside a constexpr
  variable's initializer. No file in kiln's own tree relied on the old
  acceptance (WP-1330).

### Testing and tooling (not user-visible)

- The OSS harness builds C with kiln (`kiln-cc`, a per-project `c` key,
  per-language TU counts), with the five C rows above (WP-1277).
  `cargo xtask gcc-torture` runs GCC 16.1's `gcc.c-torture` suites
  through kiln, GCC and clang and writes the generated tables of
  docs/status/c-mode.md (WP-1277, WP-1282). The `tests/c/` corpus has
  clang 19 `-x c` as its oracle (WP-1275).
- PCH: `cargo xtask test` and `cargo xtask libcxx` take
  `--pch-diff load|replay|check`, a differential of the image and the
  replay path, run over a corpus slice in `cargo xtask ci` (WP-1036).
  `cargo xtask bench bench/pch` measures kiln's PCH against clang 19's
  and writes docs/status/pch.md, judging kiln's accounted memory rather
  than peak RSS (WP-1294, WP-1303).

## [1.0.2] - 2026-11-11

A bug-fix release: fixes found while building the reference workload,
each with a test. Nothing in it changes what
[docs/STABILITY.md](docs/STABILITY.md) promises.

### Fixed

- Wrong code: the default member initializers of a member class template
  were lost when a member declaration of the enclosing class instantiated
  the template before the end of that class
  (`struct Outer { template <class T> struct F { bool set{false}; T v; }; F<bool> f; };`).
  A constructor or aggregate initialization then left those members
  uninitialized. The initializer is now found at its first use; a use
  before it is parsed is the error it is for a non-template class
  ([class.mem.general]/8).
- Linking against libstdc++: a member explicitly defaulted on its first
  declaration is defined where it is used even when its class has an
  explicit instantiation declaration, as clang does (`linkonce_odr`).
  `std::filesystem` directory iteration failed to link without it.
- Debug info: a constexpr member of an explicitly instantiated class
  template (`template class std::vector<T>;`) could be emitted twice
  when an earlier member called it, and with `-g` LLVM then dropped the
  translation unit's debug info.
- Debug info: an explicit instantiation definition of a class template
  (`template struct V<int>;`) completes the specialization's type and
  keeps it in the compile unit's retained types, as clang does. The type
  was described only if a member that referred to it was emitted.
- Linking: a function called in the operand of `typeid` whose result is a
  glvalue of polymorphic class type is odr-used, so it is defined or
  instantiated: `typeid(*p)` with a class template's member `operator*`
  no longer leaves an undefined symbol at link time. A non-polymorphic
  operand still names nothing it odr-uses ([expr.typeid]/3).
- Overload resolution: a comparison whose regular and reversed rewritten
  candidates tie (`a < b` through `operator<=>(const Base&)`, or `a == b`
  through the pre-C++20 `operator==(const Base&)`) is a substitution
  failure in a requires-expression, a concept or a SFINAE'd signature, as
  in clang and GCC; outside SFINAE it is still accepted with
  `-Wambiguous-reversed-operator`.
- Lookup: the namespace-name of a using-directive or of a namespace alias
  is looked up among namespaces only ([basic.lookup.udir]). A class of
  the same name in a nearer scope no longer gives "expected namespace
  name".
- Constraints: a function's constraints are checked from the function, so
  a hidden friend's trailing requires-clause may name a private member of
  the befriending class. Such a friend was dropped from overload
  resolution, and a deduced-return template was chosen in its place.
- Placement new: a placement parameter of type reference to a base class
  accepts an argument of a derived class
  (`operator new(std::size_t, Allocator &)` with a derived allocator).
- Abstract classes: binding a reference to an abstract class from an
  object of an unrelated class through a converting constructor is
  ill-formed ([class.abstract]/3), and a substitution failure where one
  is allowed. GoogleTest's printer selection relied on it.
- Parsing: in `X<args>::name`, `name` is classified in the explicit
  specialization the template-id names when every argument is a decimal
  literal or `sizeof(name)`. Nested `BOOST_SCOPE_EXIT` blocks need it.
- Warnings:
  - a comparison or conditional expression with an unnamed enumeration
    warns under `-Wdeprecated-anon-enum-enum-conversion`, as in clang, so
    `-Wno-deprecated-anon-enum-enum-conversion` silences it;
  - `-Wcomment` no longer reports a multi-line `//` comment when the
    spliced-in line is itself a comment.

## [1.0.1] - 2026-11-11

A bug-fix release. Nothing in it changes what [docs/STABILITY.md](docs/STABILITY.md)
promises.

### Fixed

- Reflection mangling (the ABI note in
  [docs/STABILITY.md](docs/STABILITY.md#covered-stable-within-a-major-version)): two
  different dependent reflected template-ids could get the same symbol
  (`^^AT<T>` and `^^TT<T*>`, and `^^F<T,int>` and `^^F<T,long>` in gcc
  mode). They no longer collide (WP-1250). GCC 16.1's spelling is used
  wherever it cannot collide. Three of its spellings lose information,
  and GCC itself fails to assemble the resulting overload pairs: a
  dependent function template-id without its arguments, a qualified
  variable template-id without its qualifier, and an alias template-id
  whose aliased type does not pin down the alias's arguments. kiln keeps
  its own spelling in those three cases, so the names differ from GCC's
  there; spec/abi.md §8.14 lists them. A reflected alias whose parameter
  appears only in a non-deduced context now gets GCC's `ta` form when
  its argument is dependent, so it links with g++-16 (WP-1260). A value-dependent reflection is no longer evaluated before
  instantiation (WP-1260).
- A consteval-only reference in a template is checked again in each
  instantiation. A dependent initializer such as
  `constexpr bool b = m == members[0];` in a `template for` body is no
  longer rejected (WP-1263, P3032R2).
- A failed `consteval` block in a lambda body leaves the lambda valid
  (WP-1257). A failed `static_assert` in a lambda body no longer stops
  evaluation, so a second refusal is reported too (WP-1247).
- Diagnostics:
  - When a reflection query triggers an instantiation error, the
    diagnostic names the consteval call frames (WP-1248). Those frames
    still show under copy elision and nesting, and spec names keep their
    template arguments (WP-1258).
  - Text the program provides (an uncaught exception's `what()`,
    attribute messages, reflected names) is escaped as clang escapes it
    (WP-1259).
  - An uncaught exception's `what()` keeps the characters read before a
    fault, and its length cap does not split a UTF-8 sequence (WP-1251).
  - An invalid escape in an unevaluated string (a `static_assert`
    message, `[[deprecated]]`, `[[nodiscard]]`, `extern "…"`, asm) no
    longer causes follow-on errors. The construct is dropped, as in
    clang 22. Escape diagnostics quote the whole escape and use clang 22's
    source ranges (WP-1268).
  - Column widths and the escaping of unprintable characters use LLVM
    22's Unicode 15.1 tables, so an unassigned code point is shown as
    `<U+XXXX>` as clang shows it (WP-1268).
- Invalid UTF-8 in an ordinary string or character literal is the
  `-Winvalid-source-encoding` warning, not an error, as in clang
  (WP-1267).
- Under `-fkiln-source-path-style=gcc`, header paths are spelled as
  g++-16 spells them (the `-I` directory exactly as given, `"../x.h"`
  relative to the includer). This is the name that `source_location` and
  `std::meta::source_location_of` report (WP-1262).
- C++20 modules: the experimental declaration image
  (`-fkiln-bmi-image`) now covers imports, partitions and
  using-declarations (WP-1068).

### Testing and tooling (not user-visible)

- The harness can run GCC 16.1 as the compiler under test
  (`--compiler $KILN_GCC`). It runs clean over the reflection and
  contracts tests (WP-1249), and every harness command maps its flags for
  GCC (WP-1261, WP-1266). Test compiles pass `-fno-crash-diagnostics`
  (WP-1264).
- The clang oracle retries once when a compile dies on a signal
  (WP-1254).
- The sweep workflow's `oss` job fails when it produces no report
  (WP-1241). It runs as one `oss-matrix` job over the `sweep = true` rows
  of `projects.toml` (WP-1265).
- `release.yml` uploads the dist archives from the workspace, and can be
  started by hand as a dry run (WP-1253).
- Spec notes with pinning tests: `type_of(reflect_constant(^^int))`
  (WP-1255), and `reflect_constant` of a function reflection's value,
  which instantiates nothing (WP-1252).

## [1.0.0] - 2026-11-10

The first release. This section summarises what kiln 1.0 does. Measured
results are in [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md), and the
limits are in [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md).

### Language

- C++98 through C++23, and C++26 as in the Croydon DIS draft: 260 of the
  261 rows of [docs/STANDARD_SUPPORT.md](docs/STANDARD_SUPPORT.md) are
  done. The one partial row is P2786 (trivial relocatability), which was
  removed from C++26 at Kona and is not in the DIS.
- C++26 static reflection (P2996, `<meta>`), on by default at
  `-std=c++26` (`-fno-reflection` turns it off). The `<meta>` header is
  GCC 16.1's libstdc++ one, selected with `--gcc-toolchain=`; libstdc++
  14 and libc++ 22 do not ship it. When `#include <meta>` is not found at
  C++26, a note names the GCC installation (or libc++) kiln selected, and
  a GCC 16 found through `g++-16` or `g++` on `PATH`.
- C++26 contracts (P2900): `pre`, `post`, `contract_assert`, the four
  evaluation semantics (`-fcontract-evaluation-semantic=`), and a
  replaceable violation handler.
- C++26 erroneous behaviour (P2795): at `-std=c++26` an uninitialized
  automatic variable (up to 4096 bytes) is zero-filled at its
  declaration. clang 22 does not do this. `[[indeterminate]]` on the
  variable, or `-ftrivial-auto-var-init=uninitialized`, opts out.
- C++20 modules, header units and `import std;`, in one invocation or in
  clang's two-step build: `--precompile`, `-fmodule-output`,
  `-fmodule-file=`, `-fprebuilt-module-path=`. kiln's module files are
  its own format. The declaration image (`-fkiln-bmi-image`, which builds
  an imported module from a serialized image instead of replaying it) is
  experimental and off by default; it does not yet cover `import std;`.
- Dependency scanning: `kiln-scan-deps` (also installed as
  `clang-scan-deps-22` and `clang-scan-deps`) writes P1689r5, so CMake
  3.28+ with Ninja builds C++20 module targets unchanged.
- Coroutines, exceptions, RTTI, `thread_local`, and clang's and GCC's
  common extensions and builtins.

### Constant evaluation (the Oven)

- All constant evaluation (`constexpr`, `consteval`, `static_assert`,
  template arguments, reflection) runs in the Oven, kiln's bytecode
  evaluator. It has exact per-evaluation budgets for steps, depth and
  live memory (`-fconstexpr-steps=`, `-fconstexpr-depth=`,
  `-fconstexpr-memory=`), and memory freed during an evaluation is
  reclaimed during that evaluation.
- On kiln's benchmark corpus, compared with clang 19: geometric mean
  0.21x the time and 0.17x the peak memory
  ([docs/status/perf.md](docs/status/perf.md)).
- Reports: `-fconstexpr-report=` (NDJSON per evaluation),
  `-fkiln-instantiation-report`, `-fkiln-memory-report`.

### Standard libraries and compatibility

- libc++ 22.1.8 test suite at `-std=c++26`: 99.4%. libc++ 19.1.7: 98.3%.
  Every public libstdc++ 14 header compiles at `-std=c++20` and
  `-std=c++26`.
- 47 open-source projects built with their own build systems and test
  suites, including all of LLVM, Boost, folly, Abseil, DuckDB and CMake:
  46 at parity with clang 19 or better on every test; the Boost
  metaprogramming libraries match clang 22.
- clang-compatible command line and predefined macros. CMake and
  autotools work unchanged. kiln reproduces clang 22 by default and
  clang 19 with `-fclang-compat=19`.

### Code generation and tools

- Writes LLVM IR as text and drives LLVM's tools. It never links against
  LLVM. Supports LLVM 22 (preferred) and LLVM 19, found on its own
  (`--llvm-dir=` and `KILN_LLVM_DIR` name one); `--version` and `-v`
  print the LLVM in use.
- Itanium C++ ABI as clang implements it, on Linux aarch64 and x86_64.
- `-O0` to `-O3`, `-Os`, `-Oz`; LTO (`-flto`, `-flto=thin`);
  floating-point model flags; AArch64 branch protection.
- DWARF debug information (`-g` and its levels, `-gdwarf-N`) that gdb and
  lldb read as they read clang's.
- Sanitizers: address, hwaddress, thread, memory, leak, fuzzer, and
  undefined behaviour.
- Coverage: `--coverage` (gcov) and `-fprofile-instr-generate`
  counters. Coverage mapping and PGO are not in 1.0.

### Robustness and distribution

- An internal compiler error prints a report and writes a reproducer
  (`.ii` and `.sh`) instead of a panic
  ([docs/REPORTING-BUGS.md](docs/REPORTING-BUGS.md)). kiln runs out of
  stack only as a diagnostic.
- Every crate is `#![forbid(unsafe_code)]`. The MSRV is Rust 1.95.
- Release archives from `cargo xtask dist`: relocatable,
  `kiln-<version>-<triple>.tar.xz` with a stripped binary, plus a
  `-symbols` archive and `SHA256SUMS`.

### Fixed in the final release candidate

These landed after the first 1.0 release candidate was measured.

- x86 builtins: every x86 builtin that clang lowers to generic IR is now
  lowered (AVX and AVX2 extract, insert and `perm2x128`, F16C, the FMA
  family and more). The builtins kiln declares but cannot lower went
  from 595 to 23 (`maskmovq`, Key Locker, the AMX internals), and those
  23 answer `__has_builtin` with 0. A call of an x86 builtin sets
  `min-legal-vector-width`, which fixes an LLVM backend crash at
  `-march=skylake-avx512 -O2`. OpenCV's AVX2 dispatch TUs now compile
  (WP-1242).
- Reflection: `extract` of a function now instantiates and odr-uses it
  (WP-1244).
- Constant evaluation: an uncaught exception in a constant evaluation
  prints its `what()` whole, up to 64 KiB, after which it ends with
  "… [N more characters]" (WP-1245).
- Reflection: `^^` accepts operator, literal-operator, conversion and
  dependent template-ids, such as `^^ns::operator==<int>` (WP-1246).
- Reflection: `members_of` leaves out members whose type is an undeduced
  placeholder, and `type_of` on one throws, as in GCC.
  `display_string_of` prints requires-clauses as GCC 16.1 does
  (WP-1243).

## Before 1.0

Before 1.0 kiln was developed in the open on `main`, at version 0.0.1,
with no tagged releases. The history is in the git log, the milestones
in [docs/ROADMAP.md](docs/ROADMAP.md), and each work package's record in
[docs/BACKLOG.md](docs/BACKLOG.md) and `spec/landed/`.

[1.0.2]: https://github.com/christopherfretz/kiln/releases/tag/v1.0.2
[1.0.1]: https://github.com/christopherfretz/kiln/releases/tag/v1.0.1
[1.0.0]: https://github.com/christopherfretz/kiln/releases/tag/v1.0.0
