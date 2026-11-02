# spec: kiln_driver — the `kiln` and `kiln++` command lines

`kiln` is a clang-compatible driver. Like clang and clang++ it is one
driver under two names: `kiln++` compiles every input as C++, `kiln` picks
the language by extension (see "Driver modes"). C++ is kiln's language, so
`kiln++` is the name to use; xtask, the docs and the examples here invoke
it. A command line that clang 19 accepts
for preprocessing means the same thing to kiln. An option kiln does not
implement is an error, never silently ignored. The oracle is Debian's
`clang++-19` / `clang-19` on aarch64 and x86_64 Linux.

Three actions are implemented:

- preprocessing: `-E`, `-M`, `-MM`, and `-MD`/`-MMD` alongside `-E`;
- `-fsyntax-only [-fdump-ast] [-fdump-record-layouts]
  [-fdump-vtable-layouts]`, which runs
  WP-048's parser and Sema;
- code generation (WP-052b): `-S -emit-llvm`, `-c -emit-llvm`, `-S`, `-c`
  and linking (see "Code generation").

Crate layout (each part is replaceable on its own):

| module | job |
|---|---|
| `args` | parse argv into `Args`: no file system, no diagnostics |
| `toolchain` | target, `-std=`, resource dir, GCC/libc++ detection, the system search directories |
| `pp_cmd` | one `-E` job: predefines, search list, preprocessor (WP-015's `IfEval` and `ClangFeatures`), printer, dependency list |
| `syntax_cmd` | one `-fsyntax-only` job: parser, Sema, record-layout dump, AST dump |
| `codegen_cmd` | the codegen path: phases, output names, the step plan, `-###`, the in-process frontend (parser, Sema, `kiln_codegen::emit`) |
| `link` | the external commands (`llc`, `llvm-as`, `clang++`): rendering and running them |
| `constexpr_opts` | the `-fconstexpr-*` flags: Oven limits for the parser's Sema, the report |
| `deps` | Make-style dependency output |
| `time` | reproducible `__DATE__`/`__TIME__`/`__TIMESTAMP__` |
| `lib` (`run`, `run_as`) | orchestration, driver diagnostics, outputs, exit status |
| `main` | the `kiln` binary: `run_as(argv[0], ...)` on a big-stack thread |

## Options

Options are parsed the way clang parses them. A valued option takes its
value joined (`-Ifoo`, `-std=c++20`, `--target=x`) or as the next argument
(`-I foo`, `-target x`). `-std`, `--sysroot`, `-resource-dir`, and
`-target`/`--target` also accept `=`. A lone `-` and any argument not
starting with `-` are inputs.

| option | effect |
|---|---|
| `-E` | preprocess to standard output (or `-o`) |
| `-P` | no line markers |
| `-C`, `-CC` | keep comments (`-CC`: also in macro expansions) |
| `-dM` | print the macro table at the end instead of the text |
| `-o <file>` | output file (`-` is standard output) |
| `--driver-mode=g++`, `--driver-mode=gcc` | hidden (not in `--help`): the driver mode, overriding the program name; the last wins, wherever it is (see "Driver modes") |
| `-x <lang>` | language of the following inputs: `c`, `c-header`, `cpp-output` -> C; `c++`, `c++-header`, `c++-cpp-output`, `c++-module` -> C++; `none` resets |
| `-std=<std>` | `c++98`..`c++26`, `gnu++NN`, the `0x/1y/1z/2a/2b/2c` aliases; C: `c89/c99/c11/c17/c23`, `gnuNN`, `iso9899:*` |
| `-D`, `-U` | command-line macros, in order |
| `-I`, `-iquote`, `-isystem`, `-idirafter`, `-cxx-isystem` | user search directories; `-I=dir` is relative to the sysroot |
| `-nostdinc`, `-nostdinc++`, `-nostdlibinc`, `-nobuiltininc` | drop toolchain directories (see "Toolchain") |
| `--sysroot`, `-isysroot` | sysroot (`-isysroot` wins for headers) |
| `-resource-dir <dir>` | the builtin headers are in `<dir>/include` |
| `-stdlib=libstdc++` / `libc++` | C++ standard library headers (default libstdc++) |
| `--target=<triple>` | `aarch64`/`arm64` or `x86_64`/`amd64` Linux; anything else is an error |
| `--embed-dir=<dir>` | `#embed` search directory |
| `-M`, `-MM`, `-MD`, `-MMD`, `-MF`, `-MT`, `-MQ`, `-MP` | dependency output (see below) |
| `-trigraphs`, `-ftrigraphs`, `-fno-trigraphs` | trigraphs (default: on only for strict C and C++ before 17) |
| `-f[no-]dollars-in-identifiers` | `$` in identifiers (default on) |
| `-O*` | also selects `llc`'s level and the `opt` pipeline, and is passed to the link (see "Code generation") |
| `-disable-llvm-passes` (usually after `-Xclang`) | no `opt` run: `-emit-llvm` writes the frontend's IR (see "IR optimization") |
| `-O*`, `-f[no-]exceptions`, `-f[no-]cxx-exceptions`, `-f[no-]rtti`, `-f[no-]char8_t`, `-f[no-]pic/PIC/pie/PIE`, `-f[no-]fast-math`, `-ffreestanding`, `-fhosted`, `-pthread`, `-f[no-]threadsafe-statics`, `-f[no-]sized-deallocation` | change predefined macros only (see "Predefines") |
| `-fsigned-char`, `-funsigned-char` (`-fno-unsigned-char`, `-fno-signed-char` are their synonyms; the last one wins) | the signedness of plain `char` ([basic.fundamental]/7), overriding the target's (signed on x86-64, unsigned on AArch64): it sets `TargetTypes::char_signed` of the `kiln_abi::Target` the parser, sema, the Oven and codegen use, the `#if` evaluator's character type ([cpp.cond]), and adds or removes `__CHAR_UNSIGNED__` as clang 19 does (see "Predefines") |
| `-fclang-compat=<major>` | kiln: which clang release's observable output to reproduce, `19` (default) or `22`: predefined macros and `__has_*` answers (spec/pp.md "clang compatibility level"), diagnostic text and names, AST dump types and deprecation warnings (spec/sema.md §1.1, spec/ast-dump.md §2, spec/codegen.md); any other value is an error |
| `-W...`, `-w`, `-pedantic`, `-ferror-limit=`, `-fdiagnostics-format=`, `-f[no-]color-diagnostics`, `-f[no-]caret-diagnostics`, ... | diagnostic options, validated and applied by `kiln_diag::DiagOptions` (`spec/diag.md`) |
| `-fkiln-reproducible-time` | see "Reproducible time" |
| `-fconstexpr-steps=N`, `-fconstexpr-depth=N`, `-fconstexpr-backtrace-limit=N` | constant-evaluation limits, as clang's (defaults 1048576, 512, 10; a backtrace limit of 0 shows every frame); `constexpr_opts`, spec/oven.md "Integration" |
| `-fconstexpr-memory=N[k\|m\|g]` | kiln: live abstract-memory bytes per evaluation (default 256 MiB; binary suffixes) |
| `-f[no-]reflection` | kiln: the P2996 reflection spike (WP-267; default off; the last one wins): `^^`, `[:`, `:]` are lexed (C++26 only), and `^^`, type splices and the kiln intrinsics are accepted (spec/sema.md §8a, spec/parse.md §5.2). `__cpp_reflection` is not predefined. Passed to the preprocessor, the parser and sema for `-E`, `-fsyntax-only` and codegen jobs |
| `-f[no-]coro-aligned-allocation` | aligned coroutine frame allocation (default off, as clang; the last one wins): sema looks up the `std::align_val_t` forms of the frame's `operator new`/`delete` first (spec/sema.md §7.13). Passed to sema for `-fsyntax-only` and codegen jobs |
| `-fconstexpr-report=<path>` | kiln: write the per-TU NDJSON constant-evaluation report (spec/oven.md "Reporting protocol"); `-fsyntax-only` and every codegen job |
| `-ftemplate-depth=N`, `-ftemplate-backtrace-limit=N`, `-fkiln-instantiation-report[=all\|json\|text]` | template limits and the instantiation report (spec/sema-templates.md §10.3, §11.3) |
| `-fkiln-stack-size=N[k\|m\|g]` | kiln: the stack reserved for the compiling thread (default 1 GiB; at least 4 MiB; the last valid value wins). Running out is the fatal "kiln: stack exhausted during …" (spec/sema-templates.md §10.3), never a crash |
| `-v`, `--verbose`, `--version`, `--help` | see below |
| `-F <dir>` | accepted; warns `ignoring framework directory` |
| `-fsyntax-only` | parse and check (see "Syntax checking") |
| `-verify[=<prefixes>]`, `-verify-ignore-unexpected[=<levels>]` | with `-fsyntax-only`: check the diagnostics against `expected-*` comments (see "-verify") |
| `-Xclang <arg>` | `<arg>` is parsed as a driver option (kiln has no separate `cc1`), so `-Xclang -verify` is `-verify` |
| `-fdump-ast[=ids,locs,canonical,implicit,all-files]`, `-fdump-ast-filter=<s>` | with `-fsyntax-only`: the AST dump (`spec/ast-dump.md`) to the output |
| `-fdump-record-layouts`, `-fdump-record-layouts-complete` | with `-fsyntax-only`: the record layouts (`spec/abi.md` §6, §8.4) to the output |
| `-fdump-vtable-layouts` | with `-fsyntax-only`: the vtable groups (`spec/abi.md` §8.5) to the output |
| `-c`, `-S`, `-emit-llvm` | code generation (see "Code generation") |
| `-###` | with a codegen action: print the steps instead of running them |
| `-moutline-atomics`, `-mno-outline-atomics` | AArch64: `+outline-atomics` (the default, as clang 19 on an AArch64 Linux host with GCC >= 9.3.1) or `-outline-atomics` in every function's `target-features` (spec/codegen.md); the last wins. Ignored on x86-64 with clang's warning `'x86_64' does not support '<last spelling>'; flag ignored [-Woption-ignored]`, once per source input (clang warns once per compile job) and in every mode; `-Werror` makes it an error and no job runs (WP-228) |
| `-fcx-limited-range`, `-fno-cx-limited-range`, `-fcx-fortran-rules`, `-fno-cx-fortran-rules`, `-fcomplex-arithmetic=full\|improved\|basic` | the `_Complex` multiply/divide algorithm (`CodegenOptions::complex_range`, spec/codegen.md §7n): basic, full, improved, full, or the named one; the last of these and `-ffast-math` (basic) wins, and without any it is full, or basic under `-Ofast` unless `-fno-fast-math`. `-fno-fast-math` leaves it alone (clang 19 likewise). `-fcomplex-arithmetic=promoted` and unknown values are `unsupported argument '<v>' to option '-fcomplex-arithmetic='`. clang's `-Woverriding-option` between these flags is not issued (WP-237) |
| `-g`, `-g0`..`-g3`, `-ggdb`, `-ggdb0`..`-ggdb3` | accepted and ignored (no debug info yet) |
| `-l<lib>`, `-L<dir>`, `-Wl,<args>`, `-fuse-ld=<ld>`, `-static`, `-nostdlib`, `-shared`, `-rdynamic`, `-pie`, `-no-pie` | passed to the link command, in command-line order relative to the inputs |
| `-dD`, `-dI` | parsed; "not implemented yet" (WP-017) |
| `-fmodule-file=<name>=<path>`, `-fprebuilt-module-path=<dir>` | C++20 modules: where module interface sources are found (see "Modules"); `-fmodule-file=<path>` without a name is accepted and ignored |

## Driver modes

One binary, two names, as clang/clang++ (WP-244). Cargo cannot build a
binary target named `kiln++` (a target name must be a crate name), so
`kiln_driver`'s build script makes `kiln++` a relative symlink to `kiln`
in the profile directory (`target/<profile>/kiln++ -> kiln`); xtask's
`build_kiln` makes it again if it is missing and returns its path.

The mode comes from the program name (`argv[0]`), as clang's driver name
parsing: take the basename, drop a `.exe`; if that does not end in `++`,
also drop a trailing version (`[0-9.]*` and then one `-`: `kiln++-0.1`,
`clang++-19`) and look again. A name ending in `++` (`kiln++`, `c++`) is
C++ mode (clang's `g++` mode); every other name is `kiln` mode (clang's
`gcc` mode). The hidden
`--driver-mode=g++|gcc` overrides the name (the last one wins);
any other value is the error `unsupported argument '<v>' to option
'--driver-mode='`. `kiln_driver::run` runs as `kiln++`;
`kiln_driver::run_as(argv0, ...)` takes the name.

The program's basename, as invoked, prefixes the driver diagnostics
(`kiln++: error: no input files`, `kiln: warning: ...`), as clang prints
`clang++-19: error:`; `--driver-mode=` does not change it. `--version`
prints the same text under both names.

## Input languages

An input's language is its `-x` (the last one before it; `-x none`
resets), else its extension and the mode. Extensions are case-sensitive,
as in clang's table:

| extension | clang's type | `kiln` | `kiln++` |
|---|---|---|---|
| `.c` | `c` | C | C++, with the warning below |
| `.h` | `c-header` | C | C++ (`c++-header`), with the warning |
| `.i` | `cpp-output` | C | C++ (`c++-cpp-output`), with the warning |
| `.cc`, `.cp`, `.cpp`, `.CPP`, `.cxx`, `.c++`, `.C`, `.ii`, `.cppm`, `.ixx`, `.hh`, `.hpp`, `.hxx`, `.H`, `.inc` | C++ types | C++ | C++ |
| standard input (`-`) | | C | C++ |
| anything else | | C++ for `-E`/`-fsyntax-only`; a linker input for codegen | same |

In C++ mode a `.c`, `.h` or `.i` input without `-x` gets clang++'s
driver warning, once per input, before any job runs:
`treating '<c type>' input as '<c++ type>' when in C++ mode, this
behavior is deprecated [-Wdeprecated]`. The `-W` options apply to it
(`-w` and `-Wno-deprecated` silence it); under `-Werror` it is an error
and no job runs. C is preprocessed (`-E`, deviation 1); compiling it
(`-fsyntax-only`, codegen) is "not implemented yet" (WP-243).

## Toolchain

`Toolchain::new(target, sysroot, resource_dir)` locates:

- **Resource directory:** `-resource-dir`, else `$KILN_RESOURCE_DIR`, else
  the nearest `resource/` with a `predefines/` directory above the
  executable, else the source tree's `resource/`. kiln's predefines base
  files are `<resource>/predefines/<triple>.<std>.txt`.
- **LLVM:** `$KILN_LLVM_DIR`, default `/usr/lib/llvm-19`. This is where
  libc++'s headers are found. Its major version (`Toolchain::llvm_major`:
  the leading integer of `<dir>/bin/llvm-config --version`, probed once
  per directory per process; if that tool is missing or fails, the `N` of
  a directory named `llvm-N`, after resolving symlinks; else 19) selects
  the IR that differs between LLVM 19 and 22 (spec/codegen.md,
  coroutines and §6). A tree whose name says nothing about its version
  (`/opt/llvm`) is keyed correctly by the probe.
- **GCC:** clang `GCCInstallationDetector`. The newest
  `<sysroot>/usr/lib/gcc/<triple>/<version>` that contains `crtbegin.o`,
  over the target's candidate triples in clang's order.

The search list is the user's directories in command-line order, then the
toolchain's (clang 19 `Linux::AddClangCXXStdlibIncludeArgs` followed by
`AddClangSystemIncludeArgs`):

1. C++ only, unless `-nostdinc++` or `-nostdlibinc`:
   - **libstdc++:** `<gcc>/../../../../include/c++/<v>`, then
     `.../<v>/<triple>`, `.../<v>/<triple>/<multilib>`, and `.../backward`.
   - **libc++:** the first of `<llvm>/bin/../include`,
     `<sysroot>/usr/local/include` or `<sysroot>/usr/include` that has a
     `c++/v1`. Its per-target `<triple>/c++/v1` comes first if it exists.
2. Unless `-nobuiltininc`: `<resource>/include`.
3. Unless `-nostdlibinc`:
   - `<sysroot>/usr/local/include`;
   - `<gcc>/../../../../<triple>/include`;
   - then extern-C system directories (line-marker flags `3 4`):
     `<sysroot>/usr/include/<multiarch>` (if it exists),
     `<sysroot>/include`, and `<sysroot>/usr/include`.

`-nostdinc` drops all of them. `-cxx-isystem` applies only to C++. The
list is realized by `kiln_pp::SearchList::build` (`spec/pp.md` "Header
search"), which drops nonexistent and duplicate directories.

## Predefines

The `<built-in>` buffer is `kiln_pp::build_predefines(base, cfg, -D/-U)`.

- `base` is the clang 19 `-dM` output for the target and strict standard,
  recorded in `resource/predefines/`.
- `cfg` applies the measured flag effects (`resource/README.md`):
  - the GNU dialect;
  - `-O` levels;
  - exceptions and RTTI;
  - char8_t;
  - plain `char` signedness (`__CHAR_UNSIGNED__`);
  - PIC/PIE (Debian default PIE 2);
  - fast-math;
  - freestanding;
  - `-pthread`;
  - thread-safe statics;
  - sized deallocation;
  - `-stdlib=libc++`: no `__STDCPP_FLOAT16_T__` ... `__STDCPP_BFLOAT16_T__`
    (`std_float_aliases`; libc++ has no `<stdfloat>`, spec/pp.md P1467
    macros).
- kiln adds `__KILN__ 1`, `__KILN_MAJOR__` and `__KILN_MINOR__`. kiln
  defines `__clang__` and the clang version macros exactly as clang 19 does,
  so headers take their clang paths.

The layout follows clang:

```
# 1 "<built-in>" 3
#define ... (sorted)
# 1 "<command line>" 1
-D / -U in command-line order
#define __GCC_HAVE_DWARF2_CFI_ASM 1
# 1 "<built-in>" 2
```

## `#if` and feature queries (WP-015)

- Each `-E` job installs `IfEval::for_triple(<triple>)` for the target's
  character types.
- It also installs `ClangFeatures::from_args(<triple>, <std>, argv)`,
  clang 19's tables for `__has_feature`, `__has_extension`,
  `__has_builtin`, `__has_attribute`, `__has_cpp_attribute`,
  `__has_declspec_attribute`, `__is_identifier` and the target queries.
  - `from_args` reads the `-f` options that change answers (exceptions,
    RTTI, char8_t, sanitizers, ...) from the command line.
  - The driver appends the effective dialect (`-std=gnu++` or `-std=c++`)
    so that clang's default GNU mode applies when no `-std=` is given.
  - A target the tables do not cover is a driver error, never a silent
    fallback to `NoFeatures`. This cannot happen today: `Target` has only
    covered triples.

## Syntax checking (`-fsyntax-only`)

Each input is preprocessed and parsed in one pass. The driver builds the
preprocessor exactly as `-E` does: `Job::pp_inputs` (search list, `-v`
block, main file, predefines with `-D`/`-U`/`-include`) and then
`Job::build_preprocessor` (header search, `#if` evaluation, feature
queries, callbacks, entering the main file). `-E` and `-fsyntax-only`
share both functions, so macros, `#include` and `#if` behave identically
in the two modes. The parser then reads the preprocessor's tokens through
`kiln_parse::parse_preprocessed` (spec/parse.md §2) at the `-std=`
standard (default `gnu++17`, as clang; `cargo xtask test` always passes
`-std=`). The target is `kiln_sema::target::Target::from_triple`. The
output and status follow the `-E` rules:

- diagnostics, then the error summary, go to standard error;
- `-fdump-ast` writes the dump to the output. It covers the main file's
  declarations; `-fdump-ast=all-files` adds those of included files;
- `-fdump-record-layouts` and `-fdump-record-layouts-complete` (clang
  spells the second `-Xclang -fdump-record-layouts-complete`; kiln
  accepts it directly) are the same flag. They write
  `kiln_abi::dump_all_record_layouts` for the `-target` triple to the
  output, before any `-fdump-ast` dump: every complete, valid,
  non-template class of the translation unit, in clang's `-complete`
  order. Unnamed classes print their `at F:L:C` location. The dump is
  written even when there are errors;
- `-fdump-vtable-layouts` (clang: `-Xclang -fdump-vtable-layouts`)
  writes `kiln_abi::vtable::dump_all_vtable_layouts` for the `-target`
  triple to the output, after the record layouts and before any
  `-fdump-ast` dump: the vtable group (`dump_vtable_group`: the vtable,
  its construction vtables, thunks, vbase offset offsets and method
  indices) of every complete, valid, non-template dynamic class, in the
  record-layout dump's order (`kiln_abi::record_definitions`). Classes kiln cannot lay out are skipped. Written even
  when there are errors;
- the parser's Sema evaluates constants under the `-fconstexpr-*`
  limits; after the parse `-fconstexpr-report=` writes the Oven's report
  (`constexpr_opts`);
- the exit status is 1 on any error.

An unknown `-fdump-ast` option gives `invalid value '<opt>' in
'-fdump-ast=<opts>'`.

C inputs (`-x c`, `.c`), `-MD`/`-MMD` with `-fsyntax-only`, and
`-fdump-ast`, `-fdump-record-layouts`, `-fdump-vtable-layouts` or
`-verify` without `-fsyntax-only` are "not implemented yet".

## `-verify`

`-verify` is clang's `-cc1 -verify` (WP-143). The consumer, its
directive grammar, the matching rules and the report are spec/diag.md
§11. This section covers the options and the driver's part.

- **Options.** These follow clang's `ParseDiagnosticArgs`.
  - `-verify` adds the prefix `expected`.
  - `-verify=<p>[,<p>...]` adds each listed prefix. The option may be
    repeated, and the prefixes accumulate.
  - All prefixes are then validated, sorted and deduplicated. An
    invalid one gives `invalid value '<p>' in '-verify='` plus the note
    `-verify prefixes must start with a letter and contain only
    alphanumeric characters, hyphens, and underscores`, and turns
    verify off.
  - `-verify-ignore-unexpected` ignores unexpected diagnostics of every
    level. `=<levels>` (comma-separated `note`, `remark`, `warning`,
    `error`) ignores only those. Any other value gives
    `invalid value '<v>' in '-verify-ignore-unexpected='`. These errors
    come after the prefix errors.
  - `-Xclang <arg>` forwards `<arg>` to the same parser. A trailing
    `-Xclang` is `argument to '-Xclang' is missing`.
- **Run.**
  - Each input is parsed as for `-fsyntax-only`, with diagnostics going
    to a buffer instead of stderr.
  - The preprocessor's comment handler feeds every lexed comment to the
    verifier.
  - `@<file>:N` resolves `<file>` like a quoted include: an absolute
    path as is; otherwise relative to the directive's file, then the
    search directories in order, then relative to the working directory.
  - After the parse the verifier reports marker errors and checks the
    buffer.
  - Its report goes to stderr in the selected diagnostic format. In the
    text format it is followed by `N error[s] generated.`.
- **Exit status.** 1 if the report has any problem, else 0, whatever
  the matched diagnostics were. The error limit (`-ferror-limit`)
  still applies to the buffered diagnostics, as it does in clang;
  libc++ passes `-ferror-limit=0`.

## Code generation

`codegen_cmd::compile` runs when the action is `-c`, `-S`, `-emit-llvm`
or linking (no action option). The phase is:

| options | phase | output (default name) |
|---|---|---|
| `-S -emit-llvm` | textual IR, written by kiln | `<stem>.ll` |
| `-c -emit-llvm` | bitcode, through `llvm-as` | `<stem>.bc` |
| `-S` | assembly, through `llc` | `<stem>.s` |
| `-c` | object, through `llc -filetype=obj` | `<stem>.o` |
| none | objects in a temporary directory, then `clang++` links | `a.out` |

`-S` wins over `-c`, as in clang. `<stem>` is the input's file stem in
the current directory (standard input is `-`). `-o -` is standard output.

- **Inputs.** A source (`.c .i .cc .cp .cpp .CPP .cxx .c++ .C .ii .cppm
  .ixx .h .hh .hpp .hxx .H .inc`, any input after `-x`, and `-`) is
  compiled; anything else is a linker input. A C source reports
  `compiling C is not implemented yet`.
- **Frontend.** The preprocessor is built as for `-E`
  (`Job::build_preprocessor`), then `parse_preprocessed`, then
  `kiln_codegen::emit_with_diags` (spec/codegen.md) with the target's
  data layout, `source_filename` = the input path, and `!llvm.ident` =
  `kiln version <v>`. The warnings codegen returns (`-Watomic-alignment`)
  are emitted first, in order, under the usual policy (`-Werror` makes
  them errors, failing the job); then a `CodegenError` is reported at its
  location as an error (`cg_error`). The `-fconstexpr-report` is written for every job.
  With an error, nothing is written to the output and no further step of
  that input runs; other inputs still compile; the link does not run.
- **Tools.** Each tool is `$KILN_LLVM_DIR/bin/<tool>` (default
  `/usr/lib/llvm-19`), else the versioned name (`llc-19`, `llvm-as-19`,
  `clang++-19`) on `PATH`. The IR is piped to the tool's standard input
  (`-o <out> -`).
- **`llc`** gets `-O0` (default), `-O1` (`-O`, `-O1`, `-Og`), `-O2`
  (`-O2`, `-Os`, `-Oz`) or `-O3` (`-O3`, `-O4`, `-Ofast`), then
  `-relocation-model=static` with `-fno-pic`/`-fno-pie`, else
  `-relocation-model=pic` (clang's default PIE), then `-filetype=obj`
  unless `-S`.
- **IR optimization (WP-351, WP-370).** `llc` runs no IR pass that
  splits coroutines, and kiln emits unoptimized IR, so before the IR is
  written (`-S -emit-llvm`, `-c -emit-llvm`) or piped to `llc` it goes
  through `opt` (found like the other tools, `opt-19`):
  - at `-O1` and above, `-passes=default<O1|O2|O3|Os|Oz>` (`-O`/`-Og`
    give O1; `-O4`/`-Ofast` O3): clang's default pipeline, which runs
    CoroEarly, CoroSplit, CoroElide and CoroCleanup in their standard
    positions, so a coroutine whose frame does not escape its caller is
    allocated on the caller's stack (`coro_elide_at_o2` in
    `crates/kiln_driver/tests/codegen.rs`: no `operator new` call and no
    `llvm.coro.*` call left, as clang 19 `-O2`). Codegen drops the
    `noinline optnone` pair at these levels (spec/codegen.md §7p);
  - at `-O0`, only when the IR declares an `llvm.coro.*` intrinsic (a
    coroutine's `presplitcoroutine` body or a lone
    `__builtin_coro_noop`; clang's `CoroConditionalWrapper` test
    `coro::declaresAnyIntrinsic`),
    `-passes=coro-early,cgscc(coro-split),coro-cleanup,globaldce` (the
    coroutine part of clang 19's O0 pipeline); any other `-O0` module
    takes the unchanged `llc`-only path.

  The pass run writes bitcode, which a second `opt -discard-value-names
  -S` reads back so the local value names the passes made are dropped,
  as in clang's release pipeline (`opt` refuses `-discard-value-names`
  for textual input). `-disable-llvm-passes` (a cc1 option, usually
  spelled `-Xclang -disable-llvm-passes`) skips both runs, so
  `-emit-llvm` writes the frontend's IR (pre-split coroutines). `-v`
  prints the `opt` commands before the `llc` one; `-###` does not show
  them (they depend on the generated IR). An `opt` failure is a driver
  error like `llc`'s. Cost (WP-351, aarch64, LLVM 19, `-O0`): on
  libc++'s `end.to.end/generator.pass.cpp` (4.8k lines of IR) `opt`
  takes 0.02 s and 62 MB RSS, against 2.9 s for the whole debug-build
  `kiln++ -c`; `go.pass.cpp` 0.01 s.
- **Link.** `clang++` with `--target=<triple>` (only if given),
  `--sysroot=`, the `-O` flag, `-pthread`, `-stdlib=`, then the objects,
  linker inputs and link options in command-line order, then
  `-o <out>` (default `a.out`). The temporary objects are removed
  afterwards (by their directory's explicit path).
- **`-###`** prints, to standard error, the `--version` block, then one
  line per step. An external step is clang's format: a leading space, then
  every argument quoted (`"`, `\` and `$` escaped). The frontend step is
  ` (in-process) frontend "<input>" -> "<out>"`, where `<out>` is `|` when
  the IR is piped to the next step. Nothing runs; the exit status is 0.
- **`-v`** prints the `--version` block and each external command before
  running it, quoted only where needed (`llvm::sys::printArg`).
- **Unused inputs.** In a compile-only phase, linker inputs and link
  options are unused, warned in command-line order with clang's wording
  (`-Wunused-command-line-argument`): `<x>: 'linker' input unused` for
  linker inputs, `-l` and `-Wl,`; `argument unused during compilation:
  '<o>'` for `-L`, `-fuse-ld=`, `-shared`, `-rdynamic`, `-pie`, `-no-pie`.
  `-static` and `-nostdlib` are silent, as in clang.
- **Errors.** `-o` with several sources in a compile-only phase is an
  error, as is `-emit-llvm` when linking. A tool that is missing, cannot
  start, fails or is killed is a driver error (below); the exit status is
  then 1.

Not implemented: `-MD`/`-MMD` when compiling ("not implemented yet"),
`-###` with `-E` or `-fsyntax-only` (likewise), and debug information.

## Modules

C++20 modules are enabled for `-fsyntax-only` and code generation from
`-std=c++20` on; `-E` and `-M*` print module and import directives as text
(spec/modules.md §2). There is no binary module interface: an imported
module is built in-process from its interface source, so the flags name
sources, not BMIs. A module (or partition `M:P`) is looked up in this
order:

1. `-fmodule-file=<name>=<path>` (repeatable; the first one naming the
   module wins). The path is used as given (relative to the working
   directory).
2. `-fprebuilt-module-path=<dir>` (repeatable, in order): `<dir>/<name>.cppm`,
   `.ixx`, then `.cpp`; a partition `M:P` as `<dir>/M-P.*`.
3. `std` and `std.compat` only: `<llvm>/share/libc++/v1/std.cppm` /
   `std.compat.cppm`, where `<llvm>` is the toolchain's LLVM directory
   (`KILN_LLVM_DIR`, default `/usr/lib/llvm-19`; clang 22's libc++ has the
   same layout).

A module not found is `fatal error: module 'M' not found` at the module
name. Header units (`import "h";`, `import <h>;`) are found like
`#include` and need no flag.

To link a program, every module unit whose non-inline definitions are used
is also compiled as an input of the same command (or separately with
`-c`): `kiln++ -std=c++20 -fmodule-file=m=m.cppm m.cppm main.cpp`. An
importer emits only declarations for the non-inline functions and
variables of the units it imports. `--precompile`, `-fmodule-output` and
`-fmodule-header` are not supported (part B, spec/modules.md §9).

## Dependency output

This is clang's `DependencyCollector` and `DependencyFileGenerator`.

- `-M`/`-MM` print the rule instead of the text and imply `-w`. Their
  output goes to `-MF`, else `-o`, else standard output.
- `-MD`/`-MMD` preprocess normally and write the rule to `-MF`, else to
  `-o` with the extension replaced by `.d`, else to `<input stem>.d`.
- `-MM`/`-MMD` leave out system headers.
- Files are listed in first-seen order:
  - the main file;
  - every entered header;
  - every header found by `__has_include`;
  - every `#embed` resource.
  `<built-in>`, `<command line>` and `<stdin>` are left out, and `./` is
  removed.
- The target is `-MT` (verbatim) or `-MQ` (Make-quoted), in order. Without
  either it is `-o` (for `-MD`) or `<input stem>.o`.
- Lines wrap at 75 columns with ` \`. `-MP` adds a phony target for every
  dependency except the main file.
- Dependency files are written only when there were no errors.

## Output, diagnostics, exit status

- Driver diagnostics (bad arguments, missing inputs) are prefixed with the
  program's basename (`kiln++: `, `kiln: `) as clang prefixes `clang++: `. They are reported before any input is
  processed, and any one of them stops the run with status 1.
- Each input gets its own `SourceManager` and diagnostic engine. In text
  format a job prints clang's `N warning(s) and M error(s) generated.`
  summary after its diagnostics.
- With several inputs the outputs are concatenated, as clang does with
  `-E`.
- Exit status is 0 on success and 1 if any error was reported (clang's
  convention; `-Werror` promotions count).
- With errors, `-o <file>` is not written and neither are dependency
  files. Standard output still receives the text, as clang does.
- An output that cannot be opened reports
  `unable to open output file '<f>': '<reason>'`.

Driver diagnostics, in clang's wording:

- `unknown argument: '<a>'`
- `argument to '<o>' is missing (expected 1 value)`
- `language not recognized: '<x>'`
- `invalid library name in argument '<a>'`
- `invalid value '<s>' in '-std=<s>'`
- `invalid argument '-std=<s>' not allowed with 'C++'` (or `'C'`)
- `no such file or directory: '<f>'`
- `no input files`
- `unsupported target '<t>'` (kiln only)
- `unsupported argument '<v>' to option '--driver-mode='`
- `<action> is not implemented yet` (kiln only)
- `cannot specify -o when generating multiple output files`
- `-emit-llvm cannot be used when linking`
- `unable to execute command: Executable "<tool>" doesn't exist!`
- `unable to execute command: <program>: <reason>`
- `<backend|linker> command failed with exit code <n> (use -v to see invocation)`
- `<backend|linker> command failed due to signal (use -v to see invocation)`
- warning `<x>: 'linker' input unused [-Wunused-command-line-argument]`
- warning `argument unused during compilation: '<o>' [-Wunused-command-line-argument]`
- warning `'<arch>' does not support '<flag>'; flag ignored [-Woption-ignored]`
- warning `treating '<c type>' input as '<c++ type>' when in C++ mode, this behavior is deprecated [-Wdeprecated]`

## `-v`, `--version`, `--help`

- `--version` prints to standard output:

  ```
  kiln version <v> (C++26 frontend, LLVM IR backend)
  Target: <triple>
  Thread model: posix
  InstalledDir: <dir of the executable>
  ```

- `-v` prints the same to standard error, plus the selected GCC
  installation. For each input it then prints clang's search-list block:
  the `ignoring nonexistent directory` and `ignoring duplicate directory`
  notes, the `#include "..." search starts here:` and
  `#include <...> search starts here:` sections, and
  `End of search list.`. That block is byte-identical to clang's (tested).
  The other `-v` lines are not.
- `-v` or `--version` with no inputs exits 0.
- `--help` prints a short summary of the implemented options and the
  two names' language rule (the usage line says `kiln` under both names).

## Reproducible time

`__DATE__`, `__TIME__` and `__TIMESTAMP__` come from a fixed epoch (UTC)
when one is given:

1. `-fkiln-reproducible-time` uses epoch 0.
2. Else `$KILN_REPRODUCIBLE_TIME`, where a non-number means 0.
3. Else `$SOURCE_DATE_EPOCH`, as clang does.

Otherwise they come from the clock. The test harness always sets
`KILN_REPRODUCIBLE_TIME`.

## Deviations from clang 19

Each deviation is covered by a test or a test note.

1. **C is preprocessed with kiln's C++ lexer** at the closest standard:
   - C89/C99 as C++98;
   - C11/C17 as C++11;
   - C23 as C++17.

   C has no C++ keywords-as-operators and no C++ ud-suffix rules. The pp
   corpus is unaffected, but a C file using `and` as an identifier would
   be.
2. **Driver modes and names.** `--driver-mode=` takes only `g++` and
   `gcc` (clang also takes `cpp`, `cl`, `flang`, `dxc`). `kiln++` is a
   symlink made by the build script, not a second binary target. Standard input is compiled in the mode's
   language with any action, where clang asks for `-E` or `-x` unless
   preprocessing. An input with an unknown extension is C++ for `-E` and
   `-fsyntax-only` (clang takes it as a linker input). Pass `-x c++` to
   compile a C header as C++ without the deprecation warning.
3. **`-dD` and `-dI` are not implemented** (WP-017). Neither is `-MG`
   (unknown argument).
4. **`-F` is ignored** with a warning. kiln does not search frameworks.
5. **No `note: use '...' for '...' standard`** list after an invalid
   `-std=`.
6. **`-v` output differs** except the search-list block. kiln prints no
   `cc1` command line.
7. **No `# N "<built-in>" 3` line** before the main file. clang prints the
   move to the include location of its `<command line>` marker; kiln's
   predefines differ in line count because of the `__KILN__` macros. This
   is text-only: token-wise output is identical.
8. **Only aarch64 and x86_64 Linux targets.**
9. **C feature queries are answered as C++.** `ClangFeatures` has only
   C++ configurations, so a C input gets the answers of its lexer
   standard (deviation 1). For example, `__has_feature(cxx_exceptions)`
   follows `-fexceptions`, where clang answers 0 in C.
10. **No `cc1` frontend command in `-###`/`-v`.** The frontend runs in
    process and prints as ` (in-process) frontend "<in>" -> "<out>"`;
    the backend is a separate `llc` (clang runs it inside `cc1`), and
    the assembler is `llc -filetype=obj`, not `cc1as`.
11. **The IR is optimized by a separate `opt` run** of LLVM's default
    pipeline ("IR optimization" above), not inside the frontend, and
    kiln's unoptimized IR differs from clang's in places (no lifetime
    markers outside coroutines, spec/codegen.md §7p), so `-O2` output
    matches clang's in shape, not bit for bit. The IR-diff corpus runs
    at `-O0`.
12. **`.ll` and `.bc` inputs are linker inputs**, not compiled (clang
    would run its backend on them).
13. **`-MD`/`-MMD` with a codegen action is "not implemented yet"**
    (clang writes the dependency file alongside the object).
14. **No debug information:** `-g*` is accepted and ignored.
15. **`-fdump-record-layouts` dumps every complete class**, as
    `-fdump-record-layouts-complete` does. clang dumps only the classes
    it lays out, in the order it lays them out (spec/abi.md A5).
16. **`-fdump-vtable-layouts` works under `-fsyntax-only`** and dumps
    every dynamic class once, in the record-layout dump's order. clang dumps a vtable
    only when code generation emits it (so never under `-fsyntax-only`),
    in emission order, sometimes repeating a construction vtable, and
    prints its `Original map` debugging sections, which kiln omits.
17. **`-verify` needs `-fsyntax-only`.** clang also verifies a full
    compile. `-Xclang` forwards to the driver parser, not to a `cc1`,
    so only options the driver knows are accepted after it.

## Tests

- `crates/kiln_driver/tests/driver.rs`: argv to exit status, standard
  output, standard error and files, run in-process through
  `kiln_driver::run` with fake streams. Covers:
  - every option;
  - the error summaries and exit codes;
  - `-M*` rules;
  - `-v` against clang's search block;
  - reproducible time;
  - the driver modes (`driver_modes`): the name, `--driver-mode=`, `-x`
    precedence, the C++-mode warning under `-W` options, `kiln` taking
    `.c`/`.h`/`.i`/stdin as C.
- `crates/kiln_driver/tests/verify.rs`: `-verify` end to end under both
  names (`kiln` and `kiln++`): the report pinned against clang 19, and
  compared live with `clang++-19 -Xclang -verify` when it is installed.
  `args.rs` unit tests cover the option semantics.
- `crates/kiln_driver/tests/codegen.rs`: the codegen path end to end
  (`.ll`, `.s`, `.o`, linking and running, output naming, `-###`, `-v`,
  unused-input warnings, missing tools; `kiln_and_kilnpp`: both names as
  real executables, `--version`, `.c` under each). It runs `kiln++`
  (the build script's symlink beside `CARGO_BIN_EXE_kiln`). Steps needing `llc-19`/`clang++-19`
  are skipped without them.
- `crates/kiln_driver/tests/record_layouts.rs`: the `tests/layout/`
  corpus (at least 40 programs). Each is dumped by
  `kiln++ -fdump-record-layouts` and by `clang -Xclang
  -fdump-record-layouts-complete` on both triples, and the dumps are
  compared byte for byte (clang's builtin records are dropped). The test
  also covers both spellings and the "not implemented" error with
  `-S -emit-llvm`.
- `crates/kiln_driver/tests/vtable_layouts.rs`: small programs (every
  dynamic class with an out-of-line key function, no constructors) are
  dumped by `kiln++ -fsyntax-only -fdump-vtable-layouts` and by
  `clang -S -emit-llvm -Xclang -fdump-vtable-layouts` on both triples;
  the dumps are compared as sets of blocks without clang's `Original
  map` sections. Also the "not implemented" error with `-S -emit-llvm`.
  The exhaustive differential is `kiln_abi`'s `vtable_vs_clang`
  (spec/abi.md §7).
- `crates/kiln_codegen/tests/ir_vs_clang.rs`: the IR-diff corpus through
  `kiln_driver::run` (spec/codegen.md §6).
- `run` conformance tests compile and link through the driver.
- Unit tests sit in each module.
- Parity: `cargo xtask test tests/differential/pp tests/conformance/cpp.*
  tests/conformance/lex* --report-parity docs/status/pp-parity.md`. This
  runs `kiln++ -E` against `clang++ -E` on the corpus, including every libc++
  19 header, the sysroot headers, and the torture tests.
