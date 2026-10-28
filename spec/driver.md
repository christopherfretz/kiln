# spec: kiln_driver — the `kiln` command line

`kiln` is a clang-compatible driver. A command line that clang 19 accepts
for preprocessing means the same thing to kiln. An option kiln does not
implement is an error, never silently ignored. The oracle is Debian's
`clang++-19` / `clang-19` on aarch64 and x86_64 Linux.

Only preprocessing is implemented (`-E`, `-M`, `-MM`, and `-MD`/`-MMD`
alongside `-E`). `-fsyntax-only` (WP-048), `-c`/`-S`/`-emit-llvm`, and
linking parse, then report `<action> is not implemented yet` and exit 1.

Crate layout (each part is replaceable on its own):

| module | job |
|---|---|
| `args` | parse argv into `Args`: no file system, no diagnostics |
| `toolchain` | target, `-std=`, resource dir, GCC/libc++ detection, the system search directories |
| `pp_cmd` | one `-E` job: predefines, search list, preprocessor, printer, dependency list |
| `deps` | Make-style dependency output |
| `time` | reproducible `__DATE__`/`__TIME__`/`__TIMESTAMP__` |
| `stub_features` | STUB(WP-015): table-driven `FeatureQueries` |
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
| `-O*`, `-f[no-]exceptions`, `-f[no-]cxx-exceptions`, `-f[no-]rtti`, `-f[no-]char8_t`, `-f[no-]pic/PIC/pie/PIE`, `-f[no-]fast-math`, `-ffreestanding`, `-fhosted`, `-pthread`, `-f[no-]threadsafe-statics`, `-f[no-]sized-deallocation` | change predefined macros only (see "Predefines") |
| `-W...`, `-w`, `-pedantic`, `-ferror-limit=`, `-fdiagnostics-format=`, `-f[no-]color-diagnostics`, `-f[no-]caret-diagnostics`, ... | diagnostic options, validated and applied by `kiln_diag::DiagOptions` (`spec/diag.md`) |
| `-fkiln-reproducible-time` | see "Reproducible time" |
| `-v`, `--verbose`, `--version`, `--help` | see below |
| `-F <dir>` | accepted; warns `ignoring framework directory` |
| `-fsyntax-only`, `-c`, `-S`, `-emit-llvm` | parsed; "not implemented yet" |
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

## Feature queries (STUB(WP-015))

Until WP-015's feature tables land, the feature queries are answered by
`stub_features` from a table of clang 19's answers. This covers
`__has_feature`, `__has_extension`, `__has_builtin`, `__has_attribute`,
`__has_cpp_attribute`, `__has_declspec_attribute`, `__is_identifier` and
`__has_keyword`, for every query that appears in the libc++ 19, libstdc++
and glibc headers and in the test corpus. The table is generated by
`scripts/gen-stub-features.py` and is per language and C++20-ness, with
`-fno-exceptions` and `-fno-rtti` applied. An unknown query falls back to
`NoFeatures`. The `#if` evaluator is the core's stub until WP-015's
`IfEval` lands.

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

## Tests

- `crates/kiln_driver/tests/driver.rs`: argv to exit status, standard
  output, standard error and files, run in-process through
  `kiln_driver::run` with fake streams. Covers:
  - every option;
  - the error summaries and exit codes;
  - `-M*` rules;
  - `-v` against clang's search block;
  - reproducible time.
- Unit tests sit in each module.
- Parity: `cargo xtask test tests/differential/pp tests/conformance/cpp.*
  tests/conformance/lex* --report-parity docs/status/pp-parity.md`. This
  runs `kiln -E` against `clang -E` on the corpus, including every libc++
  19 header, the sysroot headers, and the torture tests.
