// [dcl.constexpr]/6 (C++20): clang's never-constant walk tracks the values
// of locals and of a condition variable, and evaluates a discarded
// parameter without noting a side effect (WP-1150), so it reaches and
// reports these static variables.
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -ferror-limit=0
//@ strict-diags
//@ diag: 26:15 error "constexpr function never produces a constant expression"
//@ diag: 26:73 note "control flows through the definition of a static variable"
//@ diag: 27:15 error "constexpr function never produces a constant expression"
//@ diag: 27:74 note "control flows through the definition of a static variable"
//@ diag: 28:15 error "constexpr function never produces a constant expression"
//@ diag: 28:55 note "control flows through the definition of a static variable"
//@ diag: 29:15 error "constexpr function never produces a constant expression"
//@ diag: 29:93 note "control flows through the definition of a static variable"
//@ diag: 30:15 error "constexpr function never produces a constant expression"
//@ diag: 30:104 note "control flows through the definition of a static variable"
//@ diag: 31:15 error "constexpr function never produces a constant expression"
//@ diag: 31:80 note "control flows through the definition of a static variable"
//@ diag: 32:15 error "constexpr function never produces a constant expression"
//@ diag: 32:99 note "control flows through the definition of a static variable"

struct S { int v; };

constexpr int discarded_parameter(int x) { x; { int y = 0; } static int i = 0; return i; }
constexpr int void_parameter(int x) { (void)x; { int y = 0; } static int i = 0; return i; }
constexpr int comma_temp(int x) { x, S{1}; static int i = 0; return i; }
constexpr int for_local_condition(int x) { for (bool b = true; b; b = false) { } static int i = 0; return i; }
constexpr int local_flag(int x) { bool f = true; while (true) { if (!f) break; f = false; } static int i = 0; return i; }
constexpr int counted_loop(int x) { for (int k = 0; k < 3; ++k) { } static int i = 0; return i; }
constexpr int switch_condition_variable(int x) { switch (int k = 1) { case 1: break; } static int i = 0; return i; }
