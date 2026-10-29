//! The command line: a clang-compatible parser for the options kiln
//! implements (`spec/driver.md` "Options"). Anything else is an
//! `unknown argument` error, never silently ignored.

use kiln_pp::{CmdMacro, DirGroup, OptLevel, PicModel};

use crate::toolchain::StdLib;

/// The language of an input (`-x`, else the file extension).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    /// C (`-x c`, `-x c-header`, `.c`).
    C,
    /// C++ (`-x c++`, `-x c++-header`, everything else).
    Cxx,
}

impl Lang {
    /// The language of a `-x` value.
    pub fn from_x(v: &str) -> Option<Lang> {
        match v {
            "c" | "c-header" | "cpp-output" => Some(Lang::C),
            "c++" | "c++-header" | "c++-cpp-output" | "c++-module" => Some(Lang::Cxx),
            _ => None,
        }
    }

    /// The language of a file by its extension: `.c` and `.i` are C,
    /// anything else (`.h` included) is C++.
    pub fn from_path(p: &str) -> Lang {
        match p.rsplit_once('.').map(|(_, e)| e) {
            Some("c" | "i") => Lang::C,
            _ => Lang::Cxx,
        }
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
    /// `-stdlib=` with an unknown library.
    InvalidStdlib(String),
    /// An option with a bad integer value: `(argument, value)`.
    InvalidInt(String, String),
}

/// The parsed command line.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Args {
    /// The command line as given (for `ClangFeatures::from_args`).
    pub argv: Vec<String>,
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
    /// `-c`, `-S`, `-emit-llvm`.
    pub compile: CompileArgs,
    /// Linker options, in order.
    pub link: Vec<LinkOpt>,
    /// The last `-O<level>` as written (`-O2`, `-Os`, ...).
    pub opt_flag: Option<String>,
    /// `-###`: print the commands instead of running them.
    pub print_commands: bool,
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
    /// Parses `argv` (without the program name).
    pub fn parse(argv: &[String]) -> Args {
        let mut a = Args { argv: argv.to_vec(), ..Args::default() };
        let mut x: Option<Lang> = None;
        let mut i = 0;
        while i < argv.len() {
            let arg = argv[i].as_str();
            i += 1;
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
            if let Some(r) = a.constexpr.parse_arg(arg) {
                a.errors.extend(r.err());
            } else if !a.flag(arg) && !a.predef_flag(arg) && !a.diag_flag(arg) {
                a.errors.push(ArgError::Unknown(arg.to_owned()));
            }
        }
        a
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

    /// Flags that change predefined macros.
    fn predef_flag(&mut self, arg: &str) -> bool {
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
    fn deps_and_print() {
        let a = parse("-MD -MF d -MT t -MQ q -MP -P -C -dM -");
        assert_eq!(a.deps.side, Some(DepHeaders::All));
        assert_eq!(a.deps.file.as_deref(), Some("d"));
        assert_eq!(a.deps.targets, [("t".into(), false), ("q".into(), true)]);
        assert!(a.deps.phony && a.print.no_line_markers && a.print.comments && a.print.macros_only);
        assert_eq!(a.inputs[0].path, "-");
    }

    #[test]
    fn lang_from_path() {
        assert_eq!(Lang::from_path("a.c"), Lang::C);
        assert_eq!(Lang::from_path("a.h"), Lang::Cxx);
        assert_eq!(Lang::from_path("dir.c/a"), Lang::Cxx);
    }
}
