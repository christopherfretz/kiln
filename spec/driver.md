# spec: kiln_driver — the `kiln` command line

`kiln` is a clang-compatible driver. A command line that clang 19 accepts
for preprocessing means the same thing to kiln. An option kiln does not
implement is an error, never silently ignored. The oracle is Debian's
`clang++-19` / `clang-19` on aarch64 and x86_64 Linux.

Three actions are implemented:

- preprocessing: `-E`, `-M`, `-MM`, and `-MD`/`-MMD` alongside `-E`;
- `-fsyntax-only [-fdump-ast] [-fdump-record-layouts]`, which runs
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
| `lib` (`run`) | orchestration, driver diagnostics, outputs, exit status |

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
| `-O*` | also selects `llc`'s level and is passed to the link (see "Code generation") |
| `-O*`, `-f[no-]exceptions`, `-f[no-]cxx-exceptions`, `-f[no-]rtti`, `-f[no-]char8_t`, `-f[no-]pic/PIC/pie/PIE`, `-f[no-]fast-math`, `-ffreestanding`, `-fhosted`, `-pthread`, `-f[no-]threadsafe-statics`, `-f[no-]sized-deallocation` | change predefined macros only (see "Predefines") |
| `-W...`, `-w`, `-pedantic`, `-ferror-limit=`, `-fdiagnostics-format=`, `-f[no-]color-diagnostics`, `-f[no-]caret-diagnostics`, ... | diagnostic options, validated and applied by `kiln_diag::DiagOptions` (`spec/diag.md`) |
| `-fkiln-reproducible-time` | see "Reproducible time" |
| `-fconstexpr-steps=N`, `-fconstexpr-depth=N`, `-fconstexpr-backtrace-limit=N` | constant-evaluation limits, as clang's (defaults 1048576, 512, 10; a backtrace limit of 0 shows every frame); `constexpr_opts`, spec/oven.md "Integration" |
| `-fconstexpr-memory=N[k\|m\|g]` | kiln: live abstract-memory bytes per evaluation (default 256 MiB; binary suffixes) |
| `-fconstexpr-report=<path>` | kiln: write the per-TU NDJSON constant-evaluation report (spec/oven.md "Reporting protocol"); `-fsyntax-only` and every codegen job |
| `-v`, `--verbose`, `--version`, `--help` | see below |
| `-F <dir>` | accepted; warns `ignoring framework directory` |
| `-fsyntax-only` | parse and check (see "Syntax checking") |
| `-fdump-ast[=ids,locs,canonical,implicit,all-files]`, `-fdump-ast-filter=<s>` | with `-fsyntax-only`: the AST dump (`spec/ast-dump.md`) to the output |
| `-fdump-record-layouts`, `-fdump-record-layouts-complete` | with `-fsyntax-only`: the record layouts (`spec/abi.md` §6, §8.4) to the output |
| `-c`, `-S`, `-emit-llvm` | code generation (see "Code generation") |
| `-###` | with a codegen action: print the steps instead of running them |
| `-g`, `-g0`..`-g3`, `-ggdb`, `-ggdb0`..`-ggdb3` | accepted and ignored (no debug info yet) |
| `-l<lib>`, `-L<dir>`, `-Wl,<args>`, `-fuse-ld=<ld>`, `-static`, `-nostdlib`, `-shared`, `-rdynamic`, `-pie`, `-no-pie` | passed to the link command, in command-line order relative to the inputs |
| `-dD`, `-dI` | parsed; "not implemented yet" (WP-017) |

Language of an input: `-x`, else the extension (`.c` and `.i` are C;
everything else, `.h` included, is C++). Standard input (`-`) is C++
unless `-x` says otherwise.

## Toolchain

`Toolchain::new(target, sysroot, resource_dir)` locates:

- **Resource directory:** `-resource-dir`, else `$KILN_RESOURCE_DIR`, else
  the nearest `resource/` with a `predefines/` directory above the
  executable, else the source tree's `resource/`. kiln's predefines base
  files are `<resource>/predefines/<triple>.<std>.txt`.
- **LLVM:** `$KILN_LLVM_DIR`, default `/usr/lib/llvm-19`. This is where
  libc++'s headers are found.
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
  - PIC/PIE (Debian default PIE 2);
  - fast-math;
  - freestanding;
  - `-pthread`;
  - thread-safe statics;
  - sized deallocation.
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
- the parser's Sema evaluates constants under the `-fconstexpr-*`
  limits; after the parse `-fconstexpr-report=` writes the Oven's report
  (`constexpr_opts`);
- the exit status is 1 on any error.

An unknown `-fdump-ast` option gives `invalid value '<opt>' in
'-fdump-ast=<opts>'`.

C inputs (`-x c`, `.c`), `-MD`/`-MMD` with `-fsyntax-only`, and
`-fdump-ast` or `-fdump-record-layouts` without `-fsyntax-only` are "not
implemented yet".

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
  `kiln_codegen::emit` (spec/codegen.md) with the target's data layout,
  `source_filename` = the input path, and `!llvm.ident` =
  `kiln version <v>`. A `CodegenError` is reported at its location as an
  error (`cg_error`). The `-fconstexpr-report` is written for every job.
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
`-###` with `-E` or `-fsyntax-only` (likewise), debug information, and
IR-level optimization (spec/codegen.md).

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

- Driver diagnostics (bad arguments, missing inputs) are prefixed `kiln: `
  as clang prefixes `clang: `. They are reported before any input is
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
- `<action> is not implemented yet` (kiln only)
- `cannot specify -o when generating multiple output files`
- `-emit-llvm cannot be used when linking`
- `unable to execute command: Executable "<tool>" doesn't exist!`
- `unable to execute command: <program>: <reason>`
- `<backend|linker> command failed with exit code <n> (use -v to see invocation)`
- `<backend|linker> command failed due to signal (use -v to see invocation)`
- warning `<x>: 'linker' input unused [-Wunused-command-line-argument]`
- warning `argument unused during compilation: '<o>' [-Wunused-command-line-argument]`

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
- `--help` prints a short summary of the implemented options.

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
2. **`.h` is C++** (clang treats it as C, then as C++ when compiling as
   C++). Pass `-x c` for C headers.
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
11. **`-O1` and above do not optimize the IR.** `-S -emit-llvm -O2` emits
    the `-O0` IR; `-O<n>` only selects
    `llc`'s level. The IR-diff corpus runs at `-O0`.
12. **`.ll` and `.bc` inputs are linker inputs**, not compiled (clang
    would run its backend on them).
13. **`-MD`/`-MMD` with a codegen action is "not implemented yet"**
    (clang writes the dependency file alongside the object).
14. **No debug information:** `-g*` is accepted and ignored.
15. **`-fdump-record-layouts` dumps every complete class**, as
    `-fdump-record-layouts-complete` does. clang dumps only the classes
    it lays out, in the order it lays them out (spec/abi.md A5).

## Tests

- `crates/kiln_driver/tests/driver.rs`: argv to exit status, standard
  output, standard error and files, run in-process through
  `kiln_driver::run` with fake streams. Covers:
  - every option;
  - the error summaries and exit codes;
  - `-M*` rules;
  - `-v` against clang's search block;
  - reproducible time.
- `crates/kiln_driver/tests/codegen.rs`: the codegen path end to end
  (`.ll`, `.s`, `.o`, linking and running, output naming, `-###`, `-v`,
  unused-input warnings, missing tools). Steps needing `llc-19`/`clang++-19`
  are skipped without them.
- `crates/kiln_driver/tests/record_layouts.rs`: the `tests/layout/`
  corpus (at least 40 programs). Each is dumped by
  `kiln -fdump-record-layouts` and by `clang -Xclang
  -fdump-record-layouts-complete` on both triples, and the dumps are
  compared byte for byte (clang's builtin records are dropped). The test
  also covers both spellings and the "not implemented" error with
  `-S -emit-llvm`.
- `crates/kiln_codegen/tests/ir_vs_clang.rs`: the IR-diff corpus through
  `kiln_driver::run` (spec/codegen.md §6).
- `run` conformance tests compile and link through the driver.
- Unit tests sit in each module.
- Parity: `cargo xtask test tests/differential/pp tests/conformance/cpp.*
  tests/conformance/lex* --report-parity docs/status/pp-parity.md`. This
  runs `kiln -E` against `clang -E` on the corpus, including every libc++
  19 header, the sysroot headers, and the torture tests.
