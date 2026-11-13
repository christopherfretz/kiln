//! `-fkiln-backend=hod` (spec/driver.md "Back-end selection", WP-1371)
//! with a FAKE hod: a shell script that records its arguments and
//! standard input, then plays one of hod's answers (status 0 with an
//! object, status 2 declined, status 1, a signal). No test needs a real
//! hod. Tests whose fallback runs `opt` / `llc` skip without `llc-19`,
//! `opt-19` and `clang++-19`; the others use a stub LLVM installation
//! (`--llvm-dir=`) whose `llc` and `opt` only record that they ran.
#![forbid(unsafe_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn tools() -> bool {
    let ok = have("llc-19") && have("opt-19") && have("clang++-19");
    if !ok {
        eprintln!("skipping: llc-19 / opt-19 / clang++-19 not found");
    }
    ok
}

const PLAIN: &str = "int add(int a, int b) { return a + b; }\n";

const INLINE: &str = "[[gnu::always_inline]] inline int twice(int x) { return 2 * x; }\n\
                      int f(int x) { return twice(x) + twice(x + 1); }\n";

const CORO: &str = r"namespace std {
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
  operator coroutine_handle<>() const noexcept { return coroutine_handle<>::from_address(p); }
};
struct suspend_never {
  bool await_ready() const noexcept { return true; }
  void await_suspend(coroutine_handle<>) const noexcept {}
  void await_resume() const noexcept {}
};
} // namespace std
struct task {
  struct promise_type {
    task get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};
task run() { co_return; }
";

/// A generator (a real switch-resumed coroutine) that prints what it
/// yields: `0 1 1 2 3 5 8 13 21 34 | 88`; `gen_inline.cpp` has an
/// `always_inline` function of its own too.
const GEN: &str = r#"extern "C" int printf(const char *, ...);
namespace std {
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
    coroutine_handle h; h.p = __builtin_coro_promise(&pr, alignof(P), true); return h;
  }
  operator coroutine_handle<>() const noexcept { return coroutine_handle<>::from_address(p); }
  void *address() const noexcept { return p; }
  P &promise() const { return *static_cast<P *>(__builtin_coro_promise(p, alignof(P), false)); }
  bool done() const { return __builtin_coro_done(p); }
  void resume() const { __builtin_coro_resume(p); }
  void destroy() const { __builtin_coro_destroy(p); }
};
struct suspend_always {
  bool await_ready() const noexcept { return false; }
  void await_suspend(coroutine_handle<>) const noexcept {}
  void await_resume() const noexcept {}
};
} // namespace std
struct gen {
  struct promise_type {
    int value = 0;
    gen get_return_object() { return gen{std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_always initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    std::suspend_always yield_value(int v) noexcept { value = v; return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
  ~gen() { h.destroy(); }
  bool next() { h.resume(); return !h.done(); }
};
STEP
gen fib(int n) {
  int a = 0, b = 1;
  for (int i = 0; i < n; ++i) { co_yield a; int t = step(a, b); a = b; b = t; }
}
int main() {
  gen g = fib(10);
  int sum = 0;
  while (g.next()) { printf("%d ", g.h.promise().value); sum += g.h.promise().value; }
  printf("| %d\n", sum);
  return sum == 88 ? 0 : 1;
}
"#;

/// Only a coroutine handle's operations, no coroutine body: the `opt`
/// trigger set {`coro`} (a body's `await_suspend` wrapper is
/// `alwaysinline`, so every module with a body has {`alwaysinline`,
/// `coro`}, as clang's).
const HANDLE: &str =
    "void resume(void *p) { if (!__builtin_coro_done(p)) __builtin_coro_resume(p); }\n";

/// What [`GEN`] prints.
const GEN_OUT: &str = "0 1 1 2 3 5 8 13 21 34 | 88\n";

/// hod's status-2 line for a declined module.
const DECLINED: &str = "hod: error: unsupported construct 'alwaysinline:frame-address' \
                        (strict mode: no fallback)";

fn script(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-hod-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("plain.cpp"), PLAIN).unwrap();
    std::fs::write(d.join("inline.cpp"), INLINE).unwrap();
    std::fs::write(d.join("coro.cpp"), CORO).unwrap();
    std::fs::write(d.join("handle.cpp"), HANDLE).unwrap();
    let step = "inline int step(int a, int b) { return a + b; }";
    std::fs::write(d.join("gen.cpp"), GEN.replace("STEP", step)).unwrap();
    let step = format!("[[gnu::always_inline]] {step}");
    std::fs::write(d.join("gen_inline.cpp"), GEN.replace("STEP", &step)).unwrap();
    d
}

/// A fake hod at `path`: it appends its arguments to `<dir>/hod.args`,
/// saves its standard input as `<dir>/hod.stdin`, finds its `-o` file
/// (`$out`), then runs `then`.
fn fake_hod(path: &Path, dir: &Path, then: &str) {
    let d = dir.display();
    script(
        path,
        &format!(
            "out=\nprev=\nfor a in \"$@\"; do\n  [ \"$prev\" = -o ] && out=\"$a\"\n  prev=\"$a\"\ndone\n\
             echo \"$*\" >> '{d}/hod.args'\ncat > '{d}/hod.stdin'\n{then}"
        ),
    );
}

/// The fake's answers.
const OK: &str = "printf FAKEOBJ > \"$out\"\nexit 0";
const DECLINE: &str = "echo \"$DECLINED\" >&2\nexit 2";
const FAIL: &str = "printf PARTIAL > \"$out\"\necho 'hod: error: internal error: boom' >&2\nexit 1";
const KILLED: &str = "printf PARTIAL > \"$out\"\nkill -9 $$";
/// hod's contained panic (its reason `internal-panic`): status 1, as
/// `internal-error`.
const PANICKED: &str = "printf PARTIAL > \"$out\"\n\
    echo 'hod: error: internal error: hod panicked: index out of bounds (strict mode: no \
    fallback)' >&2\nexit 1";

/// A stub LLVM installation at `<dir>/llvm` whose `llc` and `opt` append
/// their name to `<dir>/llvm.ran` (and write nothing).
fn stub_llvm(dir: &Path) -> String {
    let llvm = dir.join("llvm");
    for t in ["opt", "llc"] {
        script(
            &llvm.join("bin").join(t),
            &format!("cat > /dev/null\necho {t} >> '{}/llvm.ran'", dir.display()),
        );
    }
    script(&llvm.join("bin/clang++"), "exit 0");
    std::fs::create_dir_all(llvm.join("lib/clang/22")).unwrap();
    format!("--llvm-dir={}", llvm.display())
}

fn run(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut c = Command::new(kilnpp());
    c.args(args)
        .current_dir(dir)
        .env_remove("KILN_BACKEND")
        .env_remove("KILN_HOD")
        .env_remove("KILN_HOD_STRICT")
        .env_remove("KILN_STATS_LOG")
        .env("DECLINED", DECLINED);
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn read(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_default()
}

/// Status 0: hod's object is the output; neither `opt` nor `llc` runs
/// (`-v` shows only the hod command); hod got `--strict`, the `llc`
/// arguments and the module before `opt` on its standard input: the
/// text of `-S -emit-llvm -Xclang -disable-llvm-passes`.
#[test]
fn compiled_by_hod_skips_opt_and_llc() {
    let d = scratch("ok");
    let llvm = stub_llvm(&d);
    let hod = d.join("bin/hod");
    fake_hod(&hod, &d, OK);
    let flag = format!("--hod={}", hod.display());
    let o = run(&d, &[], &["-c", "-v", "-fkiln-backend=hod", &flag, &llvm, "inline.cpp"]);
    let err = text(&o.stderr);
    assert!(o.status.success(), "{err}");
    assert_eq!(read(&d.join("inline.o")), b"FAKEOBJ");
    assert!(!d.join("llvm.ran").exists(), "{}", text(&read(&d.join("llvm.ran"))));
    let hod_line =
        format!(" \"{}\" --strict -O0 -relocation-model=pic -filetype=obj", hod.display());
    assert!(err.lines().any(|l| l.starts_with(&hod_line)), "{err}");
    assert!(!err.contains("llvm/bin/llc") && !err.contains("llvm/bin/opt"), "{err}");
    let args = text(&read(&d.join("hod.args")));
    assert!(args.starts_with("--strict -O0 ") && args.trim_end().ends_with("-o inline.o -"));
    let o = run(
        &d,
        &[],
        &["-S", "-emit-llvm", "-Xclang", "-disable-llvm-passes", &llvm, "inline.cpp", "-o", "x.ll"],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert_eq!(text(&read(&d.join("hod.stdin"))), text(&read(&d.join("x.ll"))));
    assert!(text(&read(&d.join("x.ll"))).contains("alwaysinline"));
    // A link: hod writes each source's temporary object; `llc` never runs.
    let o = run(&d, &[], &["-fkiln-backend=hod", &flag, &llvm, "plain.cpp", "-o", "a.out"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(!d.join("llvm.ran").exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// The executable: `hod` next to `llc`, then `PATH`; `KILN_HOD` beats
/// both; `--hod=` beats `KILN_HOD`. Not found: an error naming what was
/// searched, never a silent LLVM compile.
#[test]
fn executable_search_and_not_found() {
    let d = scratch("find");
    let llvm = stub_llvm(&d);
    let ok = |o: &Output| assert!(o.status.success(), "{}", text(&o.stderr));
    // Next to llc.
    fake_hod(&d.join("llvm/bin/hod"), &d, "echo next >> \"$out.who\"\nprintf X > \"$out\"");
    // On PATH.
    fake_hod(&d.join("path/hod"), &d, "echo path >> \"$out.who\"\nprintf X > \"$out\"");
    // Named.
    fake_hod(&d.join("env/hod"), &d, "echo env >> \"$out.who\"\nprintf X > \"$out\"");
    fake_hod(&d.join("flag/hod"), &d, "echo flag >> \"$out.who\"\nprintf X > \"$out\"");
    let path = format!("{}:/usr/bin:/bin", d.join("path").display());
    let env_hod = d.join("env/hod").display().to_string();
    let flag = format!("--hod={}", d.join("flag/hod").display());
    let c = ["-c", "-fkiln-backend=hod", llvm.as_str(), "plain.cpp"];
    ok(&run(&d, &[("PATH", &path)], &c));
    ok(&run(&d, &[("PATH", &path), ("KILN_HOD", &env_hod)], &c));
    ok(&run(&d, &[("PATH", &path), ("KILN_HOD", &env_hod)], &[&c[..], &[&flag]].concat()));
    std::fs::remove_file(d.join("llvm/bin/hod")).unwrap();
    ok(&run(&d, &[("PATH", &path)], &c));
    assert_eq!(text(&read(&d.join("plain.o.who"))), "next\nenv\nflag\npath\n");
    // Nothing found.
    let o = run(&d, &[("PATH", "/nonexistent-wp1371")], &c);
    assert_eq!(o.status.code(), Some(1));
    let want = format!(
        "kiln++: error: -fkiln-backend=hod: no hod executable found (searched '{}' (next to \
         llc), PATH)\n",
        d.join("llvm/bin/hod").display()
    );
    assert_eq!(text(&o.stderr), want);
    assert!(!d.join("llvm.ran").exists());
    let o = run(&d, &[], &[&c[..], &["--hod=/nonexistent-wp1371/hod"]].concat());
    assert_eq!(
        text(&o.stderr),
        "kiln++: error: -fkiln-backend=hod: no hod executable found (searched \
         '/nonexistent-wp1371/hod' (from --hod=))\n"
    );
    let o = run(&d, &[("KILN_HOD", "/nonexistent-wp1371/hod")], &c);
    assert!(text(&o.stderr).contains("'/nonexistent-wp1371/hod' (from KILN_HOD)"));
    // Where hod is not used, none is needed.
    let o = run(
        &d,
        &[("PATH", "/nonexistent-wp1371")],
        &["-S", "-fkiln-backend=hod", &llvm, "plain.cpp"],
    );
    ok(&o);
    let _ = std::fs::remove_dir_all(&d);
}

/// `-fkiln-backend=` beats `KILN_BACKEND`; the last flag wins; LLVM by
/// default; any other value is a usage error.
#[test]
fn selection_precedence_and_bad_values() {
    let d = scratch("select");
    let llvm = stub_llvm(&d);
    let hod = d.join("hod");
    fake_hod(&hod, &d, OK);
    let k = [("KILN_HOD", hod.to_str().unwrap())];
    let calls = || text(&read(&d.join("hod.args"))).lines().count();
    let c = ["-c", llvm.as_str(), "plain.cpp"];
    run(&d, &k, &c);
    assert_eq!(calls(), 0, "default is llvm");
    run(&d, &[k[0], ("KILN_BACKEND", "hod")], &c);
    assert_eq!(calls(), 1, "KILN_BACKEND=hod");
    run(&d, &[k[0], ("KILN_BACKEND", "hod")], &[&c[..], &["-fkiln-backend=llvm"]].concat());
    assert_eq!(calls(), 1, "the flag beats the variable");
    run(&d, &[k[0], ("KILN_BACKEND", "llvm")], &[&c[..], &["-fkiln-backend=hod"]].concat());
    assert_eq!(calls(), 2, "the flag beats the variable");
    run(&d, &k, &[&c[..], &["-fkiln-backend=hod", "-fkiln-backend=llvm"]].concat());
    assert_eq!(calls(), 2, "the last flag wins");
    run(&d, &[k[0], ("KILN_BACKEND", "")], &c);
    assert_eq!(calls(), 2, "an empty variable is unset");
    let o = run(&d, &k, &[&c[..], &["-fkiln-backend=gcc"]].concat());
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(text(&o.stderr), "kiln++: error: invalid value 'gcc' in '-fkiln-backend=gcc'\n");
    let o = run(&d, &[k[0], ("KILN_BACKEND", "Hod")], &c);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(text(&o.stderr), "kiln++: error: invalid value 'Hod' in 'KILN_BACKEND=Hod'\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// `-###` shows the hod command, then the marker naming the `llc`
/// command after it as the fallback; nothing runs.
#[test]
fn print_commands_marks_the_fallback() {
    let d = scratch("hash");
    let llvm = stub_llvm(&d);
    let hod = d.join("hod");
    fake_hod(&hod, &d, OK);
    let flag = format!("--hod={}", hod.display());
    let o = run(&d, &[], &["-###", "-c", "-fkiln-backend=hod", &flag, &llvm, "plain.cpp"]);
    assert!(o.status.success());
    let err = text(&o.stderr);
    let lines: Vec<&str> = err.lines().skip_while(|l| !l.contains("(in-process)")).collect();
    assert_eq!(
        lines[1],
        format!(
            " \"{}\" \"--strict\" \"-O0\" \"-relocation-model=pic\" \"-filetype=obj\" \"-o\" \"plain.o\" \"-\"",
            hod.display()
        )
    );
    assert_eq!(lines[2], " (fallback if hod declines or fails: opt as the module needs, then llc)");
    assert!(lines[3].contains("/llc\" \"-O0\""), "{err}");
    assert!(!d.join("hod.args").exists());
    // Not planned at -O2: no hod line.
    let o = run(&d, &[], &["-###", "-c", "-O2", "-fkiln-backend=hod", &flag, &llvm, "plain.cpp"]);
    assert!(!text(&o.stderr).contains("--strict"), "{}", text(&o.stderr));
    let _ = std::fs::remove_dir_all(&d);
}

/// Compiles `src` to `out` with `extra` and the environment `env`;
/// returns stderr.
fn compile(d: &Path, env: &[(&str, &str)], extra: &[&str], src: &str, out: &str) -> Output {
    run(d, env, &[&["-c", src, "-o", out][..], extra].concat())
}

/// Status 2: the LLVM route, byte-identical to `-fkiln-backend=llvm`,
/// no diagnostic; `-v` names the reason before the `opt` / `llc`
/// commands.
#[test]
fn declined_falls_back_silently() {
    if !tools() {
        return;
    }
    let d = scratch("declined");
    let hod = d.join("hod");
    fake_hod(&hod, &d, DECLINE);
    let flag = format!("--hod={}", hod.display());
    let o = compile(&d, &[], &[], "inline.cpp", "llvm.o");
    assert!(o.status.success(), "{}", text(&o.stderr));
    let o = compile(&d, &[], &["-fkiln-backend=hod", &flag], "inline.cpp", "hod.o");
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert_eq!(text(&o.stderr), "");
    assert!(!read(&d.join("hod.args")).is_empty());
    assert!(!read(&d.join("llvm.o")).is_empty());
    assert_eq!(read(&d.join("llvm.o")), read(&d.join("hod.o")));
    let o = compile(&d, &[], &["-v", "-fkiln-backend=hod", &flag], "inline.cpp", "v.o");
    let err = text(&o.stderr);
    let at = |s: &str| err.find(s).unwrap_or_else(|| panic!("{s}: {err}"));
    let reason = at(" (hod declined 'alwaysinline:frame-address'; falling back to llc)\n");
    assert!(at("--strict") < reason && reason < at("-passes=default<O0>") && reason < at("/llc"));
    let _ = std::fs::remove_dir_all(&d);
}

/// Status 1 and death by a signal: one warning, then the LLVM route (the
/// fake's partial `-o` file is replaced); under `KILN_HOD_STRICT=1` an
/// error, and no `-o` file is left behind.
#[test]
fn failure_warns_and_falls_back_or_is_an_error_when_strict() {
    if !tools() {
        return;
    }
    let d = scratch("failed");
    let o = compile(&d, &[], &[], "inline.cpp", "llvm.o");
    assert!(o.status.success(), "{}", text(&o.stderr));
    for (then, status, message) in [
        (FAIL, "exit code 1", "hod: error: internal error: boom"),
        (KILLED, "signal 9", "no message"),
    ] {
        let hod = d.join("hod");
        fake_hod(&hod, &d, then);
        let flag = format!("--hod={}", hod.display());
        let o = compile(&d, &[], &["-fkiln-backend=hod", &flag], "inline.cpp", "hod.o");
        assert!(o.status.success(), "{}", text(&o.stderr));
        assert_eq!(
            text(&o.stderr),
            format!("kiln++: warning: hod failed ({status}): {message}; falling back to llc\n")
        );
        assert_eq!(read(&d.join("llvm.o")), read(&d.join("hod.o")));
        let _ = std::fs::remove_file(d.join("hod.o"));
        let o = compile(
            &d,
            &[("KILN_HOD_STRICT", "1")],
            &["-fkiln-backend=hod", &flag],
            "inline.cpp",
            "hod.o",
        );
        assert_eq!(o.status.code(), Some(1));
        assert_eq!(
            text(&o.stderr),
            format!(
                "kiln++: error: hod failed ({status}): {message} (KILN_HOD_STRICT=1: no \
                 fallback)\n"
            )
        );
        assert!(!d.join("hod.o").exists(), "a partial object survived");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Outside its cases hod is never invoked: `-O2`, `-flto`, a coroutine
/// module with the entry-instrumentation attribute, a sanitizer, `-S`,
/// `-emit-llvm`.
#[test]
fn not_invoked_outside_its_cases() {
    if !tools() {
        return;
    }
    let d = scratch("outside");
    let hod = d.join("hod");
    fake_hod(&hod, &d, OK);
    let flag = format!("--hod={}", hod.display());
    for (extra, src) in [
        (&["-O2"][..], "inline.cpp"),
        (&["-flto"][..], "plain.cpp"),
        (&["-std=c++20", "-finstrument-function-entry-bare"][..], "coro.cpp"),
        (&["-fsanitize=address"][..], "plain.cpp"),
        (&["-fsanitize=undefined"][..], "plain.cpp"),
        (&["-S"][..], "plain.cpp"),
        (&["-emit-llvm"][..], "plain.cpp"),
    ] {
        let o = compile(&d, &[], &[&["-fkiln-backend=hod", &flag][..], extra].concat(), src, "x");
        assert!(o.status.success(), "{extra:?}: {}", text(&o.stderr));
        assert!(!d.join("hod.args").exists(), "{extra:?} {src}: hod ran");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// `KILN_STATS_LOG` names the back end, hod's reason and hod's time.
#[test]
fn stats_log_names_the_backend() {
    let d = scratch("stats");
    let llvm = stub_llvm(&d);
    let hod = d.join("hod");
    fake_hod(&hod, &d, OK);
    let log = d.join("s.jsonl");
    let env = [("KILN_STATS_LOG", log.to_str().unwrap()), ("KILN_BACKEND", "hod")];
    let flag = format!("--hod={}", hod.display());
    let o = run(&d, &env, &["-c", &flag, &llvm, "plain.cpp"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    fake_hod(&hod, &d, DECLINE);
    let o = run(&d, &env, &["-c", &flag, &llvm, "plain.cpp"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let o = run(&d, &[env[0]], &["-c", &flag, &llvm, "plain.cpp"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let lines = text(&read(&log));
    let lines: Vec<&str> = lines.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("\"children_ms\":{\"hod\":"), "{}", lines[0]);
    assert!(
        lines[0]
            .ends_with(",\"exit\":0,\"backend\":\"hod\",\"hod_reason\":null,\"hod_skip\":null}")
    );
    assert!(lines[1].ends_with(
        ",\"backend\":\"hod-declined\",\"hod_reason\":\"alwaysinline:frame-address\",\
             \"hod_skip\":null}"
    ));
    assert!(lines[1].contains("\"hod\":") && lines[1].contains("\"llc\":"), "{}", lines[1]);
    assert!(
        lines[2].ends_with(",\"backend\":\"llvm\",\"hod_reason\":null,\"hod_skip\":null}"),
        "{}",
        lines[2]
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// `-ftime-trace`: hod's run is a `CodeGenPasses` span with detail
/// `hod`; it is passed no `--time-trace` options and no trace file is
/// expected from it.
#[test]
fn time_trace_scope() {
    let d = scratch("trace");
    let llvm = stub_llvm(&d);
    let hod = d.join("hod");
    fake_hod(&hod, &d, OK);
    let flag = format!("--hod={}", hod.display());
    let o = run(&d, &[], &["-c", "-ftime-trace", "-fkiln-backend=hod", &flag, &llvm, "plain.cpp"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(!text(&read(&d.join("hod.args"))).contains("time-trace"));
    let t = text(&read(&d.join("plain.json")));
    assert!(t.contains("\"name\":\"CodeGenPasses\""), "{t}");
    assert!(t.contains("\"detail\":\"hod\""), "{t}");
    let _ = std::fs::remove_dir_all(&d);
}

/// hod's pre-opt modules (hod `spec/cli.md` "Pre-opt modules"): a
/// module with only coroutine handle operations (`opt` triggers {`coro`})
/// and coroutine modules ({`alwaysinline`, `coro`}) are offered to hod,
/// which replaces `opt` and `llc`; the entry-instrumentation attribute
/// (`instrument-entry`) keeps a coroutine module on the LLVM route.
#[test]
fn coroutine_modules_are_offered() {
    let d = scratch("coro");
    let llvm = stub_llvm(&d);
    let hod = d.join("hod");
    fake_hod(&hod, &d, OK);
    let flag = format!("--hod={}", hod.display());
    for src in ["handle.cpp", "coro.cpp", "gen.cpp", "gen_inline.cpp"] {
        let o = run(&d, &[], &["-c", "-std=c++20", "-fkiln-backend=hod", &flag, &llvm, src]);
        assert!(o.status.success(), "{src}: {}", text(&o.stderr));
        let obj = d.join(src.replace(".cpp", ".o"));
        assert_eq!(read(&obj), b"FAKEOBJ", "{src}");
        assert!(!d.join("llvm.ran").exists(), "{src}: {}", text(&read(&d.join("llvm.ran"))));
        let ir = text(&read(&d.join("hod.stdin")));
        // The module before `opt`: the coroutine is not split.
        let body = src != "handle.cpp";
        assert_eq!(ir.contains("presplitcoroutine"), body, "{src}");
        assert_eq!(ir.contains("@llvm.coro.id"), body, "{src}");
        assert!(ir.contains("@llvm.coro."), "{src}");
    }
    let calls = text(&read(&d.join("hod.args"))).lines().count();
    let o = run(
        &d,
        &[],
        &[
            "-c",
            "-std=c++20",
            "-finstrument-function-entry-bare",
            "-fkiln-backend=hod",
            &flag,
            &llvm,
            "coro.cpp",
            "-o",
            "instr.o",
        ],
    );
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert_eq!(text(&read(&d.join("hod.args"))).lines().count(), calls, "hod ran");
    assert_eq!(text(&read(&d.join("llvm.ran"))), "opt\nllc\n");
    let _ = std::fs::remove_dir_all(&d);
}

/// A declined coroutine module takes today's LLVM route: the coroutine
/// passes run (`-v` shows the decline line, then the `opt` run of the
/// whole `-O0` pipeline, or of the coroutine lowering alone for a module
/// with only handle operations, then `llc`), the object is
/// byte-identical to `-fkiln-backend=llvm`'s, and the program links and
/// runs.
#[test]
fn declined_coroutine_falls_back_and_runs() {
    if !tools() {
        return;
    }
    let d = scratch("coro-declined");
    let hod = d.join("hod");
    fake_hod(&hod, &d, DECLINE);
    let flag = format!("--hod={}", hod.display());
    for (src, passes) in [
        ("handle.cpp", "-passes=coro-early,cgscc(coro-split),coro-cleanup,globaldce"),
        ("gen.cpp", "-passes=default<O0>"),
        ("gen_inline.cpp", "-passes=default<O0>"),
    ] {
        let o = compile(&d, &[], &["-std=c++20"], src, "llvm.o");
        assert!(o.status.success(), "{src}: {}", text(&o.stderr));
        let o = compile(&d, &[], &["-std=c++20", "-v", "-fkiln-backend=hod", &flag], src, "hod.o");
        assert!(o.status.success(), "{src}: {}", text(&o.stderr));
        let err = text(&o.stderr);
        let at = |s: &str| err.find(s).unwrap_or_else(|| panic!("{src}: {s}: {err}"));
        let reason = at(" (hod declined 'alwaysinline:frame-address'; falling back to llc)\n");
        assert!(at("--strict") < reason && reason < at(passes) && at(passes) < at("/llc"), "{err}");
        assert!(!read(&d.join("llvm.o")).is_empty());
        assert_eq!(read(&d.join("llvm.o")), read(&d.join("hod.o")), "{src}");
        if src == "handle.cpp" {
            continue;
        }
        let o = run(&d, &[], &["-std=c++20", "-fkiln-backend=hod", &flag, src, "-o", "gen.out"]);
        assert!(o.status.success(), "{src}: {}", text(&o.stderr));
        assert_eq!(text(&o.stderr), "", "{src}: a decline is silent");
        let r = Command::new(d.join("gen.out")).output().unwrap();
        assert!(r.status.success(), "{src}: {r:?}");
        assert_eq!(text(&r.stdout), GEN_OUT, "{src}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// hod's own bugs on a coroutine module (`internal-error`, and a
/// contained panic, `internal-panic`) are status 1, not a decline: a
/// warning and the fallback; under `KILN_HOD_STRICT=1` an error, and no
/// `-o` file is left behind.
#[test]
fn coroutine_hod_bug_is_a_failure() {
    let d = scratch("coro-failed");
    let llvm = stub_llvm(&d);
    for (then, message) in [
        (FAIL, "hod: error: internal error: boom"),
        (
            PANICKED,
            "hod: error: internal error: hod panicked: index out of bounds (strict mode: no \
             fallback)",
        ),
    ] {
        let hod = d.join("hod");
        fake_hod(&hod, &d, then);
        let flag = format!("--hod={}", hod.display());
        let args = ["-c", "-std=c++20", "-fkiln-backend=hod", &flag, &llvm, "gen.cpp"];
        let o = run(&d, &[("KILN_HOD_STRICT", "1")], &args);
        assert_eq!(o.status.code(), Some(1));
        assert_eq!(
            text(&o.stderr),
            format!(
                "kiln++: error: hod failed (exit code 1): {message} (KILN_HOD_STRICT=1: no \
                 fallback)\n"
            )
        );
        assert!(!d.join("gen.o").exists(), "a partial object survived");
        assert!(!d.join("llvm.ran").exists(), "strict: no fallback");
        let o = run(&d, &[], &args);
        assert!(o.status.success(), "{}", text(&o.stderr));
        assert_eq!(
            text(&o.stderr),
            format!("kiln++: warning: hod failed (exit code 1): {message}; falling back to llc\n")
        );
        // The fallback ran the coroutine lowering, then `llc` (the stub
        // writes no object, and the partial one was removed).
        assert_eq!(text(&read(&d.join("llvm.ran"))), "opt\nllc\n");
        assert!(!d.join("gen.o").exists(), "a partial object survived");
        std::fs::remove_file(d.join("llvm.ran")).unwrap();
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// `KILN_STATS_LOG`'s `hod_skip`: why a job was not offered to hod
/// (spec/driver.md "Compile statistics log"), the first reason in the
/// documented order; `null` when it was offered or hod is not selected.
#[test]
fn stats_log_names_why_hod_was_skipped() {
    let d = scratch("skip");
    let llvm = stub_llvm(&d);
    let hod = d.join("hod");
    fake_hod(&hod, &d, OK);
    let log = d.join("s.jsonl");
    let flag = format!("--hod={}", hod.display());
    let env = [("KILN_STATS_LOG", log.to_str().unwrap()), ("KILN_BACKEND", "hod")];
    let cases: [(&[&str], &str, &str); 13] = [
        (&["-c"], "plain.cpp", "null"),
        (&["-c", "-std=c++20"], "gen_inline.cpp", "null"),
        (&["-S"], "plain.cpp", "\"output-kind\""),
        (&["-S", "-O2"], "plain.cpp", "\"output-kind\""),
        (&["-S", "-emit-llvm"], "plain.cpp", "\"output-kind\""),
        (&["-fsyntax-only"], "plain.cpp", "\"output-kind\""),
        (&["-c", "-O2", "-flto"], "plain.cpp", "\"opt-level\""),
        (&["-c", "-flto"], "plain.cpp", "\"lto\""),
        (&["-c", "-fsanitize=undefined", "--coverage"], "plain.cpp", "\"sanitizer\""),
        (&["-c", "--coverage"], "plain.cpp", "\"coverage\""),
        (&["-c", "-finstrument-functions"], "plain.cpp", "\"instrument\""),
        (&["-c", "-fpatchable-function-entry=2"], "plain.cpp", "\"instrument\""),
        (
            &["-c", "-std=c++20", "-finstrument-function-entry-bare"],
            "coro.cpp",
            "\"instrument-entry\"",
        ),
    ];
    for (extra, src, _) in cases {
        let o = run(&d, &env, &[&[flag.as_str(), llvm.as_str(), src][..], extra].concat());
        assert!(o.status.success(), "{extra:?}: {}", text(&o.stderr));
    }
    // hod not selected: `null` whatever the run.
    let o = run(&d, &[env[0]], &["-S", &llvm, "plain.cpp"]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    let lines = text(&read(&log));
    let lines: Vec<&str> = lines.lines().collect();
    assert_eq!(lines.len(), cases.len() + 1);
    for ((extra, _, want), l) in cases.iter().zip(&lines) {
        assert!(l.ends_with(&format!(",\"hod_skip\":{want}}}")), "{extra:?}: {l}");
    }
    assert!(lines[cases.len()].ends_with(",\"hod_skip\":null}"), "{}", lines[cases.len()]);
    let _ = std::fs::remove_dir_all(&d);
}
