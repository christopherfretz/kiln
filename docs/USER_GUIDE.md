# kiln user guide

This guide covers using kiln: installing it, using it from a build system,
choosing a language mode, working with the Oven's limits, and the flags
and environment variables that differ from clang. The complete
command-line contract is [spec/driver.md](../spec/driver.md). What a
version number promises is in [STABILITY.md](STABILITY.md). What does not
work yet is in [KNOWN_ISSUES.md](KNOWN_ISSUES.md).

## Install

kiln supports Linux on aarch64 and x86_64 and nothing else. Each
[GitHub release](https://github.com/christopherfretz/kiln/releases) has
two archives per architecture and a checksum file:

- `kiln-<version>-<triple>.tar.xz`: the compiler. `bin/kiln` has its debug
  information stripped but keeps its symbol table, so crash reports name
  functions.
- `kiln-<version>-<triple>-symbols.tar.xz`: the same tree with the
  unstripped binary. You only need it to debug kiln itself.
- `SHA256SUMS`.

```sh
sha256sum -c --ignore-missing SHA256SUMS
tar -xJf kiln-<version>-<triple>.tar.xz
export PATH=$PWD/kiln-<version>-<triple>/bin:$PATH
kiln++ --version
```

The tree is relocatable, so you can unpack it anywhere:

```
bin/kiln
bin/kiln++ -> kiln            kiln++ compiles every input as C++
lib/kiln/<major>/include/     clang's builtin headers, <contracts>
lib/kiln/<major>/predefines/  predefined macro sets
share/doc/kiln/               README, licenses, COMPATIBILITY, STABILITY
```

kiln finds `lib/kiln/<major>` next to the binary on its own.
`-resource-dir=<dir>` or `KILN_RESOURCE_DIR` override that location (see
spec/driver.md, "Toolchain").

To build the same archives from a checkout, run `cargo xtask dist`. See
[CONTRIBUTING.md](../CONTRIBUTING.md).

### Run-time dependencies

kiln writes LLVM IR and runs LLVM's tools to finish the job, so you need:

- **LLVM 19 or LLVM 22**, with `llc`, `llvm-as` and `clang++`. The
  `clang++` performs the link, so its linker, crt files and runtime
  libraries are the ones used. kiln looks for LLVM in `$KILN_LLVM_DIR`,
  which defaults to `/usr/lib/llvm-19`. To use LLVM 22, point it there,
  for example `KILN_LLVM_DIR=/usr/lib/llvm-22`. kiln detects the major
  version and emits IR in that LLVM's dialect. LLVM 20 and 21 are not
  supported.
  <!-- update when WP-1016 lands: default 22, discovery order (--llvm-dir, KILN_LLVM_DIR, llvm-config, /usr/lib/llvm-22, /usr/lib/llvm-19, PATH), 20/21 refused, -v prints the LLVM, per-distro one-liners in COMPATIBILITY.md -->
- **A C++ standard library.** The default is the system libstdc++ from
  the GCC installation clang would pick (GCC 14 is what kiln is tested
  with). For libc++, pass `-stdlib=libc++`; it comes from the LLVM
  directory.
- **glibc 2.39 or newer** for the binary itself (Debian 13, Ubuntu
  24.04). Lowering that floor is tracked as
  [WP-943](BACKLOG.md).
- For coverage and sanitizers, also LLVM's compiler-rt runtimes (Debian:
  `libclang-rt-19-dev`).

On Debian 13: `sudo apt install clang-19 llvm-19 g++` (add
`libclang-rt-19-dev` for coverage or sanitizers).

## Using kiln from a build system

`kiln++` accepts clang's command line and defines clang's predefined
macros (`__clang__`, `__clang_major__` 22, `__GNUC__` 4.2.1). In text,
`--version` says `kiln version <v>`. It never claims to be clang. Detail
for each build system, with the projects that validate it, is in
[COMPATIBILITY.md](COMPATIBILITY.md#build-systems).

- **CMake** works unchanged. It reports
  `The CXX compiler identification is Clang 22.1.8` and uses its Clang
  rules:

  ```sh
  CXX=kiln++ cmake -S . -B build
  # or: cmake -DCMAKE_CXX_COMPILER=kiln++ ...
  ```

- **Autotools** works unchanged: `./configure CXX=kiln++`. `AC_PROG_CXX`
  sees a GNU-compatible compiler through `__GNUC__`, and libtool
  queries the linker through `-print-prog-name=ld` and `-shared -v`, both
  of which kiln answers as clang does.
- **Meson** does not accept kiln directly. Meson requires the word
  `clang` in `--version` output, and kiln's banner does not contain it,
  so configure stops with `Unknown compiler(s)`. Use a wrapper that
  prints a clang-compatible line first, as kiln's own OSS harness does
  (`scripts/oss/kiln-cxx`):

  ```sh
  #!/bin/sh
  # kiln-meson: lets Meson recognise kiln++ as Clang
  [ "$1" = --version ] && echo "clang-compatible $(kiln++ -dumpversion) (kiln wrapper)"
  exec kiln++ "$@"
  ```

  Then run `CXX=kiln-meson meson setup build`. Meson reads this as
  Clang 22.1.8. See [KNOWN_ISSUES.md](KNOWN_ISSUES.md#build-systems).

Probe flags behave as clang 22.1.8's: `-dumpversion` prints `22.1.8`,
and `-dumpmachine`, `-print-search-dirs`, `-print-file-name=`,
`-print-prog-name=`, `-print-resource-dir`, `-print-target-triple` and
`-print-libgcc-file-name` are implemented (spec/driver.md, "Toolchain
probes"). For code that must tell kiln apart, test `__KILN__` and
`__KILN_MAJOR__`, `__KILN_MINOR__`, `__KILN_PATCH__`.

## Language modes

- **`-std=`**: `c++98` to `c++26` and their `gnu++` forms, with clang's
  aliases (`c++2c`, `c++23`/`c++2b`, ...). The default is clang 22's,
  `gnu++17`. Feature status per standard is in
  [STANDARD_SUPPORT.md](STANDARD_SUPPORT.md). kiln targets the C++26
  Croydon DIS draft.
- **Reflection** (P2996, `^^`, splices, `<meta>`) is on by default at
  `-std=c++26`. Turn it off with `-fno-reflection`. Below C++26 it is
  always off, and an explicit `-freflection` is accepted and ignored.
  `-fkiln-reflection-mangling=kiln|gcc|kiln-hashed` chooses how a
  `std::meta::info` template argument is mangled. `gcc` is link-compatible
  with GCC 16.1. The default is `kiln`. This flag is experimental (see
  [STABILITY.md](STABILITY.md)).
- **Contracts** (P2900: `pre`, `post`, `contract_assert`) are supported
  at C++26. `-fcontract-evaluation-semantic=ignore|observe|enforce|quick_enforce`
  (GCC's spelling) selects the semantic for the translation unit. The
  default is `enforce`. A violation calls the default handler from
  kiln's `<contracts>`, or your own `handle_contract_violation`.
- **clang compatibility level.** kiln reproduces clang 22's observable
  behaviour by default: predefined macros, `__has_*` answers and the IR
  details that differ between releases. `-fclang-compat=19` selects
  clang 19's instead. This flag is independent of which LLVM installation
  is used.
- **C**: `kiln` (not `kiln++`) treats `.c`, `.h` and `.i` as C, and can
  only preprocess them (`-E`). See
  [KNOWN_ISSUES.md](KNOWN_ISSUES.md#c-mode).

### Modules

C++20 modules work for `-fsyntax-only` and code generation from
`-std=c++20` on. kiln has no binary module interface (BMI) yet. An
imported module is compiled in-process from its interface source, so the
flags name sources:

- `-fmodule-file=<name>=<path>` maps a module (or partition) name to its
  interface source.
- `-fprebuilt-module-path=<dir>` searches `<dir>` for `<name>.cppm`,
  `.ixx` or `.cpp` (a partition `M:P` as `M-P.*`).
- `import std;` and `import std.compat;` are found in the LLVM
  directory's `share/libc++/v1/`.
- Header units (`import <h>;`) are found like `#include`.

To link, compile every module unit whose definitions you use, either as
an input of the same command or separately with `-c`:

```sh
kiln++ -std=c++20 -fmodule-file=m=m.cppm m.cppm main.cpp -o prog
```

`--precompile`, `-fmodule-output` and `-fmodule-header` are not
supported yet. See [KNOWN_ISSUES.md](KNOWN_ISSUES.md#modules).
<!-- update when WP-917 lands: BMI ("source capsule"), --precompile, -fmodule-output[=], two-step compiles, mismatch diagnostics -->

## The Oven: compile-time evaluation

The Oven evaluates every constant expression: `constexpr` and `consteval`
calls, `static_assert`, template arguments, and reflection. Its budgets
are per evaluation and exact. When one is exceeded, the expression is not
a constant expression and kiln reports an error with a note at the point
where the limit hit, plus up to `-fconstexpr-backtrace-limit` call
frames.

| Flag | Limits | Default |
|---|---|---|
| `-fconstexpr-steps=N` | evaluation steps (statements, loop iterations, calls) | 1048576 |
| `-fconstexpr-ops-limit=N` | GCC's spelling; sets the same step budget | — |
| `-fconstexpr-depth=N` | call depth | 512 |
| `-fconstexpr-memory=N[k\|m\|g]` | live abstract memory per evaluation (kiln only) | 256 MiB |
| `-fconstexpr-backtrace-limit=N` | frames in the note backtrace; 0 shows all | 10 |
| `-ftemplate-depth=N` | template instantiation depth | 1024 |
| `-fkiln-stack-size=N[k\|m\|g]` | the compiling thread's stack (kiln only; at least 4 MiB) | 1 GiB |

For example, hitting the memory budget looks like this:

```
x.cpp:2:15: error: static assertion expression is not an integral constant expression
x.cpp:1:37: note: constexpr evaluation exceeded the memory limit of 1048576 bytes
```

The step limit prints `note: constexpr evaluation hit maximum step limit;
possible infinite loop?`, as clang does. Memory freed during an
evaluation is reclaimed during that evaluation, so the memory budget
counts live bytes, not total allocation. Running out of stack never
crashes kiln. It is a fatal `kiln: stack exhausted during …` error;
raise `-fkiln-stack-size` if you hit it.

### Reports and profiling

- `-fconstexpr-report=<path>` writes one NDJSON line per top-level
  evaluation of the translation unit (its location and kind, steps,
  bytes, peak memory, calls, allocations, wall time) and a final `total`
  line. The format is in spec/oven.md, "Reporting protocol".
- `-fkiln-instantiation-report[=all|json|text]` prints per-template
  instantiation counts, AST nodes, bytes and self/inclusive time. Use it
  to find which templates dominate a slow TU.
- `-fkiln-memory-report` prints kiln's arena usage by subsystem.
- `-ftime-trace` is accepted, because build systems add it for clang,
  but it writes no file and warns. Use the reports above instead.

The `-fkiln-*` report flags and their formats are experimental and may
change in a minor release ([STABILITY.md](STABILITY.md)).

## Flags

kiln implements clang's driver flags for the Linux targets. A flag clang
has and kiln does not is an error (`unknown argument`, or `'<flag>' is
not supported by kiln`), so a build never silently loses a flag's
effect. The few flags that are accepted with no effect are listed in
[KNOWN_ISSUES.md](KNOWN_ISSUES.md#flags-and-attributes). `kiln++ --help`
gives a short list. The full list is spec/driver.md, "Options".

### kiln-only flags

| Flag | Effect |
|---|---|
| `-fconstexpr-memory=N[k\|m\|g]` | Oven memory budget (above) |
| `-fkiln-stack-size=N[k\|m\|g]` | compiling thread's stack (above) |
| `-fclang-compat=19\|22` | which clang release's behaviour to reproduce (default 22) |
| `-fkiln-reflection-mangling=kiln\|gcc\|kiln-hashed` | mangling of reflection template arguments |
| `-fkiln-instantiation-report[=all\|json\|text]`, `-fkiln-memory-report` | reports (above) |
| `-fkiln-source-path-style=clang\|gcc` | how `__builtin_FILE()` and `std::source_location` spell a file name: as clang does (default) or as GCC does |
| `-fkiln-reproducible-time` | fixed `__DATE__`/`__TIME__`/`__TIMESTAMP__` (see [Environment variables](#environment-variables)) |
| `-fkiln-outline-expansions=N`, `-fkiln-dump-cfg`, `-fkiln-cfg-stats`, `-fkiln-cfg-max-elems=N` | internal tuning and debugging of the codegen CFG |

All `-fkiln-*` flags are experimental.

### Flags that behave differently from clang

- `-fcontract-evaluation-semantic=` uses GCC's spelling. clang 22 has no
  P2900 contracts.
- `-fconstexpr-ops-limit=` (GCC's) is accepted and sets the step budget.
- `-ftime-trace[=]`, `-ftime-trace-granularity=` and
  `-ftime-trace-verbose` warn and write nothing.
- `-fcoverage-mapping` and the PGO flags are rejected (see
  [Coverage](#coverage)).
- `--target=` accepts only aarch64 and x86_64 Linux triples.
- `-x c` and `.c` inputs can only be preprocessed. `kiln++` warns about a
  `.c` input and compiles it as C++, as `clang++` does.
- The full list of intentional differences is spec/driver.md,
  "Deviations from clang 19".

## Environment variables

| Variable | Effect |
|---|---|
| `KILN_LLVM_DIR` | LLVM installation (default `/usr/lib/llvm-19`) <!-- update when WP-1016 lands --> |
| `KILN_RESOURCE_DIR` | resource directory (builtin headers, predefines), if `-resource-dir` is not given |
| `KILN_REPRODUCIBLE_TIME`, `SOURCE_DATE_EPOCH` | fixed values for `__DATE__`, `__TIME__`, `__TIMESTAMP__` (spec/driver.md, "Reproducible time") |
| `TMPDIR` | where crash reproducers go when `-fcrash-diagnostics-dir` is not given (default `/tmp`) |
| `KILN_BACKTRACE=1` (or `RUST_BACKTRACE=1`) | add kiln's own backtrace to a crash report |
| `LLVM_PROFILE_FILE` | at run time, where an `-fprofile-instr-generate` program writes its profile (LLVM's runtime) |

Packagers can set `KILN_DEFAULT_LLVM_DIR` and `KILN_DEFAULT_GCC` when
building kiln, to compile in a different default LLVM directory or a GCC
driver outside `/usr` (spec/driver.md, "Toolchain").

## Debugging, sanitizers and coverage

### Debug information

`-g` and its levels (`-g1`, `-gline-tables-only`, `-g2`, `-g3`), `-gdwarf-2`
to `-gdwarf-5`, `-fstandalone-debug`, `-fdebug-prefix-map=` and the other
clang debug flags emit DWARF that gdb and lldb read as they read
clang's. A flag kiln cannot honour is an error, not ignored. The details
are in spec/driver.md, "Debug information flags", and
[status/debuginfo.md](status/debuginfo.md).

### Sanitizers

`-fsanitize=` supports `address`, `hwaddress`, `thread`, `memory`,
`leak`, `fuzzer`, `fuzzer-no-link`, `undefined`,
`unsigned-integer-overflow` and `unsigned-shift-base`, with clang's
groups and the `-fsanitize-recover`/`-fsanitize-trap` options. The
runtime is the linking `clang++`'s. Other sanitizers (cfi, dataflow,
safe-stack, kcfi, memtag, ...) are rejected. A few `undefined` checks and
the ignorelist are not done yet; see
[KNOWN_ISSUES.md](KNOWN_ISSUES.md#sanitizers-and-coverage).

### Coverage

- `--coverage` (gcov line coverage) works. `llvm-cov gcov` reports the
  same line counts as for clang 19.
- `-fprofile-instr-generate[=<file>]` (source-based coverage counters)
  works. `llvm-profdata show --counts` matches clang 19.
- `-fcoverage-mapping` (`llvm-cov show`/`report`) is not implemented yet
  and is rejected.
- Profile-guided optimization (`-fprofile-generate`, `-fprofile-use`,
  `-fprofile-instr-use`, `-fprofile-sample-use`, `-fcs-profile-generate`)
  is not part of 1.0 and is rejected with an error that says so.

Both coverage modes link LLVM's profile runtime (`libclang-rt-19-dev`).
See [COMPATIBILITY.md](COMPATIBILITY.md#coverage-and-profiling).

## Targets and LLVM versions

- **Targets:** `aarch64-unknown-linux-gnu` and `x86_64-unknown-linux-gnu`
  (and their clang aliases). The default is the host. Any other `--target`
  is `unsupported target`. `-march=`, `-mcpu=` and AArch64's feature
  flags work. x86 `-m<feature>` flags such as `-mavx2` are not accepted
  yet; see [KNOWN_ISSUES.md](KNOWN_ISSUES.md#platforms).
- **LLVM:** 19 and 22. The LLVM major version selects the IR dialect
  kiln writes. It does not change the language: language and macro
  behaviour follow `-fclang-compat`. LTO (`-flto`, `-flto=thin`) works
  with the selected LLVM.
  <!-- update when WP-1016 lands: default 22 -->

## Crashes

When kiln crashes it prints an internal compiler error report instead of
a panic:

```
kiln++: error: internal compiler error: <message>
kiln++: note: while compiling 'foo.cpp' (phase: parse, at foo.cpp:12:1)
...
kiln++: note: preprocessed source and associated run script are located at:
kiln++: note: diagnostic msg: /tmp/foo-3fa9c1.ii
kiln++: note: diagnostic msg: /tmp/foo-3fa9c1.sh
```

The `.ii` and `.sh` are a reproducer. Please attach both, with the whole
report, to an issue. [REPORTING-BUGS.md](REPORTING-BUGS.md) explains the
report, how to reduce a reproducer, and the controls:
`-fcrash-diagnostics-dir=<dir>`, `-fno-crash-diagnostics` and
`KILN_BACKTRACE=1`.

Exit status: 0 on success, 1 when an error was reported (including after
a crash), and 70 after a crash under `-fno-crash-diagnostics`.

If a crash looks security-relevant, for example it is triggered by
untrusted input in a service that compiles it, follow
[SECURITY.md](../SECURITY.md) instead of filing a public issue.
