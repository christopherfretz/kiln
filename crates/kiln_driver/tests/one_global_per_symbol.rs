//! One LLVM global per symbol (spec/codegen.md §7a "One global per
//! symbol", WP-1620): a unit that defines the C++ runtime's entry points
//! (as libc++abi and libc++ do), before or after code that made codegen
//! declare them by name, defines each once under its own symbol, and no
//! `name.N` copy of a non-local symbol appears. The IR differs from
//! clang's only in attributes (clang keeps the runtime declaration's), so
//! this checks symbols rather than comparing with clang. No header and no
//! pinned target: the source is target-neutral.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

const PRELUDE: &str = r"typedef decltype(sizeof 0) size_t;
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *n;
};
} // namespace std
";
const USERS: &str = r"struct A { virtual void f() = 0; virtual void g() = delete; virtual ~A(); };
struct B : A { void f() override; };
A::~A() {}
void B::f() {}
struct E { int x; };
struct G { ~G(); };
G::~G() {}
int cleanup_term() noexcept;
void may_throw();
void thrower() { throw E{1}; }
int catcher() {
  try { may_throw(); } catch (E &e) { return e.x; }
  return 0;
}
int cleanup_term() noexcept { G g; may_throw(); return 0; }
B *down(A *a) { return dynamic_cast<B *>(a); }
int *alloc() { return new int(1); }
void dealloc(int *p) { delete p; }
int &guarded() { static G g; static int i = cleanup_term(); return i; }
G global_g;
thread_local G tl_g;
G &tl() { return tl_g; }
void rethrow() { try { may_throw(); } catch (...) { throw; } }
B &down_ref(A &a) { return dynamic_cast<B &>(a); }
const std::type_info &dyn_type(A *a) { return typeid(*a); }
";
const DEFS: &str = r#"extern "C" {
void __cxa_pure_virtual() {}
void __cxa_deleted_virtual() {}
void *__cxa_allocate_exception(size_t n) noexcept { return nullptr; }
void __cxa_throw(void *, std::type_info *, void (*)(void *)) {}
void *__cxa_begin_catch(void *) noexcept { return nullptr; }
void __cxa_end_catch() {}
void *__dynamic_cast(const void *, const void *, const void *, long) { return nullptr; }
int __cxa_guard_acquire(long long *) { return 0; }
void __cxa_guard_release(long long *) noexcept {}
void __cxa_guard_abort(long long *) noexcept {}
int __cxa_atexit(void (*)(void *), void *, void *) { return 0; }
int __cxa_thread_atexit(void (*)(void *), void *, void *) { return 0; }
void __cxa_rethrow() {}
void __cxa_bad_cast() {}
void __cxa_bad_typeid() {}
}
namespace std { [[noreturn]] void terminate() noexcept { for (;;) {} } }
void *operator new(size_t n) { return reinterpret_cast<void *>(n); }
void operator delete(void *, size_t) noexcept {}
"#;

/// The runtime symbols `DEFS` defines.
const SYMBOLS: [&str; 18] = [
    "__cxa_pure_virtual",
    "__cxa_deleted_virtual",
    "__cxa_allocate_exception",
    "__cxa_throw",
    "__cxa_begin_catch",
    "__cxa_end_catch",
    "__dynamic_cast",
    "__cxa_guard_acquire",
    "__cxa_guard_release",
    "__cxa_guard_abort",
    "__cxa_atexit",
    "__cxa_thread_atexit",
    "__cxa_rethrow",
    "__cxa_bad_cast",
    "__cxa_bad_typeid",
    "_ZSt9terminatev",
    "_Znwm",
    "_ZdlPvm",
];

fn ir_of(name: &str, src: &str) -> String {
    let d = std::env::temp_dir().join(format!("kiln-one-global-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("t.cpp"), src).unwrap();
    let o = Command::new(kilnpp())
        .args(["-std=c++26", "-w", "-fexceptions", "-S", "-emit-llvm", "-o", "-", "t.cpp"])
        .current_dir(&d)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}

fn check(ir: &str) {
    for s in SYMBOLS {
        let defs = ir
            .lines()
            .filter(|l| l.starts_with("define ") && l.contains(&format!(" @{s}(")))
            .count();
        assert_eq!(defs, 1, "one definition of {s}:\n{ir}");
        assert!(
            !ir.lines().any(|l| l.starts_with("declare ") && l.contains(&format!(" @{s}("))),
            "no declaration beside the definition of {s}:\n{ir}"
        );
        assert!(!ir.contains(&format!("@{s}.")), "no renamed copy of {s}:\n{ir}");
    }
}

/// [except.throw], [except.handle], [expr.dynamic.cast], [stmt.dcl]/3,
/// [basic.start.term], [class.abstract], [expr.new], [expr.delete]: the
/// uses come first (codegen declares the runtime functions by name), then
/// the definitions.
#[test]
fn runtime_functions_defined_after_use() {
    check(&ir_of("after", &format!("{PRELUDE}{USERS}{DEFS}")));
}

/// The same definitions before the uses.
#[test]
fn runtime_functions_defined_before_use() {
    check(&ir_of("before", &format!("{PRELUDE}{DEFS}{USERS}")));
}
