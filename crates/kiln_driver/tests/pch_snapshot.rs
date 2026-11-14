//! The PCH snapshot end to end through `kiln++` (spec/pch.md).
//!
//! - [`product`] (WP-1392): the modes, the identity refusing with the
//!   first differing component named (`-v`), options that cannot change
//!   the prefix not refusing, the file check (modification time first,
//!   the content when it differs, `-fkiln-pch-validate=content`), the
//!   section header, and `=check`. Every refusal replays the header
//!   ([cpp.include]/2: the result is as if the header were included).
//! - [`behaviour`] (WP-1394): the diagnostics policy (clang's: the
//!   header's own diagnostics are not shown again), `-verify` consumers,
//!   C headers, prefixes that end with open state, the `-MD` list, and
//!   chained and module consumers. Each restored compile is compared with
//!   the replay of the same PCH ([cpp.include]); where clang's PCH is the
//!   reference, clang++-19's output is checked when it is installed.
#![forbid(unsafe_code)]

/// WP-1392: identity, validation, modes and the section header.
mod product {

    use std::fmt::Write as _;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, SystemTime};

    use kiln_pp::bmi;

    fn kilnpp() -> PathBuf {
        Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kiln-pch-snap-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("sys")).unwrap();
        d
    }

    struct Out {
        code: i32,
        stdout: String,
        stderr: String,
    }

    fn kiln(dir: &Path, args: &[&str]) -> Out {
        let o = Command::new(kilnpp())
            .args(args)
            .current_dir(dir)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
        assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
        Out {
            code: o.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr,
        }
    }

    fn ok(dir: &Path, args: &[&str]) -> Out {
        let o = kiln(dir, args);
        assert_eq!(o.code, 0, "kiln++ {args:?}:\n{}", o.stderr);
        o
    }

    /// The `-v` snapshot notes of a run.
    fn notes(o: &Out) -> Vec<&str> {
        o.stderr.lines().filter(|l| l.contains("': snapshot ")).collect()
    }

    /// The single snapshot note of a run, after `snapshot `.
    fn note(o: &Out) -> String {
        let n = notes(o);
        assert_eq!(n.len(), 1, "{}", o.stderr);
        n[0].split_once("': snapshot ").map(|(_, w)| w.to_owned()).unwrap_or_default()
    }

    /// The prefix: a user header including a system one.
    const HEADER: &str = "#include <s.h>\nstruct S { int a; };\ninline int f() { return V; }\n";
    const SYS: &str = "#define V 1\n";
    const MAIN: &str = "int x[V];\nint main() { S s{f()}; return s.a - 1; }\n";

    /// A directory with the prefix built into `h.pch` with `flags` and
    /// `-fkiln-pch-snapshot`.
    fn built(name: &str, flags: &[&str]) -> PathBuf {
        let d = scratch(name);
        std::fs::write(d.join("h.h"), HEADER).unwrap();
        std::fs::write(d.join("sys/s.h"), SYS).unwrap();
        std::fs::write(d.join("t.cpp"), MAIN).unwrap();
        let mut a = vec!["-std=c++20", "-isystem", "sys", "-fkiln-pch-snapshot", "-v"];
        a.extend_from_slice(flags);
        a.extend_from_slice(&["-x", "c++-header", "h.h", "-o", "h.pch"]);
        let o = ok(&d, &a);
        assert_eq!(note(&o), "written", "{}", o.stderr);
        d
    }

    /// The consumer's command line: the producer's flags, the PCH, `extra`.
    fn consume<'a>(extra: &[&'a str]) -> Vec<&'a str> {
        let mut a = vec!["-std=c++20", "-isystem", "sys", "-include-pch", "h.pch", "-v"];
        a.extend_from_slice(extra);
        a.extend_from_slice(&["-fsyntax-only", "t.cpp"]);
        a
    }

    const USED: &str = "matches; restoring it";

    /// The modes: the default and bare (both sides), `=write` and `=load`
    /// (one side each), `-fno-` (neither), the last one winning.
    #[test]
    fn modes() {
        let d = built("modes", &[]);
        assert_eq!(note(&ok(&d, &consume(&["-fkiln-pch-snapshot"]))), USED);
        assert_eq!(note(&ok(&d, &consume(&["-fkiln-pch-snapshot=load"]))), USED);
        assert!(notes(&ok(&d, &consume(&["-fkiln-pch-snapshot=write"]))).is_empty());
        // WP-1404: no option is `-fkiln-pch-snapshot`.
        assert_eq!(note(&ok(&d, &consume(&[]))), USED);
        let off = ["-fkiln-pch-snapshot", "-fno-kiln-pch-snapshot"];
        assert!(notes(&ok(&d, &consume(&off))).is_empty());
        assert!(notes(&ok(&d, &consume(&["-fno-kiln-pch-snapshot"]))).is_empty());
        // The default producer writes the section; `-fno-` writes none,
        // and the default consumer of that PCH replays with a `-v` note
        // only.
        let producer = |extra: &[&str], out: &str| {
            let mut a = vec!["-std=c++20", "-isystem", "sys", "-v"];
            a.extend_from_slice(extra);
            a.extend_from_slice(&["-x", "c++-header", "h.h", "-o", out]);
            ok(&d, &a)
        };
        assert_eq!(note(&producer(&[], "dflt.pch")), "written");
        let o = producer(&["-fno-kiln-pch-snapshot"], "none.pch");
        assert!(notes(&o).is_empty(), "{}", o.stderr);
        let mut c = consume(&[]);
        c[4] = "none.pch";
        let o = ok(&d, &c);
        assert_eq!(note(&o), "not used: the PCH has no snapshot section", "{}", o.stderr);
        assert!(!o.stderr.contains("warning:"), "{}", o.stderr);
        // A producer with `=load` writes no section.
        let o = ok(
            &d,
            &["-std=c++20", "-isystem", "sys", "-fkiln-pch-snapshot=load", "-x"]
                .into_iter()
                .chain(["c++-header", "h.h", "-o", "n.pch"])
                .collect::<Vec<_>>(),
        );
        assert!(notes(&o).is_empty(), "{}", o.stderr);
        let n = kiln(&d, &consume(&["-fkiln-pch-snapshot=sometimes"]));
        assert_ne!(n.code, 0);
        assert!(n.stderr.contains("invalid value 'sometimes'"), "{}", n.stderr);
    }

    /// Options that cannot change the prefix's state do not refuse it; each
    /// that can is named, and the compile replays (spec/pch.md
    /// "Identity").
    #[test]
    fn identity_components() {
        let d = built("identity", &["-Wshadow", "-Wunused-variable"]);
        let used = |extra: &[&str]| {
            let mut a = vec!["-Wshadow", "-Wunused-variable"];
            a.extend_from_slice(extra);
            a.push("-fkiln-pch-snapshot");
            assert_eq!(note(&ok(&d, &consume(&a))), USED, "{extra:?}");
        };
        used(&[]);
        used(&["-g", "-MD", "-MF", "t.d", "-ferror-limit=2", "-fcolor-diagnostics"]);
        used(&["-fdump-ast=locs", "-fkiln-pch-validate=content", "-include", "sys/s.h"]);
        // WP-1403: another search list restores when every lookup finds
        // what it found (tests/pch_snapshot_search.rs has the rest).
        used(&["-I", "."]);
        // The `-W` options commute: the policy is the same.
        let swapped = consume(&["-Wunused-variable", "-Wshadow", "-fkiln-pch-snapshot"]);
        assert_eq!(note(&ok(&d, &swapped)), USED);
        let refused = |extra: &[&str], why: &str| {
            let mut a = vec!["-Wshadow", "-Wunused-variable"];
            a.extend_from_slice(extra);
            a.push("-fkiln-pch-snapshot");
            let o = ok(&d, &consume(&a));
            let n = note(&o);
            assert!(n.starts_with(&format!("not used: {why}")), "{extra:?}: {n}");
        };
        refused(
            &["-DNEW=1"],
            "-D/-U list differs at line 1: '<end>' (producer) vs '-DNEW=1' (here)",
        );
        refused(&["-Wextra"], "warning policy differs");
        refused(&["-fmacro-prefix-map=/x=/y"], "prefix maps differ");
        refused(&["-fkiln-source-path-style=gcc"], "source path style differs");
        refused(&["-fconstexpr-steps=12345"], "options differ at line");
        // Without the `-W` options at all.
        let o = ok(&d, &consume(&["-fkiln-pch-snapshot"]));
        assert!(note(&o).starts_with("not used: warning policy differs"), "{}", o.stderr);
    }

    /// Sets `path`'s modification time.
    fn set_mtime(path: &Path, t: SystemTime) {
        let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(t).unwrap();
    }

    fn mtime(path: &Path) -> SystemTime {
        std::fs::metadata(path).unwrap().modified().unwrap()
    }

    /// The file check: a touched file with the same text is accepted (after
    /// hashing), a file with the recorded time but another size is refused,
    /// and the content mode reads every file. The system header is not
    /// checked by replay (clang does not check system input files), so the
    /// compile goes on by replaying it.
    #[test]
    fn file_check() {
        let d = built("files", &[]);
        let s = d.join("sys/s.h");
        let t0 = mtime(&s);
        set_mtime(&s, t0 + Duration::from_secs(5));
        assert_eq!(note(&ok(&d, &consume(&["-fkiln-pch-snapshot"]))), USED);
        // Same size, other text, the recorded time: `mtime` trusts it (as
        // clang does; a compile that then reads the edited text ends with a
        // fatal error: `behaviour::prefix_file_changed_after_the_check`),
        // `content` does not.
        std::fs::write(&s, "#define V 2\n").unwrap();
        set_mtime(&s, t0);
        assert_eq!(note(&kiln(&d, &consume(&["-fkiln-pch-snapshot"]))), USED);
        let o = ok(&d, &consume(&["-fkiln-pch-snapshot", "-fkiln-pch-validate=content"]));
        assert_eq!(note(&o), "not used: 'sys/s.h' changed");
        // Another size, the recorded time.
        std::fs::write(&s, "#define V 1 \n").unwrap();
        set_mtime(&s, t0);
        let o = ok(&d, &consume(&["-fkiln-pch-snapshot"]));
        assert_eq!(note(&o), "not used: 'sys/s.h' changed size");
        std::fs::remove_file(&s).unwrap();
        let o = kiln(&d, &consume(&["-fkiln-pch-snapshot"]));
        assert!(
            notes(&o).iter().any(|n| n.ends_with("not used: 'sys/s.h' is gone")),
            "{}",
            o.stderr
        );
    }

    /// A rewrite of a section's bytes.
    type Edit = dyn Fn(&[u8]) -> Vec<u8>;

    /// `pch` with its snapshot section's bytes replaced by `edit(bytes)`:
    /// the section block, the record's `image` line and its `end` checksum
    /// written again (spec/pch.md "File layout").
    fn with_section(pch: &[u8], edit: impl Fn(&[u8]) -> Vec<u8>) -> Vec<u8> {
        let at = pch.windows(5).position(|w| w == b"\nend ").unwrap();
        let text_end = at + 1 + pch[at + 1..].iter().position(|&b| b == b'\n').unwrap() + 1;
        let text = std::str::from_utf8(&pch[..=at]).unwrap();
        let mut sections = bmi::read_sections(&pch[text_end..]).unwrap();
        for s in &mut sections {
            if s.tag == bmi::SECTION_SNAPSHOT {
                s.bytes = edit(&s.bytes);
            }
        }
        let block = bmi::write_sections(&sections);
        let mut out = String::new();
        for line in text.lines() {
            if line.starts_with("image ") {
                let _ =
                    writeln!(out, "image {}\t{:016x}", block.len(), bmi::block_checksum(&block));
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        let sum = bmi::checksum(out.as_bytes());
        let _ = writeln!(out, "end {sum:016x}");
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&block);
        bytes
    }

    /// A truncated, a foreign and an old-version section are refused before
    /// anything is decoded; the compile replays.
    #[test]
    fn section_header() {
        let d = built("header", &[]);
        let pch = std::fs::read(d.join("h.pch")).unwrap();
        // The rewrite itself keeps the section usable.
        std::fs::write(d.join("h.pch"), with_section(&pch, <[u8]>::to_vec)).unwrap();
        assert_eq!(note(&ok(&d, &consume(&["-fkiln-pch-snapshot"]))), USED);
        let cases: [(&str, Box<Edit>); 3] = [
            ("its section is truncated (no header)", Box::new(|b: &[u8]| b[..5].to_vec())),
            (
                "its section is not a kiln snapshot (bad magic)",
                Box::new(|b: &[u8]| [b"FOREIGN!".as_slice(), &b[8..]].concat()),
            ),
            (
                "its section has format version 0, expected 2",
                Box::new(|b: &[u8]| [&b[..8], &[0, 0, 0, 0][..], &b[12..]].concat()),
            ),
        ];
        for (why, edit) in cases {
            std::fs::write(d.join("h.pch"), with_section(&pch, edit)).unwrap();
            let o = ok(&d, &consume(&["-fkiln-pch-snapshot"]));
            assert_eq!(note(&o), format!("not used: {why}"));
        }
    }

    /// `pch` with its record text's lines rewritten by `edit` (a line it
    /// maps to `None` is kept) and the `end` checksum written again; the
    /// section block after the text is kept.
    fn with_record_line(pch: &[u8], edit: impl Fn(&str) -> Option<String>) -> Vec<u8> {
        let at = pch.windows(5).position(|w| w == b"\nend ").unwrap();
        let text_end = at + 1 + pch[at + 1..].iter().position(|&b| b == b'\n').unwrap() + 1;
        let text = std::str::from_utf8(&pch[..=at]).unwrap();
        let mut out = String::new();
        for line in text.lines() {
            out.push_str(&edit(line).unwrap_or_else(|| line.to_owned()));
            out.push('\n');
        }
        let sum = bmi::checksum(out.as_bytes());
        let _ = writeln!(out, "end {sum:016x}");
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&pch[text_end..]);
        bytes
    }

    /// WP-1397 (spec/pch.md "Format versioning"): a PCH from another
    /// snapshot format version, another schema hash, with no snapshot
    /// section (what 1.1.0 wrote), or from another kiln is never misread:
    /// each compile ends as the replay's ([cpp.include]/2: as if the header
    /// were included), with `-Wpch-snapshot-unused` where the configuration
    /// is what differs, or in a clean error that the PCH must be rebuilt.
    #[test]
    fn cross_version() {
        let d = built("xver", &[]);
        let pch = std::fs::read(d.join("h.pch")).unwrap();
        let compile = |file: &str, extra: &[&str]| {
            let mut a = vec!["-std=c++20", "-isystem", "sys", "-include-pch", file, "-v"];
            a.extend_from_slice(extra);
            a.extend_from_slice(&["-S", "-emit-llvm", "t.cpp", "-o", "-"]);
            kiln(&d, &a)
        };
        let replay = compile("h.pch", &["-fno-kiln-pch-snapshot"]);
        assert_eq!(replay.code, 0, "{}", replay.stderr);
        assert!(replay.stdout.contains("define"), "{}", replay.stdout);
        let restored = compile("h.pch", &[]);
        assert_eq!(note(&restored), USED, "{}", restored.stderr);
        assert_eq!(restored.stdout, replay.stdout);
        // Another snapshot format version (a newer kiln's; an older one
        // is `section_header`'s version 0) and another schema hash (a
        // build whose snapshotted types differ): the configuration
        // differs, so the consumer warns and replays.
        let version: Box<Edit> =
            Box::new(|b: &[u8]| [&b[..8], &[3, 0, 0, 0][..], &b[12..]].concat());
        let schema: Box<Edit> = Box::new(|b: &[u8]| {
            let mut b = b.to_vec();
            b[12] ^= 0x5a;
            b
        });
        for (name, edit, why) in [
            ("v3.pch", version, "its section has format version 3, expected 2"),
            ("schema.pch", schema, "written by another build (schema hash)"),
        ] {
            std::fs::write(d.join(name), with_section(&pch, edit)).unwrap();
            let o = compile(name, &[]);
            assert_eq!(o.code, 0, "{name}: {}", o.stderr);
            assert_eq!(note(&o), format!("not used: {why}"), "{name}");
            assert!(
                o.stderr.contains(&format!(
                    "warning: precompiled header '{name}' was built with a different configuration ({why}); \
                     the header is processed again [-Wpch-snapshot-unused]"
                )),
                "{name}: {}",
                o.stderr
            );
            assert_eq!(o.stdout, replay.stdout, "{name}");
        }
        // No snapshot section (1.1.0's record, or `-fno-kiln-pch-snapshot`):
        // a quiet replay, `-v` only.
        let o = ok(
            &d,
            &["-std=c++20", "-isystem", "sys", "-fno-kiln-pch-snapshot", "-x", "c++-header"]
                .into_iter()
                .chain(["h.h", "-o", "none.pch"])
                .collect::<Vec<_>>(),
        );
        assert!(notes(&o).is_empty(), "{}", o.stderr);
        let o = compile("none.pch", &[]);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert_eq!(note(&o), "not used: the PCH has no snapshot section");
        assert!(!o.stderr.contains("warning:"), "{}", o.stderr);
        assert_eq!(o.stdout, replay.stdout);
        // Another kiln in the record: clang's "different branch" error,
        // which a rebuild of the PCH fixes. With the record check off, the
        // snapshot (which this build wrote: its identity is in the
        // section, not the record) restores.
        let other =
            |l: &str| l.starts_with("kiln ").then(|| "kiln 0.9.0 - -fclang-compat=22".to_owned());
        std::fs::write(d.join("old.pch"), with_record_line(&pch, other)).unwrap();
        let o = compile("old.pch", &[]);
        assert_eq!(o.code, 1, "{}", o.stderr);
        assert!(
            o.stderr.contains("error: PCH file built from a different branch (0.9.0 - "),
            "{}",
            o.stderr
        );
        assert!(o.stdout.is_empty(), "{}", o.stdout);
        let o = compile("old.pch", &["-Xclang", "-fno-validate-pch"]);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert_eq!(note(&o), "matches; restoring it", "{}", o.stderr);
        assert_eq!(o.stdout, replay.stdout);
        // Another kiln in the snapshot's identity (the same schema): the
        // `kiln build` component refuses it with the warning, and the
        // header is replayed.
        let version = env!("CARGO_PKG_VERSION").as_bytes();
        let needle = [b"kiln ", version].concat();
        let other_build = move |s: &[u8]| {
            let at = s.windows(needle.len()).position(|w| w == needle.as_slice()).unwrap();
            let mut s = s.to_vec();
            for b in &mut s[at + 5..at + 5 + version.len()] {
                if b.is_ascii_digit() {
                    *b = b'9';
                }
            }
            s
        };
        std::fs::write(d.join("build.pch"), with_section(&pch, other_build)).unwrap();
        let o = compile("build.pch", &[]);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert!(note(&o).starts_with("not used: kiln build differs"), "{}", o.stderr);
        assert!(o.stderr.contains("[-Wpch-snapshot-unused]"), "{}", o.stderr);
        assert_eq!(o.stdout, replay.stdout);
        // An older record format (`KILNPCH 1`, before the section block):
        // the error says the PCH is no longer supported.
        let v1 = |l: &str| (l == "KILNPCH 2").then(|| "KILNPCH 1".to_owned());
        std::fs::write(d.join("v1.pch"), with_record_line(&pch, v1)).unwrap();
        let o = compile("v1.pch", &[]);
        assert_eq!(o.code, 1, "{}", o.stderr);
        assert!(
            o.stderr.contains(
                "error: PCH file 'v1.pch' uses an older format that is no longer supported"
            ),
            "{}",
            o.stderr
        );
    }

    /// `=check`: clean when the snapshot and replay agree; a system header
    /// changed behind the recorded time makes them differ, an error naming
    /// the first differing line.
    #[test]
    fn check_mode() {
        let d = built("check", &[]);
        let o = ok(&d, &consume(&["-fkiln-pch-snapshot=check", "-fdump-ast"]));
        assert!(!o.stderr.contains("differs from replaying"), "{}", o.stderr);
        assert!(o.stdout.contains("VarDecl"), "the job's own dump: {}", o.stdout);
        let s = d.join("sys/s.h");
        let t0 = mtime(&s);
        std::fs::write(&s, "#define V 2\n").unwrap();
        set_mtime(&s, t0);
        let o = kiln(&d, &consume(&["-fkiln-pch-snapshot=check"]));
        assert_eq!(o.code, 1, "{}", o.stderr);
        assert!(
            o.stderr.contains(
                "error: the snapshot of precompiled header 'h.pch' differs from replaying the header: "
            ),
            "{}",
            o.stderr
        );
        // A compile reports it too, and writes no output.
        let o = kiln(
            &d,
            &[
                "-std=c++20",
                "-isystem",
                "sys",
                "-include-pch",
                "h.pch",
                "-fkiln-pch-snapshot=check",
                "-S",
                "-emit-llvm",
                "t.cpp",
                "-o",
                "t.ll",
            ],
        );
        assert_eq!(o.code, 1, "{}", o.stderr);
        assert!(o.stderr.contains("differs from replaying the header"), "{}", o.stderr);
    }

    /// `=check` with a prefix that warns: the restore does not show the
    /// header's warning again (WP-1394) and the replay does, so `=check`
    /// compares the main file's diagnostics only, and finds no difference;
    /// what the compile prints is the replay's.
    #[test]
    fn check_mode_prefix_warns() {
        let d = scratch("check-warns");
        std::fs::write(d.join("h.h"), "#warning \"the prefix warns\"\nstruct S { int a; };\n")
            .unwrap();
        std::fs::write(d.join("t.cpp"), "#warning \"the main file warns\"\nS s{0};\n").unwrap();
        let o = ok(
            &d,
            &["-std=c++20", "-fkiln-pch-snapshot", "-x", "c++-header", "h.h", "-o", "h.pch"],
        );
        assert!(o.stderr.contains("the prefix warns"), "{}", o.stderr);
        let o = ok(
            &d,
            &[
                "-std=c++20",
                "-include-pch",
                "h.pch",
                "-fkiln-pch-snapshot=check",
                "-fdump-ast",
                "-fsyntax-only",
                "t.cpp",
            ],
        );
        assert!(!o.stderr.contains("differs from replaying"), "{}", o.stderr);
        // The output is the replay's (spec/pch.md), so the
        // prefix's warning is shown once, as without the option.
        assert_eq!(o.stderr.matches("warning: \"the prefix warns\"").count(), 1, "{}", o.stderr);
        assert_eq!(o.stderr.matches("warning: \"the main file warns\"").count(), 1, "{}", o.stderr);
    }
}

/// WP-1394: the consumer's behaviour on a restore.
#[allow(clippy::many_single_char_names, reason = "d(ir), r(eplay), s(napshot), o(utput)")]
mod behaviour {

    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    fn kiln_bin(cxx: bool) -> PathBuf {
        let k = PathBuf::from(env!("CARGO_BIN_EXE_kiln"));
        if cxx { k.with_file_name("kiln++") } else { k }
    }

    fn have(tool: &str) -> bool {
        Command::new(tool)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kiln-pchsnap-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(d: &Path, files: &[(&str, &str)]) {
        for (f, t) in files {
            std::fs::write(d.join(f), t).unwrap();
        }
    }

    struct Out {
        code: i32,
        stderr: String,
    }

    fn run_with(prog: &Path, dir: &Path, args: &[&str]) -> Out {
        let o = Command::new(prog)
            .args(args)
            .current_dir(dir)
            .env("KILN_SNAP_TIMING", "1")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
        assert!(!stderr.contains("panicked"), "{} {args:?} panicked:\n{stderr}", prog.display());
        Out { code: o.status.code().unwrap_or(-1), stderr }
    }

    fn kiln(dir: &Path, args: &[&str]) -> Out {
        run_with(&kiln_bin(true), dir, args)
    }

    /// The stderr without the `-v` banner, the snapshot's timing lines and
    /// `-v` notes.
    fn diags(o: &Out) -> String {
        let e = o.stderr.as_str();
        let e = e.split_once("End of search list.\n").map_or(e, |(_, r)| r);
        e.lines()
            .filter(|l| {
                !l.starts_with("snapshot-timing: ") && !l.starts_with("note: precompiled header")
            })
            .fold(String::new(), |mut out, l| {
                out.push_str(l);
                out.push('\n');
                out
            })
    }

    fn restored(o: &Out) -> bool {
        o.stderr.lines().any(|l| l.starts_with("snapshot-timing: restore "))
    }

    /// Builds `pch` from `header` with `flags`; asserts the snapshot was
    /// written.
    fn produce(d: &Path, flags: &[&str], lang: &str, header: &str, pch: &str) -> Out {
        let mut a: Vec<&str> = flags.to_vec();
        a.extend(["-fkiln-pch-snapshot=write", "-v", "-x", lang, header, "-o", pch]);
        let o = kiln(d, &a);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert!(o.stderr.contains(&format!("'{pch}': snapshot written")), "{}", o.stderr);
        o
    }

    /// The consumer's compile of `main` with `pch`, by replay
    /// (`-fno-kiln-pch-snapshot`) and by snapshot (the default, WP-1404):
    /// (replay, snapshot). `extra` comes after `-include-pch`.
    fn both(d: &Path, flags: &[&str], pch: &str, extra: &[&str]) -> (Out, Out) {
        let mut a: Vec<&str> = flags.to_vec();
        a.extend(["-include-pch", pch]);
        a.extend(extra);
        let mut r = a.clone();
        r.push("-fno-kiln-pch-snapshot");
        let replay = kiln(d, &r);
        a.push("-v");
        let snap = kiln(d, &a);
        (replay, snap)
    }

    const DIAG_PREFIX: &str = "#warning \"the prefix warns\"\n\
    [[deprecated]] inline int old_api() { return 1; }\n\
    inline int uses_old() { return old_api(); }\n\
    template <class T> int needs_member(T t) { return t.member; }\n";
    const DIAG_MAIN: &str = "int f() { return old_api(); }\n\
    int g() { return needs_member(1); }\n";

    /// The header's own warnings are shown when the PCH is built, not when it
    /// is used, and the summary counts the consumer's only: clang's PCH.
    /// clang 19:
    /// `clang++-19 -std=c++26 -x c++-header p.h -o p.pch` prints the two
    /// prefix warnings ("2 warnings generated."); `clang++-19 -std=c++26
    /// -include-pch p.pch -fsyntax-only m.cpp` prints `m.cpp:1:18: warning:
    /// 'old_api' is deprecated`, `p.h:4:52: error: member reference base type
    /// 'int' is not a structure or union` and "1 warning and 1 error
    /// generated.".
    #[test]
    fn header_diagnostics_are_not_shown_again() {
        let d = scratch("diag");
        write(&d, &[("p.h", DIAG_PREFIX), ("m.cpp", DIAG_MAIN)]);
        let p = produce(&d, &["-std=c++26"], "c++-header", "p.h", "p.pch");
        assert!(p.stderr.contains("2 warnings generated."), "{}", p.stderr);
        let (replay, snap) = both(&d, &["-std=c++26"], "p.pch", &["-fsyntax-only", "m.cpp"]);
        assert!(restored(&snap), "{}", snap.stderr);
        assert_eq!((replay.code, snap.code), (1, 1));
        assert!(replay.stderr.contains("3 warnings and 1 error generated."), "{}", replay.stderr);
        let s = diags(&snap);
        assert!(s.ends_with("1 warning and 1 error generated.\n"), "{s}");
        assert!(!s.contains("the prefix warns") && !s.contains("p.h:3:"), "{s}");
        assert!(s.contains("m.cpp:1:18: warning: 'old_api' is deprecated"), "{s}");
        assert!(s.contains("p.h:4:52: error: member reference base type 'int'"), "{s}");
        assert!(snap.stderr.contains("the header reported 2 diagnostic(s)"), "{}", snap.stderr);
        assert!(
            snap.stderr.contains("'p.pch': snapshot restored; the 2 warning(s) and error(s)"),
            "{}",
            snap.stderr
        );
        if have("clang++-19") {
            let c = Path::new("clang++-19");
            let o = run_with(c, &d, &["-std=c++26", "-x", "c++-header", "p.h", "-o", "c.pch"]);
            assert!(o.stderr.contains("2 warnings generated."), "{}", o.stderr);
            let o =
                run_with(c, &d, &["-std=c++26", "-include-pch", "c.pch", "-fsyntax-only", "m.cpp"]);
            assert!(o.stderr.ends_with("1 warning and 1 error generated.\n"), "{}", o.stderr);
        }
    }

    /// A prefix with warnings gets a snapshot; with `-Werror` they are errors
    /// and there is no PCH at all.
    #[test]
    fn werror_prefix_has_no_snapshot() {
        let d = scratch("werror");
        write(&d, &[("p.h", "#warning \"w\"\nint x;\n")]);
        produce(&d, &["-std=c++26"], "c++-header", "p.h", "p.pch");
        let o = kiln(
            &d,
            &[
                "-std=c++26",
                "-Werror",
                "-fkiln-pch-snapshot=write",
                "-v",
                "-x",
                "c++-header",
                "p.h",
            ],
        );
        assert_eq!(o.code, 1, "{}", o.stderr);
        assert!(o.stderr.contains("error: \"w\" [-Werror,-W#warnings]"), "{}", o.stderr);
        assert!(!o.stderr.contains("snapshot written"), "{}", o.stderr);
        assert!(!d.join("p.h.pch").exists());
    }

    /// State that affects later diagnostics is carried: a `#pragma clang
    /// diagnostic push` left open with its mappings, a `GCC diagnostic`
    /// mapping, and the warnings kiln gives once (an undefined internal
    /// function used in the prefix is reported once, at its first use).
    /// Same as replay; clang 19's PCH prints the same three warnings.
    #[test]
    fn diagnostic_state_is_carried() {
        let d = scratch("pragmas");
        write(
            &d,
            &[
                (
                    "p.h",
                    "static int internal_undefined();\n\
                     inline int pfx() { return internal_undefined(); }\n\
                     #pragma GCC diagnostic warning \"-Wshadow\"\n\
                     #pragma clang diagnostic push\n\
                     #pragma clang diagnostic ignored \"-Wunused-variable\"\n",
                ),
                (
                    "m.cpp",
                    "int quiet() { int a = 1; return internal_undefined(); }\n\
                     #pragma clang diagnostic pop\n\
                     int loud(int x) { int b = 1; { int x = 2; (void)x; } return 0; }\n",
                ),
            ],
        );
        produce(&d, &["-std=c++26", "-Wall"], "c++-header", "p.h", "p.pch");
        let (replay, snap) =
            both(&d, &["-std=c++26", "-Wall"], "p.pch", &["-fsyntax-only", "m.cpp"]);
        assert!(restored(&snap), "{}", snap.stderr);
        assert_eq!(diags(&snap), diags(&replay));
        let s = diags(&snap);
        assert!(!s.contains("unused variable 'a'"), "{s}");
        assert!(s.contains("unused variable 'b'"), "{s}");
        assert!(s.contains("declaration shadows a local variable"), "{s}");
        assert_eq!(s.matches("internal linkage but is not defined").count(), 1, "{s}");
        assert!(s.contains("p.h:1:12: warning: function 'internal_undefined'"), "{s}");
    }

    /// `-verify` with a snapshot (spec/pch.md "Consumer"): restored
    /// when the header reported nothing and holds no directive; a failed
    /// expectation is reported as by replay. Otherwise a silent replay with a
    /// `-v` note. clang 19 restores and does not see the header's
    /// diagnostics or directives (`-Xclang -verify -include-pch p.pch` with a
    /// main file expecting `expected-warning@p.h:1` fails: "expected but not
    /// seen").
    #[test]
    fn verify_consumers() {
        let d = scratch("verify");
        write(
            &d,
            &[
                ("clean.h", "template <class T> int get(T t) { return t.member; }\nint quiet();\n"),
                ("warns.h", "#warning \"pw\"\nint quiet();\n"),
                ("dir.h", "// expected-no-diagnostics\nint quiet();\n"),
                (
                    "ok.cpp",
                    "int f() { return quiet() + nope; } // expected-error {{undeclared identifier}}\n\
                     int g() { return get(1); } // expected-note {{requested here}}\n\
                     // expected-error@clean.h:1 {{member reference base type}}\n",
                ),
                ("bad.cpp", "int f() { return quiet(); } // expected-error {{nothing}}\n"),
                ("mw.cpp", "int h() { return quiet(); } // expected-warning@warns.h:1 {{pw}}\n"),
                ("md.cpp", "int h() { return quiet(); }\n"),
            ],
        );
        for h in ["clean", "warns", "dir"] {
            produce(&d, &["-std=c++26"], "c++-header", &format!("{h}.h"), &format!("{h}.pch"));
        }
        let verify = |pch: &str, main: &str| {
            both(&d, &["-std=c++26"], pch, &["-Xclang", "-verify", "-fsyntax-only", main])
        };
        let (r, s) = verify("clean.pch", "ok.cpp");
        assert!(restored(&s), "{}", s.stderr);
        assert_eq!((r.code, s.code), (0, 0), "{}", s.stderr);
        let (r, s) = verify("clean.pch", "bad.cpp");
        assert!(restored(&s), "{}", s.stderr);
        assert_eq!((r.code, s.code), (1, 1));
        assert_eq!(diags(&s), diags(&r));
        let (r, s) = verify("warns.pch", "mw.cpp");
        assert_eq!((r.code, s.code), (0, 0), "{}", s.stderr);
        assert!(
            s.stderr.contains("snapshot not used (-verify: the header reported 1 diagnostic(s)"),
            "{}",
            s.stderr
        );
        let (r, s) = verify("dir.pch", "md.cpp");
        assert_eq!((r.code, s.code), (0, 0), "{}", s.stderr);
        assert!(s.stderr.contains("'dir.h' may hold -verify directives"), "{}", s.stderr);
    }

    /// [cpp.include] A C header gets a snapshot (tentative definitions,
    /// K&R definitions, `#pragma pack`): the restored compile's IR and
    /// diagnostics are replay's.
    #[test]
    fn c_header() {
        let d = scratch("c");
        write(
            &d,
            &[
                (
                    "p.h",
                    "int counter;\nint counter;\nstatic int add(a, b) int a; int b; { return a + b; }\n\
                     _Static_assert(sizeof(int) == 4, \"int\");\n\
                     static inline int twice(int x) { return add(x, x); }\n\
                     #pragma pack(push, 1)\nstruct packed { char c; int i; };\n#pragma pack(pop)\n",
                ),
                (
                    "m.c",
                    "int vla(int n) { typedef int row[n]; return (int)sizeof(row); }\n\
                     int main(void) { counter++; return twice(counter) + (int)sizeof(struct packed) + vla(2); }\n",
                ),
            ],
        );
        let c = kiln_bin(false);
        let o = run_with(
            &c,
            &d,
            &[
                "-std=c17",
                "-fkiln-pch-snapshot=write",
                "-v",
                "-x",
                "c-header",
                "p.h",
                "-o",
                "p.pch",
            ],
        );
        assert!(o.stderr.contains("snapshot written"), "{}", o.stderr);
        let r = run_with(
            &c,
            &d,
            &[
                "-std=c17",
                "-include-pch",
                "p.pch",
                "-fno-kiln-pch-snapshot",
                "-S",
                "-emit-llvm",
                "m.c",
                "-o",
                "r.ll",
            ],
        );
        // The default consumer restores (WP-1404).
        let s = run_with(
            &c,
            &d,
            &["-std=c17", "-include-pch", "p.pch", "-S", "-emit-llvm", "m.c", "-o", "s.ll"],
        );
        assert!(restored(&s), "{}", s.stderr);
        assert_eq!((r.code, s.code), (0, 0), "{}", s.stderr);
        // The header's K&R warning is the producer's, not shown again.
        assert!(r.stderr.contains("p.h:3:12: warning: a function definition without a prototype"));
        assert_eq!(diags(&s), "");
        let ir = |f: &str| std::fs::read_to_string(d.join(f)).unwrap();
        assert_eq!(ir("s.ll"), ir("r.ll"));
        assert!(ir("s.ll").contains("@counter = dso_local global i32 0"), "{}", ir("s.ll"));
    }

    /// Prefixes that end with open state: carried exactly (a `push_macro`
    /// whose pop is in the main file, `#pragma pack(push)`, `#pragma GCC
    /// visibility push`) or refused at the producer with a `-v` note (a
    /// global module fragment's `module;`); a header ending inside `#if` is
    /// an error, so there is no PCH.
    #[test]
    fn open_state_at_the_end_of_the_prefix() {
        let d = scratch("open");
        write(
            &d,
            &[
                (
                    "p.h",
                    "#define LEVEL 1\n#pragma push_macro(\"LEVEL\")\n#undef LEVEL\n#define LEVEL 2\n\
                     #pragma pack(push, 1)\n#pragma GCC visibility push(hidden)\n",
                ),
                (
                    "m.cpp",
                    "struct S { char c; int i; };\nint hidden_fn() { return LEVEL; }\n\
                     #pragma GCC visibility pop\n#pragma pack(pop)\n#pragma pop_macro(\"LEVEL\")\n\
                     int shown() { return LEVEL * 10 + (int)sizeof(S) + hidden_fn(); }\n",
                ),
                ("gmf.h", "module;\n"),
                ("if.h", "#if 1\nint x;\n"),
            ],
        );
        produce(&d, &["-std=c++26"], "c++-header", "p.h", "p.pch");
        let (r, s) = both(&d, &["-std=c++26"], "p.pch", &["-fsyntax-only", "m.cpp"]);
        assert!(restored(&s), "{}", s.stderr);
        assert_eq!((r.code, s.code), (0, 0), "{}", s.stderr);
        assert_eq!(diags(&s), diags(&r));
        let out = |extra: &[&str], f: &str| {
            let mut a =
                vec!["-std=c++26", "-include-pch", "p.pch", "-S", "-emit-llvm", "m.cpp", "-o", f];
            a.extend(extra);
            let o = kiln(&d, &a);
            assert_eq!(o.code, 0, "{}", o.stderr);
            assert_eq!(restored(&o), extra.is_empty(), "{}", o.stderr);
            std::fs::read_to_string(d.join(f)).unwrap()
        };
        let (rl, sl) = (out(&["-fno-kiln-pch-snapshot"], "r.ll"), out(&[], "s.ll"));
        assert_eq!(sl, rl);
        assert!(sl.contains("define hidden noundef i32 @_Z9hidden_fnv()"), "{sl}");
        assert!(sl.contains("ret i32 2"), "{sl}");
        assert!(sl.contains("define dso_local noundef i32 @_Z5shownv()"), "{sl}");
        let o = kiln(
            &d,
            &["-std=c++26", "-fkiln-pch-snapshot=write", "-v", "-x", "c++-header", "gmf.h"],
        );
        assert!(
            o.stderr.contains("snapshot not written: Parser.mods: not at rest"),
            "{}",
            o.stderr
        );
        let o = kiln(&d, &["-std=c++26", "-fkiln-pch-snapshot=write", "-x", "c++-header", "if.h"]);
        assert_eq!(o.code, 1);
        assert!(o.stderr.contains("unterminated conditional directive"), "{}", o.stderr);
        assert!(!d.join("if.h.pch").exists());
    }

    /// The `-MD` list (spec/driver.md "Dependency output"): with
    /// `-include-pch`, the PCH's input files and then the main file, by
    /// replay and by snapshot alike, as clang lists them; with `-include`,
    /// the main file first. clang 19: `clang++-19 -include-pch p.pch -c
    /// m.cpp -MD -MF m.d` writes `m.o: <abs>/p.h <abs>/inner.h m.cpp m.h`;
    /// `-include p.h` writes `m.o: m.cpp p.h inner.h m.h`.
    #[test]
    fn depfile_order() {
        let d = scratch("md");
        write(
            &d,
            &[
                ("p.h", "#include \"inner.h\"\nint pf();\n"),
                ("inner.h", "int inner();\n"),
                ("m.h", "int mh();\n"),
                ("m.cpp", "#include \"m.h\"\nint main() { return pf(); }\n"),
            ],
        );
        produce(&d, &["-std=c++26"], "c++-header", "p.h", "p.pch");
        let dep = |extra: &[&str], f: &str| {
            let mut a = vec!["-std=c++26", "-fsyntax-only", "m.cpp", "-MD", "-MF", f, "-MT", "m.o"];
            a.extend(extra);
            let o = kiln(&d, &a);
            assert_eq!(o.code, 0, "{}", o.stderr);
            std::fs::read_to_string(d.join(f))
                .unwrap()
                .replace("\\\n", "")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        let replay = dep(&["-include-pch", "p.pch"], "r.d");
        let snap = dep(&["-include-pch", "p.pch", "-fkiln-pch-snapshot=load"], "s.d");
        let plain = dep(&["-include", "p.h"], "p.d");
        assert_eq!(snap, replay);
        assert_eq!(replay, "m.o: p.h inner.h m.cpp m.h");
        assert_eq!(plain, "m.o: m.cpp p.h inner.h m.h");
    }

    /// Chained and module consumers: a PCH built with another `-include-pch`
    /// restores; so does a consumer with `-fmodule-file=` when the producer
    /// had it too. A module unit consumer replays: a replay puts the header
    /// before its module declaration, which is an error ([module.unit]), and
    /// a restore would accept it as clang's PCH does.
    #[test]
    fn chained_and_module_consumers() {
        let d = scratch("chain");
        write(
            &d,
            &[
                ("a.h", "inline int a1() { return 1; }\n"),
                ("b.h", "inline int b1() { return a1() + 1; }\n"),
                ("m.cpp", "int main() { return a1() + b1() - 3; }\n"),
                ("mod.cppm", "export module mod;\nexport inline int mf() { return 2; }\n"),
                ("u.cpp", "import mod;\nint main() { return a1() + mf() - 3; }\n"),
                ("x.cppm", "export module x;\nexport inline int xf() { return a1(); }\n"),
            ],
        );
        produce(&d, &["-std=c++26"], "c++-header", "a.h", "a.pch");
        produce(&d, &["-std=c++26", "-include-pch", "a.pch"], "c++-header", "b.h", "b.pch");
        let ll = |args: &[&str], f: &str| {
            let mut a = args.to_vec();
            a.extend(["-S", "-emit-llvm", "-o", f]);
            let o = kiln(&d, &a);
            (o, std::fs::read_to_string(d.join(f)).unwrap_or_default())
        };
        let (_, r) = ll(&["-std=c++26", "-include-pch", "b.pch", "m.cpp"], "r.ll");
        let (s, sl) = ll(
            &["-std=c++26", "-include-pch", "b.pch", "-fkiln-pch-snapshot=load", "m.cpp"],
            "s.ll",
        );
        assert!(restored(&s), "{}", s.stderr);
        assert_eq!(sl, r);
        let o = kiln(&d, &["-std=c++26", "--precompile", "mod.cppm", "-o", "mod.pcm"]);
        assert_eq!(o.code, 0, "{}", o.stderr);
        produce(&d, &["-std=c++26", "-fmodule-file=mod=mod.pcm"], "c++-header", "a.h", "am.pch");
        let base = ["-std=c++26", "-fmodule-file=mod=mod.pcm", "-include-pch", "am.pch"];
        let (_, r) = ll(&[&base[..], &["u.cpp"]].concat(), "ru.ll");
        let (s, sl) = ll(&[&base[..], &["-fkiln-pch-snapshot=load", "u.cpp"]].concat(), "su.ll");
        assert!(restored(&s), "{}", s.stderr);
        assert_eq!(sl, r);
        let o = kiln(
            &d,
            &[
                "-std=c++26",
                "-include-pch",
                "a.pch",
                "-fkiln-pch-snapshot=load",
                "-v",
                "-fsyntax-only",
                "x.cppm",
            ],
        );
        assert!(!restored(&o), "{}", o.stderr);
        assert!(o.stderr.contains("'x.cppm' is a module unit"), "{}", o.stderr);
        assert!(o.stderr.contains("module declaration must occur at the start"), "{}", o.stderr);
    }

    /// A prefix file whose text changed after the PCH was built, in a way
    /// neither check sees: a system header (the record's check skips system
    /// files, as clang does) edited to the same size with its mtime put back
    /// (the default `-fkiln-pch-validate=mtime` trusts it). The restored
    /// source manager finds the mismatch when it first needs the text (the
    /// note's caret line in `s.h`), and the compile ends with clang's fatal
    /// error instead of rendering blank text ([cpp.include]; spec/pch.md
    /// "Consumer"). WP-1396.
    #[test]
    fn prefix_file_changed_after_the_check() {
        let d = scratch("stale");
        std::fs::create_dir_all(d.join("sys")).unwrap();
        let before = "inline int f(int a, int b) { return a + b; }\nstruct S { int a; };\n";
        let after_ = "inline int f(int x, int y) { return x - y; }\nstruct S { int b; };\n";
        assert_eq!(before.len(), after_.len());
        write(
            &d,
            &[
                ("sys/s.h", before),
                ("p.h", "#include <s.h>\n"),
                ("m.cpp", "int g() { return f(1); }\n"),
            ],
        );
        let flags = ["-std=c++26", "-isystem", "sys"];
        produce(&d, &flags, "c++-header", "p.h", "p.pch");
        let path = d.join("sys/s.h");
        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::fs::write(&path, after_).unwrap();
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(mtime).unwrap();
        let mut a: Vec<&str> = flags.to_vec();
        a.extend([
            "-include-pch",
            "p.pch",
            "-fkiln-pch-snapshot=load",
            "-v",
            "-c",
            "m.cpp",
            "-o",
            "m.o",
        ]);
        let o = kiln(&d, &a);
        assert!(restored(&o), "{}", o.stderr);
        assert_eq!(o.code, 1, "{}", o.stderr);
        assert!(
            o.stderr.contains(
                "s.h' has been modified since the precompiled header 'p.pch' was built: content changed"
            ),
            "{}",
            o.stderr
        );
        assert!(
            o.stderr.contains("m.cpp:1:18: error: no matching function for call to 'f'"),
            "{}",
            o.stderr
        );
        assert!(!d.join("m.o").exists());
    }

    /// WP-1395: a module whose global module fragment includes the prefix's
    /// header again ([module.global.frag]/1). A replay of the prefix records
    /// its declarations for the cross-unit deduplication (spec/modules.md §6),
    /// so the fragment's copy is skipped; a restored snapshot carries the
    /// same keys (`ModSema::prefix_keys`). Before, the restored consumer
    /// reported `template parameter redefines default argument` in the
    /// fragment's copy of the header. clang 19 accepts the program too.
    #[test]
    fn module_fragment_repeats_the_prefix() {
        let d = scratch("gmf");
        write(
            &d,
            &[
                ("h.h", "#pragma once\ntemplate <class R = int> int g() { return 1; }\n"),
                ("p.h", "#include \"h.h\"\n"),
                (
                    "m.cppm",
                    "module;\n#include \"h.h\"\nexport module m;\nexport int f() { return g<int>(); }\n",
                ),
                ("t.cpp", "\nimport m;\nint main() { return f() + g() - 2; }\n"),
            ],
        );
        let o = kiln(&d, &["-std=c++26", "--precompile", "m.cppm", "-o", "m.pcm"]);
        assert_eq!(o.code, 0, "{}", o.stderr);
        let flags = ["-std=c++26", "-fmodule-file=m=m.pcm"];
        produce(&d, &flags, "c++-header", "p.h", "p.pch");
        let (r, s) = both(&d, &flags, "p.pch", &["-fsyntax-only", "t.cpp"]);
        assert_eq!(r.code, 0, "{}", r.stderr);
        assert!(restored(&s), "{}", s.stderr);
        assert_eq!(s.code, 0, "{}", s.stderr);
        assert_eq!(diags(&s), diags(&r));
    }
}
