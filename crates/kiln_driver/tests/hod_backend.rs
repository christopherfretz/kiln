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
/// module, a sanitizer, `-S`, `-emit-llvm`.
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
        (&["-std=c++20"][..], "coro.cpp"),
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
    assert!(lines[0].ends_with(",\"exit\":0,\"backend\":\"hod\",\"hod_reason\":null}"));
    assert!(
        lines[1].ends_with(
            ",\"backend\":\"hod-declined\",\"hod_reason\":\"alwaysinline:frame-address\"}"
        )
    );
    assert!(lines[1].contains("\"hod\":") && lines[1].contains("\"llc\":"), "{}", lines[1]);
    assert!(lines[2].ends_with(",\"backend\":\"llvm\",\"hod_reason\":null}"), "{}", lines[2]);
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
