//! WP-1586: a kind 2 row must not depend on anything else in its unit
//! (spec/abi.md "Definition hash": no byte of a row depends on unit-wide
//! state; mortar's must-equal corpus O-13). The definitions of
//! `tests/mason/context/defs.h` (one of each row kind and one per area of
//! the hash) are compiled once as they are and once behind each preamble
//! of `tests/mason/context/preambles/`, before the `#include`, after it (the
//! trailer side) or around it, as the preamble's `// sides:` line says.
//! Every key present in both units has the same hash and flags, except the
//! symbols a preamble's `// expect: differ` line names (a redeclaration that
//! the standard lets change the definition: a must-differ case), which
//! must differ. `scripts/mason/odr_preamble.py` runs the same preambles over
//! arbitrary units (spec/harness.md "Context independence of kind 2 rows").

mod mason_common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mason_common::*;

fn context_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/mason/context")
}

/// A preamble file: its sides, whether it names `defs.h`'s entities, the
/// symbols it must make differ, and its text.
struct Preamble {
    name: String,
    sides: Vec<String>,
    differ: Vec<String>,
    text: String,
}

fn preambles() -> Vec<Preamble> {
    let dir = context_dir().join("preambles");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| Path::new(n).extension().is_some_and(|e| e == "h"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let text = std::fs::read_to_string(dir.join(&name)).unwrap();
            let mut p = Preamble {
                name,
                sides: vec!["pre".into(), "post".into()],
                differ: Vec::new(),
                text: text.clone(),
            };
            for line in text.lines().take_while(|l| l.starts_with("// ")) {
                let (k, v) = line[3..].split_once(':').unwrap_or(("", ""));
                match k {
                    "sides" => p.sides = v.split_whitespace().map(String::from).collect(),
                    "expect" => {
                        let mut w = v.split_whitespace();
                        if w.next() == Some("differ") {
                            p.differ = w.map(String::from).collect();
                        }
                    }
                    _ => {}
                }
            }
            p
        })
        .collect()
}

/// The source of the unit with `pre` placed on `side`.
fn placed(side: &str, pre: &str, unit: &str) -> String {
    match side {
        "pre" => format!("{pre}\n{unit}"),
        "post" => format!("{unit}\n{pre}\n"),
        "wrap" => {
            let (before, after) = pre.split_once("//@ include\n").expect("a wrap preamble");
            format!("{before}{unit}\n{after}")
        }
        s => panic!("unknown side {s}"),
    }
}

/// The kind 2 rows of `src` by symbol (key resolved through the IR's
/// definitions).
fn rows_by_symbol(d: &Path, file: &str, src: &str, args: &[&str]) -> BTreeMap<String, Def> {
    let ir = ir_of(d, file, src, args);
    let rows = def_rows(&ir_bytes(meta_line(&ir).expect("the global"))).expect("kind 2 rows");
    let mut out = BTreeMap::new();
    for s in defined(&ir) {
        if let Some(r) = row(&rows, &s) {
            out.insert(s, r.clone());
        }
    }
    assert_eq!(out.len(), rows.len(), "{file}: every row's symbol is defined");
    out
}

/// Compares the rows of the plain unit with each preamble's, for one
/// standard and argument list; returns the failures as text.
fn compare(d: &Path, std: &str, extra: &[&str], only: Option<&str>) -> Vec<String> {
    let inc = context_dir();
    let inc = inc.to_str().unwrap();
    // `-w`: a preamble may deprecate an entity the unit uses.
    let mut args = vec![std, "-w", "-I", inc];
    args.extend(extra);
    let unit = "#include \"defs.h\"\nint anchor() { return use_all(); }\n";
    let plain = rows_by_symbol(d, "plain.cpp", unit, &args);
    assert!(plain.len() > 60, "{std}: {} rows", plain.len());
    for (s, r) in &plain {
        assert_eq!(r.flags & PARTIAL, 0, "{std}: {s} is PARTIAL");
    }
    let mut failures = Vec::new();
    for p in preambles() {
        if only.is_some_and(|o| o != p.name) {
            continue;
        }
        for side in &p.sides {
            let file = format!("{}-{side}.cpp", p.name.trim_end_matches(".h"));
            let other = rows_by_symbol(d, &file, &placed(side, &p.text, unit), &args);
            for (s, r) in &plain {
                let Some(o) = other.get(s) else { continue };
                let differs = (r.hash, r.flags) != (o.hash, o.flags);
                let expected = p.differ.iter().any(|x| x == s);
                if differs != expected {
                    failures.push(format!(
                        "{std} {extra:?} preamble {} ({side}): {s} {}: {:032x} f{} / {:032x} f{}",
                        p.name,
                        if differs { "differs" } else { "does not differ (expected to)" },
                        r.hash,
                        r.flags,
                        o.hash,
                        o.flags
                    ));
                }
            }
            for s in &p.differ {
                if !other.contains_key(s) || !plain.contains_key(s) {
                    failures.push(format!("{std} preamble {} ({side}): no row for {s}", p.name));
                }
            }
        }
    }
    failures
}

/// Every preamble, both sides, at C++20 and C++26.
#[test]
fn rows_do_not_depend_on_the_rest_of_the_unit() {
    let d = scratch("context");
    let mut failures = compare(&d, "-std=c++26", &[], None);
    failures.extend(compare(&d, "-std=c++20", &[], None));
    assert!(failures.is_empty(), "context-dependent rows:\n{}", failures.join("\n"));
}

/// The standard-header preamble with the other standard library.
#[test]
fn rows_do_not_depend_on_the_standard_library_included_before() {
    let d = scratch("context-libcxx");
    std::fs::write(d.join("probe.cpp"), "#include <vector>\n").unwrap();
    if !run(&d, &["-std=c++20", "-stdlib=libc++", "-fsyntax-only", "probe.cpp"]).status.success() {
        eprintln!("no libc++: skipped");
        return;
    }
    let failures = compare(&d, "-std=c++20", &["-stdlib=libc++"], Some("std-headers.h"));
    assert!(failures.is_empty(), "context-dependent rows:\n{}", failures.join("\n"));
}

/// The rows of `use_` after `header` reached by each route: `#include`,
/// a PCH of `header`, `import` of it as a header unit.
fn routes(d: &Path, header: &str, use_: &str, args: &[&str]) -> [Vec<Def>; 3] {
    let mut a: Vec<&str> = vec!["-w", "-I", "."];
    a.extend(args);
    let mut pch = a.clone();
    pch.extend(["-x", "c++-header", header, "-o", "h.pch"]);
    ok(d, &pch);
    let inc = format!("#include \"{header}\"\n{use_}");
    let textual = defs_of(d, "t.cpp", &inc, &a).expect("textual");
    let mut p = a.clone();
    p.extend(["-include-pch", "h.pch"]);
    let with_pch = defs_of(d, "p.cpp", &inc, &p).expect("PCH");
    let hu =
        defs_of(d, "i.cpp", &format!("import \"{header}\";\n{use_}"), &a).expect("header unit");
    [textual, with_pch, hu]
}

/// Must-equal across routes (mortar's ruling b): every definition kind of
/// `defs.h` through `#include`, a PCH and a header unit gives the same
/// rows (keys, hashes, flags, places).
#[test]
fn three_routes_one_hash() {
    let d = scratch("context-routes");
    std::fs::copy(context_dir().join("defs.h"), d.join("defs.h")).unwrap();
    let use_ = "int anchor() { return use_all(); }\n";
    for std in ["-std=c++20", "-std=c++26"] {
        let [t, p, h] = routes(&d, "defs.h", use_, &[std]);
        assert!(t.len() > 60 && t.iter().all(|r| r.file.ends_with("defs.h")), "{std}: {t:#?}");
        assert_eq!(t, p, "{std}: textual vs PCH");
        assert_eq!(t, h, "{std}: textual vs header unit");
    }
}

/// The same for `<vector>`, `<string>` and `<map>` of each installed
/// standard library, and through `import <vector>;` etc.
#[test]
fn three_routes_one_hash_standard_library() {
    let d = scratch("context-routes-std");
    std::fs::write(d.join("s.h"), "#include <vector>\n#include <string>\n#include <map>\n")
        .unwrap();
    let use_ = "int anchor() {\n  std::vector<int> v{1, 2, 3};\n  v.push_back(4);\n  \
                std::string s = \"abc\";\n  s += std::to_string(v.size());\n  \
                std::map<std::string, int> m;\n  m[s] = 1;\n  m.emplace(\"x\", 2);\n  \
                for (auto &[k, x] : m) v.push_back(x + (int)k.size());\n  \
                return (int)v.size() + m.count(\"x\");\n}\n";
    std::fs::write(d.join("probe.cpp"), "#include <vector>\n").unwrap();
    for lib in ["-stdlib=libstdc++", "-stdlib=libc++"] {
        if !run(&d, &["-std=c++20", lib, "-fsyntax-only", "probe.cpp"]).status.success() {
            eprintln!("{lib} not installed: skipped");
            continue;
        }
        let args = ["-std=c++20", lib];
        let [text, pch, unit] = routes(&d, "s.h", use_, &args);
        assert!(
            text.len() > 150 && text.iter().all(|r| r.flags & SYSTEM != 0),
            "{lib}: {}",
            text.len()
        );
        assert_eq!(text, pch, "{lib}: textual vs PCH");
        assert_eq!(text, unit, "{lib}: textual vs header unit");
        let mut a = vec!["-w"];
        a.extend(args);
        let angled = format!("import <vector>;\nimport <string>;\nimport <map>;\n{use_}");
        assert_eq!(text, defs_of(&d, "a.cpp", &angled, &a).unwrap(), "{lib}: angled header units");
    }
}
