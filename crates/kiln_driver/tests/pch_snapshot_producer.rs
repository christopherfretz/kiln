//! WP-1393: the PCH snapshot producer's single pass (spec/snap.md "The
//! producer's single pass"). With `-fkiln-pch-snapshot=write`, the
//! snapshot pass stands in for the header pass when nothing a main file
//! alone gets could differ. Each header here is precompiled twice, with
//! the single pass allowed and with `KILN_PCH_SNAPSHOT_PASSES=2`: the
//! exit status, the diagnostics, the `-MD` file and the PCH's record must
//! be the same, and whether the single pass stood in must be as expected
//! (`KILN_SNAP_TIMING` says). A consumer then restores either PCH with the
//! same result. No C++ standard section applies: this is how the
//! compiler is built, not what it accepts ([cpp.include] is unchanged).
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-pchsnap-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Out {
    code: i32,
    stderr: String,
    /// The `snapshot-timing:` lines, taken out of `stderr`.
    timing: String,
}

fn kiln(dir: &Path, passes: &str, args: &[&str]) -> Out {
    let o = Command::new(kilnpp())
        .args(args)
        .current_dir(dir)
        .env("KILN_SNAP_TIMING", "1")
        .env("KILN_PCH_SNAPSHOT_PASSES", passes)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let all = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!all.contains("panicked"), "kiln++ {args:?} panicked:\n{all}");
    let (timing, rest): (Vec<&str>, Vec<&str>) =
        all.lines().partition(|l| l.starts_with("snapshot-timing:"));
    Out {
        code: o.status.code().unwrap_or(-1),
        stderr: rest.iter().flat_map(|l| [*l, "\n"]).collect(),
        timing: timing.join("\n"),
    }
}

/// The PCH's record up to its image line (the snapshot's own bytes vary
/// from run to run with hash-map order; consumers compare it).
fn record(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap();
    let at = bytes.windows(7).position(|w| w == b"\nimage ").unwrap_or(bytes.len());
    bytes[..at].to_vec()
}

/// Precompiles `header` (written as `hdr.h`) both ways with `flags`;
/// checks the two agree, and that the single pass stood in iff `single`.
fn both(name: &str, header: &str, flags: &[&str], single: bool) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("hdr.h"), header).unwrap();
    let mut args: Vec<&str> = flags.to_vec();
    let is_c = flags.iter().any(|f| f.starts_with("-std=") && !f.contains("++"));
    let lang = if is_c { "c-header" } else { "c++-header" };
    args.extend(["-fkiln-pch-snapshot=write", "-x", lang, "hdr.h", "-MD", "-MF", "hdr.d"]);
    let mut outs = Vec::new();
    for (passes, pch) in [("1", "one.pch"), ("2", "two.pch")] {
        let mut a = args.clone();
        a.extend(["-o", pch]);
        let o = kiln(&d, passes, &a);
        let deps = std::fs::read_to_string(d.join("hdr.d")).unwrap_or_default();
        let _ = std::fs::remove_file(d.join("hdr.d"));
        let rec = d.join(pch).exists().then(|| record(&d.join(pch)));
        let o = Out { stderr: o.stderr.replace(pch, "PCH"), ..o };
        outs.push((o, deps.replace(pch, "PCH"), rec));
    }
    let (one, two) = (&outs[0], &outs[1]);
    assert_eq!(one.0.code, two.0.code, "{name}: exit status");
    assert_eq!(one.0.stderr, two.0.stderr, "{name}: diagnostics");
    assert_eq!(one.1, two.1, "{name}: -MD file");
    assert_eq!(one.2, two.2, "{name}: record");
    let stood_in = one.0.timing.contains("producer single pass: stands in");
    assert_eq!(stood_in, single, "{name}: single pass taken\n{}", one.0.timing);
    // WP-1403: the forced two passes say why the single pass was not
    // tried.
    assert!(
        two.0.timing.contains("producer single pass: not attempted (")
            && !two.0.timing.contains("producer single pass: stands in"),
        "{name}: forced two passes\n{}",
        two.0.timing
    );
    d
}

/// Restores `one.pch` and `two.pch` in `d` for `main` (with the
/// producer's `flags`) and checks the two consumers agree on exit status,
/// diagnostics (minus `-v`'s lines, which name the PCH) and IR.
fn consume_both(d: &Path, flags: &[&str], main: &str, text: &str) {
    std::fs::write(d.join(main), text).unwrap();
    let mut outs = Vec::new();
    for pch in ["one.pch", "two.pch"] {
        let mut args = flags.to_vec();
        args.extend(["-fkiln-pch-snapshot=load", "-include-pch", pch, "-v", "-S", "-emit-llvm"]);
        args.extend(["-o", "main.ll", main]);
        let o = kiln(d, "", &args);
        assert_eq!(o.code, 0, "{pch}: {}", o.stderr);
        assert!(o.stderr.contains("snapshot matches; restoring it"), "{pch}: {}", o.stderr);
        let diags: String = o
            .stderr
            .lines()
            .filter(|l| !l.contains(pch) && !l.starts_with(' ') && !l.contains("kiln version"))
            .flat_map(|l| [l, "\n"])
            .collect();
        outs.push((diags, std::fs::read_to_string(d.join("main.ll")).unwrap()));
    }
    assert_eq!(outs[0], outs[1], "{}: the two PCHs' consumers", d.display());
}

#[test]
fn the_single_pass_stands_in_for_a_quiet_header() {
    let d = both("plain", "#include <vector>\n// comment\n#define X 1\n", &["-std=c++26"], true);
    // A consumer restores either PCH, with the same result.
    consume_both(
        &d,
        &["-std=c++26"],
        "main.cpp",
        "int main() { std::vector<int> v{X}; return v[0] - 1; }\n",
    );
    both(
        "once",
        "#pragma once\n#include <string>\ninline int f() { return 1; }\n",
        &["-std=c++26"],
        true,
    );
    // `-include`s before the header: the header still moves to the front.
    std::fs::write(d.join("pre.h"), "#define PRE 1\n").unwrap();
    let pre = d.join("pre.h").display().to_string();
    both("include", "#include <vector>\n", &["-std=c++26", "-include", &pre], true);
    // Unused-declaration warnings on, but the header declares nothing.
    both("wall", "#include <vector>\n", &["-std=c++26", "-Wall"], true);
    // ... or declares only what it uses.
    both("nowall", "#include <vector>\nstatic int unused_x;\n", &["-std=c++26"], true);
}

#[test]
fn the_header_pass_runs_when_a_main_file_could_differ() {
    let w = &["-std=c++26", "-Wall"][..];
    // Diagnostics: the header pass shows its own.
    let d = both("warn", "#include <vector>\n#warning \"hi\"\n", w, false);
    // The producer showed the header's warning; neither PCH's consumer
    // shows it again (spec/pch.md "Consumer").
    consume_both(&d, w, "main.cpp", "int main() { std::vector<int> v{1}; return v[0] - 1; }\n");
    // A C header: two passes (spec/snap.md "The producer's single pass").
    let d = both(
        "c",
        "#include <stdio.h>\nstatic inline int one(void) { return 1; }\n",
        &["-std=c17"],
        false,
    );
    consume_both(&d, &["-std=c17", "-x", "c"], "main.c", "int main(void) { return one() - 1; }\n");
    both("error", "#include <vector>\nint broken = ;\n", w, false);
    both(
        "late",
        "template <class T> struct S { static_assert(sizeof(T) == 0); };\ninline void f() { S<int>(); }\n",
        w,
        false,
    );
    // Warned of in a main file only.
    both("unused", "#include <vector>\nstatic int unused_x;\n", w, false);
    both("next", "#include_next <vector>\n", w, false);
    both("sysh", "#pragma GCC system_header\nint sys;\n", w, false);
    // Expands differently in a main file.
    both("level", "#if __INCLUDE_LEVEL__ == 0\nint top;\n#endif\n", w, false);
    // Asks for more than diagnostics and dependencies.
    both("verbose", "#include <vector>\n", &["-std=c++26", "-v"], false);
}

/// WP-1404: with no option, a PCH build whose prefix cannot be
/// snapshotted (here a global module fragment left open) still writes the
/// PCH and says why, once, in `-Wpch-snapshot-unused`; a blanket `-Werror`
/// leaves it a warning and the PCH is written (WP-1397: the group is
/// exempt), `-Werror=pch-snapshot-unused` makes it an error and no PCH;
/// `-Wno-pch-snapshot-unused` silences it. The consumer of that PCH
/// replays the header with no warning (a `-v` note only). spec/pch.md
/// "Producer".
#[test]
fn a_prefix_without_a_snapshot_still_gets_a_pch() {
    let d = scratch("unsnap");
    std::fs::write(d.join("gmf.h"), "module;\n#define X 1\n").unwrap();
    std::fs::write(d.join("m.cpp"), "export module m;\nexport int f() { return X; }\n").unwrap();
    let produce = |extra: &[&str], pch: &str| {
        let mut a = vec!["-std=c++26", "-x", "c++-header", "gmf.h", "-o", pch];
        a.extend(extra);
        kiln(&d, "", &a)
    };
    let o = produce(&[], "gmf.pch");
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(
        o.stderr,
        "warning: precompiled header 'gmf.pch' cannot carry a snapshot (Parser.mods: not at \
         rest); its consumers process the header again [-Wpch-snapshot-unused]\n"
    );
    assert!(o.timing.contains("note: not written: Parser.mods: not at rest"), "{}", o.timing);
    assert!(d.join("gmf.pch").exists());
    let o = produce(&["-Werror"], "werror.pch");
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(
        o.stderr.starts_with("warning: precompiled header 'werror.pch' cannot"),
        "{}",
        o.stderr
    );
    assert!(o.stderr.ends_with(" [-Wpch-snapshot-unused]\n"), "{}", o.stderr);
    assert!(d.join("werror.pch").exists());
    for extra in [&["-Werror=pch-snapshot-unused"][..], &["-Werror", "-Werror=pch-snapshot-unused"]]
    {
        let o = produce(extra, "explicit.pch");
        assert_eq!(o.code, 1, "{extra:?}: {}", o.stderr);
        assert!(
            o.stderr.starts_with("error: precompiled header 'explicit.pch' cannot"),
            "{}",
            o.stderr
        );
        assert!(o.stderr.contains("[-Werror,-Wpch-snapshot-unused]"), "{}", o.stderr);
        assert!(!d.join("explicit.pch").exists());
    }
    let o = produce(&["-Wno-pch-snapshot-unused"], "quiet.pch");
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
    for v in [false, true] {
        let mut a = vec!["-std=c++26", "-include-pch", "gmf.pch", "-fsyntax-only", "m.cpp"];
        if v {
            a.push("-v");
        }
        let o = kiln(&d, "", &a);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert!(!o.stderr.contains("warning"), "{}", o.stderr);
        assert_eq!(o.stderr.contains("snapshot not used"), v, "{}", o.stderr);
    }
}

/// A header being precompiled is a prefix (clang `TU_Prefix`): the
/// `#pragma weak` identifiers it leaves undeclared and its unused
/// file-scope variables are the including unit's to diagnose, so its
/// producer is silent (and, for the pragma, takes the single pass). The
/// same text as a main file warns.
#[test]
fn a_prefix_leaves_end_of_unit_warnings_to_its_consumer() {
    let d = scratch("prefix-eot");
    let text = "#pragma weak later_fn\nstatic int unused_x;\n";
    std::fs::write(d.join("p.h"), text).unwrap();
    std::fs::write(d.join("p.cpp"), text).unwrap();
    std::fs::write(d.join("m.cpp"), "extern \"C\" int later_fn();\n").unwrap();
    for extra in [&[][..], &["-fno-kiln-pch-snapshot"][..]] {
        let mut a = vec!["-std=c++26", "-Wall", "-x", "c++-header", "p.h", "-o", "p.pch"];
        a.extend(extra);
        let o = kiln(&d, "", &a);
        assert_eq!((o.code, o.stderr.as_str()), (0, ""), "{extra:?}");
    }
    // Without the unused variable (an unused-declaration warning on keeps
    // the header pass), the single pass stands in.
    std::fs::write(d.join("w.h"), "#pragma weak later_fn\n").unwrap();
    let o = kiln(&d, "", &["-std=c++26", "-Wall", "-x", "c++-header", "w.h", "-o", "w.pch"]);
    assert_eq!((o.code, o.stderr.as_str()), (0, ""));
    assert!(o.timing.contains("single pass: stands in"), "{}", o.timing);
    let o = kiln(&d, "", &["-std=c++26", "-Wall", "-fsyntax-only", "p.cpp"]);
    assert!(o.stderr.contains("weak identifier 'later_fn' never declared"), "{}", o.stderr);
    assert!(o.stderr.contains("unused variable 'unused_x'"), "{}", o.stderr);
}
