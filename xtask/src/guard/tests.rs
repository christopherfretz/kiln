//! Tests of `cargo xtask guard` (WP-1901; spec/harness.md "Misbehaviour
//! checks"): the strace log parser and both rules on canned logs, in the
//! shapes `strace -f -y -qq` writes (taken from a clean traced run of the
//! corpus slice, paths shortened), and the risky-change report.

use super::*;

/// The allowlist of a checkout at `/w/kiln` with its target directory.
fn allow() -> Allow {
    Allow::new(&[PathBuf::from("/w/kiln/target")])
}

/// A clean log: reads, writes under the target directory (a relative path
/// under a dirfd, a rename, a symlink, an unfinished call resumed), the
/// null device, a failed write elsewhere, signals and exits.
const CLEAN: &str = r#"100 openat(AT_FDCWD</w/kiln>, "/w/kiln/tests/a.cpp", O_RDONLY|O_CLOEXEC) = 3</w/kiln/tests/a.cpp>
100 mkdirat(AT_FDCWD</w/kiln>, "/w/kiln/target/xtask-test/run-1", 0777) = 0
101 openat(AT_FDCWD</w/kiln/target/xtask-test/run-1>, "t0/test.ll", O_WRONLY|O_CREAT|O_TRUNC|O_CLOEXEC, 0666 <unfinished ...>
102 openat(AT_FDCWD</w/kiln>, "/dev/null", O_RDWR|O_CLOEXEC) = 4</dev/null>
101 <... openat resumed>)             = 5</w/kiln/target/xtask-test/run-1/t0/test.ll>
101 renameat(AT_FDCWD</w/kiln>, "/w/kiln/target/guard/tmp-1/x-1.o.tmp", AT_FDCWD</w/kiln>, "/w/kiln/target/guard/tmp-1/x.o") = 0
101 unlinkat(7</w/kiln/target/xtask-test/run-1/t0>, "test.ll", 0 <unfinished ...>
101 <... unlinkat resumed>)           = 0
100 symlinkat("kiln", AT_FDCWD</w/kiln>, "/w/kiln/target/debug/kiln++") = 0
103 openat(AT_FDCWD</w/kiln>, "/var/cache/x", O_WRONLY|O_CREAT, 0666) = -1 EACCES (Permission denied)
103 openat(AT_FDCWD</w/kiln>, "/proc/self/coredump_filter", O_WRONLY) = 3</proc/44/coredump_filter>
100 --- SIGCHLD {si_signo=SIGCHLD, si_code=CLD_EXITED, si_pid=101, si_status=0} ---
101 +++ exited with 0 +++
"#;

#[test]
fn a_clean_log_passes() {
    let v = check_log(CLEAN, &allow());
    assert!(v.passes(), "{}", v.render());
    assert_eq!((v.calls, v.processes, v.unreadable), (9, 4, 0));
    assert!(v.render().contains("xtask guard: ok: 9 calls of 4 processes"));
}

/// The deliberate failing case the CI job's rule rests on: a canned log
/// with an `AF_INET` socket must fail, whether or not the call succeeded.
#[test]
fn an_inet_socket_fails() {
    for line in [
        "200 socket(AF_INET, SOCK_STREAM|SOCK_CLOEXEC, IPPROTO_TCP) = 3<socket:[123]>",
        "200 socket(AF_INET6, SOCK_DGRAM|SOCK_CLOEXEC, IPPROTO_IP) = -1 EAFNOSUPPORT (x)",
        "200 socket(AF_INET, SOCK_DGRAM, IPPROTO_IP <unfinished ...>",
    ] {
        let log = format!("{CLEAN}{line}\n");
        let v = check_log(&log, &allow());
        assert!(!v.passes(), "{line}");
        assert_eq!(v.network.len(), 1, "{line}");
        assert!(v.network[0].starts_with("AF_INET"), "{:?}", v.network);
        assert!(v.render().contains("xtask guard: FAILED:"));
    }
}

/// No family is allowed today (`ALLOWED_FAMILIES`): a Unix or netlink
/// socket fails too, and so does a connect or bind to an internet address.
#[test]
fn other_families_and_addresses_fail() {
    assert!(ALLOWED_FAMILIES.is_empty());
    for line in [
        "200 socket(AF_UNIX, SOCK_STREAM|SOCK_CLOEXEC|SOCK_NONBLOCK, 0) = 3<socket:[9]>",
        "200 socket(AF_LOCAL, SOCK_STREAM, 0) = 3",
        "200 socket(AF_NETLINK, SOCK_RAW|SOCK_CLOEXEC, NETLINK_ROUTE) = 3",
        "200 connect(3<socket:[9]>, {sa_family=AF_INET, sin_port=htons(443), sin_addr=inet_addr(\"1.2.3.4\")}, 16) = -1 ENETUNREACH (x)",
        "200 bind(3, {sa_family=AF_INET6, sin6_port=htons(0)}, 28) = 0",
    ] {
        let v = check_log(&format!("{CLEAN}{line}\n"), &allow());
        assert_eq!(v.network.len(), 1, "{line}: {}", v.render());
    }
    // A connect to a Unix address is no internet use (its socket call is
    // what the family rule judges).
    let unix = "200 connect(3, {sa_family=AF_UNIX, sun_path=\"/run/x\"}, 110) = 0\n";
    assert!(check_log(&format!("{CLEAN}{unix}"), &allow()).passes());
}

#[test]
fn writes_outside_the_allowlist_fail() {
    for (line, path) in [
        (
            r#"300 openat(AT_FDCWD</w/kiln>, "/tmp/cc1.s", O_RDWR|O_CREAT|O_EXCL, 0600) = 3</tmp/cc1.s>"#,
            "/tmp/cc1.s",
        ),
        (
            r#"300 openat(AT_FDCWD</w/kiln>, "../outside.txt", O_WRONLY|O_APPEND) = 3"#,
            "/w/outside.txt",
        ),
        (
            r#"300 openat(AT_FDCWD</w/kiln>, "/w/kiln/target/../src/x.rs", O_WRONLY|O_TRUNC) = 3"#,
            "/w/kiln/src/x.rs",
        ),
        (r#"300 creat("/srv/u/x", 0644) = 3"#, "/srv/u/x"),
        (r#"300 mkdir("/var/cache/kiln", 0777) = 0"#, "/var/cache/kiln"),
        (r#"300 unlinkat(AT_FDCWD</w/kiln>, "README.md", 0) = 0"#, "/w/kiln/README.md"),
        (
            r#"300 renameat2(AT_FDCWD</w/kiln>, "target/a", AT_FDCWD</w/kiln>, "/etc/a", RENAME_NOREPLACE) = 0"#,
            "/etc/a",
        ),
        (r#"300 linkat(AT_FDCWD</w>, "/etc/passwd", AT_FDCWD</w>, "/w/p", 0) = 0"#, "/w/p"),
        (r#"300 truncate("/w/kiln/Cargo.lock", 0) = 0"#, "/w/kiln/Cargo.lock"),
        (
            r#"300 openat2(AT_FDCWD</w/kiln>, "/var/x", {flags=O_WRONLY|O_CREAT, mode=0644}, 24) = 3"#,
            "/var/x",
        ),
    ] {
        let v = check_log(&format!("{CLEAN}{line}\n"), &allow());
        assert_eq!(v.writes.len(), 1, "{line}: {}", v.render());
        assert!(v.writes[0].starts_with(&format!("{path}: ")), "{line}: {:?}", v.writes);
        assert!(!v.passes());
    }
}

/// A read-only open, a failed write and a `/proc/self` write pass; a
/// relative path with no directory (no `-y`, or the plain x86-64 forms)
/// cannot be judged and fails.
#[test]
fn reads_failures_and_unresolved_paths() {
    let ok = [
        r#"300 openat(AT_FDCWD</w/kiln>, "/etc/ld.so.cache", O_RDONLY|O_CLOEXEC) = 3</etc/ld.so.cache>"#,
        r#"300 open("/etc/passwd", O_RDONLY) = 3"#,
        r#"300 openat(AT_FDCWD</w/kiln>, "/etc/x", O_WRONLY|O_CREAT, 0666) = -1 EACCES (Permission denied)"#,
        r#"300 unlink("/etc/y") = -1 ENOENT (No such file or directory)"#,
        r#"300 openat(AT_FDCWD</w/kiln>, "/proc/self/oom_score_adj", O_WRONLY) = 3"#,
    ];
    for line in ok {
        assert!(check_log(&format!("{CLEAN}{line}\n"), &allow()).passes(), "{line}");
    }
    let v = check_log(&format!("{CLEAN}300 mkdir(\"out\", 0777) = 0\n"), &allow());
    assert_eq!(v.writes.len(), 1);
    assert!(v.writes[0].starts_with("relative path \"out\" with no directory"), "{:?}", v.writes);
}

#[test]
fn an_empty_log_fails() {
    let v = check_log("", &allow());
    assert!(!v.passes());
    assert!(v.render().contains("the log holds no traced call"));
}

#[test]
fn parser_details() {
    let (calls, bad) = parse_log(
        "1 openat(AT_FDCWD</a b>, \"x, \\\"y\\\")\", O_WRONLY|O_CREAT, 0666) = 3</a b/x>\n\
         1 garbage line\n\
         x not a pid\n",
    );
    assert_eq!(bad, 2);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].args, ["AT_FDCWD</a b>", "\"x, \\\"y\\\")\"", "O_WRONLY|O_CREAT", "0666"]);
    assert_eq!(calls[0].ret, "3</a b/x>");
    assert_eq!(unquote(r#""a\x2fb\033c\\""#).as_deref(), Some("a/b\u{1b}c\\"));
    assert_eq!(normalize(Path::new("/a/./b/../c")), PathBuf::from("/a/c"));
    // A call left unfinished at the end of the log is checked as done.
    let (calls, _) = parse_log("5 mkdirat(AT_FDCWD</w>, \"/etc/z\", 0777 <unfinished ...>\n");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].ret, "?");
}

#[test]
fn traced_set_names_every_written_call() {
    let names: Vec<&str> = TRACED.split(',').map(|n| n.trim_start_matches('?')).collect();
    for n in [
        "socket",
        "connect",
        "bind",
        "openat",
        "openat2",
        "open",
        "creat",
        "rename",
        "renameat",
        "renameat2",
        "unlink",
        "unlinkat",
        "rmdir",
        "mkdir",
        "mkdirat",
        "link",
        "linkat",
        "symlink",
        "symlinkat",
        "mknod",
        "mknodat",
        "truncate",
    ] {
        assert!(names.contains(&n), "TRACED lacks {n}");
    }
    assert_eq!(names.len(), 22);
}

// ---------------------------------------------------------------- risky --

fn change(status: char, path: &str) -> Change {
    Change { status, path: path.into() }
}

fn files<'a>(m: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |p| m.iter().find(|(k, _)| *k == p).map(|(_, v)| (*v).to_owned())
}

const OLD_MANIFEST: &str = r#"[package]
name = "kiln_x"
version = "1.3.0"

[dependencies]
kiln_ast = { path = "../kiln_ast" } # local
serde = "1.0"

[dev-dependencies]
pretty = { version = "0.1",
           features = ["a"] }
"#;

#[test]
fn manifest_dependency_entries() {
    let e = manifest_entries(OLD_MANIFEST);
    let keys: Vec<&str> = e.keys().map(String::as_str).collect();
    assert_eq!(keys, ["dependencies.kiln_ast", "dependencies.serde", "dev-dependencies.pretty"]);
    assert_eq!(e["dev-dependencies.pretty"], "{version=\"0.1\",features=[\"a\"]}");
    let e = manifest_entries(
        "[workspace.dependencies]\nfoo = \"1\"\n[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n\
         [dependencies.bar]\nversion = \"2\"\n[patch.crates-io]\nbaz = { git = \"x\" }\n\
         [package]\nbuild = \"gen.rs\"\nversion = \"9\"\n[lib]\nproc-macro = true\n[features]\nx = []\n",
    );
    let keys: Vec<&str> = e.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "dependencies.bar.version",
            "lib.proc-macro",
            "package.build",
            "patch.crates-io.baz",
            "target.cfg(unix).dependencies.libc",
            "workspace.dependencies.foo",
        ]
    );
}

#[test]
fn risky_changes_are_flagged() {
    let new_manifest = OLD_MANIFEST.replace("serde = \"1.0\"", "serde = \"1.1\"\nevil = \"0.1\"")
        + "\n[lib]\nproc-macro = true\n";
    let old_lock = "[[package]]\nname = \"kiln_x\"\nversion = \"1.3.0\"\n\n[[package]]\nname = \"serde\"\n\
                    version = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\
                    checksum = \"abcdef0123456789\"\n";
    let new_lock = old_lock.replace("1.3.0", "1.4.0").replace("1.0.0", "1.1.0");
    let base = [("crates/kiln_x/Cargo.toml", OLD_MANIFEST), ("Cargo.lock", old_lock)];
    let head = [
        ("crates/kiln_x/Cargo.toml", new_manifest.as_str()),
        ("Cargo.lock", new_lock.as_str()),
        (
            "crates/kiln_y/Cargo.toml",
            "[package]\nname = \"kiln_y\"\n[build-dependencies]\ncc = \"1\"\n",
        ),
    ];
    let changes = [
        change('M', "crates/kiln_x/Cargo.toml"),
        change('M', "Cargo.lock"),
        change('A', "crates/kiln_y/Cargo.toml"),
        change('A', "crates/kiln_y/build.rs"),
        change('M', "crates/kiln_x/build.rs"),
        change('M', ".github/workflows/ci.yml"),
        change('D', ".cargo/config.toml"),
        change('M', "crates/kiln_x/src/lib.rs"),
    ];
    let flags = classify(&changes, &files(&base), &files(&head));
    let got: Vec<(Risk, &str, &str)> =
        flags.iter().map(|f| (f.risk, f.file.as_str(), f.detail.as_str())).collect();
    assert_eq!(
        got,
        [
            (
                Risk::Dependency,
                "crates/kiln_x/Cargo.toml",
                "changed `dependencies.serde = \"1.1\"`"
            ),
            (Risk::Dependency, "crates/kiln_x/Cargo.toml", "new `dependencies.evil = \"0.1\"`"),
            (Risk::Dependency, "crates/kiln_y/Cargo.toml", "new `build-dependencies.cc = \"1\"`"),
            (
                Risk::LockedPackage,
                "Cargo.lock",
                "new or changed package serde 1.1.0 (registry+https://github.com/rust-lang/crates.io-index) sha256:abcdef012345"
            ),
            (
                Risk::BuildScript,
                "crates/kiln_y/build.rs",
                "new build script crates/kiln_y/build.rs"
            ),
            (
                Risk::ProcMacro,
                "crates/kiln_x/Cargo.toml",
                "the crate is now a proc-macro (`[lib] proc-macro = true`)"
            ),
            (Risk::Workflow, ".github/workflows/ci.yml", "changes .github/workflows/ci.yml"),
            (Risk::CargoConfig, ".cargo/config.toml", "deletes .cargo/config.toml"),
        ]
    );
    // A workspace crate's version bump and an edited build script are not
    // flagged; nor is an unchanged manifest.
    assert!(!got.iter().any(|(_, _, d)| d.contains("kiln_x 1.4.0")));
    let same = classify(
        &[change('M', "Cargo.toml")],
        &files(&[("Cargo.toml", OLD_MANIFEST)]),
        &files(&[(
            "Cargo.toml",
            &OLD_MANIFEST.replace("version = \"1.3.0\"", "version = \"1.4.0\""),
        )]),
    );
    assert!(same.is_empty(), "{same:?}");

    let s = summary(&flags, "base", "head");
    assert!(s.contains("8 change(s) between `base` and `head`"));
    assert!(s.contains(
        "- **CI workflow** `.github/workflows/ci.yml`: changes .github/workflows/ci.yml"
    ));
    assert!(summary(&[], "b", "h").contains("None between `b` and `h`."));
    assert_eq!(
        annotation(&flags[6]),
        "::warning file=.github/workflows/ci.yml,title=Risky change%3A CI workflow::changes .github/workflows/ci.yml"
    );
}

#[test]
fn name_status_lines() {
    let c = parse_name_status("M\tCargo.lock\nA\tcrates/x/build.rs\n\nD\t.github/a.yml\n");
    assert_eq!(
        c,
        [change('M', "Cargo.lock"), change('A', "crates/x/build.rs"), change('D', ".github/a.yml")]
    );
}
