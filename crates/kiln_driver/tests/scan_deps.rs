//! WP-982: `kiln-scan-deps`, the P1689r5 dependency scanner (spec/modules.md
//! §11.9, spec/driver.md "Dependency scanning"), run the way CMake runs it:
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
