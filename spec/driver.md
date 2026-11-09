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
| `-include <file>` (joined or separate) | `#include "<file>"` at the end of the predefines' `<command line>` section, after the `-D`/`-U` macros, in order (clang `AddImplicitInclude`); found from the working directory first, then the `"..."` search chain |
| `-I`, `-iquote`, `-isystem`, `-idirafter`, `-cxx-isystem` | user search directories; `-I=dir` is relative to the sysroot |
| `-nostdinc`, `-nostdinc++`, `-nostdlibinc`, `-nobuiltininc` | drop toolchain directories (see "Toolchain") |
| `--sysroot`, `-isysroot` | sysroot (`-isysroot` wins for headers) |
| `-resource-dir <dir>` | the builtin headers are in `<dir>/include` |
| `-stdlib=libstdc++` / `libc++` | C++ standard library headers (default libstdc++) |
| `--gcc-toolchain=<dir>` | joined only: the GCC installation search looks under `<dir>` instead of `<sysroot>/usr` (see "Toolchain") |
| `--gcc-install-dir=<dir>` | joined only: `<dir>` (`.../lib/gcc/<triple>/<version>`) is the GCC installation; wins over `--gcc-toolchain=`; without a `crtbegin.o` in it, `'<dir>' does not contain a GCC installation` and no GCC |
| `-fexperimental-library`, `-fno-experimental-library` | libc++'s experimental library: `__has_feature(experimental_library)` and `__has_extension(experimental_library)` are 1 (a `kiln_pp` feature overlay, as clang's cc1 flag; libc++ derives `_LIBCPP_ENABLE_EXPERIMENTAL` from it), and the link step gets `-fexperimental-library` (clang adds `-lc++experimental`); the last wins (`Args::experimental_library`, WP-461) |
| `--target=<triple>` | `aarch64`/`arm64` or `x86_64`/`amd64` Linux; anything else is an error |
| `-march=<arch>[+ext...]`, `-mcpu=<cpu>[+ext...]`, `-msve-vector-bits=<N\|scalable>`, `-target-feature <±feat>` (usually via `-Xclang`) | AArch64 target features and the `__ARM_*` predefines ("Target features" below, WP-459, WP-469) |
| `--embed-dir=<dir>` | `#embed` search directory |
| `-M`, `-MM`, `-MD`, `-MMD`, `-MF`, `-MT`, `-MQ`, `-MP` | dependency output (see below) |
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
| `-fcontract-evaluation-semantic=<ignore\|observe\|enforce\|quick_enforce>` | the contract evaluation semantic of the translation unit (GCC's spelling; `quick-enforce` also accepted; default `enforce`; `ParseOptions::contract_semantic`, `Ast::contract_semantic`); any other value is `unsupported argument`. Until WP-707 only `ignore` compiles a program with contracts to IR; the others are `unsupported: contract evaluation (WP-707)` (spec/contracts.md §6) |
| `-fconstexpr-memory=N[k\|m\|g]` | kiln: live abstract-memory bytes per evaluation (default 256 MiB; binary suffixes) |
| `-f[no-]reflection` | kiln: the P2996 reflection spike (WP-267; default off; the last one wins): `^^`, `[:`, `:]` are lexed (C++26 only), and `^^`, type splices and the kiln intrinsics are accepted (spec/sema.md §8a, spec/parse.md §5.2). `__cpp_reflection` is not predefined. Passed to the preprocessor, the parser and sema for `-E`, `-fsyntax-only` and codegen jobs |
| `-fkiln-reflection-mangling=<kiln\|gcc\|kiln-hashed>` | kiln: how a `std::meta::info` template argument is spelled in a symbol (`kiln_abi::ReflectionMangling`, `CodegenOptions::reflection_mangling`; WP-501). `kiln` (the default): `L u6__info <kind> <operand> E`; `gcc`: GCC 16.1's `L Dm <kind> <operand> E` with `info` itself `Dm`, link-compatible with `g++-16`; `kiln-hashed`: `L u6__info h<32 hex> E`, a prototype. All three give the reflection its reflectee's linkage (spec/reflection.md §8.3). Any other value is `invalid value`; the last one wins. Codegen jobs only |
| `-f[no-]coro-aligned-allocation` | aligned coroutine frame allocation (default off, as clang; the last one wins): sema looks up the `std::align_val_t` forms of the frame's `operator new`/`delete` first (spec/sema.md §7.13). Passed to sema for `-fsyntax-only` and codegen jobs |
| `-fconstexpr-report=<path>` | kiln: write the per-TU NDJSON constant-evaluation report (spec/oven.md "Reporting protocol"); `-fsyntax-only` and every codegen job |
| `-ftemplate-depth=N`, `-ftemplate-backtrace-limit=N`, `-fkiln-instantiation-report[=all\|json\|text]` | template limits and the instantiation report (spec/sema-templates.md §10.3, §11.3) |
| `-fkiln-stack-size=N[k\|m\|g]` | kiln: the stack reserved for the compiling thread (default 1 GiB; at least 4 MiB; the last valid value wins). Running out is the fatal "kiln: stack exhausted during …" (spec/sema-templates.md §10.3), never a crash |
| `-v`, `--verbose`, `--version`, `--help` | see below |
| `-F <dir>` | accepted; warns `ignoring framework directory` |
| `-ftime-trace[=<path>]`, `-ftime-trace-granularity=<n>`, `-ftime-trace-verbose` | accepted, as build systems add them for clang (glaze's tests); each warns `'<flag>' ignored: kiln writes no time trace` (`drv_time_trace_ignored`, group `-Wunused-command-line-argument`, silenced by `-Qunused-arguments`), and no file is written (WP-792) |
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
| `-dD`, `-dI` | parsed; "not implemented yet" (WP-017) |
| `-fmodule-file=<name>=<path>`, `-fprebuilt-module-path=<dir>` | C++20 modules: where module interface sources are found (see "Modules"); `-fmodule-file=<path>` without a name is accepted and ignored |
| `-f[no-]fast-math`, its parts, `-ffp-model=`, `-ffp-contract=`, `-fdenormal-fp-math=`, the FP environment flags | see "Floating-point flags" (WP-806) |
| `-fstack-protector*`, `-f[no-]unwind-tables`, `-f[no-]asynchronous-unwind-tables`, `-f[no-]plt`, `-ffunction-sections`, `-fdata-sections`, `-mtune=`, `-f[no-]wrapv`, `-f[no-]strict-overflow`, `-f[no-]delete-null-pointer-checks`, `-Wp,<list>`, `-Xpreprocessor <arg>`, and the ignored and rejected sets | see "Codegen flags" (WP-605) |

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
| `-march=<cpu>` (x86-64), `-march=native`, `-mcpu=native` (AArch64, without `-march=`), `-mtune=<cpu>` | x86-64: `-march=` names one of clang's x86 CPUs (`x86-64`, `x86-64-v2`..`v4`, `skylake`, `znver4`, ...; clang 22 adds `wildcatlake`, `novalake`, `diamondrapids`) and sets `"target-cpu"` (the default `"tune-cpu"="generic"` is then omitted, as clang); a CPU clang 21 knows also sets clang 21's `"target-features"` and predefined macros for it (spec/landed/driver/WP-630.md, table layout spec/landed/codegen/WP-660.md; `wildcatlake` and `novalake` are not expanded, deviation). `native` on either target emits no `"target-cpu"` and passes `llc -mcpu=native` (the IR attribute would override `-mcpu`); kiln never inspects the host. `-mtune=<cpu>` sets `"tune-cpu"`; `-mtune=native` is accepted and adds nothing. The triple is unchanged |
| `-fwrapv`, `-fno-wrapv`, `-f[no-]wrapv-pointer`, `-fno-strict-overflow`, `-fstrict-overflow` | signed overflow wraps (`CodegenOptions::wrapv`): the last of `-f[no-]wrapv`, else `-fno-strict-overflow`; no `nsw` on signed arithmetic, increments, negation, shifts or VLA sizes. Pointer overflow wraps (`wrapv_pointer`): at `-fclang-compat=19` exactly when `wrapv`; at 22 the last of `-f[no-]wrapv-pointer`, else `-fno-strict-overflow`; no `inbounds` on pointer arithmetic and subscripts, and at 22 none on field GEPs either. The Oven is unchanged: signed overflow in a constant expression stays non-constant ([expr.const]/5), unlike GCC, which folds it as wrapping under `-fwrapv` (deliberate divergence, deferred) |
| `-fno-delete-null-pointer-checks`, `-fdelete-null-pointer-checks` | `CodegenOptions::null_pointer_is_valid`: every definition gets `null_pointer_is_valid`; reference parameters, returns and `this` lose `nonnull` (`this` gets `dereferenceable_or_null`), `nonnull`/`returns_nonnull` attributes, the `operator new` result and loaded references lose `nonnull` (and `!nonnull` metadata), as clang |
| `-fpatchable-function-entry=N[,M]`, `-fzero-call-used-regs=<kind>`, `-mfunction-return=keep\|thunk-extern` (x86), `-fcf-protection[=full\|branch\|return\|none]` (x86), `-mspeculative-load-hardening`, `-fsplit-stack`, `-finstrument-functions`, `-finstrument-functions-after-inlining`, `-finstrument-function-entry-bare`, `-fxray-instrument`, `-fxray-instruction-threshold=N`, `-fxray-ignore-loops` (WP-813) | `CodegenOptions::hardening` (`kiln_codegen::Hardening`): the function attributes and module flags clang 19 emits for each, with the class B attributes overriding them per function (spec/landed/codegen/WP-813.md). `-mspeculative-load-hardening` on x86 also adds `+retpoline-indirect-calls` to the target features; `-finstrument-functions*` at `-O0` runs `default<O0>` so ee-instrument emits the calls; `-fxray-instrument` is passed on to the link. Errors as clang: a bad `-fpatchable-function-entry=` (`invalid argument 'x' to -fpatchable-function-entry=`, or the offset larger than the count), `-fxray-instrument` with `-fpatchable-function-entry=`, a bad `-fxray-instruction-threshold=` under `-fxray-instrument` are driver errors; a bad `-mfunction-return=` / `-fzero-call-used-regs=` / `-fcf-protection=` value (the last of each wins), `-mfunction-return=` off x86 and (when generating code) `-fcf-protection` off x86 are clang's `-cc1` errors, printed without the `kiln++:` prefix and in cc1's order (an invalid `-mfunction-return=` on x86 is reported twice, as clang 19 does). Without `-fxray-instrument` the other `-fxray-*` options are `argument unused during compilation` |
| `-flto`, `-flto=full\|thin\|auto\|jobserver`, `-fno-lto`, `-f[no-]split-lto-unit` (WP-873) | the last of `-flto[=...]` / `-fno-lto` decides (clang `getLTOMode`: `auto` and `jobserver` are full LTO; another value is `unsupported argument`). The module is an LTO unit (spec/codegen.md "LTO": the `ThinLTO` / `EnableSplitLTOUnit` module flags and vtable `!type` metadata); `opt` runs the level's pre-link pipeline (`lto-pre-link<Ox>` / `thinlto-pre-link<Ox>`, at `-O0` too) instead of `default<Ox>`. `-c` (and each compile before a link) writes a bitcode-only `.o` through `opt`: full LTO with `-module-summary`, thin LTO with `--thinlto-bc` (plus `--thinlto-split-lto-unit` under `-fsplit-lto-unit`, default off for thin, always on for full), as clang; `-S -emit-llvm` carries the summary too; `-S` alone writes textual IR as clang does. `-flto[=thin]` is passed on to the link (lld or the system linker with the LLVM gold plugin). `-disable-llvm-passes` runs no pipeline and writes no summary. `-ffat-lto-objects` is accepted and ignored (the object stays bitcode only; follow-up), `-fno-fat-lto-objects` is the default |
| `-mbranch-protection=<spec>`, `-msign-return-address=none\|non-leaf\|all` (AArch64; WP-846) | `CodegenOptions::branch_protection` (`kiln_abi::BranchProtection`, clang 19 `CollectARMPACBTIOptions` and `ARM::parseBranchProtection`): the last of the two wins. `<spec>` is `none`, `standard` (`pac-ret+bti+gcs`) or a `+`-separated list of `bti`, `gcs` and `pac-ret` followed by any of `leaf`, `b-key`, `pc`; `-msign-return-address=` signs with the A key only. Every definition, declaration and call site gets clang's `"sign-return-address"`/`"sign-return-address-key"`, `"branch-target-enforcement"`, `"branch-protection-pauth-lr"` and `"guarded-control-stack"` attributes, the module gets the matching `Min` flags (value 1; 2 at `-fclang-compat=22`, as clang 22), and the predefines gain `__ARM_FEATURE_PAC_DEFAULT` (bit 0 A key, 1 B key, 2 leaf, 3 PAuth-LR), `__ARM_FEATURE_BTI_DEFAULT` and `__ARM_FEATURE_GCS_DEFAULT` (spec/pp.md "AArch64 target macros"). `__attribute__((target("branch-protection=<spec>")))` replaces the function's attributes (an invalid spec is `invalid or misplaced branch protection specification '<item>'`; off AArch64 the attribute is ignored with `unsupported 'branch-protection' in the 'target' attribute string`). An invalid spec is `unsupported argument '<item>' to option '-mbranch-protection='`; off AArch64 `-mbranch-protection=` is `unsupported option '-mbranch-protection=' for target '<triple>'` and `-msign-return-address=` is unused. kiln models no `-march=` extension enabling PAuth-LR, so `standard` never adds `pc` (spec/landed/codegen/WP-846.md) |
| `-Wp,<a>,<b>,...`, `-Xpreprocessor <arg>` | spliced into the argument list before parsing (`codegen_flags::splice_preprocessor_args`), as GCC: `-Wp,-MD,<f>` / `-Wp,-MMD,<f>` become `-MD -MF <f>` / `-MMD -MF <f>`, any other list is split at commas (empty pieces dropped), so `-Wp,-D_FORTIFY_SOURCE=2` is `-D_FORTIFY_SOURCE=2`; `-Xpreprocessor <arg>` is `<arg>` |

**2. Accepted and ignored silently** (no observable effect in kiln today;
do not add a diagnostic): `-f[no-]stack-clash-protection`,
`-f[no-]common`, `-fstack-usage`,
`-fno-trapv`, `-fno-short-enums`, `-m64` (every kiln target is 64-bit;
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
for that flag (WP-873 follow-ups). An empty `-mtune=` is `unsupported argument '<v>' to option '<opt>'`.
(`-fexcess-precision=`, `-f[no-]math-errno` and `-ffp-model=` are in
"Floating-point flags", WP-806.)

**3. Rejected** with `drv_unsupported_flag`, the error `'<flag>' is not
supported by kiln` (exit 1, no job runs; not the generic `unknown argument`):
`-ftrapv`, `-fshort-enums`, the `-fsanitize*` and `-fno-sanitize*`
options "Sanitizers" below does not list (`-fsanitize-coverage=`,
`-fsanitize-cfi-*`, ...), `-fcoverage-mapping`, `-fprofile-arcs`, `-ftest-coverage`, `--coverage`,
`-fprofile-instr-generate[=...]`, `-fprofile-generate[=...]`,
`-fprofile-instr-use[=...]`, `-fprofile-use[=...]`, and under
`-fxray-instrument` every `-fxray-*` option but the threshold and
`-fxray-ignore-loops` (`-fxray-modes=`, `-fxray-always-instrument=`, ...;
WP-813).

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
  `AllowFPOpFusion`; `fast-honor-pragmas` does not).
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
  integer-divide-by-zero, shift-base, shift-exponent, return, unreachable
  and vla-bound checks (spec/codegen.md "Sanitizers"); the other members
  are accepted and emit no check yet (spec/landed/codegen/WP-675.md).
- `-fsanitize-ignorelist=`/`-fsanitize-blacklist=` (the file must exist:
  `no such file or directory: '<f>'`; not applied yet),
  `-fno-sanitize-ignorelist`, `-f[no-]sanitize-address-use-after-scope`,
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

Tests: `sanitize.rs` unit tests; `crates/kiln_driver/tests/codegen.rs`
`sanitizer_frontend_attributes`; `tests/conformance/driver.sanitize/`.

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
<prefix>/lib/kiln/<major>/include/       resource/include: clang 19's builtin
                                         headers, contracts, module.modulemap
<prefix>/lib/kiln/<major>/predefines/    resource/predefines/*.txt
<prefix>/share/doc/kiln/                 README.md, LICENSE-MIT, LICENSE-APACHE,
                                         COMPATIBILITY.md, STABILITY.md
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
  `clang++` ("Code generation", "Tools"): an LLVM 19 (or 22) installation
  with `llc`, `llvm-as` and `clang++`, found through `$KILN_LLVM_DIR`
  (default `/usr/lib/llvm-19`) or the versioned names on `PATH`. The C++ standard
  library is the system's libstdc++ (GCC's installation, "Toolchain") or
  libc++ (`-stdlib=libc++`, from the LLVM directory). The binary itself
  needs only the C library: the newest glibc symbol version it references
  (`readelf -V bin/kiln`) is the floor, GLIBC_2.39 for the aarch64 build
  of WP-927 (Rust's `std::process` uses `pidfd_spawnp`), built in the CI
  container (Debian trixie, glibc 2.41).

## Toolchain

`Toolchain::new(target, sysroot, resource_dir)` locates:

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
- **LLVM:** `$KILN_LLVM_DIR`, default `/usr/lib/llvm-19` (a build with
  `KILN_DEFAULT_LLVM_DIR` set in its environment uses that instead, for a
  package whose LLVM is installed elsewhere; `toolchain::DEFAULT_LLVM_DIR`).
  This is where libc++'s headers are found. Its major version (`Toolchain::llvm_major`:
  the leading integer of `<dir>/bin/llvm-config --version`, probed once
  per directory per process; if that tool is missing or fails, the `N` of
  a directory named `llvm-N`, after resolving symlinks; else 19) selects
  the IR that differs between LLVM 19 and 22 (spec/codegen.md,
  coroutines and §6). A tree whose name says nothing about its version
  (`/opt/llvm`) is keyed correctly by the probe.
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
  x86-64, `-march=` takes `native` or one of clang's x86 CPU names
  ("Codegen flags") and any other value, and every `-mcpu=`, is the same
  error, `-msve-vector-bits=` is
  `unsupported option '-msve-vector-bits=' for target '<triple>'` (the
  `--target` as written; `drv_unsupported_opt_for_target`) and
  `-target-feature` is ignored. A `-target-feature` value without a sign
  is `unsupported argument`. An unknown `+name` is passed through to
  `"target-features"`.

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
- `-freflection` at C++26: `__cpp_impl_reflection 202603L` (GCC 16.1's
  value, `g++-16 -std=c++26 -freflection -dM -E`; libstdc++ 16's `<meta>`
  keys on it; spec/reflection.md §1 item 5).
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
- **Tools.** Each tool is `$KILN_LLVM_DIR/bin/<tool>` (default
  `/usr/lib/llvm-19` or the build's `KILN_DEFAULT_LLVM_DIR`), else the versioned name (`llc-19`, `llvm-as-19`,
  `clang++-19`) on `PATH`. The IR is piped to the tool's standard input
  (`-o <out> -`).
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
  in the spelling the configured LLVM's (`KILN_LLVM_DIR`) `opt`, `llc`
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
yet") and debug information.

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
   (`KILN_LLVM_DIR`, default as above; clang 22's libc++ has the
   same layout).

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
units it imports. `-fmodule-header`, header-unit module files and
dependency scanning (`-fmodules-reduced-bmi`, P1689 output) are not
supported yet (spec/modules.md §11.9).

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
  (WP-682); it never exits silently. A panic on the compile thread, already
  reported by the panic hook, also exits with 70.
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
- `-print-resource-dir` / `--print-resource-dir` prints the resource
  directory (`-resource-dir`, else the default, "Toolchain") and a
  newline to standard output and exits 0, whatever else is given, as
  clang does (WP-667).
- `--help` prints a short summary of the implemented options and the
  two names' language rule (the usage line says `kiln` under both names).

## Version

kiln's version is the workspace's `[workspace.package] version`
(`Cargo.toml`), semantic versioning with the compatibility scope of
docs/STABILITY.md; it is changed there and nowhere else (WP-927).
`kiln_driver::version` is the only reader:

- `VERSION` (`CARGO_PKG_VERSION`) is what `--version`, `-v`,
  `!llvm.ident` (`kiln version <v>`) and the release archive names
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
  `long_version()` is `<v> (<sha>)`. The identity text that shows it is
  WP-920's.

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
    errors where clang accepts them; on x86-64, `-mcpu=` is "unsupported
    argument" (clang: "unsupported option") and `-target-feature` is
    ignored (clang passes it on); a `-target-feature` value without
    `+`/`-` is an error (clang warns).
19. **Codegen flags** ("Codegen flags" above, WP-605): `-fno-plt` puts
    `nonlazybind` only on functions with no definition in the TU, where
    clang also marks a function used before its later definition and
    template instantiations; x86-64 `-march=<cpu>` expands to clang
    21's features and predefines (also at `-fclang-compat=19`), and not
    at all for `wildcatlake` / `novalake` (spec/landed/driver/WP-630.md);
    `-march=native` emits no `"target-cpu"` and `-mtune=native` no
    `"tune-cpu"` (clang resolves the host); `-fwrapv-pointer` is accepted at `-fclang-compat=19`
    (clang 19 has no such flag); the coverage, profile, `-ftrapv` and
    `-fshort-enums` flags, and the sanitizers "Sanitizers" does not
    list, are errors; and the Oven ignores `-fwrapv`.
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
  are skipped without them. `codegen_flags_classes` compiles `int
  main(){}` with each class-1 and class-2 flag of "Codegen flags" (exit 0,
  no output on stderr) and checks the class-3 error and the bad-value
  errors; `codegen_flags_backend` checks the `llc` arguments under `-###`,
  `-Wp,-MD,<f>` and a `-ffunction-sections` object. The IR effects are
  `tests/codegen/{stack_protector*,unwind_tables_*,no_plt*,wrapv,no_strict_overflow,no_delete_null_pointer_checks}.cpp`;
  `-Wp,-D` is `tests/conformance/cpp.predefined/wp-preprocessor-args.cpp`.
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
