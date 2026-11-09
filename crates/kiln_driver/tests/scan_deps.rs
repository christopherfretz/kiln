//! WP-982: `kiln-scan-deps`, the P1689r5 dependency scanner (spec/modules.md
//! §11.9, spec/driver.md "Dependency scanning"), run the way `CMake` runs it:
//! `<scanner> -format=p1689 -- <compiler> <flags> -c <src> -o <obj> -MT
//! <ddi> -MD -MF <dep>`. Every expected document below is byte for byte
//! what clang-scan-deps 22 prints for the same input ([module.unit],
//! [module.import]).
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn bin(name: &str) -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name(name)
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-scan-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Runs `<scanner> <opts> -- kiln++ <cmd>` in `dir`.
fn scan_with(scanner: &str, dir: &Path, opts: &[&str], cmd: Option<&[&str]>) -> Out {
    let mut c = Command::new(bin(scanner));
    c.args(opts);
    if let Some(cmd) = cmd {
        c.arg("--").arg(bin("kiln++")).arg("-std=c++20").args(cmd);
    }
    let o = c.current_dir(dir).stdin(Stdio::null()).output().unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "{scanner} {opts:?} {cmd:?} panicked:\n{stderr}");
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr,
    }
}

fn p1689(dir: &Path, cmd: &[&str]) -> Out {
    scan_with("clang-scan-deps-22", dir, &["-format=p1689"], Some(cmd))
}

/// A directory holding the test module sources.
fn sources(name: &str) -> PathBuf {
    let d = scratch(name);
    let files: &[(&str, &str)] = &[
        ("hdr.h", "#define HDR 1\n"),
        (
            "m.cppm",
            "module;\n#include \"hdr.h\"\nexport module m;\nexport import :p;\nimport :impl;\n",
        ),
        ("p.cppm", "export module m:p;\nexport int g() { return 2; }\n"),
        ("impl.cppm", "module m:impl;\nint h() { return 3; }\n"),
        ("m.cpp", "module m;\n#include \"hdr.h\"\nint f() { return g() + h() + HDR; }\n"),
        ("main.cpp", "import m;\nimport m;\nint main() { return f(); }\n"),
        ("plain.cpp", "int main() { return 0; }\n"),
        ("hu.cpp", "import \"hdr.h\";\nint x = HDR;\n"),
        ("e.cppm", "export module e;\n#error not seen by the scanner\nimport x.y;\n"),
    ];
    for (n, s) in files {
        std::fs::write(d.join(n), s).unwrap();
    }
    d
}

const EMPTY: &str = "{\n  \"revision\": 0,\n  \"rules\": [],\n  \"version\": 1\n}\n";

fn doc(rule: &str) -> String {
    format!(
        "{{\n  \"revision\": 0,\n  \"rules\": [\n    {{\n{rule}    }}\n  ],\n  \"version\": 1\n}}\n"
    )
}

fn provides(name: &str, interface: bool, src: &str) -> String {
    format!(
        "      \"provides\": [\n        {{\n          \"is-interface\": {interface},\n          \
         \"logical-name\": \"{name}\",\n          \"source-path\": \"{src}\"\n        }}\n      ]"
    )
}

fn requires(names: &[&str]) -> String {
    let items: Vec<String> = names
        .iter()
        .map(|n| format!("        {{\n          \"logical-name\": \"{n}\"\n        }}"))
        .collect();
    format!("      \"requires\": [\n{}\n      ]", items.join(",\n"))
}

/// [module.unit]p1-3: a primary module interface unit provides its name and
/// requires the partitions it imports, in source order, as `m:p`.
#[test]
fn interface_unit_requires_its_partitions() {
    let d = sources("interface");
    let o = p1689(&d, &["-c", "m.cppm", "-o", "m.o", "-MT", "m.ddi", "-MD", "-MF", "m.ddi.d"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let want = doc(&format!(
        "      \"primary-output\": \"m.o\",\n{},\n{}\n",
        provides("m", true, "m.cppm"),
        requires(&["m:p", "m:impl"])
    ));
    assert_eq!(o.stdout, want);
    // The depfile names the ddi target and every file the scan read.
    let dep = std::fs::read_to_string(d.join("m.ddi.d")).unwrap();
    assert!(dep.starts_with("m.ddi:"), "{dep}");
    assert!(dep.contains("m.cppm") && dep.contains("hdr.h"), "{dep}");
}

/// [module.unit]p3: an interface partition is an interface; an
/// implementation partition (`module m:impl;`) is not.
#[test]
fn partitions_report_is_interface() {
    let d = sources("partitions");
    let o = p1689(&d, &["-c", "p.cppm", "-o", "p.o"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(
        o.stdout,
        doc(&format!("      \"primary-output\": \"p.o\",\n{}\n", provides("m:p", true, "p.cppm")))
    );
    let o = p1689(&d, &["-c", "impl.cppm", "-o", "impl.o"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(
        o.stdout,
        doc(&format!(
            "      \"primary-output\": \"impl.o\",\n{}\n",
            provides("m:impl", false, "impl.cppm")
        ))
    );
}

/// [module.unit]p8: an implementation unit provides nothing and implicitly
/// imports its primary interface.
#[test]
fn implementation_unit_requires_its_module() {
    let d = sources("impl-unit");
    let o = p1689(&d, &["-c", "m.cpp", "-o", "m.cpp.o"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(
        o.stdout,
        doc(&format!("      \"primary-output\": \"m.cpp.o\",\n{}\n", requires(&["m"])))
    );
}

/// [module.import]: a consumer requires each module once; a TU with no
/// module lines has a rule with only its output, `<stem>.o` without `-o`.
#[test]
fn consumers_and_plain_units() {
    let d = sources("consumer");
    let o = p1689(&d, &["-c", "main.cpp", "-o", "main.o"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(
        o.stdout,
        doc(&format!("      \"primary-output\": \"main.o\",\n{}\n", requires(&["m"])))
    );
    let o = p1689(&d, &["-c", "plain.cpp"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(o.stdout, doc("      \"primary-output\": \"plain.o\"\n"));
}

/// clang-scan-deps's source minimizer drops `#error` and `#warning`; so
/// does kiln's scan, and the scan still sees the import after it.
#[test]
fn error_directive_is_not_a_scan_failure() {
    let d = sources("error");
    let o = p1689(&d, &["-c", "e.cppm", "-o", "e.o"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(
        o.stdout,
        doc(&format!(
            "      \"primary-output\": \"e.o\",\n{},\n{}\n",
            provides("e", true, "e.cppm"),
            requires(&["x.y"])
        ))
    );
}

/// [module.import]p5: header units are WP-983; the scan fails the way
/// clang-scan-deps fails (header line, no caret, empty document, rc 1, no
/// depfile).
#[test]
fn header_unit_import_is_a_clear_error() {
    let d = sources("header-unit");
    let o = p1689(&d, &["-c", "hu.cpp", "-o", "hu.o", "-MT", "hu.ddi", "-MD", "-MF", "hu.ddi.d"]);
    assert_eq!(o.code, 1);
    assert_eq!(o.stdout, EMPTY);
    assert_eq!(
        o.stderr,
        "Diagnostics while scanning dependencies for 'hu.cpp':\nhu.cpp:1:8: error: header unit \"hdr.h\" \
         cannot be imported: kiln-scan-deps does not support header units yet\n"
    );
    assert!(!d.join("hu.ddi.d").exists());
}

/// A missing input is a scan failure with the driver's diagnostic.
#[test]
fn missing_input() {
    let d = sources("missing");
    let o = p1689(&d, &["-c", "nofile.cppm", "-o", "n.o"]);
    assert_eq!(o.code, 1);
    assert_eq!(o.stdout, EMPTY);
    assert!(
        o.stderr.starts_with("Diagnostics while scanning dependencies for 'nofile.cppm':\n"),
        "{}",
        o.stderr
    );
    assert!(o.stderr.contains("no such file or directory: 'nofile.cppm'"), "{}", o.stderr);
}

/// The scanner's own options: `-o` writes the document to a file, a
/// missing `--` and an unknown format are clang-scan-deps's messages, and
/// every name (`kiln-scan-deps`, `clang-scan-deps`, `clang-scan-deps-22`)
/// is the same scanner.
#[test]
fn scanner_options_and_names() {
    let d = sources("options");
    let o = scan_with(
        "kiln-scan-deps",
        &d,
        &["-format=p1689", "-o", "out.json"],
        Some(&["-c", "p.cppm", "-o", "p.o"]),
    );
    assert_eq!((o.code, o.stdout.as_str()), (0, ""), "{}", o.stderr);
    let file = std::fs::read_to_string(d.join("out.json")).unwrap();
    assert_eq!(
        file,
        doc(&format!("      \"primary-output\": \"p.o\",\n{}\n", provides("m:p", true, "p.cppm")))
    );

    let o = scan_with("clang-scan-deps", &d, &["-format=p1689"], None);
    assert_eq!(o.code, 1);
    assert_eq!(
        o.stderr,
        "The compilation command line must be provided either via '-compilation-database' or after '--'.\n"
    );
    let o = scan_with("clang-scan-deps-22", &d, &["-format=bogus"], Some(&["-c", "p.cppm"]));
    assert_eq!(o.code, 1);
    assert_eq!(
        o.stderr,
        "clang-scan-deps-22: for the --format option: Cannot find option named 'bogus'\n"
    );
    // The default format is make: a rule for the object.
    let o = scan_with("kiln-scan-deps", &d, &[], Some(&["-c", "p.cppm", "-o", "p.o"]));
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.starts_with("p.o: p.cppm"), "{}", o.stdout);
}

// ---- WP-1148: `-compilation-database`, `-format=experimental-full`, the
// `clang-scan-deps-19` name, and a differential run against the real
// clang-scan-deps 19 and 22 when they are installed.

/// A JSON string literal.
fn js(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `sources` plus `sub/s.cppm` (an interface unit in another directory
/// that includes `../hdr.h`).
fn db_sources(name: &str) -> PathBuf {
    let d = sources(name);
    std::fs::create_dir_all(d.join("sub")).unwrap();
    std::fs::write(d.join("sub/s.cppm"), "module;\n#include \"../hdr.h\"\nexport module s;\n")
        .unwrap();
    d.canonicalize().unwrap()
}

/// Writes `compile_commands.json` from `(directory, file, words-or-command,
/// output)` entries: `Ok(words)` is an `arguments` array, `Err(text)` a
/// `command` string.
type DbEntry<'a> = (&'a Path, &'a str, Result<&'a [&'a str], &'a str>, Option<&'a str>);

fn write_db(d: &Path, entries: &[DbEntry<'_>]) -> PathBuf {
    use std::fmt::Write as _;
    let items: Vec<String> = entries
        .iter()
        .map(|(dir, file, cmd, out)| {
            let mut s =
                format!("{{\"directory\": {}, \"file\": {}", js(&dir.to_string_lossy()), js(file));
            match cmd {
                Ok(words) => {
                    let w: Vec<String> = words.iter().map(|w| js(w)).collect();
                    write!(s, ", \"arguments\": [{}]", w.join(", ")).unwrap();
                }
                Err(text) => write!(s, ", \"command\": {}", js(text)).unwrap(),
            }
            if let Some(o) = out {
                write!(s, ", \"output\": {}", js(o)).unwrap();
            }
            s + "}"
        })
        .collect();
    let path = d.join("compile_commands.json");
    std::fs::write(&path, format!("[\n{}\n]\n", items.join(",\n"))).unwrap();
    path
}

fn rule(primary: &str, rest: &[String]) -> String {
    let mut s = format!("    {{\n      \"primary-output\": \"{primary}\"");
    for r in rest {
        s.push_str(",\n");
        s.push_str(r);
    }
    s + "\n    }"
}

/// `-compilation-database`: every entry is scanned in its own
/// `directory`; `arguments` wins over `command`, which is split as a
/// shell would; a rule's primary output is the entry's `output` (empty
/// without one); the rules are sorted by it; and an import of a module
/// another entry provides names that entry's source (clang-scan-deps
/// 22's P1689 output for the same database, [module.unit],
/// [module.import]).
#[test]
fn compilation_database_p1689() {
    let d = db_sources("cdb-p1689");
    let kpp = bin("kiln++");
    let k = kpp.to_str().unwrap();
    let main_cmd = format!("{k} -std=c++20 -c 'main.cpp' -o \"main.o\" -DX=\\\"q\\\"");
    let sub_cmd = format!("{k} -std=c++20 -c s.cppm -o s.o");
    let db = write_db(
        &d,
        &[
            (&d, "m.cppm", Ok(&[k, "-std=c++20", "-c", "m.cppm", "-o", "m.o"]), None),
            (&d, "main.cpp", Err(&main_cmd), Some("main.o")),
            (&d.join("sub"), "s.cppm", Err(&sub_cmd), Some("s.o")),
            (&d, "p.cppm", Ok(&[k, "-std=c++20", "-c", "p.cppm"]), Some("p.o")),
        ],
    );
    let o = scan_with(
        "clang-scan-deps-22",
        &d,
        &["-format=p1689", "-compilation-database", db.to_str().unwrap()],
        None,
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    let req = |n: &str, src: Option<&str>| match src {
        Some(s) => format!(
            "        {{\n          \"logical-name\": \"{n}\",\n          \"source-path\": \"{s}\"\n        }}"
        ),
        None => format!("        {{\n          \"logical-name\": \"{n}\"\n        }}"),
    };
    let requires =
        |items: &[String]| format!("      \"requires\": [\n{}\n      ]", items.join(",\n"));
    let rules = [
        rule(
            "",
            &[
                provides("m", true, "m.cppm"),
                requires(&[req("m:p", Some("p.cppm")), req("m:impl", None)]),
            ],
        ),
        // kiln lists `import m; import m;` once (clang twice).
        rule("main.o", &[requires(&[req("m", Some("m.cppm"))])]),
        rule("p.o", &[provides("m:p", true, "p.cppm")]),
        rule("s.o", &[provides("s", true, "s.cppm")]),
    ];
    let want = format!(
        "{{\n  \"revision\": 0,\n  \"rules\": [\n{}\n  ],\n  \"version\": 1\n}}\n",
        rules.join(",\n")
    );
    assert_eq!(o.stdout, want);

    // An empty database is the empty document.
    std::fs::write(d.join("empty.json"), "[]\n").unwrap();
    let o = scan_with(
        "kiln-scan-deps",
        &d,
        &["-format=p1689", "-compilation-database=empty.json"],
        None,
    );
    assert_eq!((o.code, o.stdout.as_str(), o.stderr.as_str()), (0, EMPTY, ""));
}

/// The make format of a database: one rule per entry in database order,
/// each entry's files named from its `directory`; a failing entry is
/// reported under its `file` name and leaves the others (exit 1).
#[test]
fn compilation_database_make() {
    let d = db_sources("cdb-make");
    let kpp = bin("kiln++");
    let k = kpp.to_str().unwrap();
    let db = write_db(
        &d,
        &[
            (&d.join("sub"), "s.cppm", Ok(&[k, "-std=c++20", "-c", "s.cppm", "-o", "s.o"]), None),
            (&d, "missing.cpp", Ok(&[k, "-c", "missing.cpp"]), None),
            (&d, "m.cppm", Ok(&[k, "-std=c++20", "-c", "m.cppm"]), None),
        ],
    );
    let o = scan_with("kiln-scan-deps", &d, &["-compilation-database", db.to_str().unwrap()], None);
    assert_eq!(o.code, 1);
    let ds = d.to_str().unwrap();
    // (Long rules wrap at clang's column limit; compare unwrapped.)
    assert_eq!(
        o.stdout.replace(" \\\n  ", " "),
        format!("s.o: {ds}/sub/s.cppm {ds}/sub/../hdr.h\nm.o: {ds}/m.cppm {ds}/hdr.h\n")
    );
    assert!(
        o.stderr.starts_with(
            "Diagnostics while scanning dependencies for 'missing.cpp':\nerror: no such file or directory: 'missing.cpp'\n"
        ),
        "{}",
        o.stderr
    );
    // Under clang-scan-deps-19 the header is clang 19's.
    let o =
        scan_with("clang-scan-deps-19", &d, &["-compilation-database", db.to_str().unwrap()], None);
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.starts_with("Error while scanning dependencies for missing.cpp:\n"),
        "{}",
        o.stderr
    );
}

/// The database errors are clang-scan-deps's messages (exit 1, nothing
/// printed); a database and a command after `--` together are an error.
#[test]
fn compilation_database_errors() {
    let d = sources("cdb-errors");
    let cases: &[(&str, &str)] = &[
        ("", "Expected array."),
        ("{}", "Expected array."),
        ("[3]", "Expected object."),
        (r#"[{"directory": "."}]"#, "Missing key: \"file\"."),
        (r#"[{"file": "a"}]"#, "Missing key: \"command\" or \"arguments\"."),
        (r#"[{"file": "a", "command": "x"}]"#, "Missing key: \"directory\"."),
        (r#"[{"directory": ".", "file": "a", "arguments": "x"}]"#, "Expected sequence as value."),
        (r#"[{"directory": ".", "file": "a", "command": ["x"]}]"#, "Expected string as value."),
        (
            r#"[{"directory": ".", "file": "a", "command": "x", "zork": 1}]"#,
            "Unknown key: \"\"zork\"\"",
        ),
        ("[{\"directory\" 1}]", "bad.json:1:15: error: expected ':'"),
    ];
    for (text, msg) in cases {
        std::fs::write(d.join("bad.json"), text).unwrap();
        let o = scan_with("kiln-scan-deps", &d, &["-compilation-database", "bad.json"], None);
        assert_eq!((o.code, o.stdout.as_str()), (1, ""), "{text}");
        assert_eq!(o.stderr, format!("{msg}\n"), "{text}");
    }
    let o = scan_with("kiln-scan-deps", &d, &["-compilation-database", "nope.json"], None);
    assert_eq!(o.code, 1);
    assert_eq!(o.stderr, "Error while opening JSON database: No such file or directory\n");
    let o = scan_with(
        "kiln-scan-deps",
        &d,
        &["-compilation-database", "bad.json"],
        Some(&["-c", "p.cppm"]),
    );
    assert_eq!(o.code, 1);
    assert_eq!(
        o.stderr,
        "The compilation command line must be provided either via '-compilation-database' or after '--'.\n"
    );
}

/// `-format=experimental-full`: one object per translation unit with its
/// command line, compiler, absolute file deps, and (clang 22) the named
/// module it provides and the ones it imports; the shapes of clang 22
/// (streamed keys, no final newline) and clang 19 (sorted keys, no named
/// modules, final newline) under their program names.
#[test]
fn experimental_full_shapes() {
    let d = sources("full").canonicalize().unwrap();
    let ds = d.to_str().unwrap();
    let kpp = bin("kiln++");
    let k = kpp.to_str().unwrap();
    let run = |scanner: &str| {
        let o = scan_with(
            scanner,
            &d,
            &["-format=experimental-full"],
            Some(&["-c", "m.cppm", "-o", "m.o"]),
        );
        assert_eq!(o.code, 0, "{}", o.stderr);
        // The hash is kiln's own; check its form and blank it.
        let at = o.stdout.find("\"clang-context-hash\": \"").unwrap() + 23;
        let len = o.stdout[at..].find('"').unwrap();
        assert!(
            o.stdout[at..at + len].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        );
        format!("{}H{}", &o.stdout[..at], &o.stdout[at + len..])
    };
    let head = "{\n  \"modules\": [],\n  \"translation-units\": [\n    {\n      \"commands\": [\n        {\n          \"clang-context-hash\": \"H\",\n";
    let cmd = format!(
        "          \"clang-module-deps\": [],\n          \"command-line\": [\n            \"-std=c++20\",\n            \
         \"-c\",\n            \"m.cppm\",\n            \"-o\",\n            \"m.o\"\n          ],\n          \
         \"executable\": \"{k}\",\n          \"file-deps\": [\n            \"{ds}/./m.cppm\",\n            \
         \"{ds}/./hdr.h\"\n          ],\n          \"input-file\": \"m.cppm\"\n        }}\n      ]\n    }}\n  ]\n}}"
    );
    let named = "          \"named-module\": \"m\",\n          \"named-module-deps\": [\n            \"m:p\",\n            \"m:impl\"\n          ],\n";
    assert_eq!(run("clang-scan-deps-22"), format!("{head}{named}{cmd}"));
    assert_eq!(run("clang-scan-deps-19"), format!("{head}{cmd}\n"));
    // A failed scan is a translation unit without commands.
    let o = scan_with(
        "kiln-scan-deps",
        &d,
        &["-format=experimental-full"],
        Some(&["-c", "nofile.cpp"]),
    );
    assert_eq!(o.code, 1);
    assert_eq!(
        o.stdout,
        "{\n  \"modules\": [],\n  \"translation-units\": [\n    {\n      \"commands\": []\n    }\n  ]\n}"
    );
}

/// Whether the real `clang-scan-deps-<v>` runs here (the differential
/// test is skipped without it: `apt-get install clang-tools-19`).
fn have(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// An `experimental-full` document reduced to what kiln matches: per
/// translation unit, its command objects with the `command-line` (clang's
/// is `-cc1`) and the hash blanked, equal consecutive commands merged
/// (clang 19 splits a module interface into two), trailing commas gone.
fn full_shape(text: &str) -> Vec<Vec<Vec<String>>> {
    let mut tus: Vec<Vec<Vec<String>>> = Vec::new();
    let mut block: Option<Vec<String>> = None;
    let mut skip = false;
    for line in text.lines() {
        let l = line.trim_end_matches(',');
        if l == "      \"commands\": [" || l == "      \"commands\": []" {
            tus.push(Vec::new());
        } else if l == "        {" {
            block = Some(Vec::new());
        } else if l == "        }" {
            let b = block.take().unwrap();
            let cmds = tus.last_mut().unwrap();
            if cmds.last() != Some(&b) {
                cmds.push(b);
            }
        } else if let Some(b) = &mut block {
            if l.starts_with("          \"command-line\": [") {
                skip = !l.ends_with("[]");
                b.push("command-line".into());
            } else if skip {
                skip = l != "          ]";
            } else if l.starts_with("          \"clang-context-hash\"") {
                b.push("hash".into());
            } else {
                b.push(l.to_owned());
            }
        }
    }
    tus
}

/// Differential: the same compilation database (a module graph across
/// two directories: interface, partitions, implementation unit,
/// consumer, plain unit) through the real clang-scan-deps 19 and 22 and
/// through kiln-scan-deps under the same names gives the same make
/// rules, the same P1689 document, and the same `experimental-full`
/// documents up to the command lines (clang's `-cc1` jobs, kiln's driver
/// command) and context hashes.
#[test]
fn differential_against_clang_scan_deps() {
    fn with<'a>(cxx: &[&'a str], src: &'a str, o: &[&'a str]) -> Vec<&'a str> {
        let mut v = cxx.to_vec();
        v.push(src);
        v.extend(o);
        v
    }
    let root = db_sources("differential");
    // No unit imports a module twice (kiln lists it once, clang twice).
    std::fs::write(root.join("main.cpp"), "import m;\nint main() { return f(); }\n").unwrap();
    let cxx: &[&str] = &["clang++", "-std=c++20", "-c"];
    let (iface, main, part, imp, mi, plain) = (
        with(cxx, "m.cppm", &["-o", "m.o"]),
        with(cxx, "main.cpp", &[]),
        with(cxx, "p.cppm", &["-o", "p.o"]),
        with(cxx, "impl.cppm", &["-o", "impl.o"]),
        with(cxx, "m.cpp", &[]),
        with(cxx, "plain.cpp", &["-o", "plain.o"]),
    );
    let s_cmd = "clang++ -std=c++20 -c s.cppm -o 's.o'";
    let db = write_db(
        &root,
        &[
            (&root, "m.cppm", Ok(&iface), None),
            (&root, "main.cpp", Ok(&main), Some("main.o")),
            (&root.join("sub"), "s.cppm", Err(s_cmd), Some("s.o")),
            (&root, "p.cppm", Ok(&part), Some("p.o")),
            (&root, "impl.cppm", Ok(&imp), Some("impl.o")),
            (&root, "m.cpp", Ok(&mi), Some("mi.o")),
            (&root, "plain.cpp", Ok(&plain), None),
        ],
    );
    let db = db.to_str().unwrap();
    let mut ran = 0;
    for v in ["19", "22"] {
        let real = format!("clang-scan-deps-{v}");
        if !have(&real) || !have("clang++") {
            eprintln!("note: {real} or clang++ not installed; skipping its differential");
            continue;
        }
        ran += 1;
        for format in ["make", "p1689", "experimental-full"] {
            let fmt = format!("-format={format}");
            let opts = ["-j", "1", fmt.as_str(), "-compilation-database", db];
            let clang = Command::new(&real).args(opts).current_dir(&root).output().unwrap();
            let kiln = scan_with(&real, &root, &opts, None);
            let c_out = String::from_utf8_lossy(&clang.stdout).into_owned();
            assert_eq!(
                (kiln.code, clang.status.code()),
                (0, Some(0)),
                "{real} {format}: {}",
                kiln.stderr
            );
            if format == "experimental-full" {
                let shape = full_shape(&kiln.stdout);
                assert!(shape.len() == 7 && shape.iter().all(|t| t.len() == 1), "{shape:?}");
                assert_eq!(shape, full_shape(&c_out), "{real} {format}");
                assert_eq!(kiln.stdout.ends_with('\n'), c_out.ends_with('\n'), "{real} {format}");
            } else if format == "make" && v == "19" {
                // clang 19 names an interface unit's rule by its temporary
                // `.pcm` job; compare the prerequisites.
                let deps = |t: &str| -> Vec<String> {
                    t.replace("\\\n", " ")
                        .lines()
                        .map(|l| {
                            l.split_once(": ")
                                .map_or(l, |x| x.1)
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .collect()
                };
                assert_eq!(deps(&kiln.stdout), deps(&c_out), "{real} {format}");
            } else {
                assert_eq!(kiln.stdout, c_out, "{real} {format}");
            }
        }
    }
    eprintln!("differential: {ran} clang-scan-deps versions compared");
}
