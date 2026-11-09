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
bin/kiln-scan-deps -> kiln    the dependency scanner, also linked as
                              clang-scan-deps-22 and clang-scan-deps
lib/kiln/<major>/include/     clang's builtin headers, <contracts>
lib/kiln/<major>/predefines/  predefined macro sets
lib/kiln/<major>/share/       default sanitizer ignorelists
share/doc/kiln/               README, CHANGELOG, CONTRIBUTING, SECURITY,
                              licenses, COMPATIBILITY, KNOWN_ISSUES,
                              STABILITY, USER_GUIDE
```

kiln finds `lib/kiln/<major>` next to the binary on its own.
`-resource-dir=<dir>` or `KILN_RESOURCE_DIR` override that location (see
spec/driver.md, "Toolchain").

To build the same archives from a checkout, run `cargo xtask dist`. See
[CONTRIBUTING.md](../CONTRIBUTING.md).

### Run-time dependencies

kiln writes LLVM IR and runs LLVM's tools to finish the job, so you need:

- **LLVM 22 or LLVM 19, with clang**: kiln runs `opt`, `llc`,
  `llvm-as` and `clang++` from it. The `clang++` performs the link, so
  its linker, crt files and runtime libraries are the ones used. kiln
  writes IR in the selected LLVM's dialect. LLVM 20 and 21 are refused
  (`unsupported LLVM major N; kiln supports LLVM 19 and 22`). kiln takes
  the first usable installation of
  ([spec/driver.md](../spec/driver.md), "LLVM discovery"):
  1. `--llvm-dir=<dir>`;
  2. `KILN_LLVM_DIR`;
  3. a default compiled into the build by a packager
     (`KILN_DEFAULT_LLVM_DIR`);
  4. for LLVM 22, then LLVM 19: `llvm-config` or `llvm-config-<N>` on
     `PATH`, `/usr/lib/llvm-<N>` (Debian, Ubuntu), `/usr/lib64/llvm<N>`
     (Fedora), Homebrew on Linux's `llvm@<N>` and `llvm`, `/usr`, then
     `opt-<N>` or `clang++-<N>` on `PATH`.

  An installation is usable when it has `bin/opt`, `bin/llc` and
  `bin/clang++` and is LLVM 19 or 22. A directory you name with
  `--llvm-dir` or `KILN_LLVM_DIR` that is not usable is an error; kiln
  does not fall back to another one. When nothing is found, kiln lists
  the places it looked. `-fsyntax-only`, `-E` and `-S -emit-llvm` work
  without LLVM. `kiln++ --version` and `-v` print the choice as
  `LLVM backend: <major> (<dir>)`.
- **A C++ standard library.** The default is the system libstdc++ from
  the GCC installation clang would pick (GCC 14 is what kiln is tested
  with). For libc++, pass `-stdlib=libc++`; it comes from the LLVM
  directory.
- **glibc 2.39 or newer** for the binary itself (Debian 13, Ubuntu
  24.04). Lowering that floor is tracked as
  [WP-943](BACKLOG.md).
- For coverage and sanitizers, also LLVM's compiler-rt runtimes (Debian:
  `libclang-rt-<N>-dev`).

On Debian 13, LLVM 19 is in Debian's own archive:
`sudo apt install clang-19 llvm-19 g++` (add `libclang-rt-19-dev` for
coverage or sanitizers). LLVM 22 comes from
[apt.llvm.org](https://apt.llvm.org). The packages for each
distribution, Fedora included, are in
[COMPATIBILITY.md](COMPATIBILITY.md#runtime-dependencies).

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

  C++20 modules (`FILE_SET CXX_MODULES`) need CMake 3.28 or newer and
  the Ninja generator, as with clang. See [Modules](#modules).

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
`-std=c++20` on. They work in one invocation, where an importer compiles
the interface source it is given, and in clang's two-step build with
module files. kiln's module files use the `.pcm` extension and clang's
flags, but the format is kiln's own (spec/modules.md §11): kiln refuses
clang's module files with a diagnostic, and clang refuses kiln's.

- `--precompile m.cppm [-o m.pcm]` writes the module file of an interface
  or partition unit, and nothing else.
- `-c m.cppm -fmodule-output[=path]` writes the object and the module
  file in one compile. Without a path the module file goes next to the
  object (`-o x/m.o` gives `x/m.pcm`).
- `-c m.pcm` compiles a module file into the interface's object.
- `-fmodule-file=<name>=<path>` maps a module (or partition) name to a
  module file or to its interface source. kiln tells them apart by
  content, not by extension.
- `-fprebuilt-module-path=<dir>` searches `<dir>` for `<name>.pcm`, then
  `<name>.cppm`, `.ixx` or `.cpp` (a partition `M:P` as `M-P.*`).
- `import std;` and `import std.compat;` (with `-stdlib=libc++`) are
  built from libc++'s `std.cppm` and `std.compat.cppm` in
  `<prefix>/share/libc++/v1/`, beside the libc++ headers kiln uses (else
  in the LLVM directory's), and cached (see
  [The `std` module cache](#the-std-module-cache)).
- Header units (`import <h>;`) are found like `#include`.

To link, compile every module unit whose definitions you use, either as
an input of the same command or separately:

```sh
# one invocation
kiln++ -std=c++20 -fmodule-file=m=m.cppm m.cppm main.cpp -o prog
# two steps
kiln++ -std=c++20 -c m.cppm -fmodule-output -o m.o
kiln++ -std=c++20 -fmodule-file=m=m.pcm main.cpp m.o -o prog
```

A module file records the options it was built with that change meaning
(standard, target, language and codegen options). Importing it into a
compile with different options is an error naming each option, as in
clang. A missing, damaged or clang-built module file is a fatal error,
never a crash. Module files may only be read by the kiln build that wrote
them ([STABILITY.md](STABILITY.md#not-covered-may-change-in-any-release)).
Rebuild them when you upgrade kiln.

**Dependency scanning.** `kiln-scan-deps` is kiln's clang-scan-deps:
`kiln-scan-deps -format=p1689 -- kiln++ <flags> -c m.cppm` prints the
module dependencies as P1689r5 JSON, the same output as clang-scan-deps
22 (spec/driver.md, "Dependency scanning"). It is installed beside
`kiln++`, also as `clang-scan-deps-22` and `clang-scan-deps`, which is
where CMake looks first. CMake 3.28 or newer with Ninja therefore builds
`FILE_SET CXX_MODULES` targets with kiln and no extra configuration. To
name the scanner explicitly, for example behind a compiler wrapper, pass
`-DCMAKE_CXX_COMPILER_CLANG_SCAN_DEPS=<prefix>/bin/kiln-scan-deps`.

CMake's `import std;` support (`CXX_MODULE_STD`, 3.30+) also works: kiln
answers `-print-file-name=libc++.modules.json`, and CMake builds
`std.cppm` with kiln like any other module. It needs CMake's
experimental gate, `-stdlib=libc++` and `CXX_STANDARD 23`; the gate
values per CMake version are in
[COMPATIBILITY.md](COMPATIBILITY.md#c20-modules).

`-fmodule-header` and header-unit module files are not supported yet.
See [KNOWN_ISSUES.md](KNOWN_ISSUES.md#modules).

#### The `std` module cache

`import std;` and `import std.compat;` need no flags. The first compile
builds libc++'s module into a per-user cache, and later compiles with the
same configuration import it from there (spec/driver.md, "Module cache";
spec/modules.md §11.12).

- **Location.** `-fmodules-cache-path=<dir>`, else
  `$XDG_CACHE_HOME/kiln/modules` if `XDG_CACHE_HOME` is an absolute
  path, else `$HOME/.cache/kiln/modules`. Each configuration has its own
  entry, `<root>/<32 hex digits>/std.pcm` plus `std.stamp`.
- **Configuration.** `-std=`, `-fno-exceptions`, `-O` levels, `-D`/`-U`,
  the target, the libc++ headers and the kiln build each select a
  separate entry. `-g`, `-fPIC` and warning flags do not.
- **Turning it off.** `-fno-kiln-module-cache` (`-fkiln-module-cache`
  turns it back on; the last one wins) or an empty
  `-fmodules-cache-path=`. Without a home directory there is no cache.
  With the cache off, kiln builds the module from its source in every
  compile, as it does on a miss.
- **Precedence.** `-fmodule-file=std=<file>` or a `std.pcm` in a
  `-fprebuilt-module-path` directory wins over the cache.
- **Staleness.** An edited libc++ header or a damaged module file is a
  miss, and the entry is rebuilt.
- **Failure is not an error.** An unwritable cache directory or a failed
  build falls back to building the module from its source.
- **Parallel builds** share the cache. One compile builds a missing
  entry and the others wait for it, for at most 120 seconds, then build
  the module themselves.
- **`-v`** prints one line per lookup on standard error:

  ```
  kiln++: module cache: using '<entry>/std.pcm' for 'std'
  kiln++: module cache: building 'std' into '<entry>/std.pcm'
  kiln++: module cache: cannot create '<dir>': <reason>; building 'std' from its source
  kiln++: module cache: waiting for another compile building 'std'
  ```

- **`-MD`/`-MMD`** list the entry's `std.pcm`, and `-MD` also lists the
  libc++ files it was built from.
- **Removing it.** kiln never evicts entries. Delete the directory by
  hand (`rm -r ~/.cache/kiln/modules`); each entry is about 9 MiB.

A cached `std` costs about what building it in the compile costs, 1.5 s
and about 96 MiB for `import std;` plus a small `main`. The compile that
fills the cache takes about 2.5 s and 140 MiB (aarch64, spec/modules.md
§11.12 "Cost"). Module files hold sources, so importing one re-parses
them ([KNOWN_ISSUES.md](KNOWN_ISSUES.md#modules)).

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
lists the common flags by group and marks the kiln-only ones `[kiln]`.
The full list is spec/driver.md, "Options".

### kiln-only flags

| Flag | Effect |
|---|---|
| `-fconstexpr-memory=N[k\|m\|g]` | Oven memory budget (above) |
| `-fkiln-stack-size=N[k\|m\|g]` | compiling thread's stack (above) |
| `-fclang-compat=19\|22` | which clang release's behaviour to reproduce (default 22) |
| `--llvm-dir=<dir>` | the LLVM installation to use (see [Run-time dependencies](#run-time-dependencies)) |
| `-fkiln-reflection-mangling=kiln\|gcc\|kiln-hashed` | mangling of reflection template arguments |
| `-fkiln-instantiation-report[=all\|json\|text]`, `-fkiln-memory-report` | reports (above) |
| `-f[no-]kiln-module-cache` | use the `std` module cache (default on; see [The `std` module cache](#the-std-module-cache)) |
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
- The PGO flags are rejected (see [Coverage](#coverage)).
- `--target=` accepts only aarch64 and x86_64 Linux triples.
- `-x c` and `.c` inputs can only be preprocessed. `kiln++` warns about a
  `.c` input and compiles it as C++, as `clang++` does.
- The full list of intentional differences is spec/driver.md,
  "Deviations from clang 19".

## Environment variables

| Variable | Effect |
|---|---|
| `KILN_LLVM_DIR` | LLVM installation to use, if `--llvm-dir` is not given; otherwise kiln searches (see [Run-time dependencies](#run-time-dependencies)) |
| `KILN_RESOURCE_DIR` | resource directory (builtin headers, predefines), if `-resource-dir` is not given |
| `KILN_REPRODUCIBLE_TIME`, `SOURCE_DATE_EPOCH` | fixed values for `__DATE__`, `__TIME__`, `__TIMESTAMP__` (spec/driver.md, "Reproducible time") |
| `XDG_CACHE_HOME`, `HOME` | where the `std` module cache lives when `-fmodules-cache-path=` is not given (see [The `std` module cache](#the-std-module-cache)) |
| `TMPDIR` | where crash reproducers go when `-fcrash-diagnostics-dir` is not given (default `/tmp`) |
| `KILN_BACKTRACE=1` (or `RUST_BACKTRACE=1`) | add kiln's own backtrace to a crash report |
| `LLVM_PROFILE_FILE` | at run time, where an `-fprofile-instr-generate` program writes its profile (LLVM's runtime) |

Packagers can set `KILN_DEFAULT_LLVM_DIR` and `KILN_DEFAULT_GCC` when
building kiln, to compile in an LLVM directory to try before the search
or a GCC driver outside `/usr` (spec/driver.md, "LLVM discovery" and
"Toolchain").

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
safe-stack, kcfi, memtag, ...) are rejected. `-fsanitize-ignorelist=`,
`-ftrapv` and `-fsanitize-merge=` work. A few `undefined` checks are not
done yet; see [KNOWN_ISSUES.md](KNOWN_ISSUES.md#sanitizers-and-coverage).

### Coverage

- `--coverage` (gcov line coverage) works. `llvm-cov gcov` reports the
  same line counts as for clang 19.
- `-fprofile-instr-generate[=<file>]` (source-based coverage counters)
  works. `llvm-profdata show --counts` matches clang 19.
- `-fprofile-instr-generate -fcoverage-mapping` (source-based coverage
  with `llvm-cov show`/`report`/`export`) works as with clang:

  ```sh
  kiln++ -fprofile-instr-generate -fcoverage-mapping -O0 app.cpp -o app
  LLVM_PROFILE_FILE=app.profraw ./app
  llvm-profdata merge -o app.profdata app.profraw
  llvm-cov report ./app -instr-profile=app.profdata
  llvm-cov show ./app -instr-profile=app.profdata
  ```

  The mapping is clang 19's in every `-fclang-compat` mode; use the
  `llvm-profdata`/`llvm-cov` of the LLVM you link with.
  `-fcoverage-compilation-dir=` and `-fcoverage-prefix-map=` behave as in
  clang. Unused inline and static functions show as 0%.
- Profile-guided optimization (`-fprofile-generate`, `-fprofile-use`,
  `-fprofile-instr-use`, `-fprofile-sample-use`, `-fcs-profile-generate`)
  is not part of 1.0 and is rejected with an error that says so.

Both coverage modes link LLVM's profile runtime (Debian:
`libclang-rt-<N>-dev`).
See [COMPATIBILITY.md](COMPATIBILITY.md#coverage-and-profiling).

## Targets and LLVM versions

- **Targets:** `aarch64-unknown-linux-gnu` and `x86_64-unknown-linux-gnu`
  (and their clang aliases). The default is the host. Any other `--target`
  is `unsupported target`. `-march=`, `-mcpu=`, AArch64's feature flags
  and x86 `-m<feature>` flags such as `-mavx2` work. x86 function
  multiversioning and a few x86 code-generation flags do not yet; see
  [KNOWN_ISSUES.md](KNOWN_ISSUES.md#platforms).
- **LLVM:** 22 (preferred when both are installed) and 19, chosen as in
  [Run-time dependencies](#run-time-dependencies). The LLVM major version
  selects the IR dialect kiln writes. It does not change the language:
  language and macro behaviour follow `-fclang-compat`, which defaults to
  22 whichever LLVM is used. LTO (`-flto`, `-flto=thin`) works with the
  selected LLVM.

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
a crash), and 70 after a crash under `-fno-crash-diagnostics` or when
kiln cannot create its compile thread (lower `-fkiln-stack-size=`).

If a crash looks security-relevant, for example it is triggered by
untrusted input in a service that compiles it, follow
[SECURITY.md](../SECURITY.md) instead of filing a public issue.
