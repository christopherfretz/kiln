//! `KILN_STATS_LOG=<path>` (spec/driver.md "Compile statistics log",
//! WP-1370): each compile appends one well-formed JSON line with the
//! `opt` triggers of its IR; two compiles append two lines; the variable
//! changes no output. Tests that run `llc` / `opt` skip without `llc-19`,
//! `opt-19` and `clang++-19`.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use kiln_source::trace::Json;

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

fn setup(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-stats-log-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("plain.cpp"), PLAIN).unwrap();
    std::fs::write(d.join("inline.cpp"), INLINE).unwrap();
    std::fs::write(d.join("coro.cpp"), CORO).unwrap();
    d
}

fn kiln(dir: &Path, log: Option<&str>, args: &[&str]) -> Output {
    let mut c = Command::new(kilnpp());
    c.args(args).current_dir(dir).env_remove("KILN_STATS_LOG");
    if let Some(l) = log {
        c.env("KILN_STATS_LOG", l);
    }
    let o = c.output().unwrap();
    assert!(o.status.success(), "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
    o
}

/// The log's lines, each parsed.
fn lines(dir: &Path) -> Vec<Json> {
    let text = std::fs::read_to_string(dir.join("stats.jsonl")).unwrap_or_default();
    assert!(text.is_empty() || text.ends_with('\n'), "{text}");
    text.lines()
        .map(|l| {
            assert!(l.len() < 4096);
            Json::parse(l).unwrap_or_else(|e| panic!("{e}: {l}"))
        })
        .collect()
}

fn triggers(j: &Json) -> Vec<String> {
    j.get("triggers")
        .and_then(Json::as_array)
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap().to_owned())
        .collect()
}

fn num(j: &Json, k: &str) -> Option<u64> {
    j.get(k).and_then(Json::as_u64)
}

/// Every contract field is present, in order (spec/driver.md "Compile
/// statistics log").
fn well_formed(line: &Json) {
    for k in [
        "input",
        "output_kind",
        "opt_level",
        "debug",
        "lto",
        "triggers",
        "alwaysinline_defs",
        "alwaysinline_callsites",
        "available_externally_defs",
        "ir_bytes",
        "functions_defined",
        "frontend_ms",
        "emit_ir_ms",
        "print_ir_ms",
        "children_ms",
        "total_ms",
        "exit",
    ] {
        assert!(line.get(k).is_some(), "{k} missing: {line:?}");
    }
}

/// (a) a plain TU: no trigger, `llc` alone; two compiles, two lines.
#[test]
fn plain_tu_appends_one_line_per_compile() {
    if !tools() {
        return;
    }
    let d = setup("plain");
    let without = kiln(&d, None, &["-c", "plain.cpp", "-o", "a.o"]);
    let with = kiln(&d, Some("stats.jsonl"), &["-c", "plain.cpp", "-o", "b.o"]);
    // The variable changes no output.
    assert_eq!(without.stderr, with.stderr);
    assert_eq!(std::fs::read(d.join("a.o")).unwrap(), std::fs::read(d.join("b.o")).unwrap());
    let l = lines(&d);
    assert_eq!(l.len(), 1);
    well_formed(&l[0]);
    assert_eq!(l[0].get("input").and_then(Json::as_str), Some("plain.cpp"));
    assert_eq!(l[0].get("output_kind").and_then(Json::as_str), Some("obj"));
    assert_eq!(l[0].get("opt_level").and_then(Json::as_str), Some("0"));
    assert_eq!(l[0].get("lto").and_then(Json::as_str), Some("none"));
    assert!(triggers(&l[0]).is_empty());
    assert_eq!(num(&l[0], "functions_defined"), Some(1));
    assert_eq!(num(&l[0], "alwaysinline_defs"), Some(0));
    assert_eq!(num(&l[0], "exit"), Some(0));
    let children = l[0].get("children_ms").unwrap();
    assert!(children.get("llc").is_some() && children.get("opt").is_none(), "{children:?}");
    kiln(&d, Some("stats.jsonl"), &["-c", "plain.cpp", "-o", "b.o"]);
    assert_eq!(lines(&d).len(), 2);
    let _ = std::fs::remove_dir_all(&d);
}

/// (b) an `always_inline` function: the `alwaysinline` trigger, `opt`
/// timed, the definition counted.
#[test]
fn always_inline_tu() {
    if !tools() {
        return;
    }
    let d = setup("inline");
    kiln(&d, Some("stats.jsonl"), &["-c", "inline.cpp", "-o", "inline.o"]);
    let l = lines(&d);
    assert_eq!(l.len(), 1);
    well_formed(&l[0]);
    assert_eq!(triggers(&l[0]), ["alwaysinline"]);
    assert_eq!(num(&l[0], "alwaysinline_defs"), Some(1));
    assert_eq!(num(&l[0], "functions_defined"), Some(2));
    let children = l[0].get("children_ms").unwrap();
    assert!(children.get("opt").is_some() && children.get("llc").is_some(), "{children:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (c) a coroutine: the `coro` trigger; the IR text written as is.
#[test]
fn coroutine_tu() {
    if !tools() {
        return;
    }
    let d = setup("coro");
    kiln(&d, Some("stats.jsonl"), &["-std=c++20", "-S", "-emit-llvm", "coro.cpp", "-o", "c.ll"]);
    let l = lines(&d);
    assert_eq!(l.len(), 1);
    well_formed(&l[0]);
    assert_eq!(l[0].get("output_kind").and_then(Json::as_str), Some("llvm-ir"));
    assert!(triggers(&l[0]).contains(&"coro".to_owned()), "{l:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// (d) `-fsyntax-only`: a line with no module (null counts, no trigger,
/// no child); a link run adds the link's line (`input` null).
#[test]
fn syntax_only_and_link_lines() {
    let d = setup("syntax");
    kiln(&d, Some("stats.jsonl"), &["-fsyntax-only", "plain.cpp", "inline.cpp"]);
    let l = lines(&d);
    assert_eq!(l.len(), 2);
    for j in &l {
        well_formed(j);
        assert_eq!(j.get("output_kind").and_then(Json::as_str), Some("syntax-only"));
        assert!(triggers(j).is_empty());
        assert_eq!(j.get("ir_bytes"), Some(&Json::Null));
    }
    assert_eq!(l[1].get("input").and_then(Json::as_str), Some("inline.cpp"));
    if !tools() {
        return;
    }
    std::fs::write(d.join("main.cpp"), "int main() { return 0; }\n").unwrap();
    kiln(&d, Some("stats.jsonl"), &["main.cpp", "-o", "a.out"]);
    let l = lines(&d);
    assert_eq!(l.len(), 4);
    assert_eq!(l[2].get("output_kind").and_then(Json::as_str), Some("link"));
    assert_eq!(l[3].get("input"), Some(&Json::Null));
    assert!(l[3].get("children_ms").unwrap().get("clang++").is_some(), "{:?}", l[3]);
    let _ = std::fs::remove_dir_all(&d);
}
