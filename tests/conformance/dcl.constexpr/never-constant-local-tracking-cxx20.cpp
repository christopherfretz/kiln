// [dcl.constexpr]/6 (C++20): clang's never-constant walk
// (`isPotentialConstantExpr`) evaluates the locals and condition variables
// of a constexpr function while its parameters stay unknown: a branch,
// loop or switch over a known local is followed, a failing evaluation is
// reported, and an endless loop over locals hits the step limit. A local
// initialized from a parameter, an uninitialized read, and any local read
// after a side effect are unknown, so those functions are not diagnosed
// (WP-1150).
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -Wno-gnu-statement-expression -Wno-uninitialized -ferror-limit=0
//@ strict-diags
//@ diag: 47:15 error "constexpr function never produces a constant expression"
//@ diag: 47:85 note "control flows through the definition of a static variable"
//@ diag: 48:15 error "constexpr function never produces a constant expression"
//@ diag: 48:98 note "control flows through the definition of a static variable"
//@ diag: 49:15 error "constexpr function never produces a constant expression"
//@ diag: 49:86 note "control flows through the definition of a static variable"
//@ diag: 50:15 error "constexpr function never produces a constant expression"
//@ diag: 50:74 note "control flows through the definition of a static variable"
//@ diag: 51:15 error "constexpr function never produces a constant expression"
//@ diag: 51:95 note "control flows through the definition of a static variable"
//@ diag: 52:15 error "constexpr function never produces a constant expression"
//@ diag: 52:90 note "control flows through the definition of a static variable"
//@ diag: 53:15 error "constexpr function never produces a constant expression"
//@ diag: 53:65 note "control flows through the definition of a static variable"
//@ diag: 54:15 error "constexpr function never produces a constant expression"
//@ diag: 54:88 note "control flows through the definition of a static variable"
//@ diag: 55:15 error "constexpr function never produces a constant expression"
//@ diag: 55:89 note "control flows through the definition of a static variable"
//@ diag: 56:15 error "constexpr function never produces a constant expression"
//@ diag: 56:101 note "control flows through the definition of a static variable"
//@ diag: 57:15 error "constexpr function never produces a constant expression"
//@ diag: 57:87 note "control flows through the definition of a static variable"
//@ diag: 58:15 error "constexpr function never produces a constant expression"
//@ diag: 58:66 note "control flows through the definition of a static variable"
//@ diag: 59:15 error "constexpr function never produces a constant expression"
//@ diag: 59:54 note "division by zero"
//@ diag: 60:15 error "constexpr function never produces a constant expression"
//@ diag: 60:66 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 61:15 error "constexpr function never produces a constant expression"
//@ diag: 61:58 note "constexpr evaluation hit maximum step limit; possible infinite loop?"

struct S { int v; };

// Diagnosed: the walk knows these locals and condition variables.
constexpr int modified(int x) { int k = 0; k += 5; if (k != 5) return 0; static int i = 0; return i; }
constexpr int array(int x) { int a[3] = {1, 2, 3}; a[1] = 5; if (a[1] != 5) return 0; static int i = 0; return i; }
constexpr int aggregate(int x) { S s{1}; s.v = 4; if (s.v != 4) return 0; static int i = 0; return i; }
constexpr int assigned_later(int x) { int k; k = 1; if (k) {} static int i = 0; return i; }
constexpr int increments(int x) { int k = 0; k++; ++k; k--; k *= 3; if (k == 3) {} static int i = 0; return i; }
constexpr int reference(int x) { int k = 1; int &r = k; r = 2; if (k == 2) {} static int i = 0; return i; }
constexpr int if_variable(int x) { if (int k = 1) {} static int i = 0; return i; }
constexpr int while_variable(int x) { int n = 2; while (int k = n) { --n; } static int i = 0; return i; }
constexpr int switch_variable(int x) { switch (int k = 1) { case 1: break; } static int i = 0; return i; }
constexpr int loop_local(int x) { for (int k = 0; k < 2; ++k) { int j = k; if (j == 1) { static int i = 0; return i; } } return 0; }
constexpr int after_parameter(int x) { int k = 1; x; k = 2; if (k == 2) {} static int i = 0; return i; }
constexpr int unknown_then_static(int x) { int k = x; static int i = 0; return i; }
constexpr int division(int x) { int k = 0; int z = 1 / k; return z; }
constexpr int past_end(int x) { int a[2] = {}; int k = 2; return a[k]; }
constexpr int forever(int x) { int k = 0; while (k == 0) {} return 1; }

// Not diagnosed: a value from a parameter is unknown, and after a side
// effect clang fails quietly to read any local.
constexpr int from_parameter(int x) { int k = x; while (k) {} static int i = 0; return i; }
constexpr int assigned_parameter(int x) { int k = 0; k = x; if (k) return 0; static int i = 0; return i; }
constexpr int reassigned(int x) { int k = 0; k = x; k = 1; if (k) return 0; static int i = 0; return i; }
constexpr int uninitialized(int x) { int k; if (k) {} static int i = 0; return i; }
constexpr int after_side_effect(int x) { int k = 0; x + 1; if (k == 0) {} static int i = 0; return i; }
constexpr int after_unknown(int x) { int k = x; int j = 0; if (j == 0) {} static int i = 0; return i; }
constexpr int switch_unknown(int x) { switch (int k = x) { case 1: break; } static int i = 0; return i; }
