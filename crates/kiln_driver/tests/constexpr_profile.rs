//! `-fconstexpr-stats`, `-fconstexpr-profile[=<format>|<file>]` and
//! `-fconstexpr-profile-limit=<n>` (spec/driver.md "Constexpr profile",
//! spec/oven.md "Profile counters", WP-1116): the summary's lines, the
//! profile's exact step and call counts on small translation units whose
//! costs are known ([expr.const]: a recursion, a loop, `new`/`delete`, a
//! failing evaluation and trial), its JSON, its ranking, the importer of
//! a module, and silence without the flags.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kiln_source::trace::Json;

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln-constexpr-profile-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Out {
    code: i32,
    stderr: String,
}

/// `kiln++ -std=c++26 <args>` in `dir`.
fn kiln(dir: &Path, args: &[&str]) -> Out {
    let o = Command::new(kilnpp())
        .arg("-std=c++26")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    Out { code: o.status.code().unwrap_or(-1), stderr }
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let o = kiln(dir, args);
    assert_eq!(o.code, 0, "kiln++ {args:?}:\n{}", o.stderr);
    o.stderr
}

/// `src` as `t.cpp` in a fresh directory.
fn tu(name: &str, src: &str) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("t.cpp"), src).unwrap();
    d
}

/// Every line of an NDJSON profile, parsed (each must be one object).
fn ndjson(text: &str) -> Vec<Json> {
    text.lines()
        .map(|l| {
            let j = Json::parse(l).unwrap_or_else(|e| panic!("{e}: {l}"));
            assert!(matches!(j, Json::Obj(_)), "{l}");
            j
        })
        .collect()
}

fn n(j: &Json, k: &str) -> u64 {
    j.get(k).and_then(Json::as_u64).unwrap_or_else(|| panic!("no {k} in {j:?}"))
}

fn s<'a>(j: &'a Json, k: &str) -> &'a str {
    j.get(k).and_then(Json::as_str).unwrap_or_else(|| panic!("no {k} in {j:?}"))
}

/// The JSON profile of `t.cpp` in `d` (on stderr).
fn profile(d: &Path) -> Vec<Json> {
    ndjson(&ok(d, &["-fsyntax-only", "-fconstexpr-profile=json", "t.cpp"]))
}

fn func<'a>(p: &'a [Json], name: &str) -> &'a Json {
    p.iter()
        .find(|j| j.get("function").and_then(Json::as_str) == Some(name))
        .unwrap_or_else(|| panic!("no function {name}"))
}

fn sites(p: &[Json]) -> Vec<&Json> {
    p.iter().filter(|j| j.get("site").is_some()).collect()
}

fn total(p: &[Json]) -> &Json {
    let t = p.last().unwrap();
    assert_eq!(t.get("total"), Some(&Json::Bool(true)));
    t
}

/// The `constexpr: <what> k=v ...` line of a summary, as its pairs.
fn stat(err: &str, what: &str) -> Vec<(String, String)> {
    let line = err
        .lines()
        .find_map(|l| l.strip_prefix(&format!("constexpr: {what} ")))
        .unwrap_or_else(|| panic!("no '{what}' line:\n{err}"));
    line.split(' ')
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap();
            (k.to_owned(), v.to_owned())
        })
        .collect()
}

fn stat_n(err: &str, what: &str, key: &str) -> u64 {
    stat(err, what).into_iter().find(|(k, _)| k == key).unwrap().1.parse().unwrap()
}

const DOWN: &str = "constexpr int down(int n) { return n == 0 ? 0 : 1 + down(n - 1); }\n";

/// A recursion ([expr.const]/10.2 depth, /10.1 steps): each call of
/// `down` is one call row entry and two steps; the inclusive steps are
/// charged once for the outermost activation, not once per depth.
#[test]
fn recursion_counts_are_exact() {
    for k in [10u64, 20] {
        let d = tu(&format!("rec{k}"), &format!("{DOWN}static_assert(down({k}) == {k});\n"));
        let p = profile(&d);
        let f = func(&p, "down");
        assert_eq!(n(f, "calls"), k + 1);
        assert_eq!(n(f, "memo_hits"), 0);
        assert_eq!((n(f, "incl_steps"), n(f, "self_steps")), (2 * (k + 1), 2 * (k + 1)));
        assert!(!matches!(f.get("consteval"), Some(Json::Bool(true))));
        assert_eq!(s(f, "loc"), "t.cpp:1:15");
        let st = sites(&p);
        assert_eq!(st.len(), 1);
        assert_eq!((s(st[0], "site"), s(st[0], "loc")), ("static_assert", "t.cpp:2:1"));
        assert_eq!((n(st[0], "evaluations"), n(st[0], "failed")), (1, 0));
        assert_eq!(n(st[0], "incl_steps"), 2 * (k + 1));
        let t = total(&p);
        assert_eq!((n(t, "evaluations"), n(t, "steps"), n(t, "calls")), (1, 2 * (k + 1), k + 1));
    }
}

const SUM: &str =
    "constexpr int sum(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n";

/// A loop ([stmt.for]): one step per iteration, the same in the summary,
/// the profile and `-fconstexpr-report`.
#[test]
fn loop_steps_are_exact_and_match_the_report() {
    let mut steps = Vec::new();
    for k in [100u64, 200] {
        let d = tu(&format!("loop{k}"), &format!("{SUM}constexpr int b = sum({k});\n"));
        let err = ok(
            &d,
            &[
                "-fsyntax-only",
                "-fconstexpr-stats",
                "-fconstexpr-profile=p.json",
                "-fconstexpr-report=r.json",
                "t.cpp",
            ],
        );
        let p = ndjson(&std::fs::read_to_string(d.join("p.json")).unwrap());
        let r = ndjson(&std::fs::read_to_string(d.join("r.json")).unwrap());
        let f = func(&p, "sum");
        let st = sites(&p);
        assert_eq!((s(st[0], "site"), s(st[0], "loc")), ("init b", "t.cpp:2:15"));
        assert_eq!(n(f, "incl_steps"), n(st[0], "incl_steps"));
        assert_eq!(n(f, "incl_steps"), n(&r[0], "steps"));
        assert_eq!(stat_n(&err, "steps", "count"), n(&r[0], "steps"));
        assert_eq!(stat_n(&err, "evaluations", "count"), 1);
        steps.push(n(f, "incl_steps"));
    }
    assert_eq!(steps[1] - steps[0], 100, "{steps:?}");
}

/// `new`/`delete` ([expr.new], [expr.delete], [expr.const]/10.18): the
/// peak is the larger block, both blocks are reclaimed during the
/// evaluation (WP-060).
#[test]
fn heap_peak_and_reclaimed() {
    let d = tu(
        "heap",
        "constexpr int heap() {\n  int *p = new int[10];\n  delete[] p;\n\
         \x20 int *q = new int[5];\n  delete[] q;\n  return 0;\n}\n\
         static_assert(heap() == 0);\n",
    );
    let err = ok(&d, &["-fsyntax-only", "-fconstexpr-stats", "-fconstexpr-profile=json", "t.cpp"]);
    let mem = stat(&err, "memory");
    let get = |k: &str| mem.iter().find(|(x, _)| x == k).unwrap().1.parse::<u64>().unwrap();
    assert_eq!((get("allocs"), get("peak-live")), (2, 40), "{err}");
    assert_eq!(get("reclaimed"), get("bytes"), "{err}");
    let p = ndjson(
        err.lines().take_while(|l| l.starts_with('{')).collect::<Vec<_>>().join("\n").as_str(),
    );
    assert_eq!(n(func(&p, "heap"), "peak_bytes"), 40);
    assert_eq!(n(sites(&p)[0], "peak_bytes"), 40);
}

/// A failing evaluation ([expr.const]/5.25: a throw that is not caught)
/// counts as failed; a failing trial (the constant initialization of a
/// `const int`, [basic.start.static]/2) is a failed trial, not an
/// evaluation.
#[test]
fn failed_evaluations_and_trials() {
    let bad = "constexpr int bad(int n) { return n > 0 ? n : throw 1; }\n";
    let d = tu("trial", &format!("{bad}const int d = bad(0);\n"));
    let err = ok(&d, &["-fsyntax-only", "-fconstexpr-stats", "-fconstexpr-profile=json", "t.cpp"]);
    assert_eq!(stat_n(&err, "evaluations", "count"), 0, "{err}");
    assert_eq!(stat_n(&err, "trials", "count"), 1, "{err}");
    assert_eq!(stat_n(&err, "trials", "failed"), 1, "{err}");
    let p = ndjson(
        err.lines().take_while(|l| l.starts_with('{')).collect::<Vec<_>>().join("\n").as_str(),
    );
    assert!(sites(&p).is_empty());
    assert_eq!(n(func(&p, "bad"), "calls"), 1);
    assert_eq!(n(total(&p), "trials"), 1);

    let d = tu("fail", &format!("{bad}constexpr int e = bad(0);\n"));
    let o = kiln(&d, &["-fsyntax-only", "-fconstexpr-stats", "-fconstexpr-profile", "t.cpp"]);
    assert_eq!(o.code, 1);
    assert_eq!(stat_n(&o.stderr, "evaluations", "failed"), 1, "{}", o.stderr);
    let row = o.stderr.lines().find(|l| l.contains("  init e   (t.cpp:2:15)")).unwrap();
    let cols: Vec<&str> = row.split_whitespace().collect();
    assert_eq!(&cols[..2], ["1", "1"], "{row}");
}

const FIB: &str = "consteval int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }\n";

/// An immediate invocation ([expr.const]/16) of a memoized recursion:
/// memo hits are calls, charged the steps they stand for, so the
/// function's inclusive steps are the evaluation's. In a variable's
/// initializer the whole-initializer fold reads the stored value
/// (WP-1140, WP-1129): no trial runs the call again.
#[test]
fn consteval_memo_and_trial_rows() {
    let d = tu("fib", &format!("{FIB}int f() {{ return fib(10); }}\n"));
    let p = profile(&d);
    let f = func(&p, "fib");
    assert_eq!(f.get("consteval"), Some(&Json::Bool(true)));
    assert_eq!((n(f, "calls"), n(f, "memo_hits")), (19, 8));
    let st = sites(&p);
    assert_eq!((s(st[0], "site"), s(st[0], "loc")), ("consteval", "t.cpp:2:18"));
    assert_eq!(n(f, "incl_steps"), n(st[0], "incl_steps"));
    assert_eq!(n(total(&p), "steps"), n(st[0], "incl_steps"));

    let d = tu("fib-var", &format!("{FIB}int a = fib(10);\n"));
    let p = profile(&d);
    let (f, t) = (func(&p, "fib"), total(&p));
    assert_eq!(n(f, "incl_steps"), n(t, "steps") + n(t, "trial_steps"));
    assert_eq!(n(t, "trial_steps"), 0);
}

const CSUM: &str =
    "consteval int csum(int n) { int r = 0; for (int i = 0; i < n; ++i) r += i; return r; }\n";

/// The trial folds of a full-expression's completion checks (the
/// overflow and implicit-conversion warnings) read the values of its
/// immediate invocations evaluated first ([expr.const]/16: each is
/// evaluated once, WP-1129), so the trials cost no steps; and codegen
/// reuses a default argument's value: one evaluation per use. The
/// overflow trial of `def() + def()` reads each use's stored value
/// instead of evaluating the default argument again, and a shift count
/// that is an invocation is evaluated once, when the operator is built
/// (WP-1171).
#[test]
fn immediate_invocations_are_not_folded_again() {
    let one = tu("imm-one", &format!("{CSUM}int f(int x) {{ return x + csum(50); }}\n"));
    let one = ok(&one, &["-S", "-emit-llvm", "-o", "t.ll", "-fconstexpr-stats", "t.cpp"]);
    let cost = stat_n(&one, "steps", "count");
    assert!(cost > 50, "{one}");
    assert_eq!(stat_n(&one, "trials", "steps"), 0, "{one}");
    let src = format!(
        "{CSUM}int add(int x) {{ return x + csum(50); }}\n\
         short narrow() {{ return csum(50); }}\n\
         bool cmp(int x) {{ return x < csum(50); }}\n\
         int cond() {{ return csum(50) ? 1 : 2; }}\n\
         int def(int x = csum(50)) {{ return x; }}\n\
         int uses() {{ def(); return def(); }}\n\
         int both() {{ return def() + def(); }}\n"
    );
    let d = tu("imm-trials", &src);
    let err = ok(&d, &["-S", "-emit-llvm", "-o", "t.ll", "-fconstexpr-stats", "t.cpp"]);
    assert_eq!(stat_n(&err, "evaluations", "immediate"), 8, "{err}");
    assert_eq!(stat_n(&err, "steps", "count"), 8 * cost, "{err}");
    assert_eq!(stat_n(&err, "trials", "steps"), 0, "{err}");

    let src = format!("{CSUM}int sh(int x) {{ return x << csum(7); }}\n");
    let d = tu("imm-shift", &src);
    let err = ok(&d, &["-S", "-emit-llvm", "-o", "t.ll", "-fconstexpr-stats", "t.cpp"]);
    assert_eq!(stat_n(&err, "evaluations", "immediate"), 1, "{err}");
    assert_eq!(stat_n(&err, "evaluations", "count"), 1, "{err}");
    assert_eq!(stat_n(&err, "trials", "steps"), 0, "{err}");
}

/// Ties are broken by count, then location, then name; the report is
/// the same on every run (bar its times).
#[test]
fn ranking_is_deterministic() {
    let d = tu(
        "rank",
        "constexpr int b() { return 1; }\nconstexpr int a() { return 1; }\n\
         constexpr int c() { return a() + b(); }\n\
         static_assert(a() == 1);\nstatic_assert(b() == 1);\nstatic_assert(c() == 2);\n",
    );
    let strip = |p: Vec<Json>| -> Vec<String> {
        p.iter()
            .map(|j| match j {
                Json::Obj(m) => m
                    .iter()
                    .filter(|(k, _)| !k.ends_with("_us"))
                    .map(|(k, v)| format!("{k}={v:?}"))
                    .collect::<Vec<_>>()
                    .join(","),
                _ => unreachable!(),
            })
            .collect()
    };
    let p = profile(&d);
    let names: Vec<&str> = p.iter().filter_map(|j| j.get("function")?.as_str()).collect();
    // `a` and `b` tie on steps and calls (2 each): `b` is declared first.
    assert_eq!(names, ["b", "a", "c"]);
    let at: Vec<&str> = sites(&p).iter().map(|j| s(j, "loc")).collect();
    assert_eq!(at, ["t.cpp:6:1", "t.cpp:4:1", "t.cpp:5:1"]);
    let first = strip(p);
    for _ in 0..3 {
        assert_eq!(strip(profile(&d)), first);
    }
}

/// The text report: header, columns, the limit and its "more" line, the
/// total; `=text` is the default; a file name not ending in `.json`
/// gets text, one ending in it NDJSON; nothing then goes to stderr.
#[test]
fn text_report_limit_and_files() {
    let d = tu(
        "text",
        &format!("{DOWN}{SUM}static_assert(down(3) == 3);\nconstexpr int b = sum(4);\n"),
    );
    let err =
        ok(&d, &["-fsyntax-only", "-fconstexpr-profile", "-fconstexpr-profile-limit=1", "t.cpp"]);
    let lines: Vec<&str> = err.lines().collect();
    assert_eq!(lines[0], "===-- kiln constexpr profile: t.cpp --===");
    assert_eq!(lines[1], "functions (top 1 of 2, by inclusive steps):");
    assert!(lines[2].trim_start().starts_with("calls    memo   incl steps   self steps"), "{err}");
    assert!(lines[3].ends_with("  sum   (t.cpp:2:15)"), "{err}");
    assert_eq!(lines[4], "  ... 1 more rows (use -fconstexpr-profile-limit=0)");
    assert_eq!(lines[5], "evaluation sites (top 1 of 2, by inclusive steps):");
    assert!(lines.last().unwrap().starts_with("  total: 2 evaluations, "), "{err}");
    let all = ok(
        &d,
        &["-fsyntax-only", "-fconstexpr-profile=text", "-fconstexpr-profile-limit=0", "t.cpp"],
    );
    assert!(all.contains("functions (top 2 of 2, by inclusive steps):"), "{all}");
    assert!(!all.contains("more rows"), "{all}");

    assert_eq!(ok(&d, &["-fsyntax-only", "-fconstexpr-profile=p.txt", "t.cpp"]), "");
    let txt = std::fs::read_to_string(d.join("p.txt")).unwrap();
    assert!(txt.starts_with("===-- kiln constexpr profile: t.cpp --===\n"), "{txt}");
    assert_eq!(ok(&d, &["-fsyntax-only", "-fconstexpr-profile=p.json", "t.cpp"]), "");
    let p = ndjson(&std::fs::read_to_string(d.join("p.json")).unwrap());
    assert_eq!(s(total(&p), "file"), "t.cpp");

    let o = kiln(&d, &["-fsyntax-only", "-fconstexpr-profile=no/such/dir/p.json", "t.cpp"]);
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.contains("error: unable to open output file 'no/such/dir/p.json'"),
        "{}",
        o.stderr
    );
}

/// Every summary line is `constexpr: <what> key=value...`, in a fixed
/// order; the profile is written before it.
#[test]
fn stats_lines_are_stable() {
    let d = tu("stats", &format!("{DOWN}static_assert(down(3) == 3);\n"));
    let err = ok(&d, &["-fsyntax-only", "-fconstexpr-stats", "t.cpp"]);
    let heads: Vec<&str> = err.lines().map(|l| l.split(' ').take(2).last().unwrap()).collect();
    assert_eq!(
        heads,
        ["evaluations", "steps", "calls", "memory", "interned", "meta-ops", "trials", "compile"]
    );
    assert!(err.lines().all(|l| l.starts_with("constexpr: ")), "{err}");
    let keys: Vec<String> = stat(&err, "calls").into_iter().map(|(k, _)| k).collect();
    assert_eq!(keys, ["count", "memo-hits", "memo-misses", "memo-hit-rate"]);
    assert_eq!(stat_n(&err, "steps", "count"), 8);
    assert_eq!(stat_n(&err, "calls", "count"), 4);
    let both = ok(&d, &["-fsyntax-only", "-fconstexpr-stats", "-fconstexpr-profile", "t.cpp"]);
    let at = |t: &str| both.find(t).unwrap();
    assert!(at("===-- kiln constexpr profile:") < at("constexpr: evaluations"), "{both}");
    // Code generation reports too.
    let cg = ok(&d, &["-S", "-emit-llvm", "-o", "t.ll", "-fconstexpr-stats", "t.cpp"]);
    assert_eq!(stat_n(&cg, "steps", "count"), 8, "{cg}");
}

/// Without the flags nothing is printed, and `-fconstexpr-report` is the
/// same with and without the profile (its counters do not disturb the
/// report's).
#[test]
fn flags_off_print_nothing() {
    let d = tu(
        "off",
        &format!(
            "{FIB}{DOWN}{SUM}constexpr int k = fib(12);\nstatic_assert(down(5) == 5);\nconstexpr int b = sum(9);\n"
        ),
    );
    assert_eq!(ok(&d, &["-fsyntax-only", "t.cpp"]), "");
    assert_eq!(ok(&d, &["-S", "-emit-llvm", "-o", "t.ll", "t.cpp"]), "");
    let report = |extra: &[&str]| {
        let mut a = vec!["-fsyntax-only", "-fconstexpr-report=r.json", "t.cpp"];
        a.extend_from_slice(extra);
        ok(&d, &a);
        let r = std::fs::read_to_string(d.join("r.json")).unwrap();
        ndjson(&r);
        // Everything but the times (`"wall_us":N`, also inside `total`).
        let mut out = String::new();
        for part in r.split("\"wall_us\":") {
            out.push_str(part.trim_start_matches(|c: char| c.is_ascii_digit()));
        }
        out
    };
    let off = report(&[]);
    assert_eq!(report(&["-fconstexpr-profile=p.json", "-fconstexpr-stats"]), off);
}

/// An importer of a module ([module.import]) reports only its own
/// evaluations: the module's were paid when its BMI was built.
#[test]
fn importer_reports_its_own_evaluations() {
    let d = scratch("modules");
    std::fs::write(
        d.join("m.cppm"),
        "export module m;\nexport constexpr int tri(int n) { return n == 0 ? 0 : n + tri(n - 1); }\n\
         export constexpr int mk = tri(5);\nstatic_assert(mk == 15);\n",
    )
    .unwrap();
    std::fs::write(
        d.join("u.cpp"),
        "import m;\nstatic_assert(tri(3) == 6);\nint main() { return mk; }\n",
    )
    .unwrap();
    let pre = ok(&d, &["--precompile", "m.cppm", "-o", "m.pcm", "-fconstexpr-profile=json"]);
    let p = ndjson(&pre);
    assert_eq!(n(total(&p), "evaluations"), 2);
    assert_eq!(n(func(&p, "tri"), "calls"), 6);
    let p = ndjson(&ok(
        &d,
        &["-fsyntax-only", "-fmodule-file=m=m.pcm", "-fconstexpr-profile=json", "u.cpp"],
    ));
    let at: Vec<&str> = sites(&p).iter().map(|j| s(j, "loc")).collect();
    assert_eq!(at, ["u.cpp:2:1"]);
    assert_eq!(n(func(&p, "tri"), "calls"), 4);
    assert_eq!(s(func(&p, "tri"), "loc"), "m.cppm:2:22");
    assert_eq!((n(total(&p), "evaluations"), s(total(&p), "file")), (1, "u.cpp"));
}
