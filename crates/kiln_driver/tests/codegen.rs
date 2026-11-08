//! End-to-end tests of the driver's codegen path (`spec/driver.md` "Code
//! generation"): `-S -emit-llvm`, `-S`, `-c`, linking, `-###`, `-v`,
//! output naming, the unused-input warnings and the external-command
//! diagnostics. The `kiln` binary runs in a scratch directory; tests that
//! need `llc-19` / `clang++-19` skip without them.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The `kiln++` beside the `kiln` binary under test (`kiln_driver`'s build
/// script makes it): the C++ driver mode, spec/driver.md "Driver modes".
fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

const MAIN: &str = "extern \"C\" int printf(const char*, ...);\n\
                    int add(int, int);\n\
                    int main() { printf(\"sum %d\\n\", add(2, 3)); return 4; }\n";
const ADD: &str = "int add(int a, int b) { return a + b; }\n";

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-codegen-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn tools() -> bool {
    let ok = have("llc-19") && have("clang++-19");
    if !ok {
        eprintln!("skipping: llc-19 / clang++-19 not found");
    }
    ok
}

fn kiln_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(kilnpp()).args(args).current_dir(dir).output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn setup(name: &str) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("main.cpp"), MAIN).unwrap();
    std::fs::write(d.join("add.cpp"), ADD).unwrap();
    d
}

#[test]
fn emit_llvm_text() {
    let d = setup("ll");
    let o = kiln_in(&d, &["-S", "-emit-llvm", "add.cpp"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    let ll = std::fs::read_to_string(d.join("add.ll")).unwrap();
    assert!(ll.contains("define dso_local noundef i32 @_Z3addii(i32 noundef %0, i32 noundef %1)"));
    assert!(ll.contains("!{!\"kiln version "), "{ll}");
    let o = kiln_in(
        &d,
        &["-S", "-emit-llvm", "-o", "-", "--target=x86_64-unknown-linux-gnu", "add.cpp"],
    );
    assert!(text(&o.stdout).contains("target triple = \"x86_64-unknown-linux-gnu\""));
    // Errors: no output file, exit 1.
    std::fs::write(d.join("bad.cpp"), "int x = ;\n").unwrap();
    let o = kiln_in(&d, &["-S", "-emit-llvm", "bad.cpp"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(text(&o.stderr).ends_with("1 error generated.\n"));
    assert!(!d.join("bad.ll").exists());
}

#[test]
fn compile_only_outputs() {
    if !tools() {
        return;
    }
    let d = setup("c");
    let o = kiln_in(&d, &["-c", "add.cpp", "main.cpp"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    for f in ["add.o", "main.o"] {
        let b = std::fs::read(d.join(f)).unwrap();
        assert_eq!(&b[..4], b"\x7fELF", "{f}");
    }
    let o = kiln_in(&d, &["-S", "add.cpp", "-o", "x.s"]);
    assert_eq!(o.status.code(), Some(0));
    let s = std::fs::read_to_string(d.join("x.s")).unwrap();
    assert!(s.contains("_Z3addii:"), "{s}");
    // `-S` wins over `-c`.
    let o = kiln_in(&d, &["-c", "-S", "add.cpp", "-o", "-"]);
    assert!(text(&o.stdout).contains("_Z3addii:"));
    // Link the objects made above.
    let o = kiln_in(&d, &["add.o", "main.o", "-o", "prog"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    let r = Command::new(d.join("prog")).output().unwrap();
    assert_eq!((r.status.code(), text(&r.stdout).as_str()), (Some(4), "sum 5\n"));
}

#[test]
fn compile_and_link() {
    if !tools() {
        return;
    }
    let d = setup("link");
    let o = kiln_in(&d, &["main.cpp", "add.cpp", "-O2", "-g", "-lm"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    let r = Command::new(d.join("a.out")).output().unwrap();
    assert_eq!((r.status.code(), text(&r.stdout).as_str()), (Some(4), "sum 5\n"));
    // No object files are left behind in the current directory.
    assert!(!d.join("main.o").exists());
    // A link failure is a driver error.
    let o = kiln_in(&d, &["main.cpp", "-o", "p2"]);
    assert_eq!(o.status.code(), Some(1));
    let err = text(&o.stderr);
    assert!(err.contains("undefined reference to `add(int, int)'"), "{err}");
    assert!(
        err.ends_with(
            "kiln++: error: linker command failed with exit code 1 (use -v to see invocation)\n"
        ),
        "{err}"
    );
}

#[test]
fn driver_errors_and_warnings() {
    let d = setup("diag");
    std::fs::write(d.join("lib.o"), "").unwrap();
    let o = kiln_in(&d, &["-c", "add.cpp", "main.cpp", "-o", "x.o"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        text(&o.stderr),
        "kiln++: error: cannot specify -o when generating multiple output files\n"
    );
    let o = kiln_in(&d, &["-emit-llvm", "add.cpp"]);
    assert_eq!(text(&o.stderr), "kiln++: error: -emit-llvm cannot be used when linking\n");
    let args = [
        "-S",
        "-emit-llvm",
        "-lm",
        "add.cpp",
        "-L/x",
        "lib.o",
        "-Wl,-z,now",
        "-fuse-ld=lld",
        "-static",
    ];
    let o = kiln_in(&d, &args);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(
        text(&o.stderr),
        "kiln++: warning: -lm: 'linker' input unused [-Wunused-command-line-argument]\n\
         kiln++: warning: argument unused during compilation: '-L/x' [-Wunused-command-line-argument]\n\
         kiln++: warning: lib.o: 'linker' input unused [-Wunused-command-line-argument]\n\
         kiln++: warning: -Wl,-z,now: 'linker' input unused [-Wunused-command-line-argument]\n\
         kiln++: warning: argument unused during compilation: '-fuse-ld=lld' [-Wunused-command-line-argument]\n"
    );
    let o =
        kiln_in(&d, &["-S", "-emit-llvm", "-Wno-unused-command-line-argument", "-lm", "add.cpp"]);
    assert_eq!(text(&o.stderr), "");
    let o = kiln_in(&d, &["-S", "-emit-llvm", "-Werror", "-lm", "add.cpp"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        text(&o.stderr),
        "kiln++: error: -lm: 'linker' input unused [-Werror,-Wunused-command-line-argument]\n"
    );
    // `-Qunused-arguments` silences them all, wherever it is, also under
    // `-Werror` (clang; WP-683).
    let o = kiln_in(
        &d,
        &["-S", "-emit-llvm", "-Werror", "-lm", "add.cpp", "lib.o", "-L/x", "-Qunused-arguments"],
    );
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(text(&o.stderr), "");
}

/// One driver, two names (spec/driver.md "Driver modes"): `kiln` compiles
/// `.c` as C (not implemented yet), `kiln++` as C++ with clang++'s
/// deprecation warning, and `--driver-mode=g++` makes `kiln` behave as
/// `kiln++`. `--version` is the same under both names.
#[test]
fn kiln_and_kilnpp() {
    let d = setup("modes");
    let kiln = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_kiln")).args(args).current_dir(&d).output().unwrap()
    };
    let first_line = |o: &Output| text(&o.stdout).lines().next().unwrap_or_default().to_owned();
    let v = kiln(&["--version"]);
    assert!(first_line(&v).starts_with("kiln version "), "{}", text(&v.stdout));
    assert_eq!(first_line(&kiln_in(&d, &["--version"])), first_line(&v));

    std::fs::write(d.join("c.c"), "int f(void) { return 0; }\n").unwrap();
    let o = kiln(&["-S", "-emit-llvm", "c.c"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(text(&o.stderr), "error: compiling C is not implemented yet\n");
    let warning = "treating 'c' input as 'c++' when in C++ mode, this behavior is deprecated \
                   [-Wdeprecated]\n";
    let o = kiln_in(&d, &["-S", "-emit-llvm", "c.c"]);
    assert_eq!(
        (o.status.code(), text(&o.stderr)),
        (Some(0), format!("kiln++: warning: {warning}"))
    );
    assert!(std::fs::read_to_string(d.join("c.ll")).unwrap().contains("define"));
    let o = kiln(&["--driver-mode=g++", "-S", "-emit-llvm", "c.c", "-o", "-"]);
    assert_eq!((o.status.code(), text(&o.stderr)), (Some(0), format!("kiln: warning: {warning}")));
    let o = kiln_in(&d, &["-x", "c", "-S", "-emit-llvm", "c.c"]);
    assert_eq!(text(&o.stderr), "error: compiling C is not implemented yet\n");
}

#[test]
fn print_commands() {
    let d = setup("hash");
    std::fs::write(d.join("add.o"), "").unwrap();
    let o = kiln_in(&d, &["-###", "main.cpp", "add.o", "-lm", "-O2"]);
    assert_eq!(o.status.code(), Some(0));
    let err = text(&o.stderr);
    assert!(err.starts_with("kiln version "), "{err}");
    assert!(err.contains(" (in-process) frontend \"main.cpp\" -> \"|\"\n"), "{err}");
    assert!(err.contains("\"-O2\" \"-relocation-model=pic\" \"-filetype=obj\" \"-o\""), "{err}");
    assert!(err.contains("-main.o\" \"add.o\" \"-lm\" \"-o\" \"a.out\"\n"), "{err}");
    assert!(!d.join("a.out").exists());
    let o = kiln_in(&d, &["-###", "-S", "-fno-pie", "main.cpp"]);
    let err = text(&o.stderr);
    assert!(
        err.contains("\"-O0\" \"-relocation-model=static\" \"-o\" \"main.s\" \"-\"\n"),
        "{err}"
    );
    let o = kiln_in(&d, &["-###", "-c", "-emit-llvm", "main.cpp"]);
    assert!(text(&o.stderr).contains("\"-o\" \"main.bc\" \"-\"\n"));
}

/// WP-783, spec/driver.md "IR optimization": at `-O1` and above `-###`
/// prints the `opt` runs `-v` shows after the frontend step (the
/// `-discard-value-names` run only for a textual or bitcode output); at
/// `-O0` (IR-dependent) and under `-disable-llvm-passes` none.
#[test]
fn print_commands_shows_opt() {
    let d = setup("hash-opt");
    let o = kiln_in(&d, &["-###", "-c", "-O3", "add.cpp"]);
    let err = text(&o.stderr);
    let fe = err.find(" (in-process) frontend \"add.cpp\" -> \"|\"\n").expect(&err);
    let opt = err.find("\"-passes=default<O3>\" \"-o\" \"-\" \"-\"\n").expect(&err);
    let llc = err.find("\"-O3\" \"-relocation-model=pic\"").expect(&err);
    assert!(fe < opt && opt < llc, "{err}");
    assert!(err[fe..opt].contains("opt\""), "{err}");
    assert!(!err.contains("-discard-value-names"), "{err}");
    for (flag, level) in [("-O1", "O1"), ("-Og", "O1"), ("-O2", "O2"), ("-Os", "Os"), ("-Oz", "Oz")]
    {
        let o = kiln_in(&d, &["-###", "-S", "-emit-llvm", flag, "add.cpp"]);
        let err = text(&o.stderr);
        let opt = err.find(&format!("\"-passes=default<{level}>\"")).expect(&err);
        let names = err.find("\"-discard-value-names\" \"-S\" \"-o\" \"-\" \"-\"\n").expect(&err);
        assert!(opt < names, "{err}");
    }
    for args in [
        &["-###", "-c", "add.cpp"][..],
        &["-###", "-c", "-O2", "-Xclang", "-disable-llvm-passes", "add.cpp"],
    ] {
        let o = kiln_in(&d, args);
        assert!(!text(&o.stderr).contains("-passes="), "{}", text(&o.stderr));
    }
}

/// WP-783, spec/driver.md "IR optimization": `-S -emit-llvm` writes the
/// IR of the configured backend, so that LLVM's `opt` and `llc` read it
/// back. At the default `-fclang-compat` with an LLVM 19 backend the
/// lifetime markers keep their size operand (LLVM 22's sizeless
/// `(ptr captures(none))` declaration was a parse error for `opt-19`);
/// with an LLVM 22 backend they are LLVM 22's.
#[test]
fn emit_llvm_text_reads_back_with_backend() {
    let src = "struct S { int a[4]; };\nvoid use(S *);\nvoid f() { S s; use(&s); }\n";
    let d = scratch("ll-backend");
    std::fs::write(d.join("lt.cpp"), src).unwrap();
    for (major, sized) in [(19, true), (22, false)] {
        let dir = PathBuf::from(format!("/usr/lib/llvm-{major}"));
        if !dir.join("bin").join("opt").exists() {
            eprintln!("skipping LLVM {major}: {} not found", dir.display());
            continue;
        }
        let out = format!("lt{major}.ll");
        let o = Command::new(kilnpp())
            .args(["-S", "-emit-llvm", "-O2", "lt.cpp", "-o", &out])
            .env("KILN_LLVM_DIR", &dir)
            .current_dir(&d)
            .output()
            .unwrap();
        assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
        let ll = std::fs::read_to_string(d.join(&out)).unwrap();
        let start = if sized {
            "@llvm.lifetime.start.p0(i64 16, ptr"
        } else {
            "@llvm.lifetime.start.p0(ptr"
        };
        assert!(ll.contains(start), "{ll}");
        for tool in ["opt", "llc"] {
            let o = Command::new(dir.join("bin").join(tool))
                .args([&out, "-o", "/dev/null"])
                .current_dir(&d)
                .output()
                .unwrap();
            assert_eq!(o.status.code(), Some(0), "{tool}-{major}: {}", text(&o.stderr));
        }
    }
}

#[test]
fn verbose_prints_commands() {
    if !tools() {
        return;
    }
    let d = setup("v");
    let o = kiln_in(&d, &["-v", "-c", "add.cpp"]);
    assert_eq!(o.status.code(), Some(0));
    let err = text(&o.stderr);
    assert!(err.contains(" -O0 -relocation-model=pic -filetype=obj -o add.o -\n"), "{err}");
    // Linking, `-v` reaches clang++ too: its own banner and the linker's
    // command line follow (spec/driver.md "Link"; CMake reads the implicit
    // link directories from them, WP-667).
    let o = kiln_in(&d, &["-v", "main.cpp", "add.cpp", "-o", "v.out"]);
    assert_eq!(o.status.code(), Some(0));
    let err = text(&o.stderr);
    let link = err.lines().find(|l| l.contains("clang++\" ")).unwrap_or_default();
    assert!(link.contains("clang++\" -v "), "{err}");
    assert!(err.contains("clang version "), "{err}");
    // The linker's own line (clang's `-v` output), not clang++'s.
    assert!(err.lines().any(|l| l.contains(" -o v.out ") && !l.contains("clang++")), "{err}");
}

#[test]
fn missing_tools() {
    let d = setup("missing");
    let o = Command::new(kilnpp())
        .args(["-c", "add.cpp"])
        .current_dir(&d)
        .env("KILN_LLVM_DIR", d.join("no-llvm"))
        .env("PATH", d.join("no-bin"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        text(&o.stderr),
        "kiln++: error: unable to execute command: Executable \"llc-19\" doesn't exist!\n"
    );
    assert!(!d.join("add.o").exists());
}

/// Functions print in clang's order (`spec/codegen.md` "Function order"):
/// creation order, a replacing base-object constructor taking the place of
/// the complete-object declaration it replaces, deferred inline and
/// template definitions after their first use. Two compilations print the
/// same text, and the order matches clang 19 when it is installed.
#[test]
fn function_order_is_stable() {
    const SRC: &str = "struct S { S(); ~S(); int v = 1; };\n\
                       template <class T> T twice(T t) { return t + t; }\n\
                       int use() { S s; return twice(s.v) + twice(2L); }\n\
                       inline S::S() {}\n\
                       S::~S() {}\n\
                       static int helper() { return 3; }\n\
                       int later() { return helper() + twice(1u); }\n";
    const ORDER: [&str; 9] = [
        "_Z3usev",
        "_ZN1SC2Ev",
        "_Z5twiceIiET_S0_",
        "__gxx_personality_v0",
        "_Z5twiceIlET_S0_",
        "_ZN1SD2Ev",
        "_Z5laterv",
        "_ZL6helperv",
        "_Z5twiceIjET_S0_",
    ];
    fn order(ll: &str) -> Vec<String> {
        ll.lines()
            .filter(|l| l.starts_with("define ") || l.starts_with("declare "))
            .filter_map(|l| l.split('@').nth(1)?.split('(').next().map(str::to_owned))
            .collect()
    }
    let d = scratch("order");
    std::fs::write(d.join("order.cpp"), SRC).unwrap();
    let args = ["-S", "-emit-llvm", "-o", "-", "--target=aarch64-unknown-linux-gnu", "order.cpp"];
    let a = kiln_in(&d, &args);
    assert_eq!((a.status.code(), text(&a.stderr).as_str()), (Some(0), ""));
    let b = kiln_in(&d, &args);
    assert_eq!(text(&a.stdout), text(&b.stdout), "two compilations differ");
    assert_eq!(order(&text(&a.stdout)), ORDER);
    if have("clang++-19") {
        let c = Command::new("clang++-19")
            .args([
                "-std=c++26",
                "-S",
                "-emit-llvm",
                "-o",
                "-",
                "--target=aarch64-unknown-linux-gnu",
            ])
            .arg("order.cpp")
            .current_dir(&d)
            .output()
            .unwrap();
        assert_eq!(c.status.code(), Some(0), "{}", text(&c.stderr));
        assert_eq!(order(&text(&c.stdout)), ORDER, "clang's order changed");
    }
}

/// A generator whose lifetime ends inside its caller: at `-O2` the
/// default pipeline's coroutine passes (`CoroEarly`, `CoroSplit`, `CoroElide`,
/// `CoroCleanup`, spec/driver.md "IR optimization") inline the ramp, elide
/// the heap frame ([dcl.fct.def.coroutine]/9 lets the allocation be
/// omitted) and lower every `llvm.coro.*` call, as clang 19's `-O2` does;
/// at `-O0` the ramp still calls `operator new`.
const ELIDE: &str = r"namespace std {
template <class R, class... A> struct coroutine_traits { using promise_type = typename R::promise_type; };
template <class P = void> struct coroutine_handle;
template <> struct coroutine_handle<void> {
  void *p = nullptr;
  static coroutine_handle from_address(void *a) noexcept { coroutine_handle h; h.p = a; return h; }
  void *address() const noexcept { return p; }
};
template <class P> struct coroutine_handle {
  void *p = nullptr;
  static coroutine_handle from_address(void *a) noexcept { coroutine_handle h; h.p = a; return h; }
  static coroutine_handle from_promise(P &pr) noexcept {
    return from_address(__builtin_coro_promise(&pr, alignof(P), true));
  }
  operator coroutine_handle<>() const noexcept { return coroutine_handle<>::from_address(p); }
  void resume() const { __builtin_coro_resume(p); }
  void destroy() const { __builtin_coro_destroy(p); }
  bool done() const { return __builtin_coro_done(p); }
  P &promise() const { return *static_cast<P *>(__builtin_coro_promise(p, alignof(P), false)); }
};
struct suspend_always {
  bool await_ready() const noexcept { return false; }
  void await_suspend(coroutine_handle<>) const noexcept {}
  void await_resume() const noexcept {}
};
} // namespace std
struct gen {
  struct promise_type {
    int v = 0;
    gen get_return_object() { return gen{std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_always initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    std::suspend_always yield_value(int x) noexcept { v = x; return {}; }
    void return_void() noexcept {}
    void unhandled_exception() noexcept {}
  };
  std::coroutine_handle<promise_type> h;
  explicit gen(std::coroutine_handle<promise_type> h) : h(h) {}
  gen(const gen &) = delete;
  ~gen() { h.destroy(); }
};
static gen count(int n) {
  for (int i = 0; i < n; ++i) co_yield i;
}
int sum(int n) {
  int s = 0;
  gen g = count(n);
  for (g.h.resume(); !g.h.done(); g.h.resume()) s += g.h.promise().v;
  return s;
}
";

#[test]
fn coro_elide_at_o2() {
    if !tools() || !have("opt-19") {
        return;
    }
    let d = scratch("coro-elide");
    std::fs::write(d.join("elide.cpp"), ELIDE).unwrap();
    let elided = |ll: &str| {
        let sum = ll.split("\ndefine ").find(|f| f.contains("@_Z3sumi(")).unwrap_or_default();
        !ll.contains("@llvm.coro.") && !sum.contains("@_Znwm") && !ll.contains("@_ZL5counti")
    };
    let o = kiln_in(&d, &["-std=c++20", "-O2", "-S", "-emit-llvm", "-o", "-", "elide.cpp"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    assert!(elided(&text(&o.stdout)), "{}", text(&o.stdout));
    let o = kiln_in(&d, &["-std=c++20", "-S", "-emit-llvm", "-o", "-", "elide.cpp"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    assert!(text(&o.stdout).contains("call noalias noundef nonnull ptr @_Znwm("));
    let c = Command::new("clang++-19")
        .args(["-std=c++20", "-O2", "-S", "-emit-llvm", "-o", "-", "elide.cpp"])
        .current_dir(&d)
        .output()
        .unwrap();
    assert_eq!(c.status.code(), Some(0), "{}", text(&c.stderr));
    assert!(elided(&text(&c.stdout)), "clang no longer elides");
}

/// The module text without comments, `; preds` annotations, the
/// producer lines and trailing blanks.
fn ir_body(ll: &str) -> String {
    let mut out = String::new();
    for l in ll.lines() {
        let l = l.split(" ; preds = ").next().unwrap_or(l).trim_end();
        if l.starts_with(';')
            || l.starts_with("source_filename")
            || l.starts_with("!llvm.ident")
            || l.contains("!{!\"kiln")
            || l.contains("clang version")
        {
            continue;
        }
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// [expr.await]/5.1, [dcl.fct.def.coroutine]: before LLVM passes at -O1
/// (`-Xclang -disable-llvm-passes`) the `await_suspend` wrappers match
/// clang 19's exactly: `alwaysinline mustprogress` (x86: plus
/// `min-legal-vector-width`) and "any pointer" TBAA on the prolog stores
/// of their `void *` awaiter and frame parameters (spec/codegen.md
/// "Coroutines"). After the -O1 pipeline the wrappers are inlined away,
/// so `ir_vs_clang` cannot see them (`coro_await_suspend_o1`).
#[test]
fn coro_wrappers_before_passes_match_clang19() {
    if !tools() {
        return;
    }
    let src =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/codegen/coro_await_suspend_o1.cpp");
    let src = src.to_str().unwrap();
    // Pin the triple: clang's default spells the vendor `pc` on some x86
    // distributions where kiln prints `unknown`. clang 19 has no C++26
    // erroneous values, so kiln's fill (of the promise, spec/codegen.md
    // §7y) is pinned off, as in `ir_vs_clang`.
    let mut args = vec![
        "-std=c++26",
        "-O1",
        "-Xclang",
        "-disable-llvm-passes",
        "-S",
        "-emit-llvm",
        "-ftrivial-auto-var-init=uninitialized",
    ];
    if cfg!(target_arch = "aarch64") {
        args.extend(["--target=aarch64-unknown-linux-gnu", "-moutline-atomics"]);
    } else {
        args.push("--target=x86_64-unknown-linux-gnu");
    }
    args.extend(["-o", "-", src]);
    let d = scratch("coro-wrappers");
    let mut kargs = vec!["-fclang-compat=19"];
    kargs.extend(&args);
    let k = kiln_in(&d, &kargs);
    assert_eq!(k.status.code(), Some(0), "{}", text(&k.stderr));
    let c = Command::new("clang++-19").args(&args).current_dir(&d).output().unwrap();
    assert_eq!(c.status.code(), Some(0), "{}", text(&c.stderr));
    let (k, c) = (ir_body(&text(&k.stdout)), ir_body(&text(&c.stdout)));
    assert!(k.contains("__await_suspend_wrapper__init"), "{k}");
    assert_eq!(k, c);
}

/// The diagnostics of GNU `alias` / `ifunc` definitions at code
/// generation (spec/codegen.md "Aliases and ifuncs", WP-763; clang
/// `EmitAliasDefinition`, `emitIFuncDefinition`, `checkAliases`): a
/// self-alias is a cycle; an alias chain must end at a definition (the
/// target is a mangled name: a note per declaration of that identifier
/// names its symbol, with a fix-it); an ifunc's resolver must return a
/// pointer; an ifunc over a definition is a duplicate definition; an
/// alias in another section than its aliasee, or of a weak alias, is
/// `-Wignored-attributes`. The checks after emission are skipped once an
/// error was reported. Each case's message lines (sources and carets
/// dropped) are pinned; the whole text is compared with clang++-19's.
#[test]
fn alias_ifunc_diagnostics() {
    let d = scratch("alias-diags");
    let cases: [(&str, &str, i32); 7] = [
        (
            "int undef(int);\nint a1(int) __attribute__((alias(\"undef\")));\n",
            "t.cpp:2:28: error: alias must point to a defined variable or function\n\
             t.cpp:2:28: note: the function or variable specified in an alias must refer to its mangled name\n\
             1 error generated.\n",
            1,
        ),
        (
            "extern \"C\" int c1(int) __attribute__((alias(\"c2\")));\n\
             extern \"C\" int c2(int) __attribute__((alias(\"c1\")));\n\
             int self(int) __attribute__((alias(\"_Z4selfi\")));\n",
            "t.cpp:3:30: error: alias definition is part of a cycle\n1 error generated.\n",
            1,
        ),
        (
            "extern \"C\" int c1(int) __attribute__((alias(\"c2\")));\n\
             extern \"C\" int c2(int) __attribute__((alias(\"c1\")));\n",
            "t.cpp:1:39: error: alias definition is part of a cycle\n\
             t.cpp:2:39: error: alias definition is part of a cycle\n2 errors generated.\n",
            1,
        ),
        (
            "extern \"C\" int r() { return 0; }\nextern \"C\" int f(int) __attribute__((ifunc(\"r\")));\n",
            "t.cpp:2:38: error: ifunc resolver function must return a pointer\n1 error generated.\n",
            1,
        ),
        (
            "int foo(int x) { return x; }\nnamespace n { int foo(int x) { return x; } }\n\
             extern \"C\" int a(int) __attribute__((alias( \"fo\" \"o\" )));\n\
             extern \"C\" int i(int) __attribute__((ifunc(\"foo\")));\n\
             int v;\nextern int va __attribute__((alias(\"missing\")));\n",
            "t.cpp:3:38: error: alias must point to a defined variable or function\n\
             t.cpp:3:38: note: the function or variable specified in an alias must refer to its mangled name\n\
             t.cpp:3:38: note: function by that name is mangled as \"_Z3fooi\"\n\
             t.cpp:3:38: note: function by that name is mangled as \"_ZN1n3fooEi\"\n\
             t.cpp:4:38: error: ifunc must point to a defined function\n\
             t.cpp:4:38: note: the function specified in an ifunc must refer to its mangled name\n\
             t.cpp:4:38: note: function by that name is mangled as \"_Z3fooi\"\n\
             t.cpp:4:38: note: function by that name is mangled as \"_ZN1n3fooEi\"\n\
             t.cpp:6:30: error: alias must point to a defined variable or function\n\
             t.cpp:6:30: note: the function or variable specified in an alias must refer to its mangled name\n\
             3 errors generated.\n",
            1,
        ),
        (
            "extern \"C\" int impl(int x) { return x; }\n\
             int g(int) asm(\"impl\") __attribute__((ifunc(\"r\")));\n\
             extern \"C\" void *r() { return 0; }\n",
            "t.cpp:2:5: error: definition with same mangled name 'impl' as another definition\n\
             t.cpp:1:16: note: previous definition is here\n1 error generated.\n",
            1,
        ),
        (
            "extern \"C\" int impl(int x) { return x; }\n\
             extern \"C\" __attribute__((weak)) int w(int) __attribute__((alias(\"impl\")));\n\
             extern \"C\" int a(int) __attribute__((alias(\"w\")));\n\
             extern \"C\" __attribute__((section(\"s1\"))) int sa(int) __attribute__((alias(\"impl\")));\n\
             extern \"C\" __attribute__((weak)) int wi(int) __attribute__((alias(\"r\")));\n\
             extern \"C\" int i2(int) __attribute__((ifunc(\"wi\")));\n\
             extern \"C\" __attribute__((section(\"s3\"))) int i3(int) __attribute__((ifunc(\"r\")));\n\
             extern \"C\" void *r() { return 0; }\n\
             int gv = 1;\nextern int gs __attribute__((section(\"s2\"), alias(\"gv\")));\n",
            "t.cpp:3:38: warning: alias will always resolve to impl even if weak definition of w is overridden [-Wignored-attributes]\n\
             t.cpp:4:27: warning: alias will not be in section 's1' but in the same section as the aliasee [-Wignored-attributes]\n\
             t.cpp:6:39: warning: ifunc will always resolve to r even if weak definition of wi is overridden [-Wignored-attributes]\n\
             t.cpp:7:27: warning: ifunc will not be in section 's3' but in the same section as the resolver [-Wignored-attributes]\n\
             t.cpp:10:30: warning: alias will not be in section 's2' but in the same section as the aliasee [-Wignored-attributes]\n\
             5 warnings generated.\n",
            0,
        ),
    ];
    let messages = |s: &str| -> String {
        s.lines()
            .filter(|l| l.starts_with("t.cpp:") || l.ends_with("generated."))
            .fold(String::new(), |acc, l| acc + l + "\n")
    };
    for (src, want, code) in cases {
        std::fs::write(d.join("t.cpp"), src).unwrap();
        let _ = std::fs::remove_file(d.join("t.ll"));
        let o = kiln_in(&d, &["-S", "-emit-llvm", "t.cpp"]);
        let got = text(&o.stderr);
        assert_eq!(messages(&got), want, "{src}");
        assert_eq!(o.status.code(), Some(code), "{src}");
        assert_eq!(d.join("t.ll").exists(), code == 0, "{src}");
        if have("clang++-19") {
            let c = Command::new("clang++-19")
                .args(["-S", "-emit-llvm", "-o", "c.ll", "t.cpp"])
                .current_dir(&d)
                .output()
                .unwrap();
            assert_eq!(text(&c.stderr), got, "clang++-19: {src}");
        }
    }
}

/// clang `err_duplicate_mangled_name` (spec/codegen.md "Duplicate mangled
/// names", WP-492): a definition whose symbol (an asm label, or a name
/// equal to another entity's mangled name) is already defined by another
/// entity is an error at code generation, once, with the representative
/// definition as the note; nothing is written. Every structor variant
/// taking one label is a single entity and is no error.
#[test]
fn duplicate_mangled_name() {
    let d = scratch("dupname");
    let cases = [
        (
            "void f() asm(\"x\");\nvoid g() asm(\"x\");\nvoid f() {}\nvoid g() {}\nvoid h() asm(\"x\");\nvoid h() {}\n",
            "t.cpp:4:6: error: definition with same mangled name 'x' as another definition\n    \
             4 | void g() {}\n      |      ^\nt.cpp:3:6: note: previous definition is here\n    \
             3 | void f() {}\n      |      ^\n1 error generated.\n",
        ),
        (
            "extern \"C\" void _Z1fv() {}\nvoid f() {}\n",
            "t.cpp:2:6: error: definition with same mangled name '_Z1fv' as another definition\n    \
             2 | void f() {}\n      |      ^\nt.cpp:1:17: note: previous definition is here\n    \
             1 | extern \"C\" void _Z1fv() {}\n      |                 ^\n1 error generated.\n",
        ),
        (
            "void af() asm(\"actor\");\nvoid af() {}\nstruct A { A() asm(\"actor\"); };\nA::A() {}\n",
            "t.cpp:4:4: error: definition with same mangled name 'actor' as another definition\n    \
             4 | A::A() {}\n      |    ^\nt.cpp:2:6: note: previous definition is here\n    \
             2 | void af() {}\n      |      ^\n1 error generated.\n",
        ),
        ("struct S { S() asm(\"s\"); ~S() asm(\"t\"); };\nS::S() {}\nS::~S() {}\n", ""),
    ];
    for (src, want) in cases {
        std::fs::write(d.join("t.cpp"), src).unwrap();
        let _ = std::fs::remove_file(d.join("t.ll"));
        let o = kiln_in(&d, &["-S", "-emit-llvm", "t.cpp"]);
        assert_eq!(text(&o.stderr), want, "{src}");
        assert_eq!(o.status.code(), Some(i32::from(!want.is_empty())), "{src}");
        assert_eq!(d.join("t.ll").exists(), want.is_empty(), "{src}");
        if have("clang++-19") {
            let c = Command::new("clang++-19")
                .args(["-S", "-emit-llvm", "-o", "c.ll", "t.cpp"])
                .current_dir(&d)
                .output()
                .unwrap();
            assert_eq!(text(&c.stderr), want, "clang++-19: {src}");
        }
    }
}

/// `-fPIC` and `-fvisibility=` through the whole driver (spec/codegen.md
/// "Symbol visibility", "`dso_local`"; spec/driver.md "Code generation"):
/// in a shared-library build a default-visibility function may be
/// interposed, so neither its definition nor its address is `dso_local`;
/// a hidden one is. `-fvisibility=hidden` hides the definitions it does
/// not export, and `-c` builds the object as position-independent code.
#[test]
fn pic_visibility_and_objects() {
    let d = scratch("pic");
    let src = "void f() {}\n__attribute__((visibility(\"hidden\"))) void h() {}\n\
               __attribute__((visibility(\"default\"))) void e() {}\nvoid (*g)() = f;\n";
    std::fs::write(d.join("v.cpp"), src).unwrap();
    let ll = |extra: &[&str]| {
        let mut a = vec!["-S", "-emit-llvm", "-o", "-", "-fPIC"];
        a.extend(extra);
        a.push("v.cpp");
        let o = kiln_in(&d, &a);
        assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""), "{extra:?}");
        text(&o.stdout)
    };
    let pic = ll(&[]);
    for want in [
        "define void @_Z1fv()",
        "define hidden void @_Z1hv()",
        "define void @_Z1ev()",
        "@g = global ptr @_Z1fv,",
        "!{i32 8, !\"PIC Level\", i32 2}",
    ] {
        assert!(pic.contains(want), "{want}\n{pic}");
    }
    assert!(!pic.contains("dso_local void @_Z1fv"), "{pic}");
    let hidden = ll(&["-fvisibility=hidden"]);
    for want in [
        "define hidden void @_Z1fv()",
        "define hidden void @_Z1hv()",
        "define void @_Z1ev()",
        "@g = hidden global ptr @_Z1fv,",
    ] {
        assert!(hidden.contains(want), "{want}\n{hidden}");
    }
    if !tools() {
        return;
    }
    for extra in [&[][..], &["-fvisibility=hidden"]] {
        let _ = std::fs::remove_file(d.join("v.o"));
        let mut a = vec!["-c", "-fPIC"];
        a.extend(extra);
        a.push("v.cpp");
        let o = kiln_in(&d, &a);
        assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""), "{extra:?}");
        let b = std::fs::read(d.join("v.o")).unwrap();
        assert_eq!(&b[..4], b"\x7fELF", "{extra:?}");
    }
}

/// A NEON builtin whose target feature the calling function lacks is an
/// error only when the call is emitted (spec/landed/codegen/WP-558.md "Target
/// features"; tests/conformance/target.neon/bfdot-without-feature.cpp
/// pins that `-fsyntax-only` accepts it): calling the `always_inline`
/// `target("bf16,neon")` header wrapper from a plain function names the
/// first missing feature, calling the builtin directly names the
/// builtin's whole feature list. Exact clang 19 stderr, `AArch64` only.
#[test]
fn neon_bf16_call_without_feature() {
    if !cfg!(target_arch = "aarch64") {
        return;
    }
    let d = scratch("neonbf16");
    let cases = [
        (
            "#include <arm_neon.h>\n\
             float32x4_t f(float32x4_t r, bfloat16x8_t a, bfloat16x8_t b) { return vbfdotq_f32(r, a, b); }\n",
            "t.cpp:2:71: error: always_inline function 'vbfdotq_f32' requires target feature 'bf16', \
             but would be inlined into function 'f' that is compiled without support for 'bf16'\n    \
             2 | float32x4_t f(float32x4_t r, bfloat16x8_t a, bfloat16x8_t b) { return vbfdotq_f32(r, a, b); }\n      \
             |                                                                       ^\n1 error generated.\n",
        ),
        (
            "#include <arm_neon.h>\n\
             int8x16_t g(int8x16_t r, int8x16_t a, int8x16_t b) { return (int8x16_t)__builtin_neon_vbfdotq_f32(r, a, b, 41); }\n",
            "t.cpp:2:72: error: '__builtin_neon_vbfdotq_f32' needs target feature bf16,neon\n    \
             2 | int8x16_t g(int8x16_t r, int8x16_t a, int8x16_t b) { return (int8x16_t)__builtin_neon_vbfdotq_f32(r, a, b, 41); }\n      \
             |                                                                        ^\n1 error generated.\n",
        ),
    ];
    for (src, want) in cases {
        std::fs::write(d.join("t.cpp"), src).unwrap();
        let o = kiln_in(&d, &["-fclang-compat=19", "-S", "-emit-llvm", "-o", "k.ll", "t.cpp"]);
        assert_eq!(text(&o.stderr), want, "{src}");
        assert_eq!(o.status.code(), Some(1), "{src}");
        if have("clang++-19") {
            let c = Command::new("clang++-19")
                .args(["-S", "-emit-llvm", "-o", "c.ll", "t.cpp"])
                .current_dir(&d)
                .output()
                .unwrap();
            assert_eq!(text(&c.stderr), want, "clang++-19: {src}");
        }
    }
}

/// spec/landed/codegen/WP-630.md "Target features" (WP-630): an x86
/// builtin, or the `always_inline` `target("...")` header wrapper of one,
/// called from a function without the feature it needs is a codegen error
/// (`-fsyntax-only` accepts it): the wrapper names the first missing
/// feature, the builtin its whole feature list, and an SSE compare's
/// predicate above 7 needs `avx`. Exact clang 21 stderr, x86-64 only.
#[test]
fn x86_call_without_feature() {
    if !cfg!(target_arch = "x86_64") {
        return;
    }
    let d = scratch("x86feat");
    let cases = [
        (
            "#include <immintrin.h>\n\
             unsigned f(unsigned c, unsigned char v) { return _mm_crc32_u8(c, v); }\n",
            "t.cpp:2:50: error: always_inline function '_mm_crc32_u8' requires target feature \
             'crc32', but would be inlined into function 'f' that is compiled without support \
             for 'crc32'\n    \
             2 | unsigned f(unsigned c, unsigned char v) { return _mm_crc32_u8(c, v); }\n      \
             |                                                  ^\n1 error generated.\n",
        ),
        (
            "unsigned g(unsigned c, unsigned v) { return __builtin_ia32_crc32si(c, v); }\n",
            "t.cpp:1:45: error: '__builtin_ia32_crc32si' needs target feature crc32\n    \
             1 | unsigned g(unsigned c, unsigned v) { return __builtin_ia32_crc32si(c, v); }\n      \
             |                                             ^\n1 error generated.\n",
        ),
        (
            "typedef float v4f __attribute__((vector_size(16)));\n\
             v4f h(v4f a, v4f b) { return __builtin_ia32_cmpps(a, b, 8); }\n",
            "t.cpp:2:30: error: '__builtin_ia32_cmpps' needs target feature avx\n    \
             2 | v4f h(v4f a, v4f b) { return __builtin_ia32_cmpps(a, b, 8); }\n      \
             |                              ^\n1 error generated.\n",
        ),
    ];
    for (src, want) in cases {
        std::fs::write(d.join("t.cpp"), src).unwrap();
        let o = kiln_in(&d, &["-S", "-emit-llvm", "-o", "k.ll", "t.cpp"]);
        assert_eq!(text(&o.stderr), want, "{src}");
        assert_eq!(o.status.code(), Some(1), "{src}");
        let o = kiln_in(&d, &["-fsyntax-only", "t.cpp"]);
        assert_eq!(text(&o.stderr), "", "{src}");
    }
}

/// spec/driver.md "Codegen flags" (WP-605): every honoured or ignored
/// GCC/clang codegen flag compiles `int main(){}` to an object; every
/// rejected one is the driver error "'<flag>' is not supported by kiln".
#[test]
fn codegen_flags_classes() {
    if !tools() {
        return;
    }
    let d = scratch("cgflags");
    std::fs::write(d.join("m.cpp"), "int main(){}\n").unwrap();
    let accepted: &[&[&str]] = &[
        // Class 1: honoured.
        &["-fstack-protector"],
        &["-fstack-protector-strong"],
        &["-fstack-protector-all"],
        &["-fno-stack-protector"],
        &["-funwind-tables"],
        &["-fno-unwind-tables"],
        &["-fasynchronous-unwind-tables"],
        &["-fno-asynchronous-unwind-tables"],
        &["-fno-plt"],
        &["-fplt"],
        &["-ffunction-sections"],
        &["-fno-function-sections"],
        &["-fdata-sections"],
        &["-fno-data-sections"],
        &["-march=native"],
        &["-mcpu=native"],
        &["-mtune=native"],
        &["-mtune=generic"],
        &["-fwrapv"],
        &["-fno-wrapv"],
        &["-fwrapv-pointer"],
        &["-fno-wrapv-pointer"],
        &["-fno-strict-overflow"],
        &["-fstrict-overflow"],
        &["-fno-delete-null-pointer-checks"],
        &["-fdelete-null-pointer-checks"],
        &["-Wp,-D_FORTIFY_SOURCE=2"],
        &["-Xpreprocessor", "-DX=1"],
        // WP-813: `-fcf-protection` other than `none` is x86-only.
        &["-fcf-protection"],
        &["-fcf-protection=full"],
        &["-fcf-protection=none"],
        &["-fpatchable-function-entry=2,1"],
        &["-fzero-call-used-regs=used-gpr"],
        &["-mspeculative-load-hardening"],
        &["-fsplit-stack"],
        &["-finstrument-functions"],
        &["-fxray-instrument"],
        // Class 2: accepted and ignored.
        &["-fstack-clash-protection"],
        &["-fno-stack-clash-protection"],
        &["-fno-common"],
        &["-fcommon"],
        &["-fexcess-precision=standard"],
        &["-fno-math-errno"],
        &["-fmath-errno"],
        &["-fstack-usage"],
        &["-gz"],
        &["-gz=zlib"],
        &["-fno-trapv"],
        &["-fno-short-enums"],
        &["-ffp-model=precise"],
        // The common hardening set together, at -O2.
        &[
            "-O2",
            "-fstack-protector-strong",
            "-fstack-clash-protection",
            "-fcf-protection",
            "-Wp,-D_FORTIFY_SOURCE=2",
            "-fasynchronous-unwind-tables",
            "-fno-plt",
            "-ffunction-sections",
            "-fdata-sections",
        ],
    ];
    for flags in accepted {
        // `-mcpu=` is AArch64-only: on x86-64 it is "unsupported argument",
        // as clang ("Target features"), so the host target decides.
        if flags.contains(&"-mcpu=native") && !cfg!(target_arch = "aarch64") {
            continue;
        }
        let mut flags = flags.to_vec();
        if cfg!(target_arch = "aarch64") {
            flags.retain(|f| !matches!(*f, "-fcf-protection" | "-fcf-protection=full"));
            if flags.is_empty() {
                continue;
            }
        }
        let mut args = flags.clone();
        args.extend(["-c", "m.cpp", "-o", "m.o"]);
        let o = kiln_in(&d, &args);
        assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""), "{flags:?}");
        assert_eq!(&std::fs::read(d.join("m.o")).unwrap()[..4], b"\x7fELF", "{flags:?}");
    }
}

/// spec/driver.md "Codegen flags" (WP-605): the rejected class is the
/// driver error "'<flag>' is not supported by kiln", and values outside
/// clang's sets are clang's errors; no job runs.
#[test]
fn codegen_flags_rejected() {
    let d = scratch("cgflags-rej");
    std::fs::write(d.join("m.cpp"), "int main(){}\n").unwrap();
    let rejected = [
        "-ftrapv",
        "-fshort-enums",
        "-fsanitize=cfi",
        "-fsanitize-coverage=trace-pc",
        "-fcoverage-mapping",
        "-fprofile-arcs",
        "-ftest-coverage",
        "--coverage",
        "-fprofile-instr-generate",
        "-fprofile-instr-generate=a.profraw",
        "-fprofile-generate",
        "-fprofile-use=a.profdata",
        "-fprofile-instr-use=a.profdata",
    ];
    for f in rejected {
        let o = kiln_in(&d, &[f, "-c", "m.cpp", "-o", "r.o"]);
        assert_eq!(
            (o.status.code(), text(&o.stderr)),
            (Some(1), format!("kiln++: error: '{f}' is not supported by kiln\n")),
            "{f}"
        );
        assert!(!d.join("r.o").exists(), "{f}");
    }
    // Values outside clang's sets are clang's errors (`-fcf-protection=`'s
    // is `-cc1`'s, with no program name; WP-813).
    let o = kiln_in(&d, &["-fcf-protection=bogus", "-c", "m.cpp", "-o", "r.o"]);
    assert_eq!(
        (o.status.code(), text(&o.stderr).as_str()),
        (Some(1), "error: invalid value 'bogus' in '-fcf-protection=bogus'\n")
    );
    for (f, want) in [
        ("-gz=lzma", "unsupported argument 'lzma' to option '-gz='"),
        ("-fexcess-precision=none", "unsupported argument 'none' to option '-fexcess-precision='"),
        ("-mtune=", "unsupported argument '' to option '-mtune='"),
    ] {
        let o = kiln_in(&d, &[f, "-c", "m.cpp", "-o", "r.o"]);
        assert_eq!(
            (o.status.code(), text(&o.stderr)),
            (Some(1), format!("kiln++: error: {want}\n"))
        );
    }
}

/// spec/driver.md "Floating-point flags" (WP-806): flags whose final
/// effect needs the floating-point environment ([cfenv.syn]) or promoted
/// complex division are driver errors, never ignored; a later flag that
/// restores the default clears the error.
#[test]
fn fp_environment_flags_rejected() {
    let d = scratch("fpenv-rej");
    std::fs::write(d.join("m.cpp"), "int main(){}\n").unwrap();
    for (flags, want) in [
        (&["-frounding-math"][..], "'-frounding-math' is not supported by kiln"),
        (
            &["-ffp-exception-behavior=strict"],
            "'-ffp-exception-behavior=strict' is not supported by kiln",
        ),
        (
            &["-ffp-exception-behavior=maytrap"],
            "'-ffp-exception-behavior=maytrap' is not supported by kiln",
        ),
        (&["-ffp-model=strict"], "'-ffp-model=strict' is not supported by kiln"),
        // `precise` keeps `strict`'s rounding and exception behavior.
        (
            &["-ffp-model=strict", "-ffp-model=precise"],
            "'-ffp-model=strict' is not supported by kiln",
        ),
        (&["-ffp-eval-method=double"], "'-ffp-eval-method=double' is not supported by kiln"),
        (
            &["-ffp-model=fast"],
            "'-ffp-model=fast (promoted complex arithmetic; add -fcomplex-arithmetic=basic)' is not supported by kiln",
        ),
        (
            &["-fclang-compat=19", "-ffp-model=aggressive"],
            "unsupported argument 'aggressive' to option '-ffp-model='",
        ),
    ] {
        let mut args = flags.to_vec();
        args.extend(["-c", "m.cpp", "-o", "r.o"]);
        let o = kiln_in(&d, &args);
        assert_eq!(
            (o.status.code(), text(&o.stderr)),
            (Some(1), format!("kiln++: error: {want}\n")),
            "{flags:?}"
        );
    }
    for flags in [
        &["-frounding-math", "-fno-rounding-math"][..],
        &["-ffp-model=strict", "-fno-rounding-math", "-ffp-exception-behavior=ignore"],
        &["-ffp-model=fast", "-fcx-limited-range"],
        &["-fclang-compat=19", "-ffp-model=fast"],
        &["-ffp-model=aggressive"],
    ] {
        let mut args = flags.to_vec();
        args.extend(["-c", "m.cpp", "-o", "a.o"]);
        let o = kiln_in(&d, &args);
        assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""), "{flags:?}");
    }
}

/// `-ffunction-sections` / `-fdata-sections` and `-march=native` reach
/// `llc` (spec/driver.md "Codegen flags"); `-Wp,-MD,<file>` is `-MD -MF
/// <file>`.
#[test]
fn codegen_flags_backend() {
    let d = setup("cgbackend");
    let o = kiln_in(
        &d,
        &["-###", "-c", "-ffunction-sections", "-fdata-sections", "-march=native", "add.cpp"],
    );
    let err = text(&o.stderr);
    assert_eq!(o.status.code(), Some(0), "{err}");
    assert!(
        err.contains(
            "\"-filetype=obj\" \"-function-sections\" \"-data-sections\" \"-mcpu=native\" \"-o\""
        ),
        "{err}"
    );
    let o = kiln_in(&d, &["-###", "-c", "-mtune=native", "add.cpp"]);
    assert!(text(&o.stderr).contains("\"-filetype=obj\" \"-o\""), "{}", text(&o.stderr));
    if !tools() {
        return;
    }
    let o = kiln_in(&d, &["-c", "-Wp,-MD,add.dep", "add.cpp"]);
    assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""));
    let dep = std::fs::read_to_string(d.join("add.dep")).unwrap();
    assert!(dep.starts_with("add.o: add.cpp"), "{dep}");
    let o = kiln_in(&d, &["-c", "-ffunction-sections", "add.cpp", "-o", "fs.o"]);
    assert_eq!(o.status.code(), Some(0));
    let obj = std::fs::read(d.join("fs.o")).unwrap();
    let name: &[u8] = b".text._Z3addii\0";
    assert!(obj.windows(name.len()).any(|w| w == name), "no .text._Z3addii section");
}

/// [stmt.expand] (P1306R5) elements of a large expansion statement are
/// emitted as `<fn>.expand.<n>` helpers while optimizing (WP-793,
/// spec/landed/codegen/WP-793.md): a `break`/`return` element returns an
/// `i32` status the parent switches on; `-O0`, `=0` and a small expansion
/// keep the inline emission byte for byte.
#[test]
fn expansion_outlining_shape() {
    const SRC: &str = "void g(int);\n\
        int f(int n) {\n\
          template for (constexpr int k : {1, 2, 3}) {\n\
            for (int i = 0; i < n; ++i) { g(i * k + n); g(i - k); }\n\
            if (n == k) break;\n\
            if (n > 10 * k) return k;\n\
          }\n\
          template for (constexpr int k : {4, 5}) {\n\
            for (int i = 0; i < n; ++i) { g(i * k + n); g(i - k); }\n\
          }\n\
          return 0;\n\
        }\n";
    let d = scratch("expoutline");
    std::fs::write(d.join("e.cpp"), SRC).unwrap();
    let ir = |extra: &[&str], out: &str| {
        let mut a = vec!["-std=c++26", "-S", "-emit-llvm", "-Xclang", "-disable-llvm-passes"];
        a.extend_from_slice(extra);
        a.extend(["e.cpp", "-o", out]);
        let o = kiln_in(&d, &a);
        assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""), "{extra:?}");
        std::fs::read_to_string(d.join(out)).unwrap()
    };
    let on = ir(&["-O2", "-fkiln-outline-expansions=1"], "on.ll");
    for n in 0..3 {
        assert!(on.contains(&format!("define internal i32 @_Z1fi.expand.{n}(")), "{on}");
    }
    for n in 3..5 {
        assert!(on.contains(&format!("define internal void @_Z1fi.expand.{n}(")), "{on}");
    }
    assert!(!on.contains("@_Z1fi.expand.5("), "{on}");
    let parent = on.split("define ").find(|f| f.contains("@_Z1fi(")).unwrap();
    assert_eq!(parent.matches("switch i32").count(), 3, "{parent}");
    assert_eq!(parent.matches("call ").count(), 5, "{parent}");
    let default = ir(&["-O2"], "default.ll");
    let off = ir(&["-O2", "-fkiln-outline-expansions=0"], "off.ll");
    assert!(!default.contains(".expand."), "{default}");
    assert_eq!(default, off);
    let o0 = ir(&["-O0", "-fkiln-outline-expansions=1"], "o0.ll");
    assert!(!o0.contains(".expand."), "{o0}");
}

/// spec/codegen.md "SVE fixed-length vectors" (WP-810): an AAPCS64 pure
/// scalable type (an aggregate of `arm_sve_vector_bits` vectors and
/// predicates) passed or returned by value is a codegen error from
/// `-fclang-compat=22` (clang 20+ passes it in SVE registers, which kiln
/// does not lower yet), at the definition and at the call; clang 19's
/// lowering (`-fclang-compat=19`) and a struct that also holds a NEON
/// vector compile. `-fsyntax-only` accepts all of them.
#[test]
fn sve_pure_scalable_by_value() {
    let d = scratch("svepst");
    let pre = "typedef __SVInt32_t svint32_t; typedef __SVBool_t svbool_t;\n\
               typedef svint32_t fi32 __attribute__((arm_sve_vector_bits(128)));\n\
               typedef svbool_t fb __attribute__((arm_sve_vector_bits(128)));\n\
               typedef __attribute__((neon_vector_type(4))) int n4;\n\
               struct A { fi32 v; };\n\
               struct C { fi32 v; n4 n; };\n\
               struct D { fi32 v[2]; fb p; };\n\
               void c(C x) { c(x); }\n";
    let what = "by value is not supported yet: it is an AAPCS64 pure scalable type (an \
                aggregate of fixed-length SVE types)";
    let cases = [
        ("void a(A) {}\n", format!("t.cpp:9:6: error: passing or returning 'A' {what}")),
        (
            "D d();\nint e() { d(); return 0; }\n",
            format!("t.cpp:10:11: error: passing or returning 'D' {what}"),
        ),
    ];
    let flags =
        ["--target=aarch64-unknown-linux-gnu", "-march=armv8-a+sve", "-msve-vector-bits=128"];
    for (body, want) in cases {
        std::fs::write(d.join("t.cpp"), format!("{pre}{body}")).unwrap();
        let mut args = vec!["-fclang-compat=22", "-S", "-emit-llvm", "-o", "k.ll"];
        args.extend(flags);
        args.push("t.cpp");
        let o = kiln_in(&d, &args);
        assert!(text(&o.stderr).starts_with(&want), "{body}: {}", text(&o.stderr));
        assert_eq!(o.status.code(), Some(1), "{body}");
        args[0] = "-fclang-compat=19";
        let o = kiln_in(&d, &args);
        assert!(o.status.success(), "{body}: {}", text(&o.stderr));
        let mut args = vec!["-fclang-compat=22", "-fsyntax-only"];
        args.extend(flags);
        args.push("t.cpp");
        let o = kiln_in(&d, &args);
        assert!(o.status.success(), "{body}: {}", text(&o.stderr));
    }
}

/// WP-810: an `<arm_sve.h>` intrinsic kiln does not lower, a call without
/// the `sve` feature, an SVE tuple by value at `-fclang-compat=22` and an
/// alias whose declaration does not match its builtin are hard errors.
/// `-ffreestanding` keeps `<stdint.h>` off the host libc, so the test
/// runs on an x86-64 host without `AArch64` headers.
#[test]
fn sve_builtin_alias_errors() {
    let d = scratch("svealias");
    let sve = "#include <arm_sve.h>\n";
    let cases = [
        (
            format!("{sve}svint8x2_t a(svint8_t x) {{ return svcreate2_s8(x, x); }}\n"),
            "-fclang-compat=19",
            "-march=armv8-a+sve",
            "t.cpp:2:47: error: code generation for SVE builtin '__builtin_sve_svcreate2_s8' \
             (called as 'svcreate2_s8') is not supported yet",
        ),
        (
            format!("{sve}uint64_t a() {{ return svcntw(); }}\n"),
            "-fclang-compat=19",
            "-march=armv8-a",
            "t.cpp:2:23: error: 'svcntw' needs target feature sve|sme",
        ),
        (
            format!("{sve}void a(svint8x2_t) {{}}\n"),
            "-fclang-compat=22",
            "-march=armv8-a+sve",
            "t.cpp:2:6: error: passing or returning '__clang_svint8x2_t' by value is not \
             supported yet: it is an SVE tuple type, which clang 20 and later pass as a \
             struct of vectors",
        ),
        (
            "unsigned long f(int) \
             __attribute__((__clang_arm_builtin_alias(__builtin_sve_svcntw)));\n\
             unsigned long a() { return f(1); }\n"
                .to_string(),
            "-fclang-compat=19",
            "-march=armv8-a+sve",
            "t.cpp:2:29: error: the function called does not have the signature of SVE \
             builtin '__builtin_sve_svcntw'",
        ),
    ];
    for (src, compat, march, want) in cases {
        std::fs::write(d.join("t.cpp"), &src).unwrap();
        let args = [
            compat,
            "--target=aarch64-unknown-linux-gnu",
            "-ffreestanding",
            march,
            "-S",
            "-emit-llvm",
            "-o",
            "k.ll",
        ];
        let o = kiln_in(&d, &[&args[..], &["t.cpp"]].concat());
        assert!(text(&o.stderr).starts_with(want), "{src}: {}", text(&o.stderr));
        assert_eq!(o.status.code(), Some(1), "{src}");
    }
    // The tuple is accepted by value where clang 19 passes it as one vector.
    std::fs::write(d.join("t.cpp"), format!("{sve}void a(svint8x2_t) {{}}\n")).unwrap();
    let o = kiln_in(
        &d,
        &[
            "-fclang-compat=19",
            "--target=aarch64-unknown-linux-gnu",
            "-ffreestanding",
            "-march=armv8-a+sve",
            "-S",
            "-emit-llvm",
            "-o",
            "k.ll",
            "t.cpp",
        ],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
}

/// WP-810: function multiversioning kiln does not implement is a hard
/// error, never a silently different function: on x86-64 every form clang
/// multiversions (`target_version` only before clang 22, an unknown
/// attribute from 22), and on `AArch64` a non-default `target_version` member
/// or friend (clang crashes or drops a later definition there).
#[test]
fn multiversion_unsupported_errors() {
    let d = scratch("fmvx86");
    let src = "__attribute__((target_clones(\"avx2\",\"default\"))) int a() { return 1; }\n\
               __attribute__((cpu_dispatch(atom, generic))) int b();\n\
               __attribute__((cpu_specific(atom))) int c() { return 1; }\n\
               __attribute__((target(\"default\"))) int d() { return 1; }\n\
               __attribute__((target(\"avx2\"))) int e();\n\
               __attribute__((target(\"sse4.2\"))) int e();\n\
               __attribute__((target(\"avx2\"))) int f() { return 1; }\n\
               int f();\n\
               __attribute__((target_version(\"default\"))) int g() { return 1; }\n";
    std::fs::write(d.join("t.cpp"), src).unwrap();
    let mv = |l: u32, k: &str| {
        format!("t.cpp:{l}:16: error: function multiversioning with '{k}' is not supported yet")
    };
    for (compat, last) in [
        (
            "-fclang-compat=22",
            "t.cpp:9:16: warning: unknown attribute 'target_version' ignored".to_owned(),
        ),
        ("-fclang-compat=19", mv(9, "target_version")),
    ] {
        let args = [compat, "--target=x86_64-unknown-linux-gnu", "-fsyntax-only", "t.cpp"];
        let o = kiln_in(&d, &args);
        let err = text(&o.stderr);
        let lines: Vec<&str> = err.lines().filter(|l| l.starts_with("t.cpp:")).collect();
        let want = [
            mv(1, "target_clones"),
            mv(2, "cpu_dispatch"),
            mv(3, "cpu_specific"),
            mv(4, "target"),
            mv(6, "target"),
            last,
        ];
        assert_eq!(lines.len(), want.len(), "{compat}: {err}");
        for (l, w) in lines.iter().zip(&want) {
            assert!(l.starts_with(w.as_str()), "{compat}: {l} != {w}");
        }
        assert_eq!(o.status.code(), Some(1), "{compat}");
    }
    let src = "struct S {\n  __attribute__((target_version(\"sve\"))) int m();\n  \
               friend __attribute__((target_version(\"sve\"))) int fr(S) { return 1; }\n};\n";
    std::fs::write(d.join("t.cpp"), src).unwrap();
    let args = ["--target=aarch64-unknown-linux-gnu", "-fsyntax-only", "t.cpp"];
    let o = kiln_in(&d, &args);
    let err = text(&o.stderr);
    let lines: Vec<&str> = err.lines().filter(|l| l.starts_with("t.cpp:")).collect();
    let tv = "error: function multiversioning with 'target_version' is not supported yet";
    assert_eq!(lines, [format!("t.cpp:2:18: {tv}"), format!("t.cpp:3:25: {tv}")], "{err}");
    assert_eq!(o.status.code(), Some(1));
}

/// spec/codegen.md "Sanitizers" (WP-675): the frontend's ASan and MSan
/// output: function attributes, the `no_sanitize` opt-outs, the global
/// `no_sanitize_address`/`sanitize_address_dyninit` markers and the
/// use-after-scope lifetime markers. (Not an `expect: ir` test: clang keeps
/// local value names under ASan/MSan and kiln does not name values.)
#[test]
fn sanitizer_frontend_attributes() {
    let d = scratch("sanitize");
    let src = "int g = 1;\n\
               __attribute__((no_sanitize(\"address\"))) int ng = 2;\n\
               extern int ext;\n\
               int dyn = ext + 1;\n\
               void use(int*);\n\
               int f(int x) { int a[4]; use(a); return a[x] + g; }\n\
               __attribute__((no_sanitize(\"address\", \"memory\"))) int f2(int x) { return x; }\n\
               __attribute__((disable_sanitizer_instrumentation)) int f3(int x) { return x; }\n";
    std::fs::write(d.join("s.cpp"), src).unwrap();
    let ir = |san: &str| {
        let o = kiln_in(
            &d,
            &[san, "-Xclang", "-disable-llvm-passes", "-S", "-emit-llvm", "-o", "-", "s.cpp"],
        );
        assert_eq!((o.status.code(), text(&o.stderr).as_str()), (Some(0), ""), "{san}");
        text(&o.stdout)
    };
    // The attribute group of the function `name`'s definition.
    let attrs = |ll: &str, name: &str| -> String {
        let def = ll.lines().find(|l| l.starts_with("define") && l.contains(name)).unwrap();
        let group = def.rsplit(' ').nth(1).unwrap();
        let line = format!("attributes {group} = ");
        ll.lines().find(|l| l.starts_with(&line)).unwrap().to_owned()
    };
    let ll = ir("-fsanitize=address");
    assert!(ll.contains("@ng = dso_local global i32 2, no_sanitize_address, align 4"), "{ll}");
    assert!(ll.contains("@dyn = dso_local global i32 0, sanitize_address_dyninit, align 4"));
    assert!(ll.contains("@g = dso_local global i32 1, align 4"), "{ll}");
    assert!(attrs(&ll, "@_Z1fi").contains(" sanitize_address "), "{ll}");
    assert!(!attrs(&ll, "@_Z2f2i").contains("sanitize_"), "{ll}");
    assert!(attrs(&ll, "@_Z2f3i").contains("disable_sanitizer_instrumentation"), "{ll}");
    assert!(!attrs(&ll, "@_Z2f3i").contains(" sanitize_address"), "{ll}");
    assert!(ll.contains("call void @llvm.lifetime.start.p0("), "{ll}");
    let ll = ir("-fsanitize=memory");
    assert!(attrs(&ll, "@_Z1fi").contains(" sanitize_memory "), "{ll}");
    assert!(!attrs(&ll, "@_Z2f2i").contains("sanitize_"), "{ll}");
    assert!(!ll.contains("no_sanitize_address"), "{ll}");
}
