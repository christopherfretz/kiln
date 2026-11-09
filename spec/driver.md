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

- preprocessing: `-E`, `-M`, `-MM`, and `-MD`/`-MMD` alongside `-E`
  (`-MD`/`-MMD` also go with the other actions: "Dependency output");
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
starting with `-` are inputs. Linker inputs (`-l<lib>`, `-Wl,<args>`,
`-Xlinker <arg>`) count as inputs for `no input files`, as clang's: with
nothing else, `kiln++ -Wl,--version` runs the link step alone (the
linker prints its version; Meson's linker probe, WP-920), and `-c
-Wl,--version` warns `'linker' input unused`.

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
| `-include <file>` (joined or separate) | `#include "<file>"` at the end of the predefines' `<command line>` section, after the `-D`/`-U` macros, in order (clang `AddImplicitInclude`); found from the working directory first, then the `"..."` search chain |
| `-I`, `-iquote`, `-isystem`, `-idirafter`, `-cxx-isystem` | user search directories; `-I=dir` is relative to the sysroot |
| `-nostdinc`, `-nostdinc++`, `-nostdlibinc`, `-nobuiltininc` | drop toolchain directories (see "Toolchain") |
| `--sysroot`, `-isysroot` | sysroot (`-isysroot` wins for headers) |
| `-resource-dir <dir>` | the builtin headers are in `<dir>/include` |
| `-stdlib=libstdc++` / `libc++` | C++ standard library headers (default libstdc++) |
| `--gcc-toolchain=<dir>` | joined only: the GCC installation search looks under `<dir>` instead of `<sysroot>/usr` (see "Toolchain") |
| `--gcc-install-dir=<dir>` | joined only: `<dir>` (`.../lib/gcc/<triple>/<version>`) is the GCC installation; wins over `--gcc-toolchain=`; without a `crtbegin.o` in it, `'<dir>' does not contain a GCC installation` and no GCC |
| `--llvm-dir=<dir>` | joined only, last wins (WP-1016): `<dir>` is the LLVM installation kiln drives ("LLVM discovery"); not one, or not LLVM 19 or 22, is an error |
| `-fexperimental-library`, `-fno-experimental-library` | libc++'s experimental library: `__has_feature(experimental_library)` and `__has_extension(experimental_library)` are 1 (a `kiln_pp` feature overlay, as clang's cc1 flag; libc++ derives `_LIBCPP_ENABLE_EXPERIMENTAL` from it), and the link step gets `-fexperimental-library` (clang adds `-lc++experimental`); the last wins (`Args::experimental_library`, WP-461) |
| `--target=<triple>` | `aarch64`/`arm64` or `x86_64`/`amd64` Linux; anything else is an error |
| `-march=<arch>[+ext...]`, `-mcpu=<cpu>[+ext...]`, `-msve-vector-bits=<N\|scalable>`, `-target-feature <±feat>` (usually via `-Xclang`) | AArch64 target features and the `__ARM_*` predefines ("Target features" below, WP-459, WP-469) |
| `--embed-dir=<dir>` | `#embed` search directory |
| `-M`, `-MM`, `-MD`, `-MMD`, `-MF`, `-MT`, `-MQ`, `-MP`, `-MG` | dependency output (see below) |
| `-fmodule-file-deps`, `-fno-module-file-deps` | list the module files imports read in the dependency output (on by default for `--precompile`; "Dependency output", WP-1051); the last wins |
| `-MJ <file>` | the JSON compilation-database fragment (see "`-MJ`", WP-920) |
| `-dumpversion`, `-dumpmachine`, `-print-*` | toolchain probes (see "Toolchain probes", WP-920) |
| `-trigraphs`, `-ftrigraphs`, `-fno-trigraphs` | trigraphs (default: on only for strict C and C++ before 17) |
| `-f[no-]dollars-in-identifiers` | `$` in identifiers (default on) |
| `-O*` | also selects `llc`'s level and the `opt` pipeline, and is passed to the link (see "Code generation") |
| `-disable-llvm-passes` (usually after `-Xclang`) | no `opt` run: `-emit-llvm` writes the frontend's IR (see "IR optimization") |
| `-O*`, `-f[no-]exceptions`, `-f[no-]cxx-exceptions`, `-f[no-]rtti`, `-f[no-]char8_t`, `-ffreestanding`, `-fhosted`, `-pthread`, `-f[no-]threadsafe-statics`, `-f[no-]sized-deallocation` | change predefined macros only (see "Predefines"); `-f[no-]char8_t` also reaches the lexer, the parser and sema (`ParseOptions::char8`, `PpOptions::char8`: the `char8_t` keyword and the type of `u8` literals, spec/lex.md "-fno-char8_t", WP-808); `-fno-sized-deallocation` also reaches sema (`ParseOptions::sized_deallocation`: no implicit sized global `operator delete`, none preferred, spec/sema.md "`-fno-sized-deallocation`", WP-808); `-fno-rtti` also makes sema reject `typeid` and `dynamic_cast` to a class, and codegen leave vtables' RTTI slots null (`CodegenOptions::rtti`, spec/codegen.md "RTTI", WP-720) |
| `-f[no-]builtin`, `-fno-builtin-<name>` (repeatable), `-ffreestanding`, `-fhosted` | clang's library-builtin switches (WP-808): `Args::no_builtin()` is the last `-f[no-]builtin` negative or the last `-ffreestanding` / `-fhosted` freestanding (`-fbuiltin` does not cancel `-ffreestanding`); `Args::no_builtin_fns()` the sorted `-fno-builtin-<name>` names, none under `no_builtin()` (the clang driver drops them). They reach sema (`ParseOptions::no_builtin{,_fns}`: no library-builtin recognition, spec/sema.md §5.4), codegen (`CodegenOptions::no_builtin{,_fns}`, spec/codegen.md "no-builtin") and the feature queries (`__has_builtin` of a library builtin is 0, spec/pp.md) |
| `-fsigned-char`, `-funsigned-char` (`-fno-unsigned-char`, `-fno-signed-char` are their synonyms; the last one wins) | the signedness of plain `char` ([basic.fundamental]/7), overriding the target's (signed on x86-64, unsigned on AArch64): it sets `TargetTypes::char_signed` of the `kiln_abi::Target` the parser, sema, the Oven and codegen use, the `#if` evaluator's character type ([cpp.cond]), and adds or removes `__CHAR_UNSIGNED__` as clang 19 does (see "Predefines") |
| `-finput-charset=<v>`, `-fexec-charset=<v>` | the source and execution character sets ([lex.charset]/1, [lex.phases]/1.1): kiln reads source as UTF-8 and encodes ordinary literals as UTF-8, so, as clang's driver, it accepts only `UTF-8` (any letter case) and changes nothing; another value is the driver error `invalid value '<v>' in '-finput-charset=<v>'` (WP-726; folly passes `-finput-charset=UTF-8`) |
| `-fclang-compat=<major>` | kiln: which clang release's observable output to reproduce, `19` or `22` (default since WP-394, the oracle switch; the library-level defaults, `ClangCompat::default()` and every crate's `clang_compat`, follow it since WP-413): predefined macros and `__has_*` answers (spec/pp.md "clang compatibility level"), diagnostic text and names, AST dump types and deprecation warnings (spec/sema.md §1.1, spec/ast-dump.md §2, spec/codegen.md); any other value is an error |
| `-W...`, `-w`, `-pedantic`, `-ferror-limit=`, `-fdiagnostics-format=`, `-f[no-]color-diagnostics`, `-f[no-]caret-diagnostics`, ... | diagnostic options, validated and applied by `kiln_diag::DiagOptions` (`spec/diag.md`) |
| `-fkiln-reproducible-time` | see "Reproducible time" |
| `-fkiln-source-path-style=<clang\|gcc>` | kiln: how `__builtin_FILE()`, `__builtin_FILE_NAME()`, `std::source_location::current()` (also as a default argument) and `std::meta::source_location_of` spell a file (`SourcePathStyle`, `SourceManager::presumed_file_name`; WP-636). `clang` (the default): the presumed name as written, as clang 19 and 22 (`SourceLocExpr::EvaluateInContext`), so `-I /p//inc/./` gives `/p//inc/./x.h`; `gcc`: GCC 16.1's, an included file's absolute path with empty and `.` components dropped and `dir/..` folded through a real directory (`/p/inc/x.h`; spec/reflection.md §3.9). `-fmacro-prefix-map=` applies after either; `__FILE__` and `__BASE_FILE__` keep clang's spelling under both. Any other value is `invalid value`; the last one wins |
| `-fconstexpr-steps=N`, `-fconstexpr-depth=N`, `-fconstexpr-backtrace-limit=N` | constant-evaluation limits, as clang's (defaults 1048576, 512, 10; a backtrace limit of 0 shows every frame); `constexpr_opts`, spec/oven.md "Integration". GCC's `-fconstexpr-ops-limit=N` (passed by the reference workload's generator compile) sets the step budget too |
| `-ffile-prefix-map=<old>=<new>`, `-fmacro-prefix-map=<old>=<new>` | `__FILE__`, `__BASE_FILE__`, `__builtin_FILE()`, `std::source_location` and `std::meta::source_location_of` spell a path starting with `<old>` with `<new>` instead (`SourceManager::remap_macro_path`; clang `processPathForFileMacro`). Entries are tried in descending order of `<old>`, the first match wins, and the match is a plain string prefix. A value without `=` is `invalid value` (deviation 20). `-ffile-prefix-map=` also remaps debug information file names, as `-fdebug-prefix-map=` does ("Debug information flags") |
| `-fcontract-evaluation-semantic=<ignore\|observe\|enforce\|quick_enforce>` | the contract evaluation semantic of the translation unit (GCC's spelling; `quick-enforce` also accepted; default `enforce`; `ParseOptions::contract_semantic`, `Ast::contract_semantic`); any other value is `unsupported argument`. All four semantics compile to IR and are evaluated in the Oven (spec/contracts.md §6, WP-707) |
| `-fconstexpr-memory=N[k\|m\|g]` | kiln: live abstract-memory bytes per evaluation (default 256 MiB; binary suffixes) |
| `-f[no-]reflection` | P2996 reflection (WP-267; on by default at C++26 since WP-920, operator decision Q7; the last one wins; below C++26 off whatever the flag, an explicit `-freflection` accepted and ignored; spec/reflection.md §1.4): `^^`, `[:`, `:]` are lexed, and `^^`, type splices and the kiln intrinsics are accepted (spec/sema.md §8a, spec/parse.md §5.2). `__cpp_reflection` is not predefined. Passed to the preprocessor, the parser and sema for `-E`, `-fsyntax-only` and codegen jobs |
| `-fkiln-reflection-mangling=<kiln\|gcc\|kiln-hashed>` | kiln: how a `std::meta::info` template argument is spelled in a symbol (`kiln_abi::ReflectionMangling`, `CodegenOptions::reflection_mangling`; WP-501). `kiln` (the default): `L u6__info <kind> <operand> E`; `gcc`: GCC 16.1's `L Dm <kind> <operand> E` with `info` itself `Dm`, link-compatible with `g++-16`; `kiln-hashed`: `L u6__info h<32 hex> E`, a prototype. All three give the reflection its reflectee's linkage (spec/reflection.md §8.3). Any other value is `invalid value`; the last one wins. Codegen jobs only |
| `-f[no-]coro-aligned-allocation` | aligned coroutine frame allocation (default off, as clang; the last one wins): sema looks up the `std::align_val_t` forms of the frame's `operator new`/`delete` first (spec/sema.md §7.13). Passed to sema for `-fsyntax-only` and codegen jobs |
| `-fconstexpr-report=<path>` | kiln: write the per-TU NDJSON constant-evaluation report (spec/oven.md "Reporting protocol"); `-fsyntax-only` and every codegen job |
| `-fconstexpr-stats`, `-fconstexpr-profile[=text\|json\|<file>]`, `-fconstexpr-profile-limit=N` | kiln: after the translation unit, the constant-evaluation summary and the per-function / per-site profile (see "Constexpr profile"; WP-1116); `-fsyntax-only` and every codegen job. An empty `-fconstexpr-profile=` is `drv_missing_value`; a limit that is not an unsigned integer is `drv_invalid_int_value` |
| `-ftemplate-depth=N`, `-ftemplate-backtrace-limit=N`, `-fkiln-instantiation-report[=all\|json\|text]` | template limits and the instantiation report (spec/sema-templates.md §10.3, §11.3) |
| `-fkiln-stack-size=N[k\|m\|g]` | kiln: the stack reserved for the compiling thread (default 1 GiB; at least 4 MiB; the last valid value wins). Running out is the fatal "kiln: stack exhausted during …" (spec/sema-templates.md §10.3), never a crash |
| `-v`, `--verbose`, `--version`, `--help` | see below |
| `-F <dir>` | accepted; warns `ignoring framework directory` |
| `-ftime-trace[=<path>]`, `-ftime-trace-granularity=<n>`, `-ftime-trace-verbose` | write a Chrome trace per compiled input where clang writes it (see "Time trace"; WP-1115). Without a compiler job that writes one (`-fsyntax-only`, a header precompile), each spelling is `argument unused during compilation` (`-Wunused-command-line-argument`, silenced by `-Qunused-arguments`), as clang; granularity and verbose are unused without `-ftime-trace`. A granularity that is not an unsigned integer is `invalid integral value '<v>' in '<flag>'` (`drv_invalid_int_value`) |
| `-fsyntax-only` | parse and check (see "Syntax checking") |
| `-verify[=<prefixes>]`, `-verify-ignore-unexpected[=<levels>]` | with `-fsyntax-only`: check the diagnostics against `expected-*` comments (see "-verify") |
| `-Xclang <arg>` | `<arg>` is parsed as a driver option (kiln has no separate `cc1`), so `-Xclang -verify` is `-verify` |
| `-fdump-ast[=ids,locs,canonical,implicit,all-files]`, `-fdump-ast-filter=<s>` | with `-fsyntax-only`: the AST dump (`spec/ast-dump.md`) to the output |
| `-fkiln-cfg-stats` | with `-fsyntax-only` (otherwise not implemented): one line per function body sema analyzes, `cfg-stats: '<function>' status=<Ok\|TooLarge\|...> blocks=N elems=N edges=N bytes=N build_ns=N`, to the output (spec/sema.md §7.5.1); a measurement aid, not a stable format |
| `-fkiln-memory-report` | with `-fsyntax-only` or codegen: after the diagnostics (and after `-fkiln-instantiation-report`'s rows), one line per memory row to stderr, `memory: <name> count=N live=N reserved=N`, then `memory: total live=N reserved=N` (the sum over the rows, leaving out the derived `instantiation.*` rows). The rows are their producers' (`Ast::memory` with the type context's `types.*`, `Sema::memory`, `Oven::memory`), in that order; the driver lists none itself. Taken when parsing ends, before codegen. A measurement aid, not a stable format (WP-663) |
| `-fpermissive` | GCC's; its one effect: in C++26 the arithmetic between different enumeration types, or an enumeration and a floating type (P2864, an error), is diagnosed with the C++20/23 deprecation warning and its group (`-Wdeprecated-enum-enum-conversion`, ...) instead. GCC 16.1 makes it a pedwarn (a warning even without the flag); kiln keeps clang's error by default |
| `-fkiln-dump-cfg` | with `-fsyntax-only` (otherwise not implemented): the control-flow graph of every function body sema analyzes (spec/sema.md §7.5.1), in clang's `DumpCFG` layout, to the output (after the diagnostics, before an AST dump) |
| `-fkiln-cfg-max-elems=N` | kiln: the element budget of one body's graph (default 33554432); a body over it builds no graph and its CFG-based warnings are skipped (spec/sema.md §7.5.1). A value that is not an integer is `invalid integral value` |
| `-fkiln-outline-expansions=N` | kiln: the size threshold, in AST nodes summed over the elements, above which an expansion statement's elements are emitted as separate internal helpers at `-O1` and above (default 2048; `0` disables; spec/landed/codegen/WP-793.md). A value that is not an integer is `invalid integral value` |
| `-fdump-record-layouts`, `-fdump-record-layouts-complete` | with `-fsyntax-only`: the record layouts (`spec/abi.md` §6, §8.4) to the output |
| `-fdump-vtable-layouts` | with `-fsyntax-only`: the vtable groups (`spec/abi.md` §8.5) to the output |
| `-c`, `-S`, `-emit-llvm` | code generation (see "Code generation") |
| `-###` | with a codegen action: print the steps instead of running them |
| `-moutline-atomics`, `-mno-outline-atomics` | AArch64: `+outline-atomics` or `-outline-atomics` in every function's `target-features` (spec/codegen.md); the last wins. Without either, `+outline-atomics` only when the GCC installation ("Toolchain") is 9.3.1 or later, else neither feature (clang 19/22 `Linux::IsAArch64OutlineAtomicsDefault`, the libgcc runtime, `GCCVersion::isOlderThan`), so a cross compile with no AArch64 GCC has inline atomics as clang's (WP-808). Ignored on x86-64 with clang's warning `'x86_64' does not support '<last spelling>'; flag ignored [-Woption-ignored]`, once per source input (clang warns once per compile job) and in every mode; `-Werror` makes it an error and no job runs (WP-228) |
| `-fomit-frame-pointer`, `-fno-omit-frame-pointer`, `-momit-leaf-frame-pointer`, `-mno-omit-leaf-frame-pointer` | the frame-pointer kind of every function and the `frame-pointer` module flag (spec/codegen.md "Frame pointer"); the last of each pair wins (WP-405) |
| `-fstrict-aliasing`, `-fno-strict-aliasing` | whether loads and stores carry `!tbaa` above `-O0` (`CodegenOptions::strict_aliasing`, spec/codegen.md "TBAA"); the last wins, strict by default (WP-412) |
| `-flax-vector-conversions[=none\|integer\|all]`, `-fno-lax-vector-conversions` | which same-size vector bitcasts are implicit conversions (`Sema::lax_vector_conversions`, spec/types.md `Vector`): `all` without a flag, `integer` for the bare flag, `none` for the negative; the last wins. An unknown value is `invalid value '<v>' in '-flax-vector-conversions=<v>'` (WP-445) |
| `-fcx-limited-range`, `-fno-cx-limited-range`, `-fcx-fortran-rules`, `-fno-cx-fortran-rules`, `-fcomplex-arithmetic=full\|improved\|basic` | the `_Complex` multiply/divide algorithm (`CodegenOptions::complex_range`, spec/codegen.md §7n): basic, full, improved, full, or the named one; the last of these and the fast-math flags (basic; "Floating-point flags") wins, and without any it is full. At `-fclang-compat=19`, `-fno-fast-math` leaves it alone; at 22 it clears it, and `-ffp-model=precise\|strict` set full. `-fcomplex-arithmetic=promoted` and unknown values are `unsupported argument '<v>' to option '-fcomplex-arithmetic='`. clang's `-Woverriding-option` between these flags is not issued (WP-237) |
| `-ftrivial-auto-var-init=uninitialized\|zero\|pattern`, `-ftrivial-auto-var-init-max-size=N` | the erroneous value of an uninitialized automatic variable (`CodegenOptions::auto_var_init` / `auto_var_init_max_size`, spec/codegen.md §7r, §7y, WP-448, WP-452): `zero` fills with zero bytes, `pattern` with clang's pattern (`0xAA` bytes, all-ones floating point), `uninitialized` does not; the last wins, and without either it is `zero` under C++26 and later, `uninitialized` before. `N` bytes is the largest variable filled (default 4096, 0 = no limit); a non-integer is `invalid integral value '<v>' in '<arg>'`. An unknown value is `unsupported argument '<v>' to option '-ftrivial-auto-var-init='` |
| `-fpic`, `-fPIC`, `-fpie`, `-fPIE`, `-fno-pic` (and `-fno-PIC`/`-fno-pie`/`-fno-PIE`) | the relocation model, the last one winning (default `-fPIE`, as clang on Linux): the `__pic__`/`__pie__` predefines, `llc`'s `-relocation-model=`, the `PIC Level`/`PIE Level` module flags and which globals are `dso_local` (spec/codegen.md "Symbol visibility and `dso_local`", WP-544) |
| `-f[no-]semantic-interposition`, `-f[no-]direct-access-external-data` | the `dso_local` inputs of clang's `shouldAssumeDSOLocal` (spec/codegen.md "Symbol visibility and `dso_local`"): under `-fPIC`, `-fno-semantic-interposition` makes external non-COMDAT function definitions `dso_local` and `-fsemantic-interposition` adds the `SemanticInterposition` module flag; `-fdirect-access-external-data` (default only with `-fno-pic`) makes non-TLS variable declarations (and, under `-fno-pic`, function declarations) `dso_local` under `-fPIE`/`-fno-pic`, and a value other than the relocation model's default adds the `direct-access-external-data` module flag. The last of each pair wins |
| `-fvisibility=default\|hidden\|protected\|internal`, `-fvisibility-ms-compat`, `-f[no-]visibility-inlines-hidden` | the default symbol visibility (`CodegenOptions::visibility` / `type_visibility` / `inlines_hidden`, spec/codegen.md "Symbol visibility and `dso_local`", WP-544): the last `-fvisibility=` or `-fvisibility-ms-compat` wins; `internal` is `hidden`, as clang; `-fvisibility-ms-compat` is hidden value visibility with default type visibility (vtables, RTTI), accepted silently as by clang. `-fvisibility-inlines-hidden` hides inline functions that are not explicit instantiations (the last of the pair wins). An unknown value is clang's `-cc1` error `invalid value '<v>' in '-fvisibility=<v>'` (no `kiln++:` prefix, once per input, as the `-std=` one), unless a later `-fvisibility=`/`-fvisibility-ms-compat` replaces it (clang forwards only the last) |
| `-g`, `-g0`..`-g3`, `-ggdb*`, `-gline-tables-only`, `-gdwarf-N`, `-gsplit-dwarf`, `-gz`, ... | DWARF debug information; see "Debug information flags" below (WP-814) |
| `-l<lib>`, `-L<dir>`, `-B<dir>`, `-Wl,<args>`, `-Xlinker <arg>`, `-fuse-ld=<ld>`, `-static`, `-nostdlib`, `-shared`, `-rdynamic`, `-pie`, `-no-pie` | passed to the link command, in command-line order relative to the inputs |
| `-dD` | with `-E`: the text, plus every `#define`/`#undef` where it occurs, the predefines' and command line's first (as clang's; WP-920) |
| `-dI` | with `-E`: the text, plus every `#include`/`#include_next`/`#import` directive as `#include "x.h" /* clang -E -dI */` before the header's text (clang's spelling, WP-920) |
| `-fmodule-file=<name>=<path>`, `-fprebuilt-module-path=<dir>` | C++20 modules: where module interface sources are found (see "Modules"); `-fmodule-file=<path>` without a name names a module file whose header gives the module (spec/modules.md §11.6) |
| `-fmodules-cache-path=<dir>`, `-f[no-]kiln-module-cache` | where the `std` / `std.compat` module cache lives, and turning it off (see "Module cache") |
| `-fmodules-prune-interval=<seconds>`, `-fmodules-prune-after=<seconds>` | how often the module cache is pruned, and after how long unused an entry is removed (clang's flags and defaults; 0 turns pruning off; see "Module cache") |
| `-f[no-]fast-math`, its parts, `-ffp-model=`, `-ffp-contract=`, `-fdenormal-fp-math=`, the FP environment flags | see "Floating-point flags" (WP-806) |
| `-fstack-protector*`, `-f[no-]unwind-tables`, `-f[no-]asynchronous-unwind-tables`, `-f[no-]plt`, `-ffunction-sections`, `-fdata-sections`, `-mtune=`, `-f[no-]wrapv`, `-f[no-]strict-overflow`, `-f[no-]delete-null-pointer-checks`, `-Wp,<list>`, `-Xpreprocessor <arg>`, and the ignored and rejected sets | see "Codegen flags" (WP-605) |
| `-fcrash-diagnostics[=compiler\|all\|off]`, `-fno-crash-diagnostics` | whether an internal compiler error writes the crash reproducer (default on; `=off` and the negative turn it off, the last wins); see "Internal compiler errors" (WP-911) |
| `-fcrash-diagnostics-dir=<dir>` | where the crash reproducer is written (default `$TMPDIR`, else `/tmp`) (WP-911) |
| `-fkiln-crash-for-testing=<phase>` | hidden (kiln's tests): panic on purpose in `<phase>`; see "Internal compiler errors" (WP-911) |
| `-fkiln-bmi-kind=<1\|2>` | hidden (kiln's tests and measurements): the kind of module file `--precompile` / `-fmodule-output` writes. `1`: the source capsule only; `2` (the default): the capsule plus the carried constant results when at least one is carried, else kind 1. `-v` names the kind written and why (spec/modules.md §11.11). Any other value is `invalid value`; the last one wins (WP-985) |
| `-fkiln-bmi-image`, `-fno-kiln-bmi-image` | hidden, **off by default** (kiln's tests and measurements): at `--precompile` / `-fmodule-output`, also write the declaration image section when the unit qualifies (the file is then kind 2 even with no constant result carried; `-v` adds ", with a declaration image of N declarations" or "(no declaration image: <reason>)"); at an import, build a module whose file has an image from the image instead of replaying its capsule. Either side alone is harmless: no section is written, or a present one is ignored. Not part of the flags fingerprint. The last one wins (spec/modules.md §11.11 "Part 2a", WP-1048) |

### Response files (WP-799)

An argument `@file` is replaced by the arguments read from `file`
before anything else is parsed (`rsp::expand`, both in `main` and in
`run_as`), as clang's driver (`ExpansionContext::expandResponseFiles`,
GNU tokenizer): whitespace separates arguments, `'` and `"` quote (an
empty pair is an empty argument), and a backslash takes the next
character literally, also inside quotes. A nested `@file` is expanded
in turn, relative to the current directory, up to 32 levels deep. An
`@file` that cannot be read is left as written, so it is an input that
does not exist. CMake's Makefile generator links through
`@CMakeFiles/<target>.dir/objects1.rsp` when a target has many objects.

### Codegen flags (WP-605)

The common GCC/clang code-generation and hardening flags production builds
pass (CMake presets, distribution `CFLAGS`). `CodegenFlags`
(`crates/kiln_driver/src/codegen_flags.rs`) parses them; within each
`-fX`/`-fno-X` pair the last wins. Every flag falls in one of three classes.

**1. Honoured** (each matches clang 19's IR for the same flag, pinned by
`tests/codegen/`):

| flag | effect |
|---|---|
| `-fno-inline`, `-finline` | the last wins; `-fno-inline` keeps `__NO_INLINE__` defined at every `-O` level and makes every definition but an `always_inline` one `noinline` (clang `CodeGenOptions::OnlyAlwaysInlining`; at `-O0` that is already the case) |
| `-mcmodel=tiny\|small\|kernel\|medium\|large` | the `Code Model` module flag (LLVM's enumerator, 0 to 4) and `llc -code-model=<model>`; on x86-64, `medium` and `large` also add the `Large Data Threshold` module flag (`i64` 65536 and 0, clang's driver defaults). Another value is `unsupported argument` |
| `-fstack-protector`, `-fstack-protector-strong`, `-fstack-protector-all`, `-fno-stack-protector` | function attribute `ssp`, `sspstrong`, `sspreq`, or none (the default) on every function kiln defines (`CodegenOptions::stack_protector`); the last of the four wins. A `no_stack_protector` function gets none of them (WP-813) |
| `-f[no-]asynchronous-unwind-tables`, `-f[no-]unwind-tables` | `CodegenOptions::unwind_tables`: asynchronous (`uwtable`, `"uwtable"` module flag 2) unless the last `-f[no-]asynchronous-unwind-tables` is the negative (default: asynchronous, except under `-ffreestanding`); otherwise synchronous (`uwtable(sync)`, flag 1) with `-funwind-tables`, else none (no attribute, no flag). So the flags never remove the default `uwtable` alone |
| `-fno-plt`, `-fplt` | `nonlazybind` on every function declared but not defined in the TU, the `"RtLibUseGOT"` module flag, and under `-fno-pic` function declarations are not assumed `dso_local` (clang `shouldAssumeDSOLocal`) (`CodegenOptions::no_plt`) |
| `-ffunction-sections`, `-fdata-sections` | `llc -function-sections` / `-data-sections` (see "Code generation") |
| `-fc++-static-destructors[=all\|thread-local\|none]`, `-fno-c++-static-destructors` | which static and thread-local variables register their destructors (`CodegenOptions::static_destructors`, clang `RegisterStaticDestructorsKind`; WP-805): `all` (the default, and the bare flag), `thread-local` (only `thread_local` ones), `none` (= `-fno-`; `[[clang::always_destroy]]` variables still register); the last wins; another value is `invalid value '<v>' in '-fc++-static-destructors=<v>'`. spec/landed/sema/WP-805.md |
| `-march=<cpu>` (x86-64), `-march=native`, `-mcpu=native` (AArch64, without `-march=`), `-mtune=<cpu>` | x86-64: `-march=` names one of clang's x86 CPUs (`x86-64`, `x86-64-v2`..`v4`, `skylake`, `znver4`, ...; clang 22 adds `wildcatlake`, `novalake`, `diamondrapids`) and sets `"target-cpu"` (the default `"tune-cpu"="generic"` is then omitted, as clang); its features and predefined macros are clang 19's or clang 22's ("x86-64 target features"; spec/landed/driver/WP-630.md, WP-1000.md, table layout spec/landed/codegen/WP-660.md; `wildcatlake` and `novalake` are not expanded, deviation). `native` on either target emits no `"target-cpu"` and passes `llc -mcpu=native` (the IR attribute would override `-mcpu`); on a Linux x86-64 host x86 `-march=native` takes its features from `/proc/cpuinfo`, otherwise kiln does not inspect the host. `-mtune=<cpu>` sets `"tune-cpu"`; `-mtune=native` is accepted and adds nothing. The triple is unchanged |
| `-fwrapv`, `-fno-wrapv`, `-f[no-]wrapv-pointer`, `-fno-strict-overflow`, `-fstrict-overflow` | signed overflow wraps (`CodegenOptions::wrapv`): the last of `-f[no-]wrapv`, else `-fno-strict-overflow`; no `nsw` on signed arithmetic, increments, negation, shifts or VLA sizes. Pointer overflow wraps (`wrapv_pointer`): at `-fclang-compat=19` exactly when `wrapv`; at 22 the last of `-f[no-]wrapv-pointer`, else `-fno-strict-overflow`; no `inbounds` on pointer arithmetic and subscripts, and at 22 none on field GEPs either. The Oven is unchanged: signed overflow in a constant expression stays non-constant ([expr.const]/5), unlike GCC, which folds it as wrapping under `-fwrapv` (deliberate divergence, deferred) |
| `-ftrapv`, `-fno-trapv`, `-ftrapv-handler=<name>` (WP-1032) | `CodegenOptions::trapv` / `trapv_handler`, clang's `SOB_Trapping`: the last of `-ftrapv` / `-fno-trapv` (a kiln spelling; clang has none and GCC's cancels `-ftrapv`). Signed `+ - *`, unary `-` (checked as `0 - x`), `++`/`--` and compound assignment use `llvm.s{add,sub,mul}.with.overflow` and, on overflow, a bare `@llvm.ubsantrap(<id>)` (clang `EmitTrapCheck`: add 0, sub 21, mul 12), with no handler call and no `.src` data; `-ftrapv` beats `-fwrapv` and `-fno-strict-overflow`. Under `-fsanitize=signed-integer-overflow` the sanitizer's check wins for the signed operations it covers. An `_Atomic` signed operand takes the compare-exchange loop. `-ftrapv-handler=<name>`, with `-ftrapv` (alone it has no effect, as clang), replaces the trap or `__ubsan_handle_*` call of every overflow check, `-ftrapv`'s and signed/unsigned-integer-overflow's, by `<name>(i64 sext l, i64 sext r, i8 op, i8 width, ...)` in a block at the function's end whose `i64` result, truncated, is the operation's value (`op` is add 1, sub 2, mul 3, shifted left once, plus 1 when signed). Test: `tests/conformance/driver.sanitize/trapv-*.cpp`, `crates/kiln_driver/tests/codegen.rs` |
| `-fno-delete-null-pointer-checks`, `-fdelete-null-pointer-checks` | `CodegenOptions::null_pointer_is_valid`: every definition gets `null_pointer_is_valid`; reference parameters, returns and `this` lose `nonnull` (`this` gets `dereferenceable_or_null`), `nonnull`/`returns_nonnull` attributes, the `operator new` result and loaded references lose `nonnull` (and `!nonnull` metadata), as clang |
| `-fpatchable-function-entry=N[,M]`, `-fzero-call-used-regs=<kind>`, `-mfunction-return=keep\|thunk-extern` (x86), `-fcf-protection[=full\|branch\|return\|none]` (x86), `-mspeculative-load-hardening`, `-fsplit-stack`, `-finstrument-functions`, `-finstrument-functions-after-inlining`, `-finstrument-function-entry-bare`, `-fxray-instrument`, `-fxray-instruction-threshold=N`, `-fxray-ignore-loops` (WP-813) | `CodegenOptions::hardening` (`kiln_codegen::Hardening`): the function attributes and module flags clang 19 emits for each, with the class B attributes overriding them per function (spec/landed/codegen/WP-813.md). `-mspeculative-load-hardening` on x86 also adds `+retpoline-indirect-calls` to the target features; `-finstrument-functions*` at `-O0` runs `default<O0>` so ee-instrument emits the calls; `-fxray-instrument` is passed on to the link. Errors as clang: a bad `-fpatchable-function-entry=` (`invalid argument 'x' to -fpatchable-function-entry=`, or the offset larger than the count), `-fxray-instrument` with `-fpatchable-function-entry=`, a bad `-fxray-instruction-threshold=` under `-fxray-instrument` are driver errors; a bad `-mfunction-return=` / `-fzero-call-used-regs=` / `-fcf-protection=` value (the last of each wins), `-mfunction-return=` off x86 and (when generating code) `-fcf-protection` off x86 are clang's `-cc1` errors, printed without the `kiln++:` prefix and in cc1's order (an invalid `-mfunction-return=` on x86 is reported twice, as clang 19 does). Without `-fxray-instrument` the other `-fxray-*` options are `argument unused during compilation` |
| `-mbranches-within-32B-boundaries` (x86; WP-961) | `llc -x86-branches-within-32B-boundaries`, as clang 19's `addX86AlignBranchArgs`: the integrated assembler pads so no branch (fused, `jcc`, `jmp`) crosses or ends on a 32-byte boundary. Off x86, clang's driver error `unsupported option '-mbranches-within-32B-boundaries' for target '<triple>'`. There is no negative form (clang has none). Test: `crates/kiln_driver/tests/hardening.rs` `branches_within_32b_boundaries`, `flag_errors` |
| `-flto`, `-flto=full\|thin\|auto\|jobserver`, `-fno-lto`, `-f[no-]split-lto-unit` (WP-873) | the last of `-flto[=...]` / `-fno-lto` decides (clang `getLTOMode`: `auto` and `jobserver` are full LTO; another value is `unsupported argument`). The module is an LTO unit (spec/codegen.md "LTO": the `ThinLTO` / `EnableSplitLTOUnit` module flags and vtable `!type` metadata); `opt` runs the level's pre-link pipeline (`lto-pre-link<Ox>` / `thinlto-pre-link<Ox>`, at `-O0` too) instead of `default<Ox>`. `-c` (and each compile before a link) writes a bitcode-only `.o` through `opt`: full LTO with `-module-summary`, thin LTO with `--thinlto-bc` (plus `--thinlto-split-lto-unit` under `-fsplit-lto-unit`, default off for thin, always on for full), as clang; `-S -emit-llvm` carries the summary too; `-S` alone writes textual IR as clang does. `-flto[=thin]` is passed on to the link (lld or the system linker with the LLVM gold plugin). `-disable-llvm-passes` runs no pipeline and writes no summary. `-ffat-lto-objects` is accepted and ignored (the object stays bitcode only; follow-up), `-fno-fat-lto-objects` is the default |
| `-mbranch-protection=<spec>`, `-msign-return-address=none\|non-leaf\|all` (AArch64; WP-846) | `CodegenOptions::branch_protection` (`kiln_abi::BranchProtection`, clang 19 `CollectARMPACBTIOptions` and `ARM::parseBranchProtection`): the last of the two wins. `<spec>` is `none`, `standard` (`pac-ret+bti+gcs`) or a `+`-separated list of `bti`, `gcs` and `pac-ret` followed by any of `leaf`, `b-key`, `pc`; `-msign-return-address=` signs with the A key only. Every definition, declaration and call site gets clang's `"sign-return-address"`/`"sign-return-address-key"`, `"branch-target-enforcement"`, `"branch-protection-pauth-lr"` and `"guarded-control-stack"` attributes, the module gets the matching `Min` flags (value 1; 2 at `-fclang-compat=22`, as clang 22), and the predefines gain `__ARM_FEATURE_PAC_DEFAULT` (bit 0 A key, 1 B key, 2 leaf, 3 PAuth-LR), `__ARM_FEATURE_BTI_DEFAULT` and `__ARM_FEATURE_GCS_DEFAULT` (spec/pp.md "AArch64 target macros"). `__attribute__((target("branch-protection=<spec>")))` replaces the function's attributes (an invalid spec is `invalid or misplaced branch protection specification '<item>'`; off AArch64 the attribute is ignored with `unsupported 'branch-protection' in the 'target' attribute string`). An invalid spec is `unsupported argument '<item>' to option '-mbranch-protection='`; off AArch64 `-mbranch-protection=` is `unsupported option '-mbranch-protection=' for target '<triple>'` and `-msign-return-address=` is unused. kiln models no `-march=` extension enabling PAuth-LR, so `standard` never adds `pc` (spec/landed/codegen/WP-846.md) |
| `-Wp,<a>,<b>,...`, `-Xpreprocessor <arg>` | spliced into the argument list before parsing (`codegen_flags::splice_preprocessor_args`), as GCC: `-Wp,-MD,<f>` / `-Wp,-MMD,<f>` become `-MD -MF <f>` / `-MMD -MF <f>`, any other list is split at commas (empty pieces dropped), so `-Wp,-D_FORTIFY_SOURCE=2` is `-D_FORTIFY_SOURCE=2`; `-Xpreprocessor <arg>` is `<arg>` |

**2. Accepted and ignored silently** (no observable effect in kiln today;
do not add a diagnostic): `-f[no-]stack-clash-protection`,
`-f[no-]common`, `-fstack-usage`,
`-fno-short-enums`, `-m64` (every kiln target is 64-bit;
`-m32` stays unknown) and `-pipe`.
WP-677 (configure scripts and CMake probe them; clang 19 accepts them):
`-fdiagnostics-show-template-tree` (kiln prints template diffs inline,
as clang without it), `-f[no-]vectorize`, `-f[no-]slp-vectorize`,
`-f[no-]tree-vectorize`, `-f[no-]tree-slp-vectorize` (clang turns the loop
and SLP vectorizers on or off in its pass pipeline; kiln runs `opt`'s
default pipeline, which decides itself by `-O` level) and
`-f[no-]strict-vtable-pointers` (clang adds `invariant.group` loads and
`launder.invariant.group` barriers; deliberately ignored: the IR without
them is valid, only less optimized). Each is a deviation from clang's IR
for that flag (WP-873 follow-ups). WP-982: `-f[no-]modules-reduced-bmi`
(clang 18+ writes a smaller BMI; kiln's module file has one form, the
source capsule, spec/modules.md §11.9). An empty `-mtune=` is `unsupported argument '<v>' to option '<opt>'`.
(`-fexcess-precision=`, `-f[no-]math-errno` and `-ffp-model=` are in
"Floating-point flags", WP-806.)

**3. Rejected** with `drv_unsupported_flag`, the error `'<flag>' is not
supported by kiln` (exit 1, no job runs; not the generic `unknown argument`):
`-fshort-enums`, the `-fsanitize*` and `-fno-sanitize*`
options "Sanitizers" below does not list (`-fsanitize-coverage=`,
`-fsanitize-cfi-*`, ...), and under
`-fxray-instrument` every `-fxray-*` option but the threshold and
`-fxray-ignore-loops` (`-fxray-modes=`, `-fxray-always-instrument=`, ...;
WP-813). The coverage and profile flags are in "Coverage flags" (WP-941).

### Debug information flags (WP-814)

`DebugFlags` (`crates/kiln_driver/src/debug_flags.rs`) reads the debug
flags as clang 19's driver does on Linux and gives codegen its
`DebugOptions` (spec/codegen.md §5.1, spec/landed/codegen/WP-814.md).
Every `-g` flag is implemented, accepted as clang's default, or an error.

- **Level** (the last one wins): `-g`, `-g2`, `-g3`, `-ggdb`, `-ggdb2`,
  `-ggdb3`, `-gfull`, `-gused` give clang's default kind (`Constructor`:
  limited debug information with constructor type homing); `-g1`,
  `-ggdb1`, `-gmlt`, `-gline-tables-only` give line tables only;
  `-gline-directives-only` gives `DebugDirectivesOnly`; `-g0`, `-ggdb0`
  and no flag give none. `-gdwarf-2`..`-gdwarf-5` and `-gdwarf` (5) set
  the `"Dwarf Version"` module flag and also enable `-g`;
  `-fdebug-default-version=N` (2..5) sets the version when no
  `-gdwarf-N` does.
- **Kind modifiers:** `-fstandalone-debug` / `-fno-limit-debug-info`
  (every used type in full) and `-fno-standalone-debug` /
  `-flimit-debug-info`, the last wins;
  `-fno-eliminate-unused-debug-types` (also unused types);
  `-g[no-]column-info`.
- **Paths:** `-fdebug-prefix-map=<old>=<new>` and
  `-ffile-prefix-map=<old>=<new>` remap file and directory names (clang
  `remapDIPath`: the last matching entry wins); a value without `=` is
  `invalid value`. `-fdebug-compilation-dir=` /
  `-ffile-compilation-dir=` name the compilation directory (default:
  the working directory).
- **Output:** `-gsplit-dwarf[=split]` writes the `.dwo` next to the
  object (`llc -split-dwarf-file=` / `-split-dwarf-output=`; clang's
  names: `<output stem>.dwo`, or `<a.out stem>-<input stem>.dwo` when
  linking), `-gno-split-dwarf` turns it off. `-gz`, `-gz=zlib`,
  `-gz=zstd` compress the object's debug sections (`llc` cannot, so
  `objcopy --compress-debug-sections=` runs on the object after `-c`, and
  the link gets `-gz=<type>`; `.s` output is not compressed); `-gz=none` turns it off; another value is `unsupported
  argument`. Every `llc` run under `-g` gets `-debugger-tune=gdb`, as
  clang on Linux.
- **Accepted with no effect** (each spells clang's Linux default):
  `-gcolumn-info`, `-gdwarf32`, `-gno-embed-source`, `-gno-codeview`,
  `-gno-pubnames`, `-gno-gnu-pubnames`, `-ggnu-pubnames-none`,
  `-gno-simple-template-names`, `-gno-record-command-line`,
  `-gno-record-gcc-switches`, `-gno-strict-dwarf`, `-gno-modules`,
  `-gno-inline-line-tables`, `-gno-template-alias`,
  `-feliminate-unused-debug-types`, `-fno-debug-macro`,
  `-fno-debug-types-section`, `-fno-split-dwarf-inlining`,
  `-f[no-]dwarf-directory-asm`.
- **Rejected** with `'<flag>' is not supported by kiln` (exit 1):
  `-gembed-source`, `-gcodeview`, `-gcodeview-ghash`, `-gpubnames`,
  `-ggnu-pubnames`, `-gsimple-template-names[=simple|mangled]`,
  `-grecord-command-line`, `-grecord-gcc-switches`, `-gstrict-dwarf`,
  `-gdwarf64`, `-gmodules`, `-glldb`, `-gsce`, `-gdbx`,
  `-gtemplate-alias`, `-ginline-line-tables`, `-gsplit-dwarf=single`,
  `-fdebug-macro`, `-fdebug-types-section`, `-fsplit-dwarf-inlining`,
  `-fdebug-info-for-profiling`.

Tests: `debug_flags.rs` unit tests; `crates/kiln_driver/tests/driver.rs`
`debug_info_flags_reach_the_ir`.

### Floating-point flags (WP-806)

`FpFlags` (`crates/kiln_driver/src/fp_flags.rs`) records the FP flags in
command-line order and runs clang's `RenderFloatingPointOptions` state
machine over them (clang 19's, or clang 22's at `-fclang-compat=22`).
The result, `FpModel`, gives codegen its `FpOptions`
(spec/codegen.md §3.2, spec/landed/codegen/WP-806.md), the predefined
macros, the `llc` flags and the link's `crtfastmath.o`. Every flag is
implemented or an error; none is accepted and ignored.

- **Implemented:** `-f[no-]fast-math`, `-f[no-]unsafe-math-optimizations`,
  `-f[no-]finite-math-only`, `-f[no-]honor-infinities`,
  `-f[no-]honor-nans`, `-f[no-]approx-func`, `-f[no-]math-errno`,
  `-f[no-]associative-math`, `-f[no-]reciprocal-math`,
  `-f[no-]signed-zeros`, `-fno-trapping-math`, `-fno-rounding-math`,
  `-ffp-contract=on|off|fast|fast-honor-pragmas`,
  `-ffp-model=precise|fast` (and `aggressive` at compat 22),
  `-ffp-exception-behavior=ignore`, `-ffp-eval-method=source`,
  `-fexcess-precision=fast|standard`, `-fdenormal-fp-math=<m>[,<m>]`
  (`ieee`, `preserve-sign`, `positive-zero`, `dynamic`: the
  `denormal-fp-math` attribute), and `-Ofast` when it is the last `-O`
  (`-ffast-math`). Another value is `unsupported argument '<v>' to
  option '<opt>'` (`-fdenormal-fp-math=`: `invalid value`).
- **The state machine.** `-ffast-math` (and `-Ofast`) sets every
  value-changing flag, no `errno`, `contract` fast and the basic complex
  range. Each part sets its own field. `-fno-fast-math` /
  `-fno-unsafe-math-optimizations` restore the defaults and the last
  `-ffp-contract=` (else `on` after a fast-math flag). Every
  `-ffp-model=` starts from the defaults (rounding and trapping stay):
  `precise` is them, `fast` is `-ffast-math`, `strict` adds trapping,
  rounding and `contract` off.
- **clang 22 (`-fclang-compat=22`).** `-ffp-model=fast` keeps NaNs and
  infinities (no `__FAST_MATH__`, no `__FINITE_MATH_ONLY__`) and selects
  promoted complex division. `-ffp-model=aggressive` is clang 19's
  `fast`; at compat 19 it is `unsupported argument 'aggressive' to option
  '-ffp-model='`. Promoted division is not implemented. When it is the
  final complex range, it is the error `'-ffp-model=fast (promoted
  complex arithmetic; add -fcomplex-arithmetic=basic)' is not supported
  by kiln`. Any later complex-range flag avoids it.
- **The floating-point environment.** Each flag whose final effect needs
  it is the error `'<flag>' is not supported by kiln`
  (`drv_unsupported_flag`, one per flag, exit 1):
  - `-ftrapping-math`, `-ffp-exception-behavior=maytrap|strict`;
  - `-frounding-math`;
  - `-ffp-model=strict`;
  - `-ffp-eval-method=double|extended`;
  - `-fexcess-precision=16`.
  A later flag that undoes the effect avoids the error: `-frounding-math
  -fno-rounding-math` is accepted, and so is `-ffp-model=strict
  -fno-rounding-math -ffp-exception-behavior=ignore`. A later
  `-ffp-model=precise` does not undo it, as in clang.
- **Macros.**
  - `__FAST_MATH__`: every fast-math flag (finite, no `errno`, unsafe).
  - `__FINITE_MATH_ONLY__ 1`: no infinities and no NaNs.
  - `__NO_MATH_ERRNO__`: `-fno-math-errno`.
  - AArch64 `__ARM_FP_FAST`: unsafe math.
  - `__FLT_EVAL_METHOD__` stays 0.
- **Backend.** Function attributes carry the semantics. `llc` gets
  `-fp-contract=fast` only for exactly `-ffp-contract=fast` (clang's
  `AllowFPOpFusion`; `fast-honor-pragmas` does not), except x86 with
  LLVM 22 or newer: that `llc` warns its X86 backend ignores the option
  and fuses on the IR's `contract` flags, which every FP operation
  carries under `-ffp-contract=fast` (the object code is the same;
  WP-1016).
- **Link and denormals.** The link passes the last of `-ffast-math`,
  `-funsafe-math-optimizations` and `-ffp-model=fast|aggressive` (unless
  a negation follows) to `clang++`, which adds GCC's `crtfastmath.o`.
  The denormal mode comes only from the last `-fdenormal-fp-math=`; no
  other flag changes it, and every target defaults to IEEE (no
  attribute). This holds on x86-64 too when `crtfastmath.o` is linked:
  clang 19 and 22 then emit no `denormal-fp-math` (checked with a fake
  GCC installation).
- **Tests:**
  - `kiln_driver` `fp_flags` unit tests;
  - `tests/driver.rs` `fast_math_x86_64_crtfastmath_keeps_ieee_denormals`;
  - `tests/codegen.rs` `fp_environment_flags_rejected`;
  - `tests/codegen/fp_*.cpp`.

### Sanitizers (WP-675)

`SanitizeFlags` (`crates/kiln_driver/src/sanitize.rs`; design:
docs/design/sanitizers.md) reads the sanitizer options as clang 19's
`SanitizerArgs` does on Linux and gives codegen a `SanitizeOptions`
(enabled, recover and trap sets, plus the per-sanitizer options):

- `-f[no-]sanitize=`, `-f[no-]sanitize-recover[=]`,
  `-f[no-]sanitize-trap[=]`, `-f[no-]sanitize-undefined-trap-on-error`:
  comma lists read left to right, with clang's groups (`undefined`,
  `integer`, `shift`, `bounds`, `all`), its silent drops (vptr under
  `-fno-rtti`, object-size at `-O0`), its default recover set and its
  errors: `unsupported argument '<v>' to option '<opt>'` for an unknown
  name or one the list may not take, `invalid argument '<a>' not allowed
  with '<b>'` for an incompatible pair (each side the last argument that
  enabled it), `unsupported option '<opt><v>' for target '<triple>'`.
- Sanitizers kiln implements: address, hwaddress, thread, memory, leak,
  fuzzer, fuzzer-no-link, the `undefined` group, unsigned-integer-overflow
  and unsigned-shift-base. Any other clang sanitizer (cfi, dataflow,
  safe-stack, kcfi, memtag, the kernel variants, ...) is
  `drv_unsupported_flag` (`'-fsanitize=<x>' is not supported by kiln`).
  Of `undefined`, codegen emits signed-integer-overflow,
  integer-divide-by-zero, shift-base, shift-exponent, return,
  unreachable, vla-bound, null, alignment, object-size, array-bounds,
  bool, enum, float-cast-overflow, pointer-overflow, nonnull-attribute,
  returns-nonnull-attribute and builtin checks, and the implicit
  conversion checks (spec/codegen.md §5.2; WP-675, WP-871). vptr,
  function, the nullability checks and local-bounds are accepted and
  emit no check yet (spec/landed/codegen/WP-871.md).
- `-f[no-]sanitize-merge[=<list>]` (WP-1032, clang 20+): the checks whose
  traps and handler calls may merge, read like the recover list (no
  value is `all`; default the `undefined` group at `-fclang-compat=22`,
  every check at 19). A check outside the set gets its own trap block
  even above `-O0`, and its trap or handler call `nomerge`; `nomerge`
  is emitted at `-fclang-compat=22`, and at 19 once either flag is
  given.
- `-fsanitize-ignorelist=<f>`/`-fsanitize-blacklist=<f>` (WP-1032;
  `kiln_codegen::ignorelist`, clang's `NoSanitizeList`): LLVM special
  case lists, `prefix:glob[=category]` lines under optional
  `[<sanitizer glob>]` sections (none: every sanitizer), `#` comments,
  LLVM's `GlobPattern` (`*`, `?`, `[...]`, `{a,b}`, `\`). Every file is
  read; a later `=sanitize` category entry takes an earlier match back
  (clang 20+ precedence, at `-fclang-compat=22`). What each prefix
  suppresses, per sanitizer the section names:
  - `fun:<mangled name>`, `src:<file>` (the presumed file of the
    function's expansion location) and `mainfile:<file>`: every
    instrumentation of the function, as `no_sanitize`: its `sanitize_*`
    attributes and its UBSan checks;
  - `global:<mangled name>`, `src:`, `mainfile:` and `type:<record>` (the
    canonical, fully scoped class of the variable, arrays drilled) on a
    global: `no_sanitize_address`/`no_sanitize_hwaddress`, and memtag;
    `=init` on any of them (under `[address]`) drops only
    `sanitize_address_dyninit`;
  - at `-fclang-compat=22`, `type:<integer type>` also elides the
    signed/unsigned-integer-overflow checks of arithmetic in that type
    and the implicit truncation and sign-change checks into it (clang
    20+ `isTypeIgnoredBySanitizer`).
  The resource directory's `share/asan_ignorelist.txt`,
  `hwasan_ignorelist.txt` and `msan_ignorelist.txt` (clang's, shipped in
  `resource/share/`) load first for their sanitizer, when present;
  `-fno-sanitize-ignorelist` drops those and every earlier
  `-fsanitize-ignorelist=`. Errors, as clang 19: a missing file is
  `no such file or directory: '<f>'`, a malformed one `malformed
  sanitizer ignorelist: 'error parsing file '<f>': <LLVM message>'`
  (`malformed line <n>: '<line>'`, `malformed section header on line
  <n>: <text>`, `malformed glob in line <n>: '<glob>': <reason>`, ...),
  and a `#!special-case-list-v1` (regular expression) list is
  `drv_regex_sanitizer_ignorelist`, `sanitizer ignorelist '<f>' uses
  regular expressions ('#!special-case-list-v1'), which kiln does not
  support; use glob patterns` (a deviation; clang 19 reads them).
- `-f[no-]sanitize-address-use-after-scope`,
  `-fsanitize-address-use-after-return=never|runtime|always`,
  `-f[no-]sanitize-memory-track-origins[=0|1|2]`, `-shared-libsan`,
  `-static-libsan`.
- The pass sanitizers (address, hwaddress, thread, memory) run clang's
  `opt` pipeline extension points at every `-O` level, with clang's
  `cl::opt` flags. The link passes the final set on to the linking
  `clang++` (`-fsanitize=<enabled>`, plus `-shared-libsan`/`-static-libsan`),
  so the runtime is the one in its resource directory; with every
  enabled UB check in trap mode and no pass sanitizer, nothing is passed
  on and no runtime is linked.
- `__has_feature(address_sanitizer)` and friends follow the enabled set;
  `__SANITIZE_ADDRESS__`, `__SANITIZE_THREAD__` and
  `__SANITIZE_HWADDRESS__` are predefined from `-fclang-compat=22`.

Tests: `sanitize.rs` and `kiln_codegen::ignorelist` unit tests;
`crates/kiln_driver/tests/codegen.rs` `sanitizer_frontend_attributes`,
`sanitizer_ignorelist_asan`, `sanitizer_ignorelist_errors`;
`tests/conformance/driver.sanitize/` (the `*-run.cpp` tests that link
a runtime need `requires: sanitizer-runtime`, spec/harness.md).

### Coverage flags (WP-941, WP-1003)

`CoverageFlags` (`crates/kiln_driver/src/coverage_flags.rs`) reads the
coverage flags as clang 19's driver does (`addPGOAndCoverageFlags`) and
gives codegen its `CoverageOptions` (spec/codegen.md "Instrumentation",
spec/landed/codegen/WP-941.md). Each flag below is implemented or an
error; none is ignored.

| flag | kiln |
|---|---|
| `--coverage` (`-coverage`) | gcov notes and counters: `-fprofile-arcs -ftest-coverage` |
| `-f[no-]profile-arcs` | gcov counters (`.gcda`); `opt`'s pass writes the notes too |
| `-f[no-]test-coverage` | gcov notes (`.gcno`); without counters, `drv_unsupported_flag` (`opt`'s gcov pass always writes both) |
| `-fprofile-dir=<dir>` | the `.gcda` file goes under `<dir>` (clang's absolute-path join) |
| `-fprofile-update=single\|atomic\|prefer-atomic` | atomic counter updates (`--gcov-atomic-counter`, `--instrprof-atomic-counter-update-all`); another value: `unsupported argument '<v>' to option '-fprofile-update='` |
| `-f[no-]profile-instr-generate[=<path>]` | source-based coverage counters (`llvm.instrprof.increment`), written at exit to `default.profraw`, `<path>`, or `LLVM_PROFILE_FILE` (which wins) |
| `-f[no-]coverage-mapping` | with `-fprofile-instr-generate`: the coverage mapping (`__llvm_covmap` / `__llvm_covfun`, read by `llvm-cov show` / `report` / `export`; spec/codegen.md §5.4); without it: `'-fcoverage-mapping' only allowed with '-fprofile-instr-generate'` |
| `-fcoverage-compilation-dir=<dir>` (also `-ffile-compilation-dir=`) | the mapping's compilation directory, its first filename (default: the working directory) |
| `-fcoverage-prefix-map=<old>=<new>` (also `-ffile-prefix-map=`) | rewrites the mapping's filenames after `.`/`..` removal; the last matching pair wins (clang `normalizeFilename`); without `=`: `invalid argument '<v>' to -fcoverage-prefix-map=` |
| `-fprofile-filter-files=`, `-fprofile-exclude-files=` | with `--coverage`: `drv_unsupported_flag` (the `opt` pass takes no filter); without: `only allowed with '--coverage'` |
| `-fprofile-generate[=<dir>]`, `-fcs-profile-generate[=<dir>]` | `drv_unsupported_flag`: "IR profile-guided optimization instrumentation is a known kiln 1.0 limitation" |
| `-fprofile-use[=]`, `-fprofile-instr-use[=]`, `-fprofile-sample-use[=]`, `-fauto-profile[=]`, `-fprofile-remapping-file=` | `drv_unsupported_flag`: "profile-guided optimization is a known kiln 1.0 limitation" |

A rejected profile-guided optimization flag reads
`'<flag> (<what> is a known kiln 1.0 limitation; for coverage use
--coverage or -fprofile-instr-generate)' is not supported by kiln`.
`-fprofile-generate` is classified with PGO, not coverage: clang turns it
into IR-level PGO instrumentation (`PGOInstrumentationGen`), whose
`.profraw` feeds `-fprofile-use` and that `llvm-cov` does not read.

- **gcov.** The `.gcno`/`.gcda` names are clang's: `${dumpdir}${base}`
  under `-dumpdir` (a compile-and-link defaults it to `<output>-`, `a-`
  without `-o`), else the `-o` output, else the input's base name; made
  absolute against the working directory, extension replaced. Codegen
  names them in `!llvm.gcov` and, with no `-g`, emits clang's
  location-tracking-only debug information (`emissionKind: NoDebug`), which
  `GCOVProfiling` needs; with `-g` the debug information is the `-g`
  one.
- **Passes.** `opt` runs `insert-gcov-profiling` (gcov) and `instrprof`
  (counters) at `--passes-ep-pipeline-start`, as clang's
  `GCOVProfilerPass` and `InstrProfilingLoweringPass`, at every `-O`
  level (`-O0` runs `default<O0>`). Under `-Xclang -disable-llvm-passes`
  no pass runs: gcov leaves only `!llvm.gcov`, and InstrProf the
  unlowered `llvm.instrprof.increment` calls (without
  `@__llvm_profile_filename`, which the lowering creates).
- **Mapping.** `-fcoverage-mapping` sets `CoverageOptions::mapping`
  and collects, through `kiln_pp::PpCallbacks`, the skipped groups and
  the macro invocations the mapping needs; codegen does the rest. The
  flag changes nothing but the mapping globals: without it the IR is
  the same, byte for byte.
- **Link.** The linking `clang++` gets `--coverage` and/or
  `-fprofile-instr-generate`, so it adds the profile runtime
  `<resource-dir>/lib/linux/libclang_rt.profile-<arch>.a` (from the
  LLVM 19 compiler-rt package, `libclang-rt-19-dev` on Debian; without it
  the link fails with clang's "cannot find" error). Both gcov and InstrProf
  use the same runtime.

Tests: `coverage_flags.rs` unit tests; `crates/kiln_driver/tests/codegen.rs`
`coverage_flags_rejected`, `coverage_mapping_end_to_end` (compile, run,
`llvm-profdata merge`, `llvm-cov report/show` at `-fclang-compat=19` and
`22` with that LLVM's tools); `tests/codegen/{gcov,instrprof,coverage}_*.cpp`
(IR against clang, `ir_vs_clang`); `scripts/coverage-compare.py`
(end-to-end `.gcov`, `llvm-profdata` and `llvm-cov export` equality with
clang).

## Driver modes

One binary, two names, as clang/clang++ (WP-244). Cargo cannot build a
binary target named `kiln++` (a target name must be a crate name), so
`kiln_driver`'s build script makes `kiln++` a relative symlink to `kiln`
in the profile directory (`target/<profile>/kiln++ -> kiln`); xtask's
`build_kiln` makes it again if it is missing and returns its path. The
build script adds no link arguments: WP-653's `-z pack-relative-relocs`
(RELR) was reverted by WP-662: it tied the `kiln` binary to GNU ld 2.38+ /
lld 15+ and, at run time, glibc 2.36+ (`GLIBC_ABI_DT_RELR`), and failed on
the first older glibc it met, for about 0.2–0.5 MB of start-up RSS. Any
future link flag that constrains the build or run-time environment needs
the operator's explicit approval first.

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

A third mode is the dependency scanner: a basename `kiln-scan-deps`,
`clang-scan-deps` or `clang-scan-deps-<version>` runs
`kiln_driver::scan_deps` instead of the compiler ("Dependency scanning";
the build script makes these links too, WP-982).

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

## Installation layout

`cargo xtask dist` (spec/harness.md "Dist") packages kiln as a
relocatable tree, unpacked anywhere as `<prefix>` (WP-927):

```
<prefix>/bin/kiln
<prefix>/bin/kiln++ -> kiln              relative symlink ("Driver modes")
<prefix>/bin/kiln-scan-deps -> kiln      the dependency scanner, also as
<prefix>/bin/clang-scan-deps-22 -> kiln  the names CMake looks for
<prefix>/bin/clang-scan-deps -> kiln     ("Dependency scanning")
<prefix>/lib/kiln/<major>/include/       resource/include: clang 19's builtin
                                         headers, contracts, module.modulemap
<prefix>/lib/kiln/<major>/predefines/    resource/predefines/*.txt
<prefix>/lib/kiln/<major>/share/         resource/share/*.txt: the default
                                         sanitizer ignorelists (WP-1032)
<prefix>/share/doc/kiln/                 README.md, CHANGELOG.md, CONTRIBUTING.md,
                                         SECURITY.md, LICENSE-MIT, LICENSE-APACHE,
                                         COMPATIBILITY.md, KNOWN_ISSUES.md,
                                         STABILITY.md, USER_GUIDE.md (flat; links
                                         rewritten, spec/harness.md "Dist")
```

- `<major>` is the major version ("Version"): two major versions install
  side by side under one prefix, each binary finding its own resource
  directory. `lib/kiln/<major>` is the resource directory: `kiln
  -print-resource-dir` prints it ("Toolchain", step 3), and it holds what
  a source checkout's `resource/` holds except the test-only reference
  data (`include-paths/`, `predefines/structure.diff`).
- Nothing else is read from the tree, and nothing in it names the build
  host: the `dist` build compiles the source-tree fallback out and remaps
  the checkout, target directory, `$CARGO_HOME` and `$RUSTUP_HOME`
  prefixes in panic locations and debug information
  (`--remap-path-prefix` to `/kiln`, `/kiln/target`, `/cargo`, `/rustup`).
  `--version`'s `InstalledDir:` is the directory the executable was run
  from.
- `bin/kiln` is stripped of debug information but keeps its symbol table
  (so backtraces and the ICE report name functions) and carries a
  `.gnu_debuglink` to `kiln.debug`. The `-symbols` archive is the same
  tree with the unstripped binary; `objcopy --only-keep-debug
  <symbols>/bin/kiln <prefix>/bin/kiln.debug` makes the file the link
  names (its CRC matches; `cargo xtask dist --check` checks it), or the
  symbols tree can be used directly.
- **Run-time dependencies.** kiln runs LLVM's tools and links through
  `clang++` ("Code generation", "Tools"): an LLVM 22 or 19 installation
  with `opt`, `llc`, `llvm-as` and `clang++`, found as "LLVM discovery"
  says (docs/COMPATIBILITY.md "Runtime dependencies" is the user-facing
  list). The C++ standard
  library is the system's libstdc++ (GCC's installation, "Toolchain") or
  libc++ (`-stdlib=libc++`, from the LLVM directory). The binary itself
  needs only the C library: the newest glibc symbol version it references
  (`readelf -V bin/kiln`) is the floor, GLIBC_2.39 for the aarch64 build
  of WP-927 (Rust's `std::process` uses `pidfd_spawnp`), built in the CI
  container (Debian trixie, glibc 2.41).

## Toolchain

`Toolchain::new(target, sysroot, resource_dir, llvm_dir)` locates:

- **Resource directory:** the first of (WP-927; "Installation layout"):
  1. `-resource-dir=<dir>` (or `-resource-dir <dir>`);
  2. `$KILN_RESOURCE_DIR`, when set and not empty;
  3. the install layout's `<prefix>/lib/kiln/<major>`, where `<prefix>` is
     the parent of the directory holding the executable after resolving
     every symlink (`std::env::current_exe`, then canonicalized: a
     `kiln++` symlink, a symlink to it on `PATH` and a hard link all find
     the tree they belong to), when it has a `predefines/` directory;
  4. the nearest `resource/` with a `predefines/` directory above the
     executable (a source checkout whose target directory is inside it);
  5. in a source build, the source tree's `resource/` (compiled in from
     `CARGO_MANIFEST_DIR`). A `dist` build (`kiln_driver`'s cargo feature
     `dist`, which `cargo xtask dist` sets) has no such path, so that a
     release binary never holds or prints the build host's checkout; its
     last resort is step 3's directory even though it is missing, so that
     the error names where the files belong.

  `toolchain::default_resource_dir` is steps 2-5;
  `toolchain::install_resource_dir(exe)` is step 3's path. kiln's
  predefines base files are `<resource>/predefines/<triple>.<std>.txt`
  and its builtin headers `<resource>/include`. Test:
  `crates/kiln_driver/tests/install_layout.rs`.
- **LLVM:** the installation "LLVM discovery" (below) selects
  (`Toolchain::llvm`, a `llvm::LlvmSelection`). This is where kiln's
  tools and libc++'s headers are found. Its major (`Toolchain::llvm_major`;
  22 when none was found, `llvm::DEFAULT_LLVM_MAJOR`) selects the IR that
  differs between LLVM 19 and 22 (spec/codegen.md, coroutines and §6).
- **GCC:** clang 19/22 `GCCInstallationDetector::init`. The prefix is
  `--gcc-toolchain=<dir>` (a trailing `/` dropped) if given, else
  `<sysroot>/usr`. Under it, for `lib64` then `lib` and the target's
  candidate triples in clang's order, every `<prefix>/<lib>/gcc/<triple>/<version>`
  whose name parses as a GCC version is a candidate (`-v`: `Found
  candidate GCC installation: <path>` for each, sorted, as clang's
  `CandidateGCCInstallPaths` set); the newest with a `crtbegin.o` and a
  version of at least 4.1.1 is selected (ties: the first found;
  `Selected GCC installation: <path>`). `--gcc-install-dir=<dir>` skips
  the search and names the installation (triple and version are the
  names of `<dir>`'s parent and of `<dir>`). Test:
  `crates/kiln_driver/tests/driver.rs` `gcc_toolchain_matches_clang` (a
  fake tree; `-v` compared with clang 19). A build with
  `KILN_DEFAULT_GCC=<driver>` in its environment (`toolchain::DEFAULT_GCC`;
  for a distribution whose C++ compiler is a GCC outside `/usr` behind a
  wrapper, such as RHEL's gcc-toolset) instead takes, when there is no
  `--sysroot`, `--gcc-toolchain` or `--gcc-install-dir`, the `install:`
  directory of `<driver> -print-search-dirs` as if it were given to
  `--gcc-install-dir` (no candidates; the link passes it to `clang++`), and
  falls back to the search above if the driver fails or names another
  target's installation.

The search list is the user's directories in command-line order, then the
toolchain's (clang 19 `Linux::AddClangCXXStdlibIncludeArgs` followed by
`AddClangSystemIncludeArgs`):

1. C++ only, unless `-nostdinc++` or `-nostdlibinc`:
   - **libstdc++:** `<gcc>/../../../../include/c++/<v>`, then
     `.../<v>/<triple>` if it exists (Debian multiarch), else
     `.../<v>/<triple>/<multilib>` (listed even when missing, as clang
     `addLibStdCXXIncludePaths`), and `.../backward`. With
     `--gcc-toolchain=<gcc 16.1 root>/usr/local` this is GCC 16.1's own
     libstdc++ (docs/VALIDATION.md "GCC 16.1 header sweep").
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

### LLVM discovery (WP-1016)

kiln 1.0 supports LLVM **22 and 19** (`llvm::SUPPORTED_LLVM_MAJORS`, in
preference order; 22 is the default, operator decision Q6). Supporting
another major is one entry there plus its tests; LLVM 20 and 21 are
refused. `llvm::Search::run` takes the first valid candidate of:

1. `--llvm-dir=<dir>` (joined form only; the last one wins);
2. `$KILN_LLVM_DIR`, when set and not empty;
3. the build's `KILN_DEFAULT_LLVM_DIR` (a compile-time environment
   variable, for a distribution package whose LLVM lives elsewhere);
4. for each supported major `N` in preference order (all of 22's
   candidates before any of 19's):
   1. `llvm-config` on `PATH` when its installation has major `N`, then
      `llvm-config-N`; the installation is the directory above the one
      holding the tool after resolving symlinks (what `llvm-config
      --prefix` prints; nothing is run);
   2. `/usr/lib/llvm-N` (Debian, Ubuntu, apt.llvm.org),
      `/usr/lib64/llvmN` (Fedora's versioned compat packages),
      `/home/linuxbrew/.linuxbrew/opt/llvm@N`,
      `/home/linuxbrew/.linuxbrew/opt/llvm` (Homebrew on Linux), `/usr`
      (Fedora's main `llvm`/`clang` packages);
   3. `opt-N`, then `clang++-N`, on `PATH`: the installation above its
      resolved location.

A directory is a valid installation when `bin/opt`, `bin/llc` and
`bin/clang++` exist and its major is supported. The major
(`llvm::dir_major`) is the single numeric entry of `lib/clang/` (clang's
resource directory, so normally nothing is run), else the leading integer
of `bin/llvm-config --version` (run once per directory per process), else
the `N` of a directory named `llvm-N`, `llvmN` or `llvm@N` (before or
after resolving symlinks). The same directory is checked once.

- An explicit choice (1 or 2) that is not valid is an error before
  anything runs, whatever the action (`--help` excepted), exit status 1;
  the search does not fall through:
  `error: '<dir>' (from --llvm-dir) is not a usable LLVM installation: <why>`
  (`no such directory`, `no 'bin/llc'`, `cannot tell its version (...)`),
  or `error: unsupported LLVM major 21 at '<dir>' (from KILN_LLVM_DIR);
  kiln supports LLVM 19 and 22`.
- An implicit candidate that is not valid is skipped. When none is valid,
  the first command that needs a tool (a link, `-c`, a probe delegated to
  `clang++`) reports `error: no supported LLVM installation found (kiln
  supports LLVM 19 and 22)` with a note `skipped '<dir>': <why>` per
  directory that exists or was looked at, a note `not found on PATH:
  <names>`, and a note saying what to install and how to name an
  installation. Actions that need no tool (`-fsyntax-only`, `-E`,
  `-S -emit-llvm`, which writes LLVM 22 IR) still work, using
  `/usr/share/libc++/v1` for `-stdlib=libc++` headers.
- **`-fclang-compat` and the backend are independent.** The default
  `-fclang-compat` stays 22 whatever LLVM is selected: the compat level
  is what the program sees (`__clang_major__`, clang-version-specific
  semantics and diagnostics), the backend is how its IR is spelled
  (`Toolchain::llvm_major`), and every pairing works
  (`-fclang-compat=19` with LLVM 22 and `-fclang-compat=22` with LLVM 19
  are tested). Making the default follow the backend would change a
  program's meaning with whatever LLVM happens to be installed.
- `--version` and `-v` print the selection on the line after
  `InstalledDir:` ("`-v`, `--version`, `--help`").

Tests: `crates/kiln_driver/src/llvm.rs` (the search on fake trees: the
precedence, each step, per-major preference, skipped and explicit
invalid candidates, unsupported majors, nothing found, the messages) and
`crates/kiln_driver/tests/llvm_discovery.rs` (the driver with fake
installations through the flag and the variable: errors, precedence,
delegated probes, `-v`/`--version`).

## Target features

`features::resolve` (WP-459, WP-469) turns `-march=`, `-mcpu=`,
`-msve-vector-bits=` and `-target-feature` into the AArch64 target state
the way clang's AArch64 driver does (`getAArch64TargetFeatures`, LLVM's
`AArch64::ExtensionSet`); `run_as` reports its errors once, the `-E`,
`-fsyntax-only` and codegen paths read the result. Both oracles are
modelled: clang 22 by default, clang 19 at `-fclang-compat=19`.

- **Base.** The last `-march=` wins: an architecture level, then `+ext`
  modifiers. The levels are `armv8-a`, `armv8.1-a` ... `armv8.9-a`,
  `armv9-a`, `armv9.1-a` ... `armv9.5-a`, and with clang 22 also
  `armv9.6-a`/`armv9.7-a`; spelling is case-insensitive and the `-` before
  `a` optional (`armv8a`), as clang's. Each level enables its default
  extensions (those of every lower level, plus its own: `crc`, `lse`,
  `rdm` at v8.1, ..., `sve2` at v9.0, ...). Without `-march=`, the last
  `-mcpu=` is used: `generic` (armv8-a, adds `+ete,+trbe`) followed by
  `+ext` modifiers. When `-march=` is present, `-mcpu=` contributes nothing
  (clang uses the CPU only without an architecture).
- **Extensions** are case-insensitive and applied left to right (empty
  `+` segments are skipped): `+x` enables `x` and, transitively, its
  dependencies; `+nox` disables `x` if it is enabled and, transitively,
  everything depending on it (LLVM's `ExtensionDependencies`; e.g. `sve`
  needs `fullfp16`, which needs `fp`, so `+nofp` removes `sve`). The
  spellings are clang's (`fp`, `simd`, `fp16`, `crypto`, `crc`, `lse`,
  `rdm`, `rcpc`, `dotprod`, `bf16`, `i8mm`, `sve`, `sve2`, `sve2-aes`,
  ..., `memtag`, `rng`, `pauth`, `mops`, `gcs`, `d128`, ...; the table is
  `features::extension`). The special cases are LLVM's: `+crypto` is
  `aes`+`sha2`, and from v8.4 also `sha3`+`sm4`; `+fp16` on v8.4 .. v8.9
  also enables `fp16fml`; `+nocrypto` disables `aes`, `sha2`, `sha3` and
  `sm4`. clang 22 splits each `sve2-x` into `sve2` + `sve-x` (and accepts
  `sve-aes`, `sve-bitperm`, `sve-sha3`, `sve-sm4`, `sve2p2`, `sve2p3`,
  `fprcvt`, `f16f32dot`, ...), drops `tme`; clang 19 has no `sve-x`.
- **`-target-feature ±name`** applies after the base, in order, last
  wins, with no dependency closure; `+sve` alone adds only `+sve`. `-Xclang
  -target-feature -Xclang +sve` is accepted: an `-Xclang` before a
  separate value is skipped.
- **`-msve-vector-bits=N`** (`128`, `256`, `512`, `1024`, `2048` or
  `scalable`, the default) fixes the SVE vector length: `vscale_range(N/128,
  N/128)` on every definition and `__ARM_FEATURE_SVE_BITS N`, with or
  without `sve` (clang alike).
- **Result.** `TargetFeatures { sve, extra, arch, sve_vector_bits }`:
  - `sve` sets `ParseOptions::sve` / `Sema::sve` (SVE types usable,
    spec/types.md §5.1) and `CodegenOptions::sve` (`vscale_range(1,16)`,
    spec/codegen.md §7aa; at `-fclang-compat=19` only with NEON too, as
    clang 19's `SveMode`);
  - `extra` (the touched features, `+` if on, `-` if off, minus the
    base's `fp-armv8`, `neon` and `v8a`) overrides the default AArch64
    `"target-features"` entries (`CodegenOptions::target_features`), which
    print as clang's: sorted `+` entries, then sorted `-` entries;
  - `TargetFeatures::enabled()` with `arch` and `sve_vector_bits` is
    `PredefConfig::arm` (`kiln_pp::ArmTarget`), from which the
    `__ARM_ARCH`, `__ARM_FP`, `__ARM_NEON*` and `__ARM_FEATURE_*`
    predefines derive (spec/pp.md "AArch64 target macros").
- **Errors.** Any other architecture, CPU or extension (`armv8.0-a`,
  `armv7-a`, `cortex-a57`, `sme`, ...) is `unsupported argument '<v>' to
  option '-march='` (`-mcpu=` alike), and so is a `-msve-vector-bits=`
  value outside the list (`'<v>' to option '-msve-vector-bits='`).
  `native` is accepted (no features; "Codegen flags"). On
  x86-64 ("x86-64 target features" below), `-march=` takes `native` or
  one of clang's x86 CPU names and any other value is the same error;
  `-mcpu=` and `-msve-vector-bits=` are
  `unsupported option '<opt>' for target '<triple>'` (the
  `--target` as written; `drv_unsupported_opt_for_target`) and
  `-target-feature` is ignored. A `-target-feature` value without a sign
  is `unsupported argument`. An unknown `+name` is passed through to
  `"target-features"`.

## x86-64 target features (WP-1000)

On an x86-64 target `features::resolve` builds the feature map clang's
x86 driver and `X86TargetInfo::initFeatureMap` build, modelling clang 22
by default and clang 19 at `-fclang-compat=19`
(`crates/kiln_driver/src/x86_features.rs`; implication graph: the
generated clang 21 table `x86_targets` with the measured clang 19 / 22
differences).

- **Flags.** `-m<feature>` / `-mno-<feature>` for every name of clang's
  `m_x86_Features_Group`: the 104 both versions accept
  (`x86_features::FLAGS`: `sse`..`sse4.2`, `sse4a`, `ssse3`, `avx`,
  `avx2`, the `avx512*` family, `avxvnni*`, `avxifma`, `fma`, `fma4`,
  `f16c`, `bmi`, `bmi2`, `lzcnt`, `popcnt`, `pclmul`, `aes`, `sha`,
  `sha512`, `sm3`, `sm4`, `rdrnd`, `rdseed`, `adx`, `movbe`, `xsave*`,
  `cx16`, `crc32`, `gfni`, `vaes`, `vpclmulqdq`, `amx-*`, `apxf`, ...),
  clang 19's `avx10.1-256`, `avx10.1-512`, `evex512` and clang 22's
  `amx-avx512`, `amx-fp8`, `amx-movrs`, `amx-tf32`, `avx10.2`, `movrs`;
  a name the selected clang lacks is `unknown argument`. Also
  `-m[no-]apx-features=<list>` (`egpr`, `push2pop2`, `ppx`, `ndd`,
  `ccmp`, `nf`, `cf`, `zu`; any other value is `unsupported argument
  '<v>' to option '-m[no-]apx-features='`) and
  `-mapx-inline-asm-use-gpr32` (`+inline-asm-use-gpr32`).
- **Spellings.** As clang's driver: `-msse4` is `+sse4.2` and
  `-mno-sse4` `-sse4.1`; `-m80387` is `x87`; `-mapxf` is the APX list
  (clang 19: `egpr,push2pop2,ppx,ndd,ccmp,nf,cf,zu`; clang 22 without
  `cf`); clang 19's `-mavx10.1` is `avx10.1-256`; `-m3dnow` / `-m3dnowa`
  add nothing and warn `the clang compiler does not support '-m3dnow'`.
  The items are deduplicated as clang's `unifyTargetFeatures`: the last
  item naming a feature wins.
- **CPU.** `-march=<cpu>` (clang 19's list, plus clang 22's
  `wildcatlake`, `novalake`, `diamondrapids` at 22) gives the CPU's
  features: the clang 21 row with clang 19's / clang 22's differences
  (`x86_features::CPU_19` / `CPU_22`); `wildcatlake` and `novalake` are
  not expanded (deviation). `-march=native` on a Linux x86-64 host takes
  the features of `/proc/cpuinfo`'s first `flags` line clang knows, over
  `x86-64`'s; on another host none. Without `-march=` the base is
  `x86-64` (`cmov,cx8,fxsr,mmx,sse,sse2,x87`).
- **Map.** clang's `initFeatureMap`: `sse2`, then the CPU's features
  (each with what it implies), then each item (`+f` enables `f` and what
  it implies; `-f` disables `f` and every feature implying it), then
  `sse4.2` adds `popcnt` and `crc32`, `sse` `mmx` and `avx` `xsave`
  unless an item disabled that feature. clang 19 applies the `evex512`
  items after the others and adds `+evex512` when an AVX-512 feature is
  on (clang 22 has no `evex512`). `-mspeculative-load-hardening` adds
  `+retpoline-indirect-calls` ("Codegen flags").
- **Effects.** The map's macros (`x86_features::feature_macros`, clang's
  `getTargetDefines`: `__AVX2__`, `__SSE4_1__`, `__PCLMUL__`, `__SHA__`,
  `__SSE_MATH__`/`__SSE2_MATH__`, the `_Float16` macros with `sse2`,
  `__GCC_HAVE_SYNC_COMPARE_AND_SWAP_16` with `cx16`, `__LAHF_SAHF__`,
  clang 19's `__EVEX256__`/`__EVEX512__`, `__AVX10_*__`, `__APX_F__`
  with every APX feature) replace the default ones; the map is every
  function's `"target-features"` and the `-march=` CPU its
  `"target-cpu"`, as clang prints them (`CodegenOptions::target_features`,
  `target_cpu_features`, `target_features_written`). The intrinsic and
  builtin gating, `target("...")` attributes and the AVX calling
  convention checks of spec/codegen.md read the same map; a `target`
  attribute's map starts from the `arch=` CPU (else the `-march=` CPU),
  then the command line's items as written, then the attribute's own
  (clang `getFunctionFeatureMap`).
- **Other targets.** On AArch64 every x86 feature flag is `unsupported
  option '<flag>' for target '<triple>'` (`-mno-sse4` names
  `-mno-sse4.1`, `-m80387` `-mx87`, clang 19's `-mapxf`
  `-mapx-features=`), except `-msse4`, `-msse4.2` and `-m[no-]3dnow[a]`,
  which clang claims: `argument unused during compilation`. A non-x86
  `-march=` value on x86-64 (or an x86 CPU on AArch64) is `unsupported
  argument`.
- **Not modelled.** `-mgeneral-regs-only`, `-msoft-float`,
  `-mfp-ret-in-387`, `-mretpoline`, `-mretpoline-external-thunk`,
  `-mlvi-cfi`, `-mseses`, `-mx32` stay unknown arguments; `-Xclang
  -target-feature` is ignored; clang's `-Winvalid-feature-combination`
  is not emitted; a bad `-march=` / `-mtune=` CPU is the driver's
  `unsupported argument` (clang: cc1 `unknown target CPU` with a note),
  and a bad `-mtune=` is accepted.
- Tests: `tests/conformance/cpp.predefined/x86-feature-macros-*.cpp`
  (per CPU level and flag combination, `-19` siblings),
  `tests/conformance/target.x86/ir-target-features-*.cpp`,
  `target.x86/m-feature-flags.cpp`, `crates/kiln_driver/tests/codegen.rs`
  `x86_target_feature_gating`, `x86_avx_calling_convention`,
  `x86_flag_driver_diagnostics`, and the `features` / `x86_features`
  unit tests.

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
  - on AArch64, the target state of "Target features" (`PredefConfig::arm`;
    the `__ARM_*` target macros, spec/pp.md "AArch64 target macros");
  - `-stdlib=libc++`: no `__STDCPP_FLOAT16_T__` ... `__STDCPP_BFLOAT16_T__`
    (`std_float_aliases`; libc++ has no `<stdfloat>`, spec/pp.md P1467
    macros).
- Reflection on (the default at C++26 unless `-fno-reflection`, WP-920;
  never below C++26): `__cpp_impl_reflection 202603L` (GCC 16.1's value,
  `g++-16 -std=c++26 -freflection -dM -E`; libstdc++ 16's `<meta>` keys on
  it; spec/reflection.md §1 item 4-5).
- kiln adds `__KILN__ 1`, `__KILN_MAJOR__`, `__KILN_MINOR__` and
  `__KILN_PATCH__` (WP-927), the components of the version ("Version"). kiln
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

C inputs (`-x c`, `.c`) and
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
- **Tools.** Each tool is `<llvm>/bin/<tool>` (`opt`, `llc`, `llvm-as`,
  `clang++`), `<llvm>` being the installation "LLVM discovery" selects. A
  tool missing from it is `unable to execute command: Executable "<tool>"
  doesn't exist!`; with no installation found, the not-found error. The
  IR is piped to the tool's standard input (`-o <out> -`).
- **`llc`** gets `-O0` (default), `-O1` (`-O`, `-O1`, `-Og`), `-O2`
  (`-O2`, `-Os`, `-Oz`) or `-O3` (`-O3`, `-O4`, `-Ofast`), then
  `-relocation-model=static` with `-fno-pic`/`-fno-pie`, else
  `-relocation-model=pic` (clang's default PIE), then `-filetype=obj`
  unless `-S`, then `-function-sections` / `-data-sections`
  (`-ffunction-sections` / `-fdata-sections`) and `-mcpu=native`
  (`-march=native`, "Codegen flags").
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
    takes the unchanged `llc`-only path;
    this scan and the `always_inline` one below read only the IR before
    its first top-level `!` line (`code_part`, WP-817): the metadata
    section, half or more of a `-g` module, holds no attribute group or
    function;
  - at `-O0`, when the IR defines an `always_inline` function (an
    `attributes #N` line holds `alwaysinline`; `needs_always_inline`;
    WP-389), `-passes=default<O0>`: clang 19's whole O0 module pipeline
    (`buildO0DefaultPipeline`), i.e. `AlwaysInlinerPass` without
    lifetime markers (`-passes=always-inline` alone would insert them)
    followed by the coroutine lowering above when needed. clang runs it
    before `-emit-llvm` writes the module, so `-S -emit-llvm` gets every
    call to such a function inlined and a discardable one deleted
    (spec/codegen.md §6 "Always-inliner").

  The pass run writes bitcode, which a second `opt -discard-value-names
  -S` reads back so the local value names the passes made are dropped,
  as in clang's release pipeline (`opt` refuses `-discard-value-names`
  for textual input). For an object (`-c`, or the objects of a link)
  the second run is skipped and `llc` reads the bitcode directly
  (WP-397): value names do not reach an object, it saves a process and
  a text round trip (about half the `-O0` always-inliner's cost,
  spec/codegen.md §6 "Always-inliner"), and the bitcode's preserved
  use-list order makes `llc`'s output match clang's in-process codegen.
  `-S -emit-llvm`, `-c -emit-llvm` and `-S` keep both runs. `-disable-llvm-passes` (a cc1 option, usually
  spelled `-Xclang -disable-llvm-passes`) skips both runs, so
  `-emit-llvm` writes the frontend's IR (pre-split coroutines). `-v`
  prints the `opt` commands before the `llc` one. `-###` prints them
  after the frontend step at `-O1` and above (WP-783:
  `-passes=default<ON>`, then the `-discard-value-names -S` run unless
  the output is an object), exactly as `-v` shows them; at `-O0` the
  runs depend on what the generated IR declares and are not shown. An
  `opt` failure is a driver error like `llc`'s. Cost (WP-351, aarch64, LLVM 19, `-O0`): on
  libc++'s `end.to.end/generator.pass.cpp` (4.8k lines of IR) `opt`
  takes 0.02 s and 62 MB RSS, against 2.9 s for the whole debug-build
  `kiln++ -c`; `go.pass.cpp` 0.01 s.

  Lifetime markers (WP-462): LLVM 22 dropped the size operand of
  `llvm.lifetime.start`/`.end` (LLVM 21's verifier still requires it).
  At `-fclang-compat=22` with an LLVM 22+ backend codegen emits the new
  `(ptr captures(none))` form itself; with an older backend it emits the
  sized form (LLVM 19's `(i64 immarg, ptr nocapture)`, LLVM 21's
  `(i64 immarg, ptr captures(none))`), which that backend's `opt` and
  `llc` verify. **The IR kiln writes is the IR of its backend** (WP-783):
  `-S -emit-llvm` writes the same module the `-c` path hands to `llc`,
  in the spelling the selected LLVM's ("LLVM discovery") `opt`, `llc`
  and `llvm-as` read, whatever the `-fclang-compat`. (Before WP-783 the
  `-S -emit-llvm` text at `-fclang-compat=22` was rewritten to LLVM 22's
  sizeless markers even for an LLVM 19 or 21 backend, which those tools
  then rejected: `expected ')' at end of argument list`.) So the text
  equals clang 22's only with an LLVM 22 backend, the pairing the test
  harness uses (spec/harness.md "Oracle toolchain").
- **Link.** `clang++` with `--target=<triple>` (only if given),
  `--sysroot=`, `--gcc-toolchain=`, `--gcc-install-dir=` (as given, else
  the `KILN_DEFAULT_GCC` installation, see "Toolchain"), the `-O` flag, `-pthread`, `-stdlib=`,
  `-fexperimental-library` (if given), then the objects,
  linker inputs and link options in command-line order, then
  `-o <out>` (default `a.out`). The temporary objects are removed
  afterwards (by their directory's explicit path).
- **`-###`** prints, to standard error, the `--version` block, then one
  line per step (at `-O1` and above the `opt` runs follow the frontend
  step, "IR optimization"). An external step is clang's format: a leading space, then
  every argument quoted (`"`, `\` and `$` escaped). The frontend step is
  ` (in-process) frontend "<input>" -> "<out>"`, where `<out>` is `|` when
  the IR is piped to the next step. Nothing runs; the exit status is 0.
- **`-v`** prints the `--version` block and each external command before
  running it, quoted only where needed (`llvm::sys::printArg`). When
  linking, `clang++` also gets `-v` (first), so its own `-v` output —
  its banner and the linker's command line — follows: build systems read
  the implicit link directories and libraries from it (CMake's compiler
  ABI probe; WP-667).
- **Unused inputs.** In a compile-only phase, linker inputs and link
  options are unused, warned in command-line order with clang's wording
  (`-Wunused-command-line-argument`): `<x>: 'linker' input unused` for
  linker inputs, `-l`, `-Wl,` and `-Xlinker <arg>`; `argument unused during compilation:
  '<o>'` for `-L`, `-fuse-ld=`, `-shared`, `-rdynamic`, `-pie`, `-no-pie`.
  `-static` and `-nostdlib` are silent, as in clang. `-Qunused-arguments`
  (anywhere on the command line) silences all of them, as in clang
  (doctest's CMake build passes it; WP-683).
- **Errors.** `-o` with several sources in a compile-only phase is an
  error, as is `-emit-llvm` when linking. A tool that is missing, cannot
  start, fails or is killed is a driver error (below); the exit status is
  then 1.

Not implemented: `-###` with `-E` or `-fsyntax-only` ("not implemented
yet"). Debug information (`-g` and its levels, `-gdwarf-N`) is
implemented; see "Debug information flags" (WP-814).

## Modules

C++20 modules are enabled for `-fsyntax-only` and code generation from
`-std=c++20` on; `-E` and `-M*` print module and import directives as text
(spec/modules.md §2). An imported module comes either from its interface
source, built in-process (part A), or from a kiln module file written by
an earlier compile (spec/modules.md §11; WP-917). A module (or partition
`M:P`) is looked up in this order:

1. `-fmodule-file=<name>=<path>` (repeatable; the first one naming the
   module wins). The path is used as given (relative to the working
   directory) and may name a module file or a module source; which one is
   decided by the file's first bytes, not its extension.
2. `-fprebuilt-module-path=<dir>` (repeatable, in order): `<dir>/<name>.pcm`,
   then the sources `<dir>/<name>.cppm`, `.ixx`, `.cpp`; a partition `M:P`
   as `<dir>/M-P.*`.
3. A module that an imported module file itself imported, at the path that
   file recorded.
4. `std` and `std.compat` only: `<llvm>/share/libc++/v1/std.cppm` /
   `std.compat.cppm`, where `<llvm>` is the toolchain's LLVM directory
   ("LLVM discovery"; clang 19's and 22's libc++ have the
   same layout). The module file comes from the module cache ("Module
   cache"), which builds it from that source on a miss.

A module not found is `fatal error: module 'M' not found` at the module
name. Header units (`import "h";`, `import <h>;`) are found like
`#include` and need no flag.

Module files are clang's driver interface with kiln's own file format
(spec/modules.md §11.3; kiln never reads or writes clang's, and says so):

- `--precompile <unit> [-o <out>]` writes the module file of a module
  interface or partition unit (default `<stem>.pcm` in the working
  directory) and nothing else. `-x c++-module` or a `.cppm`/`.ixx`
  extension marks an input as C++ (the unit's `export module` line, not
  the extension, makes it a module unit); any other unit is `error: --precompile requires a module interface or
  partition unit`.
- `-fmodule-output[=<path>]` with `-c` (or a link) also writes the module
  file of each module interface input: at `<path>`, else next to the
  object (`-o x/m.o` -> `x/m.pcm`), else `<stem>.pcm`. On an input that is
  not an interface unit it is unused (`warning: argument unused during
  compilation: '-fmodule-output'`). The module file is the one
  `--precompile` writes, byte for byte.
- A module file is a compile input: `-c m.pcm` compiles the interface unit
  it holds, so `--precompile` then `-c` gives the object one `-c` gives.
- `-fmodule-file=<path>` (no name) is clang's deprecated form: the module
  file's own module name is read from it, with clang's warning
  `the form '-fmodule-file=<BMI-path>' is deprecated ...
  [-Weager-load-cxx-named-modules]`. A source there is an error.
- A module file records the configuration it was built with (standard,
  target, language and codegen options that change meaning; spec/modules.md
  §11.4). Importing it into a compile that differs is one `error: module
  file 'm.pcm' was built with '-std=c++20' but the current compilation uses
  '-std=c++23'` per differing option, then `fatal error: module file
  'm.pcm' cannot be loaded due to a configuration mismatch with the
  current compilation`. A missing, foreign (clang), wrong-module, or
  damaged module file is a fatal error naming it; never a crash.

To link a program, every module unit whose non-inline definitions are used
is also compiled as an input (or separately with `-c`, from its source or
its module file): `kiln++ -std=c++20 -fmodule-file=m=m.cppm m.cppm
main.cpp`, or in two steps `kiln++ -std=c++20 -c m.cppm -fmodule-output -o
m.o && kiln++ -std=c++20 -fmodule-file=m=m.pcm main.cpp m.o`. An importer
emits only declarations for the non-inline functions and variables of the
units it imports. `-fmodule-header` and header-unit module files are not
supported yet (spec/modules.md §11.10); `-fmodules-reduced-bmi` is
accepted and ignored ("Codegen flags").

### Module cache (WP-984)

`std` and `std.compat` found by step 4 above are imported from module
files in a per-user cache, built there on first use (spec/modules.md
§11.12 has the design):

- **Location.** `-fmodules-cache-path=<dir>` (made absolute), else
  `$XDG_CACHE_HOME/kiln/modules` when `XDG_CACHE_HOME` is an absolute
  path (the XDG rule: a relative one is ignored), else `$HOME/.cache/kiln/modules`. An entry is
  `<root>/<key>/<name>.pcm` plus `<name>.stamp`, where `<key>` is 32 hex
  digits (spec/modules.md §11.12 "Key").
- **Turning it off.** `-fno-kiln-module-cache` (`-fkiln-module-cache`
  turns it back on; the last one wins) or an empty
  `-fmodules-cache-path=`. Without a home directory there is no cache.
  The cache is also off for `kiln-scan-deps`, which imports nothing. A
  kiln-specific flag, not clang's `-fno-implicit-module-maps` family,
  because clang has no equivalent: clang never builds `std` implicitly.
  `-fmodules-cache-path=` is clang's spelling for the Clang-modules
  cache, which kiln reuses for the one cache it has.
- **Precedence.** Only step 4 consults the cache: `-fmodule-file=std=`
  or a `std.pcm` / `std.cppm` in a `-fprebuilt-module-path` wins, as
  without it.
- **Failure is never an error.** A cache directory that cannot be
  created, a module that fails to build there, or a stamp that cannot be
  written falls back to building the module from its source in-process
  (part A), which reports the module's own errors as before. Under `-v`
  each case prints one line on standard error
  (`kiln++: module cache: cannot create '<dir>': <reason>; building 'std'
  from its source`), as do a miss (`building 'std' into '<pcm>'`) and a
  hit (`using '<pcm>' for 'std'`).
- **Concurrency.** A hit takes no lock. Racing compiles on a miss take
  the entry's `<name>.lock` (an advisory lock the kernel releases when
  its holder dies); one builds and publishes, the others wait and use
  its module file. Every one of them succeeds, the entry ends as one
  `<name>.pcm` and one `<name>.stamp` (no temporary or lock file), and a
  killed compile leaves nothing the next one does not clear. A waiter
  gives up after 120 s (`KILN_MODULE_CACHE_WAIT_MS`) and builds the
  module from its source; `-v` says `waiting for another compile building
  'std'` and, then, `gave up waiting for '<lock>'; building 'std' from
  its source`, or `cannot lock '<lock>': <reason>; ...` (spec/modules.md
  §11.12 "Concurrency", WP-1074).
- **Dependency output.** An importer's `-MD`/`-MMD` rule lists the
  entry's module file (absolute), the same on a hit and a miss, and
  under `-MD` the entry's stamped inputs ("Dependency output", WP-1051).
- **Pruning** (WP-1050), as clang prunes its module cache. Every
  configuration, and every rebuilt kiln, has its own entry of about
  9 MiB. A compile using the cache checks `<root>/modules.timestamp`
  after its first lookup: created when missing, else, once older than
  the interval, updated, and every other entry unused for longer than
  prune-after is removed (spec/modules.md §11.12 "Pruning" has the
  rules, and how they differ from clang's).

  | flag | default | meaning |
  |---|---|---|
  | `-fmodules-prune-interval=<seconds>` | 604800 (7 days) | at most how often the cache is pruned; 0 never |
  | `-fmodules-prune-after=<seconds>` | 2678400 (31 days) | an entry unused this long is removed; 0 never |

  The values are clang's `unsigned` (decimal, `0x` hex, `0` octal, at
  most 4294967295); anything else is clang's `invalid integral value
  '<v>' in '<arg>'`. An entry's last use is the newest modification time
  of its files; a hit sets its stamp's to now when it is a day old or
  older (never the module file's, which dependency files list), where
  clang reads access times. An entry whose lock another compile holds is
  kept, as is anything in the root that is not an entry directory (32 hex
  digits), and links are not followed. Removal failures are silent;
  under `-v` the cache says `pruning '<root>'`, `removed unused
  '<entry>'` and `cannot remove '<path>': <reason>`. Removing the cache
  by hand (`rm -r ~/.cache/kiln/modules`) is always safe when no compile
  runs.
- `-print-file-name=libc++.modules.json` prints libc++'s module
  manifest for the libc++ that `-stdlib=libc++` selects
  (`<prefix>/lib/<triple>/` then `<prefix>/lib/`, where
  `<prefix>/include/c++/v1` holds the headers), so CMake's
  `CXX_MODULE_STD` finds `std.cppm` through its relative `source-path`
  (spec/modules.md §11.12 "CMake").

## Dependency scanning

`kiln-scan-deps` (WP-982; design, output and deviations in
spec/modules.md §11.9) is clang-scan-deps for CMake and other build
tools: `kiln-scan-deps [options] -- <compiler> <compile flags>` runs
kiln's preprocessor on the one compile after `--` and prints its module
dependencies. It is the `kiln` binary under another name ("Driver
modes"); `clang-scan-deps-22` and `clang-scan-deps` are the same links,
installed beside `kiln++` so CMake finds it without configuration;
`-DCMAKE_CXX_COMPILER_CLANG_SCAN_DEPS=<prefix>/bin/kiln-scan-deps`
names it explicitly. The compiler after `--` is parsed as `kiln++`
whatever its name (`clang++`, a path to `kiln++`); its flags are kiln's.

| option | effect |
|---|---|
| `-format=p1689` (`--format`, `-format p1689`) | P1689r5 JSON, as clang-scan-deps 22 |
| `-format=make` (the default) | the make rule `<obj>: <src> <headers>` |
| `-o <file>` | write the output to `<file>` instead of stdout |
| `-MT`/`-MD`/`-MF`/`-MMD`/`-MQ` in the command | the dependency file of the scanned files, after a clean scan ("Dependency output") |
| `-j <n>`, `-mode <m>`, `-resource-dir-recipe <r>`, `-optimize-args[=...]`, `-eager-load-pcm`, `-reuse-filemanager`, `-skip-excluded-pp-ranges`, `-brief-result`, `-round-trip-args` | accepted and ignored (clang-scan-deps tuning; one input is one scan) |
| `-compilation-database <f>`, `-format=experimental-full` | an error (not supported) |
| `-help`, `--help` | the option summary, exit 0 |
| anything else | `<prog>: Unknown command line argument '<a>'.  Try: '<prog> --help'`, exit 1 |

Exit status 0 after a clean scan, 1 otherwise (diagnostics under
`Diagnostics while scanning dependencies for '<src>':`, the empty
document on stdout). Warnings are not printed.

## Precompiled headers (WP-1023)

kiln accepts clang's precompiled-header (PCH) driver interface, so build
systems that precompile headers work unchanged. The main one is CMake's
`target_precompile_headers`, which uses `Compiler/Clang.cmake`'s flags.
**In 1.0 a kiln PCH is about correctness, not speed.** It is a *textual*
record. The consumer checks that the record is still valid and then
processes the header again, as if `-include`d. A PCH build is therefore
never faster than the same build without one. The CMake end-to-end check
(`scripts/oss/selftest.sh`) times both. A serialized-AST PCH, which would
store Oven state, is a follow-up; it would share the module-file format
(spec/modules.md §11).

### Flags

- **Producing a PCH.** These are the producers:
  - `-x c++-header <h>`, `-x c-header <h>`, or a header by extension
    (`.h` in C++ mode, `.hh`, `.hpp`, `.hxx`, `.H`), with `-c`, `-S` or no
    action;
  - any input with `-emit-pch`. This is a cc1 option, also accepted as
    `-Xclang -emit-pch`; CMake spells it that way, with input
    `cmake_pch.hxx.cxx` and `-x c++-header`.

  The header is checked exactly as `-fsyntax-only` checks it. Any error
  means no PCH is written and the exit status is 1. Where the PCH goes:
  - with `-o`, at that path, unless a link follows (non-header inputs on
    the same line);
  - otherwise `<input>.pch` for a header (clang's name);
  - otherwise, for `-emit-pch` on a non-header, the object's default name.

  `-o` with more than one output is clang's `cannot specify -o when
  generating multiple output files`. `-fsyntax-only`, `-E` and `-M`/`-MM`
  treat a header as an ordinary input, and produce nothing. A header is
  its own main file, so `#pragma once` in it gets no warning. C headers
  are recorded, but compiling C is "not implemented yet" (WP-243).
- **`-include-pch <file>`.** This is separate-only, as in clang:
  `-include-pchX` is `-include` with the value `-pchX`. It is also
  accepted as `-Xclang -include-pch -Xclang <file>`, and the last one
  wins. The record is validated (see Diagnostics) and its header is then
  processed as if `-include`d ([Replay](#replay)).
- **`-Xclang -include -Xclang <h>`** is `-include <h>`, which CMake uses.
- **`-fno-validate-pch`** (cc1; `-Xclang`) skips validation.
- **`-Winvalid-pch`** is accepted and is a known warning group. In clang
  it is an empty group: no diagnostic belongs to it. Every PCH problem
  is an error whatever `-W` options are given, as in clang.
- **Accepted, no effect:**
  - `-f[no-]pch-instantiate-templates`: instantiation happens on replay
    in any case.
  - `-f[no-]pch-codegen` and `-f[no-]pch-debuginfo`: there is no PCH
    object to share code or debug info from.
  - `-f[no-]pch-validate-input-files-content`: kiln always compares
    content, never mtime.
- **The longest option wins.** When an argument could be read as more
  than one option, the longest spelling is taken, as in clang's option
  table. So `-include-pch` is not `-include` with `-pch`, `-isystem` is not
  `-I` with `system`, `-cxx-isystem` is not `-c…`, `-target-feature` is
  not `-target` with `-feature`, and `-MT`/`-MQ` are not `-M`. `args.rs`
  `longest_option_wins` tests each such collision.

### Format (`KILNPCH 1`)

The record is UTF-8 text, one field per line. Values are tab-separated,
with `\\`, `\t`, `\n` and `\r` escaped. It is written atomically (a
temporary file, then a rename). Lines, in order:

```
KILNPCH 1                               magic and format version
kiln <version> <commit|-> -fclang-compat=<N>
cwd <producer's working directory>
main <the header, as spelled>
include <a -include of the producer>    (0 or more, in order)
opt b\t<option>\t0|1                    a language option (bool)
opt v\t<option>\t<value>                a language option (value)
target <triple>
macro D\t<name>\t<definition>           -D/-U of the producer, in order
macro U\t<name>
file <size>\t<fnv1a64 hex>\t<0|1 system>\t<spelled>\t<absolute>
end <fnv1a64 hex of every byte before this line>
```

The `file` lines list every file the producer read (the dependency list
`-MD` writes), with each file's size and content hash. An unknown line, a
missing field or a bad checksum makes the record corrupt. A different
version is "older" or "newer". A different `kiln` line is "built from a
different branch". The language options are those of the module-file
fingerprint (spec/modules.md §11.4), named as clang names them: the
standard levels, `char8_t keyword`, GNU extensions, exceptions, RTTI,
freestanding, signed char, sized deallocation, C++26 reflection, the
reflection mangling, and the optimization level.

### Diagnostics

The consumer validates in clang's order and with clang's wording, at the
`-fclang-compat` level's version (19 says "PCH file" and "AST file"; 22
says "precompiled file"):

1. **Unreadable record.**
   - A missing file: `fatal error: PCH file 'p.pch' not found: module file
     not found`.
   - Not a PCH: `fatal error: file 'p.pch' is not a valid precompiled PCH
     file: file doesn't start with AST file magic`.
   - A clang PCH (`CPCH`): `fatal error: file 'p.pch' is a precompiled
     header built by clang; kiln reads only its own precompiled headers`.
   - A corrupt record: `fatal error: malformed or corrupted AST file:
     '<why> in kiln PCH record 'p.pch''`.
2. **Language options.** For the first option that differs: `error: C++20
   was enabled in PCH file but is currently disabled` (a bool), or
   `error: OptimizationLevel differs in precompiled file 'p.pch' vs.
   current file` (a value). In clang 19, `-O` shows up as
   `__OPTIMIZE__ predefined macro`.
3. **Target.** `error: PCH file was compiled for the target '<a>' but the
   current translation unit is being compiled for target '<b>'`.
4. **Macros.** The consumer's `-D`/`-U` are checked against the
   producer's with clang's `checkPreprocessorOptions`, and the first
   mismatch is reported:
   - `error: definition of macro 'X' differs between the precompiled
     header ('1') and the command line ('2')`;
   - `error: macro 'X' was defined in the precompiled header but undef'd
     on the command line`.

   A macro that one side defines and the other never mentions is
   accepted, as in clang.
5. **Files.** Any error in steps 2–4 stops the compile before this step.
   Every non-system file is then compared with its recorded size and
   hash, and the first one that changed is fatal:
   - `fatal error: file '/abs/h.hpp' has been modified since the
     precompiled header 'p.pch' was built: size changed (was 41, now 47)`,
     or `content changed`;
   - with `note: please rebuild precompiled header 'p.pch'`.

   A deleted file is `fatal error: malformed or corrupted AST file:
   'could not find file '/abs/h.hpp' referenced by AST file 'p.pch''`.

   System files are not checked, as in clang. This includes CMake's
   prefix header, because `cmake_pch.hxx` says `#pragma clang
   system_header`.

### Replay

A valid PCH makes the compile behave like:

```
<consumer args> -include <producer's -include>... -include <producer's
main> -include <consumer's -include>...
```

A consumer `-include` that the producer already had is dropped. Without
this, CMake's `-include-pch p.pch -include cmake_pch.hxx` would enter its
unguarded `cmake_pch.hxx` twice. A relative path is resolved against the
producer's working directory when the consumer's is different. `-E` and
`-M*` replay the producer's main and the consumer's includes without
validating.

**Dependency files.** As in clang, the producer's depfile lists the
producer input and every file it read. The consumer's depfile lists the
PCH's inputs first and then its own files. `-MMD` drops the system files
from both.

### Deviations

- A touch that leaves the content unchanged is accepted. A same-size edit
  is reported as `content changed`. Clang compares mtimes: it reports
  `mtime changed (was N, now M)`, or misses an edit made within the same
  second.
- `-include foo.h` does not probe for `foo.h.pch` or `foo.h.gch`. Only
  `-include-pch` reads a PCH.
- A `.gch` directory is not supported, and `-fpch-preprocess` is an
  unknown argument.
- A PCH is not an object: `-fpch-codegen` emits nothing.

## Dependency output

This is clang's `DependencyCollector` and `DependencyFileGenerator`.

- `-M`/`-MM` print the rule instead of the text and imply `-w`. Their
  output goes to `-MF`, else `-o`, else standard output.
- `-MD`/`-MMD` run the action normally (`-E`, `-fsyntax-only`, `-S`,
  `-c`, or compiling for a link) and write the rule of each source input
  as a side effect to `-MF` (`-` is standard output), else to `-o` with
  the extension replaced by `.d` (`-o x/y.o` -> `x/y.d`), else to
  `<input stem>.d` in the working directory. Linking several sources
  writes one rule per input: `<stem>.d` each without `-o`; with `-o`
  every input's rule goes to `<output>.d` and the last one stays
  (clang's naming, `getDependencyFileName`).
- `-MM`/`-MMD` leave out system headers.
- Files are listed in first-seen order:
  - the main file;
  - every entered header;
  - every header an `#include` found but skipped (`#pragma once`,
    `#import`, the guard optimization; clang's `FileSkipped`);
  - every header found by `__has_include`;
  - every `#embed` resource.
  A header is listed by the path the lookup found it by
  (`ResolvedInclude::path`, spec/pp.md "Header search"), once per
  distinct path: a file reached as `real/x.h`, `lnk/x.h` (a symlinked
  directory) and `sub/../real/x.h` is listed three times, even though it
  is entered once and `__FILE__` names it `real/x.h` (WP-820).
  `<built-in>`, `<command line>` and `<stdin>` are left out, and `./` is
  removed.
- Module files (WP-1051), as clang 22 lists them:
  - Nothing an import reads is listed from inside its module file: no
    header of its global module fragment, no file of a capsule replay
    (§11.8), whatever kind of module file it is (spec/modules.md).
    The TU's own `#include`s are listed as usual.
  - With `-fmodule-file-deps` (clang `IncludeModuleFiles`) every module
    file an import reads, the named module's and transitively the ones
    it imports, is listed after the files seen so far, in import order.
    An `-fmodule-file=<name>=<path>` file is listed as spelled (`./`
    kept), a `-fprebuilt-module-path` one joined onto the working
    directory (absolute, `./` kept), as clang's
    `getPrebuiltModuleFileName`. `-MM`/`-MMD` keep them; `-MP` gives them
    phony rules. It is on by default for `--precompile` (the clang
    driver passes `-module-file-deps` then); `-fno-module-file-deps`
    turns it off.
  - A nameless `-fmodule-file=<path>` is listed first, as spelled,
    whether or not it is imported and whatever the switches (clang's
    `ExtraDeps`); then an `-include-pch` file's inputs, then the main
    file.
  - `import std;` / `import std.compat;` from the module cache ("Module
    cache") list the entry's module file (absolute) unless
    `-fno-module-file-deps`, so a rebuilt entry recompiles the importer.
    Under `-M`/`-MD` the entry's stamped inputs (`std.cppm`, then every
    libc++ header the build read) follow as system files, so a libc++
    edited in place does too. Both come from the stamp, so the rule is
    byte-identical on a hit and a miss, and with either kind of module
    file. clang has no cache: with `-fmodule-file=std=std.pcm` it lists
    `hello.o: hello.cpp` (or `hello.cpp std.pcm` with
    `-fmodule-file-deps`).
  - Without the cache (off, or fallen back to part A) `std` is built from
    its source in-process and its files are listed as for any header.
  - Shapes checked against clang 19 and 22 (kiln_driver tests
    `depfiles` `module_imports_list_module_files_as_clang_does`,
    `module_cache` `depfile_same_on_miss_and_hit`):
    `-MMD -fmodule-file=m=m.pcm`: `u.o: u.cpp user.h`; with
    `-fmodule-file-deps -MP`: `u.o: u.cpp user.h m.pcm`, `user.h:`,
    `m.pcm:`; `--precompile m2.cppm -o m2.pcm -fmodule-file=b=b.pcm
    -MMD`: `m2.pcm: m2.cppm mhdr.h b.pcm`; `-fmodule-file=m.pcm`:
    `u.o: m.pcm u.cpp user.h`.
  - Deviations: clang lists a module file's imports before it (`b.pcm
    m.pcm`), kiln in the order it reads them (`m.pcm b.pcm`). clang 19
    lists module files under `-c m.cppm` by default; kiln follows
    clang 22, which does not.
- The target is `-MT` (verbatim) or `-MQ` (Make-quoted), in order. Without
  either it is the `-o` value (for `-MD`/`-MMD`, whatever the action) or
  `<input stem>.o`, Make-quoted.
- Lines wrap at 75 columns with ` \`. `-MP` adds a phony target for every
  dependency except the main file.
- With `-E`, dependency files are written only when there were no
  errors. With the other actions the rule is written once the
  preprocessor reached the end of the input, whatever errors Sema or
  code generation report (clang and GCC keep the file); after a fatal
  error the preprocessor lexes on to the end, its diagnostics dropped,
  so the list is complete. An `#include` whose file is not found writes
  no file (clang's `SeenMissingHeader`).
- `-MG` (WP-596, WP-920): under `-M`/`-MM`, an `#include` whose file is
  not found is no error; the header is listed as spelled (a generated
  file the build will make) and preprocessing goes on without it, as
  clang's. Without `-M` or `-MM` it is the driver error `option '-MG'
  requires '-M' or '-MM'` (`drv_mg_requires_m_or_mm`).

### `-MJ`

`-MJ <file>` writes clang's JSON compilation-database fragment
(`Clang::DumpCompilationDatabase`, `crates/kiln_driver/src/cdb.rs`): one
line per frontend job (`-E`, `-M`, `-fsyntax-only`, each source of `-S`,
`-c` or a link),

```
{ "directory": "<cwd>", "file": "<input>", "output": "<out>", "arguments": ["<exe>", "-x<type>", "<input>", "-o", "<out>", "--driver-mode=g++", <the command line>, "--target=<triple>"]},
```

which build systems concatenate into `compile_commands.json`. `output` is
left out under `-fsyntax-only`; a link names each source's temporary
object; `--driver-mode=g++` only under `kiln++`. The command line is
clang's rendering: without the inputs, `-o`, `-x`, `-MJ` and the other
dependency options, a separately valued option as two strings (`-D`,
`X=1`), `-target X` as `--target=X`. The file is truncated once per run
and written whatever the jobs' outcome; `-###` writes none (clang's). A
file that cannot be written is the driver error `compilation database
'<file>' could not be opened: <reason>` (`drv_compilation_database`),
exit 1. The executable path is kiln's own, the one difference from a
clang fragment.

## Time trace

`-ftime-trace[=<path>]` (WP-1115) writes, for each input the run compiles,
a Chrome trace in the format of clang's (LLVM `TimeProfiler`), loadable in
Perfetto (ui.perfetto.dev) and chrome://tracing. The recorder is
`kiln_source::trace`: thread-confined (kiln compiles a TU on one thread;
the recorder is thread-local, so in-process parallel compiles never mix),
monotonic (`Instant`), dependency-free, safe Rust.

**Which runs.** A run with a compiler job: `-c`, `-S`, linking, `-E`,
`-M`/`-MM`. `-fsyntax-only` and a header precompile write nothing and
warn (the options table). One trace per input; nothing is written for an
input whose compile, `opt` or `llc` step failed (clang's failing `cc1`
writes none).

**Where** (clang `handleTimeTrace`; `time_trace::path`):

| Command line | Trace |
|---|---|
| `-ftime-trace=<dir>` (an existing directory) | `<dir>/<output file name>` with the extension replaced by `.json` |
| `-ftime-trace=<file>` | `<file>` (every input writes it; the last wins, as clang) |
| `-ftime-trace`, linking | `<-o or a>-<input file name>`, extension `.json` (clang's implicit `-dumpdir`): `-o out/app x.cpp` gives `out/app-x.json` |
| `-ftime-trace`, otherwise | the job's output with the extension `.json`: `x.o` gives `x.json`, `-o out/o1.o` gives `out/o1.json`, `-E` to standard output gives `-.json` |

An extension is replaced as LLVM's `sys::path::replace_extension` does (a
dot in a directory is not one; a name with none gets `.json`). A trace
that cannot be written is `unable to open output file '<path>': '<os
error>'`, as clang's; kiln then exits 1 (clang 22 exits 0).

**Format.** `{"traceEvents":[...],"beginningOfTime":N}` (N: the start, µs
since the Unix epoch). Events, in clang's order: the complete events
(`"ph":"X"`, `pid`, `tid` = the process id, `ts` and `dur` in µs from the
start, `name`, `args.detail` when non-empty plus the verbose args) in the
order they end; the `Source` events as async pairs (`"ph":"b"`/`"e"`,
`"cat":"Source"`, `id`, the detail on `b`); one `Total <name>` per name
(`"ph":"X"`, `ts` 0, `dur` the total, `args.count`, `args."avg ms"` the
integral average) on `tid` main + 1, + 2, ... in decreasing total; then
the `process_name` (`kiln`, as clang's `clang`) and `thread_name` (the
program name, `kiln++`) metadata (`"ph":"M"`). A total counts only the
outermost of nested same-name events.

**Granularity.** `-ftime-trace-granularity=<n>` (µs, default 500): a
shorter event is dropped but still counted in its total; `0` keeps every
event. The `opt` and `llc` children run with `--time-trace
--time-trace-granularity=<n> --time-trace-file=<temp>` and their events
(shifted to kiln's time base, metadata dropped) and totals are merged in.

**`-ftime-trace-verbose`.** As clang: `InstantiateClass` /
`InstantiateFunction` get `file` and `line` (the template's presumed
location). kiln adds `kind` and `steps` to the Oven's evaluations and
records the per-call `ConstevalCall` / `ConstexprCall` events (gated:
one clock read per VM call is measurable on call-heavy evaluations).
clang 19 accepts the flag too; it behaves the same under
`-fclang-compat=19`.

**Events** (clang name: where kiln records it; detail):

| Event | kiln | Detail |
|---|---|---|
| `ExecuteCompiler` | driver, the whole input (`time_trace::InputTrace`) | — |
| `Frontend` | driver, the in-process frontend: pp, parse, sema, Oven, IR emission (clang has two nested `Frontend`s; kiln one) | — |
| `Source` | `kiln_pp` `push_file` .. `end_of_file`, each `#include`d file (not the main file; not under `-E`/`-M`, as clang) | the presumed file name (`./h.h`) |
| `ParseDeclarationOrFunctionDefinition` | `kiln_parse` `parse_external_declaration_inner`: simple declarations and linkage specifications (not `template`, `namespace`, `using`, `static_assert`, `export`, as clang) | `file:line:col` |
| `ParseFunctionDefinition` | the namespace-scope function definitions (`decl.rs`; not the late-parsed member bodies, as clang) | the name |
| `ParseClass` | `parse_record_body`, the late-parsed member bodies included | the name |
| `InstantiateClass`, `InstantiateFunction` | `kiln_sema` `push_inst_frame` .. `pop_inst_frame` (class / member class; function / member function specialization) | the specialization (`Box<int>::get`) |
| `PerformPendingInstantiations` | `end_of_tu_instantiations` | — |
| `EvaluateAsConstantExpr`, `EvaluateAsInitializer`, `EvaluateAsRValue` | the Oven's top-level evaluations (spec/oven.md "Trace hook"): a variable's initializer, a `static_assert` condition, any other | the variable's qualified name, or the expression's range (`<t.cpp:3:19, col:25>`) |
| `CodeGen Function` | `kiln_codegen` `emit_fn_def` | the qualified name |
| `Backend` | driver, from after the frontend to the end of `llc` | — |
| `Optimizer` | driver, the `opt` child (its `OptModule`, `OptFunction`, `RunPass`, ... merged) | — |
| `CodeGenPasses` | driver, the `llc` child (its passes merged) | — |
| `OvenCompile`, `ConstevalCall`, `ConstexprCall`, `OvenMetaOp` | kiln only: spec/oven.md "Trace hook" | |

Not recorded (no kiln counterpart, or a follow-up): `ParseTemplate` (clang
records it only for `-fdelayed-template-parsing`), the folds clang traces
as `EvaluateAsRValue` / `EvaluateAsInt` / `EvaluateAsBooleanCondition` /
`EvaluateForOverflow` (kiln folds outside the Oven's entry points),
`CheckConstraintSatisfaction`, `BuildCFG`, `DebugType`, and the
`PCH`/module events.

**Deviations.** An event's `ts`/`dur` are kiln's, so the trace differs
from clang's in its numbers, not its shape. Linking with
`-ftime-trace=<dir>` names the trace after the input's object
(`<dir>/x.json`), not clang's temporary. An `EvaluateAs*` range ends at
the start of the last token, as clang's. An immediate invocation inside a
`constexpr` variable's initializer is its own `EvaluateAsConstantExpr`
inside the initializer's `EvaluateAsInitializer` (clang folds it into
the one evaluation). kiln memoizes constant calls, so a verbose trace
has one `ConstevalCall` / `ConstexprCall` per evaluated call, not per
call in the source's recursion.

## Constexpr profile

`-fconstexpr-stats` and `-fconstexpr-profile[=<format>|<file>]`
(WP-1116, kiln extensions) report where the Oven spent its time and
memory in one translation unit. Both run in `-fsyntax-only` and every
codegen job, after the `-fconstexpr-report` and the instantiation report;
the profile is written first, then the summary. A module interface built
in-process for an importer (a module cache miss, "Module cache") prints
neither: the importer reports only its own evaluations. A module source
named directly by `-fmodule-file=<name>=<source>` is compiled in the
importer's own Oven, so its evaluations are the importer's. Without
either flag nothing is printed and no counter is kept (spec/oven.md
"Profile counters").

**`-fconstexpr-stats`** prints eight lines on stderr, always in this
order, each `constexpr: <group> key=value ...` with keys in a fixed
order (spec/oven.md "Profile counters" defines each):

```text
constexpr: evaluations count=3 failed=0 immediate=1 wall-us=1858
constexpr: steps count=476
constexpr: calls count=179 memo-hits=16 memo-misses=24 memo-hit-rate=40.0%
constexpr: memory allocs=1 bytes=40 peak-live=40 reclaimed=40 peak-arena=40
constexpr: interned frozen-bytes=48 refl-bytes=0 static-bytes=16
constexpr: meta-ops count=0 splice-fast=0
constexpr: trials count=1 failed=0 steps=354 wall-us=52
constexpr: compile functions=3 wall-us=196
```

**`-fconstexpr-profile`** selects the output as
`-fkiln-instantiation-report` does:

| Spelling | Output |
|---|---|
| `-fconstexpr-profile`, `=text` | text on stderr |
| `=json` | NDJSON on stderr |
| `=<file>` ending in `.json` | NDJSON to `<file>` |
| `=<file>`, otherwise | text to `<file>` |

The last spelling wins. A file that cannot be written is `unable to open
output file` (`drv_cannot_open_output`, an error). `-fconstexpr-profile-limit=N`
(default 20; 0 is unlimited) caps the rows of each table; the rows left
out are counted on a `... K more rows` line (text only; NDJSON stops
silently, its `total` row still counts every function and site).

The report has two tables, each ranked by inclusive steps (descending),
then count (descending), then location (file, line, column), then name,
so it is the same on every run but for its times:

- **functions**: each constexpr or consteval function the Oven ran,
  with its qualified name and declaration location (`[consteval]` marks
  an immediate function), its calls (memo hits included, and also
  counted on their own), inclusive and exclusive steps, time and the
  peak constexpr heap live during a call. The rows include the trial
  evaluations' calls (folds, constant-initialization checks), which have
  no site row.
- **evaluation sites**: each recorded top-level evaluation
  ([expr.const]/22 manifestly constant-evaluated contexts:
  `static_assert`, an immediate invocation, a `constexpr` variable's
  initializer `init <variable>`, a template argument, ...), its count,
  failures, steps, time and peak.

The text form:

```text
===-- kiln constexpr profile: fib.cpp --===
functions (top 3 of 3, by inclusive steps):
   calls    memo   incl steps   self steps    incl ms    self ms  peak bytes  functions
      38      16          708          708      0.089      0.089           0  fib [consteval]   (fib.cpp:1:15)
       1       0          105          105      0.039      0.039           0  sum   (fib.cpp:2:15)
       1       0           17           17      0.021      0.021          40  heap   (fib.cpp:3:15)
evaluation sites (top 3 of 3, by inclusive steps):
   evals  failed   incl steps   self steps    incl ms    self ms  peak bytes  evaluation sites
       1       0          354            0      2.477      2.417           0  consteval   (fib.cpp:5:9)
       1       0          105            0      0.107      0.067           0  init b   (fib.cpp:6:15)
       1       0           17            0      0.084      0.063          40  init c   (fib.cpp:7:15)
  total: 3 evaluations, 476 steps, 2.668 ms; 1 trials, 354 steps; 40 calls
```

(`fib`'s 708 steps are the immediate invocation's 354 plus those of a
trial that folds `int a = fib(10);`'s whole initializer again, WP-1129.)

The NDJSON form has one object per row, then a totals object:

```text
{"function":"fib","consteval":true,"loc":"fib.cpp:1:15","calls":38,"memo_hits":16,"incl_steps":708,"self_steps":708,"incl_us":94,"self_us":94,"peak_bytes":0}
{"site":"consteval","loc":"fib.cpp:5:9","evaluations":1,"failed":0,"incl_steps":354,"self_steps":0,"incl_us":2019,"self_us":1955,"peak_bytes":0}
{"total":true,"file":"fib.cpp","functions":3,"sites":3,"evaluations":3,"steps":476,"wall_us":2293,"trials":1,"trial_steps":354,"calls":40}
```

A text row's location is the file's base name; a JSON `loc` is the
presumed file name as spelled. Times are wall-clock, so only the
counts are reproducible. Step and call counts are exact: every step is
counted, nothing is sampled. A site's exclusive time includes compiling
its functions to bytecode.

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
- The compile thread's stack (`-fkiln-stack-size=`, default 1 GiB) is
  reserved before anything else runs. When the thread cannot be created
  (typically a `ulimit -v` cap below the reservation) the driver prints
  `<prog>: error: cannot create the compile thread with a <n>-byte stack:
  <os reason> (lower it with -fkiln-stack-size=)` and exits with status 70
  (WP-682); it never exits silently. A panic (an internal compiler
  error) exits with 1, or 70 under `-fno-crash-diagnostics`; see
  "Internal compiler errors".
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

## Internal compiler errors

A panic anywhere in the compiler is an internal compiler error (ICE): a
kiln bug, never the program's fault. The `kiln` binary installs a panic
hook before it does anything else (`crates/kiln_driver/src/crash.rs`,
WP-911), so an ICE is reported the way clang reports a crash and not as
Rust's `thread '...' panicked at` text. In-process users of the driver
library (the tests) install no hook and see Rust's default behaviour.

### The report

On standard error, each line prefixed with the program's basename like
other driver diagnostics:

```
kiln++: error: internal compiler error: <panic message, first line>
  <further lines of the message, indented>
kiln++: note: while compiling '<input>' (phase: <phase>, at <file>:<line>:<col>)
kiln++: note: in compiler source <crate file>:<line>:<col> (thread '<name>')
kiln <version> (<git sha>)
kiln++: note: please submit a bug report to https://github.com/christopherfretz/kiln/issues and include the crash reproducer
kiln++: note: set KILN_BACKTRACE=1 to include the compiler backtrace
kiln++: note: preprocessed source and associated run script are located at:
kiln++: note: diagnostic msg: <dir>/<stem>-<random>.ii
kiln++: note: diagnostic msg: <dir>/<stem>-<random>.sh
```

- `<input>` is the input being compiled, `<stdin>` for `-`. A panic
  outside any input (argument handling, linking) says `while running the
  driver`.
- `<phase>` and the location come from the current-activity breadcrumb
  (`kiln_source::activity`), a thread-local cell holding a phase and a
  `Loc`. The pipeline updates it at a few coarse points, and an update is
  one thread-local store:
  - `preprocess`: when an `-E` job starts;
  - `parse`: at each top-level declaration, with its location (so the
    location is the start of the declaration being parsed);
  - `oven`: around each constant evaluation, with the expression's
    location, restoring the previous activity afterwards;
  - `sema`: end-of-translation-unit analysis (pending instantiations,
    final checks);
  - `codegen`: LLVM IR generation;
  - `driver`: everything else.

  `, at <loc>` is left out when no location is known. The location is
  resolved, as a presumed location, only after the panic, in the
  input's own source manager.
- The `in compiler source` line gives the panic's location in kiln's
  source and the thread's name (the compile thread is `kiln`).
- The version line is `kiln <version> (<sha>)`: the version and the
  commit of "Version" (`kiln_driver::version::GIT_SHA`), with `unknown`
  when the build recorded no commit.
- The backtrace: with `KILN_BACKTRACE` or `RUST_BACKTRACE` set to
  anything other than empty or `0`, the backtrace note is replaced by
  `note: compiler backtrace:` followed by the Rust backtrace of the
  panicking thread. Otherwise none is captured.
- The last three lines are the reproducer (below). Under
  `-fno-crash-diagnostics` they are left out.

### Exit status

kiln matches clang 19's driver:

- **1** after the reproducer is written, or after an attempt to write
  it that failed. clang's driver exits 1 once it has printed its
  `diagnostic msg:` lines.
- **70** (`EX_SOFTWARE`) under `-fno-crash-diagnostics`. clang's driver
  then passes on the status of the crashed `-cc1`, and its driver treats
  70 as a `-cc1` crash. Before WP-911 every kiln panic exited 70.

A failure to create the compile thread still exits 70 (see "Output,
diagnostics, exit status").

### The reproducer

Unless `-fno-crash-diagnostics` (or `-fcrash-diagnostics=off`) is given,
kiln writes two files:

- `<dir>/<stem>-<random>.ii`: the crashing input, preprocessed as `-E`
  would do it with the same flags (same macros, include paths and
  `-std=`; dependency output, `-o` and `-v` are dropped), with
  linemarkers. It is `.i` for a C input.
- `<dir>/<stem>-<random>.sh`: a POSIX shell script with three comment
  lines (the compiler identity, the original command line after
  response-file expansion, and the working directory), then `cd` to that
  directory, then the same command with the input replaced by the
  `.ii`. Each word is quoted for the shell. A relative program path is
  made absolute, and the program keeps its name so the driver mode is
  the same.

The parts of the names:

- `<dir>` is `-fcrash-diagnostics-dir=<dir>`, else `$TMPDIR` (when set
  and not empty), else `/tmp`. It is created if missing, as clang does.
- `<stem>` is the input's file stem (`stdin` for `-`).
- `<random>` is six hexadecimal digits. The `.sh` is created exclusively,
  and a name that already exists is retried with new digits.

Edge cases:

- The `.ii` is produced on a separate thread whose panics are caught. If
  preprocessing the input also panics, only the `.sh` is written (it
  runs the original command on the original input), and the report says
  `note: diagnostic msg: preprocessing the input also crashed; only the
  run script was written`.
- For a panic outside any input, only the `.sh` is written, with a
  similar `diagnostic msg:`.
- A directory that cannot be created or a file that cannot be written
  is reported as a `diagnostic msg:` line naming the reason.
- A file on a `diagnostic msg:` line is `<dir>` joined with its name, so
  it is absolute when `<dir>` is. The other `diagnostic msg:` lines are
  prose (the harness tells them apart this way).

`-fcrash-diagnostics` and `-fcrash-diagnostics=compiler|all` turn the
reproducer back on. The last of these flags, `-fno-crash-diagnostics`
and `-fcrash-diagnostics=off` wins.

### Threads

The hook records only the first panic of the process and prints nothing
itself. The panic unwinds to `main`, which prints the report once:

- A panic on a helper thread that the compile thread re-raises (as
  `std::thread::scope` does) is the same crash, so the second record is
  dropped. The link step's scoped threads are the only helper threads
  today; codegen is single-threaded.
- A second panic on the panicking thread before `main` has reported
  (a panic in a destructor during unwinding) makes the Rust runtime
  abort. The hook therefore prints the banner at once, followed by
  `note: a second panic while unwinding aborts the compiler: <message>`,
  with no reproducer. The process then aborts with `SIGABRT`.

### Constraints

- Every Cargo profile keeps `panic = "unwind"`. With `abort` the report
  could still print from the hook, but the reproducer (written after
  unwinding to `main`) and the exit code would be lost.
- Stack exhaustion is not a panic. The stack guard reports it as a fatal
  diagnostic (§1 P4), and an overflow it misses is a `SIGSEGV` that no
  hook sees.
- On the non-crashing path the cost is the breadcrumb stores plus one
  `catch_unwind` frame and a recorded input path per input.

### The test hook

The hidden `-fkiln-crash-for-testing=<phase>` makes kiln panic on purpose
with `crash requested by -fkiln-crash-for-testing=<phase>`. The phases
are `driver`, `preprocess` (also `lex` or `pp`), `parse`, `sema`, `oven`
and `codegen`.

- The panic happens when that phase is entered. For `parse` it happens
  at the first top-level declaration, for `oven` at the first constant
  evaluation, and for `driver` right after argument parsing.
- The request is per thread. A `preprocess` request also crashes the
  reproducer's own preprocessing, which is how the `.sh`-only path is
  tested.
- An unknown phase is `invalid value`. The flag is not listed in
  `--help`.

## `-v`, `--version`, `--help`

- `--version` prints to standard output:

  ```
  kiln version <v> (C++26 frontend, LLVM IR backend)
  Target: <triple>
  Thread model: posix
  InstalledDir: <dir of the executable>
  LLVM backend: <major> (<llvm dir>)
  ```

  The last line (WP-1016) names the installation "LLVM discovery"
  selected; with none, `LLVM backend: none (kiln supports LLVM 19 and
  22)`. It never says `clang version` (the identity rule, "Identity").

- `-v` prints the same to standard error, plus the selected GCC
  installation. For each input it then prints clang's search-list block:
  the `ignoring nonexistent directory` and `ignoring duplicate directory`
  notes, the `#include "..." search starts here:` and
  `#include <...> search starts here:` sections, and
  `End of search list.`. That block is byte-identical to clang's (tested).
  The other `-v` lines are not.
- `-v` or `--version` with no inputs exits 0.
- `-print-resource-dir` / `--print-resource-dir` prints the resource
  directory (`-resource-dir`, else the default, "Toolchain") and a
  newline to standard output and exits 0, whatever else is given, as
  clang does (WP-667). It is one of the toolchain probes below.
- `--help` prints the option summary (`kiln_driver::help::HELP`, WP-1027)
  in clang's `--help` layout: `OVERVIEW: kiln, ...`, the usage lines of
  both names (`kiln++` and `kiln`), the two names' language rule, then
  the options by group (actions, language, preprocessor, dependency
  output, target and toolchain, code generation, debug information,
  sanitizers and coverage, modules, linking, diagnostics, compile-time
  evaluation, kiln development, information). `[kiln]` opens the
  description of an option clang does not have. It is a summary: this
  section's "Options" is the complete list, and hidden options
  (`--driver-mode=`, `-fkiln-crash-for-testing=`) are not in it.
- Every option `--help` names is accepted: a unit test expands each
  listed form (`[...]` with and without, `<...>` with a sample value),
  parses it with an input, and requires no argument error and no
  diagnostic-option error. The reverse (every accepted option is listed)
  is not required. The text never calls kiln clang ("Identity") and fits
  80 columns.

## Version

kiln's version is the workspace's `[workspace.package] version`
(`Cargo.toml`), semantic versioning with the compatibility scope of
docs/STABILITY.md; it is changed there and nowhere else (WP-927).
`kiln_driver::version` is the only reader:

- `VERSION` (`CARGO_PKG_VERSION`) is what `--version`, `-v`,
  `__VERSION__` (`kiln <v>`), `!llvm.ident` (`kiln version <v>`) and the release archive names
  (`kiln-<v>-<triple>.tar.xz`) show;
- `MAJOR`, `MINOR`, `PATCH` are `__KILN_MAJOR__`, `__KILN_MINOR__`,
  `__KILN_PATCH__` ("Predefines") and `MAJOR` names the installed
  resource directory `lib/kiln/<major>` ("Installation layout");
- `GIT_SHA` is the 12-digit commit the binary was built from, recorded
  by `crates/kiln_driver/build.rs`: `$KILN_GIT_SHA` at build time when it
  is 7-40 hex digits, else `git rev-parse --short=12 HEAD` when the
  workspace is the root of a git work tree (a source archive unpacked
  inside another repository does not take that repository's commit),
  else none; a build without git or `.git` records nothing and succeeds.
  `long_version()` is `<v> (<sha>)`. The identity text is "Identity"
  (WP-920); `-dumpversion` prints the compatibility version, not `<v>`.

## Identity (operator decision Q3, WP-920)

kiln keeps clang's macros, so headers and build systems that test them
take their clang paths: `__clang__`, `__clang_major__`/`__clang_minor__`/
`__clang_patchlevel__` (the compatibility level, `-fclang-compat=`),
`__clang_version__`, `__llvm__`, and GCC's `__GNUC__ 4.2.1`. It does not
call itself clang in text: `--version`, `-v` and `-###` print `kiln
version <v> ...` ("`-v`, `--version`, `--help`"), and `__VERSION__` is
`"kiln <v>"`. No output of kiln's own says `clang version`; the one
exception is a link run with `-v`, which relays the backing clang++'s own
banner to standard error (the link is that program's work). `<v>` is
`kiln_driver::version::VERSION` ("Version"), which every identity string
reads.

`-dumpversion` prints the **compatibility version**, the one the clang
macros spell (`22.1.8`, `19.1.7` under `-fclang-compat=19`), not kiln's
own `<v>`: configure scripts compare it with clang release thresholds
(a CMake or autotools check for "clang >= 15" would fail on `0.0.1`), and
it agrees with `__clang_major__.__clang_minor__.__clang_patchlevel__`.
clang prints the same triple; GCC prints its major version only.

What the build systems key on (WP-920; docs/COMPATIBILITY.md "Build
systems" has the detail): CMake identifies the compiler from the macros
(`Clang`, the compatibility version); autotools from `__GNUC__` (GNU)
and libtool from `-print-prog-name=ld` and `-v` link lines; Meson from
`--version` text, where it requires the word `clang` and so does not
recognise kiln without the harness's shim (`scripts/oss/kiln-cxx`,
`KILN_OSS_VERSION_SHIM`).

## Toolchain probes (WP-920)

Each prints one answer to standard output and exits 0 without compiling,
whatever inputs are given; with several, the first in clang's order
(`Driver::HandleImmediateArgs`) wins: `-dumpmachine`, `-dumpversion`,
`-print-search-dirs`, `-print-resource-dir`, `-print-file-name=`,
`-print-prog-name=`, `-print-libgcc-file-name`, `-print-multiarch`,
`-print-target-triple`, `-print-sysroot`. Unknown arguments are still
reported next to the answer (clang stays silent).

| probe | answer |
|---|---|
| `-dumpversion` | the compatibility version ("Identity") |
| `-dumpmachine`, `-print-target-triple` | the normalized target triple (`x86_64-unknown-linux-gnu`) |
| `-print-multiarch` | the Debian multiarch name (`x86_64-linux-gnu`, `aarch64-linux-gnu`); GCC's probe, which clang 19 rejects |
| `-print-sysroot` | `--sysroot`, else an empty line; GCC's probe |
| `-print-resource-dir` | the resource directory |
| `-print-file-name=<f>` | `<resource dir>/<f>` if it exists; for `libc++.modules.json`, libc++'s module manifest ("Module cache"); else the backing clang++'s answer (which searches the GCC installation and library paths and echoes `<f>` when nothing matches) |
| `-print-prog-name=<p>`, `-print-libgcc-file-name`, `-print-search-dirs` | the backing clang++'s answer, run with the toolchain arguments kiln links with (`--target=`, `--sysroot=`, `--gcc-toolchain=`, `--gcc-install-dir=`; `toolchain_link_args`): links go through that program, so its tools, runtime library and search directories are the honest ones |

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
13. **`-E -MD` writes no dependency file after an error** (clang writes
    it unless a header was missing); the other actions match clang.
14. **Debug information gaps:** `-g` emits clang 19's DWARF for what
    kiln lowers, with the gaps listed in docs/status/debuginfo.md (each
    either a hard error or a documented follow-up); the debug flags kiln
    does not implement are errors ("Debug information flags").
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
18. **Target features** ("Target features" above): the CPU names other
    than `generic` and `native`, `-march=armv8-r`, and the extensions kiln does
    not model (`sme*`, `fp8*`, clang 19's `b16b16`/`sve-b16b16`, ...) are
    errors where clang accepts them; on x86-64 `-target-feature` is
    ignored (clang passes it on), and the flags listed under "x86-64
    target features", "Not modelled", differ; a `-target-feature` value without
    `+`/`-` is an error (clang warns).
19. **Codegen flags** ("Codegen flags" above, WP-605): `-fno-plt` puts
    `nonlazybind` only on functions with no definition in the TU, where
    clang also marks a function used before its later definition and
    template instantiations; x86-64 `-march=<cpu>` is not expanded
    for `wildcatlake` / `novalake` (spec/landed/driver/WP-630.md), and
    `-march=native` has `/proc/cpuinfo`'s features but not clang's host
    CPU name or tuning (WP-1000);
    `-march=native` emits no `"target-cpu"` and `-mtune=native` no
    `"tune-cpu"` (clang resolves the host); `-fwrapv-pointer` is accepted at `-fclang-compat=19`
    (clang 19 has no such flag); the
    profile-guided optimization flags ("Coverage flags"),
    `-fshort-enums`, and the sanitizers "Sanitizers" does not
    list, are errors; `-fno-trapv` is accepted (clang has no such
    flag) and so are `-f[no-]sanitize-merge[=]` at `-fclang-compat=19`
    (clang 19 has neither); version 1 (regular expression) sanitizer
    ignorelists are rejected; and the Oven ignores `-fwrapv` and
    `-ftrapv`.
20. **`-fmacro-prefix-map=` keeps `.` components and repeated `/`.** Once
    any `-fmacro-prefix-map=`/`-ffile-prefix-map=` is given, clang also
    applies `llvm::sys::path::remove_dots` (no `..` folding) to every
    `__FILE__` and source-location file name, mapped or not (cc1's
    `UseTargetPathSeparator`; `-fno-file-reproducible` turns it off);
    kiln replaces the prefix only, so `-I /p//inc/./` with a map that does
    not swallow the `//` and `/./` keeps them where clang drops them.
21. **No MS extensions mode, no PAuth-LR from `-march=`** (WP-846).
    `-fms-extensions` (and `-fms-compatibility`) is `unknown argument`:
    clang's mode adds MS keywords, `__declspec` attributes and language
    builtins such as `__dmb`/`__dsb`/`__isb`/`__yield` on AArch64, and
    accepting the flag without them would hide the gaps (spec/pp.md
    "Feature queries"). `-mbranch-protection=standard` never adds
    PAuth-LR (`pc`), because kiln models no `-march=` extension that
    enables it; `pac-ret+pc` does. A function's
    `target("arch=...")` changes no target features (only its
    `branch-protection=` and the WP-459 `+feature` items are read).
    Tests: `conformance/target.acle/ms-barrier-builtins-*.cpp`,
    `ir-branch-protection-*.cpp`.

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
    `.c`/`.h`/`.i`/stdin as C;
  - WP-920: the toolchain probes (`dumpversion_is_the_clang_compat_version`,
    `target_probes`, `delegated_probes`), `__VERSION__`
    (`version_macro_names_kiln`), the reflection default
    (`reflection_default_follows_the_standard`), `-dD`/`-dI`
    (`dump_defines_and_includes`), `-MG` (`missing_headers_as_generated`),
    `-MJ` (`compilation_database_fragment`, plus `cdb.rs` unit tests) and
    linker-only inputs (`linker_inputs_alone_are_inputs`).
- `crates/kiln_driver/tests/time_trace.rs` (WP-1115): "Time trace": the
  path of every command-line shape, compared with `clang++-19`'s; the
  JSON's structure (members, phases, nesting, metadata, totals); the
  event names and details of a TU with a header, a class template and a
  `consteval` call, plain and verbose; the granularity; no flag, no file.
  `driver.rs` `time_trace_syntax_only`: the unused warnings and the
  invalid granularity.
- `crates/kiln_driver/tests/constexpr_profile.rs` (WP-1116): "Constexpr
  profile": exact steps and calls of a recursion, a loop and a memoized
  consteval recursion (also in a variable's initializer, with its trial);
  heap peak and reclaimed bytes of `new`/`delete`; a failed evaluation
  and a failed trial; NDJSON well-formedness; ranking tie-breaks and
  run-to-run determinism; the text layout, the limit, the file outputs
  and an unwritable file; the summary's lines; codegen jobs; nothing
  printed without the flags and an unchanged `-fconstexpr-report` with
  them; an importer of a BMI. `module_cache.rs`
  `constexpr_profile_is_the_importers_only`: an in-process module build
  prints nothing. `constexpr_opts.rs` unit tests: the spellings and
  their errors.
- `crates/kiln_driver/tests/verify.rs`: `-verify` end to end under both
  names (`kiln` and `kiln++`): the report pinned against clang 19, and
  compared live with `clang++-19 -Xclang -verify` when it is installed.
  `args.rs` unit tests cover the option semantics.
- `crates/kiln_driver/tests/pch.rs` (WP-1023): CMake's producer and
  consumer command lines, with their depfiles byte-compared with clang 19
  when it is installed; object and link; output naming; include order and
  the producer's working directory; each option, target and macro mismatch
  under both compat levels; stale, deleted, missing and foreign PCH files;
  `-MMD`. `args.rs` `longest_option_wins` and `pch_flags`, `pch.rs`
  `record_round_trip`. The harness's `pch:` tests are
  `tests/conformance/cpp.include/pch-*.cpp`, and
  `scripts/oss/selftest.sh` builds a CMake `target_precompile_headers`
  project.
- `crates/kiln_driver/tests/codegen.rs`: the codegen path end to end
  (`.ll`, `.s`, `.o`, linking and running, output naming, `-###`, `-v`,
  unused-input warnings, missing tools; `kiln_and_kilnpp`: both names as
  real executables, `--version`, `.c` under each). It runs `kiln++`
  (the build script's symlink beside `CARGO_BIN_EXE_kiln`). Steps needing `llc-19`/`clang++-19`
  are skipped without them. `codegen_flags_classes` compiles `int
  main(){}` with each class-1 and class-2 flag of "Codegen flags" (exit 0,
  no output on stderr) and checks the class-3 error and the bad-value
  errors; `codegen_flags_backend` checks the `llc` arguments under `-###`,
  `-Wp,-MD,<f>` and a `-ffunction-sections` object. The IR effects are
  `tests/codegen/{stack_protector*,unwind_tables_*,no_plt*,wrapv,no_strict_overflow,no_delete_null_pointer_checks}.cpp`;
  `-Wp,-D` is `tests/conformance/cpp.predefined/wp-preprocessor-args.cpp`.
- `crates/kiln_driver/tests/crash.rs` (WP-911): internal compiler
  errors through the real `kiln++` with `-fkiln-crash-for-testing=`. It
  checks:
  - the banner line by line and exit 1;
  - the `.ii` (linemarkers, the source) and the `.sh` (quoting, the input
    replaced), and that replaying the `.sh` crashes again;
  - `$TMPDIR` as the default directory;
  - `-fno-crash-diagnostics` (exit 70, nothing written);
  - backtraces only on request;
  - a preprocessing crash (`.sh` only);
  - a crash outside any input (`.sh` only);
  - standard input.

  The `crash.rs` unit tests cover quoting, the run script, the
  suffixes and the thread funnel. The `kiln_source::activity` unit tests
  cover the breadcrumb, and `tests/conformance/driver.crash/` the
  harness's `expect: ice` tests (spec/harness.md).
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
- `crates/kiln_driver/tests/scan_deps.rs` (WP-982): the dependency
  scanner against clang-scan-deps 22's output (spec/modules.md Tests);
  `scripts/oss/selftest.sh` `cmake_modules`: a CMake `FILE_SET
  CXX_MODULES` build with Ninja, run and rebuilt incrementally, with a
  `CXX_MODULE_STD` target (WP-984).
- `crates/kiln_driver/tests/module_cache.rs` (WP-984): the `std` module
  cache against a miniature libc++ sysroot (miss then hit, both modules,
  invalidation, racing misses, an unwritable root, location and
  switches, `-print-file-name=libc++.modules.json`).
- `run` conformance tests compile and link through the driver.
- Unit tests sit in each module.
- Parity: `cargo xtask test tests/differential/pp tests/conformance/cpp.*
  tests/conformance/lex* --report-parity docs/status/pp-parity.md`. This
  runs `kiln++ -E` against `clang++ -E` on the corpus, including every libc++
  19 header, the sysroot headers, and the torture tests.
