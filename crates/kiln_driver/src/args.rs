//! The command line: a clang-compatible parser for the options kiln
//! implements (`spec/driver.md` "Options"). Anything else is an
//! `unknown argument` error, never silently ignored.

use kiln_codegen::{AutoVarInit, ComplexRange, Visibility};
use kiln_pp::{ClangCompat, CmdMacro, DirGroup, OptLevel, PicModel};
use kiln_sema::LaxVectorConversions;
use kiln_source::SourcePathStyle;

use crate::toolchain::StdLib;

/// The language of an input (`-x`, else the file extension and the driver
/// mode; spec/driver.md "Input languages").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    /// C (`-x c`, `-x c-header`, `.c`, `.h` and `.i` under `kiln`).
    C,
    /// C++ (`-x c++`, `-x c++-header`, everything else).
    Cxx,
}

/// A C input type that C++ mode compiles as C++, as clang's
/// `lookupCXXTypeForCType`: clang's names of the type before and after
/// (`treating '<c>' input as '<c++>' when in C++ mode`).
pub type Promotion = (&'static str, &'static str);

impl Lang {
    /// The language of a `-x` value.
    pub fn from_x(v: &str) -> Option<Lang> {
        match v {
            "c" | "c-header" | "cpp-output" => Some(Lang::C),
            "c++" | "c++-header" | "c++-cpp-output" | "c++-module" => Some(Lang::Cxx),
            _ => None,
        }
    }

    /// clang's C input type of a file by its extension (`.c`, `.h`, `.i`;
    /// case-sensitive, as clang's table: `.C` and `.H` are C++), as the
    /// [`Promotion`] C++ mode applies to it; `None` for anything else.
    pub fn c_type_of(p: &str) -> Option<Promotion> {
        match std::path::Path::new(p).extension().and_then(|e| e.to_str()) {
            Some("c") => Some(("c", "c++")),
            Some("h") => Some(("c-header", "c++-header")),
            Some("i") => Some(("cpp-output", "c++-cpp-output")),
            _ => None,
        }
    }

    /// The language of an input without `-x` in `mode`. Under `kiln`
    /// (`Gcc`) `.c`, `.h` and `.i` are C and standard input (`-`) is C;
    /// everything else is C++. Under `kiln++` (`Gxx`) everything is C++,
    /// and a C extension also returns its [`Promotion`] (the
    /// `-Wdeprecated` warning).
    pub fn of_input(p: &str, mode: DriverMode) -> (Lang, Option<Promotion>) {
        match mode {
            DriverMode::Gxx if p == "-" => (Lang::Cxx, None),
            DriverMode::Gxx => (Lang::Cxx, Lang::c_type_of(p)),
            DriverMode::Gcc if p == "-" => (Lang::C, None),
            DriverMode::Gcc if Lang::c_type_of(p).is_some() => (Lang::C, None),
            DriverMode::Gcc => (Lang::Cxx, None),
        }
    }
}

/// The driver mode, as clang's (`--driver-mode=`, else the program name;
/// spec/driver.md "Driver modes"): what an input without `-x` is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DriverMode {
    /// `kiln` (`--driver-mode=gcc`): C by extension, else C++.
    #[default]
    Gcc,
    /// `kiln++` (`--driver-mode=g++`): every input is C++.
    Gxx,
}

impl DriverMode {
    /// The mode of a program name (`argv[0]`), as clang's driver name
    /// parsing: the basename without `.exe`, then, if that does not decide
    /// it, without a trailing version (`kiln++-0.1`, `clang++-19`). A name
    /// ending in `++` (`kiln++`, `c++`) is C++ mode; anything else is `Gcc`
    /// (clang's preprocessor-only `*-cpp` names too: there is no such mode).
    pub fn from_program_name(argv0: &str) -> DriverMode {
        let base = program_basename(argv0);
        let base = base.strip_suffix(".exe").unwrap_or(base);
        let cxx = |n: &str| n.ends_with("++");
        let unversioned = base.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        let unversioned = unversioned.strip_suffix('-').unwrap_or(unversioned);
        if cxx(base) || cxx(unversioned) { DriverMode::Gxx } else { DriverMode::Gcc }
    }

    /// The mode a `--driver-mode=` value selects (`g++`, `gcc`); `None`
    /// for anything else (clang's `cpp`, `cl`, ... are not implemented).
    pub fn from_flag(v: &str) -> Option<DriverMode> {
        match v {
            "g++" => Some(DriverMode::Gxx),
            "gcc" => Some(DriverMode::Gcc),
            _ => None,
        }
    }
}

/// The basename of `argv0` (the name driver diagnostics carry, as clang's
/// `clang++-19: error:`); `kiln` when it is empty.
pub fn program_basename(argv0: &str) -> &str {
    match argv0.rsplit('/').next() {
        Some(b) if !b.is_empty() => b,
        _ => "kiln",
    }
}

/// An input file (`-` is standard input).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    /// The path as given.
    pub path: String,
    /// The language set by the last `-x` before it, if any.
    pub lang: Option<Lang>,
}

/// `-M` / `-MM` / `-MD` / `-MMD`: which headers the dependency output lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepHeaders {
    /// `-M` / `-MD`: every header.
    All,
    /// `-MM` / `-MMD`: no system headers.
    User,
}

/// Dependency-output options.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DepArgs {
    /// `-M` / `-MM`: the dependencies are the output (implies `-E`, `-w`).
    pub only: Option<DepHeaders>,
    /// `-MD` / `-MMD`: dependencies to a `.d` file as a side effect.
    pub side: Option<DepHeaders>,
    /// `-MF <file>`.
    pub file: Option<String>,
    /// `-MT <t>` (false) / `-MQ <t>` (true: make-quoted), in order.
    pub targets: Vec<(String, bool)>,
    /// `-MP`: a phony target per header.
    pub phony: bool,
}

impl DepArgs {
    /// Which headers to list, if dependencies were requested at all.
    pub fn headers(&self) -> Option<DepHeaders> {
        self.only.or(self.side)
    }
}

/// `-E` printing options.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrintArgs {
    /// `-P`: no line markers.
    pub no_line_markers: bool,
    /// `-C`: keep comments.
    pub comments: bool,
    /// `-CC`: keep comments, also in macros.
    pub macro_comments: bool,
    /// `-dM`: print only the macro definitions.
    pub macros_only: bool,
    /// `-dD`: print macro definitions too.
    pub defines_too: bool,
    /// `-dI`: print `#include` directives too.
    pub includes_too: bool,
}

/// The `-fclang-compat` level without the flag: clang 22 (WP-394; the
/// oracle switch, spec/driver.md); the library default
/// (`ClangCompat::default()`, WP-413) is the same.
pub const DEFAULT_CLANG_COMPAT: ClangCompat = ClangCompat::Clang22;

/// Flags that change predefined macros; `None` keeps clang's default.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PredefArgs {
    /// `-O<n>`.
    pub opt: OptLevel,
    /// `-Ofast` was the last `-O`.
    pub ofast: bool,
    /// `-f[no-]exceptions`.
    pub exceptions: Option<bool>,
    /// `-f[no-]rtti`.
    pub rtti: Option<bool>,
    /// `-f[no-]char8_t`.
    pub char8_t: Option<bool>,
    /// `-fsigned-char` / `-funsigned-char` (and their `-fno-` inverses,
    /// last wins): the signedness of plain `char` (\[basic.fundamental]/7);
    /// `None` keeps the target's (signed on x86-64, unsigned on `AArch64`).
    pub char_signed: Option<bool>,
    /// `-f[no-]pic`, `-f[no-]PIC`, `-f[no-]pie`, `-f[no-]PIE` (last wins).
    pub pic: Option<PicModel>,
    /// `-f[no-]fast-math`.
    pub fast_math: Option<bool>,
    /// `-ffreestanding` / `-fhosted`.
    pub freestanding: Option<bool>,
    /// `-pthread`.
    pub pthread: bool,
    /// `-f[no-]threadsafe-statics`.
    pub threadsafe_statics: Option<bool>,
    /// `-f[no-]sized-deallocation`.
    pub sized_deallocation: Option<bool>,
    /// `-f[no-]coro-aligned-allocation`: aligned coroutine frame
    /// allocation (default off, as clang).
    pub coro_aligned_allocation: bool,
    /// `-fno-inline` (last of it and `-finline`): `__NO_INLINE__` at every
    /// `-O` level, and every definition but an `always_inline` one is
    /// `noinline` (clang `CodeGenOptions::OnlyAlwaysInlining`).
    pub no_inline: bool,
    /// `-f[no-]reflection` (P2996 spike, WP-267): the `^^`/`[: :]` tokens
    /// and the kiln reflection intrinsics. Default off; `__cpp_reflection`
    /// is not defined either way.
    pub reflection: bool,
    /// `-fclang-compat=<major>`: whose predefined macros and feature-query
    /// answers kiln reproduces (spec/pp.md "clang compatibility level").
    pub clang_compat: ClangCompat,
}

/// What to do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Action {
    /// No action flag: compile and link (`codegen_cmd`).
    #[default]
    Link,
    /// `-E` (or `-M` / `-MM`).
    Preprocess,
    /// `-fsyntax-only` (not implemented yet; WP-048).
    SyntaxOnly,
    /// `-c` / `-S` / `-emit-llvm` (`codegen_cmd`; [`CompileArgs`] says
    /// which).
    Compile,
}

/// Which of `-c`, `-S` and `-emit-llvm` were given (`spec/driver.md`
/// "Code generation"). `-S` wins over `-c`, as in clang.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompileArgs {
    /// `-c`: stop after the object file.
    pub object: bool,
    /// `-S`: stop after the assembly (or `.ll` with `-emit-llvm`).
    pub assembly: bool,
    /// `-emit-llvm`: LLVM IR instead of assembly (`-S`) or an object (`-c`).
    pub emit_llvm: bool,
}

/// A linker option, passed through to the link step in command-line order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkOpt {
    /// How many inputs precede it on the command line.
    pub pos: usize,
    /// The option as passed on (`-lm`, `-L/dir`, `-Wl,...`, `-fuse-ld=...`,
    /// `-static`, ...; a separate `-l m` becomes `-lm`).
    pub arg: String,
    /// What a compile-only run (`-c`, `-S`) says about it.
    pub kind: LinkOptKind,
}

/// How a compile-only run reports an unused [`LinkOpt`], as clang does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkOptKind {
    /// `<opt>: 'linker' input unused` (`-l`, `-Wl,`).
    Input,
    /// `argument unused during compilation: '<opt>'` (`-L`, `-fuse-ld=`,
    /// `-shared`, `-rdynamic`, `-pie`, `-no-pie`).
    Option,
    /// Nothing (`-static`, `-nostdlib`).
    Silent,
    /// The value of `-Xlinker <arg>` (`arg` is the value): passed as
    /// `-Xlinker <arg>`, unused as `-Xlinker <arg>: 'linker' input unused`.
    Xlinker,
}

/// A command-line error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgError {
    /// An option kiln does not know.
    Unknown(String),
    /// An option whose value is missing.
    MissingValue(String),
    /// An option with a bad value: `(option as written, value)`.
    InvalidValue(String, String),
    /// A `-verify=` prefix that is not a letter followed by letters,
    /// digits, `-` and `_`.
    InvalidVerifyPrefix(String),
    /// `-stdlib=` with an unknown library.
    InvalidStdlib(String),
    /// An option with a bad integer value: `(argument, value)`.
    InvalidInt(String, String),
    /// A flag with a value outside its set: `(argument, value)`
    /// ("invalid value '{v}' in '{arg}'").
    InvalidFlagValue(String, String),
    /// An option whose value kiln does not support: `(option, value)`
    /// ("unsupported argument '{v}' to option '{opt}'"; `--driver-mode=`).
    UnsupportedValue(String, String),
    /// An option the target does not support: `(option, triple)`
    /// ("unsupported option '{opt}' for target '{triple}'").
    UnsupportedForTarget(String, String),
    /// A GCC/clang flag kiln recognises but deliberately does not
    /// implement (`-ftrapv`, `-fsanitize=`, coverage/profiling;
    /// spec/driver.md "Codegen flags"): "'{arg}' is not supported by kiln".
    UnsupportedByKiln(String),
}

/// The parsed command line.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Args {
    /// The command line as given (for `ClangFeatures::from_args`).
    pub argv: Vec<String>,
    /// The program name driver diagnostics carry (`kiln`, `kiln++`).
    pub program: String,
    /// The driver mode: the program name's, else the last
    /// `--driver-mode=`.
    pub mode: DriverMode,
    /// Input files, in order.
    pub inputs: Vec<Input>,
    /// `-fdump-ast[=<opts>]` (`-fsyntax-only`): the options, `""` for none.
    pub dump_ast: Option<String>,
    /// `-fdump-ast-filter=<substr>`.
    pub dump_ast_filter: Option<String>,
    /// `-fdump-record-layouts` or `-fdump-record-layouts-complete`
    /// (`-fsyntax-only`; clang's `-Xclang` options of those names): the
    /// layout of every complete class, spec/driver.md.
    pub dump_record_layouts: bool,
    /// `-verify[=<prefixes>]` (clang's `-Xclang` option; `-fsyntax-only`):
    /// check the diagnostics against `expected-*` comments, spec/driver.md
    /// "-verify". `None` without `-verify`, or with an invalid prefix.
    pub verify: Option<kiln_diag::verify::VerifyOptions>,
    /// `-fdump-vtable-layouts` (`-fsyntax-only`; clang's `-Xclang` option
    /// of that name): the vtable group of every complete dynamic class,
    /// spec/driver.md.
    pub dump_vtable_layouts: bool,
    /// `-fkiln-dump-cfg` (`-fsyntax-only`): the control-flow graph of
    /// every analysed function body, in clang's `debug.DumpCFG` layout
    /// (spec/sema.md §7.5.1).
    pub dump_cfg: bool,
    /// `-fkiln-cfg-stats` (`-fsyntax-only`): one line per analysed
    /// function body with its graph's size and build time
    /// (spec/sema.md §7.5.1).
    pub cfg_stats: bool,
    /// `-fkiln-memory-report` (`-fsyntax-only` and codegen jobs): after
    /// the diagnostics, one `memory:` line per accounting row of the
    /// translation unit ([`kiln_parse::ParsedTu::memory`]) and a total,
    /// on stderr (spec/driver.md).
    pub memory_report: bool,
    /// GCC's `-fpermissive`: the C++26 enumeration arithmetic error
    /// (P2864) is the C++20 deprecation warning instead
    /// ([`kiln_sema::Sema::permissive`]; spec/driver.md).
    pub permissive: bool,
    /// `-fcontract-evaluation-semantic=ignore|observe|enforce|quick_enforce`
    /// (GCC's spelling; P2900 evaluation semantics, spec/contracts.md §6).
    /// `None` is the default, enforce.
    pub contract_semantic: Option<kiln_ast::stmt::ContractSemantic>,
    /// `-fkiln-cfg-max-elems=N`: the element cap of one body's graph
    /// (spec/sema.md §7.5).
    pub cfg_max_elems: Option<usize>,
    /// `-fkiln-outline-expansions=N`: the size from which an expansion
    /// statement's elements are emitted into helpers
    /// (spec/landed/codegen/WP-793.md; `0` never; the last one wins).
    pub outline_expansions: Option<usize>,
    /// `-o <file>`.
    pub output: Option<String>,
    /// The action.
    pub action: Action,
    /// `-std=` as written.
    pub std: Option<String>,
    /// `--version`.
    pub version: bool,
    /// `-v`.
    pub verbose: bool,
    /// `--help`.
    pub help: bool,
    /// `-print-resource-dir` / `--print-resource-dir`.
    pub print_resource_dir: bool,
    /// `-E` printing options.
    pub print: PrintArgs,
    /// `-D` / `-U`, in order.
    pub macros: Vec<CmdMacro>,
    /// User include directories, in order (`-I`, `-iquote`, `-isystem`,
    /// `-idirafter`, `-cxx-isystem`).
    pub dirs: Vec<(DirGroup, String)>,
    /// `-F` directories (ignored with a warning).
    pub frameworks: Vec<String>,
    /// `-ftime-trace[=<path>]`, `-ftime-trace-granularity=<n>` and
    /// `-ftime-trace-verbose` as written: accepted for build systems that
    /// add them for clang, warned about, and otherwise ignored (kiln
    /// writes no time trace; its own reports are `-fkiln-*-report`).
    pub time_trace: Vec<String>,
    /// `--embed-dir=<dir>`.
    pub embed_dirs: Vec<String>,
    /// `-include <file>`, in order: `#include "<file>"` at the end of the
    /// predefines' `<command line>` section (clang `AddImplicitInclude`).
    pub includes: Vec<String>,
    /// `-ffile-prefix-map=<old>=<new>` and `-fmacro-prefix-map=<old>=<new>`,
    /// in order: the path prefixes `__FILE__` and source locations replace
    /// (`SourceManager::remap_macro_path`).
    pub macro_prefix_map: Vec<(String, String)>,
    /// `-fmodule-file=<name>=<path>`: module interface sources
    /// (spec/modules.md §2).
    pub module_files: Vec<(String, String)>,
    /// `-fprebuilt-module-path=<dir>`, in order.
    pub prebuilt_module_paths: Vec<String>,
    /// `-nostdinc`.
    pub nostdinc: bool,
    /// `-nostdinc++`.
    pub nostdincxx: bool,
    /// `-nostdlibinc`.
    pub nostdlibinc: bool,
    /// `-nobuiltininc`.
    pub nobuiltininc: bool,
    /// `--sysroot`.
    pub sysroot: Option<String>,
    /// `-isysroot`.
    pub isysroot: Option<String>,
    /// `-resource-dir`.
    pub resource_dir: Option<String>,
    /// `--gcc-toolchain=<dir>`: the only prefix the GCC installation
    /// search looks under (spec/driver.md "Toolchain").
    pub gcc_toolchain: Option<String>,
    /// `--gcc-install-dir=<dir>`: the GCC installation itself
    /// (`<prefix>/lib/gcc/<triple>/<version>`).
    pub gcc_install_dir: Option<String>,
    /// `-stdlib=`.
    pub stdlib: Option<StdLib>,
    /// `-f[no-]experimental-library`: `__has_feature(experimental_library)`
    /// (`kiln_pp`, from the raw command line) and the link step's
    /// `-lc++experimental` (passed through to the clang++ link driver).
    pub experimental_library: bool,
    /// `--target=` / `-target`.
    pub target: Option<String>,
    /// Diagnostic options, for `kiln_diag::DiagOptions::parse_arg`.
    pub diag_args: Vec<String>,
    /// Dependency output.
    pub deps: DepArgs,
    /// Predefine-affecting flags.
    pub predef: PredefArgs,
    /// `-trigraphs` / `-f[no-]trigraphs`.
    pub trigraphs: Option<bool>,
    /// `-f[no-]dollars-in-identifiers`.
    pub dollars: Option<bool>,
    /// `-fkiln-reproducible-time`.
    pub reproducible_time: bool,
    /// `-fkiln-source-path-style=<clang|gcc>`: the file spelling of the
    /// source-location builtins and `std::meta::source_location_of`
    /// (`SourceManager::presumed_file_name`; the last one wins).
    pub source_path_style: SourcePathStyle,
    /// `-fkiln-reflection-mangling=<kiln|gcc>`: how `std::meta::info`
    /// template arguments mangle (spec/reflection.md §8.3; the hidden
    /// `kiln-hashed` is a prototype; the last one wins).
    pub reflection_mangling: kiln_abi::ReflectionMangling,
    /// Errors, in order.
    pub errors: Vec<ArgError>,
    /// The `-fconstexpr-*` options.
    pub constexpr: crate::constexpr_opts::ConstexprOpts,
    /// The `-ftemplate-*` / `-fkiln-instantiation-report` options.
    pub templ: crate::templ_opts::TemplOpts,
    /// The GCC/clang code generation flags (`-fstack-protector*`, unwind
    /// tables, `-fwrapv`, …; spec/driver.md "Codegen flags").
    pub codegen: crate::codegen_flags::CodegenFlags,
    /// The floating-point flags, in order (spec/driver.md "Floating-point
    /// flags"; resolved with [`Args::fp_model`]).
    pub fp: crate::fp_flags::FpFlags,
    /// `-c`, `-S`, `-emit-llvm`.
    pub compile: CompileArgs,
    /// Linker options, in order.
    pub link: Vec<LinkOpt>,
    /// The last `-O<level>` as written (`-O2`, `-Os`, ...).
    pub opt_flag: Option<String>,
    /// `-disable-llvm-passes` (a cc1 option, usually behind `-Xclang`):
    /// no `opt` pass runs on the IR, which is written as the frontend
    /// emits it (pre-split coroutines; spec/driver.md "IR optimization").
    pub disable_llvm_passes: bool,
    /// `-###`: print the commands instead of running them.
    pub print_commands: bool,
    /// `-Qunused-arguments`: no `-Wunused-command-line-argument` warning
    /// for an unused linker input or link option (clang's driver option).
    pub quiet_unused_arguments: bool,
    /// `-m[no-]outline-atomics` (the last wins): `AArch64`'s
    /// `+outline-atomics` target feature; ignored on x86-64, with clang's
    /// `-Woption-ignored` warning naming the last spelling. `None` is the
    /// target's default (on, as clang 19 on an `AArch64` Linux host).
    pub outline_atomics: Option<bool>,
    /// `-march=<arch>[+ext...]` as written (the last wins); resolved with
    /// [`crate::features::resolve`] once the target is known.
    pub march: Option<String>,
    /// `-mcpu=<cpu>[+ext...]` as written (the last wins).
    pub mcpu: Option<String>,
    /// `-msve-vector-bits=<bits|scalable>` as written (the last wins).
    pub sve_vector_bits: Option<String>,
    /// `-target-feature <+name|-name>` (clang's `-Xclang` option), in order.
    pub target_features: Vec<String>,
    /// `-f[no-]omit-frame-pointer` (the last wins; `Some(true)` omits);
    /// `None` is the target's default (spec/codegen.md "Frame pointer").
    pub omit_frame_pointer: Option<bool>,
    /// `-m[no-]omit-leaf-frame-pointer` (the last wins; `Some(true)`
    /// omits it in leaf functions); `None` is the target's default.
    pub omit_leaf_frame_pointer: Option<bool>,
    /// `-f[no-]strict-aliasing` (the last wins); `None` is the default
    /// (strict: TBAA metadata above `-O0`, spec/codegen.md "TBAA").
    pub strict_aliasing: Option<bool>,
    /// The `_Complex` multiply/divide algorithm, the last of
    /// `-f[no-]cx-limited-range`, `-f[no-]cx-fortran-rules`,
    /// `-fcomplex-arithmetic=` and `-ffast-math` (basic); `None` is
    /// clang's default (full, or basic under `-Ofast`).
    pub complex_range: Option<ComplexRange>,
    /// `-flax-vector-conversions[=none|integer|all]` and
    /// `-fno-lax-vector-conversions` (the last wins; clang's default
    /// `all`, a bare flag `integer`).
    pub lax_vector_conversions: LaxVectorConversions,
    /// `-ftrivial-auto-var-init=` (the last wins); `None` is the
    /// standard's default (spec/codegen.md §7r).
    pub auto_var_init: Option<AutoVarInit>,
    /// `-ftrivial-auto-var-init-max-size=` (the last wins).
    pub auto_var_init_max_size: Option<u64>,
    /// The last of `-fvisibility=` and `-fvisibility-ms-compat` (clang's
    /// `OPT_fvisibility_EQ`/`OPT_fvisibility_ms_compat` group): the
    /// default visibility of values (`internal` is hidden); `None` is
    /// default visibility (spec/codegen.md "Symbol visibility").
    pub visibility: Option<Visibility>,
    /// The last of those was `-fvisibility-ms-compat`: values hidden,
    /// types (vtables, RTTI) default (clang `-ftype-visibility=default`).
    pub visibility_ms_compat: bool,
    /// The last of `-fvisibility=` and `-fvisibility-ms-compat` was
    /// `-fvisibility=` with an unknown value: the argument, reported per
    /// input as clang's `-cc1` "invalid value 'v' in '-fvisibility=v'".
    pub invalid_visibility: Option<String>,
    /// The last of `-f[no-]visibility-inlines-hidden`.
    pub visibility_inlines_hidden: bool,
    /// The last of `-f[no-]semantic-interposition` (`None`: neither, clang's
    /// `-fhalf-no-semantic-interposition` under `-fpic`).
    pub semantic_interposition: Option<bool>,
    /// The last of `-f[no-]direct-access-external-data`.
    pub direct_access_external_data: Option<bool>,
}

/// Options taking a value, joined (`-Ifoo`) or separate (`-I foo`).
const JOINED_OR_SEPARATE: &[&str] = &[
    "-cxx-isystem",
    "-include",
    "-idirafter",
    "-iquote",
    "-isysroot",
    "-isystem",
    "-MF",
    "-MQ",
    "-MT",
    "-D",
    "-B",
    "-F",
    "-I",
    "-L",
    "-U",
    "-l",
    "-o",
    "-x",
];

/// Options taking a separate value only (`-target foo`), or `=`-joined
/// (`--sysroot=foo`, `-resource-dir=foo`).
const SEPARATE_OR_EQ: &[&str] =
    &["--sysroot", "-resource-dir", "-target-feature", "-target", "--target", "-std", "--std"];

impl Args {
    /// Parses `argv` (without the program name) as `kiln++` would.
    pub fn parse(argv: &[String]) -> Args {
        Args::parse_as("kiln++", argv)
    }

    /// Parses `argv` (without the program name) for the program `argv0`,
    /// which sets [`Args::program`] and the default [`Args::mode`].
    pub fn parse_as(argv0: &str, argv: &[String]) -> Args {
        let spliced = crate::codegen_flags::splice_preprocessor_args(argv);
        let argv = spliced.as_slice();
        let mut a = Args {
            argv: argv.to_vec(),
            program: program_basename(argv0).to_owned(),
            mode: DriverMode::from_program_name(argv0),
            predef: PredefArgs { clang_compat: DEFAULT_CLANG_COMPAT, ..PredefArgs::default() },
            ..Args::default()
        };
        let mut x: Option<Lang> = None;
        let mut verify = VerifyRaw::default();
        let mut i = 0;
        while i < argv.len() {
            let mut arg = argv[i].as_str();
            i += 1;
            // `-Xclang <arg>`: kiln has no separate frontend command line;
            // the argument is parsed like any other. A separate value of a
            // frontend option comes in its own `-Xclang` (`-Xclang
            // -target-feature -Xclang +sve`).
            // `-Xlinker <arg>`: passed to the link step as is, in order.
            if arg == "-Xlinker" {
                let Some(next) = argv.get(i) else {
                    a.errors.push(ArgError::MissingValue(arg.to_owned()));
                    continue;
                };
                i += 1;
                a.link.push(LinkOpt {
                    pos: a.inputs.len(),
                    arg: next.clone(),
                    kind: LinkOptKind::Xlinker,
                });
                continue;
            }
            let mut xclang = false;
            if arg == "-Xclang" {
                let Some(next) = argv.get(i) else {
                    a.errors.push(ArgError::MissingValue(arg.to_owned()));
                    continue;
                };
                arg = next.as_str();
                i += 1;
                xclang = true;
            }
            if xclang
                && argv.get(i).is_some_and(|n| n == "-Xclang")
                && let Some((_, Valued::Next)) =
                    split_valued(arg, argv.get(i + 1).map(String::as_str))
            {
                i += 1;
            }
            if verify.parse_arg(arg) {
                continue;
            }
            if arg == "-" || !arg.starts_with('-') {
                a.inputs.push(Input { path: arg.to_owned(), lang: x });
                continue;
            }
            // Valued options.
            if let Some((opt, value)) = split_valued(arg, argv.get(i).map(String::as_str)) {
                match value {
                    Valued::Joined(v) => a.valued(opt, v, &mut x),
                    Valued::Next => {
                        i += 1;
                        a.valued(opt, &argv[i - 1], &mut x);
                    }
                    Valued::Missing => a.errors.push(ArgError::MissingValue(opt.to_owned())),
                }
                continue;
            }
            if let Some(r) = a.fp.parse_arg(arg) {
                if r.is_ok() {
                    a.fp_flag(arg);
                }
                a.errors.extend(r.err());
            } else if let Some(r) = a
                .constexpr
                .parse_arg(arg)
                .or_else(|| a.templ.parse_arg(arg))
                .or_else(|| a.codegen.parse_arg(arg))
            {
                a.errors.extend(r.err());
            } else if !a.flag(arg)
                && !a.target_flag(arg)
                && !a.stdlib_flag(arg)
                && !a.complex_flag(arg)
                && !a.lax_vector_flag(arg)
                && !a.charset_flag(arg)
                && !a.auto_var_init_flag(arg)
                && !a.contract_semantic_flag(arg)
                && !a.visibility_flag(arg)
                && !a.predef_flag(arg)
                && !a.diag_flag(arg)
            {
                a.errors.push(ArgError::Unknown(arg.to_owned()));
            }
        }
        a.verify = verify.finish(&mut a.errors);
        let _ = a.fp.resolve(a.predef.ofast, None, &mut a.errors);
        a
    }

    /// The floating-point semantics of the command line, with the
    /// target's default denormal mode (spec/driver.md "Floating-point
    /// flags"; the environment flags were errors at parse time).
    #[must_use]
    pub fn fp_model(&self, default_denormal: Option<&str>) -> crate::fp_flags::FpModel {
        self.fp.resolve(self.predef.ofast, default_denormal, &mut Vec::new())
    }

    /// The other effects of the FP flag `arg`: `-ffast-math` and
    /// `-ffp-model=fast` select the basic complex range (clang 19
    /// renders `-complex-range=basic`; nothing turns it back).
    fn fp_flag(&mut self, arg: &str) {
        match arg {
            "-ffast-math" | "-ffp-model=fast" => {
                self.complex_range = Some(ComplexRange::Basic);
                if arg == "-ffast-math" {
                    self.predef.fast_math = Some(true);
                }
            }
            "-fno-fast-math" => self.predef.fast_math = Some(false),
            _ => {}
        }
    }

    /// The language of `input`: its `-x`, else [`Lang::of_input`] in
    /// [`Args::mode`].
    pub fn input_lang(&self, input: &Input) -> Lang {
        input.lang.unwrap_or_else(|| Lang::of_input(&input.path, self.mode).0)
    }

    /// The inputs C++ mode compiles as C++ although their extension is C's
    /// (no `-x`), with the [`Promotion`]: clang's deprecated
    /// `treating 'c' input as 'c++'` warning, once per input.
    pub fn promoted_inputs<'a>(&self, inputs: &[&'a Input]) -> Vec<(&'a Input, Promotion)> {
        inputs
            .iter()
            .filter(|i| i.lang.is_none())
            .filter_map(|&i| Lang::of_input(&i.path, self.mode).1.map(|p| (i, p)))
            .collect()
    }

    fn valued(&mut self, opt: &str, v: &str, x: &mut Option<Lang>) {
        let v_owned = v.to_owned();
        match opt {
            "-D" => self.macros.push(CmdMacro::Define(v_owned)),
            "-U" => self.macros.push(CmdMacro::Undef(v_owned)),
            "-include" => self.includes.push(v_owned),
            "-I" => self.dirs.push((DirGroup::Angled, v_owned)),
            "-iquote" => self.dirs.push((DirGroup::Quoted, v_owned)),
            "-isystem" => self.dirs.push((DirGroup::System, v_owned)),
            "-cxx-isystem" => self.dirs.push((DirGroup::CxxSystem, v_owned)),
            "-idirafter" => self.dirs.push((DirGroup::After, v_owned)),
            "-F" => self.frameworks.push(v_owned),
            "-isysroot" => self.isysroot = Some(v_owned),
            "--sysroot" => self.sysroot = Some(v_owned),
            "-resource-dir" => self.resource_dir = Some(v_owned),
            "-target" | "--target" => self.target = Some(v_owned),
            "-target-feature" => self.target_features.push(v_owned),
            "-std" | "--std" => self.std = Some(v_owned),
            "-o" => self.output = Some(v_owned),
            "-l" | "-L" => self.link.push(LinkOpt {
                pos: self.inputs.len(),
                arg: format!("{opt}{v}"),
                kind: if opt == "-l" { LinkOptKind::Input } else { LinkOptKind::Option },
            }),
            // A program search directory: clang++ finds the linker there
            // (the reference workload's toolchain passes `-B/opt/llvm/bin -fuse-ld=lld`).
            // Silent when compiling, as in clang.
            "-B" => self.link.push(LinkOpt {
                pos: self.inputs.len(),
                arg: format!("-B{v}"),
                kind: LinkOptKind::Silent,
            }),
            "-MF" => self.deps.file = Some(v_owned),
            "-MT" => self.deps.targets.push((v_owned, false)),
            "-MQ" => self.deps.targets.push((v_owned, true)),
            "-x" => match Lang::from_x(v) {
                Some(l) => *x = Some(l),
                None if v == "none" => *x = None,
                None => self.errors.push(ArgError::InvalidValue("-x".to_owned(), v_owned)),
            },
            _ => unreachable!("valued option {opt}"),
        }
    }

    /// `-march=` / `-mcpu=` / `-msve-vector-bits=` (the last of each wins; resolved by
    /// [`crate::features::resolve`]). Returns false if `arg` is neither.
    fn target_flag(&mut self, arg: &str) -> bool {
        if let Some(v) = arg.strip_prefix("-march=") {
            self.march = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("-mcpu=") {
            self.mcpu = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("-msve-vector-bits=") {
            self.sve_vector_bits = Some(v.to_owned());
        } else {
            return false;
        }
        true
    }

    /// Plain flags. Returns false if `arg` is not one.
    #[allow(clippy::too_many_lines, reason = "one arm per plain flag")]
    fn flag(&mut self, arg: &str) -> bool {
        match arg {
            "-E" => self.action = Action::Preprocess,
            "-fsyntax-only" => self.action = Action::SyntaxOnly,
            "-c" => (self.action, self.compile.object) = (Action::Compile, true),
            "-S" => (self.action, self.compile.assembly) = (Action::Compile, true),
            "-emit-llvm" => (self.action, self.compile.emit_llvm) = (Action::Compile, true),
            "-###" => self.print_commands = true,
            "-Qunused-arguments" => self.quiet_unused_arguments = true,
            "-ftime-trace" | "-ftime-trace-verbose" => self.time_trace.push(arg.to_owned()),
            "-disable-llvm-passes" => self.disable_llvm_passes = true,
            "-moutline-atomics" => self.outline_atomics = Some(true),
            "-mno-outline-atomics" => self.outline_atomics = Some(false),
            "-fomit-frame-pointer" => self.omit_frame_pointer = Some(true),
            "-fno-omit-frame-pointer" => self.omit_frame_pointer = Some(false),
            "-momit-leaf-frame-pointer" => self.omit_leaf_frame_pointer = Some(true),
            "-mno-omit-leaf-frame-pointer" => self.omit_leaf_frame_pointer = Some(false),
            "-fstrict-aliasing" => self.strict_aliasing = Some(true),
            "-fno-strict-aliasing" => self.strict_aliasing = Some(false),
            // Debug information is not generated (spec/codegen.md §5).
            "-g" | "-g0" | "-g1" | "-g2" | "-g3" | "-ggdb" | "-ggdb0" | "-ggdb1" | "-ggdb2"
            | "-ggdb3" => {}
            "-static" | "-nostdlib" | "-shared" | "-rdynamic" | "-pie" | "-no-pie" => {
                let kind = if matches!(arg, "-static" | "-nostdlib") {
                    LinkOptKind::Silent
                } else {
                    LinkOptKind::Option
                };
                self.link.push(LinkOpt { pos: self.inputs.len(), arg: arg.to_owned(), kind });
            }
            "--version" => self.version = true,
            "-v" | "--verbose" => self.verbose = true,
            "--help" | "-help" => self.help = true,
            "-print-resource-dir" | "--print-resource-dir" => self.print_resource_dir = true,
            "-P" => self.print.no_line_markers = true,
            "-C" => self.print.comments = true,
            "-CC" => self.print.macro_comments = true,
            "-dM" => self.print.macros_only = true,
            "-dD" => self.print.defines_too = true,
            "-dI" => self.print.includes_too = true,
            "-nostdinc" | "--no-standard-includes" => self.nostdinc = true,
            "-nostdinc++" => self.nostdincxx = true,
            "-nostdlibinc" => self.nostdlibinc = true,
            "-nobuiltininc" => self.nobuiltininc = true,
            "-M" | "--dependencies" => self.deps.only = Some(DepHeaders::All),
            "-MM" | "--user-dependencies" => self.deps.only = Some(DepHeaders::User),
            "-MD" | "--write-dependencies" => self.deps.side = Some(DepHeaders::All),
            "-MMD" | "--write-user-dependencies" => self.deps.side = Some(DepHeaders::User),
            "-MP" => self.deps.phony = true,
            "-trigraphs" | "--trigraphs" | "-ftrigraphs" => self.trigraphs = Some(true),
            "-fno-trigraphs" => self.trigraphs = Some(false),
            "-fdollars-in-identifiers" => self.dollars = Some(true),
            "-fno-dollars-in-identifiers" => self.dollars = Some(false),
            "-fkiln-reproducible-time" => self.reproducible_time = true,
            "-fdump-ast" => self.dump_ast = Some(String::new()),
            "-fdump-record-layouts" | "-fdump-record-layouts-complete" => {
                self.dump_record_layouts = true;
            }
            "-fdump-vtable-layouts" => self.dump_vtable_layouts = true,
            "-fkiln-dump-cfg" => self.dump_cfg = true,
            "-fkiln-cfg-stats" => self.cfg_stats = true,
            "-fkiln-memory-report" => self.memory_report = true,
            "-fpermissive" => self.permissive = true,
            _ => {
                if let Some(v) = arg.strip_prefix("-fkiln-source-path-style=") {
                    match v {
                        "clang" => self.source_path_style = SourcePathStyle::Clang,
                        "gcc" => self.source_path_style = SourcePathStyle::Gcc,
                        _ => self.errors.push(ArgError::InvalidValue(
                            "-fkiln-source-path-style=".to_owned(),
                            v.to_owned(),
                        )),
                    }
                } else if let Some(v) = arg.strip_prefix("-fkiln-reflection-mangling=") {
                    match kiln_abi::ReflectionMangling::parse(v) {
                        Some(m) => self.reflection_mangling = m,
                        None => self
                            .errors
                            .push(ArgError::InvalidFlagValue(arg.to_owned(), v.to_owned())),
                    }
                } else if let Some(v) = arg.strip_prefix("-fkiln-outline-expansions=") {
                    match v.parse::<usize>() {
                        Ok(n) => self.outline_expansions = Some(n),
                        Err(_) => {
                            self.errors.push(ArgError::InvalidInt(arg.to_owned(), v.to_owned()));
                        }
                    }
                } else if let Some(v) = arg.strip_prefix("-fkiln-cfg-max-elems=") {
                    match v.parse::<usize>() {
                        Ok(n) => self.cfg_max_elems = Some(n),
                        Err(_) => {
                            self.errors.push(ArgError::InvalidInt(arg.to_owned(), v.to_owned()));
                        }
                    }
                } else if let Some(v) = arg.strip_prefix("--driver-mode=") {
                    match DriverMode::from_flag(v) {
                        Some(m) => self.mode = m,
                        None => self.errors.push(ArgError::UnsupportedValue(
                            "--driver-mode=".to_owned(),
                            v.to_owned(),
                        )),
                    }
                } else if let Some(v) = arg.strip_prefix("--embed-dir=") {
                    self.embed_dirs.push(v.to_owned());
                } else if let Some(v) = arg.strip_prefix("-fmodule-file=") {
                    // `<path>` alone names a BMI: none in part A, ignored.
                    if let Some((name, path)) = v.split_once('=') {
                        self.module_files.push((name.to_owned(), path.to_owned()));
                    }
                } else if let Some((opt, v)) = ["-ffile-prefix-map=", "-fmacro-prefix-map="]
                    .iter()
                    .find_map(|p| arg.strip_prefix(p).map(|v| (p, v)))
                {
                    // `__FILE__` and source locations; kiln emits no debug
                    // information for `-ffile-prefix-map=` to remap too.
                    match v.split_once('=') {
                        Some((old, new)) => {
                            self.macro_prefix_map.push((old.to_owned(), new.to_owned()));
                        }
                        None => self
                            .errors
                            .push(ArgError::InvalidValue((*opt).to_owned(), v.to_owned())),
                    }
                } else if arg.starts_with("-ftime-trace=")
                    || arg.starts_with("-ftime-trace-granularity=")
                {
                    self.time_trace.push(arg.to_owned());
                } else if let Some(v) = arg.strip_prefix("-fdebug-prefix-map=") {
                    // No debug information: validated, otherwise ignored.
                    if !v.contains('=') {
                        self.errors.push(ArgError::InvalidValue(
                            "-fdebug-prefix-map=".to_owned(),
                            v.to_owned(),
                        ));
                    }
                } else if let Some(v) = arg.strip_prefix("-fprebuilt-module-path=") {
                    self.prebuilt_module_paths.push(v.to_owned());
                } else if let Some(v) = arg.strip_prefix("-fdump-ast-filter=") {
                    self.dump_ast_filter = Some(v.to_owned());
                } else if let Some(v) = arg.strip_prefix("-fdump-ast=") {
                    self.dump_ast = Some(v.to_owned());
                } else if arg.starts_with("-Wl,") || arg.starts_with("-fuse-ld=") {
                    self.link.push(LinkOpt {
                        pos: self.inputs.len(),
                        arg: arg.to_owned(),
                        kind: if arg.starts_with("-Wl,") {
                            LinkOptKind::Input
                        } else {
                            LinkOptKind::Option
                        },
                    });
                } else {
                    return false;
                }
            }
        }
        true
    }

    /// The standard library flags: `-stdlib=` (also `--stdlib=`),
    /// `-f[no-]experimental-library`, `--gcc-toolchain=` and
    /// `--gcc-install-dir=` (joined only, as clang).
    fn stdlib_flag(&mut self, arg: &str) -> bool {
        if let Some(v) = arg.strip_prefix("--gcc-toolchain=") {
            self.gcc_toolchain = Some(v.to_owned());
        } else if let Some(v) = arg.strip_prefix("--gcc-install-dir=") {
            self.gcc_install_dir = Some(v.to_owned());
        } else if let Some(v) =
            arg.strip_prefix("-stdlib=").or_else(|| arg.strip_prefix("--stdlib="))
        {
            match v {
                "libc++" => self.stdlib = Some(StdLib::LibCxx),
                "libstdc++" => self.stdlib = Some(StdLib::LibStdCxx),
                _ => self.errors.push(ArgError::InvalidStdlib(arg.to_owned())),
            }
        } else if matches!(arg, "-fexperimental-library" | "-fno-experimental-library") {
            self.experimental_library = arg == "-fexperimental-library";
        } else {
            return false;
        }
        true
    }

    /// `-ftrivial-auto-var-init=uninitialized|zero|pattern` and
    /// `-ftrivial-auto-var-init-max-size=N`.
    fn auto_var_init_flag(&mut self, arg: &str) -> bool {
        if let Some(v) = arg.strip_prefix("-ftrivial-auto-var-init=") {
            match v {
                "uninitialized" => self.auto_var_init = Some(AutoVarInit::Uninitialized),
                "zero" => self.auto_var_init = Some(AutoVarInit::Zero),
                "pattern" => self.auto_var_init = Some(AutoVarInit::Pattern),
                _ => self.errors.push(ArgError::UnsupportedValue(
                    "-ftrivial-auto-var-init=".to_owned(),
                    v.to_owned(),
                )),
            }
        } else if let Some(v) = arg.strip_prefix("-ftrivial-auto-var-init-max-size=") {
            match v.parse() {
                Ok(n) => self.auto_var_init_max_size = Some(n),
                Err(_) => self.errors.push(ArgError::InvalidInt(arg.to_owned(), v.to_owned())),
            }
        } else {
            return false;
        }
        true
    }

    /// `-fcontract-evaluation-semantic=<semantic>`.
    fn contract_semantic_flag(&mut self, arg: &str) -> bool {
        let Some(v) = arg.strip_prefix("-fcontract-evaluation-semantic=") else {
            return false;
        };
        match kiln_ast::stmt::ContractSemantic::from_flag(v) {
            Some(k) => self.contract_semantic = Some(k),
            None => self.errors.push(ArgError::UnsupportedValue(
                "-fcontract-evaluation-semantic=".to_owned(),
                v.to_owned(),
            )),
        }
        true
    }

    /// `-fvisibility=default|hidden|protected|internal` (`internal` is
    /// hidden, as clang's `LangOptions`), `-fvisibility-ms-compat` and
    /// `-f[no-]visibility-inlines-hidden`, plus the `dso_local` inputs
    /// `-f[no-]semantic-interposition` and
    /// `-f[no-]direct-access-external-data`; the last of each group wins;
    /// an unknown value is clang's "invalid value 'v' in
    /// '-fvisibility=v'" (from `-cc1`, so per input: [`Args::invalid_visibility`]).
    fn visibility_flag(&mut self, arg: &str) -> bool {
        match arg {
            "-fvisibility-inlines-hidden" => self.visibility_inlines_hidden = true,
            "-fno-visibility-inlines-hidden" => self.visibility_inlines_hidden = false,
            "-fsemantic-interposition" => self.semantic_interposition = Some(true),
            "-fno-semantic-interposition" => self.semantic_interposition = Some(false),
            "-fdirect-access-external-data" => self.direct_access_external_data = Some(true),
            "-fno-direct-access-external-data" => self.direct_access_external_data = Some(false),
            "-fvisibility-ms-compat" => {
                (self.visibility, self.visibility_ms_compat) = (Some(Visibility::Hidden), true);
                self.invalid_visibility = None;
            }
            _ => {
                let Some(v) = arg.strip_prefix("-fvisibility=") else { return false };
                let vis = match v {
                    "default" => Visibility::Default,
                    "hidden" | "internal" => Visibility::Hidden,
                    "protected" => Visibility::Protected,
                    _ => {
                        self.invalid_visibility = Some(arg.to_owned());
                        return true;
                    }
                };
                (self.visibility, self.visibility_ms_compat) = (Some(vis), false);
                self.invalid_visibility = None;
            }
        }
        true
    }

    /// clang's complex-range flags (`RenderComplexRangeOption`), the last
    /// wins. `-fcomplex-arithmetic=promoted` is an unsupported value.
    fn complex_flag(&mut self, arg: &str) -> bool {
        self.complex_range = Some(match arg {
            "-fcx-limited-range" => ComplexRange::Basic,
            "-fcx-fortran-rules" => ComplexRange::Improved,
            "-fno-cx-limited-range" | "-fno-cx-fortran-rules" => ComplexRange::Full,
            _ => {
                let Some(v) = arg.strip_prefix("-fcomplex-arithmetic=") else { return false };
                let Some(r) = ComplexRange::from_arg(v) else {
                    self.errors.push(ArgError::UnsupportedValue(
                        "-fcomplex-arithmetic=".to_owned(),
                        v.to_owned(),
                    ));
                    return true;
                };
                r
            }
        });
        true
    }

    /// `-flax-vector-conversions[=v]` / `-fno-lax-vector-conversions`
    /// (clang `Options.td`); an unknown value is clang's
    /// "invalid value 'v' in '-flax-vector-conversions=v'".
    fn lax_vector_flag(&mut self, arg: &str) -> bool {
        self.lax_vector_conversions = match arg {
            "-flax-vector-conversions" => LaxVectorConversions::Integer,
            "-fno-lax-vector-conversions" => LaxVectorConversions::None,
            _ => {
                let Some(v) = arg.strip_prefix("-flax-vector-conversions=") else { return false };
                let Some(k) = LaxVectorConversions::from_arg(v) else {
                    self.errors.push(ArgError::InvalidFlagValue(arg.to_owned(), v.to_owned()));
                    return true;
                };
                k
            }
        };
        true
    }

    /// `-finput-charset=v` / `-fexec-charset=v`: kiln, like clang, reads
    /// and writes UTF-8 only, so `v` must be `UTF-8` (any case); another
    /// value is clang's driver error "invalid value 'v' in
    /// '-finput-charset=v'".
    fn charset_flag(&mut self, arg: &str) -> bool {
        let Some(v) =
            arg.strip_prefix("-finput-charset=").or_else(|| arg.strip_prefix("-fexec-charset="))
        else {
            return false;
        };
        if !v.eq_ignore_ascii_case("utf-8") {
            self.errors.push(ArgError::InvalidFlagValue(arg.to_owned(), v.to_owned()));
        }
        true
    }

    /// The complex range in effect: the last complex-range flag, else
    /// basic under `-Ofast` (unless `-fno-fast-math`), else full.
    #[must_use]
    pub fn complex_range(&self) -> ComplexRange {
        self.complex_range.unwrap_or(if self.predef.ofast && self.predef.fast_math != Some(false) {
            ComplexRange::Basic
        } else {
            ComplexRange::Full
        })
    }

    /// Flags that change predefined macros.
    fn predef_flag(&mut self, arg: &str) -> bool {
        if let Some(v) = arg.strip_prefix("-fclang-compat=") {
            match v.parse().ok().and_then(ClangCompat::from_major) {
                Some(c) => self.predef.clang_compat = c,
                None => self
                    .errors
                    .push(ArgError::UnsupportedValue("-fclang-compat=".to_owned(), v.to_owned())),
            }
            return true;
        }
        if matches!(
            arg,
            "-O0" | "-O" | "-O1" | "-O2" | "-O3" | "-O4" | "-Og" | "-Ofast" | "-Os" | "-Oz"
        ) {
            self.opt_flag = Some(arg.to_owned());
        }
        let p = &mut self.predef;
        match arg {
            "-O0" => (p.opt, p.ofast) = (OptLevel::None, false),
            "-O" | "-O1" | "-O2" | "-O3" | "-O4" | "-Og" => {
                (p.opt, p.ofast) = (OptLevel::Speed, false);
            }
            "-Ofast" => {
                (p.opt, p.ofast) = (OptLevel::Speed, true);
                self.fp.note_ofast();
            }
            "-Os" | "-Oz" => (p.opt, p.ofast) = (OptLevel::Size, false),
            "-fexceptions" | "-fcxx-exceptions" => p.exceptions = Some(true),
            "-fno-exceptions" | "-fno-cxx-exceptions" => p.exceptions = Some(false),
            "-freflection" => p.reflection = true,
            "-fno-reflection" => p.reflection = false,
            "-frtti" => p.rtti = Some(true),
            "-fno-rtti" => p.rtti = Some(false),
            "-fchar8_t" => p.char8_t = Some(true),
            "-fno-char8_t" => p.char8_t = Some(false),
            "-fsigned-char" | "-fno-unsigned-char" => p.char_signed = Some(true),
            "-funsigned-char" | "-fno-signed-char" => p.char_signed = Some(false),
            "-fpic" => p.pic = Some(PicModel::Pic(1)),
            "-fPIC" => p.pic = Some(PicModel::Pic(2)),
            "-fpie" => p.pic = Some(PicModel::Pie(1)),
            "-fPIE" => p.pic = Some(PicModel::Pie(2)),
            "-fno-pic" | "-fno-PIC" | "-fno-pie" | "-fno-PIE" => p.pic = Some(PicModel::None),
            "-ffreestanding" => p.freestanding = Some(true),
            "-fhosted" => p.freestanding = Some(false),
            "-pthread" => p.pthread = true,
            "-fthreadsafe-statics" => p.threadsafe_statics = Some(true),
            "-fno-threadsafe-statics" => p.threadsafe_statics = Some(false),
            "-fsized-deallocation" => p.sized_deallocation = Some(true),
            "-fno-sized-deallocation" => p.sized_deallocation = Some(false),
            "-fcoro-aligned-allocation" => p.coro_aligned_allocation = true,
            "-fno-coro-aligned-allocation" => p.coro_aligned_allocation = false,
            "-fno-inline" => p.no_inline = true,
            "-finline" => p.no_inline = false,
            _ => return false,
        }
        true
    }

    /// Diagnostic options (validated by `kiln_diag`).
    fn diag_flag(&mut self, arg: &str) -> bool {
        let mut probe = kiln_diag::DiagOptions::default();
        if probe.parse_arg(arg) {
            self.diag_args.push(arg.to_owned());
            true
        } else {
            false
        }
    }
}

/// The `-verify` family as given (clang `ParseDiagnosticArgs`).
#[derive(Default)]
struct VerifyRaw {
    /// `-verify` or `-verify=` seen.
    on: bool,
    /// Plain `-verify` seen (adds the `expected` prefix).
    bare: bool,
    /// The `-verify=` values, comma-split, in order.
    prefixes: Vec<String>,
    /// `-verify-ignore-unexpected[=...]`.
    ignore: [bool; 4],
    /// Invalid `-verify-ignore-unexpected=` values, reported after the
    /// prefix errors (clang's `ParseDiagnosticArgs` order).
    bad_levels: Vec<String>,
}

impl VerifyRaw {
    fn parse_arg(&mut self, arg: &str) -> bool {
        use kiln_diag::verify::VerifyLevel;
        if arg == "-verify" {
            (self.on, self.bare) = (true, true);
        } else if let Some(v) = arg.strip_prefix("-verify=") {
            self.on = true;
            self.prefixes.extend(v.split(',').map(str::to_owned));
        } else if arg == "-verify-ignore-unexpected" {
            self.ignore = [true; 4];
        } else if let Some(v) = arg.strip_prefix("-verify-ignore-unexpected=") {
            for level in v.split(',') {
                match VerifyLevel::from_name(level) {
                    Some(l) => self.ignore[l as usize] = true,
                    None => self.bad_levels.push(level.to_owned()),
                }
            }
        } else {
            return false;
        }
        true
    }

    /// clang `checkVerifyPrefixes`: an invalid prefix is an error and
    /// turns `-verify` off.
    fn finish(mut self, errors: &mut Vec<ArgError>) -> Option<kiln_diag::verify::VerifyOptions> {
        if self.bare {
            self.prefixes.push("expected".to_owned());
        }
        let mut ok = self.on;
        for p in &self.prefixes {
            if !kiln_diag::verify::VerifyOptions::is_valid_prefix(p) {
                errors.push(ArgError::InvalidVerifyPrefix(p.clone()));
                ok = false;
            }
        }
        for level in self.bad_levels.drain(..) {
            errors.push(ArgError::InvalidFlagValue("-verify-ignore-unexpected=".to_owned(), level));
        }
        if !ok {
            return None;
        }
        self.prefixes.sort();
        self.prefixes.dedup();
        Some(kiln_diag::verify::VerifyOptions {
            prefixes: self.prefixes,
            ignore_unexpected: self.ignore,
        })
    }
}

enum Valued<'a> {
    Joined(&'a str),
    Next,
    Missing,
}

/// If `arg` is a valued option, the option and where its value is.
fn split_valued<'a>(arg: &'a str, next: Option<&str>) -> Option<(&'static str, Valued<'a>)> {
    for &opt in SEPARATE_OR_EQ {
        if arg == opt {
            return Some((opt, if next.is_some() { Valued::Next } else { Valued::Missing }));
        }
        if let Some(v) = arg.strip_prefix(opt).and_then(|r| r.strip_prefix('=')) {
            return Some((opt, Valued::Joined(v)));
        }
    }
    for &opt in JOINED_OR_SEPARATE {
        if arg == opt {
            return Some((opt, if next.is_some() { Valued::Next } else { Valued::Missing }));
        }
        if let Some(v) = arg.strip_prefix(opt) {
            // `-isystem` must not be read as `-I` + `system`, etc.: the
            // table lists longer options first.
            return Some((opt, Valued::Joined(v)));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Args {
        Args::parse(&s.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
    }

    #[test]
    fn clang_compat_level() {
        assert_eq!(parse("t.cpp").predef.clang_compat, ClangCompat::Clang22);
        let a = parse("-fclang-compat=22 t.cpp");
        assert_eq!(a.predef.clang_compat, ClangCompat::Clang22);
        assert_eq!(a.errors, []);
        assert_eq!(
            parse("-fclang-compat=22 -fclang-compat=19 t.cpp").predef.clang_compat,
            ClangCompat::Clang19
        );
        let bad = parse("-fclang-compat=20 t.cpp");
        assert_eq!(bad.errors, [ArgError::UnsupportedValue("-fclang-compat=".into(), "20".into())]);
    }

    #[test]
    fn joined_and_separate() {
        let a = parse(
            "-E -Ia -I b -isystem c -isystemd -iquote e -idirafter f -DX=1 -D Y -UZ -o out t.cpp",
        );
        assert_eq!(a.action, Action::Preprocess);
        assert_eq!(
            a.dirs,
            [
                (DirGroup::Angled, "a".into()),
                (DirGroup::Angled, "b".into()),
                (DirGroup::System, "c".into()),
                (DirGroup::System, "d".into()),
                (DirGroup::Quoted, "e".into()),
                (DirGroup::After, "f".into()),
            ]
        );
        assert_eq!(
            a.macros,
            [
                CmdMacro::Define("X=1".into()),
                CmdMacro::Define("Y".into()),
                CmdMacro::Undef("Z".into())
            ]
        );
        assert_eq!(a.output.as_deref(), Some("out"));
        assert_eq!(a.inputs, [Input { path: "t.cpp".into(), lang: None }]);
        assert_eq!(a.errors, []);
    }

    #[test]
    fn equals_forms_and_x() {
        let a = parse(
            "--sysroot=/s -resource-dir r -std=c++26 --target=x86_64-linux-gnu -x c a.h -x none b.h -stdlib=libc++",
        );
        assert_eq!(a.sysroot.as_deref(), Some("/s"));
        assert_eq!(a.resource_dir.as_deref(), Some("r"));
        assert_eq!(a.std.as_deref(), Some("c++26"));
        assert_eq!(a.target.as_deref(), Some("x86_64-linux-gnu"));
        assert_eq!(a.stdlib, Some(StdLib::LibCxx));
        assert_eq!(a.inputs[0].lang, Some(Lang::C));
        assert_eq!(a.inputs[1].lang, None);
        assert!(a.errors.is_empty(), "{:?}", a.errors);
    }

    /// WP-525: clang's joined-only GCC installation options.
    #[test]
    fn gcc_toolchain_options() {
        let a = parse("--gcc-toolchain=/opt/gcc --gcc-install-dir=/g/lib/gcc/t/16 t.cpp");
        assert_eq!(a.gcc_toolchain.as_deref(), Some("/opt/gcc"));
        assert_eq!(a.gcc_install_dir.as_deref(), Some("/g/lib/gcc/t/16"));
        assert_eq!(a.errors, []);
        let a = parse("--gcc-toolchain /opt/gcc t.cpp");
        assert_eq!(a.errors, [ArgError::Unknown("--gcc-toolchain".into())]);
    }

    #[test]
    fn experimental_library() {
        assert!(!parse("t.cpp").experimental_library);
        let a = parse("-fexperimental-library t.cpp");
        assert!(a.experimental_library);
        assert_eq!(a.errors, []);
        assert!(
            !parse("-fexperimental-library -fno-experimental-library t.cpp").experimental_library
        );
    }

    #[test]
    fn errors() {
        let a = parse("-E -fbogus -x pascal -stdlib=libfoo -o");
        assert_eq!(
            a.errors,
            [
                ArgError::Unknown("-fbogus".into()),
                ArgError::InvalidValue("-x".into(), "pascal".into()),
                ArgError::InvalidStdlib("-stdlib=libfoo".into()),
                ArgError::MissingValue("-o".into()),
            ]
        );
    }

    #[test]
    fn diag_and_predef_flags() {
        let a = parse(
            "-Wall -Werror -w -ferror-limit=3 -fno-caret-diagnostics -O2 -fno-rtti -fPIC -pthread",
        );
        assert_eq!(
            a.diag_args,
            ["-Wall", "-Werror", "-w", "-ferror-limit=3", "-fno-caret-diagnostics"]
        );
        assert_eq!(a.predef.opt, OptLevel::Speed);
        assert_eq!(a.predef.rtti, Some(false));
        assert_eq!(a.predef.pic, Some(PicModel::Pic(2)));
        assert!(a.predef.pthread);
        assert_eq!(a.errors, []);
    }

    #[test]
    fn outline_atomics_last_wins() {
        assert_eq!(parse("a.cpp").outline_atomics, None);
        assert_eq!(parse("-mno-outline-atomics a.cpp").outline_atomics, Some(false));
        let a = parse("-mno-outline-atomics -moutline-atomics a.cpp");
        assert_eq!((a.outline_atomics, a.errors), (Some(true), vec![]));
    }

    #[test]
    fn strict_aliasing_last_wins() {
        assert_eq!(parse("a.cpp").strict_aliasing, None);
        let a = parse("-fstrict-aliasing -fno-strict-aliasing a.cpp");
        assert_eq!((a.strict_aliasing, a.errors), (Some(false), vec![]));
        assert_eq!(
            parse("-fno-strict-aliasing -fstrict-aliasing a.cpp").strict_aliasing,
            Some(true)
        );
    }

    #[test]
    fn contract_evaluation_semantic() {
        use kiln_ast::stmt::ContractSemantic as S;
        assert_eq!(parse("a.cpp").contract_semantic, None);
        for (v, k) in [
            ("ignore", S::Ignore),
            ("observe", S::Observe),
            ("enforce", S::Enforce),
            ("quick_enforce", S::QuickEnforce),
        ] {
            let a = parse(&format!("-fcontract-evaluation-semantic={v} a.cpp"));
            assert_eq!((a.contract_semantic, a.errors), (Some(k), vec![]));
        }
        assert_eq!(
            parse("-fcontract-evaluation-semantic=assume a.cpp").errors,
            [ArgError::UnsupportedValue("-fcontract-evaluation-semantic=".into(), "assume".into())]
        );
    }

    #[test]
    fn build_system_flags_accepted() {
        // the reference workload's clang toolchain and the reference workload's generator compile.
        let a = parse("-m64 -pipe -fpermissive -fconstexpr-ops-limit=2000000000 a.cpp");
        assert_eq!(a.errors, []);
        assert!(a.permissive);
        assert_eq!(a.constexpr.steps, Some(2_000_000_000));
        assert_eq!(parse("-m32 a.cpp").errors, [ArgError::Unknown("-m32".into())]);
        let a = parse("-mcmodel=medium -mcmodel=large a.cpp");
        assert_eq!(
            (a.codegen.code_model, a.errors),
            (Some(kiln_codegen::CodeModel::Large), vec![])
        );
        assert_eq!(
            parse("-mcmodel=huge a.cpp").errors,
            [ArgError::UnsupportedValue("-mcmodel=".into(), "huge".into())]
        );
        let a = parse("-B/opt/llvm/bin -B /x -fuse-ld=lld a.o");
        assert_eq!(a.errors, []);
        let link: Vec<_> = a.link.iter().map(|l| (l.arg.as_str(), l.kind)).collect();
        assert_eq!(
            link,
            [
                ("-B/opt/llvm/bin", LinkOptKind::Silent),
                ("-B/x", LinkOptKind::Silent),
                ("-fuse-ld=lld", LinkOptKind::Option)
            ]
        );
        let a = parse("-Xlinker --push-state -Xlinker --whole-archive a.o");
        assert_eq!(a.errors, []);
        let link: Vec<_> = a.link.iter().map(|l| (l.arg.as_str(), l.kind)).collect();
        assert_eq!(
            link,
            [("--push-state", LinkOptKind::Xlinker), ("--whole-archive", LinkOptKind::Xlinker)]
        );
        assert_eq!(parse("a.o -Xlinker").errors, [ArgError::MissingValue("-Xlinker".into())]);
        let a = parse("-fno-inline a.cpp");
        assert!(a.predef.no_inline);
        assert!(!parse("-fno-inline -finline a.cpp").predef.no_inline);
    }

    #[test]
    fn implicit_includes() {
        let a = parse("-include a.h -includeb.h -isystem /s t.cpp");
        assert_eq!(a.errors, []);
        assert_eq!(a.includes, ["a.h", "b.h"]);
        assert_eq!(parse("t.cpp -include").errors, [ArgError::MissingValue("-include".into())]);
    }

    #[test]
    fn prefix_maps() {
        let a = parse(
            "-ffile-prefix-map=/src/= -fmacro-prefix-map=/a=/b -fdebug-prefix-map=/c=/d a.cpp",
        );
        assert_eq!(a.errors, []);
        assert_eq!(
            a.macro_prefix_map,
            [("/src/".to_owned(), String::new()), ("/a".to_owned(), "/b".to_owned())]
        );
        assert_eq!(
            parse("-ffile-prefix-map=/src a.cpp").errors,
            [ArgError::InvalidValue("-ffile-prefix-map=".into(), "/src".into())]
        );
    }

    #[test]
    fn outline_expansions() {
        assert_eq!(parse("a.cpp").outline_expansions, None);
        let a = parse("-fkiln-outline-expansions=0 a.cpp");
        assert_eq!((a.outline_expansions, a.errors), (Some(0), vec![]));
        assert_eq!(
            parse("-fkiln-outline-expansions=x a.cpp").errors,
            [ArgError::InvalidInt("-fkiln-outline-expansions=x".into(), "x".into())]
        );
    }

    #[test]
    fn reflection_mangling() {
        use kiln_abi::ReflectionMangling as M;
        assert_eq!(parse("a.cpp").reflection_mangling, M::Kiln);
        let a = parse("-fkiln-reflection-mangling=gcc a.cpp");
        assert_eq!((a.reflection_mangling, a.errors), (M::Gcc, vec![]));
        let a = parse("-fkiln-reflection-mangling=gcc -fkiln-reflection-mangling=kiln a.cpp");
        assert_eq!(a.reflection_mangling, M::Kiln);
        assert_eq!(
            parse("-fkiln-reflection-mangling=kiln-hashed a.cpp").reflection_mangling,
            M::KilnHashed
        );
        assert_eq!(
            parse("-fkiln-reflection-mangling=msvc a.cpp").errors,
            [ArgError::InvalidFlagValue("-fkiln-reflection-mangling=msvc".into(), "msvc".into())]
        );
    }

    #[test]
    fn source_path_style() {
        assert_eq!(parse("a.cpp").source_path_style, SourcePathStyle::Clang);
        let a = parse("-fkiln-source-path-style=gcc a.cpp");
        assert_eq!((a.source_path_style, a.errors), (SourcePathStyle::Gcc, vec![]));
        let a = parse("-fkiln-source-path-style=gcc -fkiln-source-path-style=clang a.cpp");
        assert_eq!(a.source_path_style, SourcePathStyle::Clang);
        assert_eq!(
            parse("-fkiln-source-path-style=msvc a.cpp").errors,
            [ArgError::InvalidValue("-fkiln-source-path-style=".into(), "msvc".into())]
        );
    }

    #[test]
    fn frame_pointer_flags_last_win() {
        let a = parse("a.cpp");
        assert_eq!((a.omit_frame_pointer, a.omit_leaf_frame_pointer), (None, None));
        let a = parse("-fomit-frame-pointer -fno-omit-frame-pointer a.cpp");
        assert_eq!((a.omit_frame_pointer, a.errors), (Some(false), vec![]));
        let a = parse("-fno-omit-frame-pointer -fomit-frame-pointer a.cpp");
        assert_eq!(a.omit_frame_pointer, Some(true));
        let a = parse("-mno-omit-leaf-frame-pointer -momit-leaf-frame-pointer a.cpp");
        assert_eq!((a.omit_leaf_frame_pointer, a.errors), (Some(true), vec![]));
        let a = parse("-momit-leaf-frame-pointer -mno-omit-leaf-frame-pointer a.cpp");
        assert_eq!(a.omit_leaf_frame_pointer, Some(false));
    }

    #[test]
    fn visibility_flags_last_win() {
        use Visibility::{Default, Hidden, Protected};
        let v = |s| {
            let a = parse(s);
            (a.visibility, a.visibility_ms_compat, a.visibility_inlines_hidden, a.errors)
        };
        assert_eq!(v("a.cpp"), (None, false, false, vec![]));
        assert_eq!(v("-fvisibility=hidden a.cpp"), (Some(Hidden), false, false, vec![]));
        assert_eq!(v("-fvisibility=hidden -fvisibility=default a.cpp").0, Some(Default));
        assert_eq!(v("-fvisibility=internal a.cpp").0, Some(Hidden));
        assert_eq!(v("-fvisibility=protected a.cpp").0, Some(Protected));
        assert_eq!(v("-fvisibility=default -fvisibility-ms-compat a.cpp").0, Some(Hidden));
        assert!(v("-fvisibility-ms-compat a.cpp").1);
        assert!(!v("-fvisibility-ms-compat -fvisibility=hidden a.cpp").1);
        assert!(v("-fvisibility-inlines-hidden a.cpp").2);
        assert!(!v("-fvisibility-inlines-hidden -fno-visibility-inlines-hidden a.cpp").2);
        let bad = |s| parse(s).invalid_visibility;
        assert_eq!(bad("-fvisibility=bogus a.cpp").as_deref(), Some("-fvisibility=bogus"));
        assert_eq!(bad("-fvisibility=bogus -fvisibility=hidden a.cpp"), None);
        let si = |s| parse(s).semantic_interposition;
        assert_eq!(si("a.cpp"), None);
        assert_eq!(si("-fsemantic-interposition -fno-semantic-interposition a.cpp"), Some(false));
        let da = |s| parse(s).direct_access_external_data;
        assert_eq!(da("-fno-direct-access-external-data a.cpp"), Some(false));
    }

    #[test]
    fn complex_range_last_wins() {
        use ComplexRange::{Basic, Full, Improved};
        let r = |s| parse(s).complex_range();
        assert_eq!(r("a.cpp"), Full);
        assert_eq!(r("-fcx-limited-range a.cpp"), Basic);
        assert_eq!(r("-fcx-limited-range -fno-cx-limited-range a.cpp"), Full);
        assert_eq!(r("-fcx-fortran-rules a.cpp"), Improved);
        assert_eq!(r("-fcx-fortran-rules -fno-cx-fortran-rules a.cpp"), Full);
        assert_eq!(r("-fcomplex-arithmetic=improved -fcx-limited-range a.cpp"), Basic);
        assert_eq!(r("-fcx-limited-range -fcomplex-arithmetic=full a.cpp"), Full);
        assert_eq!(r("-ffast-math a.cpp"), Basic);
        assert_eq!(r("-ffast-math -fno-cx-limited-range a.cpp"), Full);
        assert_eq!(r("-fcx-limited-range -fno-fast-math a.cpp"), Basic);
        assert_eq!(r("-Ofast a.cpp"), Basic);
        assert_eq!(r("-Ofast -fno-fast-math a.cpp"), Full);
        for v in ["promoted", "bogus"] {
            let a = parse(&format!("-fcomplex-arithmetic={v} a.cpp"));
            let e = ArgError::UnsupportedValue("-fcomplex-arithmetic=".into(), v.into());
            assert_eq!((a.complex_range, a.errors), (None, vec![e]));
        }
    }

    #[test]
    fn lax_vector_conversions() {
        use LaxVectorConversions::{All, Integer, None};
        let l = |s: &str| parse(s).lax_vector_conversions;
        assert_eq!(l("a.cpp"), All);
        assert_eq!(l("-flax-vector-conversions a.cpp"), Integer);
        assert_eq!(l("-fno-lax-vector-conversions a.cpp"), None);
        assert_eq!(l("-flax-vector-conversions=none -flax-vector-conversions=all a.cpp"), All);
        assert_eq!(l("-flax-vector-conversions=integer a.cpp"), Integer);
        let a = parse("-flax-vector-conversions=bogus a.cpp");
        let e = ArgError::InvalidFlagValue("-flax-vector-conversions=bogus".into(), "bogus".into());
        assert_eq!((a.lax_vector_conversions, a.errors), (All, vec![e]));
    }

    #[test]
    fn deps_and_print() {
        let a = parse("-MD -MF d -MT t -MQ q -MP -P -C -dM -");
        assert_eq!(a.deps.side, Some(DepHeaders::All));
        assert_eq!(a.deps.file.as_deref(), Some("d"));
        assert_eq!(a.deps.targets, [("t".into(), false), ("q".into(), true)]);
        assert!(a.deps.phony && a.print.no_line_markers && a.print.comments && a.print.macros_only);
        assert_eq!(a.inputs[0].path, "-");
    }

    #[test]
    fn lang_by_extension_and_mode() {
        use DriverMode::{Gcc, Gxx};
        let c = |p| Lang::of_input(p, Gcc);
        assert_eq!(c("a.c"), (Lang::C, None));
        assert_eq!(c("a.h"), (Lang::C, None));
        assert_eq!(c("a.i"), (Lang::C, None));
        assert_eq!(c("-"), (Lang::C, None));
        for p in ["a.cpp", "a.cc", "a.C", "a.H", "a.hpp", "a.ii", "a.cppm", "dir.c/a", "a"] {
            assert_eq!(c(p), (Lang::Cxx, None), "{p}");
        }
        let cxx = |p| Lang::of_input(p, Gxx);
        assert_eq!(cxx("a.c"), (Lang::Cxx, Some(("c", "c++"))));
        assert_eq!(cxx("d/a.h"), (Lang::Cxx, Some(("c-header", "c++-header"))));
        assert_eq!(cxx("a.i"), (Lang::Cxx, Some(("cpp-output", "c++-cpp-output"))));
        assert_eq!(cxx("-"), (Lang::Cxx, None));
        assert_eq!(cxx("a.cpp"), (Lang::Cxx, None));
    }

    #[test]
    fn mode_from_program_name() {
        use DriverMode::{Gcc, Gxx};
        for n in ["kiln++", "/usr/bin/kiln++", "kiln++.exe", "kiln++-0.1", "x/clang++-19", "c++"] {
            assert_eq!(DriverMode::from_program_name(n), Gxx, "{n}");
        }
        let gcc = ["kiln", "/a/b/kiln", "kiln.exe", "kiln-0.1", "clang-19", "", "kiln++/kiln"];
        for n in gcc.into_iter().chain(["kiln-cpp", "clang-cpp-19", "cpp"]) {
            assert_eq!(DriverMode::from_program_name(n), Gcc, "{n}");
        }
        assert_eq!(program_basename("/usr/bin/kiln++"), "kiln++");
        assert_eq!(program_basename(""), "kiln");
    }

    #[test]
    fn driver_mode_flag_last_wins() {
        let p = |a: &[&str]| {
            Args::parse_as("kiln", &a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
        };
        let a = p(&["a.c"]);
        assert_eq!((a.program.as_str(), a.mode), ("kiln", DriverMode::Gcc));
        let a = p(&["--driver-mode=g++", "a.c"]);
        assert_eq!((a.program.as_str(), a.mode), ("kiln", DriverMode::Gxx));
        let a = p(&["--driver-mode=g++", "a.c", "--driver-mode=gcc"]);
        assert_eq!((a.mode, a.errors), (DriverMode::Gcc, vec![]));
        let a = p(&["--driver-mode=cl", "a.c"]);
        assert_eq!(a.errors, [ArgError::UnsupportedValue("--driver-mode=".into(), "cl".into())]);
        assert_eq!(parse("a.c").mode, DriverMode::Gxx);
    }

    /// `-x` beats the mode and the extension; `-x none` gives them back.
    #[test]
    fn x_precedence() {
        let a = parse("-x c a.cpp b.c -x none c.c -x c++ d.c");
        let langs: Vec<Lang> = a.inputs.iter().map(|i| a.input_lang(i)).collect();
        assert_eq!(langs, [Lang::C, Lang::C, Lang::Cxx, Lang::Cxx]);
        let refs: Vec<&Input> = a.inputs.iter().collect();
        let promoted: Vec<&str> =
            a.promoted_inputs(&refs).iter().map(|(i, _)| i.path.as_str()).collect();
        assert_eq!(promoted, ["c.c"]);
        let mut g = a.clone();
        g.mode = DriverMode::Gcc;
        let langs: Vec<Lang> = g.inputs.iter().map(|i| g.input_lang(i)).collect();
        assert_eq!(langs, [Lang::C, Lang::C, Lang::C, Lang::Cxx]);
        assert_eq!(g.promoted_inputs(&refs), []);
    }

    /// spec/driver.md "-verify": clang `ParseDiagnosticArgs` semantics:
    /// bare `-verify` adds `expected`, `-verify=` values are comma-split,
    /// all prefixes are sorted and deduplicated; `-Xclang` forwards the
    /// next argument to the same parser.
    #[test]
    fn verify_prefixes() {
        use kiln_diag::verify::VerifyLevel;
        assert_eq!(parse("-fsyntax-only t.cpp").verify, None);
        let a = parse("-fsyntax-only -verify t.cpp");
        assert_eq!(a.verify.as_ref().map(|v| v.prefixes.clone()), Some(vec!["expected".into()]));
        let a = parse("-Xclang -verify=foo,bar -Xclang -verify=foo -verify t.cpp");
        let v = a.verify.unwrap();
        assert_eq!(v.prefixes, ["bar", "expected", "foo"]);
        assert_eq!(v.ignore_unexpected, [false; 4]);
        assert_eq!(a.errors, []);
        let v = parse("-verify -verify-ignore-unexpected t.cpp").verify.unwrap();
        assert_eq!(v.ignore_unexpected, [true; 4]);
        let v =
            parse("-verify -Xclang -verify-ignore-unexpected=note,warning t.cpp").verify.unwrap();
        let mut want = [false; 4];
        want[VerifyLevel::Note as usize] = true;
        want[VerifyLevel::Warning as usize] = true;
        assert_eq!(v.ignore_unexpected, want);
    }

    /// spec/driver.md "-verify": an invalid prefix is an error and turns
    /// verify off; invalid ignore levels are reported after the prefixes;
    /// a trailing `-Xclang` is a missing value.
    #[test]
    fn verify_errors() {
        let a = parse("-verify=9x,ok,a.b -verify-ignore-unexpected=bogus,note t.cpp");
        assert_eq!(a.verify, None);
        assert_eq!(
            a.errors,
            [
                ArgError::InvalidVerifyPrefix("9x".into()),
                ArgError::InvalidVerifyPrefix("a.b".into()),
                ArgError::InvalidFlagValue("-verify-ignore-unexpected=".into(), "bogus".into()),
            ]
        );
        let a = parse("-verify= t.cpp");
        assert_eq!(a.errors, [ArgError::InvalidVerifyPrefix(String::new())]);
        let a = parse("t.cpp -Xclang");
        assert_eq!(a.errors, [ArgError::MissingValue("-Xclang".into())]);
        let a = parse("-verify=a-b_c1 t.cpp");
        assert_eq!(a.verify.unwrap().prefixes, ["a-b_c1"]);
    }
}
