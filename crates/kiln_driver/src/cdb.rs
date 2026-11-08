//! `-MJ <file>`: clang's JSON compilation-database fragment
//! (spec/driver.md "`-MJ`", WP-920). One line per frontend job,
//!
//! ```text
//! { "directory": "<cwd>", "file": "<input>", "output": "<out>", "arguments": [...]},
//! ```
//!
//! which build systems concatenate (between `[` and `]`) into a
//! `compile_commands.json`. The arguments are clang's
//! (`Clang::DumpCompilationDatabase`): the driver executable, `-x<type>`,
//! the input, `-o <out>`, `--driver-mode=g++` under `kiln++`, the command
//! line without its inputs, `-o`, `-x` and dependency options (a valued
//! option with a separate value rendered separately, `-target X` as
//! `--target=X`), then `--target=<triple>`. The file is truncated once per
//! driver run and written whatever the jobs' outcome (also under `-###`).

use std::path::Path;

use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_source::Span;

use crate::args::{Args, DriverMode, Input, Lang, Valued, split_valued};
use crate::toolchain::Toolchain;

/// One frontend job of the fragment: its input and output (`None` under
/// `-fsyntax-only`).
pub(crate) struct CdbJob<'a> {
    /// The input as given.
    pub input: &'a Input,
    /// The output file (`-` for standard output).
    pub output: Option<String>,
}

/// Writes the `-MJ` fragment of `jobs`, if `-MJ` was given. Reports a
/// file that cannot be written on `drv`; returns success.
pub(crate) fn write(args: &Args, tc: &Toolchain, jobs: &[CdbJob<'_>], drv: &DiagEngine<'_>) -> bool {
    let Some(path) = &args.deps.cdb_fragment else { return true };
    let text = fragment(args, tc, jobs);
    // `-MJ -` is a file named `-`, as clang's.
    let r = std::fs::write(path, &text).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "No such file or directory".to_owned()
        } else {
            e.to_string()
        }
    });
    if let Err(e) = r {
        Diag::new(DiagId::drv_compilation_database, Span::INVALID).arg(path).arg(e).emit(drv);
        return false;
    }
    true
}

/// The fragment text of `jobs`.
pub(crate) fn fragment(args: &Args, tc: &Toolchain, jobs: &[CdbJob<'_>]) -> String {
    let dir = std::env::current_dir().map(|d| d.display().to_string()).unwrap_or_default();
    let exe = std::env::current_exe().map(|e| e.display().to_string()).unwrap_or_default();
    let rest = render_args(&args.argv);
    let mut out = String::new();
    for job in jobs {
        let input = &job.input.path;
        out.push_str("{ \"directory\": \"");
        escape(&mut out, &dir);
        out.push_str("\", \"file\": \"");
        escape(&mut out, input);
        out.push('"');
        if let Some(o) = &job.output {
            out.push_str(", \"output\": \"");
            escape(&mut out, o);
            out.push('"');
        }
        out.push_str(", \"arguments\": [");
        let mut first = true;
        let mut arg = |s: &str| {
            if !first {
                out.push_str(", ");
            }
            first = false;
            out.push('"');
            escape(&mut out, s);
            out.push('"');
        };
        arg(&exe);
        arg(&format!("-x{}", type_name(args, job.input)));
        arg(input);
        if let Some(o) = &job.output {
            arg("-o");
            arg(o);
        }
        if args.mode == DriverMode::Gxx {
            arg("--driver-mode=g++");
        }
        for a in &rest {
            arg(a);
        }
        arg(&format!("--target={}", tc.target.triple));
        out.push_str("]},\n");
    }
    out
}

/// clang's name of the input's type (`c++`, `c++-header`, ...), from its
/// language and extension.
fn type_name(args: &Args, input: &Input) -> &'static str {
    let ext = Path::new(&input.path).extension().and_then(|e| e.to_str()).unwrap_or("");
    match args.input_lang(input) {
        Lang::C => match ext {
            "h" => "c-header",
            "i" => "cpp-output",
            _ => "c",
        },
        Lang::Cxx => match ext {
            "h" | "hh" | "hpp" | "hxx" | "h++" | "H" | "HPP" => "c++-header",
            "ii" => "c++-cpp-output",
            _ => "c++",
        },
    }
}

/// The dependency options a fragment leaves out (clang's `M_Group`).
const DEP_FLAGS: &[&str] = &[
    "-M",
    "-MM",
    "-MD",
    "-MMD",
    "-MP",
    "-MG",
    "-MV",
    "--dependencies",
    "--user-dependencies",
    "--write-dependencies",
    "--write-user-dependencies",
    "--print-missing-file-dependencies",
];

/// The command line as a fragment lists it, walked as `Args::parse_as`
/// does.
fn render_args(argv: &[String]) -> Vec<String> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        i += 1;
        if arg == "-Xclang" || arg == "-Xlinker" {
            v.push(arg.to_owned());
            if let Some(n) = argv.get(i) {
                v.push(n.clone());
                i += 1;
            }
            continue;
        }
        if arg == "-" || !arg.starts_with('-') || DEP_FLAGS.contains(&arg) {
            continue;
        }
        let Some((opt, value)) = split_valued(arg, argv.get(i).map(String::as_str)) else {
            v.push(arg.to_owned());
            continue;
        };
        let (value, joined) = match value {
            Valued::Joined(s) => (s, true),
            Valued::Next => {
                i += 1;
                (argv[i - 1].as_str(), false)
            }
            Valued::Missing => {
                v.push(arg.to_owned());
                continue;
            }
        };
        match opt {
            "-o" | "-x" | "-MF" | "-MJ" | "-MQ" | "-MT" => {}
            "-target" | "--target" => v.push(format!("--target={value}")),
            "-L" | "-l" => v.push(format!("{opt}{value}")),
            // `--sysroot=`, `-std=`, ... stay as written.
            _ if joined && arg.len() > opt.len() && arg.as_bytes()[opt.len()] == b'=' => {
                v.push(arg.to_owned());
            }
            _ => {
                v.push(opt.to_owned());
                v.push(value.to_owned());
            }
        }
    }
    v
}

/// clang's `llvm::yaml::escape` for the fragment's strings.
fn escape(out: &mut String, s: &str) {
    use std::fmt::Write as _;
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if u32::from(c) < 0x20 || c == '\u{7f}' => {
                let _ = write!(out, "\\x{:02X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> Vec<String> {
        render_args(&s.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
    }

    #[test]
    fn renders_like_clang() {
        assert_eq!(
            r("-c a.cpp -o a.o -Iinc -I inc2 -DX=1 -target x86_64-linux-gnu -Lp -l m -MD -MF a.d"),
            ["-c", "-I", "inc", "-I", "inc2", "-D", "X=1", "--target=x86_64-linux-gnu", "-Lp", "-lm"]
        );
        assert_eq!(
            r("-std=c++20 --sysroot=/s -xc++ -Xclang -fno-rtti -MJ f.json"),
            ["-std=c++20", "--sysroot=/s", "-Xclang", "-fno-rtti"]
        );
    }

    #[test]
    fn escapes_quotes_and_backslashes() {
        let mut s = String::new();
        escape(&mut s, "a\"b\\c\td");
        assert_eq!(s, "a\\\"b\\\\c\\td");
    }
}
