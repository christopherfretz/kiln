//! The run-time objects of P3491R3's `define_static_string` /
//! `define_static_array` (spec/reflection-values.md §4.4, WP-479): each
//! frozen array is emitted once, with internal linkage and a deterministic
//! name; a character array shares the string pool with an equal string
//! literal. An `ir` corpus test cannot pin this (those always compare with
//! clang, which has no reflection), so the IR is read here.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

/// The intrinsics `define_static_*` are built on, declared directly so the
/// test does not depend on the library header.
const SRC: &str = r#"
namespace std::meta {
using info = decltype(^^int);
template <class T> consteval T extract(info r);
template <class R> consteval info __kiln_reflect_constant_string(R&& r);
template <class R> consteval info __kiln_reflect_constant_array(R&& r);
}
using namespace std::meta;
constexpr int arr[] = {3, 1, 4};
constexpr int same[] = {3, 1, 4};
const char* s() { return extract<const char*>(__kiln_reflect_constant_string("hi there")); }
const char* lit() { return "hi there"; }
const int* a() { return extract<const int*>(__kiln_reflect_constant_array(arr)); }
const int* b() { return extract<const int*>(__kiln_reflect_constant_array(same)); }
constexpr const int* g = extract<const int*>(__kiln_reflect_constant_array(arr));
const int* c() { return g; }
"#;

fn emit(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("kiln-define-static-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("t.cpp"), SRC).unwrap();
    let o = Command::new(kilnpp())
        .args(["-std=c++26", "-freflection", "-S", "-emit-llvm", "-o", "t.ll", "t.cpp"])
        .current_dir(&d)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let ll = std::fs::read_to_string(d.join("t.ll")).unwrap();
    let _ = std::fs::remove_dir_all(&d);
    ll
}

fn globals(ll: &str) -> Vec<&str> {
    ll.lines().filter(|l| l.starts_with('@')).collect()
}

#[test]
fn static_objects_are_internal_unique_and_deterministic() {
    let ll = emit("a");
    let g = globals(&ll);
    // One string-pool global, shared by the literal and the promoted string.
    let strs: Vec<_> = g.iter().filter(|l| l.contains("c\"hi there\\00\"")).collect();
    assert_eq!(strs.len(), 1, "{ll}");
    assert!(strs[0].starts_with("@.str = private unnamed_addr constant [9 x i8]"), "{ll}");
    // One array object for equal contents, internal.
    let arrs: Vec<_> = g.iter().filter(|l| l.contains("[i32 3, i32 1, i32 4]")).collect();
    let consts: Vec<_> = arrs.iter().filter(|l| l.starts_with("@.constant")).collect();
    assert_eq!(consts.len(), 1, "{ll}");
    assert!(consts[0].starts_with("@.constant = private unnamed_addr constant [3 x i32]"), "{ll}");
    for f in ["s", "lit"] {
        let body = ll.split(&format!("@_Z{}{f}v()", f.len())).nth(1).unwrap_or("");
        assert!(body.lines().take(4).any(|l| l.contains("ret ptr @.str")), "{f}: {ll}");
    }
    for f in ["a", "b"] {
        let body = ll.split(&format!("@_Z1{f}v()")).nth(1).unwrap_or("");
        assert!(body.lines().take(4).any(|l| l.contains("ret ptr @.constant")), "{f}: {ll}");
    }
    // The names do not depend on the run.
    assert_eq!(globals(&emit("b")), g);
}
