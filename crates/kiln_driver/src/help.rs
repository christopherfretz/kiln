//! `kiln++ --help` (spec/driver.md "`-v`, `--version`, `--help`"): the
//! option summary, in clang's `--help` layout. Every option it names is
//! accepted by [`crate::args::Args::parse`] (tested below); the complete
//! list is spec/driver.md "Options".

/// The `--help` text. Layout (checked by the tests): an option line is
/// two spaces, the option forms (comma-separated; `[...]` optional,
/// `<...>` a value), and the description at column 26, or the forms alone
/// with the description on the following lines, indented 26. `[kiln]`
/// opens the description of an option clang does not have.
pub(crate) const HELP: &str = "\
OVERVIEW: kiln, a C++26 compiler frontend emitting LLVM IR

USAGE: kiln++ [options] file...
       kiln [options] file...

kiln++ compiles every input as C++. kiln picks by extension: .c, .h and
.i are C (preprocessing only), everything else is C++. -x <language>
overrides both. The options behave as clang's; [kiln] marks kiln's own.
spec/driver.md, \"Options\", lists every accepted option.

ACTIONS:
  -###                    Print the commands to run instead of running them
  -c                      Compile and assemble, but do not link
  -E                      Only run the preprocessor
  -emit-llvm              Use LLVM IR for -S (.ll) and -c (.bc) output
  -fsyntax-only           Only preprocess, parse and check the input
  -o <file>               Write the output to <file>
  -S                      Compile to an assembly file, but do not assemble
  -x <language>           Treat the inputs that follow as <language>: c,
                          c-header, cpp-output, c++, c++-header,
                          c++-cpp-output, c++-module, or none (by extension)

LANGUAGE:
  -std=<standard>         Language standard: c++98 ... c++26, gnu++98 ...
                          gnu++26 (default gnu++17); C: c89 ... c23, gnuNN
  -f[no-]reflection       C++26 static reflection (P2996; on by default
                          at -std=c++26)
  -fcontract-evaluation-semantic=<semantic>
                          Contract evaluation semantic (P2900): ignore,
                          observe, enforce (default) or quick_enforce
  -fclang-compat=<major>
                          [kiln] The clang release whose behaviour to
                          reproduce: 19 or 22 (default 22)
  -f[no-]exceptions       C++ exceptions (default on)
  -f[no-]rtti             Run-time type information (default on)
  -f[no-]char8_t          The char8_t type (default on at C++20 and later)
  -fsigned-char, -funsigned-char
                          The signedness of plain char
  -f[no-]builtin          Treat library functions as builtins (default on)
  -fno-builtin-<function> Do not treat <function> as a builtin
  -ffreestanding          Freestanding implementation (implies -fno-builtin)
  -f[no-]coro-aligned-allocation
                          Aligned allocation for coroutine frames
  -f[no-]sized-deallocation
                          C++14 sized deallocation functions
  -f[no-]threadsafe-statics
                          Thread-safe initialization of static locals
  -fpermissive            Accept the arithmetic between enumerations that
                          C++26 removed (P2864) with a warning

PREPROCESSOR:
  -C, -CC                 Keep comments (-CC: also in macro expansions)
  -D <macro>[=<value>]    Define <macro> (to 1, or to <value>)
  -dD, -dI, -dM           With -E: also print the #defines (-dD) or the
                          #includes (-dI); print only the macro table (-dM)
  --embed-dir=<dir>       Add <dir> to the #embed search path
  -I <dir>                Add <dir> to the include search path
  -idirafter <dir>        Add <dir> after the system include directories
  -include <file>         Include <file> before the main file
  -iquote <dir>           Add <dir> to the \"...\" include search path
  -isystem <dir>          Add <dir> to the system include search path
  -nostdinc, -nostdinc++  No standard (or standard C++) include directories
  -P                      With -E: no line markers
  -U <macro>              Undefine <macro>

DEPENDENCY OUTPUT:
  -M, -MM                 Print the make dependencies (-MM: user headers only)
  -MD, -MMD               Write the make dependencies alongside compilation
  -MF <file>              Write the dependencies to <file>
  -MG                     Treat missing headers as generated files
  -MJ <file>              Write a compilation database entry to <file>
  -MP                     Add a phony target for each dependency
  -fmodule-file-deps      List the module files imports read as dependencies
  -MQ <target>, -MT <target>
                          The dependency target (-MQ: quoted for make)

TARGET AND TOOLCHAIN:
  --target=<triple>       Generate code for <triple>: aarch64 or x86_64 Linux
  -march=<arch>           Target architecture (with +<extension>s on AArch64)
  -mcpu=<cpu>             Target CPU
  --sysroot=<dir>         Use <dir> as the root of the headers and libraries
  -isysroot <dir>         Use <dir> as the root of the headers
  -resource-dir <dir>     The builtin headers and predefines are under <dir>
  -stdlib=<library>       C++ standard library: libstdc++ (default) or libc++
  -f[no-]experimental-library
                          Enable libc++'s experimental library features
  --gcc-toolchain=<dir>   Search for the GCC installation under <dir>
  --gcc-install-dir=<dir> Use the GCC installation in <dir>
  --llvm-dir=<dir>        [kiln] Use the LLVM 19 or 22 installation in <dir>
                          (default: $KILN_LLVM_DIR, else a search)

CODE GENERATION:
  -O<level>               Optimization level: 0, 1, 2, 3, s, z, g or fast
  -fpic, -fPIC, -fpie, -fPIE, -fno-pic
                          Relocation model (default -fPIE)
  -flto[=<kind>]          Link-time optimization: full (default) or thin
  -fno-lto                No link-time optimization
  -fvisibility=<visibility>
                          Default symbol visibility: default, hidden,
                          protected or internal
  -fvisibility-inlines-hidden
                          Give inline member functions hidden visibility
  -f[no-]omit-frame-pointer
                          Omit the frame pointer where possible
  -ffunction-sections, -fdata-sections
                          Place each function (or variable) in its own section
  -fstack-protector, -fstack-protector-strong, -fstack-protector-all
                          Stack smashing protection
  -ftrivial-auto-var-init=<init>
                          Initialize automatic variables: uninitialized,
                          zero or pattern
  -ffast-math             Allow aggressive floating-point optimizations
  -ffp-model=<model>      Floating-point model: precise, strict, fast or
                          aggressive
  -ffp-contract=<mode>    Floating-point contraction: on, off, fast or
                          fast-honor-pragmas

DEBUG INFORMATION:
  -g, -g<level>           DWARF debug information; level 0 (none) to 3
                          (with macros)
  -gline-tables-only      Line tables only
  -gdwarf-<version>       DWARF version: 2 to 5 (default 5)
  -gsplit-dwarf           Put the debug information in a .dwo file
  -gz[=<type>]            Compress the debug sections: none, zlib or zstd
  -fdebug-prefix-map=<old>=<new>
                          Remap <old> to <new> in the debug information
  -ffile-prefix-map=<old>=<new>
                          Remap <old> to <new> in the debug information,
                          __FILE__ and source locations
  -fmacro-prefix-map=<old>=<new>
                          Remap <old> to <new> in __FILE__ and source
                          locations

SANITIZERS AND COVERAGE:
  -fsanitize=<checks>     Runtime checks: address, hwaddress, thread,
                          memory, leak, fuzzer, undefined and its parts
  -fno-sanitize=<checks>  Turn off checks
  -fsanitize-recover=<checks>
                          Continue after a failed check
  -fsanitize-trap=<checks>
                          Trap on a failed check instead of reporting it
  -fsanitize-merge[=<checks>]
                          Let failed checks share one handler call
  -fsanitize-ignorelist=<file>
                          Do not instrument what <file> lists
  -fno-sanitize-ignorelist
                          Drop the default ignorelists
  -ftrapv                 Trap on signed integer overflow
  --coverage              gcov coverage notes and counters
  -fprofile-instr-generate[=<file>]
                          Source-based coverage counters, written to <file>
                          (default default.profraw)

MODULES:
  --precompile            Compile a module interface unit to a BMI (.pcm)
  -fmodule-file=[<module>=]<file>
                          Use <file> for the module: a BMI, or with
                          <module>= also an interface source
  -fmodule-output[=<file>]
                          Also write the BMI when compiling an interface
  -fprebuilt-module-path=<dir>
                          Look for BMIs in <dir>
  -fmodules-cache-path=<dir>
                          Keep the cached std and std.compat modules in <dir>
  -fkiln-module-cache, -fno-kiln-module-cache
                          [kiln] Cache the std and std.compat modules for
                          import std (default on)
  -fmodules-prune-interval=<seconds>
                          Check the module cache for unused entries at
                          most this often (default 7 days; 0: never)
  -fmodules-prune-after=<seconds>
                          Remove module cache entries unused this long
                          (default 31 days; 0: never)

LINKING:
  -fuse-ld=<linker>       Link with <linker> (bfd, gold, lld, ...)
  -l<lib>                 Link with the library <lib>
  -L <dir>                Add <dir> to the library search path
  -nostdlib               No standard libraries or startup files
  -pie, -no-pie           Link a position-independent executable (or not)
  -pthread                POSIX threads
  -rdynamic               Export all symbols to the dynamic symbol table
  -shared                 Link a shared library
  -static                 Link statically
  -Wl,<args>              Pass the comma-separated <args> to the linker
  -Xlinker <arg>          Pass <arg> to the linker

DIAGNOSTICS:
  -W<warning>             Enable the warning <warning>
  -Wno-<warning>          Disable the warning <warning>
  -Werror, -Werror=<warning>
                          Make warnings (or <warning>) errors
  -w                      Suppress all warnings
  -pedantic, -pedantic-errors
                          Warn about (or reject) extensions
  -ferror-limit=<n>       Stop after <n> errors (0: no limit)
  -fdiagnostics-format=<format>
                          Diagnostic format: clang (default) or json
  -f[no-]color-diagnostics
                          Colored diagnostics
  -fcrash-diagnostics-dir=<dir>
                          Write crash reproducers to <dir>
  -fno-crash-diagnostics  Write no crash reproducer
  -Qunused-arguments      No warning for unused driver arguments
  -verify[=<prefixes>]    With -fsyntax-only: check the diagnostics against
                          expected-* comments
  -Xclang <option>        Pass <option> to the frontend (same as the driver)

COMPILE-TIME EVALUATION:
  -fconstexpr-steps=<n>, -fconstexpr-ops-limit=<n>
                          Constant evaluation step limit (default 1048576)
  -fconstexpr-depth=<n>   Constant evaluation call depth limit (default 512)
  -fconstexpr-memory=<size>
                          [kiln] Memory limit per constant evaluation, in
                          bytes or with k, m, g (default 256m)
  -fconstexpr-backtrace-limit=<n>
                          Notes per constant evaluation backtrace (default 10)
  -fconstexpr-report=<file>
                          [kiln] Write a constant evaluation report to <file>
  -fconstexpr-stats       [kiln] Summarize constant evaluation on stderr
  -fconstexpr-profile[=<format>|<file>]
                          [kiln] Profile constant evaluation per function and
                          evaluation site: text (default) or json on stderr,
                          or to <file> (json if it ends in .json)
  -fconstexpr-profile-limit=<n>
                          [kiln] Rows per profile table (default 20; 0: all)
  -ftemplate-depth=<n>    Template instantiation depth limit (default 1024)
  -ftemplate-backtrace-limit=<n>
                          Notes per instantiation backtrace (default 10)
  -fkiln-stack-size=<size>
                          [kiln] Stack of the compiling thread (default 1g)
  -fkiln-instantiation-report[=<format>]
                          [kiln] Report template instantiation costs: text
                          (default), json or all
  -fkiln-memory-report    [kiln] Report the compiler's memory use

KILN DEVELOPMENT:
  -fdump-ast[=<options>]  With -fsyntax-only: dump the AST (options: ids,
                          locs, canonical, implicit, all-files)
  -fdump-ast-filter=<text>
                          Dump only the declarations whose qualified name
                          contains <text>
  -fdump-record-layouts   With -fsyntax-only: dump the record layouts
  -fkiln-dump-cfg         [kiln] With -fsyntax-only: dump each control-flow
                          graph
  -fkiln-cfg-stats        [kiln] With -fsyntax-only: control-flow graph sizes
  -fkiln-cfg-max-elems=<n>
                          [kiln] Control-flow graph element limit per body
  -fkiln-outline-expansions=<n>
                          [kiln] Expansion statement outlining threshold
  -fkiln-reflection-mangling=<scheme>
                          [kiln] Mangling of reflection template arguments:
                          kiln (default), gcc or kiln-hashed
  -fkiln-reproducible-time
                          [kiln] Fixed __DATE__, __TIME__ and __TIMESTAMP__
  -fkiln-source-path-style=<style>
                          [kiln] File names in source locations: clang
                          (default) or gcc

INFORMATION:
  --help                  Print this help
  -v                      Print the version, the GCC installation and the
                          include search paths to standard error
  --version               Print the version
  -dumpmachine            Print the target triple
  -dumpversion            Print the clang compatibility version
  -print-file-name=<file> Print the full path of the library <file>
  -print-prog-name=<name> Print the full path of the program <name>
  -print-resource-dir     Print the resource directory
  -print-search-dirs      Print the program and library search paths
  -print-target-triple    Print the target triple
";

#[cfg(test)]
mod tests {
    use super::HELP;
    use crate::args::Args;

    /// The description column (clang's: options in 2..24, text from 26).
    const COL: usize = 26;

    /// Each option line's forms and the first line of its description.
    fn entries() -> Vec<(Vec<String>, String)> {
        let lines: Vec<&str> = HELP.lines().collect();
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if !line.starts_with("  -") {
                continue;
            }
            let b = line.as_bytes();
            let (opts, desc) = if b.len() > COL && b[COL - 1] == b' ' && b[COL] != b' ' {
                (line[2..COL].trim_end(), line[COL..].to_owned())
            } else {
                let next = lines.get(i + 1).copied().unwrap_or_default();
                assert!(
                    next.len() > COL && next[..COL].trim().is_empty(),
                    "{line:?}: no description at column {COL} on this line or the next"
                );
                (line[2..].trim_end(), next[COL..].to_owned())
            };
            assert!(!opts.contains("  "), "{line:?}: the description must start at column {COL}");
            out.push((opts.split(", ").map(str::to_owned).collect(), desc));
        }
        out
    }

    /// A value for the placeholder `<ph>` of `form`.
    fn sample(form: &str, ph: &str) -> &'static str {
        match ph {
            "file" => "f.txt",
            "dir" => "/d",
            "language" => "c++",
            "standard" => "c++26",
            "semantic" => "observe",
            "major" => "19",
            "version" => "5",
            "function" => "memcpy",
            "macro" => "NAME",
            "value" => "1",
            "target" => "t.o",
            "triple" => "x86_64-unknown-linux-gnu",
            "arch" => "x86-64-v2",
            "cpu" => "generic",
            "library" => "libc++",
            "lib" => "z",
            "level" => "2",
            "kind" => "thin",
            "visibility" => "hidden",
            "init" => "zero",
            "model" => "precise",
            "mode" => "fast",
            "type" => "zlib",
            "old" => "/a",
            "new" => "/b",
            "checks" => "undefined",
            "module" => "m",
            "linker" => "lld",
            "args" => "--as-needed",
            "arg" => "-v",
            "warning" => "unused-variable",
            "n" => "10",
            "format" => "json",
            "option" => "-fsyntax-only",
            "options" => "ids",
            "prefixes" => "expected",
            "size" => "64m",
            "scheme" => "kiln-hashed",
            "style" => "gcc",
            "text" => "main",
            "name" => "ld",
            "seconds" => "604800",
            _ => panic!("{form}: no sample value for <{ph}>; add one to `sample`"),
        }
    }

    /// The command lines `form` stands for: each `[...]` present and
    /// absent, each `<...>` replaced by its [`sample`].
    fn expand(form: &str) -> Vec<Vec<String>> {
        if let Some(open) = form.find('[') {
            let close = open + form[open..].find(']').expect("unbalanced [");
            let (pre, opt, post) = (&form[..open], &form[open + 1..close], &form[close + 1..]);
            let mut v = expand(&format!("{pre}{opt}{post}"));
            v.extend(expand(&format!("{pre}{post}")));
            return v;
        }
        let mut s = String::new();
        let mut rest = form;
        while let Some(open) = rest.find('<') {
            let close = open + rest[open..].find('>').expect("unbalanced <");
            s.push_str(&rest[..open]);
            s.push_str(sample(form, &rest[open + 1..close]));
            rest = &rest[close + 1..];
        }
        s.push_str(rest);
        vec![s.split(' ').map(str::to_owned).collect()]
    }

    /// WP-1027: `--help` advertises no option the parser rejects. Each
    /// form, expanded, is parsed alone with an input; it must produce no
    /// argument error and no diagnostic-option error.
    #[test]
    fn every_listed_option_is_accepted() {
        let entries = entries();
        assert!(entries.len() > 100, "{} option lines", entries.len());
        let mut checked = 0;
        for (forms, _) in &entries {
            for form in forms {
                for mut argv in expand(form) {
                    argv.push("t.cpp".to_owned());
                    let a = Args::parse(&argv);
                    assert_eq!(a.errors, [], "{argv:?} (from `{form}`)");
                    let d = kiln_diag::DiagOptions::from_args(&a.diag_args);
                    assert!(d.pending.is_empty(), "{argv:?} (from `{form}`)");
                    checked += 1;
                }
            }
        }
        assert!(checked > 150, "{checked} command lines");
    }

    /// The options the 1.0 user docs name are listed, and an option
    /// clang does not have says `[kiln]`.
    #[test]
    fn lists_the_option_groups_and_marks_kiln_only() {
        let entries = entries();
        let all: Vec<&str> =
            entries.iter().flat_map(|(f, _)| f.iter().map(String::as_str)).collect();
        for want in [
            "-g",
            "-g<level>",
            "-gdwarf-<version>",
            "-fsanitize=<checks>",
            "--coverage",
            "-fprofile-instr-generate[=<file>]",
            "-flto[=<kind>]",
            "--precompile",
            "-fmodule-file=[<module>=]<file>",
            "-fprebuilt-module-path=<dir>",
            "-fmodules-cache-path=<dir>",
            "-fno-kiln-module-cache",
            "-fmodules-prune-interval=<seconds>",
            "-fmodules-prune-after=<seconds>",
            "-fcontract-evaluation-semantic=<semantic>",
            "-fclang-compat=<major>",
            "--llvm-dir=<dir>",
            "-fconstexpr-memory=<size>",
            "-fkiln-instantiation-report[=<format>]",
            "-fconstexpr-stats",
            "-fconstexpr-profile[=<format>|<file>]",
            "-fconstexpr-profile-limit=<n>",
        ] {
            assert!(all.contains(&want), "--help does not list {want}");
        }
        for (forms, desc) in &entries {
            let kiln_only = forms.iter().any(|f| {
                f.starts_with("-fkiln-")
                    || [
                        "--llvm-dir=",
                        "-fclang-compat=",
                        "-fconstexpr-memory=",
                        "-fconstexpr-report=",
                        "-fconstexpr-stats",
                        "-fconstexpr-profile",
                    ]
                    .iter()
                    .any(|p| f.starts_with(p))
            });
            assert_eq!(desc.starts_with("[kiln]"), kiln_only, "{forms:?}: {desc}");
        }
    }

    /// The identity rule (spec/driver.md "Identity"): the help names kiln,
    /// never calls kiln clang, and fits 80 columns.
    #[test]
    fn says_kiln_and_fits_80_columns() {
        assert!(HELP.starts_with("OVERVIEW: kiln, "));
        assert!(HELP.contains("USAGE: kiln++ [options] file...\n"));
        assert!(!HELP.contains("clang version") && !HELP.contains("OVERVIEW: clang"));
        for l in HELP.lines() {
            assert!(l.len() <= 80, "{l:?}");
        }
    }
}
