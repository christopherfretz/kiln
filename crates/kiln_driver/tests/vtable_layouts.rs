//! Differential test of `kiln -fsyntax-only -fdump-vtable-layouts`
//! (`spec/driver.md`, `spec/abi.md` §8.5). clang dumps a vtable only
//! when it emits it, so its side is `clang++-19 -S -emit-llvm -Xclang
//! -fdump-vtable-layouts` on programs whose every dynamic class has an
//! out-of-line key function. The order differs (kiln: definition order;
//! clang: emission order, construction vtables possibly twice) and
//! clang's `Original map` sections are debugging state kiln does not
//! print, so the dumps are compared as *sets* of blocks with those
//! sections removed. Without `clang++-19` only kiln's side runs.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::process::{Command, Stdio};

const CLANG: &str = "clang++-19";
const TRIPLES: [&str; 2] = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];
const HEADS: [&str; 5] = [
    "Vtable for ",
    "Construction vtable for ",
    "VTable indices for ",
    "Thunks for ",
    "Virtual base offset offsets for ",
];

/// Programs without constructors/destructors (not yet in sema); every
/// dynamic class has a key function defined out of line.
const PROGRAMS: [&str; 3] = [
    // Non-virtual this thunk.
    "struct A { virtual void f(); int a; };\n\
     struct B { virtual void f(); int b; };\n\
     struct C : A, B { void f(); };\n\
     void A::f() {} void B::f() {} void C::f() {}\n",
    // Diamond over a virtual base: vcall offsets, construction vtables.
    "struct V { virtual void g(); int v; };\n\
     struct L : virtual V { void g(); int l; };\n\
     struct R : virtual V { int r; virtual void m(); };\n\
     struct D : L, R { virtual void k(); };\n\
     void V::g() {} void L::g() {} void R::m() {} void D::k() {}\n",
    // Covariant return thunk, pure virtual, const method.
    "struct Q { int q; }; struct Z { int z; }; struct W : Z, Q { };\n\
     struct T0 { virtual Q* h(); };\n\
     struct T1 : T0 { W* h(); };\n\
     Q* T0::h() { return 0; } W* T1::h() { return 0; }\n\
     struct X { virtual void f() const; virtual void g() = 0; };\n\
     struct Y : X { void g() override; };\n\
     void X::f() const {} void Y::g() {}\n",
];

fn clang_available() -> bool {
    Command::new(CLANG)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The set of dump blocks of `text`, `Original map` sections dropped and
/// trailing blank lines trimmed.
fn blocks(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut cur: Vec<&str> = Vec::new();
    let mut in_map = false;
    let mut flush = |cur: &mut Vec<&str>| {
        while cur.last().is_some_and(|l| l.is_empty()) {
            cur.pop();
        }
        if !cur.is_empty() {
            out.insert(cur.join("\n"));
        }
        cur.clear();
    };
    for line in text.lines() {
        if line == "Original map" {
            in_map = true;
            continue;
        }
        if in_map {
            if line.starts_with(' ') {
                continue;
            }
            in_map = false;
        }
        if HEADS.iter().any(|h| line.starts_with(h)) {
            flush(&mut cur);
        }
        if !cur.is_empty() || !line.is_empty() {
            cur.push(line);
        }
    }
    flush(&mut cur);
    out
}

fn kiln_dump(src: &std::path::Path, triple: &str) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(["-std=c++2c", "-fsyntax-only", "-fdump-vtable-layouts"])
        .arg(format!("--target={triple}"))
        .arg(src)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn vtable_layouts_match_clang() {
    let clang = clang_available();
    if !clang {
        eprintln!("{CLANG} not found: checking kiln's side only");
    }
    let dir = std::env::temp_dir().join(format!("kiln-vtable-dump-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = Vec::new();
    let mut checked = 0usize;
    for (i, prog) in PROGRAMS.iter().enumerate() {
        let src = dir.join(format!("p{i}.cpp"));
        std::fs::write(&src, prog).unwrap();
        for triple in TRIPLES {
            let ours = blocks(&kiln_dump(&src, triple));
            assert!(ours.iter().any(|b| b.starts_with("Vtable for ")), "p{i} {triple}: no vtable");
            if !clang {
                continue;
            }
            let ll = dir.join(format!("p{i}.ll"));
            let c = Command::new(CLANG)
                .args(["-std=c++2c", "-w", "-S", "-emit-llvm", "-Xclang", "-fdump-vtable-layouts"])
                .arg(format!("--target={triple}"))
                .arg("-o")
                .arg(&ll)
                .arg(&src)
                .output()
                .unwrap();
            assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
            let _ = std::fs::remove_file(&ll);
            let theirs = blocks(&String::from_utf8_lossy(&c.stdout));
            checked += theirs.len();
            if ours != theirs {
                let only_k: Vec<_> = ours.difference(&theirs).collect();
                let only_c: Vec<_> = theirs.difference(&ours).collect();
                failures.push(format!(
                    "p{i} {triple}:\n--- only kiln ---\n{only_k:#?}\n--- only clang ---\n{only_c:#?}"
                ));
            }
        }
        let _ = std::fs::remove_file(&src);
    }
    let _ = std::fs::remove_dir(&dir);
    eprintln!("vtable_layouts: {checked} clang blocks checked");
    for f in &failures {
        eprintln!("{f}");
    }
    assert!(failures.is_empty(), "{} runs differ", failures.len());
}

/// The flag needs `-fsyntax-only`; a non-dynamic class dumps nothing.
#[test]
fn flag_action() {
    let dir = std::env::temp_dir().join(format!("kiln-vtable-flag-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("s.cpp");
    std::fs::write(&src, "struct S { int i; };\nstruct P { virtual void f(); };\n").unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(["-fsyntax-only", "-fdump-vtable-layouts", "--target=x86_64-unknown-linux-gnu"])
        .arg(&src)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.starts_with("Vtable for 'P' (3 entries).\n"), "{out}");
    assert!(!out.contains("'S'"), "{out}");
    let o = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(["-S", "-emit-llvm", "-o", "-", "-fdump-vtable-layouts"])
        .arg(&src)
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("-fdump-vtable-layouts without -fsyntax-only")
    );
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_dir(&dir);
}
