//! The command line: a clang-compatible parser for the options kiln
//! implements (`spec/driver.md` "Options"). Anything else is an
//! `unknown argument` error, never silently ignored.

use kiln_codegen::ComplexRange;
use kiln_pp::{CmdMacro, DirGroup, OptLevel, PicModel};

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

/// Flags that change predefined macros; `None` keeps clang's default.
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
    /// `-E` printing options.
    pub print: PrintArgs,
    /// `-D` / `-U`, in order.
    pub macros: Vec<CmdMacro>,
    /// User include directories, in order (`-I`, `-iquote`, `-isystem`,
    /// `-idirafter`, `-cxx-isystem`).
    pub dirs: Vec<(DirGroup, String)>,
    /// `-F` directories (ignored with a warning).
    pub frameworks: Vec<String>,
    /// `--embed-dir=<dir>`.
    pub embed_dirs: Vec<String>,
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
    /// `-stdlib=`.
    pub stdlib: Option<StdLib>,
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
    /// Errors, in order.
    pub errors: Vec<ArgError>,
    /// The `-fconstexpr-*` options.
    pub constexpr: crate::constexpr_opts::ConstexprOpts,
    /// The `-ftemplate-*` / `-fkiln-instantiation-report` options.
    pub templ: crate::templ_opts::TemplOpts,
    /// `-c`, `-S`, `-emit-llvm`.
    pub compile: CompileArgs,
    /// Linker options, in order.
    pub link: Vec<LinkOpt>,
    /// The last `-O<level>` as written (`-O2`, `-Os`, ...).
    pub opt_flag: Option<String>,
    /// `-###`: print the commands instead of running them.
    pub print_commands: bool,
    /// `-m[no-]outline-atomics` (the last wins): `AArch64`'s
    /// `+outline-atomics` target feature; ignored on x86-64, with clang's
    /// `-Woption-ignored` warning naming the last spelling. `None` is the
    /// target's default (on, as clang 19 on an `AArch64` Linux host).
    pub outline_atomics: Option<bool>,
    /// The `_Complex` multiply/divide algorithm, the last of
    /// `-f[no-]cx-limited-range`, `-f[no-]cx-fortran-rules`,
    /// `-fcomplex-arithmetic=` and `-ffast-math` (basic); `None` is
    /// clang's default (full, or basic under `-Ofast`).
    pub complex_range: Option<ComplexRange>,
}

/// Options taking a value, joined (`-Ifoo`) or separate (`-I foo`).
const JOINED_OR_SEPARATE: &[&str] = &[
    "-cxx-isystem",
    "-idirafter",
    "-iquote",
    "-isysroot",
    "-isystem",
    "-MF",
    "-MQ",
    "-MT",
    "-D",
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
    &["--sysroot", "-resource-dir", "-target", "--target", "-std", "--std"];

impl Args {
    /// Parses `argv` (without the program name) as `kiln++` would.
    pub fn parse(argv: &[String]) -> Args {
        Args::parse_as("kiln++", argv)
    }

    /// Parses `argv` (without the program name) for the program `argv0`,
    /// which sets [`Args::program`] and the default [`Args::mode`].
    pub fn parse_as(argv0: &str, argv: &[String]) -> Args {
        let mut a = Args {
            argv: argv.to_vec(),
            program: program_basename(argv0).to_owned(),
            mode: DriverMode::from_program_name(argv0),
            ..Args::default()
        };
        let mut x: Option<Lang> = None;
        let mut verify = VerifyRaw::default();
        let mut i = 0;
        while i < argv.len() {
            let mut arg = argv[i].as_str();
            i += 1;
            // `-Xclang <arg>`: kiln has no separate frontend command line;
            // the argument is parsed like any other.
            if arg == "-Xclang" {
                let Some(next) = argv.get(i) else {
                    a.errors.push(ArgError::MissingValue(arg.to_owned()));
                    continue;
                };
                arg = next.as_str();
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
            if let Some(r) = a.constexpr.parse_arg(arg).or_else(|| a.templ.parse_arg(arg)) {
                a.errors.extend(r.err());
            } else if !a.flag(arg)
                && !a.complex_flag(arg)
                && !a.predef_flag(arg)
                && !a.diag_flag(arg)
            {
                a.errors.push(ArgError::Unknown(arg.to_owned()));
            }
        }
        a.verify = verify.finish(&mut a.errors);
        a
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
            "-std" | "--std" => self.std = Some(v_owned),
            "-o" => self.output = Some(v_owned),
            "-l" | "-L" => self.link.push(LinkOpt {
                pos: self.inputs.len(),
                arg: format!("{opt}{v}"),
                kind: if opt == "-l" { LinkOptKind::Input } else { LinkOptKind::Option },
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

    /// Plain flags. Returns false if `arg` is not one.
    fn flag(&mut self, arg: &str) -> bool {
        match arg {
            "-E" => self.action = Action::Preprocess,
            "-fsyntax-only" => self.action = Action::SyntaxOnly,
            "-c" => (self.action, self.compile.object) = (Action::Compile, true),
            "-S" => (self.action, self.compile.assembly) = (Action::Compile, true),
            "-emit-llvm" => (self.action, self.compile.emit_llvm) = (Action::Compile, true),
            "-###" => self.print_commands = true,
            "-moutline-atomics" => self.outline_atomics = Some(true),
            "-mno-outline-atomics" => self.outline_atomics = Some(false),
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
            _ => {
                if let Some(v) =
                    arg.strip_prefix("-stdlib=").or_else(|| arg.strip_prefix("--stdlib="))
                {
                    match v {
                        "libc++" => self.stdlib = Some(StdLib::LibCxx),
                        "libstdc++" => self.stdlib = Some(StdLib::LibStdCxx),
                        _ => self.errors.push(ArgError::InvalidStdlib(arg.to_owned())),
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
        if arg == "-ffast-math" {
            // clang 19 renders `-complex-range=basic`; `-fno-fast-math`
            // leaves the range alone.
            self.complex_range = Some(ComplexRange::Basic);
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
            "-Ofast" => (p.opt, p.ofast) = (OptLevel::Speed, true),
            "-Os" | "-Oz" => (p.opt, p.ofast) = (OptLevel::Size, false),
            "-fexceptions" | "-fcxx-exceptions" => p.exceptions = Some(true),
            "-fno-exceptions" | "-fno-cxx-exceptions" => p.exceptions = Some(false),
            "-frtti" => p.rtti = Some(true),
            "-fno-rtti" => p.rtti = Some(false),
            "-fchar8_t" => p.char8_t = Some(true),
            "-fno-char8_t" => p.char8_t = Some(false),
            "-fpic" => p.pic = Some(PicModel::Pic(1)),
            "-fPIC" => p.pic = Some(PicModel::Pic(2)),
            "-fpie" => p.pic = Some(PicModel::Pie(1)),
            "-fPIE" => p.pic = Some(PicModel::Pie(2)),
            "-fno-pic" | "-fno-PIC" | "-fno-pie" | "-fno-PIE" => p.pic = Some(PicModel::None),
            "-ffast-math" => p.fast_math = Some(true),
            "-fno-fast-math" => p.fast_math = Some(false),
            "-ffreestanding" => p.freestanding = Some(true),
            "-fhosted" => p.freestanding = Some(false),
            "-pthread" => p.pthread = true,
            "-fthreadsafe-statics" => p.threadsafe_statics = Some(true),
            "-fno-threadsafe-statics" => p.threadsafe_statics = Some(false),
            "-fsized-deallocation" => p.sized_deallocation = Some(true),
            "-fno-sized-deallocation" => p.sized_deallocation = Some(false),
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
