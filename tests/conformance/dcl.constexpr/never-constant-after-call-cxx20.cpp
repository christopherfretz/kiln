// [dcl.constexpr]/6 (C++20): clang's never-constant check goes on past an
// expression statement calling a function with an unknown argument
// (`EvaluateIgnoredValue` notes a side effect and continues) and reports the
// static variable after it. kiln's walk cannot tell such a call from one
// whose callee fails, after which clang stops quietly, so it stops too.
//@ xfail(kiln): WP-1123 follow-up, never-constant walk stops after a call with unknown arguments (Sema::nc_note_unknown)
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-c++23-extensions
//@ strict-diags
//@ diag: 15:15 error "constexpr function never produces a constant expression"
//@ diag: 15:48 note "control flows through the definition of a static variable"

constexpr void callee(int) {}
constexpr int f(int x) { callee(x); static int i = 0; return i; }
