//! Differential test of `kiln -fsyntax-only -fdump-record-layouts`
//! (`spec/driver.md`, `spec/abi.md` §8): every program of the corpus
//! `tests/layout/*.cpp` is compiled by kiln and by `clang++-19 -Xclang
//! -fdump-record-layouts-complete` for each supported target, and the
//! dumps must be identical block for block, in order. clang's builtin
//! records (`__NSConstantString_tag`, `__va_list_tag`, `std::__va_list`)
//! are dropped from its output. Without `clang++-19` only kiln's side
//! runs (it must succeed and dump at least one class per program).
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kiln_abi::dump_blocks::{RECORD_LAYOUT_MARKER as MARKER, record_layout_blocks as blocks};

const CLANG: &str = "clang++-19";
const TRIPLES: [&str; 2] = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];
/// Concurrent workers.
const JOBS: usize = 4;

fn corpus() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/layout");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "cpp"))
        .collect();
    files.sort();
    files
}

fn clang_available() -> bool {
    Command::new(CLANG)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Checks one program on one target; `Err` describes the mismatch.
fn check(file: &Path, triple: &str, clang: bool) -> Result<(), String> {
    let name = file.display();
    let target = format!("--target={triple}");
    let k = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(["-std=c++2c", "-fsyntax-only", "-fdump-record-layouts", &target])
        .arg(file)
        .output()
        .map_err(|e| e.to_string())?;
    let ours = String::from_utf8_lossy(&k.stdout).into_owned();
    if !k.status.success() {
        return Err(format!(
            "{name} {triple}: kiln failed:\n{}",
            String::from_utf8_lossy(&k.stderr)
        ));
    }
    let ours_blocks = blocks(&ours);
    if ours_blocks.is_empty() {
        return Err(format!("{name} {triple}: kiln dumped no class"));
    }
    if !clang {
        return Ok(());
    }
    let c = Command::new(CLANG)
        .args(["-std=c++2c", "-fsyntax-only", "-w", "-Xclang", "-fdump-record-layouts-complete"])
        .arg(&target)
        .arg(file)
        .output()
        .map_err(|e| e.to_string())?;
    if !c.status.success() {
        return Err(format!(
            "{name} {triple}: clang failed:\n{}",
            String::from_utf8_lossy(&c.stderr)
        ));
    }
    let theirs = String::from_utf8_lossy(&c.stdout).into_owned();
    let theirs_blocks = blocks(&theirs);
    if ours_blocks == theirs_blocks {
        return Ok(());
    }
    Err(format!(
        "{name} {triple}: dumps differ\n--- kiln ---\n{}\n--- clang ---\n{}",
        ours_blocks.join(MARKER),
        theirs_blocks.join(MARKER)
    ))
}

#[test]
fn record_layouts_match_clang() {
    let clang = clang_available();
    if !clang {
        eprintln!("{CLANG} not found: checking kiln's side only");
    }
    let files = corpus();
    assert!(files.len() >= 40, "corpus has {} programs", files.len());
    let jobs: Vec<(&Path, &str)> =
        files.iter().flat_map(|f| TRIPLES.iter().map(move |t| (f.as_path(), *t))).collect();
    let failures: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .chunks(jobs.len().div_ceil(JOBS))
            .map(|chunk| {
                s.spawn(move || {
                    chunk.iter().filter_map(|(f, t)| check(f, t, clang).err()).collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("worker")).collect()
    });
    for f in &failures {
        eprintln!("{f}");
    }
    assert!(failures.is_empty(), "{} of {} runs failed", failures.len(), jobs.len());
}

/// Both spellings are accepted; the flag needs `-fsyntax-only`.
#[test]
fn flag_spellings_and_action() {
    let dir = std::env::temp_dir().join(format!("kiln-layout-flag-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("s.cpp");
    std::fs::write(&src, "struct S { char c; int i; };\n").unwrap();
    for flag in ["-fdump-record-layouts", "-fdump-record-layouts-complete"] {
        let o = Command::new(env!("CARGO_BIN_EXE_kiln"))
            .args(["-fsyntax-only", flag, "--target=x86_64-unknown-linux-gnu"])
            .arg(&src)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(
            String::from_utf8_lossy(&o.stdout),
            "\n*** Dumping AST Record Layout\n         0 | struct S\n         0 |   char c\n         \
             4 |   int i\n           | [sizeof=8, dsize=8, align=4,\n           |  nvsize=8, \
             nvalign=4]\n"
        );
    }
    let o = Command::new(env!("CARGO_BIN_EXE_kiln"))
        .args(["-S", "-emit-llvm", "-o", "-", "-fdump-record-layouts"])
        .arg(&src)
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("-fdump-record-layouts without -fsyntax-only")
    );
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_dir(&dir);
}
