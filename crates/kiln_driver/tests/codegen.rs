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
