// [dcl.constexpr]/6 (C++20): clang's never-constant check goes on past an
// expression statement calling a function with an unknown argument (the call
// is refused; `EvaluateIgnoredValue` notes a side effect and continues) and
// reports the static variable after it (WP-1133).
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-c++23-extensions
//@ strict-diags
//@ diag: 13:15 error "constexpr function never produces a constant expression"
//@ diag: 13:48 note "control flows through the definition of a static variable"

constexpr void callee(int) {}
constexpr int f(int x) { callee(x); static int i = 0; return i; }
